//! Physically-based shading. Anisotropic GGX, because the circular brushing on
//! a machined face is what produces the radial sunburst, and an isotropic lobe
//! renders the same disc as flat grey plastic.

use crate::mesh::Material;
use crate::vec::{v3, Vec3};
use std::f32::consts::PI;

#[derive(Clone, Copy)]
pub struct Light {
    /// Direction from the surface towards the light, world space.
    pub dir: Vec3,
    pub colour: Vec3,
    /// Wrap-around, 0 for a point source. A large softbox keeps lighting a
    /// surface past the terminator, which is what gives a nearly flat crown a
    /// broad tonal sweep instead of one constant ambient value.
    pub wrap: f32,
    /// Whether this source contributes a specular highlight.
    pub specular: bool,
}

pub struct Rig {
    pub lights: Vec<Light>,
    pub sky: Vec3,
    pub ground: Vec3,
    /// A large bright panel in the environment. A polished flat face reflects
    /// the room, not the key light, so without something broad to mirror it
    /// renders as dark grey no matter how glossy it is.
    pub softbox_dir: Vec3,
    pub softbox_colour: Vec3,
    /// Higher values shrink the reflected panel.
    pub softbox_tightness: f32,
    /// The dome a reflection sees. Kept separate from the diffuse ambient
    /// because a real room has a lit floor and lit walls: chrome pointing
    /// sideways or down must not go black, while matte black plastic still
    /// needs a dark ambient to stay black.
    pub env_ground: Vec3,
    pub env_sky: Vec3,
    /// A bright band around the horizon of the reflection dome. Polished metal
    /// reads as metal largely because it mirrors the line where a room's walls
    /// meet its lighting; a smooth vertical gradient alone renders chrome as
    /// light grey plastic.
    pub horizon_colour: Vec3,
    pub horizon_width: f32,
    /// How far above the horizon the band sits, as the sine of its elevation.
    /// A room's bright band is where the walls meet the lighting, which is
    /// above eye level; leaving it at zero puts it exactly where a flat face
    /// on a vertical panel is looking, so every cap mirrors it head on and
    /// washes out to an even disc.
    pub horizon_height: f32,
    /// Gain on the reflected environment. A metal takes essentially all its
    /// light from reflection, so the simple two-colour dome that is plenty for
    /// a dielectric leaves aluminium looking like dark pewter.
    pub reflection_gain: f32,
    pub exposure: f32,
}

impl Rig {
    /// The panel lighting: a key from the upper left, a soft cool fill from the
    /// lower right, and a dim rim to separate the part from the faceplate.
    ///
    /// The key is kept well off the view axis. A fluted skirt is a ring of
    /// facets whose normals sweep through every azimuth, so a key close to the
    /// camera finds a facet angled at it all the way round and the knob glints
    /// as hard at four o'clock as at ten, reading as lit from nowhere in
    /// particular. Dropping the key towards the panel breaks the ring: only
    /// the flutes actually turned towards it catch a highlight.
    pub fn panel() -> Self {
        Self {
            lights: vec![
                // Key: the small bright source that makes the specular.
                Light {
                    dir: v3(-0.55, 0.68, 0.38).normalise(),
                    colour: Vec3::splat(4.30),
                    wrap: 0.0,
                    specular: true,
                },
                // Softbox: broad and wrapped, carries the tonal gradient.
                Light {
                    dir: v3(-0.52, 0.44, 0.73).normalise(),
                    colour: Vec3::splat(0.80),
                    wrap: 0.85,
                    specular: true,
                },
                // Fill: lifts the shadow side only. A fill with a specular
                // lobe puts a hard glint at four o'clock, where the rig has
                // no source, and it reads as light coming from below right.
                Light {
                    dir: v3(0.58, -0.42, 0.48).normalise(),
                    colour: v3(0.24, 0.27, 0.33),
                    wrap: 0.35,
                    specular: false,
                },
                Light {
                    dir: v3(0.10, 0.88, 0.12).normalise(),
                    colour: v3(0.19, 0.19, 0.22),
                    wrap: 0.2,
                    specular: false,
                },
            ],
            sky: v3(0.062, 0.067, 0.080),
            ground: v3(0.008, 0.008, 0.010),
            softbox_dir: v3(-0.46, 0.47, 0.75).normalise(),
            softbox_colour: v3(1.05, 1.06, 1.12),
            softbox_tightness: 3.2,
            env_ground: v3(0.055, 0.058, 0.068),
            env_sky: v3(0.395, 0.410, 0.450),
            horizon_colour: v3(0.900, 0.930, 1.000),
            horizon_width: 0.14,
            horizon_height: 0.34,
            reflection_gain: 1.56,
            exposure: 1.0,
        }
    }
}


