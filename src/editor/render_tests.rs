//! The real panel, built and drawn through Skia's raster surface. No display
//! server and no window: what is exercised is the view tree, its signals and
//! every `draw`, which is where a port from femtovg can quietly go wrong.
//!
//! Set `PULTEQFX_GUI_SNAPSHOTS` to a directory to have each state written out
//! as a PNG.

use super::*;
use vizia_plug::vizia::{
    backend::{BackendContext, WindowDescription},
    events::EventManager,
    vg as sk,
};
use vizia_plug::widgets::param_registry::ParamRegistry;

struct Root;
impl View for Root {}

/// Lays out and draws a few frames, then checks that every pixel of the
/// window was painted -- the panel covers all of it, so a transparent one is
/// one nothing drew -- and returns the picture.
fn render(
    backend: &mut BackendContext,
    events: &mut EventManager,
    surfaces: &mut (sk::Surface, sk::Surface),
    name: &str,
) -> Vec<u8> {
    for _ in 0..4 {
        events.flush_events(backend.context(), |_| {});
        backend.process_style_updates();
        backend.process_animations();
        backend.process_visual_updates();
        backend.draw(Entity::root(), &mut surfaces.0, &mut surfaces.1);
    }
    let image = surfaces.0.image_snapshot();
    let info = image.image_info();
    let mut pixels = vec![0u8; (info.width() * info.height() * 4) as usize];
    assert!(image.read_pixels(
        info,
        &mut pixels,
        (info.width() * 4) as usize,
        (0, 0),
        sk::image::CachingHint::Allow,
    ));
    let undrawn = pixels
        .as_chunks::<4>()
        .0
        .iter()
        .filter(|p| p[3] != 255)
        .count();
    assert_eq!(
        undrawn,
        0,
        "{name}: {undrawn} of {}x{} pixels never drawn",
        info.width(),
        info.height()
    );
    if let Ok(directory) = std::env::var("PULTEQFX_GUI_SNAPSHOTS") {
        let data = image
            .encode(None, sk::EncodedImageFormat::PNG, None)
            .unwrap();
        std::fs::write(
            std::path::Path::new(&directory).join(format!("{name}.png")),
            data.as_bytes(),
        )
        .unwrap();
    }
    pixels
}

/// How many bytes of two pictures differ, for telling that something moved.
fn differs(a: &[u8], b: &[u8]) -> usize {
    a.iter().zip(b).filter(|(a, b)| a != b).count()
}

#[test]
fn the_panel_draws_and_its_overlays_follow_their_signals() {
    Runtime::init_on_ui_thread();
    let mut cx = Context::new();
    cx.ignore_default_theme = true;
    let mut backend = BackendContext::new(cx);
    let size = (PANEL_W as u32, WINDOW_H as u32);
    let desc = WindowDescription::new().with_inner_size(size.0, size.1);
    backend.add_main_window(Entity::root(), &desc, 1.0);
    backend.add_window(Root);
    backend.0.windows.insert(
        Entity::root(),
        WindowState {
            window_description: desc,
            ..Default::default()
        },
    );
    backend.context().add_built_in_styles();
    ParamRegistry::new().build(backend.context());

    let params = Arc::new(PultEqFxParams::default());
    let meters = Arc::new(Meters::default());
    build(backend.context(), params.clone(), meters.clone(), 1.0);

    let mut events = EventManager::new();
    let new_surface = || sk::surfaces::raster_n32_premul((size.0 as i32, size.1 as i32)).unwrap();
    let mut surfaces = (new_surface(), new_surface());

    let panel = render(&mut backend, &mut events, &mut surfaces, "panel");

    // A host's automation arriving, and the overlays one after another. Each
    // has to draw in full and go away again when it is closed.
    backend.context().emit(RawParamEvent::ParametersChanged);
    let mut previous = render(&mut backend, &mut events, &mut surfaces, "automated");
    for (event, name, shows) in [
        (settings::UiEvent::ToggleSettings, "settings", true),
        (settings::UiEvent::ToggleScaleMenu, "sizes", true),
        (settings::UiEvent::Close, "closed", false),
        (settings::UiEvent::TogglePresetMenu, "presets", true),
        (settings::UiEvent::TogglePresetMenu, "presets-closed", false),
        (settings::UiEvent::OpenSaveDialog, "save", true),
    ] {
        backend.context().emit(event);
        let now = render(&mut backend, &mut events, &mut surfaces, name);
        assert!(
            differs(&now, &previous) > 0,
            "{name}: the picture did not change"
        );
        // Closing an overlay leaves the panel exactly as it was.
        assert_eq!(
            differs(&now, &panel) == 0,
            !shows,
            "{name}: {} pixels differ from the bare panel",
            differs(&now, &panel) / 4
        );
        previous = now;
    }

    // The save dialog opens with the name field ready to type into.
    assert_eq!(backend.focused_element(), Some("textbox"));
    for c in "Air".chars() {
        backend.emit_origin(WindowEvent::CharInput(c));
    }
    render(&mut backend, &mut events, &mut surfaces, "typed");
    assert_eq!(backend.context().data::<UiState>().ui().name, "Air");

    // Cancelled rather than saved: this test must not write into the real
    // preset folder.
    backend.context().emit(settings::UiEvent::CloseDialog);
    render(&mut backend, &mut events, &mut surfaces, "cancelled");
    assert_eq!(
        backend.context().data::<UiState>().ui().dialog,
        settings::Dialog::None
    );
    Runtime::deinit_on_ui_thread();
}

