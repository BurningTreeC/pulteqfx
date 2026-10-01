//! The panel size, which has to survive a session.
//!
//! A plain round trip through the two calls the host makes when it saves and
//! reloads a session. The bug this is here to catch was not in the drawing or
//! in the menu -- both worked -- but in the figure never reaching the state
//! that gets written down.

use nice_plug::params::Params;
use pulteqfx::editor::settings::SCALES;
use pulteqfx::editor::{default_state, remember_scale, BASE_SCALE};
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

/// A plugin whose size has never been touched opens at 100 %, which is drawn
/// at the 1.5 base: the menu's percentage and the rendering scale are two
/// different numbers, as they are in GainStageFx.
#[test]
fn an_untouched_panel_opens_at_100_percent_with_a_1_5_base() {
    let params = fresh();
    let restored = fresh();
    restored.deserialize_fields(&params.serialize_fields());
    assert_eq!(BASE_SCALE, 1.5);
    assert_eq!(restored.editor_state.user_scale_factor(), 1.0);
    assert_eq!(restored.editor_state.rendering_scale_factor(), 1.5);
    assert_eq!(restored.editor_state.inner_logical_size(), (1160, 356));
    assert_eq!(restored.editor_state.scaled_logical_size(), (1740, 534));
}

/// Every size in the menu is a percentage of the base, not of the panel's own
/// pixels.
#[test]
fn the_menu_sizes_are_relative_to_the_base() {
    let state = default_state();
    for (zoom, rendering, size) in [
        (0.5, 0.75, (870, 267)),
        (1.0, 1.5, (1740, 534)),
        (1.5, 2.25, (2610, 801)),
        (2.0, 3.0, (3480, 1068)),
    ] {
        assert!(SCALES.contains(&zoom), "{zoom} is not in the menu");
        remember_scale(&state, zoom);
        assert_eq!(state.user_scale_factor(), zoom);
        assert_eq!(state.rendering_scale_factor(), rendering);
        assert_eq!(state.scaled_logical_size(), size, "at {zoom}");
    }
}

/// Choosing a size has to *ask the host to resize the window*, which is a
/// different thing from storing the number and was once the half missing.
///
/// Under nih-plug the editor stored the scale and then called
/// `GuiContext::request_resize`, and had to undo the store by hand when the
/// host refused. Now the menu asks vizia's baseview backend for the new size
/// (`WindowEvent::SetUserScale`), the backend asks the host through
/// `request_user_scale`, and the scale is only adopted -- drawn at, shown on
/// the button, written into the session -- once the window reports the size
/// it actually reached (`resolve_user_scale`, then `UserScaleChanged`). A
/// refusal reports the old size, so nothing has to be put back.
mod resize {
    use vizia_plug::vizia::{request_user_scale, resolve_user_scale};

    /// The panel's own size, which every zoom is a multiple of.
    const PANEL: (u32, u32) = (1160, 356);

    // The backend's zoom is the rendering scale, the base times the menu's
    // percentage: 100 % is 1.5, 150 % is 2.25. `UiState` multiplies by the
    // base on the way out and divides on the way back.

    #[test]
    fn choosing_a_size_asks_the_host_for_that_size() {
        let mut asked = None;
        let accepted = request_user_scale(1.5, 2.25, PANEL, |size| {
            asked = Some((size.width, size.height));
            true
        });
        assert_eq!(accepted, Some(2.25));
        assert_eq!(
            asked,
            Some((2610.0, 801.0)),
            "the host was asked for a window the panel will not fit"
        );
    }

    #[test]
    fn a_refused_resize_keeps_the_size_the_window_has() {
        assert_eq!(request_user_scale(1.875, 3.0, PANEL, |_| false), None);
        // The window stays at 125 % and says so; that is the size kept.
        assert_eq!(resolve_user_scale((2175.0, 667.5), PANEL, 3.0), 1.875);
    }

    #[test]
    fn a_resize_that_lands_keeps_the_size_asked_for() {
        assert_eq!(resolve_user_scale((2610.0, 801.0), PANEL, 2.25), 2.25);
        // 85 %, rounded to whole pixels on the way, which must not read as a
        // refusal.
        assert_eq!(resolve_user_scale((1479.0, 453.9), PANEL, 1.275), 1.275);
    }

    #[test]
    fn repeated_zoom_changes_ask_once_for_each_new_size() {
        let mut current = 1.5;
        let mut calls = 0;
        for zoom in [0.5, 2.0, 0.75, 1.5, 1.0] {
            let requested = zoom * 1.5;
            current = request_user_scale(current, requested, PANEL, |_| {
                calls += 1;
                true
            })
            .unwrap();
            assert_eq!(
                request_user_scale(current, requested, PANEL, |_| panic!("asked twice")),
                None
            );
        }
        assert_eq!(calls, 5);
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
                (uw as f64 * scale * BASE_SCALE).round() as u32,
                (uh as f64 * scale * BASE_SCALE).round() as u32
            ),
            "the window the host is told to make at {scale:.2} does not match \
             the scale the panel will draw at"
        );
    }
}
