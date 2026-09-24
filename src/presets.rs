//! Preset storage.
//!
//! A preset is the panel's parameter values keyed by parameter id. Built-in
//! presets are compiled in and cannot be overwritten; the ones you save go
//! into the user's config directory as one small JSON file each, so they can
//! be copied around and edited by hand.

use nih_plug::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};

/// Parameters a preset leaves alone. Oversampling is a choice about the
/// machine rather than about the sound.
const EXCLUDED: &[&str] = &["os"];

/// Built-in presets, written in the values the panel shows so they can be read
/// off the dial. They are converted to normalised values against the live
/// parameters, so changing a control's range cannot silently move them.
const BUILT_IN: &[(&str, &[(&str, f32)])] = &[(
    // The low end trick: boost and attenuate the same low frequency to lift
    // the bottom and scoop the mud just above it, with a little air on top.
    "Low End Punch",
    &[
        ("power", 1.0),  // ON
        ("eqin", 1.0),   // EQ IN
        ("lofreq", 3.0), // 100 cps
        ("loboost", 6.0),
        ("loatten", 7.0),
        ("bandw", 5.0),
        ("hifreq", 4.0), // 10 kc
        ("hiboost", 3.0),
        ("hiafreq", 1.0), // 10 kc
        ("hiatten", 0.0),
        ("drive", 25.0),
        ("output", 0.0),
    ],
)];

/// A preset: parameter id to normalised value.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Preset {
    pub name: String,
    pub values: BTreeMap<String, f32>,
    /// Compiled in rather than loaded from disk, so it cannot be overwritten.
    #[serde(default, skip)]
    pub built_in: bool,
    /// The file a saved preset was read from. This, not the name, is what
    /// identifies one: two files can hold names that differ only in case, and
    /// a file renamed by hand still shows under the name stored inside it.
    #[serde(skip)]
    pub path: Option<PathBuf>,
}

/// The dial positions of a built-in preset, as the panel shows them. Exposed
/// so the response tests can check that a preset does what its name claims.
pub fn built_in_dials(name: &str) -> Option<&'static [(&'static str, f32)]> {
    BUILT_IN
        .iter()
        .find(|(preset, _)| *preset == name)
        .map(|(_, dials)| *dials)
}

/// Where saved presets live.
///
/// Per platform, because `$XDG_CONFIG_HOME` and `$HOME` are a Unix
/// convention: neither is normally set on Windows, so the Unix rule returned
/// nothing there and saving a preset failed with "no config directory". A host
/// started from a Unix-style shell was worse than that -- it did find `$HOME`,
/// and wrote the presets somewhere no Windows DAW session would look again.
/// Windows keeps per-user application data under `%APPDATA%`, which is the
/// roaming profile, and a preset is exactly the kind of thing that should
/// roam.
pub fn preset_dir() -> Option<PathBuf> {
    // `cfg!` rather than `#[cfg]`, so both rules are compiled on both
    // platforms and the Windows one can be tested from a Linux machine, which
    // is where this is developed.
    if cfg!(windows) {
        windows_preset_dir(std::env::var_os("APPDATA"), std::env::var_os("USERPROFILE"))
    } else {
        xdg_preset_dir(
            std::env::var_os("XDG_CONFIG_HOME"),
            std::env::var_os("HOME"),
        )
    }
}

/// `%APPDATA%\PultEQFx\Presets`, falling back to deriving the roaming
/// directory from `%USERPROFILE%` for the rare host that clears `APPDATA`.
///
/// Takes the two variables rather than reading them, so the rule is a pure
/// function and its tests do not have to touch the environment that other
/// tests are reading at the same time.
fn windows_preset_dir(appdata: Option<OsString>, userprofile: Option<OsString>) -> Option<PathBuf> {
    let base = appdata
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(|| {
            userprofile
                .map(PathBuf::from)
                .filter(|path| path.is_absolute())
                .map(|home| home.join("AppData").join("Roaming"))
        })?;
    Some(base.join("PultEQFx").join("Presets"))
}

/// `$XDG_CONFIG_HOME/pulteqfx/presets`, or `~/.config` below it. This is also
/// what macOS gets: it is where presets have always been written there, and
/// moving them would lose everyone's.
fn xdg_preset_dir(config_home: Option<OsString>, home: Option<OsString>) -> Option<PathBuf> {
    let base = config_home
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(|| home.map(|home| PathBuf::from(home).join(".config")))?;
    Some(base.join("pulteqfx").join("presets"))
}

