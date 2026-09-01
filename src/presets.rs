//! Preset storage.
//!
//! A preset is the panel's parameter values keyed by parameter id. Built-in
//! presets are compiled in and cannot be overwritten; the ones you save go
//! into the user's config directory as one small JSON file each, so they can
//! be copied around and edited by hand.

use nih_plug::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

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
        ("power", 1.0),   // ON
        ("eqin", 1.0),    // EQ IN
        ("lofreq", 3.0),  // 100 cps
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
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Preset {
    pub name: String,
    pub values: BTreeMap<String, f32>,
    /// Compiled in rather than loaded from disk, so it cannot be overwritten.
    #[serde(default, skip)]
    pub built_in: bool,
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
pub fn preset_dir() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))?;
    Some(base.join("pulteqfx").join("presets"))
}

/// Every preset, built-in first and then the saved ones in name order. A saved
/// preset that shares a name with a built-in one replaces it, so the list
/// never shows the same name twice and saving over a built-in name does what
/// it looks like it does.
pub fn load_all(params: &impl Params) -> Vec<Preset> {
    let user = load_user();
    let mut presets: Vec<Preset> = built_in(params)
        .into_iter()
        .filter(|preset| !name_taken(&preset.name, &user))
        .collect();
    presets.extend(user);
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
        })
        .collect()
}

fn load_user() -> Vec<Preset> {
    let Some(dir) = preset_dir() else {
        return Vec::new();
    };
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };

    let mut presets: Vec<Preset> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
        .filter_map(|path| {
            let text = std::fs::read_to_string(&path).ok()?;
            let mut preset: Preset = serde_json::from_str(&text).ok()?;
            preset.built_in = false;
            // A preset whose file was renamed should follow the file.
            if let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) {
                if preset.name.trim().is_empty() {
                    preset.name = stem.to_string();
                }
            }
            Some(preset)
        })
        .collect();
    presets.sort_by_key(|preset| preset.name.to_lowercase());
    presets
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
    }
}

/// Write a preset out, replacing any file with the same name.
pub fn save(preset: &Preset) -> std::io::Result<PathBuf> {
    let dir = preset_dir().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "no config directory to save presets into",
        )
    })?;
    std::fs::create_dir_all(&dir)?;

    let path = dir.join(format!("{}.json", file_stem(&preset.name)));
    let json = serde_json::to_string_pretty(preset)
        .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, err))?;
    std::fs::write(&path, json)?;
    Ok(path)
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

/// Whether saving under this name would replace something.
pub fn name_taken(name: &str, presets: &[Preset]) -> bool {
    let name = name.trim();
    presets
        .iter()
        .any(|preset| preset.name.trim().eq_ignore_ascii_case(name))
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
