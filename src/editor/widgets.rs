//! The panel's controls: knobs, rotary selectors and the equaliser switch.

use nih_plug::prelude::Param;
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::vizia::vg;
use nih_plug_vizia::widgets::param_base::ParamWidgetBase;
use nih_plug_vizia::widgets::{util::ModifiersExt, RawParamEvent};

use super::style::*;

/// Pixels of vertical drag for the full range of a knob.
const DRAG_RANGE: f32 = 260.0;
/// How much finer the drag becomes while shift is held.
const FINE: f32 = 0.15;

// ---------------------------------------------------------------------------
// Rotary knob
// ---------------------------------------------------------------------------

/// One of the panel's continuous knobs. Drag up and down to turn it, hold
/// shift for a finer grip, double click to put it back where it started.
pub struct Knob {
    param: ParamWidgetBase,
    radius: f32,
    dragging: bool,
    last_y: f32,
}

impl Knob {
    pub fn new<L, Params, P, FMap>(
        cx: &mut Context,
        params: L,
        params_to_param: FMap,
        radius: f32,
    ) -> Handle<'_, Self>
    where
        L: Lens<Target = Params> + Clone,
        Params: 'static,
        P: Param + 'static,
        FMap: Fn(&Params) -> &P + Copy + 'static,
    {
        Self {
            param: ParamWidgetBase::new(cx, params, params_to_param),
            radius,
            dragging: false,
            last_y: 0.0,
        }
        .build(
            cx,
            ParamWidgetBase::build_view(params, params_to_param, move |cx, data| {
                // Repaint whenever the host or another editor moves the value.
                let value = data.make_lens(|param| param.modulated_normalized_value());
                Binding::new(cx, value, |cx, _| cx.needs_redraw());
            }),
        )
        .width(Pixels(radius * 2.0))
        .height(Pixels(radius * 2.0))
    }

    fn nudge(&self, cx: &mut EventContext, delta: f32) {
        let current = self.param.unmodulated_normalized_value();
        self.param
            .set_normalized_value(cx, (current + delta).clamp(0.0, 1.0));
    }
}

impl View for Knob {
    fn element(&self) -> Option<&'static str> {
        Some("pulteqfx-knob")
    }

    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        let bounds = cx.bounds();
        let r = self.radius * cx.scale_factor();
        let (mx, my) = (bounds.x + bounds.w / 2.0, bounds.y + bounds.h / 2.0);

        draw_knob(
            canvas,
            mx,
            my,
            r,
            knob_angle(self.param.modulated_normalized_value()),
        );
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        // A change from the host or another editor arrives as this event.
        event.map(|param_event, _| {
            if let RawParamEvent::ParametersChanged = param_event {
                cx.needs_redraw();
            }
        });

        event.map(|window_event, meta| match window_event {
            WindowEvent::MouseDown(MouseButton::Left)
            | WindowEvent::MouseTripleClick(MouseButton::Left) => {
                if cx.modifiers().command() {
                    self.param.begin_set_parameter(cx);
                    self.param
                        .set_normalized_value(cx, self.param.default_normalized_value());
                    self.param.end_set_parameter(cx);
                } else {
                    self.dragging = true;
                    self.last_y = cx.mouse().cursory;
                    cx.capture();
                    cx.focus();
                    cx.set_active(true);
                    self.param.begin_set_parameter(cx);
                }
                meta.consume();
            }
            WindowEvent::MouseDoubleClick(MouseButton::Left)
            | WindowEvent::MouseDown(MouseButton::Right) => {
                self.param.begin_set_parameter(cx);
                self.param
                    .set_normalized_value(cx, self.param.default_normalized_value());
                self.param.end_set_parameter(cx);
                meta.consume();
            }
            WindowEvent::MouseUp(MouseButton::Left) => {
                if self.dragging {
                    self.dragging = false;
                    cx.release();
                    cx.set_active(false);
                    self.param.end_set_parameter(cx);
                    meta.consume();
                }
            }
            WindowEvent::MouseMove(_, y) => {
                if self.dragging {
                    let speed = if cx.modifiers().shift() { FINE } else { 1.0 };
                    let delta = (self.last_y - *y) / (DRAG_RANGE * cx.scale_factor()) * speed;
                    self.last_y = *y;
                    self.nudge(cx, delta);
                    cx.needs_redraw();
                }
            }
            WindowEvent::MouseScroll(_, y) => {
                let step = if cx.modifiers().shift() { 0.005 } else { 0.02 };
                self.param.begin_set_parameter(cx);
                self.nudge(cx, y * step);
                self.param.end_set_parameter(cx);
                cx.needs_redraw();
                meta.consume();
            }
            _ => {}
        });
    }
}

