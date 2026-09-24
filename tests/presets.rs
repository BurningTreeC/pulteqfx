//! Preset storage round trip.
//!
//! These are the only tests that touch the environment: each points the
//! variables the preset directory comes from -- `XDG_CONFIG_HOME`, and
//! `APPDATA` on Windows -- at a scratch directory of its own, so none ever
//! writes to the presets of whoever is running it. The environment belongs to
//! the whole process and `cargo test` runs tests on threads of one process, so
//! every test that reads presets holds a lock while it does.

use pulteqfx::params::PultEqFxParams;
use pulteqfx::presets::{self, Preset};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard};

static ENVIRONMENT: Mutex<()> = Mutex::new(());

/// A config directory of its own for the life of the guard.
struct Scratch {
    dir: PathBuf,
    _lock: MutexGuard<'static, ()>,
}

impl Scratch {
    fn new(name: &str) -> Self {
        // A test that failed while holding the lock poisons it; the next one
        // is still entitled to run.
        let lock = ENVIRONMENT.lock().unwrap_or_else(|err| err.into_inner());
        let dir = std::env::temp_dir().join(format!(
            "pulteqfx-preset-test-{}-{name}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("scratch directory");
        std::env::set_var("XDG_CONFIG_HOME", &dir);
        std::env::set_var("APPDATA", &dir);
        Self { dir, _lock: lock }
    }

    /// Asked of the plugin rather than spelled out, because where under the
    /// scratch directory they go depends on the platform.
    fn presets(&self) -> PathBuf {
        let dir = presets::preset_dir().expect("a preset directory");
        assert!(dir.starts_with(&self.dir), "presets would go to {dir:?}");
        dir
    }

    /// How many preset files there are.
    fn files(&self) -> usize {
        std::fs::read_dir(self.presets())
            .map(|entries| entries.count())
            .unwrap_or(0)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// A preset of one's own with a value that marks it out from the rest.
fn marked(name: &str, loboost: f32) -> Preset {
    Preset {
        name: name.to_string(),
        values: BTreeMap::from([("loboost".to_string(), loboost)]),
        built_in: false,
        path: None,
    }
}

fn saved<'a>(presets: &'a [Preset], name: &str) -> Vec<&'a Preset> {
    presets
        .iter()
        .filter(|preset| !preset.built_in && preset.name == name)
        .collect()
}

#[test]
fn presets_round_trip_through_the_config_directory() {
    let _scratch = Scratch::new("round-trip");
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
    let path = presets::save(&captured).expect("save");

    let presets = presets::load_all(&params);
    assert_eq!(presets.len(), 2);
    let reloaded = presets
        .iter()
        .find(|preset| preset.name == "Bright Vocal")
        .expect("the saved preset should come back");
    assert!(!reloaded.built_in);
    assert_eq!(reloaded.values, captured.values);
    assert_eq!(
        reloaded.path.as_ref(),
        Some(&path),
        "a saved preset knows its file"
    );

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
    assert!(presets
        .iter()
        .any(|preset| preset.name == "kick / bass \"trick\""));

    // Deleting takes the file away and leaves the factory preset behind.
    let mine = saved(&presets, "Low End Punch")[0].clone();
    presets::delete(&mine).expect("delete");
    let presets = presets::load_all(&params);
    let punch: Vec<_> = presets
        .iter()
        .filter(|preset| preset.name == "Low End Punch")
        .collect();
    assert_eq!(punch.len(), 1, "only the factory one is left");
    assert!(punch[0].built_in);
    assert!(
        presets::delete(punch[0]).is_err(),
        "a factory preset has no file to remove"
    );
}

/// Saving under one of your names in a different case is the replacement the
/// dialog asks about, so it has to replace -- not leave a second file beside
/// the first, as it did on a file system that tells case apart.
#[test]
fn saving_a_name_in_another_case_replaces_the_preset() {
    let scratch = Scratch::new("case");
    let params = PultEqFxParams::default();

    let first = presets::save(&marked("Bright Vocal", 0.2)).expect("save");
    assert!(presets::name_taken(
        "bright vocal",
        &presets::load_all(&params)
    ));
    let second = presets::save(&marked("bright vocal", 0.7)).expect("save");

    assert_eq!(second, first, "the replacement went into the same file");
    assert_eq!(scratch.files(), 1);
    let presets = presets::load_all(&params);
    let replaced = saved(&presets, "bright vocal");
    assert_eq!(replaced.len(), 1);
    assert_eq!(replaced[0].values["loboost"], 0.7);
    assert!(saved(&presets, "Bright Vocal").is_empty());
}

/// Two different names can come out as the same file name. The second must
/// not quietly overwrite the first, which it did with no dialog at all,
/// because as names they are not the same.
#[test]
fn names_that_share_a_file_name_keep_their_own_files() {
    let scratch = Scratch::new("stems");
    let params = PultEqFxParams::default();

    presets::save(&marked("A/B", 0.2)).expect("save");
    let presets = presets::load_all(&params);
    assert!(
        !presets::name_taken("A_B", &presets),
        "A_B is not A/B, so nothing asks before saving it"
    );
    presets::save(&marked("A_B", 0.7)).expect("save");

    assert_eq!(scratch.files(), 2);
    let presets = presets::load_all(&params);
    assert_eq!(saved(&presets, "A/B")[0].values["loboost"], 0.2);
    assert_eq!(saved(&presets, "A_B")[0].values["loboost"], 0.7);
}

/// Replacing follows the preset to whatever file it is in, including one
/// renamed by hand, rather than writing a new file named after the preset.
#[test]
fn saving_replaces_a_preset_in_a_file_renamed_by_hand() {
    let scratch = Scratch::new("renamed");
    let params = PultEqFxParams::default();

    let original = presets::save(&marked("Kick", 0.2)).expect("save");
    let renamed = original.with_file_name("drums.json");
    std::fs::rename(&original, &renamed).expect("rename");

    let path = presets::save(&marked("Kick", 0.7)).expect("save");
    assert_eq!(path, renamed);
    assert_eq!(scratch.files(), 1);
    assert_eq!(
        saved(&presets::load_all(&params), "Kick")[0].values["loboost"],
        0.7
    );
}

/// Where two files already hold names that differ only in case -- as saving
/// used to leave behind -- deleting one removes that one, not whichever the
/// directory happened to list first.
#[test]
fn deleting_removes_the_preset_asked_for_and_no_other() {
    let scratch = Scratch::new("delete");
    let params = PultEqFxParams::default();
    let dir = scratch.presets();
    std::fs::create_dir_all(&dir).expect("preset directory");
    for (file, name, value) in [("Foo.json", "Foo", 0.2), ("foo.json", "foo", 0.7)] {
        let json = serde_json::to_string(&marked(name, value)).expect("json");
        std::fs::write(dir.join(file), json).expect("write");
    }

    for doomed in ["foo", "Foo"] {
        // Case-insensitive file systems cannot hold both files; there is
        // nothing to tell apart on those.
        if scratch.files() < 2 {
            return;
        }
        let presets = presets::load_all(&params);
        let target = saved(&presets, doomed)[0].clone();
        presets::delete(&target).expect("delete");

        let presets = presets::load_all(&params);
        assert!(
            saved(&presets, doomed).is_empty(),
            "{doomed} is still there"
        );
        let other = if doomed == "foo" { "Foo" } else { "foo" };
        assert_eq!(saved(&presets, other).len(), 1, "{other} went with it");

        // Put it back for the other way round.
        let json = serde_json::to_string(&target).expect("json");
        std::fs::write(target.path.expect("a saved preset has a file"), json).expect("write");
    }
}

#[test]
fn a_preset_stops_matching_once_something_is_turned() {
    let _scratch = Scratch::new("matching");
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
