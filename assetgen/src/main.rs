//! Offline generator for the panel control sprites.
//!
//! Geometry is built parametrically and rendered here rather than shipped as
//! photographs, so the assets carry the project's own licence and the lighting
//! is consistent across every control on the panel.

mod ao;
mod gltf;
mod mesh;
mod parts;
mod raster;
mod shade;
mod vec;

use parts::KnobSpec;
use raster::Camera;
use shade::Rig;
use std::collections::HashMap;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "-h" || a == "--help") {
        eprintln!("{USAGE}");
        return ExitCode::SUCCESS;
    }

    let mut opts: HashMap<String, String> = HashMap::new();
    let mut i = 0;
    while i < args.len() {
        let key = args[i].trim_start_matches("--").to_string();
        let Some(value) = args.get(i + 1) else {
            eprintln!("error: --{key} needs a value\n\n{USAGE}");
            return ExitCode::FAILURE;
        };
        opts.insert(key, value.clone());
        i += 2;
    }

    let get = |k: &str, d: f32| -> f32 {
        opts.get(k).map(|v| v.parse().unwrap_or(d)).unwrap_or(d)
    };
    let part = opts.get("part").cloned().unwrap_or_else(|| "knob_large".into());
    let out = opts.get("out").cloned().unwrap_or_else(|| "out.png".into());
    let size = get("size", 441.0) as u32;
    let ss = get("ss", 3.0) as u32;
    let frames = get("frames", 1.0) as u32;
    let angle = get("angle", 0.0);
    // Frames cover an arc, not a full turn: a knob only sweeps 250 degrees, so
    // rendering the other 110 wastes a sixth of the strip.
    let sweep = get("sweep", 360.0);
    let ao_samples = get("ao-samples", 64.0) as u32;
    let margin = get("margin", 1.015);

    let spec = KnobSpec {
        flute_count: get("flutes", 13.0) as u32,
        flute_depth: get("flute-depth", 0.130),
        flute_sharpness: get("sharpness", 0.62),
        segments: get("segments", 384.0) as u32,
        pointer_deg: get("pointer", 90.0),
        ..Default::default()
    };

    let mut geometry = match part.as_str() {
        "knob_large" => parts::pultec_knob_large(&spec),
        "knob_pointer" => parts::pultec_knob_pointer(&KnobSpec { radius: 14.0, ..spec }),
        "toggle_up" => parts::pultec_toggle(true, spec.segments),
        "toggle_down" => parts::pultec_toggle(false, spec.segments),
        "lamp" => parts::pultec_lamp(spec.segments),
        "knob_small" => parts::pultec_knob_small(&KnobSpec { radius: 11.0, ..spec }),
        "comp76_knob" => parts::comp76_knob(
            &KnobSpec {
                flute_count: get("flutes", 12.0) as u32,
                flute_depth: get("flute-depth", 0.062),
                ..spec
            },
            &parts::Comp76Knob::LARGE,
        ),
        "comp76_knob_small" => parts::comp76_knob(
            &KnobSpec {
                flute_count: get("flutes", 10.0) as u32,
                flute_depth: get("flute-depth", 0.062),
                radius: 15.0,
                ..spec
            },
            &parts::Comp76Knob::SMALL,
        ),
        "glb" => {
            let path = opts.get("glb").cloned().unwrap_or_default();
            // --map name=material,name=material, so an export's own materials
            // can be replaced by ones tuned against the reference.
            let overrides: Vec<(String, crate::mesh::Material)> = opts
                .get("map")
                .map(|spec| {
                    spec.split(',')
                        .filter_map(|pair| {
                            let (name, mat) = pair.split_once('=')?;
                            Some((name.trim().to_string(), parts::by_name(mat.trim())?))
                        })
                        .collect()
                })
                .unwrap_or_default();
            let hide: Vec<String> = opts
                .get("hide")
                .map(|v| v.split(',').map(|n| n.trim().to_string()).collect())
                .unwrap_or_default();
            match gltf::load(&path, get("scale", 10.0), parts::BAKELITE, &overrides, &hide) {
                Ok(mut m) => {
                    // Exports vary in which axis the reference face points down.
                    let tilt = get("rotate-x", 0.0);
                    if tilt != 0.0 {
                        m.rotate_x_range(0, tilt.to_radians());
                        let low = m.verts.iter().map(|v| v.pos.z).fold(f32::MAX, f32::min);
                        m.translate_range(0, crate::vec::v3(0.0, 0.0, -low));
                    }
                    m
                }
                Err(e) => {
                    eprintln!("error: {e}");
                    return ExitCode::FAILURE;
                }
            }
        }
        other => {
            eprintln!("error: unknown part '{other}'\n\n{USAGE}");
            return ExitCode::FAILURE;
        }
    };

    eprintln!(
        "{part}: {} verts, {} tris, baking AO with {ao_samples} rays/vertex",
        geometry.verts.len(),
        geometry.tris.len()
    );
    let radius = geometry.radius_xy();
    ao::bake(&mut geometry, ao_samples, radius * 0.75);

    let rig = Rig::panel();
    let half_extent = radius * margin;
    let cam = Camera { half_extent, centre: (0.0, 0.0) };
    // The widget has to know how much of the frame the control body fills, so
    // report it rather than leaving the caller to guess a scale multiplier.
    let nominal = get("nominal", radius);
    eprintln!(
        "  extent {half_extent:.3} mm, nominal radius {nominal:.3} mm, span {:.4}",
        nominal / half_extent
    );

    let mut strip: Vec<u8> = Vec::with_capacity((size * size * 4 * frames) as usize);
    let mut frame_size = (size, size);
    for f in 0..frames {
        // Both ends of the arc are rendered, so the last frame is the control
        // at its maximum rather than one step short of it.
        let a = if frames == 1 {
            angle
        } else {
            angle + sweep * f as f32 / (frames - 1) as f32
        };
        let fb = raster::render(&geometry, &rig, &cam, size, ss, a.to_radians());
        frame_size = (fb.width, fb.height);
        strip.extend_from_slice(&fb.to_rgba());
        if frames > 1 {
            eprint!("\r  frame {}/{frames}", f + 1);
        }
    }
    if frames > 1 {
        eprintln!();
    }

    let img = image::RgbaImage::from_raw(frame_size.0, frame_size.1 * frames, strip)
        .expect("frame buffer size matches the image");
    if let Err(e) = img.save(&out) {
        eprintln!("error: could not write {out}: {e}");
        return ExitCode::FAILURE;
    }
    eprintln!("wrote {out} ({}x{})", frame_size.0, frame_size.1 * frames);
    ExitCode::SUCCESS
}

const USAGE: &str = "\
assetgen -- render panel controls from parametric geometry

  --part NAME        glb (with --glb PATH and --scale) | knob_large | knob_small | knob_pointer
                     toggle_up | toggle_down | lamp | comp76_knob
                     comp76_knob_small
  --out PATH         output PNG; a filmstrip when --frames > 1
  --size N           pixels per frame (default 441)
  --ss N             supersampling factor (default 3)
  --frames N         frames covering one full turn (default 1)
  --angle DEG        rotation, or the first frame's rotation (default 0)
  --sweep DEG        arc the frames cover, signed (default 360)
  --pointer DEG      where the pointer sits on the knob (default 90)
  --flutes N         scallops around the collar (default 13)
  --flute-depth F    scallop depth as a fraction of radius (default 0.130)
  --sharpness F      below 1 broadens the scoops (default 0.62)
  --segments N       lathe segments (default 384)
  --ao-samples N     occlusion rays per vertex (default 64)
  --margin F         framing slack around the part (default 1.015)
  --nominal MM       control body radius, for the reported span";
