//! The panel's controls: knobs, rotary selectors and the equaliser switch.

use nih_plug::prelude::Param;
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::vizia::vg;
use nih_plug_vizia::widgets::param_base::ParamWidgetBase;
use nih_plug_vizia::widgets::{util::ModifiersExt, RawParamEvent};

use super::sprites::{self, Placement, Sprite};
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
    face: Sprite,
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
            face: Sprite::new(),
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

    /// Ends a drag: releases the mouse and closes the gesture with the host.
    ///
    /// Called from more than one place because the one that must not be relied
    /// on is the mouse button coming back up. See `event`.
    fn finish(&mut self, cx: &mut EventContext) {
        if !self.dragging {
            return;
        }
        self.dragging = false;
        cx.release();
        cx.set_active(false);
        self.param.end_set_parameter(cx);
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

        // Pick the frame rendered at this angle rather than turning one image,
        // which would carry the lighting round with the knob.
        let position = self.param.modulated_normalized_value().clamp(0.0, 1.0);
        let frame = (position * (sprites::KNOB_FRAMES - 1) as f32).round() as usize;
        // The render is framed to the body's silhouette and stops dead at its
        // edge, so the knob has to be given the same contact shadow the drawn
        // controls lay down or it sits on the enamel with nothing under it.
        // Dividing by the span recovers the body from the frame, and the draw
        // factor sets it to the size the faceplate was engraved around.
        let body = r * sprites::KNOB_LARGE_DRAW / 2.0;
        contact_shadow(canvas, mx, my, body);
        self.face.draw_frame(
            canvas,
            sprites::KNOB_LARGE,
            Placement::new(
                mx,
                my,
                r * sprites::KNOB_LARGE_DRAW / sprites::KNOB_LARGE_SPAN,
                0.0,
                sprites::CENTRE,
            )
            .lit(light_at_screen(mx, my, cx.scale_factor())),
            frame,
            sprites::KNOB_FRAMES,
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
                    // A press while this knob still believes a drag is running
                    // means the button came up somewhere nothing here ever
                    // heard about, and the widget has been sitting on the
                    // capture ever since.
                    //
                    // This is the recovery that works when the others cannot.
                    // The check in `MouseMove` reads vizia's cached button
                    // state, and that state is exactly what goes stale: it is
                    // only ever written from a real button event, so if the
                    // up never arrived it still says `Pressed` and the heal
                    // never fires. Baseview takes the pointer with
                    // `SetCapture` on the way down and gives it back on the
                    // way up, and handles no `WM_CAPTURECHANGED` in between --
                    // so when the host puts up a dialog, or another window
                    // takes the pointer mid-drag, there is no up, no capture
                    // notification, and nothing to notice it with.
                    //
                    // A fresh press is proof on its own: the button cannot go
                    // down without having been up. So the stale drag is closed
                    // here -- releasing the capture and closing the gesture the
                    // host still thinks is open -- and the new one starts
                    // cleanly. That heals the panel on the first click the
                    // player makes when it looks frozen, which is the first
                    // thing anybody tries.
                    self.finish(cx);
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
                    self.finish(cx);
                    meta.consume();
                }
            }
            // Anything that means this window is no longer the one being used.
            // These are the events that do arrive when a drag is interrupted;
            // the check in `MouseMove` covers the times none of them does.
            WindowEvent::FocusOut
            | WindowEvent::WindowClose
            | WindowEvent::MouseCaptureOutEvent => {
                self.finish(cx);
            }
            WindowEvent::MouseMove(_, y) => {
                if self.dragging {
                    // The button came up somewhere this window never heard
                    // about. Without this the control holds the mouse for good.
                    if cx.mouse().left.state == MouseButtonState::Released {
                        self.finish(cx);
                        return;
                    }
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
    /// The long radial index the frequency switches carry; the equaliser and
    /// power switches get the short stub instead.
    pointer: bool,
    /// Whether the first position sits on the right of the arc rather than the
    /// left. The equaliser switch reads IN on the left, which is the opposite
    /// of the order its parameter counts in.
    reversed: bool,
    dragging: bool,
    last_y: f32,
    /// Fractional position carried between events so a slow drag still moves.
    travel: f32,
    face: Sprite,
}

impl Selector {
    pub fn new<L, Params, P, FMap>(
        cx: &mut Context,
        params: L,
        params_to_param: FMap,
        radius: f32,
        positions: usize,
        pointer: bool,
        reversed: bool,
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
            reversed,
            dragging: false,
            last_y: 0.0,
            travel: 0.0,
            face: Sprite::new(),
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

    /// Ends a drag and gives the mouse back. See the knob's `event` for why
    /// this cannot be left to the button coming up.
    fn release_drag(&mut self, cx: &mut EventContext) {
        if !self.dragging {
            return;
        }
        self.dragging = false;
        cx.release();
        cx.set_active(false);
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

        let shown = if self.reversed {
            self.positions.saturating_sub(1) - self.index()
        } else {
            self.index()
        };
        let angle = selector_angle(shown, self.positions);
        // The body is a render and is never turned; only the index moves. See
        // `sprites::KNOB_METAL`.
        let body = r * sprites::KNOB_METAL_DRAW / 2.0;
        contact_shadow(canvas, mx, my, body);
        self.face.draw(
            canvas,
            sprites::KNOB_METAL,
            Placement::new(
                mx,
                my,
                r * sprites::KNOB_METAL_DRAW / sprites::KNOB_METAL_SPAN,
                0.0,
                sprites::CENTRE,
            )
            .lit(light_at_screen(mx, my, cx.scale_factor())),
        );
        switch_pointer(canvas, mx, my, body, angle, self.pointer);
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
                    self.release_drag(cx);
                    meta.consume();
                }
            }
            // As on the knob: nothing in vizia ever clears a capture on its
            // own, so a drag whose button-up goes missing would hold the mouse
            // for the rest of the session and the window would look frozen.
            WindowEvent::FocusOut
            | WindowEvent::WindowClose
            | WindowEvent::MouseCaptureOutEvent => {
                self.release_drag(cx);
            }
            WindowEvent::MouseMove(_, y) => {
                if self.dragging {
                    if cx.mouse().left.state == MouseButtonState::Released {
                        self.release_drag(cx);
                        return;
                    }
                    // One detent every 20 pixels of drag.
                    self.travel += (self.last_y - *y) / (20.0 * cx.scale_factor());
                    self.last_y = *y;
                    let steps = self.travel.trunc();
                    if steps != 0.0 {
                        self.travel -= steps;
                        // Dragging up has to move the pointer the way the
                        // panel is engraved, not the way the parameter counts.
                        let steps = if self.reversed { -steps } else { steps };
                        self.select(cx, self.index() as isize + steps as isize);
                        cx.needs_redraw();
                    }
                }
            }
            WindowEvent::MouseScroll(_, y) => {
                if *y != 0.0 {
                    let step = if self.reversed { -y.signum() } else { y.signum() };
                    self.select(cx, self.index() as isize + step as isize);
                    cx.needs_redraw();
                }
                meta.consume();
            }
            _ => {}
        });
    }
}