// ---------------------------------------------------------------------------
// Rotary selector
// ---------------------------------------------------------------------------

/// The frequency selector switches: a smaller knob that snaps between
/// detents, either by dragging or by clicking on the position you want.
pub struct Selector {
    param: ParamWidgetBase,
    radius: f32,
    positions: usize,
    /// Bar pointer for the frequency switches, plain knob for the power switch.
    pointer: bool,
    dragging: bool,
    last_y: f32,
    /// Fractional position carried between events so a slow drag still moves.
    travel: f32,
}

impl Selector {
    pub fn new<L, Params, P, FMap>(
        cx: &mut Context,
        params: L,
        params_to_param: FMap,
        radius: f32,
        positions: usize,
        pointer: bool,
    ) -> Handle<'_, Self>
    where
        L: Lens<Target = Params> + Clone,
        Params: 'static,
        P: Param + 'static,
        FMap: Fn(&Params) -> &P + Copy + 'static,
    {
        Self {
            param: ParamWidgetBase::new(cx, params, params_to_param),
            radius,
            positions,
            pointer,
            dragging: false,
            last_y: 0.0,
            travel: 0.0,
        }
        .build(
            cx,
            ParamWidgetBase::build_view(params, params_to_param, move |cx, data| {
                let value = data.make_lens(|param| param.modulated_normalized_value());
                Binding::new(cx, value, |cx, _| cx.needs_redraw());
            }),
        )
        .width(Pixels(radius * 2.0))
        .height(Pixels(radius * 2.0))
    }

    fn index(&self) -> usize {
        let n = self.positions.max(1);
        ((self.param.modulated_normalized_value() * (n - 1) as f32).round() as usize).min(n - 1)
    }

    fn select(&self, cx: &mut EventContext, index: isize) {
        let n = self.positions.max(1) as isize;
        let index = index.clamp(0, n - 1);
        let normalized = index as f32 / (n - 1).max(1) as f32;
        self.param.begin_set_parameter(cx);
        self.param.set_normalized_value(cx, normalized);
        self.param.end_set_parameter(cx);
    }
}

impl View for Selector {
    fn element(&self) -> Option<&'static str> {
        Some("pulteqfx-selector")
    }

    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        let bounds = cx.bounds();
        let r = self.radius * cx.scale_factor();
        let (mx, my) = (bounds.x + bounds.w / 2.0, bounds.y + bounds.h / 2.0);

        let angle = selector_angle(self.index(), self.positions);
        if self.pointer {
            draw_pointer_knob(canvas, mx, my, r, angle);
        } else {
            draw_knob(canvas, mx, my, r, angle);
        }
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        // A change from the host or another editor arrives as this event.
        event.map(|param_event, _| {
            if let RawParamEvent::ParametersChanged = param_event {
                cx.needs_redraw();
            }
        });

        event.map(|window_event, meta| match window_event {
            WindowEvent::MouseDown(MouseButton::Left)
            | WindowEvent::MouseTripleClick(MouseButton::Left) => {
                self.dragging = true;
                self.last_y = cx.mouse().cursory;
                self.travel = 0.0;
                cx.capture();
                cx.focus();
                cx.set_active(true);
                meta.consume();
            }
            WindowEvent::MouseUp(MouseButton::Left) => {
                if self.dragging {
                    self.dragging = false;
                    cx.release();
                    cx.set_active(false);
                    meta.consume();
                }
            }
            WindowEvent::MouseMove(_, y) => {
                if self.dragging {
                    // One detent every 20 pixels of drag.
                    self.travel += (self.last_y - *y) / (20.0 * cx.scale_factor());
                    self.last_y = *y;
                    let steps = self.travel.trunc();
                    if steps != 0.0 {
                        self.travel -= steps;
                        self.select(cx, self.index() as isize + steps as isize);
                        cx.needs_redraw();
                    }
                }
            }
            WindowEvent::MouseScroll(_, y) => {
                if *y != 0.0 {
                    self.select(cx, self.index() as isize + y.signum() as isize);
                    cx.needs_redraw();
                }
                meta.consume();
            }
            _ => {}
        });
    }
}

