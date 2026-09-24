//! The PultEQFx front panel.
//!
//! Everything is laid out in the panel's own coordinates, taken off the
//! hardware: two rows of controls on a petrol blue 19 inch rack panel, boost
//! and attenuate for each band along the top, the frequency selectors and the
//! bandwidth control along the bottom, the equaliser switch at the left and
//! the pilot lamp and power switch at the right.
//!
//! Above the panel sits a thin strip that belongs to the plugin rather than
//! the hardware: it holds the settings button, and behind it the window scale
//! and the oversampling setting. Nor are the level meters either side of the
//! controls on the hardware, or the amplifier's drive and output trim beside
//! the output meter; they are there for gain staging.

pub mod meter;
mod panel;
pub mod settings;
mod sprites;
pub mod style;
mod widgets;

use nih_plug::prelude::{Editor, Param, ParamPtr};
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::{assets, create_vizia_editor, ViziaState, ViziaTheming};
use std::sync::Arc;

use crate::meters::Meters;
use crate::params::{HighAttenFreq, HighBoostFreq, LowFreq, PultEqFxParams};
use meter::{LevelMeter, ReadoutBox, Which};
use nih_plug::prelude::FloatParam;
use panel::Faceplate;
use settings::{Dialogs, Header, SettingsOverlay, UiState};
use style::*;
use widgets::{Detent, Knob, Lamp, Selector};

#[derive(Lens)]
pub struct Panel {
    pub params: Arc<PultEqFxParams>,
    pub meters: Arc<Meters>,
}

impl Model for Panel {}

pub fn default_state() -> Arc<ViziaState> {
    ViziaState::new_with_default_scale_factor(|| (PANEL_W as u32, WINDOW_H as u32), 1.0)
}
/// Updates the scale used by `Editor::size()` and saved in the host session.
/// Vizia's drawing scale is separate and must only change after the host has
/// accepted the resize. `PersistentField::set` copies the carrier's scale;
/// the original state's size function and open status stay intact.
pub fn remember_scale(state: &Arc<ViziaState>, scale: f64) {
    use nih_plug::params::persist::PersistentField;
    let carrier = ViziaState::new_with_default_scale_factor(|| (0, 0), scale);
    if let Ok(carrier) = Arc::try_unwrap(carrier) {
        PersistentField::set(state, carrier);
    }
}

// Where each control sits on the panel, measured off the hardware.
const LOW_BOOST_X: f32 = 330.0;
const LOW_ATTEN_X: f32 = 488.0;
const HIGH_BOOST_X: f32 = 683.0;
const HIGH_ATTEN_X: f32 = 831.0;
const ATTEN_SEL_X: f32 = 965.0;

const EQ_SWITCH_X: f32 = 293.0;
const LOW_FREQ_X: f32 = 400.0;
const BANDWIDTH_X: f32 = 580.0;
const HIGH_FREQ_X: f32 = 754.0;
const POWER_X: f32 = 985.0;
/// The left edge of the nameplate lettering, which is set flush left.
const NAMEPLATE_X: f32 = 154.0;
const LAMP_X: f32 = 934.0;
const LAMP_Y: f32 = 167.0;

// The level meters: the input between the left hand mounting screws and the
// nameplate, the output just right of the power switch. These are the centres
// of their windows; both carry their scale on the left.
const INPUT_METER_X: f32 = 100.0;
const OUTPUT_METER_X: f32 = 1071.0;
const METER_TOP: f32 = 34.0;

// The amplifier's drive and output trim, one above the other between the
// output meter and the right hand mounting screws.
const TRIM_X: f32 = 1112.0;
const DRIVE_Y: f32 = 92.0;
const OUTPUT_TRIM_Y: f32 = 182.0;
const R_TRIM: f32 = 15.0;
/// How far the engraved scale's figures stand off the window, centre to edge.
const METER_SCALE_GAP: f32 = 13.0;
/// Width of the box a scale figure sits in.
const METER_NUMERAL_W: f32 = 22.0;
/// The two readouts under each meter, and the captions over them.
const READOUT_W: f32 = 48.0;
const READOUT_H: f32 = 16.0;
const PEAK_CAPTION_Y: f32 = 251.0;
const PEAK_READOUT_Y: f32 = 257.0;
const RMS_CAPTION_Y: f32 = 283.0;
const RMS_READOUT_Y: f32 = 289.0;

