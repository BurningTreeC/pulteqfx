//! The strip above the panel and the settings it opens.
//!
//! None of this is on the hardware, so it is deliberately kept out of the way:
//! a thin dark header with a single button, and a panel behind it holding the
//! window scale, the oversampling quality and the amplifier's drive and output
//! trim.

// Views are constructed with `new` returning a `Handle`, which is how vizia
// widgets are written throughout, including NIH-plug's own.
#![allow(clippy::new_ret_no_self)]

use nih_plug::prelude::{Param, ParamPtr, Params};
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::vizia::vg;
use nih_plug_vizia::widgets::{GuiContextEvent, RawParamEvent};
use nih_plug_vizia::assets;

use super::style::*;
use super::widgets::Knob;
use super::{label_box, Panel, Place};
use crate::presets::{self, Preset};
use std::collections::BTreeMap;
use std::sync::Arc;

/// The window scales offered in the settings panel.
pub const SCALES: [f64; 9] = [0.5, 0.75, 0.85, 1.0, 1.2, 1.4, 1.5, 1.75, 2.0];
/// Oversampling options, in the order the parameter declares them.
const OVERSAMPLING: [&str; 4] = ["Off", "2x", "4x", "8x"];

const PANEL_X: f32 = PANEL_W - 344.0;
const PANEL_Y: f32 = HEADER_H + 6.0;
const PANEL_WIDTH: f32 = 332.0;
const PANEL_HEIGHT: f32 = 196.0;
const ROW_H: f32 = 24.0;

/// Which drop down list, if any, is showing.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Menu {
    None,
    Scale,
    Preset,
}

impl Data for Menu {
    fn same(&self, other: &Self) -> bool {
        self == other
    }
}

/// Which modal dialog, if any, is showing.
#[derive(Clone, PartialEq, Eq)]
pub enum Dialog {
    None,
    /// Asking for a name to save under.
    Name,
    /// Asking whether to replace a preset that already exists.
    Overwrite,
    /// Asking whether to delete one of your own presets. Carries the name
    /// rather than the row: an index re-resolved when the dialog is answered
    /// is how you end up deleting something other than what was asked for.
    Delete(String),
}

impl Data for Dialog {
    fn same(&self, other: &Self) -> bool {
        self == other
    }
}

/// So the preset list can be bound to and rebuilt when it changes.
impl Data for Preset {
    fn same(&self, other: &Self) -> bool {
        self == other
    }
}

#[derive(Lens)]
pub struct UiState {
    pub open: bool,
    pub menu: Menu,
    pub scale: f64,
    pub dialog: Dialog,
    /// Presets in the order the drop down shows them.
    pub presets: Vec<Preset>,
    /// Name on the preset button.
    pub current: String,
    /// Whether what is loaded is the factory preset of that name. A saved
    /// preset may share a name with a factory one, so the name alone does not
    /// say which row in the list is the one showing.
    pub current_built_in: bool,
    /// The values of the preset named above, kept so the panel can tell
    /// whether anything has been turned since it was loaded.
    pub reference: BTreeMap<String, f32>,
    /// First row shown in the preset list. Only the rows that fit are built,
    /// so this is how the list is scrolled rather than an offset applied to
    /// something already laid out.
    pub scroll: usize,
    /// Contents of the name field in the save dialog.
    pub name: String,
    /// What went wrong with the last save, if anything.
    pub error: String,
    pub params: Arc<crate::params::PultEqFxParams>,
}

pub enum UiEvent {
    ToggleSettings,
    Close,
    ToggleScaleMenu,
    SetScale(f64),
    TogglePresetMenu,
    LoadPreset(usize),
    /// Move the preset list by a number of rows, positive being downwards.
    ScrollPresets(i32),
    /// Ask before throwing away one of the saved presets.
    AskDelete(usize),
    /// Confirmed: throw it away. Built-in ones have no file and are refused.
    DeletePreset(String),
    OpenSaveDialog,
    NameEdited(String),
    /// Save under the name in the field, asking first if it is taken.
    RequestSave,
    /// Save even though it replaces something.
    ConfirmSave,
    CloseDialog,
}

impl UiState {
    pub fn new(scale: f64, params: Arc<crate::params::PultEqFxParams>) -> Self {
        let presets = presets::load_all(&params);
        // A reopened session remembers which preset it was set from, so pick
        // its values back up to compare against.
        let saved = params.preset_name();
        // Yours wins a tie: it is the one you made, and the factory preset of
        // that name is still there in the list either way.
        let restored = presets
            .iter()
            .find(|preset| preset.name == saved && !preset.built_in)
            .or_else(|| presets.iter().find(|preset| preset.name == saved));
        let current_built_in = restored.is_some_and(|preset| preset.built_in);
        let reference = restored
            .map(|preset| preset.values.clone())
            .unwrap_or_default();
        let current = if saved.is_empty() {
            String::from(NO_PRESET)
        } else {
            saved
        };

        Self {
            open: false,
            menu: Menu::None,
            scale,
            dialog: Dialog::None,
            presets,
            current,
            current_built_in,
            reference,
            scroll: 0,
            name: String::new(),
            error: String::new(),
            params,
        }
    }

    /// Push a preset's values out to the host as ordinary parameter gestures,
    /// so the change is automatable and undoable like any other edit.
    fn apply(&self, cx: &mut EventContext, preset: &Preset) {
        for (id, ptr, _) in self.params.param_map() {
            let Some(&value) = preset.values.get(&id) else {
                continue;
            };
            cx.emit(RawParamEvent::BeginSetParameter(ptr));
            cx.emit(RawParamEvent::SetParameterNormalized(ptr, value));
            cx.emit(RawParamEvent::EndSetParameter(ptr));
        }
        cx.needs_redraw();
    }

    fn store(&mut self, cx: &mut EventContext) {
        let name = self.name.trim().to_string();
        if name.is_empty() {
            return;
        }
        let preset = presets::capture(&self.params, &name);
        match presets::save(&preset) {
            Ok(_) => {
                self.presets = presets::load_all(&self.params);
                self.reference = preset.values.clone();
                self.params.set_preset_name(&name);
                self.current = name;
                // What is showing is now the file just written, not the
                // factory preset that may share its name.
                self.current_built_in = false;
                self.dialog = Dialog::None;
                self.error.clear();
            }
            Err(err) => {
                self.error = err.to_string();
                self.dialog = Dialog::Name;
            }
        }
        cx.needs_redraw();
    }
}

