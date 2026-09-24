//! The input and output level meters either side of the controls.
//!
//! Not on the hardware. They are there for gain staging, so they read the way
//! a DAW's meters do: in dBFS, on the scale peak meters use, with a bar per
//! channel that rises at once and falls back at 20 dB in 1.7 s, a peak hold
//! line, a clip lamp that stays lit, and beneath them the two figures a level
//! is set by -- the highest peak since the readout was cleared, and the RMS
//! level over the last 300 ms.
//!
//! The bar shows both levels at once, as a DAW's peak and RMS meters do: solid
//! up to the RMS level, and fainter from there up to the peak.

use nih_plug_vizia::assets;
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::vizia::vg;
use std::cell::Cell;
use std::sync::Arc;
use std::time::Instant;

use super::style::*;
use crate::meters::{self, Meter, Meters, CHANNELS};

/// The meter window, in logical pixels.
pub const WIDTH: f32 = 26.0;
pub const HEIGHT: f32 = 206.0;
/// Inside the window: the margin round the bars, and the clip lamps along the
/// top above the scale.
const INSET: f32 = 3.0;
const CLIP_H: f32 = 5.0;
const CLIP_GAP: f32 = 2.0;
/// Where the scale starts below the top of the window, and how tall it is.
pub const SCALE_TOP: f32 = INSET + CLIP_H + CLIP_GAP;
pub const SCALE_H: f32 = HEIGHT - SCALE_TOP - INSET;

/// The levels marked on the scale, in dBFS.
pub const TICKS: [f32; 12] = [
    6.0, 3.0, 0.0, -3.0, -6.0, -9.0, -12.0, -18.0, -24.0, -30.0, -40.0, -60.0,
];

/// How fast a bar falls once the signal drops: 20 dB in 1.7 s, the return
/// time of an IEC 60268-18 digital peak meter.
const FALL: f32 = 20.0 / 1.7;
/// How long the peak hold line stays before it falls with the bar.
const HOLD: f32 = 2.0;
/// Below the bottom of the scale.
const FLOOR: f32 = -70.0;

// The colour of the scale, fixed to its height as a real meter's is: green
// under the -18 dBFS a level is usually set around, through yellow to orange
// approaching full scale, red over it.
const GREEN: u32 = 0x39c95c;
const YELLOW: u32 = 0xe2cf3c;
const ORANGE: u32 = 0xf29a2e;
const RED: u32 = 0xef4136;

/// Which of the two meters.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Which {
    Input,
    Output,
}

impl Which {
    fn of(self, meters: &Meters) -> &Meter {
        match self {
            Which::Input => &meters.input,
            Which::Output => &meters.output,
        }
    }
}

/// How far up the scale a level sits, from 0 at the bottom to 1 at +6 dBFS.
///
/// The scale peak meters use, as Ardour draws it after IEC 60268-18: a
/// decibel is worth two and a half times as much height near the top as it is
/// near the bottom, so the range a level is actually set in is spread out and
/// the quiet end is squeezed.
pub fn deflection(db: f32) -> f32 {
    let percent = if db.is_nan() || db < -70.0 {
        0.0
    } else if db < -60.0 {
        (db + 70.0) * 0.25
    } else if db < -50.0 {
        (db + 60.0) * 0.5 + 2.5
    } else if db < -40.0 {
        (db + 50.0) * 0.75 + 7.5
    } else if db < -30.0 {
        (db + 40.0) * 1.5 + 15.0
    } else if db < -20.0 {
        (db + 30.0) * 2.0 + 30.0
    } else if db < 6.0 {
        (db + 20.0) * 2.5 + 50.0
    } else {
        115.0
    };
    percent / 115.0
}

/// Where a level sits below the top of the window, in logical pixels. The
/// engraved scale beside the window is placed with this.
pub fn scale_y(db: f32) -> f32 {
    SCALE_TOP + SCALE_H * (1.0 - deflection(db))
}

/// The ballistics of one bar: where it stands, and where its hold line is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Needle {
    pub level: f32,
    pub hold: f32,
    held_for: f32,
}

impl Default for Needle {
    fn default() -> Self {
        Self {
            level: FLOOR,
            hold: FLOOR,
            held_for: 0.0,
        }
    }
}

impl Needle {
    /// Move on by `dt` seconds, in which the highest sample was `peak` dBFS.
    /// The bar rises to a peak at once and falls back at `FALL`; the hold
    /// line waits `HOLD` seconds at the highest point before following it.
    pub fn step(&mut self, peak: f32, dt: f32) {
        let fallen = (self.level - FALL * dt).max(FLOOR);
        self.level = if peak > fallen { peak } else { fallen };
        if self.level >= self.hold {
            self.hold = self.level;
            self.held_for = 0.0;
        } else {
            self.held_for += dt;
            if self.held_for > HOLD {
                self.hold = (self.hold - FALL * dt).max(self.level);
            }
        }
    }
}