/// Deterministic value noise, so a rebuild is byte-identical.
fn hash3(x: i32, y: i32, z: i32) -> f32 {
    let mut h = (x as u32).wrapping_mul(0x8da6_b343)
        ^ (y as u32).wrapping_mul(0xd816_3841_u32)
        ^ (z as u32).wrapping_mul(0xcb1a_b31f);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2c1b_3c6d);
    h ^= h >> 12;
    h = h.wrapping_mul(0x2974_5c85);
    h ^= h >> 15;
    (h & 0xff_ffff) as f32 / 0xff_ffff as f32 - 0.5
}

fn value_noise(p: Vec3) -> f32 {
    let (xi, yi, zi) = (p.x.floor(), p.y.floor(), p.z.floor());
    let (fx, fy, fz) = (p.x - xi, p.y - yi, p.z - zi);
    // Smoothstep the interpolation so the lattice does not show through.
    let s = |t: f32| t * t * (3.0 - 2.0 * t);
    let (u, v, w) = (s(fx), s(fy), s(fz));
    let (xi, yi, zi) = (xi as i32, yi as i32, zi as i32);
    let mut acc = 0.0;
    for dz in 0..2 {
        for dy in 0..2 {
            for dx in 0..2 {
                let wx = if dx == 0 { 1.0 - u } else { u };
                let wy = if dy == 0 { 1.0 - v } else { v };
                let wz = if dz == 0 { 1.0 - w } else { w };
                acc += hash3(xi + dx, yi + dy, zi + dz) * wx * wy * wz;
            }
        }
    }
    acc
}

fn fbm(p: Vec3) -> f32 {
    let mut acc = 0.0;
    let mut amp = 1.0;
    let mut freq = 1.0;
    for _ in 0..3 {
        acc += value_noise(p * freq) * amp;
        freq *= 2.17;
        amp *= 0.5;
    }
    acc
}

/// Concentric turning marks: noise that varies fast with radius and slowly
/// around the axis, which is what a part spun against a tool actually leaves.
/// Isotropic noise on a machined cap reads as sandblasting instead.
fn brush(n: Vec3, pos: Vec3, amount: f32, scale: f32) -> Vec3 {
    if amount <= 0.0 {
        return n;
    }
    let radius = pos.x.hypot(pos.y);
    if radius < 1e-4 {
        return n;
    }
    let theta = pos.y.atan2(pos.x);
    let p = v3(radius * scale, theta * 2.6, 0.0);
    let e = 0.35;
    let d_radial = fbm(p + v3(e, 0.0, 0.0)) - fbm(p - v3(e, 0.0, 0.0));
    let d_around = fbm(p + v3(0.0, e, 0.0)) - fbm(p - v3(0.0, e, 0.0));
    let (sn, cs) = theta.sin_cos();
    // Grooves run around the axis, so the radial gradient dominates.
    let tilt = v3(cs, sn, 0.0) * d_radial + v3(-sn, cs, 0.0) * (d_around * 0.18);
    let tangential = tilt - n * n.dot(tilt);
    (n - tangential * amount).normalise()
}

/// Roughen the normal with fine noise. Moulded phenolic is never optically
/// smooth, and a perfectly clean surface is most of what makes a render read
/// as a render.
fn perturb(n: Vec3, pos: Vec3, amount: f32, scale: f32) -> Vec3 {
    if amount <= 0.0 {
        return n;
    }
    let p = pos * scale;
    let e = 0.35;
    let g = v3(
        fbm(p + v3(e, 0.0, 0.0)) - fbm(p - v3(e, 0.0, 0.0)),
        fbm(p + v3(0.0, e, 0.0)) - fbm(p - v3(0.0, e, 0.0)),
        fbm(p + v3(0.0, 0.0, e)) - fbm(p - v3(0.0, 0.0, e)),
    );
    // Only the part of the gradient along the surface tilts the normal.
    let tangential = g - n * n.dot(g);
    (n - tangential * amount).normalise()
}

fn f_schlick(f0: Vec3, cos: f32) -> Vec3 {
    let f = (1.0 - cos).clamp(0.0, 1.0).powi(5);
    f0 + (Vec3::splat(1.0) - f0) * f
}

/// Anisotropic GGX normal distribution.
fn d_ggx_aniso(n_h: f32, x_h: f32, y_h: f32, ax: f32, ay: f32) -> f32 {
    let d = (x_h / ax).powi(2) + (y_h / ay).powi(2) + n_h * n_h;
    1.0 / (PI * ax * ay * d * d).max(1e-8)
}