/// The meters follow the audio on their own: the timer started with the
/// panel reads what arrives into the figures and draws the bars again, with
/// nothing else having to ask.
#[test]
fn the_meters_show_what_arrives() {
    Runtime::init_on_ui_thread();
    let mut cx = Context::new();
    cx.ignore_default_theme = true;
    let mut backend = BackendContext::new(cx);
    let size = (PANEL_W as u32, WINDOW_H as u32);
    let desc = WindowDescription::new().with_inner_size(size.0, size.1);
    backend.add_main_window(Entity::root(), &desc, 1.0);
    backend.add_window(Root);
    backend.0.windows.insert(
        Entity::root(),
        WindowState {
            window_description: desc,
            ..Default::default()
        },
    );
    backend.context().add_built_in_styles();
    ParamRegistry::new().build(backend.context());

    let params = Arc::new(PultEqFxParams::default());
    let meters = Arc::new(Meters::default());
    meters.set_channels(2);
    build(backend.context(), params, meters.clone(), 1.0);

    let mut events = EventManager::new();
    let new_surface = || sk::surfaces::raster_n32_premul((size.0 as i32, size.1 as i32)).unwrap();
    let mut surfaces = (new_surface(), new_surface());
    let silent = render(&mut backend, &mut events, &mut surfaces, "silent");

    // Full scale on both channels of the output, and then what the baseview
    // backend does between frames once the timer is due.
    for channel in 0..2 {
        meters.output.publish(channel, 1.0, 1.0);
        meters.output.publish_figure(channel, 0.5);
    }
    std::thread::sleep(METER_INTERVAL * 2);
    backend.process_timers();
    let loud = render(&mut backend, &mut events, &mut surfaces, "loud");

    let shown = backend.context().data::<Panel>().readings.get_untracked();
    assert_eq!(shown.peak[1], "0.0");
    assert_eq!(shown.rms[1], "-3.0");
    assert!(shown.over[1], "full scale is over");
    assert!(!shown.over[0], "nothing arrived at the input");

    // The output meter's window, where the bars are.
    let (left, top) = (
        (OUTPUT_METER_X - meter::WIDTH / 2.0) as usize,
        (HEADER_H + METER_TOP) as usize,
    );
    let window = |pixels: &[u8]| -> Vec<u8> {
        (top..top + meter::HEIGHT as usize)
            .flat_map(|y| {
                let row = (y * size.0 as usize + left) * 4;
                pixels[row..row + meter::WIDTH as usize * 4].to_vec()
            })
            .collect()
    };
    assert!(
        differs(&window(&loud), &window(&silent)) > 0,
        "the output meter's bars did not move for a full scale signal"
    );
    Runtime::deinit_on_ui_thread();
}