impl Model for UiState {
    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        event.map(|ui_event, meta| {
            match ui_event {
                UiEvent::ToggleSettings => {
                    self.open = !self.open;
                    self.menu = Menu::None;
                }
                UiEvent::Close => {
                    self.open = false;
                    self.menu = Menu::None;
                }
                UiEvent::ToggleScaleMenu => {
                    self.menu = if self.menu == Menu::Scale {
                        Menu::None
                    } else {
                        Menu::Scale
                    };
                }
                UiEvent::TogglePresetMenu => {
                    self.menu = if self.menu == Menu::Preset {
                        Menu::None
                    } else {
                        self.scroll = 0;
                        Menu::Preset
                    };
                }
                UiEvent::ScrollPresets(delta) => {
                    let rows = preset_rows(self.presets.len());
                    let most = rows.saturating_sub(MAX_PRESET_ROWS);
                    let next = self.scroll as i64 + *delta as i64;
                    self.scroll = next.clamp(0, most as i64) as usize;
                }
                UiEvent::LoadPreset(index) => {
                    self.menu = Menu::None;
                    if let Some(preset) = self.presets.get(*index).cloned() {
                        self.current = preset.name.clone();
                        self.current_built_in = preset.built_in;
                        self.reference = preset.values.clone();
                        self.params.set_preset_name(&preset.name);
                        self.apply(cx, &preset);
                    }
                }
                UiEvent::AskDelete(index) => {
                    // Deleting removes a file and there is no undo, so it goes
                    // through the same confirmation as replacing one.
                    if let Some(preset) = self.presets.get(*index) {
                        if !preset.built_in {
                            self.dialog = Dialog::Delete(preset.name.clone());
                            self.menu = Menu::None;
                        }
                    }
                }
                UiEvent::DeletePreset(name) => {
                    self.dialog = Dialog::None;
                    let name = name.clone();
                    // Looked up by name, and refused for anything compiled in:
                    // a factory preset has no file, and the list would only
                    // put it straight back.
                    let ours = self
                        .presets
                        .iter()
                        .any(|preset| !preset.built_in && preset.name == name);
                    if ours {
                        match presets::delete(&name) {
                            Ok(()) => {
                                self.presets = presets::load_all(&*self.params);
                                // Nothing is loaded any more if what was
                                // loaded has just been thrown away.
                                if self.current == name {
                                    self.current = String::from(NO_PRESET);
                                    self.reference.clear();
                                    self.params.set_preset_name("");
                                }
                                self.error.clear();
                            }
                            Err(err) => self.error = format!("could not delete: {err}"),
                        }
                    }
                }
                UiEvent::OpenSaveDialog => {
                    // Offer the current preset's name so replacing one is easy.
                    self.name = if self.current == NO_PRESET {
                        String::new()
                    } else {
                        self.current.clone()
                    };
                    self.error.clear();
                    self.dialog = Dialog::Name;
                    self.menu = Menu::None;
                    self.open = false;
                }
                UiEvent::NameEdited(text) => {
                    self.name = text.clone();
                }
                UiEvent::RequestSave => {
                    let name = self.name.trim().to_string();
                    if name.is_empty() {
                        return;
                    }
                    if presets::name_taken(&name, &self.presets) {
                        self.dialog = Dialog::Overwrite;
                    } else {
                        self.store(cx);
                    }
                }
                UiEvent::ConfirmSave => self.store(cx),
                UiEvent::CloseDialog => {
                    self.dialog = Dialog::None;
                    self.error.clear();
                }
                UiEvent::SetScale(scale) => {
                    self.scale = *scale;
                    self.menu = Menu::None;
                    // Into the state the host saves and sizes the window
                    // from, which vizia does not do for us. See the note on
                    // `remember_scale`.
                    crate::editor::remember_scale(&self.params.editor_state, *scale);
                    // NIH-plug watches the user scale factor and asks the host
                    // to resize the window to match.
                    cx.set_user_scale_factor(*scale);
                    cx.emit(GuiContextEvent::Resize);
                }
            }
            meta.consume();
        });
    }
}

// ---------------------------------------------------------------------------
// Header
// ---------------------------------------------------------------------------

pub struct Header;

impl Header {
    pub fn new(cx: &mut Context) -> Handle<'_, Self> {
        let handle = Self
            .build(cx, |cx| {
                Label::new(cx, "PultEQFx")
                    .position_type(PositionType::SelfDirected)
                    .left(Pixels(14.0))
                    .top(Pixels(0.0))
                    .height(Pixels(HEADER_H))
                    .child_top(Stretch(1.0))
                    .child_bottom(Stretch(1.0))
                    .font_family(vec![FamilyOwned::Name(String::from(assets::NOTO_SANS))])
                    .font_weight(FontWeightKeyword::Bold)
                    .font_size(11.0)
                    .color(Color::rgb(0xd6, 0xdc, 0xe2));
                Label::new(cx, "program equalizer")
                    .position_type(PositionType::SelfDirected)
                    .left(Pixels(106.0))
                    .top(Pixels(0.0))
                    .height(Pixels(HEADER_H))
                    .child_top(Stretch(1.0))
                    .child_bottom(Stretch(1.0))
                    .font_family(vec![FamilyOwned::Name(String::from(assets::NOTO_SANS))])
                    .font_size(11.0)
                    .color(Color::rgb(0x6d, 0x7c, 0x88));

                Label::new(cx, "PRESET")
                    .position_type(PositionType::SelfDirected)
                    .left(Pixels(300.0))
                    .top(Pixels(0.0))
                    .height(Pixels(HEADER_H))
                    .child_top(Stretch(1.0))
                    .child_bottom(Stretch(1.0))
                    .font_family(vec![FamilyOwned::Name(String::from(assets::NOTO_SANS))])
                    .font_size(10.0)
                    .color(Color::rgb(0x6d, 0x7c, 0x88));
                PresetButton::new(cx);

                SaveButton::new(cx)
                    .position_type(PositionType::SelfDirected)
                    .left(Pixels(PANEL_W - 58.0))
                    .top(Pixels((HEADER_H - 20.0) / 2.0));
                GearButton::new(cx)
                    .position_type(PositionType::SelfDirected)
                    .left(Pixels(PANEL_W - 30.0))
                    .top(Pixels((HEADER_H - 20.0) / 2.0));
            })
            .position_type(PositionType::SelfDirected)
            .left(Pixels(0.0))
            .top(Pixels(0.0))
            .width(Pixels(PANEL_W))
            .height(Pixels(HEADER_H));
        handle
    }
}

impl View for Header {
    fn element(&self) -> Option<&'static str> {
        Some("pulteqfx-header")
    }

    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        let b = cx.bounds();
        let mut bar = vg::Path::new();
        bar.rect(b.x, b.y, b.w, b.h);
        canvas.fill_path(
            &bar,
            &vg::Paint::linear_gradient(
                b.x,
                b.y,
                b.x,
                b.y + b.h,
                rgb(0x1d2125),
                rgb(0x0d0f12),
            ),
        );
        let mut edge = vg::Path::new();
        edge.move_to(b.x, b.y + b.h - 0.5);
        edge.line_to(b.x + b.w, b.y + b.h - 0.5);
        canvas.stroke_path(
            &edge,
            &vg::Paint::color(rgba(0x000000, 0.8)).with_line_width(cx.scale_factor()),
        );
    }
}

/// The button that opens the settings panel.
pub struct GearButton;

impl GearButton {
    pub fn new(cx: &mut Context) -> Handle<'_, Self> {
        Self.build(cx, |_| {})
            .width(Pixels(20.0))
            .height(Pixels(20.0))
    }
}