/// One meter's window: a bar per channel, with its clip lamps.
pub struct LevelMeter {
    meters: Arc<Meters>,
    which: Which,
    needles: Cell<[Needle; CHANNELS]>,
    last: Cell<Option<Instant>>,
}

impl LevelMeter {
    pub fn new(cx: &mut Context, meters: Arc<Meters>, which: Which) -> Handle<'_, Self> {
        Self {
            meters,
            which,
            needles: Cell::new([Needle::default(); CHANNELS]),
            last: Cell::new(None),
        }
        .build(cx, |_| {})
    }

    /// The paint for the lit part of a bar: the scale's colours fixed to
    /// their heights, so a bar shows green, yellow, orange and red where the
    /// scale says, however far up it reaches.
    fn scale_paint(bottom: f32, top: f32, alpha: f32) -> vg::Paint {
        let stops = [
            (0.0, GREEN),
            (deflection(-18.0), GREEN),
            (deflection(-6.0), YELLOW),
            (deflection(-1.0), ORANGE),
            (deflection(0.0), RED),
            (1.0, RED),
        ];
        vg::Paint::linear_gradient_stops(
            0.0,
            bottom,
            0.0,
            top,
            stops.map(|(at, colour)| (at, rgba(colour, alpha))),
        )
    }
}

impl View for LevelMeter {
    fn element(&self) -> Option<&'static str> {
        Some("pulteqfx-meter")
    }

    fn event(&mut self, _cx: &mut EventContext, event: &mut Event) {
        // Clicking the meter clears its held peak and its clip lamps, as it
        // does on a DAW's.
        let which = self.which;
        event.map(|window_event, meta| match window_event {
            WindowEvent::MouseDown(MouseButton::Left)
            | WindowEvent::MouseDoubleClick(MouseButton::Left)
            | WindowEvent::MouseTripleClick(MouseButton::Left) => {
                which.of(&self.meters).clear_held();
                meta.consume();
            }
            _ => {}
        });
    }

    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        let b = cx.bounds();
        let s = cx.scale_factor();
        let meter = self.which.of(&self.meters);
        let channels = self.meters.channels();

        // Frames do not arrive evenly, and the first one has nothing before it.
        let now = Instant::now();
        let dt = self
            .last
            .replace(Some(now))
            .map_or(0.0, |last| (now - last).as_secs_f32().min(0.25));

        // The window, let into the panel: dark glass, its upper and left
        // walls in shadow and its lower and right lips catching the light,
        // which is up and to the left.
        let mut well = vg::Path::new();
        well.rounded_rect(b.x, b.y, b.w, b.h, 3.0 * s);
        canvas.fill_path(&well, &vg::Paint::color(rgb(0x0a0c0e)));
        canvas.fill_path(
            &well,
            &vg::Paint::linear_gradient(
                b.x,
                b.y,
                b.x,
                b.y + 8.0 * s,
                rgba(0x000000, 0.55),
                rgba(0x000000, 0.0),
            ),
        );
        let mut lip = vg::Path::new();
        lip.move_to(b.x + b.w + 0.5 * s, b.y + 3.0 * s);
        lip.line_to(b.x + b.w + 0.5 * s, b.y + b.h + 0.5 * s);
        lip.line_to(b.x + 3.0 * s, b.y + b.h + 0.5 * s);
        canvas.stroke_path(
            &lip,
            &vg::Paint::color(rgba(0xffffff, 0.16)).with_line_width(s),
        );
        let mut rim = vg::Path::new();
        rim.move_to(b.x - 0.5 * s, b.y + b.h - 3.0 * s);
        rim.line_to(b.x - 0.5 * s, b.y - 0.5 * s);
        rim.line_to(b.x + b.w - 3.0 * s, b.y - 0.5 * s);
        canvas.stroke_path(
            &rim,
            &vg::Paint::color(rgba(0x000000, 0.45)).with_line_width(s),
        );

        let inset = INSET * s;
        let gap = 2.0 * s;
        let span = b.w - 2.0 * inset;
        let bar_w = (span - gap * (channels - 1) as f32) / channels as f32;
        let top = b.y + SCALE_TOP * s;
        let height = SCALE_H * s;
        let bottom = top + height;
        let full = Self::scale_paint(bottom, top, 1.0);
        let faint = Self::scale_paint(bottom, top, 0.42);

        let mut needles = self.needles.get();
        for (channel, needle) in needles.iter_mut().enumerate().take(channels) {
            let x = b.x + inset + channel as f32 * (bar_w + gap);
            needle.step(meters::db(meter.take_peak(channel)), dt);

            // The clip lamp, lit from the first sample at or over full scale
            // until the readout is cleared.
            let over = meter.held(channel) >= 1.0;
            let mut lamp = vg::Path::new();
            lamp.rect(x, b.y + inset, bar_w, CLIP_H * s);
            let lamp_colour = if over { rgb(RED) } else { rgb(0x341311) };
            canvas.fill_path(&lamp, &vg::Paint::color(lamp_colour));

            // The unlit bar, with the scale's marks faint across it.
            let mut track = vg::Path::new();
            track.rect(x, top, bar_w, height);
            canvas.fill_path(&track, &vg::Paint::color(rgb(0x14171a)));
            for tick in TICKS {
                let y = top + height * (1.0 - deflection(tick));
                let alpha = match tick as i32 {
                    0 => 0.22,
                    -18 => 0.14,
                    _ => 0.07,
                };
                let mut mark = vg::Path::new();
                mark.move_to(x, y);
                mark.line_to(x + bar_w, y);
                canvas.stroke_path(
                    &mark,
                    &vg::Paint::color(rgba(0xffffff, alpha)).with_line_width(0.75 * s),
                );
            }

            // Faint up to the peak, solid up to the RMS level.
            let peak_at = deflection(needle.level);
            let rms_at = deflection(meters::db_power(meter.mean_square(channel))).min(peak_at);
            if peak_at > 0.0 {
                let mut bar = vg::Path::new();
                bar.rect(x, bottom - height * peak_at, bar_w, height * peak_at);
                canvas.fill_path(&bar, &faint);
            }
            if rms_at > 0.0 {
                let mut bar = vg::Path::new();
                bar.rect(x, bottom - height * rms_at, bar_w, height * rms_at);
                canvas.fill_path(&bar, &full);
            }

            // The hold line, in the colour of the scale where it stands.
            let hold_at = deflection(needle.hold);
            if hold_at > 0.0 {
                let mut line = vg::Path::new();
                line.rect(x, bottom - height * hold_at - s, bar_w, 2.0 * s);
                canvas.fill_path(&line, &full);
            }
        }
        self.needles.set(needles);
    }
}

