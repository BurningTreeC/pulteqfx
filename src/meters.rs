//! Levels, written by the audio thread and read by the editor.
//!
//! Two meters, one on what arrives and one on what leaves, so that both the
//! level going into the unit and the level coming out of it can be set by eye.
//! The output is metered after the output trim, and with the power off it is
//! the dry signal, which is what the host is actually given either way.
//!
//! Each meter keeps three figures per channel, all as linear magnitudes in
//! atomics, so the audio thread never waits on the editor:
//!
//! * the highest sample since the editor last looked, which it takes, so no
//!   peak between two frames can be missed however the buffers and the frames
//!   happen to line up;
//! * the highest sample since the readout was last cleared, which is the exact
//!   figure a gain stage is set against;
//! * the mean square over the last 300 ms, for the average level: once as it
//!   stands, for the bar, and once as it stood when last sampled, which
//!   happens every 200 ms of audio, for the figure -- so the figure can be
//!   read rather than watched flicker.
//!
//! These are sample peaks. A peak between samples, which a converter can
//! still reconstruct above full scale, is not measured.

use std::sync::atomic::{AtomicU32, Ordering};

/// The most channels the plugin is ever given.
pub const CHANNELS: usize = 2;

/// How long the RMS figure averages over, in seconds: the 300 ms a VU meter
/// integrates over.
const RMS_TIME: f64 = 0.3;

/// How often the RMS figure under a meter moves, in seconds of audio.
const FIGURE_TIME: f64 = 0.2;

/// Anything quieter than this, in dBFS, is shown as silence. It is the floor
/// of a 24 bit signal; a figure below it would only be measuring rounding.
pub const SILENCE_DB: f32 = -144.0;

/// A non-negative `f32` in an atomic. For non-negative floats the bit
/// patterns sort the same way as the values, so `fetch_max` on the bits raises
/// the value, and the audio thread can raise it without ever reading it first.
#[derive(Default)]
struct Level(AtomicU32);

impl Level {
    fn raise(&self, value: f32) {
        self.0
            .fetch_max(magnitude(value).to_bits(), Ordering::Relaxed);
    }

    fn store(&self, value: f32) {
        self.0.store(magnitude(value).to_bits(), Ordering::Relaxed);
    }

    fn load(&self) -> f32 {
        f32::from_bits(self.0.load(Ordering::Relaxed))
    }

    fn take(&self) -> f32 {
        f32::from_bits(self.0.swap(0, Ordering::Relaxed))
    }
}

/// What a level is allowed to be. A NaN is not a level and counts as nothing,
/// but an infinite sample is a signal that has blown up, and has to show as
/// far over rather than disappear.
fn magnitude(value: f32) -> f32 {
    if value.is_nan() || value <= 0.0 {
        0.0
    } else {
        value.min(f32::MAX)
    }
}

/// One meter: what arrives at the unit, or what leaves it.
#[derive(Default)]
pub struct Meter {
    peak: [Level; CHANNELS],
    held: [Level; CHANNELS],
    mean_square: [Level; CHANNELS],
    figure: [Level; CHANNELS],
}

impl Meter {
    /// Called by the audio thread with what it measured since the last call.
    pub fn publish(&self, channel: usize, peak: f32, mean_square: f32) {
        let Some(slot) = self.peak.get(channel) else {
            return;
        };
        slot.raise(peak);
        self.held[channel].raise(peak);
        self.mean_square[channel].store(mean_square);
    }

    /// The highest sample on `channel` since this was last asked.
    pub fn take_peak(&self, channel: usize) -> f32 {
        self.peak.get(channel).map_or(0.0, Level::take)
    }

    /// The highest sample on `channel` since the readout was cleared.
    pub fn held(&self, channel: usize) -> f32 {
        self.held.get(channel).map_or(0.0, Level::load)
    }

    /// The mean square on `channel` over the last 300 ms.
    pub fn mean_square(&self, channel: usize) -> f32 {
        self.mean_square.get(channel).map_or(0.0, Level::load)
    }

    /// Called by the audio thread every `FIGURE_TIME`: the mean square the
    /// RMS figure shows until the next time.
    pub fn publish_figure(&self, channel: usize, mean_square: f32) {
        if let Some(slot) = self.figure.get(channel) {
            slot.store(mean_square);
        }
    }

    /// The mean square the RMS figure on `channel` shows.
    pub fn figure(&self, channel: usize) -> f32 {
        self.figure.get(channel).map_or(0.0, Level::load)
    }

    /// Start the held figure again, as clicking the readout does.
    pub fn clear_held(&self) {
        self.held.iter().for_each(|level| level.store(0.0));
    }

    fn clear(&self) {
        for levels in [&self.peak, &self.held, &self.mean_square, &self.figure] {
            levels.iter().for_each(|level| level.store(0.0));
        }
    }
}