impl View for GearButton {
    fn element(&self) -> Option<&'static str> {
        Some("pulteqfx-gear")
    }

    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        let b = cx.bounds();
        let (mx, my) = (b.x + b.w / 2.0, b.y + b.h / 2.0);
        let r = b.w.min(b.h) * 0.40;
        let paint = vg::Paint::color(rgb(0xa8b4be));

        // Teeth.
        const TEETH: usize = 8;
        for i in 0..TEETH {
            let a = std::f32::consts::TAU * i as f32 / TEETH as f32;
            let (sa, ca) = a.sin_cos();
            let mut tooth = vg::Path::new();
            tooth.move_to(mx + r * 0.72 * sa, my - r * 0.72 * ca);
            tooth.line_to(mx + r * 1.20 * sa, my - r * 1.20 * ca);
            canvas.stroke_path(&tooth, &paint.clone().with_line_width(r * 0.34));
        }
        let mut ring = vg::Path::new();
        ring.circle(mx, my, r * 0.82);
        canvas.fill_path(&ring, &paint);
        let mut hole = vg::Path::new();
        hole.circle(mx, my, r * 0.34);
        canvas.fill_path(&hole, &vg::Paint::color(rgb(0x14171a)));
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        event.map(|window_event, meta| {
            if let WindowEvent::MouseDown(MouseButton::Left) = window_event {
                cx.emit(UiEvent::ToggleSettings);
                meta.consume();
            }
        });
    }
}

// ---------------------------------------------------------------------------
// Settings panel
// ---------------------------------------------------------------------------

/// Everything the settings button reveals, including the click catcher that
/// dismisses it.
pub struct SettingsOverlay;

impl SettingsOverlay {
    pub fn new(cx: &mut Context) {
        Binding::new(cx, UiState::open, |cx, open| {
            if !open.get(cx) {
                return;
            }
            // Clicking anywhere else puts the panel away.
            Dismiss::new(cx, true);
            Backdrop::new(cx, |cx| {
                heading(cx, "SETTINGS", 18.0, 14.0);

                setting_label(cx, "WINDOW SIZE", 48.0);
                ScaleButton::new(cx);

                setting_label(cx, "OVERSAMPLING", 84.0);
                oversampling_row(cx, 152.0, 76.0);

                setting_label(cx, "AMPLIFIER", 124.0);
                Knob::new(cx, Panel::params, |p| &p.drive, 18.0).place(184.0, 152.0, 18.0);
                caption(cx, "DRIVE", 184.0, 180.0);
                Knob::new(cx, Panel::params, |p| &p.output, 18.0).place(262.0, 152.0, 18.0);
                caption(cx, "OUTPUT", 262.0, 180.0);
            });
            ScaleMenu::new(cx);
        });
        // The preset list belongs to the header, so it shows whether or not
        // the settings panel is open.
        Binding::new(cx, UiState::menu, |cx, menu| {
            if menu.get(cx) == Menu::Preset {
                // Clicking anywhere else puts the list away again.
                Dismiss::new(cx, false);
                // Rebuilt whenever the presets change, so deleting one takes
                // its row out from under the pointer instead of leaving a
                // stale list whose indices no longer line up.
                Binding::new(cx, UiState::presets, |cx, _| {
                    PresetMenu::new(cx);
                });
            }
        });
    }
}

/// A sheet behind a panel or a menu that closes it when clicked. It only
/// darkens the plugin behind the settings panel; behind a drop down it is
/// invisible and just catches the click.
struct Dismiss {
    shaded: bool,
}

impl Dismiss {
    fn new(cx: &mut Context, shaded: bool) -> Handle<'_, Self> {
        Self { shaded }.build(cx, |_| {})
            .position_type(PositionType::SelfDirected)
            .left(Pixels(0.0))
            .top(Pixels(0.0))
            .width(Pixels(PANEL_W))
            .height(Pixels(WINDOW_H))
    }
}

impl View for Dismiss {
    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        if !self.shaded {
            return;
        }
        // A wash over the panel so the settings read as being in front.
        let b = cx.bounds();
        let mut sheet = vg::Path::new();
        sheet.rect(b.x, b.y, b.w, b.h);
        canvas.fill_path(&sheet, &vg::Paint::color(rgba(0x000408, 0.45)));
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        event.map(|window_event, meta| {
            if let WindowEvent::MouseDown(_) = window_event {
                cx.emit(UiEvent::Close);
                meta.consume();
            }
        });
    }
}

/// The dark card the settings sit on.
struct Backdrop;

impl Backdrop {
    fn new(cx: &mut Context, content: impl FnOnce(&mut Context)) -> Handle<'_, Self> {
        Self.build(cx, |cx| content(cx))
            .position_type(PositionType::SelfDirected)
            .left(Pixels(PANEL_X))
            .top(Pixels(PANEL_Y))
            .width(Pixels(PANEL_WIDTH))
            .height(Pixels(PANEL_HEIGHT))
    }
}

impl View for Backdrop {
    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        card(canvas, cx.bounds(), cx.scale_factor());
    }
}

fn card(canvas: &mut Canvas, b: BoundingBox, scale: f32) {
    let mut shadow = vg::Path::new();
    shadow.rounded_rect(b.x - 6.0, b.y - 2.0, b.w + 12.0, b.h + 14.0, 12.0 * scale);
    canvas.fill_path(&shadow, &vg::Paint::color(rgba(0x000000, 0.35)));

    let mut card = vg::Path::new();
    card.rounded_rect(b.x, b.y, b.w, b.h, 6.0 * scale);
    canvas.fill_path(
        &card,
        &vg::Paint::linear_gradient(b.x, b.y, b.x, b.y + b.h, rgb(0x23282d), rgb(0x161a1e)),
    );
    canvas.stroke_path(
        &card,
        &vg::Paint::color(rgba(0xffffff, 0.10)).with_line_width(scale),
    );
}

fn heading(cx: &mut Context, text: &str, y: f32, x: f32) {
    Label::new(cx, &super::track_out(text))
        .position_type(PositionType::SelfDirected)
        .left(Pixels(x))
        .top(Pixels(y - 9.0))
        .height(Pixels(18.0))
        .child_top(Stretch(1.0))
        .child_bottom(Stretch(1.0))
        .font_family(vec![FamilyOwned::Name(String::from(assets::NOTO_SANS))])
        .font_weight(FontWeightKeyword::Bold)
        .font_size(10.0)
        .color(Color::rgb(0x8e, 0x9c, 0xa8));
}

fn setting_label(cx: &mut Context, text: &str, y: f32) {
    Label::new(cx, text)
        .position_type(PositionType::SelfDirected)
        .left(Pixels(14.0))
        .top(Pixels(y - 9.0))
        .height(Pixels(18.0))
        .child_top(Stretch(1.0))
        .child_bottom(Stretch(1.0))
        .font_family(vec![FamilyOwned::Name(String::from(assets::NOTO_SANS))])
        .font_size(11.5)
        .color(Color::rgb(0xd2, 0xd8, 0xde));
}

fn caption(cx: &mut Context, text: &str, x: f32, y: f32) {
    label_box(cx, text, x, y, 9.0, 70.0, 0x9e, 0xaa, 0xb4, 255);
}

/// Formats a scale factor the way it is shown on the button.
pub fn scale_text(scale: f64) -> String {
    format!("{}%", (scale * 100.0).round() as i32)
}

// ---------------------------------------------------------------------------
// Window scale drop down
// ---------------------------------------------------------------------------

struct ScaleButton;