/// Stores the requested scale before the host reads `Editor::size()`.
/// Returns whether the UI should adopt it. A refusal restores the persisted
/// size, so drawing and host geometry continue to agree.
pub fn apply_scale(
    state: &Arc<ViziaState>,
    gui: &dyn nih_plug::prelude::GuiContext,
    scale: f64,
) -> bool {
    let previous = state.user_scale_factor();
    if scale == previous {
        return true;
    }
    remember_scale(state, scale);
    if gui.request_resize() {
        true
    } else {
        remember_scale(state, previous);
        false
    }
}

/// Height of a label box, which is centred on its anchor point.
const LABEL_H: f32 = 18.0;
/// Width of the box an engraved numeral sits in.
const NUMERAL_W: f32 = 26.0;
/// Width of the click target over a word beside a switch.
const WORD_W: f32 = 30.0;

pub fn create(
    params: Arc<PultEqFxParams>,
    editor_state: Arc<ViziaState>,
    meters: Arc<Meters>,
) -> Option<Box<dyn Editor>> {
    let state = editor_state.clone();
    create_vizia_editor(editor_state, ViziaTheming::None, move |cx, gui| {
        assets::register_noto_sans_regular(cx);
        assets::register_noto_sans_bold(cx);
        // Parse failures are not reported: vizia drops the sheet silently,
        // which is what `settings::sheet_tests` guards against.
        let _ = cx.add_stylesheet(settings::STYLESHEET);

        Panel {
            params: params.clone(),
            meters: meters.clone(),
        }
        .build(cx);
        UiState::new(state.user_scale_factor(), params.clone(), gui).build(cx);

        // Nothing has to ask for the meters to be drawn again, nor for their
        // figures to be read again. vizia's baseview backend draws the whole
        // window on every frame, and reads every bound value again on every
        // frame, which is how the bars and the figures follow the audio.
        //
        // A timer would not do, twice over. That backend never runs vizia's
        // timers -- only the winit one does -- and starting a second one hangs
        // outright: `modify_timer` peeks at the earliest running timer and
        // only takes it off if it is the one asked for, otherwise peeking
        // again for ever. That is how this editor once never opened.

        Header::new(cx);

        // The panel proper, offset below the header. Everything inside it is
        // positioned in panel coordinates.
        VStack::new(cx, faceplate)
            .position_type(PositionType::SelfDirected)
            .left(Pixels(0.0))
            .top(Pixels(HEADER_H))
            .width(Pixels(PANEL_W))
            .height(Pixels(PANEL_H));

        SettingsOverlay::new(cx);
        Dialogs::new(cx);
    })
}