// ---------------------------------------------------------------------------
// Pilot lamp
// ---------------------------------------------------------------------------

/// The red jewel next to the switches, lit while the equaliser is in circuit.
pub struct Lamp {
    param: ParamWidgetBase,
    /// One cache per state: an image belongs to the canvas that uploaded it,
    /// and these are two different renders rather than one image tinted.
    lit: Sprite,
    dark: Sprite,
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
            lit: Sprite::new(),
            dark: Sprite::new(),
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

    /// Mouse handling, and the one thing in it that is not obvious.
    ///
    /// A drag captures the mouse so that the control keeps receiving movement
    /// when the pointer leaves it, and releases on the button coming back up.
    /// That release must not be the *only* way out.
    ///
    /// vizia routes every mouse event to the captured entity, and nothing in
    /// vizia ever clears a capture on its own -- `MouseCaptureOutEvent` is
    /// declared in its event enum and emitted nowhere, and `release` only
    /// clears the field when the widget itself asks. So a drag whose button-up
    /// never arrives leaves this control holding the mouse for the rest of the
    /// session: every other control stops responding, the window looks frozen,
    /// and the audio thread carries on as though nothing were wrong. The
    /// gesture opened with the host is never closed either, so it also thinks
    /// an edit is still in progress.
    ///
    /// A button-up can genuinely go missing. On Windows the pointer is held
    /// with `SetCapture`, and a `WM_CAPTURECHANGED` -- another window taking
    /// capture, the host putting up a dialog, the plugin window being
    /// deactivated mid-drag -- sends the button-up somewhere else entirely.
    ///
    /// So the drag is also ended by anything that says the mouse is no longer
    /// down, and the check that does not depend on an event arriving at all is
    /// in `MouseMove`: if the button is up while this control thinks it is
    /// dragging, the drag is over whether or not anyone said so. That one
    /// heals the window the moment the pointer moves over it again.
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
        let body = r * sprites::LAMP_DRAW / 2.0;

        if lit {
            // Spill onto the surrounding panel. The render carries the lamp
            // itself; what it cannot carry is the light landing on the enamel
            // around it, because that enamel is not in the model.
            let mut glow = vg::Path::new();
            glow.circle(mx, my, body * 2.4);
            canvas.fill_path(
                &glow,
                &vg::Paint::radial_gradient(
                    mx,
                    my,
                    body * 0.85,
                    body * 2.4,
                    rgba(0xff3a18, 0.30),
                    rgba(0xff3a18, 0.0),
                ),
            );
        }

        // The same contact shadow every other control on the panel lays down,
        // so the lamp sits in the same light as the knobs beside it.
        contact_shadow(canvas, mx, my, body);
        let sprite = if lit { &self.lit } else { &self.dark };
        let bytes = if lit {
            sprites::LAMP_LIT
        } else {
            sprites::LAMP_DARK
        };
        sprite.draw(
            canvas,
            bytes,
            Placement::new(
                mx,
                my,
                r * sprites::LAMP_DRAW / sprites::LAMP_SPAN,
                0.0,
                sprites::CENTRE,
            )
            // A lamp that is burning is a source and does not dim with its
            // distance from another one; a lamp that is out is just a red
            // jewel bolted to the panel, and does.
            .lit(if lit {
                1.0
            } else {
                light_at_screen(mx, my, cx.scale_factor())
            }),
        );
    }
}