impl ScaleButton {
    fn new(cx: &mut Context) -> Handle<'_, Self> {
        Self.build(cx, |cx| {
            Label::new(cx, UiState::scale.map(|s| scale_text(*s)))
                .position_type(PositionType::SelfDirected)
                .left(Pixels(10.0))
                .top(Pixels(0.0))
                .height(Pixels(ROW_H))
                .child_top(Stretch(1.0))
                .child_bottom(Stretch(1.0))
                .font_family(vec![FamilyOwned::Name(String::from(assets::NOTO_SANS))])
                .font_size(11.5)
                .color(Color::rgb(0xf0, 0xf3, 0xf6));
        })
        .position_type(PositionType::SelfDirected)
        .left(Pixels(PANEL_WIDTH - 116.0))
        .top(Pixels(48.0 - ROW_H / 2.0))
        .width(Pixels(102.0))
        .height(Pixels(ROW_H))
    }
}

impl View for ScaleButton {
    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        let b = cx.bounds();
        field(canvas, b, cx.scale_factor());
        caret(canvas, b.x + b.w - 16.0 * cx.scale_factor(), b.y + b.h / 2.0, cx.scale_factor());
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        event.map(|window_event, meta| {
            if let WindowEvent::MouseDown(MouseButton::Left) = window_event {
                cx.emit(UiEvent::ToggleScaleMenu);
                meta.consume();
            }
        });
    }
}

/// How tall the open list is, and where it starts.
const MENU_H: f32 = SCALES.len() as f32 * ROW_H + 8.0;

/// Where the open list sits.
///
/// It would rather hang directly under the button, but a plugin editor cannot
/// draw outside its own window: the canvas is the window the host gave it, so
/// a list running past the bottom edge is cut off rather than overhanging.
/// This panel is tall enough today that the list fits as it is, but the
/// clamp costs nothing and keeps the last scale reachable if it ever is not.
fn menu_top() -> f32 {
    let under_button = PANEL_Y + 48.0 + ROW_H / 2.0 + 3.0;
    under_button.clamp(6.0, WINDOW_H - MENU_H - 6.0)
}

/// The list of scales, drawn above everything else when the button is pressed.
struct ScaleMenu;

impl ScaleMenu {
    fn new(cx: &mut Context) {
        Binding::new(cx, UiState::menu, |cx, menu| {
            if menu.get(cx) == Menu::Scale {
                MenuBackdrop::new(cx);
            }
        });
    }
}

struct MenuBackdrop;

impl MenuBackdrop {
    fn new(cx: &mut Context) -> Handle<'_, Self> {
        Self.build(cx, |cx| {
            for (i, &scale) in SCALES.iter().enumerate() {
                MenuItem::new(cx, scale, i);
            }
        })
        .position_type(PositionType::SelfDirected)
        .left(Pixels(PANEL_X + PANEL_WIDTH - 116.0))
        .top(Pixels(menu_top()))
        .width(Pixels(102.0))
        .height(Pixels(MENU_H))
    }
}

impl View for MenuBackdrop {
    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        card(canvas, cx.bounds(), cx.scale_factor());
    }
}

struct MenuItem {
    scale: f64,
}

impl MenuItem {
    fn new(cx: &mut Context, scale: f64, index: usize) -> Handle<'_, Self> {
        Self { scale }
            .build(cx, move |cx| {
                Label::new(cx, &scale_text(scale))
                    .position_type(PositionType::SelfDirected)
                    .left(Pixels(10.0))
                    .top(Pixels(0.0))
                    .height(Pixels(ROW_H))
                    .child_top(Stretch(1.0))
                    .child_bottom(Stretch(1.0))
                    .font_family(vec![FamilyOwned::Name(String::from(assets::NOTO_SANS))])
                    .font_size(11.5)
                    .color(Color::rgb(0xe4, 0xea, 0xf0));
            })
            .position_type(PositionType::SelfDirected)
            .left(Pixels(1.0))
            .top(Pixels(4.0 + index as f32 * ROW_H))
            .width(Pixels(100.0))
            .height(Pixels(ROW_H))
    }
}

impl View for MenuItem {
    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        if (UiState::scale.get(cx) - self.scale).abs() < 1e-6 {
            let b = cx.bounds();
            let mut row = vg::Path::new();
            row.rounded_rect(b.x + 2.0, b.y + 1.0, b.w - 4.0, b.h - 2.0, 3.0);
            canvas.fill_path(&row, &vg::Paint::color(rgba(0x4a7c8c, 0.55)));
        }
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        let scale = self.scale;
        event.map(|window_event, meta| {
            if let WindowEvent::MouseDown(MouseButton::Left) = window_event {
                cx.emit(UiEvent::SetScale(scale));
                meta.consume();
            }
        });
    }
}

// ---------------------------------------------------------------------------
// Oversampling
// ---------------------------------------------------------------------------

/// A segmented row, one segment per oversampling factor.
fn oversampling_row(cx: &mut Context, width: f32, y: f32) {
    let ptr = Panel::params.get(cx).oversampling.as_ptr();
    Segments::new(cx, width)
        .position_type(PositionType::SelfDirected)
        .left(Pixels(PANEL_WIDTH - 14.0 - width))
        .top(Pixels(y + 8.0 - ROW_H / 2.0))
        .width(Pixels(width))
        .height(Pixels(ROW_H));
    let seg = width / OVERSAMPLING.len() as f32;
    for (i, text) in OVERSAMPLING.iter().enumerate() {
        SegmentHit::new(cx, ptr, i)
            .position_type(PositionType::SelfDirected)
            .left(Pixels(PANEL_WIDTH - 14.0 - width + i as f32 * seg))
            .top(Pixels(y + 8.0 - ROW_H / 2.0))
            .width(Pixels(seg))
            .height(Pixels(ROW_H));
        label_box(
            cx,
            text,
            PANEL_WIDTH - 14.0 - width + (i as f32 + 0.5) * seg,
            y + 8.0,
            10.5,
            seg,
            0xe8,
            0xee,
            0xf4,
            255,
        );
    }
}

/// Draws the segmented control and marks the selected factor.
struct Segments {
    param: nih_plug_vizia::widgets::param_base::ParamWidgetBase,
    width: f32,
}

impl Segments {
    fn new(cx: &mut Context, width: f32) -> Handle<'_, Self> {
        Self {
            param: nih_plug_vizia::widgets::param_base::ParamWidgetBase::new(
                cx,
                Panel::params,
                |p| &p.oversampling,
            ),
            width,
        }
        .build(cx, |_| {})
    }
}

impl View for Segments {
    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        let b = cx.bounds();
        let scale = cx.scale_factor();
        field(canvas, b, scale);

        let count = OVERSAMPLING.len();
        let selected = (self.param.modulated_normalized_value() * (count - 1) as f32).round();
        let seg = b.w / count as f32;
        let mut pill = vg::Path::new();
        pill.rounded_rect(
            b.x + selected * seg + 2.0 * scale,
            b.y + 2.0 * scale,
            seg - 4.0 * scale,
            b.h - 4.0 * scale,
            3.0 * scale,
        );
        canvas.fill_path(&pill, &vg::Paint::color(rgba(0x4a7c8c, 0.75)));
        let _ = self.width;
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        event.map(|param_event, _| {
            if let RawParamEvent::ParametersChanged = param_event {
                cx.needs_redraw();
            }
        });
    }
}

