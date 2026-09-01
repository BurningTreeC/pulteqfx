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
pub const PANEL_TOP: u32 = 0x36_56_60;
pub const PANEL_BOTTOM: u32 = 0x1e_35_3c;


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
pub fn selector_angle(index: usize, count: usize) -> f32 {
    if count < 2 {
        return 0.0;
    }
    let step = 32.0_f32.min(120.0 / (count - 1) as f32);
    -step * (count - 1) as f32 / 2.0 + step * index as f32
}

/// The shadow a control casts onto the panel.
fn contact_shadow(canvas: &mut Canvas, cx: f32, cy: f32, r: f32) {
    let mut path = vg::Path::new();
    path.ellipse(cx, cy + r * 0.16, r * 1.20, r * 1.14);
    canvas.fill_path(
        &path,
        &vg::Paint::radial_gradient(
            cx,
            cy + r * 0.16,
            r * 0.72,
            r * 1.20,
            rgba(0x000000, 0.60),
            rgba(0x000000, 0.0),
        ),
    );
}

/// One of the panel's black bakelite knobs.
///
/// The hardware's knobs have a broad lobed skirt around a stepped top: a
/// bright rim, a recessed ring, then a raised boss in the middle. Almost all
/// of the light lands on the lobes and on that rim, which is what makes them
/// read as moulded bakelite rather than as a flat disc.
pub fn draw_knob(canvas: &mut Canvas, cx: f32, cy: f32, r: f32, angle: f32) {
    let rot = angle.to_radians();
    contact_shadow(canvas, cx, cy, r);

    // Lobed skirt, turning with the knob.
    const LOBES: usize = 11;
    const STEPS: usize = LOBES * 12;
    let mut skirt = vg::Path::new();
    for i in 0..=STEPS {
        let t = i as f32 / STEPS as f32 * std::f32::consts::TAU;
        let lobe = 1.0 - 0.042 * (1.0 - (t * LOBES as f32).cos());
        let a = t + rot;
        let (x, y) = (cx + r * lobe * a.sin(), cy - r * lobe * a.cos());
        if i == 0 {
            skirt.move_to(x, y);
        } else {
            skirt.line_to(x, y);
        }
    }
    skirt.close();
    canvas.fill_path(
        &skirt,
        &vg::Paint::linear_gradient(cx, cy - r, cx, cy + r, rgb(0x2b_2c_30), rgb(0x05_05_07)),
    );
    // The skirt falls away towards its edge.
    canvas.fill_path(
        &skirt,
        &vg::Paint::radial_gradient(cx, cy, r * 0.55, r, rgba(0x000000, 0.0), rgba(0x000000, 0.7)),
    );
    // Rim light along the lit edge of the lobes.
    canvas.stroke_path(
        &skirt,
        &vg::Paint::linear_gradient(
            cx - r * 0.5,
            cy - r,
            cx + r * 0.4,
            cy + r * 0.7,
            rgba(0xd8_e2_e8, 0.55),
            rgba(0xd8_e2_e8, 0.0),
        )
        .with_line_width(r * 0.05),
    );

    // Index line, painted across the skirt as on the hardware.
    let (sa, ca) = rot.sin_cos();
    let mut index = vg::Path::new();
    index.move_to(cx + r * 0.76 * sa, cy - r * 0.76 * ca);
    index.line_to(cx + r * 0.99 * sa, cy - r * 0.99 * ca);
    canvas.stroke_path(
        &index,
        &vg::Paint::color(rgba(0x000000, 0.8)).with_line_width(r * 0.16),
    );
    canvas.stroke_path(
        &index,
        &vg::Paint::color(rgb(0xf6_f3_ec)).with_line_width(r * 0.085),
    );

    // Step down from the skirt to the top of the knob.
    let face = r * 0.74;
    let mut step = vg::Path::new();
    step.circle(cx, cy, face);
    canvas.fill_path(&step, &vg::Paint::color(rgba(0x000000, 0.75)));
    // The bright turned rim that catches the light all the way round.
    canvas.stroke_path(
        &step,
        &vg::Paint::linear_gradient(
            cx,
            cy - face,
            cx,
            cy + face,
            rgba(0xdc_e6_ec, 0.95),
            rgba(0x6a_74_7c, 0.45),
        )
        .with_line_width(r * 0.055),
    );

    // Recessed ring inside the rim.
    let recess = r * 0.66;
    let mut inner = vg::Path::new();
    inner.circle(cx, cy, recess);
    canvas.fill_path(
        &inner,
        &vg::Paint::radial_gradient(
            cx - recess * 0.3,
            cy - recess * 0.35,
            recess * 0.1,
            recess * 1.4,
            rgb(0x33_34_39),
            rgb(0x08_08_0a),
        ),
    );
    // Light bouncing off the far wall of the recess.
    let mut bounce = vg::Path::new();
    bounce.arc(
        cx,
        cy,
        recess * 0.92,
        std::f32::consts::PI * 0.08,
        std::f32::consts::PI * 0.78,
        vg::Solidity::Solid,
    );
    canvas.stroke_path(
        &bounce,
        &vg::Paint::color(rgba(0xa8_b6_c0, 0.22)).with_line_width(recess * 0.10),
    );

    // The raised boss in the middle.
    let boss = r * 0.44;
    let mut cap = vg::Path::new();
    cap.circle(cx, cy, boss);
    canvas.fill_path(&cap, &vg::Paint::color(rgba(0x000000, 0.6)));
    canvas.stroke_path(
        &cap,
        &vg::Paint::linear_gradient(
            cx,
            cy - boss,
            cx,
            cy + boss,
            rgba(0x9c_a8_b2, 0.45),
            rgba(0x30_34_38, 0.0),
        )
        .with_line_width(r * 0.035),
    );
    canvas.fill_path(
        &cap,
        &vg::Paint::radial_gradient(
            cx - boss * 0.35,
            cy - boss * 0.4,
            0.0,
            boss * 1.5,
            rgb(0x2a_2b_2f),
            rgb(0x07_07_09),
        ),
    );
    // A soft sheen on the boss.
    let mut sheen = vg::Path::new();
    sheen.ellipse(cx - boss * 0.28, cy - boss * 0.36, boss * 0.44, boss * 0.26);
    canvas.fill_path(
        &sheen,
        &vg::Paint::radial_gradient(
            cx - boss * 0.28,
            cy - boss * 0.36,
            0.0,
            boss * 0.48,
            rgba(0xff_ff_ff, 0.16),
            rgba(0xff_ff_ff, 0.0),
        ),
    );
}

