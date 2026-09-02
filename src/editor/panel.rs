//! The faceplate: enamel, mounting screws and the edges the light catches.

use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::vizia::vg;

use super::sprites::{self, Placement, Sprite};
use super::style::*;

/// Fractions of the panel the mounting hardware sits at.
/// The mounting screws sit close in to the panel's corners.
const SCREW_X: [f32; 2] = [0.0330, 0.9670];
const HARDWARE_Y: [f32; 2] = [0.135, 0.865];

pub struct Faceplate {
    /// One cache per screw; an image belongs to the canvas that uploaded it.
    screws: [Sprite; 4],
}

impl Faceplate {
    pub fn new(cx: &mut Context) -> Handle<'_, Self> {
        Self {
            screws: [Sprite::new(), Sprite::new(), Sprite::new(), Sprite::new()],
        }
        .build(cx, |_| {})
            .position_type(PositionType::SelfDirected)
            .left(Pixels(0.0))
            .top(Pixels(0.0))
            .width(Percentage(100.0))
            .height(Percentage(100.0))
    }
}

impl View for Faceplate {
    fn element(&self) -> Option<&'static str> {
        Some("pulteqfx-faceplate")
    }

    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        let b = cx.bounds();
        let scale = cx.scale_factor();

        // The petrol blue enamel.
        let mut panel = vg::Path::new();
        panel.rect(b.x, b.y, b.w, b.h);
        canvas.fill_path(
            &panel,
            &vg::Paint::linear_gradient(
                b.x,
                b.y,
                b.x,
                b.y + b.h,
                rgb(PANEL_TOP),
                rgb(PANEL_BOTTOM),
            ),
        );
        // A broad sheen, as if lit from in front and above.
        canvas.fill_path(
            &panel,
            &vg::Paint::radial_gradient(
                b.x + b.w * 0.45,
                b.y + b.h * 0.06,
                0.0,
                b.h * 1.8,
                rgba(0xffffff, 0.085),
                rgba(0xffffff, 0.0),
            ),
        );

        // Very fine horizontal grain in the paint.
        let lines = (b.h / (3.0 * scale)).max(1.0) as usize;
        for i in 0..lines {
            let y = b.y + (i as f32 + 0.5) * b.h / lines as f32;
            let shade = if i % 2 == 0 { 0x000000 } else { 0xffffff };
            let mut line = vg::Path::new();
            line.move_to(b.x, y);
            line.line_to(b.x + b.w, y);
            canvas.stroke_path(
                &line,
                &vg::Paint::color(rgba(shade, 0.012)).with_line_width(scale),
            );
        }

        // Falloff towards the ends and the bottom edge.
        for (sx, sy, ex, ey, alpha) in [
            (b.x, b.y, b.x + b.w * 0.08, b.y, 0.16),
            (b.x + b.w, b.y, b.x + b.w * 0.92, b.y, 0.16),
            (b.x, b.y + b.h, b.x, b.y + b.h * 0.84, 0.20),
        ] {
            canvas.fill_path(
                &panel,
                &vg::Paint::linear_gradient(sx, sy, ex, ey, rgba(0x000000, alpha), rgba(0x000000, 0.0)),
            );
        }

        // Rack hardware.
        for (i, &sx) in SCREW_X.iter().enumerate() {
            for (j, &sy) in HARDWARE_Y.iter().enumerate() {
                // No two screws are ever driven to the same angle, and each
                // photograph already carries its own.
                let k = i * 2 + j;
                self.screws[k].draw(
                    canvas,
                    sprites::SCREWS[k],
                    Placement::new(
                        b.x + b.w * sx,
                        b.y + b.h * sy,
                        15.0 * scale,
                        0.0,
                        sprites::CENTRE,
                    ),
                );
            }
        }

        // Bevelled top and bottom edges.
        let mut top = vg::Path::new();
        top.move_to(b.x, b.y + scale);
        top.line_to(b.x + b.w, b.y + scale);
        canvas.stroke_path(
            &top,
            &vg::Paint::color(rgba(0xffffff, 0.24)).with_line_width(scale * 2.0),
        );
        let mut bottom = vg::Path::new();
        bottom.move_to(b.x, b.y + b.h - scale);
        bottom.line_to(b.x + b.w, b.y + b.h - scale);
        canvas.stroke_path(
            &bottom,
            &vg::Paint::color(rgba(0x000000, 0.5)).with_line_width(scale * 2.5),
        );
    }
}