/// A transparent hit area that selects one segment.
struct SegmentHit {
    ptr: ParamPtr,
    index: usize,
}

impl SegmentHit {
    fn new(cx: &mut Context, ptr: ParamPtr, index: usize) -> Handle<'_, Self> {
        Self { ptr, index }.build(cx, |_| {})
    }
}

impl View for SegmentHit {
    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        let (ptr, index) = (self.ptr, self.index);
        event.map(|window_event, meta| {
            if let WindowEvent::MouseDown(MouseButton::Left) = window_event {
                let normalized = index as f32 / (OVERSAMPLING.len() - 1) as f32;
                cx.emit(RawParamEvent::BeginSetParameter(ptr));
                cx.emit(RawParamEvent::SetParameterNormalized(ptr, normalized));
                cx.emit(RawParamEvent::EndSetParameter(ptr));
                cx.needs_redraw();
                meta.consume();
            }
        });
    }
}

// ---------------------------------------------------------------------------
// Shared chrome
// ---------------------------------------------------------------------------

fn field(canvas: &mut Canvas, b: BoundingBox, scale: f32) {
    let mut path = vg::Path::new();
    path.rounded_rect(b.x, b.y, b.w, b.h, 4.0 * scale);
    canvas.fill_path(&path, &vg::Paint::color(rgb(0x111519)));
    canvas.stroke_path(
        &path,
        &vg::Paint::color(rgba(0xffffff, 0.12)).with_line_width(scale),
    );
}

fn caret(canvas: &mut Canvas, x: f32, y: f32, scale: f32) {
    let s = 3.5 * scale;
    let mut path = vg::Path::new();
    path.move_to(x - s, y - s * 0.5);
    path.line_to(x, y + s * 0.7);
    path.line_to(x + s, y - s * 0.5);
    canvas.stroke_path(
        &path,
        &vg::Paint::color(rgb(0x9eacb8)).with_line_width(1.6 * scale),
    );
}


// ---------------------------------------------------------------------------
// Presets
// ---------------------------------------------------------------------------

const PRESET_X: f32 = 348.0;
const PRESET_W: f32 = 300.0;
/// Shown on the preset button when nothing has been loaded.
pub const NO_PRESET: &str = "\u{2014}";

/// The button in the header showing the loaded preset.
struct PresetButton;

impl PresetButton {
    fn new(cx: &mut Context) -> Handle<'_, Self> {
        Self.build(cx, |cx| {
            Label::new(cx, UiState::current)
                .position_type(PositionType::SelfDirected)
                .left(Pixels(10.0))
                .top(Pixels(0.0))
                .width(Pixels(PRESET_W - 32.0))
                .height(Pixels(ROW_H))
                .child_top(Stretch(1.0))
                .child_bottom(Stretch(1.0))
                .font_family(vec![FamilyOwned::Name(String::from(assets::NOTO_SANS))])
                .font_size(11.5)
                .color(Color::rgb(0xf0, 0xf3, 0xf6));
        })
        .position_type(PositionType::SelfDirected)
        .left(Pixels(PRESET_X))
        .top(Pixels((HEADER_H - ROW_H) / 2.0))
        .width(Pixels(PRESET_W))
        .height(Pixels(ROW_H))
    }
}

impl View for PresetButton {
    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        let b = cx.bounds();
        let scale = cx.scale_factor();
        field(canvas, b, scale);

        // A dot while the panel no longer matches the preset it was set from.
        if panel_modified(cx) {
            let mut dot = vg::Path::new();
            dot.circle(b.x + b.w - 30.0 * scale, b.y + b.h / 2.0, 3.2 * scale);
            canvas.fill_path(&dot, &vg::Paint::color(rgb(0xe0a343)));
        }

        caret(canvas, b.x + b.w - 16.0 * scale, b.y + b.h / 2.0, scale);
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        // Redraw when the host moves something, so the marker keeps up.
        event.map(|param_event, _| {
            if let RawParamEvent::ParametersChanged = param_event {
                cx.needs_redraw();
            }
        });
        event.map(|window_event, meta| {
            if let WindowEvent::MouseDown(MouseButton::Left) = window_event {
                cx.emit(UiEvent::TogglePresetMenu);
                meta.consume();
            }
        });
    }
}

/// Whether anything has been turned since the preset was loaded. Compared
/// against the values rather than tracked as a flag, so turning a control back
/// to where it was clears the marker again.
fn panel_modified(cx: &mut DrawContext) -> bool {
    let reference = UiState::reference.get(cx);
    let params = UiState::params.get(cx);
    !presets::matches(&params, &reference)
}

/// How tall the preset list may get before it starts another column, in rows.
/// Sized to the window with the header and the card's own padding taken off.
pub const MAX_PRESET_ROWS: usize = ((WINDOW_H - HEADER_H - 16.0) / ROW_H) as usize;
/// Width of the scroll bar's gutter, when there is one.
const GUTTER: f32 = 6.0;

/// How many columns fit across the panel.
fn preset_columns_available() -> usize {
    (((PANEL_W - PRESET_X - 8.0) / PRESET_W) as usize).max(1)
}

/// How many columns a given number of presets is laid out in.
pub fn preset_columns(count: usize) -> usize {
    count
        .div_ceil(MAX_PRESET_ROWS)
        .max(1)
        .min(preset_columns_available())
}

/// How deep the grid is: as few rows as the columns allow, so a short list
/// stays short and a long one gets deeper rather than wider once it has run
/// out of width.
pub fn preset_rows(count: usize) -> usize {
    count.div_ceil(preset_columns(count)).max(1)
}

/// The list of presets, built fresh each time it opens so a preset saved a
/// moment ago is in it.
///
/// The shipped presets are a handful, but the ones you save are however many
/// you save, so the list has no ceiling and cannot simply be as tall as it
/// needs to be. It fills the panel's width in columns first, and once it has
/// run out of width it scrolls. Scrolling is done by building only the rows
/// that are on screen rather than by moving something already laid out, so the
/// rows that are not visible do not exist and cannot be clicked through the
/// edge of the card.
struct PresetMenu;

impl PresetMenu {
    fn new(cx: &mut Context) -> Handle<'_, Self> {
        let names: Vec<(String, bool)> = UiState::presets
            .get(cx)
            .iter()
            .map(|preset| (preset.name.clone(), preset.built_in))
            .collect();
        let scroll = UiState::scroll.get(cx);

        let count = names.len();
        let columns = preset_columns(count);
        let rows = preset_rows(count);
        let visible = rows.min(MAX_PRESET_ROWS);
        let scrolls = rows > MAX_PRESET_ROWS;
        let width = columns as f32 * PRESET_W + if scrolls { GUTTER } else { 0.0 };