// ---------------------------------------------------------------------------
// Readouts
// ---------------------------------------------------------------------------

/// The PEAK figure under a meter: the highest sample on any channel since it
/// was cleared.
///
/// The labels read these through a lens on the panel's meters. vizia's
/// baseview backend reads every bound value again on every frame, so a label
/// follows the audio without anything having to tell it to. A timer could not
/// have: that backend never runs vizia's timers at all.
pub fn peak_figure(meters: &Meters, which: Which) -> String {
    let meter = which.of(meters);
    let held = (0..meters.channels())
        .map(|channel| meter.held(channel))
        .fold(0.0, f32::max);
    meters::readout(meters::db(held))
}

/// The RMS figure under a meter: the louder channel, as sampled every 200 ms
/// of audio.
pub fn rms_figure(meters: &Meters, which: Which) -> String {
    let meter = which.of(meters);
    let mean_square = (0..meters.channels())
        .map(|channel| meter.figure(channel))
        .fold(0.0, f32::max);
    meters::readout(meters::db_power(mean_square))
}

/// One figure under a meter, in a small dark window of its own. The peak
/// figure turns red once anything has reached full scale, and clicking it
/// starts it again, as clicking the meter does.
pub struct ReadoutBox {
    meters: Arc<Meters>,
    which: Which,
    peak: bool,
}

impl ReadoutBox {
    pub fn new<L>(
        cx: &mut Context,
        meters: Arc<Meters>,
        which: Which,
        peak: bool,
        text: L,
    ) -> Handle<'_, Self>
    where
        L: Lens<Target = String>,
    {
        Self {
            meters,
            which,
            peak,
        }
        .build(cx, move |cx| {
            Label::new(cx, text)
                .width(Stretch(1.0))
                .height(Stretch(1.0))
                .child_left(Stretch(1.0))
                .child_right(Stretch(1.0))
                .child_top(Stretch(1.0))
                .child_bottom(Stretch(1.0))
                .font_family(vec![FamilyOwned::Name(String::from(assets::NOTO_SANS))])
                .font_weight(FontWeightKeyword::Bold)
                .font_size(10.0)
                .color(Color::rgb(0xe6, 0xec, 0xf0))
                .hoverable(false);
        })
    }
}

