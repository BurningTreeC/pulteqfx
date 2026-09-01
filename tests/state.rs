//! What survives a save and reload.

use nih_plug::prelude::*;
use pulteqfx::params::{Oversampling, PultEqFxParams};

#[test]
fn oversampling_is_an_ordinary_parameter() {
    let params = PultEqFxParams::default();
    // Everything in the parameter map is written into the plugin state by the
    // wrapper, so being in here is what makes a setting persistent.
    let ids: Vec<String> = params
        .param_map()
        .into_iter()
        .map(|(id, _, _)| id)
        .collect();
    assert!(ids.contains(&"os".to_string()), "got {ids:?}");
    assert_eq!(params.oversampling.value(), Oversampling::X4);
}

#[test]
fn the_window_scale_is_persisted() {
    let params = PultEqFxParams::default();
    assert_eq!(params.editor_state.user_scale_factor(), 1.0);

    // Serialising the persistent fields is what the wrapper does when the host
    // asks for the plugin's state.
    let saved = params.serialize_fields();
    let editor_state = saved
        .get("editor-state")
        .expect("the editor state should be part of the saved state");
    assert!(
        editor_state.contains("scale_factor"),
        "expected the scale factor in {editor_state}"
    );

    // A state saved at 140 % should come back at 140 %.
    let scaled = editor_state.replace("1.0", "1.4");
    assert_ne!(scaled, *editor_state, "the test needs to actually change it");
    let mut restored = std::collections::BTreeMap::new();
    restored.insert("editor-state".to_string(), scaled);

    let params = PultEqFxParams::default();
    params.deserialize_fields(&restored);
    assert_eq!(params.editor_state.user_scale_factor(), 1.4);
}