        Self.build(cx, move |cx| {
            if names.is_empty() {
                label_box(cx, "no presets", PRESET_W / 2.0, 4.0 + ROW_H / 2.0, 11.0, PRESET_W, 0x7e, 0x8a, 0x96, 255);
            }

            for column in 0..columns {
                for row in 0..visible {
                    // Column major: a list is read down a column.
                    if scroll + row >= rows {
                        continue;
                    }
                    let index = column * rows + scroll + row;
                    let Some((name, built_in)) = names.get(index) else {
                        continue;
                    };
                    PresetItem::new(cx, name, *built_in, index)
                        .left(Pixels(1.0 + column as f32 * PRESET_W))
                        .top(Pixels(4.0 + row as f32 * ROW_H));
                }
            }

            if scrolls {
                PresetScrollBar::new(cx, scroll, rows, visible)
                    .position_type(PositionType::SelfDirected)
                    .left(Pixels(width - GUTTER - 2.0))
                    .top(Pixels(4.0))
                    .width(Pixels(GUTTER))
                    .height(Pixels(visible as f32 * ROW_H));
            }
        })
        .position_type(PositionType::SelfDirected)
        // Kept inside the window: with enough columns the natural left edge
        // would push the last one off the right hand side.
        .left(Pixels(PRESET_X.min(PANEL_W - width - 8.0).max(8.0)))
        .top(Pixels(HEADER_H - 2.0))
        .width(Pixels(width))
        .height(Pixels(visible as f32 * ROW_H + 8.0))
    }
}

impl View for PresetMenu {
    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        card(canvas, cx.bounds(), cx.scale_factor());
    }

    /// The wheel is taken here rather than on a transparent sheet behind the
    /// rows. A sheet would only work over the gaps between them: vizia sends
    /// an event the row did not consume up to its parent, never sideways to a
    /// sibling, and the rows sit on top. The menu *is* the parent, so
    /// everything the rows ignore arrives here.
    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        event.map(|window_event, meta| {
            if let WindowEvent::MouseScroll(_, y) = window_event {
                // One notch, one row. A preset list is read, not skimmed.
                cx.emit(UiEvent::ScrollPresets(if *y > 0.0 { -1 } else { 1 }));
                meta.consume();
            }
        });
    }
}

/// The bar down the right of the list, showing how much of it is on screen.
struct PresetScrollBar {
    scroll: usize,
    rows: usize,
    visible: usize,
}

impl PresetScrollBar {
    fn new(cx: &mut Context, scroll: usize, rows: usize, visible: usize) -> Handle<'_, Self> {
        Self { scroll, rows, visible }.build(cx, |_| {}).hoverable(false)
    }
}

impl View for PresetScrollBar {
    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        let b = cx.bounds();
        let scale = cx.scale_factor();
        let rows = self.rows.max(1) as f32;
        let visible = self.visible.max(1) as f32;

        let mut track = vg::Path::new();
        track.rounded_rect(b.x + b.w * 0.25, b.y, b.w * 0.5, b.h, b.w * 0.25);
        canvas.fill_path(&track, &vg::Paint::color(rgba(0xffffff, 0.06)));

        let thumb_h = (b.h * visible / rows).max(12.0 * scale);
        let travel = b.h - thumb_h;
        let progress = if rows > visible {
            self.scroll as f32 / (rows - visible)
        } else {
            0.0
        };
        let mut thumb = vg::Path::new();
        thumb.rounded_rect(
            b.x + b.w * 0.25,
            b.y + travel * progress,
            b.w * 0.5,
            thumb_h,
            b.w * 0.25,
        );
        canvas.fill_path(&thumb, &vg::Paint::color(rgba(0xffffff, 0.26)));
    }
}

struct PresetItem {
    index: usize,
}

impl PresetItem {
    fn new<'a>(cx: &'a mut Context, name: &str, built_in: bool, index: usize) -> Handle<'a, Self> {
        let text = name.to_string();
        Self { index }
            .build(cx, move |cx| {
                Label::new(cx, &text)
                    .position_type(PositionType::SelfDirected)
                    .left(Pixels(10.0))
                    .top(Pixels(0.0))
                    .width(Pixels(PRESET_W - 60.0))
                    .height(Pixels(ROW_H))
                    .child_top(Stretch(1.0))
                    .child_bottom(Stretch(1.0))
                    .font_family(vec![FamilyOwned::Name(String::from(assets::NOTO_SANS))])
                    .font_size(11.5)
                    .color(Color::rgb(0xe4, 0xea, 0xf0));
                if built_in {
                    label_box(cx, "factory", PRESET_W - 44.0, ROW_H / 2.0, 9.0, 60.0, 0x76, 0x86, 0x92, 255);
                } else {
                    DeleteButton::new(cx, index);
                }
            })
            .position_type(PositionType::SelfDirected)
            .left(Pixels(1.0))
            .top(Pixels(4.0 + index as f32 * ROW_H))
            .width(Pixels(PRESET_W - 2.0))
            .height(Pixels(ROW_H))
    }
}

/// The cross at the right of one of your own presets. Sits inside the row, so
/// it has to swallow the click: without that the row underneath would load the
/// preset on the way past, and the list would be rebuilt around a preset that
/// no longer exists.
struct DeleteButton {
    index: usize,
    /// Tracked here rather than read from the draw context, which does not
    /// expose it.
    hot: bool,
}

impl DeleteButton {
    fn new(cx: &mut Context, index: usize) -> Handle<'_, Self> {
        Self { index, hot: false }
            .build(cx, |_| {})
            .position_type(PositionType::SelfDirected)
            .left(Pixels(PRESET_W - 30.0))
            .top(Pixels((ROW_H - 16.0) / 2.0))
            .width(Pixels(16.0))
            .height(Pixels(16.0))
    }
}

impl View for DeleteButton {
    fn element(&self) -> Option<&'static str> {
        Some("preset-delete")
    }

    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        let b = cx.bounds();
        let scale = cx.scale_factor();
        let hot = self.hot;
        let (mx, my) = (b.x + b.w / 2.0, b.y + b.h / 2.0);

        if hot {
            let mut disc = vg::Path::new();
            disc.circle(mx, my, b.w * 0.5);
            canvas.fill_path(&disc, &vg::Paint::color(rgba(0xc0392b, 0.85)));
        }

        let arm = b.w * 0.24;
        let mut cross = vg::Path::new();
        cross.move_to(mx - arm, my - arm);
        cross.line_to(mx + arm, my + arm);
        cross.move_to(mx + arm, my - arm);
        cross.line_to(mx - arm, my + arm);
        let ink = if hot {
            rgba(0xffffff, 0.95)
        } else {
            rgba(0x9aa6b0, 0.75)
        };
        canvas.stroke_path(
            &cross,
            &vg::Paint::color(ink)
                .with_line_width(1.6 * scale)
                .with_line_cap(vg::LineCap::Round),
        );
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        let index = self.index;
        event.map(|window_event, meta| match window_event {
            WindowEvent::MouseDown(MouseButton::Left) => {
                cx.emit(UiEvent::AskDelete(index));
                meta.consume();
            }
            // The row below would otherwise take the release as its own.
            WindowEvent::MouseUp(MouseButton::Left) => meta.consume(),
            WindowEvent::MouseEnter => {
                self.hot = true;
                cx.needs_redraw();
            }
            WindowEvent::MouseLeave => {
                self.hot = false;
                cx.needs_redraw();
            }
            _ => {}
        });
    }
}