impl View for ReadoutBox {
    fn element(&self) -> Option<&'static str> {
        Some("pulteqfx-readout")
    }

    fn event(&mut self, _cx: &mut EventContext, event: &mut Event) {
        if !self.peak {
            return;
        }
        let which = self.which;
        event.map(|window_event, meta| match window_event {
            WindowEvent::MouseDown(MouseButton::Left)
            | WindowEvent::MouseDoubleClick(MouseButton::Left)
            | WindowEvent::MouseTripleClick(MouseButton::Left) => {
                which.of(&self.meters).clear_held();
                meta.consume();
            }
            _ => {}
        });
    }

    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        let b = cx.bounds();
        let s = cx.scale_factor();
        let meter = self.which.of(&self.meters);
        let over =
            self.peak && (0..self.meters.channels()).any(|channel| meter.held(channel) >= 1.0);

        let mut path = vg::Path::new();
        path.rounded_rect(b.x, b.y, b.w, b.h, 2.5 * s);
        let (fill, edge) = if over {
            (rgb(0x7c1712), rgba(RED, 0.9))
        } else {
            (rgb(0x0a0c0e), rgba(0xffffff, 0.14))
        };
        canvas.fill_path(&path, &vg::Paint::color(fill));
        canvas.stroke_path(&path, &vg::Paint::color(edge).with_line_width(s));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_scale_runs_from_nothing_to_six_over() {
        assert_eq!(deflection(f32::NEG_INFINITY), 0.0);
        assert_eq!(deflection(-70.0), 0.0);
        assert_eq!(deflection(6.0), 1.0);
        assert_eq!(deflection(20.0), 1.0);
        assert!((deflection(0.0) - 100.0 / 115.0).abs() < 1e-6);
        let mut last = -1.0;
        for tenth in -800..=100 {
            let at = deflection(tenth as f32 / 10.0);
            assert!(
                at >= last,
                "the scale runs backwards at {} dB",
                tenth as f32 / 10.0
            );
            last = at;
        }
    }

    /// Every engraved figure has room of its own at the size the meter is
    /// drawn, or neighbours print over each other.
    #[test]
    fn the_scale_figures_do_not_crowd_each_other() {
        for pair in TICKS.windows(2) {
            let apart = scale_y(pair[1]) - scale_y(pair[0]);
            assert!(
                apart >= 11.0,
                "{} and {} dB are only {apart:.1} px apart",
                pair[0],
                pair[1]
            );
        }
    }

    /// A bar rises to a peak at once, and falls back 20 dB in 1.7 s.
    #[test]
    fn a_bar_rises_at_once_and_falls_at_the_peak_meter_rate() {
        let mut needle = Needle::default();
        needle.step(-6.0, 0.03);
        assert_eq!(needle.level, -6.0);
        for _ in 0..170 {
            needle.step(f32::NEG_INFINITY, 0.01);
        }
        assert!(
            (needle.level + 26.0).abs() < 0.01,
            "fell to {}",
            needle.level
        );
    }

    /// The hold line stays at the highest point for two seconds, then follows
    /// the bar down, and a new peak above it takes it straight back up.
    #[test]
    fn the_hold_line_waits_then_follows() {
        let mut needle = Needle::default();
        needle.step(-3.0, 0.03);
        for _ in 0..190 {
            needle.step(f32::NEG_INFINITY, 0.01);
        }
        assert_eq!(needle.hold, -3.0, "still held after 1.9 s");
        for _ in 0..100 {
            needle.step(f32::NEG_INFINITY, 0.01);
        }
        assert!(needle.hold < -3.0, "falling after 2 s");
        assert!(needle.hold >= needle.level);
        needle.step(-1.0, 0.01);
        assert_eq!(needle.hold, -1.0);
    }

    /// Nothing is not a number the bar can fall from.
    #[test]
    fn silence_rests_the_bar_on_the_floor() {
        let mut needle = Needle::default();
        for _ in 0..100 {
            needle.step(f32::NEG_INFINITY, 0.25);
        }
        assert_eq!(needle.level, FLOOR);
        assert!(needle.hold.is_finite());
    }

    #[test]
    fn clearing_a_meter_starts_its_peak_figure_again() {
        let meters = Meters::default();
        meters.output.publish(0, 1.2, 0.5);
        meters.output.publish_figure(0, 0.5);
        assert_eq!(peak_figure(&meters, Which::Output), "+1.6");
        assert_eq!(peak_figure(&meters, Which::Input), "-inf");
        meters.output.clear_held();
        assert_eq!(peak_figure(&meters, Which::Output), "-inf");
        assert_eq!(
            rms_figure(&meters, Which::Output),
            "-3.0",
            "the average is not a held figure"
        );
    }
}