/// The bar shaped pointer knobs the frequency switches use: a dark moulded
/// lever with a polished metal cap and a white index stripe, sitting on a
/// turned collar.
pub fn draw_pointer_knob(canvas: &mut Canvas, cx: f32, cy: f32, r: f32, angle: f32) {
    contact_shadow(canvas, cx, cy, r * 1.05);

    // The collar the lever is fixed to, drawn first so the lever sits on it.
    let collar = r * 0.66;
    let mut base = vg::Path::new();
    base.circle(cx, cy, collar);
    canvas.fill_path(
        &base,
        &vg::Paint::radial_gradient(
            cx - collar * 0.35,
            cy - collar * 0.4,
            collar * 0.1,
            collar * 1.5,
            rgb(0x6a_6e_74),
            rgb(0x0c_0d_0f),
        ),
    );
    canvas.stroke_path(
        &base,
        &vg::Paint::linear_gradient(
            cx,
            cy - collar,
            cx,
            cy + collar,
            rgba(0xd0_d8_de, 0.5),
            rgba(0x20_24_28, 0.0),
        )
        .with_line_width(r * 0.06),
    );

    canvas.save();
    canvas.translate(cx, cy);
    canvas.rotate(angle.to_radians());

    let len = r * 1.78;
    let half = r * 0.36;
    let back = r * 0.46;

    // The moulded body.
    let mut body = vg::Path::new();
    body.rounded_rect(-half, -len, half * 2.0, len + back, half * 0.42);
    canvas.fill_path(&body, &vg::Paint::color(rgba(0x000000, 0.85)));
    let mut inset = vg::Path::new();
    inset.rounded_rect(
        -half * 0.94,
        -len * 0.985,
        half * 1.88,
        len * 0.985 + back,
        half * 0.40,
    );
    canvas.fill_path(
        &inset,
        &vg::Paint::linear_gradient(-half, 0.0, half, 0.0, rgb(0x3a_3b_40), rgb(0x0b_0b_0d)),
    );

    // The polished cap over the outer third, with the index stripe on it.
    let cap_len = len * 0.40;
    let mut cap = vg::Path::new();
    cap.rounded_rect(-half * 0.94, -len * 0.985, half * 1.88, cap_len, half * 0.40);
    canvas.fill_path(
        &cap,
        &vg::Paint::linear_gradient(
            -half,
            0.0,
            half,
            0.0,
            rgb(0xd4_d8_dc),
            rgb(0x5c_60_66),
        ),
    );
    let mut stripe = vg::Path::new();
    stripe.move_to(0.0, -len * 0.94);
    stripe.line_to(0.0, -len * 0.985 + cap_len * 0.94);
    canvas.stroke_path(
        &stripe,
        &vg::Paint::color(rgba(0x1c_1e_22, 0.85)).with_line_width(half * 0.34),
    );
    canvas.stroke_path(
        &stripe,
        &vg::Paint::color(rgb(0xf6_f7_f8)).with_line_width(half * 0.18),
    );

    // Highlight down the lit edge of the body.
    let mut lit = vg::Path::new();
    lit.move_to(-half * 0.72, back * 0.5);
    lit.line_to(-half * 0.72, -len * 0.55);
    canvas.stroke_path(
        &lit,
        &vg::Paint::color(rgba(0xff_ff_ff, 0.16)).with_line_width(half * 0.30),
    );

    canvas.restore();
}