/// Every preset, built-in first and then the saved ones in name order.
///
/// A saved preset that shares a name with a factory one does not hide it. The
/// factory preset is compiled in and cannot be edited, so dropping it from the
/// list would put it permanently out of reach; the two sit side by side
/// instead, told apart by the factory tag and by the fact that only yours can
/// be deleted.
pub fn load_all(params: &impl Params) -> Vec<Preset> {
    let mut presets = built_in(params);
    presets.extend(preset_dir().map(|dir| load_dir(&dir)).unwrap_or_default());
    presets
}

fn built_in(params: &impl Params) -> Vec<Preset> {
    // Plain values have to be converted against the real parameters, so build
    // a lookup of id to pointer first.
    let pointers: BTreeMap<String, ParamPtr> = params
        .param_map()
        .into_iter()
        .map(|(id, ptr, _)| (id, ptr))
        .collect();

    BUILT_IN
        .iter()
        .map(|(name, dials)| Preset {
            name: (*name).to_string(),
            values: dials
                .iter()
                .filter_map(|(id, plain)| {
                    let ptr = pointers.get(*id)?;
                    // SAFETY: the pointers come from the params we were handed,
                    // which outlive this function.
                    Some((id.to_string(), unsafe { ptr.preview_normalized(*plain) }))
                })
                .collect(),
            built_in: true,
            path: None,
        })
        .collect()
}

/// The saved presets in a directory, in name order.
fn load_dir(dir: &Path) -> Vec<Preset> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };

    let mut presets: Vec<Preset> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
        .filter_map(|path| read(&path))
        .collect();
    // The path breaks ties, so two presets of one name always list in the
    // same order rather than in whatever order the directory gives.
    presets.sort_by(|a, b| (a.name.to_lowercase(), &a.path).cmp(&(b.name.to_lowercase(), &b.path)));
    presets
}

/// One saved preset, or nothing if the file does not hold one.
fn read(path: &Path) -> Option<Preset> {
    let text = std::fs::read_to_string(path).ok()?;
    let mut preset: Preset = serde_json::from_str(&text).ok()?;
    preset.built_in = false;
    // A file with no name inside shows under its file name.
    if preset.name.trim().is_empty() {
        if let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) {
            preset.name = stem.to_string();
        }
    }
    preset.path = Some(path.to_path_buf());
    Some(preset)
}

/// Whether two names are the same preset's. Case is ignored, as the file
/// systems of macOS and Windows ignore it: there, two names differing only in
/// case could never be two files.
fn same_name(a: &str, b: &str) -> bool {
    a.trim().to_lowercase() == b.trim().to_lowercase()
}

/// Take the current panel settings as a preset.
pub fn capture(params: &impl Params, name: &str) -> Preset {
    let values = params
        .param_map()
        .into_iter()
        .filter(|(id, _, _)| !EXCLUDED.contains(&id.as_str()))
        .map(|(id, ptr, _)| {
            // SAFETY: as above, the pointers belong to the params we were given.
            let value = unsafe { ptr.unmodulated_normalized_value() };
            (id, value)
        })
        .collect();

    Preset {
        name: name.trim().to_string(),
        values,
        built_in: false,
        path: None,
    }
}

/// Write a preset out, and say which file it went into.
///
/// Saving under the name of one of your presets replaces that preset, in
/// whichever file it lives; `name_taken` is what asks about that first. Any
/// other name gets a file of its own. File names are derived from preset
/// names and two different names can derive the same one -- "A/B" and "A_B"
/// both become `A_B.json` -- so a file that already exists is never taken
/// over by a different preset. The new one is numbered instead.
pub fn save(preset: &Preset) -> io::Result<PathBuf> {
    let dir = preset_dir().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            "no config directory to save presets into",
        )
    })?;
    std::fs::create_dir_all(&dir)?;

    let path = destination(&dir, &preset.name);
    let json = serde_json::to_string_pretty(preset)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
    std::fs::write(&path, json)?;
    Ok(path)
}

/// The file a preset of this name is saved into.
fn destination(dir: &Path, name: &str) -> PathBuf {
    let saved = load_dir(dir);
    // An exact match first, in case two files already hold names that
    // differ only in case.
    let replacing = saved
        .iter()
        .find(|preset| preset.name.trim() == name.trim())
        .or_else(|| saved.iter().find(|preset| same_name(&preset.name, name)))
        .and_then(|preset| preset.path.clone());
    if let Some(path) = replacing {
        return path;
    }

    let stem = file_stem(name);
    let mut path = dir.join(format!("{stem}.json"));
    let mut number = 2;
    while path.exists() {
        path = dir.join(format!("{stem} {number}.json"));
        number += 1;
    }
    path
}