fn g_smith(n_v: f32, n_l: f32, rough: f32) -> f32 {
    let k = (rough + 1.0).powi(2) / 8.0;
    let gv = n_v / (n_v * (1.0 - k) + k);
    let gl = n_l / (n_l * (1.0 - k) + k);
    gv * gl
}

/// Shade one point. `pos` drives the brushing direction, which runs around the
/// axis of the part rather than across it.
pub fn shade(rig: &Rig, mat: &Material, pos: Vec3, normal: Vec3, view: Vec3, ao: f32) -> Vec3 {
    let base_n = normal.normalise();
    let n = if mat.aniso > 0.0 {
        brush(base_n, pos, mat.texture, mat.texture_scale)
    } else {
        perturb(base_n, pos, mat.texture, mat.texture_scale)
    };
    let v = view.normalise();
    let n_v = n.dot(v).max(1e-4);

    // Circumferential tangent, projected into the surface.
    let circ = v3(-pos.y, pos.x, 0.0);
    let x = if circ.length() > 1e-5 {
        (circ - n * n.dot(circ)).normalise()
    } else {
        n.any_perpendicular()
    };
    let y = n.cross(x);

    let rough = mat.roughness.clamp(0.03, 1.0);
    let a = rough * rough;
    // Stretch the lobe along the brushing so the highlight streaks outward.
    let aspect = (1.0 - mat.aniso * 0.9).max(0.05).sqrt();
    let (ax, ay) = (a / aspect, a * aspect);

    let f0 = Vec3::splat(0.04).lerp(mat.base, mat.metallic);
    let diffuse_albedo = mat.base * (1.0 - mat.metallic);

    let mut out = Vec3::ZERO;
    for light in &rig.lights {
        let l = light.dir;
        let n_l = n.dot(l);
        // Wrapped diffuse keeps a broad source alive past the terminator.
        let diff = ((n_l + light.wrap) / (1.0 + light.wrap)).max(0.0);
        if diff <= 0.0 {
            continue;
        }
        let h = (l + v).normalise();
        let f = f_schlick(f0, v.dot(h).max(0.0));
        let spec = if light.specular && n_l > 0.0 {
            let d = d_ggx_aniso(n.dot(h).max(0.0), x.dot(h), y.dot(h), ax, ay);
            let g = g_smith(n_v, n_l.max(1e-4), rough);
            f * (d * g / (4.0 * n_v * n_l.max(1e-4))) * n_l
        } else {
            Vec3::ZERO
        };
        let kd = (Vec3::splat(1.0) - f) * (1.0 / PI);
        // Crevices lose direct light too, not only the ambient dome.
        let shadowed = 0.14 + 0.86 * ao;
        out += (diffuse_albedo.mul(kd) * diff + spec).mul(light.colour) * shadowed;
    }

    // Hemispherical ambient, occluded. The specular half uses the reflected
    // direction so metal picks up the environment instead of going black.
    let amb = rig.ground.lerp(rig.sky, 0.5 + 0.5 * n.y);
    out += diffuse_albedo.mul(amb) * ao;
    let r = (n * (2.0 * n.dot(v)) - v).normalise();
    let sheen = r.dot(rig.softbox_dir).max(0.0).powf(rig.softbox_tightness);
    // Rougher surfaces smear the reflected panel out rather than mirroring it.
    let horizon = (-((r.y - rig.horizon_height) / rig.horizon_width).powi(2)).exp();
    let env = rig.env_ground.lerp(rig.env_sky, 0.5 + 0.5 * r.y)
        + rig.horizon_colour * (horizon * (1.0 - rough).powi(2))
        + rig.softbox_colour * (sheen * (1.0 - rough).powi(2));
    let f_env = f_schlick(f0, n_v);
    out += env.mul(f_env) * (ao * (1.0 - rough * 0.6) * rig.reflection_gain);

    (out + mat.emissive) * rig.exposure
}

/// Filmic curve, then sRGB transfer. Keeps the specular from clipping to a
/// flat white disc the way a plain clamp does.
pub fn tonemap_srgb(c: Vec3) -> [u8; 3] {
    let f = |x: f32| {
        let x = (x * 0.6).max(0.0);
        let m = (x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14);
        let m = m.clamp(0.0, 1.0);
        let s = if m <= 0.003_130_8 {
            m * 12.92
        } else {
            1.055 * m.powf(1.0 / 2.4) - 0.055
        };
        (s * 255.0 + 0.5) as u8
    };
    [f(c.x), f(c.y), f(c.z)]
}