/// A slotted machine screw holding the panel to the rack.
pub fn draw_screw(canvas: &mut Canvas, cx: f32, cy: f32, r: f32, angle: f32) {
    // The countersink it sits in.
    let mut well = vg::Path::new();
    well.circle(cx, cy + r * 0.10, r * 1.22);
    canvas.fill_path(
        &well,
        &vg::Paint::radial_gradient(
            cx,
            cy + r * 0.10,
            r * 0.85,
            r * 1.25,
            rgba(0x000000, 0.5),
            rgba(0x000000, 0.0),
        ),
    );

    // Domed head.
    let mut head = vg::Path::new();
    head.circle(cx, cy, r);
    canvas.fill_path(
        &head,
        &vg::Paint::radial_gradient(
            cx - r * 0.45,
            cy - r * 0.50,
            r * 0.05,
            r * 1.35,
            rgb(0xf4_f6_f8),
            rgb(0x3c_40_46),
        ),
    );
    // Turned edge, bright where it faces the light.
    canvas.stroke_path(
        &head,
        &vg::Paint::linear_gradient(
            cx,
            cy - r,
            cx,
            cy + r,
            rgba(0xff_ff_ff, 0.45),
            rgba(0x10_12_16, 0.85),
        )
        .with_line_width(r * 0.20),
    );
    canvas.stroke_path(
        &head,
        &vg::Paint::color(rgba(0x000000, 0.55)).with_line_width(r * 0.07),
    );

    // The slot, cut at whatever angle the screw happened to end up.
    let (sa, ca) = angle.to_radians().sin_cos();
    let mut slot = vg::Path::new();
    slot.move_to(cx - r * 0.78 * sa, cy + r * 0.78 * ca);
    slot.line_to(cx + r * 0.78 * sa, cy - r * 0.78 * ca);
    canvas.stroke_path(
        &slot,
        &vg::Paint::color(rgba(0x14_16_1a, 0.95)).with_line_width(r * 0.30),
    );
    // The lit lower lip of the slot.
    let mut lip = vg::Path::new();
    lip.move_to(cx - r * 0.74 * sa + ca * r * 0.13, cy + r * 0.74 * ca + sa * r * 0.13);
    lip.line_to(cx + r * 0.74 * sa + ca * r * 0.13, cy - r * 0.74 * ca + sa * r * 0.13);
    canvas.stroke_path(
        &lip,
        &vg::Paint::color(rgba(0xff_ff_ff, 0.30)).with_line_width(r * 0.07),
    );
}

/// The oval rack mounting slot punched through the end of the panel.
pub fn draw_rack_slot(canvas: &mut Canvas, cx: f32, cy: f32, w: f32, h: f32) {
    let mut hole = vg::Path::new();
    hole.rounded_rect(cx - w / 2.0, cy - h / 2.0, w, h, h / 2.0);
    canvas.fill_path(
        &hole,
        &vg::Paint::linear_gradient(
            cx,
            cy - h / 2.0,
            cx,
            cy + h / 2.0,
            rgb(0xd8_dc_de),
            rgb(0xa4_ab_af),
        ),
    );
    canvas.stroke_path(
        &hole,
        &vg::Paint::color(rgba(0x000000, 0.5)).with_line_width(1.4),
    );
}
