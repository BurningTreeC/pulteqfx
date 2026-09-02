//! Parametric geometry. Every control on the panel is a surface of revolution
//! whose radius may be modulated with angle, which covers plain cylinders,
//! domes, chamfers and fluted knob skirts with one generator.

use crate::vec::{v3, Vec3};
use std::f32::consts::PI;

#[derive(Clone, Copy, Debug)]
pub struct Material {
    pub base: Vec3,
    pub roughness: f32,
    pub metallic: f32,
    /// Strength of circumferential brushing, 0 for a plain finish. Anisotropy
    /// is what turns a flat metal disc into the radial sunburst you see on a
    /// machined face; the Pultec knobs are plain, the 1176 metal caps are not.
    pub aniso: f32,
    /// Strength of the fine surface roughening, in normal-tilt units.
    pub texture: f32,
    /// Noise frequency, in cycles per world unit.
    pub texture_scale: f32,
    /// Light the surface gives off itself. A powered jewel is lit from behind,
    /// which no amount of reflected key will imitate.
    pub emissive: Vec3,
}

impl Material {
    pub const fn new(base: Vec3, roughness: f32, metallic: f32) -> Self {
        Self {
            base,
            roughness,
            metallic,
            aniso: 0.0,
            texture: 0.0,
            texture_scale: 1.0,
            emissive: Vec3::ZERO,
        }
    }
    /// Circumferential brushing. The radial sunburst on a machined cap is an
    /// anisotropic highlight, not a texture, so it has to come from the lobe.
    pub const fn brushed(mut self, aniso: f32) -> Self {
        self.aniso = aniso;
        self
    }
    pub const fn glowing(mut self, e: Vec3) -> Self {
        self.emissive = e;
        self
    }
    pub const fn textured(mut self, amount: f32, scale: f32) -> Self {
        self.texture = amount;
        self.texture_scale = scale;
        self
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Vertex {
    pub pos: Vec3,
    pub normal: Vec3,
    pub ao: f32,
    pub mat: u16,
}

#[derive(Clone, Default)]
pub struct Mesh {
    pub verts: Vec<Vertex>,
    pub tris: Vec<[u32; 3]>,
    pub mats: Vec<Material>,
}

impl Mesh {
    pub fn material(&mut self, m: Material) -> u16 {
        self.mats.push(m);
        (self.mats.len() - 1) as u16
    }

    /// Tilt everything added since `from`, for a switch bat leaning out of
    /// the panel plane.
    pub fn rotate_x_range(&mut self, from: usize, radians: f32) {
        let (s, c) = radians.sin_cos();
        let f = |p: Vec3| v3(p.x, p.y * c - p.z * s, p.y * s + p.z * c);
        for v in &mut self.verts[from..] {
            v.pos = f(v.pos);
            v.normal = f(v.normal);
        }
    }

    pub fn translate_range(&mut self, from: usize, d: Vec3) {
        for v in &mut self.verts[from..] {
            v.pos += d;
        }
    }

    pub fn face_normal(&self, t: [u32; 3]) -> Vec3 {
        let a = self.verts[t[0] as usize].pos;
        let b = self.verts[t[1] as usize].pos;
        let c = self.verts[t[2] as usize].pos;
        (b - a).cross(c - a).normalise()
    }

    /// Bounding radius in the XY plane, used to frame the camera.
    pub fn radius_xy(&self) -> f32 {
        self.verts
            .iter()
            .map(|v| v.pos.x.hypot(v.pos.y))
            .fold(0.0, f32::max)
    }

}

/// How the radius is scalloped with angle.
#[derive(Clone, Copy, Debug)]
pub struct Flute {
    pub count: u32,
    pub depth: f32,
    /// Below 1 the scoops are broad and the ridges between them narrow, which
    /// is how a moulded knob skirt actually looks; above 1 inverts that.
    pub sharpness: f32,
    /// Rotational offset so the flutes can be aligned to the pointer.
    pub phase: f32,
    /// When three or more, the ring is a regular polygon with this many flats
    /// and `Ring::r` is the distance to a face, not to a corner. Flutes still
    /// apply on top, which is how a knurled hex bushing is built.
    pub sides: u32,
}

impl Default for Flute {
    fn default() -> Self {
        Self { count: 13, depth: 0.10, sharpness: 0.62, phase: 0.0, sides: 0 }
    }
}

impl Flute {
    /// 0 on a ridge, 1 at the bottom of a scoop.
    fn scoop(&self, theta: f32) -> f32 {
        let c = ((theta - self.phase) * self.count as f32).cos();
        (0.5 - 0.5 * c).max(0.0).powf(self.sharpness)
    }
}

/// One circle of the lathe profile.
#[derive(Clone, Copy, Debug)]
pub struct Ring {
    pub r: f32,
    pub z: f32,
    /// How much of the flute depth applies here, letting the scallops fade in
    /// and out along the profile instead of stopping at a hard line.
    pub flute: f32,
    pub mat: u16,
    /// Break the surface here so the edge renders crisp instead of rounded.
    pub sharp: bool,
}

impl Ring {
    pub fn new(r: f32, z: f32, mat: u16) -> Self {
        Self { r, z, flute: 0.0, mat, sharp: false }
    }
    pub fn fluted(mut self, amount: f32) -> Self {
        self.flute = amount;
        self
    }
    pub fn sharp(mut self) -> Self {
        self.sharp = true;
        self
    }
}

fn radius_at(ring: &Ring, theta: f32, f: &Flute) -> f32 {
    let mut r = ring.r;
    if f.sides >= 3 {
        // Distance from the axis to the flat, swept round to the corner.
        let step = 2.0 * PI / f.sides as f32;
        let t = (theta - f.phase).rem_euclid(step) - step * 0.5;
        r /= t.cos();
    }
    r * (1.0 - f.depth * ring.flute * f.scoop(theta))
}

/// d(radius)/d(theta), by central difference so the flute shape stays free-form.
fn dr_dtheta(ring: &Ring, theta: f32, f: &Flute) -> f32 {
    const H: f32 = 1e-3;
    (radius_at(ring, theta + H, f) - radius_at(ring, theta - H, f)) / (2.0 * H)
}

/// Outward surface normal on the band between two rings, evaluated at `ring`.
fn band_normal(ring: &Ring, other: &Ring, theta: f32, f: &Flute, upward: bool) -> Vec3 {
    let (s, c) = theta.sin_cos();
    let r = radius_at(ring, theta, f);
    let dr = dr_dtheta(ring, theta, f);
    // Tangent around the ring.
    let t_theta = v3(dr * c - r * s, dr * s + r * c, 0.0);
    // Tangent along the profile, towards the other ring.
    let r_o = radius_at(other, theta, f);
    let t_u = v3((r_o - r) * c, (r_o - r) * s, other.z - ring.z);
    let n = t_theta.cross(t_u).normalise();
    // The profile is walked from the top outward and down, so flipping by the
    // direction of travel keeps every normal pointing away from the solid.
    if upward {
        -n
    } else {
        n
    }
}

/// Build a surface of revolution from a profile.
///
/// The profile is walked in order; a `sharp` ring duplicates its vertices so
/// the bands either side get their own normals, which is what keeps a chamfer
/// edge crisp while the dome next to it stays smooth.
pub fn revolve_into(mesh: &mut Mesh, profile: &[Ring], segments: u32, flute: &Flute) {
    let seg = segments as usize;
    // Everything this call adds, so normals are aligned over its own triangles
    // and not the whole accumulated mesh.
    let vert_start = mesh.verts.len();
    let tri_start = mesh.tris.len();

    // Emit one ring of vertices whose normals face along the profile towards
    // `other`, returning their indices.
    fn emit(
        mesh: &mut Mesh,
        ring: &Ring,
        other: &Ring,
        seg: usize,
        flute: &Flute,
        upward: bool,
    ) -> Vec<u32> {
        (0..seg)
            .map(|s| {
                let theta = s as f32 / seg as f32 * 2.0 * PI;
                let (sn, cs) = theta.sin_cos();
                let r = radius_at(ring, theta, flute);
                let id = mesh.verts.len() as u32;
                mesh.verts.push(Vertex {
                    pos: v3(r * cs, r * sn, ring.z),
                    normal: band_normal(ring, other, theta, flute, upward),
                    ao: 1.0,
                    mat: ring.mat,
                });
                id
            })
            .collect()
    }

    // `top[i]` is the ring used as the upper edge of band i, `bottom[i]` the
    // ring used as the lower edge of band i - 1. They are the same vertices
    // unless the ring is sharp.
    let mut top: Vec<Vec<u32>> = Vec::with_capacity(profile.len());
    let mut bottom: Vec<Vec<u32>> = Vec::with_capacity(profile.len());

    for (i, ring) in profile.iter().enumerate() {
        let back = i.checked_sub(1).map(|p| emit(mesh, ring, &profile[p], seg, flute, true));
        let fwd = profile
            .get(i + 1)
            .map(|n| emit(mesh, ring, n, seg, flute, false));

        match (back, fwd) {
            (Some(b), Some(f)) if !ring.sharp => {
                // Smooth join: average the two normals onto the forward ring
                // and let both bands share it.
                for s in 0..seg {
                    let nb = mesh.verts[b[s] as usize].normal;
                    let nf = mesh.verts[f[s] as usize].normal;
                    mesh.verts[f[s] as usize].normal = (nb + nf).normalise();
                }
                bottom.push(f.clone());
                top.push(f);
            }
            (Some(b), Some(f)) => {
                bottom.push(b);
                top.push(f);
            }
            (None, Some(f)) => {
                bottom.push(f.clone());
                top.push(f);
            }
            (Some(b), None) => {
                bottom.push(b.clone());
                top.push(b);
            }
            (None, None) => {
                bottom.push(Vec::new());
                top.push(Vec::new());
            }
        }
    }

    for i in 0..profile.len().saturating_sub(1) {
        let (a, b) = (&top[i], &bottom[i + 1]);
        let (ra, rb) = (profile[i].r, profile[i + 1].r);
        for s in 0..seg {
            let t = (s + 1) % seg;
            // Skip the degenerate half of a quad where the profile meets the axis.
            if ra > 1e-6 {
                mesh.tris.push([a[s], b[s], a[t]]);
            }
            if rb > 1e-6 {
                mesh.tris.push([a[t], b[s], b[t]]);
            }
        }
    }

    align_normals(mesh, vert_start, tri_start);
}

/// Derive vertex normals from the surrounding faces, over just the range a
/// builder added. Used where a shape is smooth everywhere and writing the
/// analytic normal would be more work than it is worth.
pub fn smooth_normals(mesh: &mut Mesh, vert_start: usize, tri_start: usize) {
    let mut acc = vec![Vec3::ZERO; mesh.verts.len() - vert_start];
    for t in &mesh.tris[tri_start..] {
        let n = mesh.face_normal(*t);
        for i in t {
            acc[*i as usize - vert_start] += n;
        }
    }
    for (v, a) in mesh.verts[vert_start..].iter_mut().zip(acc) {
        if a.length() > 1e-12 {
            v.normal = a.normalise();
        }
    }
}

/// The analytic normals are derived from the parameterisation, which can flip
/// sign where the profile doubles back. Align each to its triangle winding.
fn align_normals(mesh: &mut Mesh, vert_start: usize, tri_start: usize) {
    let mut acc = vec![Vec3::ZERO; mesh.verts.len() - vert_start];
    for t in &mesh.tris[tri_start..] {
        let n = mesh.face_normal(*t);
        for i in t {
            acc[*i as usize - vert_start] += n;
        }
    }
    for (v, a) in mesh.verts[vert_start..].iter_mut().zip(acc) {
        if v.normal.dot(a) < 0.0 {
            v.normal = -v.normal;
        }
    }
}