fn faceplate(cx: &mut Context) {
    Faceplate::new(cx);

    // --- upper row ----------------------------------------------------------
    engraved(cx, "BOOST", LOW_BOOST_X, 21.0, 11.0);
    engraved(cx, "ATTEN", LOW_ATTEN_X, 21.0, 11.0);
    engraved(cx, "BOOST", HIGH_BOOST_X, 21.0, 11.0);
    engraved(cx, "ATTEN", HIGH_ATTEN_X, 21.0, 11.0);
    engraved(cx, "ATTEN SEL", ATTEN_SEL_X, 21.0, 11.0);

    for x in [LOW_BOOST_X, LOW_ATTEN_X, HIGH_BOOST_X, HIGH_ATTEN_X] {
        dial_scale(cx, x, TOP_ROW);
    }
    Knob::new(cx, Panel::params, |p| &p.low_boost, R_LARGE).place(LOW_BOOST_X, TOP_ROW, R_LARGE);
    Knob::new(cx, Panel::params, |p| &p.low_atten, R_LARGE).place(LOW_ATTEN_X, TOP_ROW, R_LARGE);
    Knob::new(cx, Panel::params, |p| &p.high_boost, R_LARGE).place(HIGH_BOOST_X, TOP_ROW, R_LARGE);
    Knob::new(cx, Panel::params, |p| &p.high_atten, R_LARGE).place(HIGH_ATTEN_X, TOP_ROW, R_LARGE);

    let high_atten_freq = Panel::params.get(cx).high_atten_freq.as_ptr();
    selector_scale(
        cx,
        ATTEN_SEL_X,
        TOP_ROW,
        &HighAttenFreq::LABELS,
        high_atten_freq,
    );
    Selector::new(
        cx,
        Panel::params,
        |p| &p.high_atten_freq,
        R_SELECTOR,
        3,
        true,
        false,
    )
    .place(ATTEN_SEL_X, TOP_ROW, R_SELECTOR);

    // --- lower row ----------------------------------------------------------
    // The equaliser switch. It was a bat handle toggle; it is a rotary switch
    // wearing the same knurled metal knob as every other switch on the panel,
    // thrown between IN at the top left and OUT at the top right. Both sit at
    // exactly forty-five degrees from the shaft, which is where the pointer
    // aims, so dx and dy are equal.
    small_engraved(cx, "IN", EQ_SWITCH_X - 28.0, BOTTOM_ROW - 28.0, 9.0);
    small_engraved(cx, "OUT", EQ_SWITCH_X + 28.0, BOTTOM_ROW - 28.0, 9.0);
    // IN is on the left, and is the parameter's true.
    let eq_in = Panel::params.get(cx).eq_in.as_ptr();
    position(
        cx,
        eq_in,
        1.0,
        EQ_SWITCH_X - 28.0,
        BOTTOM_ROW - 28.0,
        WORD_W,
    );
    position(
        cx,
        eq_in,
        0.0,
        EQ_SWITCH_X + 28.0,
        BOTTOM_ROW - 28.0,
        WORD_W,
    );
    Selector::new(cx, Panel::params, |p| &p.eq_in, R_SMALL, 2, false, true).place(
        EQ_SWITCH_X,
        BOTTOM_ROW,
        R_SMALL,
    );

    engraved(cx, "CPS", LOW_FREQ_X, 172.0, 10.0);
    let low_freq = Panel::params.get(cx).low_freq.as_ptr();
    selector_scale(cx, LOW_FREQ_X, BOTTOM_ROW, &LowFreq::LABELS, low_freq);
    Selector::new(
        cx,
        Panel::params,
        |p| &p.low_freq,
        R_SELECTOR,
        4,
        true,
        false,
    )
    .place(LOW_FREQ_X, BOTTOM_ROW, R_SELECTOR);
    engraved(cx, "LOW FREQUENCY", LOW_FREQ_X, 302.0, 11.0);

    dial_scale(cx, BANDWIDTH_X, BOTTOM_ROW);
    Knob::new(cx, Panel::params, |p| &p.bandwidth, R_LARGE).place(BANDWIDTH_X, BOTTOM_ROW, R_LARGE);
    small_engraved(cx, "SHARP", BANDWIDTH_X - 74.0, 286.0, 8.5);
    small_engraved(cx, "BROAD", BANDWIDTH_X + 74.0, 286.0, 8.5);
    engraved(cx, "BANDWIDTH", BANDWIDTH_X, 305.0, 11.0);

    engraved(cx, "KCS", HIGH_FREQ_X, 172.0, 10.0);
    let high_boost_freq = Panel::params.get(cx).high_boost_freq.as_ptr();
    selector_scale(
        cx,
        HIGH_FREQ_X,
        BOTTOM_ROW,
        &HighBoostFreq::LABELS,
        high_boost_freq,
    );
    Selector::new(
        cx,
        Panel::params,
        |p| &p.high_boost_freq,
        R_SELECTOR,
        7,
        true,
        false,
    )
    .place(HIGH_FREQ_X, BOTTOM_ROW, R_SELECTOR);
    engraved(cx, "HIGH FREQUENCY", HIGH_FREQ_X, 302.0, 11.0);

    // --- lamp and power -----------------------------------------------------
    Lamp::new(cx, Panel::params, |p| &p.power)
        .position_type(PositionType::SelfDirected)
        .left(Pixels(LAMP_X - 14.0))
        .top(Pixels(LAMP_Y - 14.0));

    // Both sit at exactly forty-five degrees from the knob's centre, which is
    // where the pointer now aims: dx and dy equal, above and either side of
    // the shaft at (POWER_X, BOTTOM_ROW).
    small_engraved(cx, "OFF", POWER_X - 28.0, BOTTOM_ROW - 28.0, 9.0);
    small_engraved(cx, "ON", POWER_X + 28.0, BOTTOM_ROW - 28.0, 9.0);
    let power = Panel::params.get(cx).power.as_ptr();
    position(cx, power, 0.0, POWER_X - 28.0, BOTTOM_ROW - 28.0, WORD_W);
    position(cx, power, 1.0, POWER_X + 28.0, BOTTOM_ROW - 28.0, WORD_W);
    Selector::new(cx, Panel::params, |p| &p.power, R_SMALL, 2, false, false)
        .place(POWER_X, BOTTOM_ROW, R_SMALL);

    // --- meters -------------------------------------------------------------
    level_meter(cx, Which::Input, INPUT_METER_X);
    level_meter(cx, Which::Output, OUTPUT_METER_X);

    // --- amplifier trims ----------------------------------------------------
    // Beside the output meter, because that is what they are set against: the
    // drive above, the output trim below.
    trim(cx, "DRIVE", |p| &p.drive, DRIVE_Y, |p| p.drive.to_string());
    trim(
        cx,
        "OUTPUT",
        |p| &p.output,
        OUTPUT_TRIM_Y,
        |p| p.output.to_string(),
    );

    // --- nameplate ----------------------------------------------------------
    plate(cx, "PULTEQFX", NAMEPLATE_X, 126.0, 11.0);
    // The version, under the name. Smaller than the rest of the plate: it is
    // there to be quoted when reporting a fault, not read every session.
    plate(
        cx,
        concat!("V", env!("CARGO_PKG_VERSION")),
        NAMEPLATE_X,
        142.0,
        7.5,
    );
    plate(cx, "PROGRAM EQUALIZER", NAMEPLATE_X, 158.0, 11.0);
    plate(cx, "BURNINGTREEC", NAMEPLATE_X, 177.0, 11.0);
}

