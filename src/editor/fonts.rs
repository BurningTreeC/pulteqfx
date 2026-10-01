//! The panel's typeface, embedded so the editor never depends on what happens
//! to be installed.
//!
//! nih-plug's vizia adapter shipped Noto Sans and registered it on request.
//! The adapter used now ships no fonts at all, so the same two faces are
//! carried here instead, unchanged; see `assets/fonts/NOTICE`.

use vizia_plug::vizia::prelude::Context;

/// The family name both faces register under. Bold is chosen by weight.
pub const NOTO_SANS: &str = "Noto Sans";

pub fn register_noto_sans_regular(cx: &mut Context) {
    cx.load_font_mem(include_bytes!("../../assets/fonts/NotoSans-Regular.ttf"));
}

pub fn register_noto_sans_bold(cx: &mut Context) {
    cx.load_font_mem(include_bytes!("../../assets/fonts/NotoSans-Bold.ttf"));
}
