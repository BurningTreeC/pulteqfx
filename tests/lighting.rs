//! The panel is lit from one place, and everything on it has to agree.
//!
//! The faceplate paints a highlight in the top left corner, `assetgen`'s rig
//! keys the renders from up and to the left, the contact shadows are thrown
//! down and right, and each control is tinted by how far it sits from the
//! lamp. Four separate pieces of code, one light. When they disagree the panel
//! reads as a collage, which is exactly what it looked like before: a sheen
//! across the middle of the enamel with every knob's highlight in the corner.
//!
//! These are the arithmetic parts of that agreement. What a render looks like
//! is not testable here; where the light is, is.

use pulteqfx::editor::style::{light_at, BOTTOM_ROW, PANEL_H, PANEL_W, SHADOW_X, SHADOW_Y, TOP_ROW};

/// Shadows fall away from the light, so both offsets are positive: right and
/// down. A shadow straight below a control is a lamp directly overhead, and
/// that is not where the renders put it.
#[test]
fn shadows_fall_away_from_the_light() {
    assert!(SHADOW_X > 0.0, "shadows must fall to the right of the light");
    assert!(SHADOW_Y > 0.0, "shadows must fall below the light");
}

/// The bright end of the panel is the top left and the dark end is the bottom
/// right, with nothing brighter out in the middle. This is the property the
/// first attempt failed: a radial gradient written with its stops the wrong
/// way round left the far corner *lighter* than the centre of the panel.
///
/// Not asserted along the diagonal from the corner, which is a trap worth
/// recording. The lamp hangs a little above the top edge and a little in from
/// the left, so the brightest point on the faceplate is the one directly under
/// it -- about fifty pixels in -- and the corner itself is fractionally
/// further away. Walking out from (0, 0) therefore brightens for the first few
/// steps before it falls, and that is the light being in front of the panel
/// rather than a fault.
#[test]
fn the_panel_gets_darker_away_from_the_lamp() {
    const STEPS: usize = 24;
    let at = |i: usize, j: usize| {
        light_at(
            PANEL_W * i as f32 / STEPS as f32,
            PANEL_H * j as f32 / STEPS as f32,
        )
    };

    let mut brightest = (0usize, 0usize, f32::NEG_INFINITY);
    let mut darkest = (0usize, 0usize, f32::INFINITY);
    for i in 0..=STEPS {
        for j in 0..=STEPS {
            let v = at(i, j);
            if v > brightest.2 {
                brightest = (i, j, v);
            }
            if v < darkest.2 {
                darkest = (i, j, v);
            }
        }
    }
    assert!(
        brightest.0 <= STEPS / 8 && brightest.1 == 0,
        "the brightest place on the panel is not under the lamp: ({}, {})",
        brightest.0,
        brightest.1
    );
    assert_eq!(
        (darkest.0, darkest.1),
        (STEPS, STEPS),
        "the darkest place on the panel is not the corner opposite the lamp"
    );

    // And no rise on the way across, once past the point under the lamp.
    let mut last = f32::INFINITY;
    for i in STEPS / 8..=STEPS {
        let here = at(i, i);
        assert!(
            here <= last + 1e-6,
            "the panel brightens again {:.0}% along its diagonal",
            100.0 * i as f32 / STEPS as f32
        );
        last = here;
    }
}

/// The falloff has to be visible without being theatrical. Too little and the
/// controls all look equally lit, which is the collage; too much and the right
/// hand end of a nineteen inch panel is in the dark.
#[test]
fn the_falloff_is_worth_seeing_and_no_more() {
    let spread = light_at(0.0, 0.0) - light_at(PANEL_W, PANEL_H);
    assert!(
        (0.10..=0.35).contains(&spread),
        "corner to corner the light changes by {spread:.3}, which is {} to see",
        if spread < 0.10 { "too little" } else { "too much" }
    );
    assert!(
        light_at(PANEL_W, PANEL_H) > 0.6,
        "the far corner is too dark to read the hardware in"
    );
}

/// Every control is on the panel, so every control is lit. A control whose
/// multiplier came back above one would be brighter than the render it was
/// made from, which is not a thing a tint can do.
#[test]
fn every_control_takes_light_between_none_and_all() {
    for x in [0.0, PANEL_W * 0.25, PANEL_W * 0.5, PANEL_W * 0.75, PANEL_W] {
        for y in [0.0, TOP_ROW, BOTTOM_ROW, PANEL_H] {
            let v = light_at(x, y);
            assert!(
                (0.0..=1.0).contains(&v),
                "the light at ({x}, {y}) is {v:.3}, which is not a multiplier"
            );
        }
    }
}
