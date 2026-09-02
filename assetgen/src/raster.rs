//! Orthographic z-buffer rasteriser with supersampling.
//!
//! Orthographic rather than perspective on purpose: a control on a flat panel
//! is viewed straight on, and perspective would make the same knob look
//! different at the edges of the window than in the middle.

use crate::mesh::Mesh;
use crate::shade::{shade, tonemap_srgb, Rig};
use crate::vec::{v3, Vec3};

pub struct Camera {
    /// Half the width of the visible square, in world units.
    pub half_extent: f32,
    pub centre: (f32, f32),
}

pub struct Framebuffer {
    pub width: u32,
    pub height: u32,
    /// Straight (non-premultiplied) RGBA.
    pub pixels: Vec<u8>,
}

impl Framebuffer {
    pub fn to_rgba(&self) -> Vec<u8> {
        self.pixels.clone()
    }
}

/// Render `mesh` rotated about its Z axis by `radians`, with the lighting rig
/// fixed in panel space so the highlight stays put as the control turns.
pub fn render(
    mesh: &Mesh,
    rig: &Rig,
    cam: &Camera,
    size: u32,
    ss: u32,
    radians: f32,
) -> Framebuffer {
    let n = (size * ss) as usize;
    let mut depth = vec![f32::MIN; n * n];
    let mut colour = vec![Vec3::ZERO; n * n];
    let mut cover = vec![0.0f32; n * n];

    // Rotate once up front rather than per pixel.
    let verts: Vec<(Vec3, Vec3, f32, u16)> = mesh
        .verts
        .iter()
        .map(|v| (v.pos.rotate_z(radians), v.normal.rotate_z(radians), v.ao, v.mat))
        .collect();

    let scale = n as f32 / (2.0 * cam.half_extent);
    let to_screen = |p: Vec3| -> (f32, f32) {
        (
            (p.x - cam.centre.0) * scale + n as f32 / 2.0,
            n as f32 / 2.0 - (p.y - cam.centre.1) * scale,
        )
    };

    let view = v3(0.0, 0.0, 1.0);

    for tri in &mesh.tris {
        let i = [tri[0] as usize, tri[1] as usize, tri[2] as usize];
        let p: Vec<Vec3> = i.iter().map(|&k| verts[k].0).collect();
        let geo = (p[1] - p[0]).cross(p[2] - p[0]);
        if geo.z <= 0.0 {
            continue; // back face
        }
        let s: Vec<(f32, f32)> = p.iter().map(|&q| to_screen(q)).collect();

        let minx = s.iter().map(|q| q.0).fold(f32::MAX, f32::min).floor().max(0.0) as usize;
        let maxx = (s.iter().map(|q| q.0).fold(f32::MIN, f32::max).ceil() as isize)
            .clamp(0, n as isize - 1) as usize;
        let miny = s.iter().map(|q| q.1).fold(f32::MAX, f32::min).floor().max(0.0) as usize;
        let maxy = (s.iter().map(|q| q.1).fold(f32::MIN, f32::max).ceil() as isize)
            .clamp(0, n as isize - 1) as usize;
        if minx > maxx || miny > maxy {
            continue;
        }

        let area = (s[1].0 - s[0].0) * (s[2].1 - s[0].1) - (s[2].0 - s[0].0) * (s[1].1 - s[0].1);
        if area.abs() < 1e-9 {
            continue;
        }
        let inv_area = 1.0 / area;

        for py in miny..=maxy {
            for px in minx..=maxx {
                let (fx, fy) = (px as f32 + 0.5, py as f32 + 0.5);
                let w0 = ((s[1].0 - fx) * (s[2].1 - fy) - (s[2].0 - fx) * (s[1].1 - fy)) * inv_area;
                let w1 = ((s[2].0 - fx) * (s[0].1 - fy) - (s[0].0 - fx) * (s[2].1 - fy)) * inv_area;
                let w2 = 1.0 - w0 - w1;
                if w0 < 0.0 || w1 < 0.0 || w2 < 0.0 {
                    continue;
                }
                let z = p[0].z * w0 + p[1].z * w1 + p[2].z * w2;
                let idx = py * n + px;
                if z <= depth[idx] {
                    continue;
                }
                depth[idx] = z;

                let pos = p[0] * w0 + p[1] * w1 + p[2] * w2;
                let nrm = (verts[i[0]].1 * w0 + verts[i[1]].1 * w1 + verts[i[2]].1 * w2).normalise();
                let ao = verts[i[0]].2 * w0 + verts[i[1]].2 * w1 + verts[i[2]].2 * w2;
                let mat = &mesh.mats[verts[i[0]].3 as usize];
                colour[idx] = shade(rig, mat, pos, nrm, view, ao);
                cover[idx] = 1.0;
            }
        }
    }

    // Box downsample. Averaging linear radiance before the transfer curve is
    // what keeps the fluted edges from crawling as the knob turns.
    let mut pixels = vec![0u8; (size * size * 4) as usize];
    let f = ss as usize;
    for y in 0..size as usize {
        for x in 0..size as usize {
            let (mut acc, mut a) = (Vec3::ZERO, 0.0f32);
            for sy in 0..f {
                for sx in 0..f {
                    let idx = (y * f + sy) * n + (x * f + sx);
                    acc += colour[idx];
                    a += cover[idx];
                }
            }
            let count = (f * f) as f32;
            a /= count;
            let o = (y * size as usize + x) * 4;
            if a > 0.0 {
                // Un-premultiply so the sprite composites correctly at any scale.
                let rgb = tonemap_srgb(acc / (a * count));
                pixels[o..o + 3].copy_from_slice(&rgb);
            }
            pixels[o + 3] = (a.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
        }
    }

    Framebuffer { width: size, height: size, pixels }
}
