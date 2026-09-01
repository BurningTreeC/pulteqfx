//! PultEQFx, a circuit modelled passive program equaliser.
//!
//! The circuit modelled is that of the classic 1950s tube program equaliser,
//! the Pultec EQP-1A. This plugin is not affiliated with or endorsed by Pulse
//! Techniques, LLC.
//!
//! Copyright (C) 2026 Simon Huber
//!
//! This program is free software: you can redistribute it and/or modify it
//! under the terms of the GNU General Public License as published by the Free
//! Software Foundation, either version 3 of the License, or (at your option)
//! any later version. It is distributed in the hope that it will be useful,
//! but WITHOUT ANY WARRANTY; without even the implied warranty of
//! MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General
//! Public License in `LICENSE` for more details.
//!
//! The GPL applies because `nih_export_vst3!()` links the VST3 bindings from
//! the `vst3-sys` crate, which are themselves GPLv3. Everything else the
//! plugin uses is permissively licensed; see `THIRD-PARTY-NOTICES.md`.
//!
//! The passive equaliser network is simulated as a circuit (see
//! [`dsp::eqp1a`]) rather than approximated with a bank of textbook shelves,
//! so the interactions the unit is famous for - the low end boost/attenuate
//! trick above all - are a consequence of the topology instead of a special
//! case bolted on afterwards.

use nih_plug::prelude::*;
use std::sync::Arc;

pub mod dsp;
pub mod editor;
pub mod params;
pub mod presets;

use dsp::Channel;
use params::{Oversampling, PultEqFxParams};

/// Controls are refreshed at this granularity. Recomputing the port
/// impedances of the whole network per sample would be wasteful, and a third
/// of a millisecond is far quicker than any knob can be turned.
const CONTROL_BLOCK: usize = 32;

pub struct PultEqFx {
    params: Arc<PultEqFxParams>,
    channels: Vec<Channel>,
    sample_rate: f32,
    oversampling: Oversampling,
}

impl Default for PultEqFx {
    fn default() -> Self {
        Self {
            params: Arc::new(PultEqFxParams::default()),
            channels: Vec::new(),
            sample_rate: 44100.0,
            oversampling: Oversampling::X4,
        }
    }
}

impl Plugin for PultEqFx {
    const NAME: &'static str = "PultEQFx";
    const VENDOR: &'static str = "BurningTreeC";
    const URL: &'static str = "https://github.com/";
    const EMAIL: &'static str = "huber.simon@protonmail.com";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");

    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[
        AudioIOLayout {
            main_input_channels: NonZeroU32::new(2),
            main_output_channels: NonZeroU32::new(2),
            ..AudioIOLayout::const_default()
        },
        AudioIOLayout {
            main_input_channels: NonZeroU32::new(1),
            main_output_channels: NonZeroU32::new(1),
            ..AudioIOLayout::const_default()
        },
    ];

    const SAMPLE_ACCURATE_AUTOMATION: bool = false;

    type SysExMessage = ();
    type BackgroundTask = ();

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    fn editor(&mut self, _async_executor: AsyncExecutor<Self>) -> Option<Box<dyn Editor>> {
        editor::create(self.params.clone(), self.params.editor_state.clone())
    }

    fn initialize(
        &mut self,
        audio_io_layout: &AudioIOLayout,
        buffer_config: &BufferConfig,
        context: &mut impl InitContext<Self>,
    ) -> bool {
        self.sample_rate = buffer_config.sample_rate;
        self.oversampling = self.params.oversampling.value();

        let num_channels = audio_io_layout
            .main_output_channels
            .map(NonZeroU32::get)
            .unwrap_or(2) as usize;

        self.channels.clear();
        self.channels.reserve(num_channels);
        for _ in 0..num_channels {
            self.channels.push(Channel::new(
                self.sample_rate as f64,
                self.oversampling.factor(),
            ));
        }

        context.set_latency_samples(dsp::LATENCY);
        true
    }

    fn reset(&mut self) {
        self.channels.iter_mut().for_each(Channel::reset);
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        _context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        // The reported latency does not depend on the oversampling factor, so
        // this can change freely while the plugin is running without having to
        // tell the host anything.
        let oversampling = self.params.oversampling.value();
        if oversampling != self.oversampling {
            self.oversampling = oversampling;
            for channel in self.channels.iter_mut() {
                channel.set_oversampling(oversampling.factor());
            }
        }

        let powered = self.params.power.value();
        let eq_in = self.params.eq_in.value();

        // With the power off the unit is out of circuit entirely, amplifier
        // included, so the dry signal passes straight through.
        if !powered {
            for channel in self.channels.iter_mut() {
                channel.reset();
            }
            return ProcessStatus::Normal;
        }


        for (_, mut block) in buffer.iter_blocks(CONTROL_BLOCK) {
            let steps = block.samples() as u32;

            let controls = self.params.controls(
                self.params.low_boost.smoothed.next_step(steps),
                self.params.low_atten.smoothed.next_step(steps),
                self.params.high_boost.smoothed.next_step(steps),
                self.params.high_atten.smoothed.next_step(steps),
                self.params.bandwidth.smoothed.next_step(steps),
            );
            let drive = (self.params.drive.smoothed.next_step(steps) / 100.0) as f64;
            let output = util::db_to_gain(self.params.output.smoothed.next_step(steps));

            for channel in self.channels.iter_mut() {
                channel.eq.set_controls(controls);
                channel.tube.set_drive(drive);
            }

            for (channel_idx, samples) in block.iter_mut().enumerate() {
                let Some(channel) = self.channels.get_mut(channel_idx) else {
                    continue;
                };
                for sample in samples.iter_mut() {
                    *sample = channel.process(*sample, eq_in) * output;
                }
            }
        }

        ProcessStatus::Normal
    }
}

impl ClapPlugin for PultEqFx {
    const CLAP_ID: &'static str = "com.burningtreec.pulteqfx";
    const CLAP_DESCRIPTION: Option<&'static str> =
        Some("Circuit modelled passive program equaliser");
    const CLAP_MANUAL_URL: Option<&'static str> = Some(Self::URL);
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] = &[
        ClapFeature::AudioEffect,
        ClapFeature::Stereo,
        ClapFeature::Mono,
        ClapFeature::Equalizer,
        ClapFeature::Mastering,
    ];
}

impl Vst3Plugin for PultEqFx {
    const VST3_CLASS_ID: [u8; 16] = *b"PultEQFx-BTC-001";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] =
        &[Vst3SubCategory::Fx, Vst3SubCategory::Eq];
}

nih_export_clap!(PultEqFx);
nih_export_vst3!(PultEqFx);