/// Extension for dropping a widget onto the panel at a centre point.
pub trait Place {
    fn place(self, x: f32, y: f32, radius: f32) -> Self;
}

impl<V: View> Place for Handle<'_, V> {
    fn place(self, x: f32, y: f32, radius: f32) -> Self {
        self.position_type(PositionType::SelfDirected)
            .left(Pixels(x - radius))
            .top(Pixels(y - radius))
    }
}

/// Panel lettering, engraved and filled with white.
fn engraved(cx: &mut Context, text: &str, x: f32, y: f32, size: f32) {
    lettering(cx, text, x, y, size, true);
}

fn small_engraved(cx: &mut Context, text: &str, x: f32, y: f32, size: f32) {
    lettering(cx, text, x, y, size, false);
}

/// One of the amplifier's trims: a small knob, its name, and what it is set to.
///
/// The setting is read through a lens, and vizia reads every bound value
/// again on every frame, so it follows the knob, the host's automation and a
/// preset alike.
fn trim(
    cx: &mut Context,
    name: &str,
    param: fn(&Arc<PultEqFxParams>) -> &FloatParam,
    y: f32,
    setting: fn(&Arc<PultEqFxParams>) -> String,
) {
    Knob::new(cx, Panel::params, param, R_TRIM).place(TRIM_X, y, R_TRIM);
    // Below the knob as drawn, which is wider than its layout radius.
    let below = y + R_TRIM * sprites::KNOB_LARGE_DRAW / 2.0;
    small_engraved(cx, name, TRIM_X, below + 11.0, 8.0);
    let setting = Panel::params.map(setting);
    for (dy, (r, g, b, a)) in [(1.0, (0, 0, 0, 110)), (0.0, (0xea, 0xec, 0xf0, 255))] {
        Label::new(cx, setting)
            .position_type(PositionType::SelfDirected)
            .left(Pixels(TRIM_X - TRIM_TEXT_W / 2.0))
            .top(Pixels(below + 23.0 + dy - LABEL_H / 2.0))
            .width(Pixels(TRIM_TEXT_W))
            .height(Pixels(LABEL_H))
            .child_left(Stretch(1.0))
            .child_right(Stretch(1.0))
            .child_top(Stretch(1.0))
            .child_bottom(Stretch(1.0))
            .font_family(vec![FamilyOwned::Name(String::from(assets::NOTO_SANS))])
            .font_weight(FontWeightKeyword::Bold)
            .font_size(8.0)
            .color(Color::rgba(r, g, b, a))
            .hoverable(false);
    }
}

