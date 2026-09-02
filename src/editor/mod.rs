//! The PultEQFx front panel.
//!
//! Everything is laid out in the panel's own coordinates, taken off the
//! hardware: two rows of controls on a petrol blue 19 inch rack panel, boost
//! and attenuate for each band along the top, the frequency selectors and the
//! bandwidth control along the bottom, the equaliser switch at the left and
//! the pilot lamp and power switch at the right.
//!
//! Above the panel sits a thin strip that belongs to the plugin rather than
//! the hardware: it holds the settings button, and behind it the window scale,
//! the oversampling setting and the amplifier's drive and output trim.

mod panel;
mod sprites;
mod settings;
mod style;
mod widgets;

use nih_plug::prelude::Editor;
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::{assets, create_vizia_editor, ViziaState, ViziaTheming};
use std::sync::Arc;

use crate::params::{HighAttenFreq, HighBoostFreq, LowFreq, PultEqFxParams};
use panel::Faceplate;
use settings::{Dialogs, Header, SettingsOverlay, UiState};
use style::*;
use widgets::{Knob, Lamp, Selector, Toggle};

#[derive(Lens)]
pub struct Panel {
    pub params: Arc<PultEqFxParams>,
}

impl Model for Panel {}

pub fn default_state() -> Arc<ViziaState> {
    ViziaState::new(|| (PANEL_W as u32, WINDOW_H as u32))
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
const LAMP_X: f32 = 934.0;
const LAMP_Y: f32 = 167.0;

/// Height of a label box, which is centred on its anchor point.
const LABEL_H: f32 = 18.0;

pub fn create(params: Arc<PultEqFxParams>, editor_state: Arc<ViziaState>) -> Option<Box<dyn Editor>> {
    let state = editor_state.clone();
    create_vizia_editor(editor_state, ViziaTheming::None, move |cx, _| {
        assets::register_noto_sans_regular(cx);
        assets::register_noto_sans_bold(cx);

        Panel {
            params: params.clone(),
        }
        .build(cx);
        UiState::new(state.user_scale_factor(), params.clone()).build(cx);

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

    selector_scale(cx, ATTEN_SEL_X, TOP_ROW, &HighAttenFreq::LABELS);
    Selector::new(cx, Panel::params, |p| &p.high_atten_freq, R_SELECTOR, 3, true)
        .place(ATTEN_SEL_X, TOP_ROW, R_SELECTOR);

    // --- lower row ----------------------------------------------------------
    small_engraved(cx, "IN", EQ_SWITCH_X, 194.0, 9.0);
    small_engraved(cx, "OUT", EQ_SWITCH_X, 296.0, 9.0);
    Toggle::new(cx, Panel::params, |p| &p.eq_in)
        .position_type(PositionType::SelfDirected)
        .left(Pixels(EQ_SWITCH_X - 17.0))
        .top(Pixels(BOTTOM_ROW - 29.0));

    engraved(cx, "CPS", LOW_FREQ_X, 172.0, 10.0);
    selector_scale(cx, LOW_FREQ_X, BOTTOM_ROW, &LowFreq::LABELS);
    Selector::new(cx, Panel::params, |p| &p.low_freq, R_SELECTOR, 4, true)
        .place(LOW_FREQ_X, BOTTOM_ROW, R_SELECTOR);
    engraved(cx, "LOW FREQUENCY", LOW_FREQ_X, 302.0, 11.0);

    dial_scale(cx, BANDWIDTH_X, BOTTOM_ROW);
    Knob::new(cx, Panel::params, |p| &p.bandwidth, R_LARGE).place(BANDWIDTH_X, BOTTOM_ROW, R_LARGE);
    small_engraved(cx, "SHARP", BANDWIDTH_X - 74.0, 286.0, 8.5);
    small_engraved(cx, "BROAD", BANDWIDTH_X + 74.0, 286.0, 8.5);
    engraved(cx, "BANDWIDTH", BANDWIDTH_X, 305.0, 11.0);

    engraved(cx, "KCS", HIGH_FREQ_X, 172.0, 10.0);
    selector_scale(cx, HIGH_FREQ_X, BOTTOM_ROW, &HighBoostFreq::LABELS);
    Selector::new(cx, Panel::params, |p| &p.high_boost_freq, R_SELECTOR, 7, true)
        .place(HIGH_FREQ_X, BOTTOM_ROW, R_SELECTOR);
    engraved(cx, "HIGH FREQUENCY", HIGH_FREQ_X, 302.0, 11.0);

    // --- lamp and power -----------------------------------------------------
    Lamp::new(cx, Panel::params, |p| &p.power)
        .position_type(PositionType::SelfDirected)
        .left(Pixels(LAMP_X - 14.0))
        .top(Pixels(LAMP_Y - 14.0));

    small_engraved(cx, "OFF", POWER_X - 28.0, 206.0, 9.0);
    small_engraved(cx, "ON", POWER_X + 26.0, 212.0, 9.0);
    Selector::new(cx, Panel::params, |p| &p.power, R_SMALL, 2, false)
        .place(POWER_X, BOTTOM_ROW, R_SMALL);

    // --- nameplate ----------------------------------------------------------
    plate(cx, "PULTEQFX", 154.0, 129.0, 11.0);
    plate(cx, "PROGRAM EQUALIZER", 154.0, 150.0, 11.0);
    plate(cx, "BURNINGTREEC", 154.0, 169.0, 11.0);
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

fn lettering(cx: &mut Context, text: &str, x: f32, y: f32, size: f32, spaced: bool) {
    // The hardware's lettering is widely tracked; a thin space between the
    // characters is the closest this text stack can get.
    let text = if spaced { track_out(text) } else { text.to_string() };
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
        .color(Color::rgba(r, g, b, a));
}

/// The 0 to 10 scale engraved around a large knob.
fn dial_scale(cx: &mut Context, x: f32, y: f32) {
    for i in 0..=10 {
        let (nx, ny) = polar(x, y, SCALE_RADIUS, knob_angle(i as f32 / 10.0));
        numeral(cx, &i.to_string(), nx, ny);
    }
}

/// The frequencies engraved around a selector.
fn selector_scale(cx: &mut Context, x: f32, y: f32, labels: &[&str]) {
    for (i, text) in labels.iter().enumerate() {
        let (nx, ny) = polar(x, y, SELECTOR_RADIUS, selector_angle(i, labels.len()));
        numeral(cx, text, nx, ny);
    }
}

fn numeral(cx: &mut Context, text: &str, x: f32, y: f32) {
    label_box(cx, text, x, y + 1.0, 9.5, 26.0, 0, 0, 0, 110);
    label_box(cx, text, x, y, 9.5, 26.0, 0xef, 0xf1, 0xf3, 255);
}
