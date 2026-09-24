//! The panel size, which has to survive a session.
//!
//! A plain round trip through the two calls the host makes when it saves and
//! reloads a session. The bug this is here to catch was not in the drawing or
//! in the menu -- both worked -- but in the figure never reaching the state
//! that gets written down.

use nih_plug::params::Params;
use pulteqfx::editor::settings::SCALES;
use pulteqfx::editor::{default_state, remember_scale};
use pulteqfx::params::PultEqFxParams;

fn fresh() -> PultEqFxParams {
    PultEqFxParams::default()
}

/// Setting the size has to change the state the host reads, not just what
/// vizia draws at. `Editor::size` is computed from this, so if it does not
/// move the host sizes the window for the old scale and the panel ends up
/// drawn larger than the window holding it.
#[test]
fn choosing_a_size_reaches_the_state_the_host_reads() {
    let state = default_state();
    assert_eq!(
        state.user_scale_factor(),
        1.0,
        "a fresh panel opens at 100 %"
    );
    remember_scale(&state, 1.5);
    assert_eq!(
        state.user_scale_factor(),
        1.5,
        "the size was chosen but the state never heard about it"
    );
    let (w, h) = state.inner_logical_size();
    let (sw, sh) = state.scaled_logical_size();
    println!("{w}x{h} logical, {sw}x{sh} at 150 %");
    assert!(
        sw > w && sh > h,
        "the window the host is told to make did not grow"
    );
}

/// And it has to come back, at every size the menu offers.
#[test]
fn the_size_survives_a_session() {
    for scale in SCALES {
        let saved = fresh();
        remember_scale(&saved.editor_state, scale);
        let fields = saved.serialize_fields();

        let restored = fresh();
        restored.deserialize_fields(&fields);
        println!(
            "{scale:.2} saved, {:.2} restored",
            restored.editor_state.user_scale_factor()
        );
        assert_eq!(
            restored.editor_state.user_scale_factor(),
            scale,
            "the panel reopened at a different size than it was left at"
        );
    }
}

/// A plugin whose size has never been touched still opens at full size.
#[test]
fn an_untouched_panel_opens_at_full_size() {
    let params = fresh();
    let restored = fresh();
    restored.deserialize_fields(&params.serialize_fields());
    assert_eq!(restored.editor_state.user_scale_factor(), 1.0);
}

/// Choosing a size has to *ask the host to resize the window*, which is a
/// different thing from storing the number and was the half that was missing.
///
/// Only `GuiContext::request_resize` moves a plugin window. nih-plug calls it
/// from one place -- its `WindowModel`, on a `GeometryChanged` -- behind a
/// guard that returns early when the unscaled size and the stored scale are
/// both unchanged. The panel's size function is a constant, so that guard
/// rested entirely on the scale, and the scale had already been written by
/// `remember_scale` before the event arrived. Both halves equal, early return,
/// no request, and a panel drawn at the new size inside a window still at the
/// old one.
mod resize {
    use super::*;
    use nih_plug::prelude::{GuiContext, ParamPtr, PluginApi, PluginState};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};

    struct CountingHost {
        resizes: AtomicUsize,
        state: Arc<nih_plug_vizia::ViziaState>,
        observed: Mutex<Vec<f64>>,
        accepts: bool,
    }

    impl CountingHost {
        fn new(state: Arc<nih_plug_vizia::ViziaState>, accepts: bool) -> Self {
            Self {
                state,
                accepts,
                resizes: AtomicUsize::new(0),
                observed: Mutex::new(Vec::new()),
            }
        }
    }

    impl GuiContext for CountingHost {
        fn plugin_api(&self) -> PluginApi {
            PluginApi::Clap
        }
        fn request_resize(&self) -> bool {
            self.resizes.fetch_add(1, Ordering::Relaxed);
            self.observed
                .lock()
                .unwrap()
                .push(self.state.user_scale_factor());
            self.accepts
        }
        unsafe fn raw_begin_set_parameter(&self, _: ParamPtr) {}
        unsafe fn raw_set_parameter_normalized(&self, _: ParamPtr, _: f32) {}
        unsafe fn raw_end_set_parameter(&self, _: ParamPtr) {}
        fn get_state(&self) -> PluginState {
            unimplemented!("the panel never asks the host for its state")
        }
        fn set_state(&self, _: PluginState) {
            unimplemented!("the panel never hands the host a state")
        }
    }

    #[test]
    fn choosing_a_size_asks_the_host_to_resize_the_window() {
        let state = default_state();
        let host = CountingHost::new(state.clone(), true);

        assert!(pulteqfx::editor::apply_scale(&state, &host, 1.5));
        assert_eq!(*host.observed.lock().unwrap(), [1.5]);

        assert_eq!(
            host.resizes.load(Ordering::Relaxed),
            1,
            "the size was stored but the host was never asked for a window to \
             put it in, which leaves the panel drawn larger than its window"
        );
        assert_eq!(
            state.user_scale_factor(),
            1.5,
            "the host was asked to resize before the size it would read was set"
        );
    }

    #[test]
    fn rejected_resize_restores_the_size_saved_with_the_session() {
        let params = fresh();
        remember_scale(&params.editor_state, 1.25);
        let previous_size = params.editor_state.scaled_logical_size();
        let host = CountingHost::new(params.editor_state.clone(), false);

        assert!(!pulteqfx::editor::apply_scale(
            &params.editor_state,
            &host,
            2.0
        ));
        assert_eq!(*host.observed.lock().unwrap(), [2.0]);
        assert_eq!(params.editor_state.scaled_logical_size(), previous_size);
        let restored = fresh();
        restored.deserialize_fields(&params.serialize_fields());
        assert_eq!(restored.editor_state.user_scale_factor(), 1.25);
    }

    #[test]
    fn repeated_zoom_changes_report_each_new_size_without_redundant_requests() {
        let state = default_state();
        let host = CountingHost::new(state.clone(), true);
        for scale in [0.5, 2.0, 0.75, 1.5, 1.0] {
            assert!(pulteqfx::editor::apply_scale(&state, &host, scale));
            assert!(pulteqfx::editor::apply_scale(&state, &host, scale));
            assert_eq!(state.user_scale_factor(), scale);
        }
        assert_eq!(*host.observed.lock().unwrap(), [0.5, 2.0, 0.75, 1.5, 1.0]);
    }
}

/// Reopening the plugin has to come up at the size that was chosen.
///
/// `ViziaEditor::spawn` reads exactly two things off the stored state --
/// `user_scale_factor()`, which it hands to the window description, and
/// `Editor::size()`, which is `scaled_logical_size()`. Both must already read
/// back the chosen size, or the panel reopens drawing at one scale inside a
/// window built for another.
#[test]
fn the_panel_reopens_at_the_size_it_was_left_at() {
    for scale in SCALES {
        let saved = PultEqFxParams::default();
        remember_scale(&saved.editor_state, scale);
        let fields = saved.serialize_fields();

        let restored = PultEqFxParams::default();
        restored.deserialize_fields(&fields);
        let state = &restored.editor_state;

        assert_eq!(
            state.user_scale_factor(),
            scale,
            "the window would be built to draw at a different scale than chosen"
        );
        let (uw, uh) = state.inner_logical_size();
        assert_eq!(
            state.scaled_logical_size(),
            (
                (uw as f64 * scale).round() as u32,
                (uh as f64 * scale).round() as u32
            ),
            "the window the host is told to make at {scale:.2} does not match \
             the scale the panel will draw at"
        );
    }
}