/// Width of the box a trim's setting is lettered in.
const TRIM_TEXT_W: f32 = 56.0;

fn lettering(cx: &mut Context, text: &str, x: f32, y: f32, size: f32, spaced: bool) {
    // The hardware's lettering is widely tracked; a thin space between the
    // characters is the closest this text stack can get.
    let text = if spaced {
        track_out(text)
    } else {
        text.to_string()
    };
    let width = size * text.chars().count() as f32 * 0.9 + 40.0;

    // The shadow half of the engraving, then the lit half.
    label_box(cx, &text, x, y + 1.0, size, width, 0x00, 0x00, 0x00, 110);
    label_box(cx, &text, x, y, size, width, 0xea, 0xec, 0xf0, 255);
}

fn track_out(text: &str) -> String {
    text.chars()
        .map(|c| c.to_string())
        .collect::<Vec<_>>()
        .join("\u{2009}")
}

/// The nameplate block, which is left aligned rather than centred.
fn plate(cx: &mut Context, text: &str, x: f32, y: f32, size: f32) {
    let text = track_out(text);
    for (dy, (r, g, b, a)) in [(1.0, (0, 0, 0, 110)), (0.0, (0xea, 0xec, 0xf0, 255))] {
        Label::new(cx, &text)
            .position_type(PositionType::SelfDirected)
            .left(Pixels(x))
            .top(Pixels(y + dy - LABEL_H / 2.0))
            .width(Pixels(200.0))
            .height(Pixels(LABEL_H))
            .child_top(Stretch(1.0))
            .child_bottom(Stretch(1.0))
            .font_family(vec![FamilyOwned::Name(String::from(assets::NOTO_SANS))])
            .font_weight(FontWeightKeyword::Bold)
            .font_size(size)
            .color(Color::rgba(r, g, b, a));
    }
}

#[allow(clippy::too_many_arguments)]
pub fn label_box(
    cx: &mut Context,
    text: &str,
    x: f32,
    y: f32,
    size: f32,
    width: f32,
    r: u8,
    g: u8,
    b: u8,
    a: u8,
) {
    Label::new(cx, text)
        .position_type(PositionType::SelfDirected)
        .left(Pixels(x - width / 2.0))
        .top(Pixels(y - LABEL_H / 2.0))
        .width(Pixels(width))
        .height(Pixels(LABEL_H))
        .child_left(Stretch(1.0))
        .child_right(Stretch(1.0))
        .child_top(Stretch(1.0))
        .child_bottom(Stretch(1.0))
        .font_family(vec![FamilyOwned::Name(String::from(assets::NOTO_SANS))])
        .font_weight(FontWeightKeyword::Bold)
        .font_size(size)
        .color(Color::rgba(r, g, b, a))
        // Lettering is never the thing being clicked, and leaving it in the
        // way of the pointer breaks whatever it is drawn over. These labels
        // are positioned on top of the controls they annotate, and a later
        // sibling is the one the hit test finds -- events then travel up to
        // parents, never sideways to the control underneath. That is why the
        // oversampling switch could not be clicked at all.
        .hoverable(false);
}

/// The 0 to 10 scale engraved around a large knob.
fn dial_scale(cx: &mut Context, x: f32, y: f32) {
    for i in 0..=10 {
        let (nx, ny) = polar(x, y, SCALE_RADIUS, knob_angle(i as f32 / 10.0));
        numeral(cx, &i.to_string(), nx, ny);
    }
}

/// The frequencies engraved around a selector, each of which selects itself
/// when clicked.
fn selector_scale(cx: &mut Context, x: f32, y: f32, labels: &[&str], param: ParamPtr) {
    let count = labels.len();
    let at = |i: usize| polar(x, y, SELECTOR_RADIUS, selector_angle(i, count));
    // As wide as a numeral, but never so wide that neighbours overlap where
    // the positions crowd together, as the seven high frequencies do.
    let width = if count > 1 {
        let ((x0, y0), (x1, y1)) = (at(0), at(1));
        ((x1 - x0).hypot(y1 - y0) - 1.0).min(NUMERAL_W)
    } else {
        NUMERAL_W
    };
    for (i, text) in labels.iter().enumerate() {
        let (nx, ny) = at(i);
        numeral(cx, text, nx, ny);
        let normalized = i as f32 / (count - 1).max(1) as f32;
        position(cx, param, normalized, nx, ny, width);
    }
}