// ---------------------------------------------------------------------------
// Equaliser switch
// ---------------------------------------------------------------------------

/// The bat handle toggle that lifts the passive network out of circuit.
pub struct Toggle {
    param: ParamWidgetBase,
}

impl Toggle {
    pub fn new<L, Params, P, FMap>(
        cx: &mut Context,
        params: L,
        params_to_param: FMap,
    ) -> Handle<'_, Self>
    where
        L: Lens<Target = Params> + Clone,
        Params: 'static,
        P: Param + 'static,
        FMap: Fn(&Params) -> &P + Copy + 'static,
    {
        Self {
            param: ParamWidgetBase::new(cx, params, params_to_param),
        }
        .build(
            cx,
            ParamWidgetBase::build_view(params, params_to_param, move |cx, data| {
                let value = data.make_lens(|param| param.modulated_normalized_value());
                Binding::new(cx, value, |cx, _| cx.needs_redraw());
            }),
        )
        .width(Pixels(34.0))
        .height(Pixels(58.0))
    }
}

impl View for Toggle {
    fn element(&self) -> Option<&'static str> {
        Some("pulteqfx-toggle")
    }

    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        let bounds = cx.bounds();
        let scale = cx.scale_factor();
        let (mx, my) = (bounds.x + bounds.w / 2.0, bounds.y + bounds.h / 2.0);
        let up = self.param.modulated_normalized_value() > 0.5;
        let nut = 11.0 * scale;

        // Shadow the switch throws on the panel.
        let mut shadow = vg::Path::new();
        shadow.ellipse(mx, my + nut * 0.35, nut * 1.5, nut * 1.25);
        canvas.fill_path(
            &shadow,
            &vg::Paint::radial_gradient(
                mx,
                my + nut * 0.35,
                nut * 0.8,
                nut * 1.5,
                rgba(0x000000, 0.5),
                rgba(0x000000, 0.0),
            ),
        );

        // The chrome dress nut, turned and bright along the top.
        let mut ring = vg::Path::new();
        ring.circle(mx, my, nut);
        canvas.fill_path(
            &ring,
            &vg::Paint::radial_gradient(
                mx - nut * 0.35,
                my - nut * 0.4,
                nut * 0.1,
                nut * 1.5,
                rgb(0xe6e9ec),
                rgb(0x3e4247),
            ),
        );
        canvas.stroke_path(
            &ring,
            &vg::Paint::color(rgba(0x000000, 0.45)).with_line_width(nut * 0.12),
        );
        // The washer inside it, sunk a little lower.
        let mut washer = vg::Path::new();
        washer.circle(mx, my, nut * 0.62);
        canvas.fill_path(
            &washer,
            &vg::Paint::radial_gradient(
                mx - nut * 0.2,
                my - nut * 0.25,
                0.0,
                nut * 0.9,
                rgb(0x8e9399),
                rgb(0x1c1f23),
            ),
        );

        // The bat, thrown up for in and down for out.
        let length = 17.0 * scale;
        let lean = if up { -1.0 } else { 1.0 };
        let tip_y = my + lean * length;
        let width = 7.0 * scale;

        let mut bat = vg::Path::new();
        bat.move_to(mx, my + lean * nut * 0.2);
        bat.line_to(mx, tip_y);
        canvas.stroke_path(
            &bat,
            &vg::Paint::color(rgba(0x000000, 0.9))
                .with_line_width(width * 2.3)
                .with_line_cap(vg::LineCap::Round),
        );
        canvas.stroke_path(
            &bat,
            &vg::Paint::linear_gradient(
                mx - width,
                my,
                mx + width,
                my,
                rgb(0x4c4d53),
                rgb(0x0c0c0e),
            )
            .with_line_width(width * 2.0)
            .with_line_cap(vg::LineCap::Round),
        );
        // Gloss down the lit side of the bat, soft rather than a hard stripe.
        let mut gloss = vg::Path::new();
        gloss.move_to(mx - width * 0.42, my + lean * nut * 0.1);
        gloss.line_to(mx - width * 0.42, tip_y - lean * width * 0.7);
        canvas.stroke_path(
            &gloss,
            &vg::Paint::color(rgba(0xffffff, 0.13))
                .with_line_width(width * 0.85)
                .with_line_cap(vg::LineCap::Round),
        );
        canvas.stroke_path(
            &gloss,
            &vg::Paint::color(rgba(0xffffff, 0.16))
                .with_line_width(width * 0.35)
                .with_line_cap(vg::LineCap::Round),
        );
        // Highlight on the tip.
        let mut glint = vg::Path::new();
        glint.ellipse(mx - width * 0.30, tip_y - lean * width * 0.30, width * 0.42, width * 0.30);
        canvas.fill_path(&glint, &vg::Paint::color(rgba(0xffffff, 0.45)));
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        // A change from the host or another editor arrives as this event.
        event.map(|param_event, _| {
            if let RawParamEvent::ParametersChanged = param_event {
                cx.needs_redraw();
            }
        });

        event.map(|window_event, meta| {
            if let WindowEvent::MouseDown(MouseButton::Left) = window_event {
                let value = self.param.modulated_normalized_value();
                self.param.begin_set_parameter(cx);
                self.param
                    .set_normalized_value(cx, if value > 0.5 { 0.0 } else { 1.0 });
                self.param.end_set_parameter(cx);
                cx.needs_redraw();
                meta.consume();
            }
        });
    }
}

