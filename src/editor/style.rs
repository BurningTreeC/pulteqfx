//! Panel colours, geometry and the drawing primitives the widgets share.

use nih_plug_vizia::vizia::prelude::Canvas;
use nih_plug_vizia::vizia::vg;

/// Panel size in logical pixels, in the proportions of the 19 inch by 5.25
/// inch rack panel the hardware is built on.
pub const PANEL_W: f32 = 1160.0;
pub const PANEL_H: f32 = 322.0;
/// Height of the strip above the panel that carries the settings button.
pub const HEADER_H: f32 = 34.0;
/// Total window height.
pub const WINDOW_H: f32 = PANEL_H + HEADER_H;

/// Centre line of the upper and lower rows of controls.
pub const TOP_ROW: f32 = 88.0;
pub const BOTTOM_ROW: f32 = 234.0;

/// Radii of the sizes of knob on the panel.
pub const R_LARGE: f32 = 34.0;
pub const R_SELECTOR: f32 = 22.0;
pub const R_SMALL: f32 = 19.0;

/// Where the engraved scale sits around a large knob.
pub const SCALE_RADIUS: f32 = 48.0;
/// Where the engraved values sit around a selector.
pub const SELECTOR_RADIUS: f32 = 54.0;

/// A knob sweeps 250 degrees, zero at the lower left.
pub const SWEEP: f32 = 250.0;

// The panel's dark petrol blue enamel, lit from above.
pub const PANEL_TOP: u32 = 0x365660;
pub const PANEL_BOTTOM: u32 = 0x1e353c;

pub fn rgb(hex: u32) -> vg::Color {
    vg::Color::rgb(
        ((hex >> 16) & 0xff) as u8,
        ((hex >> 8) & 0xff) as u8,
        (hex & 0xff) as u8,
    )
}

pub fn rgba(hex: u32, alpha: f32) -> vg::Color {
    let mut c = rgb(hex);
    c.set_alphaf(alpha);
    c
}

/// Position on a circle, with angles measured clockwise from twelve o'clock.
pub fn polar(cx: f32, cy: f32, radius: f32, degrees: f32) -> (f32, f32) {
    let a = degrees.to_radians();
    (cx + radius * a.sin(), cy - radius * a.cos())
}

/// The pointer angle for a normalised control position.
pub fn knob_angle(normalized: f32) -> f32 {
    (normalized - 0.5) * SWEEP
}

/// Angles the detents of an `n` position selector sit at. The hardware's
/// switches all sweep a similar arc whatever their position count, so the
/// spacing closes up as the positions multiply.
///
/// A two position switch is the exception. Sharing the multi-position rule
/// gave it a 32 degree step, so the pointer moved sixteen degrees either side
/// of vertical while the OFF and ON engraving sits at forty-five -- the knob
/// was aimed a long way inside its own lettering. A switch with two states is
/// thrown, not nudged, and the engraving is where the hardware says the
/// pointer goes.
const TWO_POSITION_STEP: f32 = 90.0;

pub fn selector_angle(index: usize, count: usize) -> f32 {
    if count < 2 {
        return 0.0;
    }
    let step = if count == 2 {
        TWO_POSITION_STEP
    } else {
        32.0_f32.min(120.0 / (count - 1) as f32)
    };
    -step * (count - 1) as f32 / 2.0 + step * index as f32
}

/// Where the panel's light is, as a direction across the faceplate.
///
/// The renders are lit by `assetgen`'s panel rig, whose key sits up and to the
/// left of the part, so everything drawn has to agree with that or the panel
/// reads as two rooms. These are the offsets a shadow takes, in multiples of
/// the caster's radius: right and down, away from the light.
pub const SHADOW_X: f32 = 0.11;
pub const SHADOW_Y: f32 = 0.15;

/// Where the panel's light sits, in panel coordinates. Matches the highlight
/// `panel.rs` paints on the enamel; the two have to agree or the hardware is
/// lit from somewhere the faceplate is not.
const LIGHT_X: f32 = PANEL_W * 0.045;
const LIGHT_Y: f32 = -PANEL_H * 0.12;

/// How much of the panel's light reaches a point on it, as a multiplier on the
/// hardware's own colour.
///
/// The renders carry the *direction* the light comes from -- their highlights
/// are all up and to the left, because that is where `assetgen`'s rig puts the
/// key. What they cannot carry is how far from the lamp the control is bolted,
/// because a render knows nothing about the panel it ends up on. So a knob in
/// the far corner was as bright as one under the light, which is the giveaway
/// that a panel is a collage rather than a photograph.
///
/// Linear in distance rather than inverse square: a real faceplate is lit by a
/// broad source at a distance, not a point at arm's length, and an inverse
/// square across a nineteen inch panel puts the right hand end in the dark.
pub fn light_at(x: f32, y: f32) -> f32 {
    const REACH: f32 = 1165.0; // corner to corner, from the light
    const FALL: f32 = 0.26;
    let d = ((x - LIGHT_X).powi(2) + (y - LIGHT_Y).powi(2)).sqrt();
    1.0 - FALL * (d / REACH).clamp(0.0, 1.0)
}

