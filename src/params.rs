//! Front panel controls.
//!
//! The knobs are calibrated the way the hardware is: 0 to 10, with the
//! non-linear relationship between knob position and decibels falling out of
//! the circuit rather than being dialled in by hand.

use nih_plug::prelude::*;
use nih_plug_vizia::ViziaState;
use std::sync::{Arc, Mutex};

use crate::dsp::Controls;
use crate::editor;

#[derive(Enum, Debug, PartialEq, Eq, Clone, Copy)]
pub enum LowFreq {
    #[id = "20"]
    #[name = "20 Hz"]
    Hz20,
    #[id = "30"]
    #[name = "30 Hz"]
    Hz30,
    #[id = "60"]
    #[name = "60 Hz"]
    Hz60,
    #[id = "100"]
    #[name = "100 Hz"]
    Hz100,
}

impl LowFreq {
    pub fn hz(self) -> f64 {
        match self {
            LowFreq::Hz20 => 20.0,
            LowFreq::Hz30 => 30.0,
            LowFreq::Hz60 => 60.0,
            LowFreq::Hz100 => 100.0,
        }
    }

    /// Short label for the front panel, which is engraved in cycles per second.
    pub const LABELS: [&'static str; 4] = ["20", "30", "60", "100"];
}

#[derive(Enum, Debug, PartialEq, Eq, Clone, Copy)]
pub enum HighBoostFreq {
    #[id = "3k"]
    #[name = "3 kHz"]
    KHz3,
    #[id = "4k"]
    #[name = "4 kHz"]
    KHz4,
    #[id = "5k"]
    #[name = "5 kHz"]
    KHz5,
    #[id = "8k"]
    #[name = "8 kHz"]
    KHz8,
    #[id = "10k"]
    #[name = "10 kHz"]
    KHz10,
    #[id = "12k"]
    #[name = "12 kHz"]
    KHz12,
    #[id = "16k"]
    #[name = "16 kHz"]
    KHz16,
}

impl HighBoostFreq {
    pub fn hz(self) -> f64 {
        match self {
            HighBoostFreq::KHz3 => 3e3,
            HighBoostFreq::KHz4 => 4e3,
            HighBoostFreq::KHz5 => 5e3,
            HighBoostFreq::KHz8 => 8e3,
            HighBoostFreq::KHz10 => 10e3,
            HighBoostFreq::KHz12 => 12e3,
            HighBoostFreq::KHz16 => 16e3,
        }
    }

    pub const LABELS: [&'static str; 7] = ["3", "4", "5", "8", "10", "12", "16"];
}

#[derive(Enum, Debug, PartialEq, Eq, Clone, Copy)]
pub enum HighAttenFreq {
    #[id = "5k"]
    #[name = "5 kHz"]
    KHz5,
    #[id = "10k"]
    #[name = "10 kHz"]
    KHz10,
    #[id = "20k"]
    #[name = "20 kHz"]
    KHz20,
}

impl HighAttenFreq {
    pub fn hz(self) -> f64 {
        match self {
            HighAttenFreq::KHz5 => 5e3,
            HighAttenFreq::KHz10 => 10e3,
            HighAttenFreq::KHz20 => 20e3,
        }
    }

    pub const LABELS: [&'static str; 3] = ["5", "10", "20"];
}

#[derive(Enum, Debug, PartialEq, Eq, Clone, Copy)]
pub enum Oversampling {
    #[id = "1x"]
    #[name = "Off"]
    Off,
    #[id = "2x"]
    #[name = "2x"]
    X2,
    #[id = "4x"]
    #[name = "4x"]
    X4,
    #[id = "8x"]
    #[name = "8x"]
    X8,
}

impl Oversampling {
    pub fn factor(self) -> usize {
        match self {
            Oversampling::Off => 1,
            Oversampling::X2 => 2,
            Oversampling::X4 => 4,
            Oversampling::X8 => 8,
        }
    }
}

#[derive(Params)]
pub struct PultEqFxParams {
    #[persist = "editor-state"]
    pub editor_state: Arc<ViziaState>,

    /// Name of the preset the panel was last set from, so a reopened session
    /// still says which one it is. Empty when nothing has been loaded.
    #[persist = "preset"]
    pub preset_name: Arc<Mutex<String>>,

    #[id = "lofreq"]
    pub low_freq: EnumParam<LowFreq>,
    #[id = "loboost"]
    pub low_boost: FloatParam,
    #[id = "loatten"]
    pub low_atten: FloatParam,

    #[id = "bandw"]
    pub bandwidth: FloatParam,
    #[id = "hifreq"]
    pub high_boost_freq: EnumParam<HighBoostFreq>,
    #[id = "hiboost"]
    pub high_boost: FloatParam,

