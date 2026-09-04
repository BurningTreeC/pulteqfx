//! The panel size, which has to survive a session.
//!
//! A plain round trip through the two calls the host makes when it saves and
//! reloads a session. The bug this is here to catch was not in the drawing or
//! in the menu -- both worked -- but in the figure never reaching the state
//! that gets written down.

use pulteqfx::editor::{default_state, remember_scale};
use pulteqfx::editor::settings::SCALES;
use pulteqfx::params::PultEqFxParams;
use nih_plug::params::Params;

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
    assert_eq!(state.user_scale_factor(), 1.0, "a fresh panel opens at 100 %");
    remember_scale(&state, 1.5);
    assert_eq!(
        state.user_scale_factor(),
        1.5,
        "the size was chosen but the state never heard about it"
    );
    let (w, h) = state.inner_logical_size();
    let (sw, sh) = state.scaled_logical_size();
    println!("{w}x{h} logical, {sw}x{sh} at 150 %");
    assert!(sw > w && sh > h, "the window the host is told to make did not grow");
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
        println!("{scale:.2} saved, {:.2} restored", restored.editor_state.user_scale_factor());
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