/// Both meters, shared between the audio thread and the editor.
pub struct Meters {
    pub input: Meter,
    pub output: Meter,
    /// How many channels the host gave the plugin: one or two.
    channels: AtomicU32,
}

impl Default for Meters {
    fn default() -> Self {
        Self {
            input: Meter::default(),
            output: Meter::default(),
            channels: AtomicU32::new(CHANNELS as u32),
        }
    }
}

impl Meters {
    pub fn channels(&self) -> usize {
        (self.channels.load(Ordering::Relaxed) as usize).clamp(1, CHANNELS)
    }

    pub fn set_channels(&self, channels: usize) {
        self.channels
            .store(channels.clamp(1, CHANNELS) as u32, Ordering::Relaxed);
    }

    /// Forget everything, as when the plugin is set up afresh.
    pub fn clear(&self) {
        self.input.clear();
        self.output.clear();
    }
}

/// The audio thread's half of one meter on one channel: what it has measured
/// since it last published.
#[derive(Clone, Copy, Default)]
pub struct Tap {
    peak: f32,
    mean_square: f64,
    /// How much of each new sample's square the average takes in.
    coeff: f64,
    /// Samples since the figure was last published, and how many apart the
    /// publications are.
    since_figure: usize,
    figure_every: usize,
}

impl Tap {
    pub fn new(sample_rate: f32) -> Self {
        let figure_every = (FIGURE_TIME * sample_rate.max(1.0) as f64).round() as usize;
        Self {
            coeff: 1.0 - (-1.0 / (RMS_TIME * sample_rate.max(1.0) as f64)).exp(),
            // Due at once, so a figure appears with the first buffer.
            since_figure: figure_every,
            figure_every,
            ..Self::default()
        }
    }

    #[inline]
    pub fn add(&mut self, sample: f32) {
        let magnitude = sample.abs();
        // Written so a NaN, which compares false with everything, is skipped.
        if magnitude > self.peak {
            self.peak = magnitude;
        }
        // One non-finite sample would otherwise poison the average for good.
        if sample.is_finite() {
            let square = sample as f64 * sample as f64;
            self.mean_square += self.coeff * (square - self.mean_square);
        }
    }

    /// Hand what has been measured over the last `samples` to the editor and
    /// start the next stretch.
    pub fn publish(&mut self, meter: &Meter, channel: usize, samples: usize) {
        // Well below anything shown, and left alone the average would decay
        // into denormals through a long silence and slow the audio thread.
        if self.mean_square < 1e-20 {
            self.mean_square = 0.0;
        }
        meter.publish(channel, self.peak, self.mean_square as f32);
        self.peak = 0.0;

        self.since_figure += samples;
        if self.since_figure >= self.figure_every {
            self.since_figure = 0;
            meter.publish_figure(channel, self.mean_square as f32);
        }
    }

    pub fn reset(&mut self) {
        self.peak = 0.0;
        self.mean_square = 0.0;
        self.since_figure = self.figure_every;
    }
}

/// The audio thread's taps on both meters, one per channel on each.
pub struct Taps {
    input: [Tap; CHANNELS],
    output: [Tap; CHANNELS],
}

impl Taps {
    pub fn new(sample_rate: f32) -> Self {
        let tap = Tap::new(sample_rate);
        Self {
            input: [tap; CHANNELS],
            output: [tap; CHANNELS],
        }
    }

    /// The input and output taps of one channel.
    pub fn channel(&mut self, channel: usize) -> Option<(&mut Tap, &mut Tap)> {
        Some((self.input.get_mut(channel)?, self.output.get_mut(channel)?))
    }

    /// Publish what the last `samples` of every channel in use measured.
    pub fn publish(&mut self, meters: &Meters, channels: usize, samples: usize) {
        for channel in 0..channels.min(CHANNELS) {
            self.input[channel].publish(&meters.input, channel, samples);
            self.output[channel].publish(&meters.output, channel, samples);
        }
    }

    pub fn reset(&mut self) {
        self.input.iter_mut().for_each(Tap::reset);
        self.output.iter_mut().for_each(Tap::reset);
    }
}

/// A linear magnitude in dBFS.
pub fn db(magnitude: f32) -> f32 {
    if magnitude > 0.0 {
        20.0 * magnitude.log10()
    } else {
        f32::NEG_INFINITY
    }
}

/// A mean square in dBFS, as an RMS level.
pub fn db_power(mean_square: f32) -> f32 {
    if mean_square > 0.0 {
        10.0 * mean_square.log10()
    } else {
        f32::NEG_INFINITY
    }
}