// ---------------------------------------------------------------------------
// Pilot lamp
// ---------------------------------------------------------------------------

/// The red jewel next to the switches, lit while the equaliser is in circuit.
pub struct Lamp {
    param: ParamWidgetBase,
}

impl Lamp {
    pub fn new<L, Params, P, FMap>(
        cx: &mut Context,
        params: L,
        params_to_param: FMap,
    ) -> Handle<'_, Self>
    where
        L: Lens<Target = Params> + Clone,
        Params: 'static,
        P: Param + 'static,
        FMap: Fn(&Params) -> &P + Copy + 'static,
    {
        Self {
            param: ParamWidgetBase::new(cx, params, params_to_param),
        }
        .build(
            cx,
            ParamWidgetBase::build_view(params, params_to_param, move |cx, data| {
                let value = data.make_lens(|param| param.modulated_normalized_value());
                Binding::new(cx, value, |cx, _| cx.needs_redraw());
            }),
        )
        .width(Pixels(28.0))
        .height(Pixels(28.0))
    }
}

impl View for Lamp {
    fn element(&self) -> Option<&'static str> {
        Some("pulteqfx-lamp")
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        event.map(|param_event, _| {
            if let RawParamEvent::ParametersChanged = param_event {
                cx.needs_redraw();
            }
        });
    }

    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        let bounds = cx.bounds();
        let (mx, my) = (bounds.x + bounds.w / 2.0, bounds.y + bounds.h / 2.0);
        let r = bounds.w.min(bounds.h) / 2.0;
        let lit = self.param.modulated_normalized_value() > 0.5;

        if lit {
            // Spill onto the surrounding panel.
            let mut glow = vg::Path::new();
            glow.circle(mx, my, r * 2.3);
            canvas.fill_path(
                &glow,
                &vg::Paint::radial_gradient(
                    mx,
                    my,
                    r * 0.8,
                    r * 2.3,
                    rgba(0xff3a18, 0.30),
                    rgba(0xff3a18, 0.0),
                ),
            );
        }

        // The brushed steel bezel.
        let mut shadow = vg::Path::new();
        shadow.circle(mx, my + r * 0.12, r * 1.04);
        canvas.fill_path(&shadow, &vg::Paint::color(rgba(0x000000, 0.5)));
        let mut bezel = vg::Path::new();
        bezel.circle(mx, my, r);
        canvas.fill_path(
            &bezel,
            &vg::Paint::linear_gradient(
                mx - r * 0.6,
                my - r,
                mx + r * 0.6,
                my + r,
                rgb(0xd2d5d8),
                rgb(0x44474b),
            ),
        );
        // Brushing across the bezel.
        for i in 0..7 {
            let t = (i as f32 + 0.5) / 7.0;
            let y = my - r + t * r * 2.0;
            let mut line = vg::Path::new();
            line.move_to(mx - r, y);
            line.line_to(mx + r, y);
            canvas.stroke_path(
                &line,
                &vg::Paint::color(rgba(if i % 2 == 0 { 0xffffff } else { 0x000000 }, 0.07))
                    .with_line_width(r * 0.10),
            );
        }
        canvas.stroke_path(
            &bezel,
            &vg::Paint::color(rgba(0x000000, 0.45)).with_line_width(r * 0.10),
        );

        // The seat the jewel is pressed into.
        let jewel = r * 0.72;
        let mut seat = vg::Path::new();
        seat.circle(mx, my, jewel * 1.10);
        canvas.fill_path(&seat, &vg::Paint::color(rgba(0x100806, 0.9)));

        // The cut glass jewel: radial facets around a bright core.
        let (core, edge, facet) = if lit {
            (rgb(0xff462a), rgb(0x8e0404), 0.30)
        } else {
            (rgb(0x741814), rgb(0x280606), 0.18)
        };
        let mut glass = vg::Path::new();
        glass.circle(mx, my, jewel);
        canvas.fill_path(
            &glass,
            &vg::Paint::radial_gradient(
                mx - jewel * 0.18,
                my - jewel * 0.22,
                0.0,
                jewel * 1.15,
                core,
                edge,
            ),
        );
        // Cut facets, seen as fine radial gradations rather than hard wedges.
        const FACETS: usize = 16;
        for i in 0..FACETS {
            let a = std::f32::consts::TAU * i as f32 / FACETS as f32;
            let (sa, ca) = a.sin_cos();
            let mut cut = vg::Path::new();
            cut.move_to(mx + jewel * 0.22 * sa, my - jewel * 0.22 * ca);
            cut.line_to(mx + jewel * 0.98 * sa, my - jewel * 0.98 * ca);
            let shade = if i % 2 == 0 { 0xffd8c0 } else { 0x380000 };
            canvas.stroke_path(
                &cut,
                &vg::Paint::color(rgba(shade, facet * 0.30)).with_line_width(jewel * 0.07),
            );
        }
        // The polished dome over the top.
        let mut spot = vg::Path::new();
        spot.ellipse(mx - jewel * 0.26, my - jewel * 0.34, jewel * 0.40, jewel * 0.26);
        canvas.fill_path(
            &spot,
            &vg::Paint::radial_gradient(
                mx - jewel * 0.26,
                my - jewel * 0.34,
                0.0,
                jewel * 0.46,
                rgba(0xffffff, if lit { 0.45 } else { 0.25 }),
                rgba(0xffffff, 0.0),
            ),
        );
    }
}