impl View for PresetItem {
    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        let selected = UiState::presets
            .get(cx)
            .get(self.index)
            .map(|preset| {
                preset.name == UiState::current.get(cx)
                    && preset.built_in == UiState::current_built_in.get(cx)
            })
            .unwrap_or(false);
        if selected {
            let b = cx.bounds();
            let mut row = vg::Path::new();
            row.rounded_rect(b.x + 2.0, b.y + 1.0, b.w - 4.0, b.h - 2.0, 3.0);
            canvas.fill_path(&row, &vg::Paint::color(rgba(0x4a7c8c, 0.55)));
        }
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        let index = self.index;
        event.map(|window_event, meta| {
            if let WindowEvent::MouseDown(MouseButton::Left) = window_event {
                cx.emit(UiEvent::LoadPreset(index));
                meta.consume();
            }
        });
    }
}

/// The button that opens the save dialog.
pub struct SaveButton;

impl SaveButton {
    pub fn new(cx: &mut Context) -> Handle<'_, Self> {
        Self.build(cx, |_| {})
            .width(Pixels(20.0))
            .height(Pixels(20.0))
    }
}

impl View for SaveButton {
    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        // A floppy disk, which is still what "save" looks like.
        let b = cx.bounds();
        let s = b.w.min(b.h) * 0.78;
        let (x, y) = (b.x + (b.w - s) / 2.0, b.y + (b.h - s) / 2.0);
        let ink = vg::Paint::color(rgb(0xa8b4be));

        let mut body = vg::Path::new();
        body.rounded_rect(x, y, s, s, s * 0.12);
        canvas.stroke_path(&body, &ink.clone().with_line_width(s * 0.11));
        // Shutter at the top.
        let mut shutter = vg::Path::new();
        shutter.rect(x + s * 0.28, y + s * 0.10, s * 0.44, s * 0.30);
        canvas.fill_path(&shutter, &ink);
        // Label at the bottom.
        let mut label = vg::Path::new();
        label.rect(x + s * 0.22, y + s * 0.55, s * 0.56, s * 0.33);
        canvas.stroke_path(&label, &ink.clone().with_line_width(s * 0.09));
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        event.map(|window_event, meta| {
            if let WindowEvent::MouseDown(MouseButton::Left) = window_event {
                cx.emit(UiEvent::OpenSaveDialog);
                meta.consume();
            }
        });
    }
}


// ---------------------------------------------------------------------------
// Dialogs
// ---------------------------------------------------------------------------

const DIALOG_W: f32 = 400.0;
const DIALOG_H: f32 = 168.0;
const DIALOG_X: f32 = (PANEL_W - DIALOG_W) / 2.0;
const DIALOG_Y: f32 = 96.0;

/// The save and overwrite dialogs, shown one at a time over everything else.
pub struct Dialogs;

impl Dialogs {
    pub fn new(cx: &mut Context) {
        Binding::new(cx, UiState::dialog, |cx, dialog| match dialog.get(cx) {
            Dialog::None => {}
            Dialog::Name => {
                Shade::new(cx);
                DialogCard::new(cx, |cx| {
                    dialog_title(cx, "SAVE PRESET");
                    dialog_text(cx, "Name this preset.", 50.0);

                    let field = Textbox::new(cx, UiState::name)
                        .position_type(PositionType::SelfDirected)
                        .left(Pixels(20.0))
                        .top(Pixels(70.0))
                        .width(Pixels(DIALOG_W - 40.0))
                        .height(Pixels(28.0))
                        .child_left(Pixels(9.0))
                        .child_top(Stretch(1.0))
                        .child_bottom(Stretch(1.0))
                        .background_color(Color::rgb(0x11, 0x15, 0x19))
                        .border_color(Color::rgb(0x44, 0x4e, 0x57))
                        .border_width(Pixels(1.0))
                        .border_radius(Pixels(4.0))
                        .font_family(vec![FamilyOwned::Name(String::from(assets::NOTO_SANS))])
                        .font_size(12.0)
                        .color(Color::rgb(0xf0, 0xf3, 0xf6))
                        .caret_color(Color::rgb(0xd0, 0xd8, 0xde))
                        .selection_color(Color::rgba(0x4a, 0x7c, 0x8c, 0xaa))
                        .on_edit(|cx, text| cx.emit(UiEvent::NameEdited(text)))
                        // The flag is true only when the field was submitted
                        // with the enter key. Losing focus also submits, with
                        // it false, so ignoring it means clicking Cancel saves
                        // the preset on the way out.
                        .on_submit(|cx, text, confirmed| {
                            cx.emit(UiEvent::NameEdited(text));
                            if confirmed {
                                cx.emit(UiEvent::RequestSave);
                            }
                        })
                        .entity();
                    // Ready to type as soon as the dialog appears.
                    cx.emit_to(field, TextEvent::StartEdit);

                    Binding::new(cx, UiState::error, |cx, error| {
                        let error = error.get(cx);
                        if !error.is_empty() {
                            label_box(cx, &error, DIALOG_W / 2.0, 112.0, 10.0, DIALOG_W - 40.0, 0xe0, 0x86, 0x78, 255);
                        }
                    });

                    DialogButton::new(cx, "CANCEL", false, |cx| cx.emit(UiEvent::CloseDialog))
                        .left(Pixels(DIALOG_W - 208.0))
                        .top(Pixels(DIALOG_H - 46.0));
                    DialogButton::new(cx, "SAVE", true, |cx| cx.emit(UiEvent::RequestSave))
                        .left(Pixels(DIALOG_W - 108.0))
                        .top(Pixels(DIALOG_H - 46.0));
                });
            }
            Dialog::Overwrite => {
                Shade::new(cx);
                DialogCard::new(cx, |cx| {
                    dialog_title(cx, "REPLACE PRESET");
                    Binding::new(cx, UiState::name, |cx, name| {
                        let message = format!("\u{201c}{}\u{201d} already exists.", name.get(cx).trim());
                        dialog_text(cx, &message, 58.0);
                    });
                    dialog_text(cx, "Saving will replace it.", 80.0);

                    DialogButton::new(cx, "CANCEL", false, |cx| cx.emit(UiEvent::CloseDialog))
                        .left(Pixels(DIALOG_W - 218.0))
                        .top(Pixels(DIALOG_H - 46.0));
                    DialogButton::new(cx, "REPLACE", true, |cx| cx.emit(UiEvent::ConfirmSave))
                        .left(Pixels(DIALOG_W - 118.0))
                        .top(Pixels(DIALOG_H - 46.0));
                });
            }
            Dialog::Delete(name) => {
                let name = name.clone();
                Shade::new(cx);
                DialogCard::new(cx, move |cx| {
                    dialog_title(cx, "DELETE PRESET");
                    let message = format!("\u{201c}{}\u{201d} will be removed.", name.trim());
                    dialog_text(cx, &message, 58.0);
                    dialog_text(cx, "This cannot be undone.", 80.0);

                    DialogButton::new(cx, "CANCEL", false, |cx| cx.emit(UiEvent::CloseDialog))
                        .left(Pixels(DIALOG_W - 218.0))
                        .top(Pixels(DIALOG_H - 46.0));
                    let confirmed = name.clone();
                    DialogButton::new(cx, "DELETE", true, move |cx| {
                        cx.emit(UiEvent::DeletePreset(confirmed.clone()))
                    })
                    .left(Pixels(DIALOG_W - 118.0))
                    .top(Pixels(DIALOG_H - 46.0));
                });
            }
        });
    }
}