/// The same, for a widget that knows only where its centre is on screen.
/// Panel coordinates are window pixels divided by the scale, less the header
/// strip the panel sits below.
pub fn light_at_screen(mx: f32, my: f32, scale: f32) -> f32 {
    light_at(mx / scale, my / scale - HEADER_H)
}

/// The shadow a control casts onto the panel. Every control on the panel sits
/// in the same light, so the drawn ones and the rendered knobs share this
/// rather than each carrying a shadow of its own.
pub fn contact_shadow(canvas: &mut Canvas, cx: f32, cy: f32, r: f32) {
    // Down and to the right, because the light is in the top left corner. It
    // used to fall straight down, which is a light directly overhead and
    // disagrees with every render on the panel.
    let (ox, oy) = (cx + r * SHADOW_X, cy + r * SHADOW_Y);
    let mut path = vg::Path::new();
    path.ellipse(ox, oy, r * 1.20, r * 1.14);
    canvas.fill_path(
        &path,
        &vg::Paint::radial_gradient(
            ox,
            oy,
            r * 0.72,
            r * 1.20,
            rgba(0x000000, 0.60),
            rgba(0x000000, 0.0),
        ),
    );
}

/// The pointer painted on a metal switch knob, and the shadow it throws.
///
/// The knob itself is a render and has no indicator on it, because a render
/// cannot be turned without turning its light. So the index goes on top: a
/// dark groove with a bright fill sitting in it, offset down and right of the
/// line it marks, which is where a groove's shadow falls under a light in the
/// top left corner.
///
/// `bar` draws the long radial index the frequency selectors carry; without it
/// the mark is the short stub the equaliser and power switches have.
pub fn switch_pointer(canvas: &mut Canvas, cx: f32, cy: f32, r: f32, angle: f32, bar: bool) {
    let (sa, ca) = angle.to_radians().sin_cos();
    let (from, to) = if bar { (0.26, 0.94) } else { (0.42, 0.94) };
    let at = |t: f32| (cx + r * t * sa, cy - r * t * ca);
    let (x0, y0) = at(from);
    let (x1, y1) = at(to);

    // The groove's own shadow, thrown down and right.
    let drop = r * 0.045;
    let mut shade = vg::Path::new();
    shade.move_to(x0 + drop, y0 + drop);
    shade.line_to(x1 + drop, y1 + drop);
    canvas.stroke_path(
        &shade,
        &vg::Paint::color(rgba(0x000000, 0.55))
            .with_line_width(r * 0.185)
            .with_line_cap(vg::LineCap::Round),
    );

    let mut index = vg::Path::new();
    index.move_to(x0, y0);
    index.line_to(x1, y1);
    canvas.stroke_path(
        &index,
        &vg::Paint::color(rgba(0x101112, 0.92))
            .with_line_width(r * 0.185)
            .with_line_cap(vg::LineCap::Round),
    );
    canvas.stroke_path(
        &index,
        &vg::Paint::color(rgb(0xf6f3ec))
            .with_line_width(r * 0.095)
            .with_line_cap(vg::LineCap::Round),
    );
}

#[cfg(test)]
mod selector_tests {
    use super::selector_angle;

    /// The OFF and ON engraving is at forty-five degrees either side of the
    /// shaft, so the pointer has to be too.
    #[test]
    fn a_two_position_switch_throws_to_its_engraving() {
        assert_eq!(selector_angle(0, 2), -45.0);
        assert_eq!(selector_angle(1, 2), 45.0);
    }

    /// The multi-position selectors are unchanged: they keep the 32 degree
    /// step until the positions are numerous enough to close it up.
    #[test]
    fn multi_position_selectors_are_unchanged() {
        assert_eq!(selector_angle(0, 3), -32.0);
        assert_eq!(selector_angle(2, 3), 32.0);
        assert_eq!(selector_angle(0, 4), -48.0);
        assert_eq!(selector_angle(0, 7), -60.0);
        assert_eq!(selector_angle(6, 7), 60.0);
    }

    /// Every selector is symmetric about vertical, whatever its count.
    #[test]
    fn selectors_are_centred() {
        for count in 2..=8 {
            let first = selector_angle(0, count);
            let last = selector_angle(count - 1, count);
            assert!((first + last).abs() < 1e-4, "{count} positions is lopsided");
        }
    }
}