/// A level as the readouts show it: to a tenth of a decibel, with a sign on
/// anything over full scale, and silence as `-inf`.
pub fn readout(db: f32) -> String {
    if db.is_nan() || db <= SILENCE_DB {
        return String::from("-inf");
    }
    let tenths = (db * 10.0).round() / 10.0;
    if tenths > 0.0 {
        format!("+{tenths:.1}")
    } else if tenths == 0.0 {
        // Otherwise a hair under full scale reads as "-0.0".
        String::from("0.0")
    } else {
        format!("{tenths:.1}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine(amplitude: f32, freq: f32, sample_rate: f32, n: usize) -> f32 {
        amplitude * (std::f32::consts::TAU * freq * n as f32 / sample_rate).sin()
    }

    /// The two figures a full scale sine is known to give: a peak of exactly
    /// 0 dBFS, and an RMS level 3 dB under it.
    #[test]
    fn a_full_scale_sine_reads_zero_peak_and_three_under_rms() {
        let sample_rate = 48_000.0;
        let meters = Meters::default();
        let mut tap = Tap::new(sample_rate);
        // A whole number of cycles per buffer, so every buffer holds the crest.
        for block in 0..200 {
            for n in 0..480 {
                tap.add(sine(1.0, 1_000.0, sample_rate, block * 480 + n + 12));
            }
            tap.publish(&meters.input, 0, 480);
        }
        assert_eq!(readout(db(meters.input.held(0))), "0.0");
        let rms = db_power(meters.input.mean_square(0));
        assert!((rms + 3.01).abs() < 0.05, "RMS read {rms:.3} dB");
        assert_eq!(readout(rms), "-3.0");
    }

    /// The bar's figure is taken, the readout's figure stays until cleared,
    /// and a quieter buffer never lowers either.
    #[test]
    fn peaks_are_taken_and_held_ones_kept_until_cleared() {
        let meters = Meters::default();
        meters.output.publish(1, 0.5, 0.0);
        meters.output.publish(1, 0.25, 0.0);
        assert_eq!(meters.output.take_peak(1), 0.5);
        assert_eq!(meters.output.take_peak(1), 0.0, "taking it starts it again");
        assert_eq!(meters.output.held(1), 0.5);
        meters.output.publish(1, 0.1, 0.0);
        assert_eq!(meters.output.held(1), 0.5, "held until cleared");
        meters.output.clear_held();
        assert_eq!(meters.output.held(1), 0.0);
        assert_eq!(meters.input.held(1), 0.0, "the two meters are apart");
    }

    /// A NaN is not a level, but an infinity is a signal that has blown up
    /// and must read as over rather than vanish.
    #[test]
    fn broken_samples_neither_vanish_nor_poison_the_meter() {
        let meters = Meters::default();
        let mut tap = Tap::new(48_000.0);
        tap.add(f32::NAN);
        tap.add(0.5);
        tap.publish(&meters.input, 0, 2);
        assert_eq!(meters.input.held(0), 0.5);
        assert!(meters.input.mean_square(0).is_finite());

        tap.add(f32::INFINITY);
        tap.publish(&meters.input, 0, 1);
        assert!(
            meters.input.held(0) >= 1.0,
            "an infinity is over full scale"
        );
        assert!(meters.input.mean_square(0).is_finite());
    }

    /// The figure is sampled every 200 ms of audio, and the first buffer
    /// gives one at once rather than leaving the readout empty.
    #[test]
    fn the_rms_figure_moves_every_fifth_of_a_second() {
        let meters = Meters::default();
        let mut tap = Tap::new(48_000.0);
        let mut moves = 0;
        let mut last = meters.input.figure(0);
        // A rising level, so every sample of it is a new figure.
        for block in 0..100 {
            for n in 0..480 {
                tap.add(sine(0.01 * (block + 1) as f32, 1_000.0, 48_000.0, n));
            }
            tap.publish(&meters.input, 0, 480);
            let now = meters.input.figure(0);
            if now != last {
                moves += 1;
                last = now;
                assert_eq!(block % 20, 0, "the figure moved after buffer {block}");
            }
        }
        // One second of audio: at the first buffer, then every fifth of a
        // second after it.
        assert_eq!(moves, 5);
        assert!(meters.input.mean_square(0) > meters.input.figure(0));
    }

    #[test]
    fn readouts_are_to_a_tenth_with_the_overs_signed() {
        assert_eq!(readout(f32::NEG_INFINITY), "-inf");
        assert_eq!(readout(-150.0), "-inf");
        assert_eq!(readout(-3.24), "-3.2");
        assert_eq!(readout(-0.04), "0.0");
        assert_eq!(readout(1.36), "+1.4");
        assert_eq!(readout(-18.0), "-18.0");
    }

    #[test]
    fn channel_counts_are_one_or_two() {
        let meters = Meters::default();
        assert_eq!(meters.channels(), 2);
        meters.set_channels(1);
        assert_eq!(meters.channels(), 1);
        meters.set_channels(0);
        assert_eq!(meters.channels(), 1);
        meters.set_channels(8);
        assert_eq!(meters.channels(), 2);
    }
}