/// Darkens the plugin behind a dialog and swallows clicks aimed past it.
struct Shade;

impl Shade {
    fn new(cx: &mut Context) -> Handle<'_, Self> {
        Self.build(cx, |_| {})
            .position_type(PositionType::SelfDirected)
            .left(Pixels(0.0))
            .top(Pixels(0.0))
            .width(Pixels(PANEL_W))
            .height(Pixels(WINDOW_H))
    }
}

impl View for Shade {
    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        let b = cx.bounds();
        let mut sheet = vg::Path::new();
        sheet.rect(b.x, b.y, b.w, b.h);
        canvas.fill_path(&sheet, &vg::Paint::color(rgba(0x000408, 0.62)));
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        event.map(|window_event, meta| {
            // A click outside the dialog cancels it, as it would anywhere else.
            if let WindowEvent::MouseDown(_) = window_event {
                cx.emit(UiEvent::CloseDialog);
                meta.consume();
            }
        });
    }
}

struct DialogCard;

impl DialogCard {
    fn new(cx: &mut Context, content: impl FnOnce(&mut Context)) -> Handle<'_, Self> {
        Self.build(cx, |cx| content(cx))
            .position_type(PositionType::SelfDirected)
            .left(Pixels(DIALOG_X))
            .top(Pixels(DIALOG_Y))
            .width(Pixels(DIALOG_W))
            .height(Pixels(DIALOG_H))
    }
}

impl View for DialogCard {
    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        card(canvas, cx.bounds(), cx.scale_factor());
    }

    fn event(&mut self, _cx: &mut EventContext, event: &mut Event) {
        // Clicks on the card belong to the card, not to the shade behind it.
        event.map(|window_event, meta| {
            if let WindowEvent::MouseDown(_) = window_event {
                meta.consume();
            }
        });
    }
}

fn dialog_title(cx: &mut Context, text: &str) {
    Label::new(cx, &super::track_out(text))
        .position_type(PositionType::SelfDirected)
        .left(Pixels(20.0))
        .top(Pixels(12.0))
        .height(Pixels(20.0))
        .child_top(Stretch(1.0))
        .child_bottom(Stretch(1.0))
        .font_family(vec![FamilyOwned::Name(String::from(assets::NOTO_SANS))])
        .font_weight(FontWeightKeyword::Bold)
        .font_size(11.0)
        .color(Color::rgb(0x8e, 0x9c, 0xa8));
}

fn dialog_text(cx: &mut Context, text: &str, y: f32) {
    Label::new(cx, text)
        .position_type(PositionType::SelfDirected)
        .left(Pixels(20.0))
        .top(Pixels(y - 9.0))
        .width(Pixels(DIALOG_W - 40.0))
        .height(Pixels(18.0))
        .child_top(Stretch(1.0))
        .child_bottom(Stretch(1.0))
        .font_family(vec![FamilyOwned::Name(String::from(assets::NOTO_SANS))])
        .font_size(11.5)
        .color(Color::rgb(0xd2, 0xd8, 0xde));
}

/// A dialog's push button.
struct DialogButton {
    action: Box<dyn Fn(&mut EventContext)>,
    accent: bool,
}

impl DialogButton {
    fn new<'a>(
        cx: &'a mut Context,
        text: &str,
        accent: bool,
        action: impl Fn(&mut EventContext) + 'static,
    ) -> Handle<'a, Self> {
        let text = text.to_string();
        Self {
            action: Box::new(action),
            accent,
        }
        .build(cx, move |cx| {
            Label::new(cx, &text)
                .position_type(PositionType::SelfDirected)
                .left(Pixels(0.0))
                .top(Pixels(0.0))
                .width(Pixels(88.0))
                .height(Pixels(28.0))
                .child_left(Stretch(1.0))
                .child_right(Stretch(1.0))
                .child_top(Stretch(1.0))
                .child_bottom(Stretch(1.0))
                .font_family(vec![FamilyOwned::Name(String::from(assets::NOTO_SANS))])
                .font_weight(FontWeightKeyword::Bold)
                .font_size(10.5)
                .color(Color::rgb(0xf2, 0xf5, 0xf8));
        })
        .position_type(PositionType::SelfDirected)
        .width(Pixels(88.0))
        .height(Pixels(28.0))
    }
}

impl View for DialogButton {
    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        let b = cx.bounds();
        let scale = cx.scale_factor();
        let mut path = vg::Path::new();
        path.rounded_rect(b.x, b.y, b.w, b.h, 4.0 * scale);
        if self.accent {
            canvas.fill_path(
                &path,
                &vg::Paint::linear_gradient(b.x, b.y, b.x, b.y + b.h, rgb(0x598ea0), rgb(0x3a6676)),
            );
        } else {
            canvas.fill_path(&path, &vg::Paint::color(rgb(0x2c3238)));
        }
        canvas.stroke_path(
            &path,
            &vg::Paint::color(rgba(0xffffff, 0.14)).with_line_width(scale),
        );
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        event.map(|window_event, meta| {
            if let WindowEvent::MouseDown(MouseButton::Left) = window_event {
                (self.action)(cx);
                meta.consume();
            }
        });
    }
}

#[cfg(test)]
mod layout_tests {
    use super::{preset_columns, preset_rows, MAX_PRESET_ROWS};

    /// What the menu would build at a given scroll offset: the preset index in
    /// each visible cell. Mirrors the loop in `PresetMenu::new`, which is the
    /// point -- the arithmetic is what is being checked, not the drawing.
    fn visible(count: usize, scroll: usize) -> Vec<usize> {
        let columns = preset_columns(count);
        let rows = preset_rows(count);
        let showing = rows.min(MAX_PRESET_ROWS);
        let mut out = Vec::new();
        for column in 0..columns {
            for row in 0..showing {
                if scroll + row >= rows {
                    continue;
                }
                let index = column * rows + scroll + row;
                if index < count {
                    out.push(index);
                }
            }
        }
        out
    }

    /// Every preset has to be reachable, and none may appear twice on screen
    /// at once. A column that spilled into the next column's range would do
    /// both at the same time.
    #[test]
    fn every_preset_is_reachable_and_shown_once() {
        for count in 1..200usize {
            let rows = preset_rows(count);
            let most = rows.saturating_sub(MAX_PRESET_ROWS);

            let mut reached = vec![false; count];
            for scroll in 0..=most {
                let shown = visible(count, scroll);
                let mut seen = shown.clone();
                seen.sort_unstable();
                seen.dedup();
                assert_eq!(
                    seen.len(),
                    shown.len(),
                    "{count} presets show a duplicate at offset {scroll}"
                );
                for index in shown {
                    reached[index] = true;
                }
            }
            let missing: Vec<usize> = reached
                .iter()
                .enumerate()
                .filter(|(_, seen)| !**seen)
                .map(|(i, _)| i)
                .collect();
            assert!(
                missing.is_empty(),
                "{count} presets: {missing:?} cannot be reached"
            );
        }
    }

    #[test]
    fn a_short_list_needs_no_scrolling() {
        assert_eq!(preset_columns(MAX_PRESET_ROWS), 1);
        assert_eq!(preset_rows(MAX_PRESET_ROWS), MAX_PRESET_ROWS);
    }
}