/// A level meter with its title, its engraved scale and its two readouts.
fn level_meter(cx: &mut Context, which: Which, x: f32) {
    let meters = Panel::meters.get(cx);
    let title = match which {
        Which::Input => "INPUT",
        Which::Output => "OUTPUT",
    };
    // On the same line as the upper row's lettering.
    engraved(cx, title, x, 21.0, 11.0);

    LevelMeter::new(cx, meters.clone(), which)
        .position_type(PositionType::SelfDirected)
        .left(Pixels(x - meter::WIDTH / 2.0))
        .top(Pixels(METER_TOP))
        .width(Pixels(meter::WIDTH))
        .height(Pixels(meter::HEIGHT));

    let figures_x = x - meter::WIDTH / 2.0 - METER_SCALE_GAP;
    for tick in meter::TICKS {
        let text = if tick > 0.0 {
            format!("+{tick}")
        } else {
            format!("{tick}")
        };
        let y = METER_TOP + meter::scale_y(tick);
        label_box(
            cx,
            &text,
            figures_x,
            y + 1.0,
            7.5,
            METER_NUMERAL_W,
            0,
            0,
            0,
            110,
        );
        label_box(
            cx,
            &text,
            figures_x,
            y,
            7.5,
            METER_NUMERAL_W,
            0xef,
            0xf1,
            0xf3,
            255,
        );
    }

    for (caption, caption_y, box_y, peak) in [
        ("PEAK", PEAK_CAPTION_Y, PEAK_READOUT_Y, true),
        ("RMS", RMS_CAPTION_Y, RMS_READOUT_Y, false),
    ] {
        small_engraved(cx, caption, x, caption_y, 8.0);
        let figure = Panel::meters.map(move |meters| {
            if peak {
                meter::peak_figure(meters, which)
            } else {
                meter::rms_figure(meters, which)
            }
        });
        let handle = ReadoutBox::new(cx, meters.clone(), which, peak, figure);
        handle
            .position_type(PositionType::SelfDirected)
            .left(Pixels(x - READOUT_W / 2.0))
            .top(Pixels(box_y))
            .width(Pixels(READOUT_W))
            .height(Pixels(READOUT_H));
    }
}

fn numeral(cx: &mut Context, text: &str, x: f32, y: f32) {
    label_box(cx, text, x, y + 1.0, 9.5, NUMERAL_W, 0, 0, 0, 110);
    label_box(cx, text, x, y, 9.5, NUMERAL_W, 0xef, 0xf1, 0xf3, 255);
}

/// Makes an engraved position clickable: clicking it sets `param` to
/// `normalized`, as turning the switch there would.
fn position(cx: &mut Context, param: ParamPtr, normalized: f32, x: f32, y: f32, width: f32) {
    Detent::new(cx, param, normalized)
        .position_type(PositionType::SelfDirected)
        .left(Pixels(x - width / 2.0))
        .top(Pixels(y - LABEL_H / 2.0))
        .width(Pixels(width))
        .height(Pixels(LABEL_H));
}

#[cfg(test)]
mod layout_tests {
    use super::panel::{HARDWARE_Y, SCREW_SIZE, SCREW_X};
    use super::*;

    /// How far across the panel a meter reaches, with its scale and readouts.
    fn meter_extent(x: f32) -> (f32, f32) {
        let half = meter::WIDTH / 2.0;
        let figures = x - half - METER_SCALE_GAP;
        let spans = [
            (x - half, x + half),
            (
                figures - METER_NUMERAL_W / 2.0,
                figures + METER_NUMERAL_W / 2.0,
            ),
            (x - READOUT_W / 2.0, x + READOUT_W / 2.0),
        ];
        (
            spans.iter().map(|span| span.0).fold(f32::MAX, f32::min),
            spans.iter().map(|span| span.1).fold(f32::MIN, f32::max),
        )
    }

