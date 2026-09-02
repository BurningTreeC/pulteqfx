//! Preset storage round trip.
//!
//! This is the only test that touches the environment: it points
//! `XDG_CONFIG_HOME` at a scratch directory so it never writes to the config
//! directory of whoever is running it.

use pulteqfx::params::PultEqFxParams;
use pulteqfx::presets;

fn scratch_dir() -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("pulteqfx-preset-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch directory");
    std::env::set_var("XDG_CONFIG_HOME", &dir);
    dir
}

#[test]
fn presets_round_trip_through_the_config_directory() {
    let dir = scratch_dir();
    let params = PultEqFxParams::default();

    // Only the built-in preset to begin with.
    let presets = presets::load_all(&params);
    assert_eq!(presets.len(), 1);
    assert_eq!(presets[0].name, "Low End Punch");
    assert!(presets[0].built_in);
    // Saving under a factory name replaces nothing: the factory preset is
    // compiled in and stays where it is, so this asks only about your files.
    assert!(
        !presets::name_taken("low end punch", &presets),
        "a factory name is not taken, since saving over it replaces no file"
    );

    // Saving the current settings and reading them back gives the same values.
    let captured = presets::capture(&params, "  Bright Vocal  ");
    assert_eq!(captured.name, "Bright Vocal", "names are trimmed");
    assert!(
        !captured.values.contains_key("os"),
        "the oversampling setting is not part of a preset"
    );
    presets::save(&captured).expect("save");

    let presets = presets::load_all(&params);
    assert_eq!(presets.len(), 2);
    let reloaded = presets
        .iter()
        .find(|preset| preset.name == "Bright Vocal")
        .expect("the saved preset should come back");
    assert!(!reloaded.built_in);
    assert_eq!(reloaded.values, captured.values);

    assert!(
        presets::name_taken("bright vocal", &presets),
        "matching one of your own names ignores case"
    );

    // A saved preset under a factory name sits beside it rather than hiding
    // it. A factory preset cannot be edited or deleted, so dropping it from
    // the list would put it permanently out of reach.
    let shadow = presets::capture(&params, "Low End Punch");
    presets::save(&shadow).expect("save");
    let presets = presets::load_all(&params);
    assert_eq!(presets.len(), 3, "the factory preset is still listed");
    let both: Vec<_> = presets
        .iter()
        .filter(|preset| preset.name == "Low End Punch")
        .collect();
    assert_eq!(both.len(), 2, "one factory, one saved");
    assert!(
        both.iter().any(|preset| preset.built_in) && both.iter().any(|preset| !preset.built_in),
        "the pair should be one of each"
    );

    // Names that would be awkward as file names still save and reload.
    let odd = presets::capture(&params, "kick / bass \"trick\"");
    presets::save(&odd).expect("save");
    let presets = presets::load_all(&params);
    assert!(presets.iter().any(|preset| preset.name == "kick / bass \"trick\""));

    // Deleting takes the file away and leaves the factory preset behind.
    presets::delete("Low End Punch").expect("delete");
    let presets = presets::load_all(&params);
    let punch: Vec<_> = presets
        .iter()
        .filter(|preset| preset.name == "Low End Punch")
        .collect();
    assert_eq!(punch.len(), 1, "only the factory one is left");
    assert!(punch[0].built_in);
    assert!(
        presets::delete("Low End Punch").is_err(),
        "a factory preset has no file to remove"
    );

    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_preset_stops_matching_once_something_is_turned() {
    let params = PultEqFxParams::default();

    // Nothing loaded: there is nothing to compare against, so nothing to mark.
    assert!(presets::matches(&params, &Default::default()));

    // The panel matches a preset taken from it a moment ago.
    let captured = presets::capture(&params, "as found");
    assert!(presets::matches(&params, &captured.values));

    // The factory preset is not what the panel is currently set to.
    let punch = presets::load_all(&params)
        .into_iter()
        .find(|preset| preset.name == "Low End Punch")
        .expect("built in");
    assert!(!presets::matches(&params, &punch.values));

    // A parameter the preset does not mention cannot make it stop matching.
    let mut partial = captured.values.clone();
    partial.remove("loboost");
    assert!(presets::matches(&params, &partial));
}