    #[id = "hiafreq"]
    pub high_atten_freq: EnumParam<HighAttenFreq>,
    #[id = "hiatten"]
    pub high_atten: FloatParam,

    #[id = "power"]
    pub power: BoolParam,
    #[id = "eqin"]
    pub eq_in: BoolParam,
    #[id = "drive"]
    pub drive: FloatParam,
    #[id = "output"]
    pub output: FloatParam,
    #[id = "os"]
    pub oversampling: EnumParam<Oversampling>,
}

/// Reads back the panel's switch positions, which are lettered rather than
/// written out as true and false.
fn switch_on(text: &str) -> bool {
    !matches!(
        text.trim().to_ascii_lowercase().as_str(),
        "off" | "out" | "false" | "no" | "0"
    )
}

/// The hardware knobs are dialled 0 to 10.
fn knob(name: &'static str) -> FloatParam {
    FloatParam::new(name, 0.0, FloatRange::Linear { min: 0.0, max: 10.0 })
        .with_smoother(SmoothingStyle::Linear(30.0))
        .with_value_to_string(Arc::new(|v| format!("{v:.1}")))
        .with_string_to_value(Arc::new(|s| s.trim().parse().ok()))
}

impl Default for PultEqFxParams {
    fn default() -> Self {
        Self {
            editor_state: editor::default_state(),
            preset_name: Arc::new(Mutex::new(String::new())),

            low_freq: EnumParam::new("Low Frequency", LowFreq::Hz100),
            low_boost: knob("Low Boost"),
            low_atten: knob("Low Atten"),

            bandwidth: FloatParam::new(
                "Bandwidth",
                5.0,
                FloatRange::Linear { min: 0.0, max: 10.0 },
            )
            .with_smoother(SmoothingStyle::Linear(30.0))
            .with_value_to_string(Arc::new(|v| {
                // Sharp on the left, broad on the right, like the engraving.
                format!("{v:.1}")
            }))
            .with_string_to_value(Arc::new(|s| s.trim().parse().ok())),
            high_boost_freq: EnumParam::new("High Boost Frequency", HighBoostFreq::KHz10),
            high_boost: knob("High Boost"),

            high_atten_freq: EnumParam::new("High Atten Frequency", HighAttenFreq::KHz10),
            high_atten: knob("High Atten"),

            // The panel's OFF/ON control, which takes the whole unit out of
            // circuit rather than just the equaliser network.
            power: BoolParam::new("Power", true)
                .with_value_to_string(Arc::new(|v| if v { "ON" } else { "OFF" }.to_string()))
                .with_string_to_value(Arc::new(|s| Some(switch_on(s)))),
            eq_in: BoolParam::new("EQ In", true)
                .with_value_to_string(Arc::new(|v| if v { "IN" } else { "OUT" }.to_string()))
                .with_string_to_value(Arc::new(|s| Some(switch_on(s)))),
            drive: FloatParam::new(
                "Drive",
                15.0,
                FloatRange::Linear {
                    min: 0.0,
                    max: 100.0,
                },
            )
            .with_unit(" %")
            .with_smoother(SmoothingStyle::Linear(30.0))
            .with_value_to_string(formatters::v2s_f32_rounded(0)),
            output: FloatParam::new(
                "Output",
                0.0,
                FloatRange::Linear {
                    min: -24.0,
                    max: 24.0,
                },
            )
            .with_unit(" dB")
            .with_smoother(SmoothingStyle::Linear(20.0))
            .with_value_to_string(formatters::v2s_f32_rounded(1)),
            oversampling: EnumParam::new("Oversampling", Oversampling::X4),
        }
    }
}

impl PultEqFxParams {
    /// The preset the panel was last set from, if any.
    pub fn preset_name(&self) -> String {
        self.preset_name
            .lock()
            .map(|name| name.clone())
            .unwrap_or_default()
    }

    pub fn set_preset_name(&self, name: &str) {
        if let Ok(mut current) = self.preset_name.lock() {
            name.clone_into(&mut current);
        }
    }

    /// Snapshot of the panel as the circuit wants it: pot fractions and
    /// frequencies in Hz.
    pub fn controls(&self, low_boost: f32, low_atten: f32, high_boost: f32, high_atten: f32, bandwidth: f32) -> Controls {
        Controls {
            low_boost: (low_boost / 10.0) as f64,
            low_atten: (low_atten / 10.0) as f64,
            high_boost: (high_boost / 10.0) as f64,
            high_atten: (high_atten / 10.0) as f64,
            bandwidth: (bandwidth / 10.0) as f64,
            low_freq: self.low_freq.value().hz(),
            high_boost_freq: self.high_boost_freq.value().hz(),
            high_atten_freq: self.high_atten_freq.value().hz(),
        }
    }
}