    /// Left of all the controls and clear of the nameplate, without running
    /// into the mounting screws.
    #[test]
    fn the_input_meter_sits_between_the_screws_and_the_nameplate() {
        let (left, right) = meter_extent(INPUT_METER_X);
        let screw = PANEL_W * SCREW_X[0] + SCREW_SIZE / 2.0;
        assert!(left > screw + 4.0, "the scale runs into the screws");
        assert!(
            right < NAMEPLATE_X - 8.0,
            "the meter runs into the nameplate"
        );
        assert!(
            right < EQ_SWITCH_X - WORD_W,
            "the meter is not left of the controls"
        );
    }

    /// How far out from its centre a trim knob is drawn.
    fn trim_radius() -> f32 {
        R_TRIM * sprites::KNOB_LARGE_DRAW / 2.0
    }

    /// Right of all the controls, the words of the power switch and the
    /// attenuation selector's figures included, and left of the trims.
    #[test]
    fn the_output_meter_sits_between_the_power_switch_and_the_trims() {
        let (left, right) = meter_extent(OUTPUT_METER_X);
        let on = POWER_X + 28.0 + WORD_W / 2.0;
        let figures = ATTEN_SEL_X
            + SELECTOR_RADIUS * selector_angle(2, 3).to_radians().sin()
            + NUMERAL_W / 2.0;
        assert!(
            left > on.max(figures) + 4.0,
            "the meter is not right of the controls"
        );
        let window = OUTPUT_METER_X + meter::WIDTH / 2.0;
        assert!(
            window < TRIM_X - trim_radius() - 6.0,
            "the trims crowd the meter"
        );
        // The readouts are wider than the window and reach under the trims,
        // so the output trim's lettering has to finish above them.
        let lettering = OUTPUT_TRIM_Y + trim_radius() + 23.0 + LABEL_H / 2.0;
        assert!(
            right < TRIM_X - TRIM_TEXT_W / 2.0 || lettering < PEAK_CAPTION_Y - 5.0,
            "the output trim's setting runs into the meter's readouts"
        );
    }

    /// The trims stand one above the other, right of the output meter,
    /// between the right hand mounting screws and clear of both.
    #[test]
    fn the_trims_sit_between_the_right_hand_screws() {
        let r = trim_radius();
        const _: () = assert!(DRIVE_Y < OUTPUT_TRIM_Y, "the drive goes above the output");
        const _: () = assert!(
            TRIM_X + TRIM_TEXT_W / 2.0 < PANEL_W,
            "a setting runs off the panel"
        );
        for y in [DRIVE_Y, OUTPUT_TRIM_Y] {
            for screw_y in HARDWARE_Y {
                let (dx, dy) = (PANEL_W * SCREW_X[1] - TRIM_X, PANEL_H * screw_y - y);
                assert!(
                    dx.hypot(dy) > r + SCREW_SIZE / 2.0 + 4.0,
                    "a trim at {y} runs into a screw"
                );
            }
        }
        let drive_bottom = DRIVE_Y + r + 23.0 + LABEL_H / 2.0;
        assert!(
            drive_bottom < OUTPUT_TRIM_Y - r,
            "the drive's lettering runs into the output knob"
        );
        let screw_below = PANEL_H * HARDWARE_Y[1] - SCREW_SIZE / 2.0;
        assert!(
            OUTPUT_TRIM_Y + r + 23.0 + LABEL_H / 2.0 < screw_below,
            "the output's lettering runs into the screw below it"
        );
    }

    /// Checked when this is compiled rather than when it runs: everything in
    /// it is a constant, so as a run time test it could only ever pass or only
    /// ever fail, and `-D warnings` in CI says so.
    #[test]
    fn the_meters_fit_the_height_of_the_panel() {
        const _: () = assert!(
            METER_TOP > 21.0 + LABEL_H / 2.0,
            "the window covers its title"
        );
        const _: () = assert!(
            METER_TOP + meter::HEIGHT < PEAK_CAPTION_Y - 5.0,
            "the window runs into its readouts"
        );
        const _: () = assert!(
            PEAK_READOUT_Y + READOUT_H < RMS_CAPTION_Y - 5.0,
            "the readouts run into each other"
        );
        const _: () = assert!(
            RMS_READOUT_Y + READOUT_H < PANEL_H - 8.0,
            "the readouts run off the panel"
        );
    }
}