/// Remove a saved preset's file: the one it was read from, so deleting a row
/// removes exactly that row even where another shares its name.
pub fn delete(preset: &Preset) -> io::Result<()> {
    match &preset.path {
        Some(path) if !preset.built_in => std::fs::remove_file(path),
        _ => Err(io::Error::new(
            io::ErrorKind::NotFound,
            "this preset has no file to remove",
        )),
    }
}

/// Whether the live parameters still match a preset's values. Comparing the
/// values rather than tracking an edited flag means that turning a control
/// back to where it was counts as unmodified again.
pub fn matches(params: &impl Params, values: &BTreeMap<String, f32>) -> bool {
    if values.is_empty() {
        return true;
    }
    params.param_map().into_iter().all(|(id, ptr, _)| {
        let Some(&saved) = values.get(&id) else {
            return true;
        };
        // SAFETY: the pointer comes from the params we were handed.
        let current = unsafe { ptr.unmodulated_normalized_value() };
        (current - saved).abs() <= 1e-5
    })
}

/// Whether saving under this name would replace a file of yours.
///
/// Factory presets are deliberately not counted: saving under one of their
/// names writes a new file beside it and replaces nothing, so warning about
/// it would be describing something that does not happen.
pub fn name_taken(name: &str, presets: &[Preset]) -> bool {
    presets
        .iter()
        .filter(|preset| !preset.built_in)
        .any(|preset| same_name(&preset.name, name))
}

/// Turns a preset name into something safe to use as a file name.
fn file_stem(name: &str) -> String {
    let stem: String = name
        .trim()
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || matches!(c, ' ' | '-' | '_') {
                c
            } else {
                '_'
            }
        })
        .collect();
    if stem.is_empty() {
        "preset".to_string()
    } else {
        stem
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Where a Windows host actually looks.
    ///
    /// Checked with a path that is absolute on whatever platform is running
    /// the test, because `Path::is_absolute` answers by the host's rules and a
    /// `C:\` path is not absolute to a Linux build. What is under test here is
    /// the shape of the result, not that check.
    #[test]
    fn the_windows_preset_directory_is_under_appdata() {
        let appdata = std::env::temp_dir().join("Roaming");
        let dir = windows_preset_dir(Some(appdata.clone().into_os_string()), None)
            .expect("APPDATA is enough on its own");
        assert_eq!(dir, appdata.join("PultEQFx").join("Presets"));
    }

    /// And the fallback, for the rare host that clears `APPDATA`.
    #[test]
    fn a_cleared_appdata_falls_back_to_the_user_profile() {
        let profile = std::env::temp_dir().join("user");
        let from_profile =
            windows_preset_dir(None, Some(profile.clone().into_os_string())).expect("a fallback");
        let from_appdata = windows_preset_dir(
            Some(profile.join("AppData").join("Roaming").into_os_string()),
            Some(profile.into_os_string()),
        )
        .expect("the direct route");
        assert_eq!(
            from_profile, from_appdata,
            "deriving the roaming directory from USERPROFILE has to land in the \
             same place APPDATA points at"
        );
        assert_eq!(
            windows_preset_dir(None, None),
            None,
            "with neither variable set there is nowhere to save, and saying so \
             beats writing to the current directory"
        );
    }

    /// A relative value is a host bug, and following it would scatter presets
    /// through whatever directory the DAW happened to start in.
    #[test]
    fn a_relative_setting_is_refused_rather_than_followed() {
        assert_eq!(
            windows_preset_dir(Some(OsString::from("AppData")), None),
            None
        );
        assert_eq!(xdg_preset_dir(Some(OsString::from(".config")), None), None);
    }

    /// Linux and macOS keep the rule they have always had, because moving it
    /// would lose everyone's saved presets.
    #[test]
    fn the_unix_preset_directory_is_unchanged() {
        let home = std::env::temp_dir().join("home");
        let expected = Some(home.join(".config").join("pulteqfx").join("presets"));
        assert_eq!(
            xdg_preset_dir(Some(home.join(".config").into_os_string()), None),
            expected
        );
        assert_eq!(xdg_preset_dir(None, Some(home.into_os_string())), expected);
    }
}
