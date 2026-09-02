//! The controls themselves. Dimensions are in millimetres and proportions were
//! measured off the reference photograph: the raised crown runs to 0.62 of the
//! radius, the recess to 0.75, and the fluted collar sits inside a plain outer
//! flange, which is why the silhouette is round while the collar reads as a
//! ring of arches.

use crate::mesh::{revolve_into, smooth_normals, Flute, Material, Mesh, Ring, Vertex};
use crate::vec::{v3, Vec3};
use std::f32::consts::PI;

pub const CROWN: Material = Material::new(v3(0.0205, 0.0205, 0.0235), 0.250, 0.0)
    .textured(0.055, 4.6);
pub const BAKELITE: Material = Material::new(v3(0.0235, 0.0235, 0.026), 0.345, 0.0).textured(0.115, 4.1);
/// Spun aluminium. Metallic, so its colour comes from the reflection rather
/// than a diffuse term, and heavily brushed around the axis.
///
/// Real aluminium reflects about 0.91, but the rig's environment is a two-tone
/// dome plus a softbox rather than a real room, so the shortfall is taken here
/// where it only touches the metal, rather than by pulling the whole rig down
/// and losing the Pultec parts' tuning. Calibrated against the 1176
/// photograph, whose cap sits at a median of 157.
pub const ALUMINIUM: Material = Material::new(v3(0.650, 0.659, 0.670), 0.302, 1.0)
    .brushed(0.88)
    .textured(0.085, 9.5);
/// The indicator rivet.
pub const BRASS: Material = Material::new(v3(0.812, 0.678, 0.412), 0.34, 1.0);
/// Nickel-plated hardware. Not a mirror: the reference bushings are satin.
pub const CHROME: Material = Material::new(v3(0.782, 0.771, 0.742), 0.215, 1.0)
    .textured(0.026, 12.0);
/// The lens is lit from behind when the unit is powered, which is why it needs
/// an emissive term rather than just a red albedo.
pub const JEWEL: Material = Material::new(v3(0.271, 0.0045, 0.0036), 0.070, 0.0)
    .glowing(v3(0.172, 0.0020, 0.0012));
pub const IVORY: Material = Material::new(v3(0.80, 0.725, 0.575), 0.44, 0.0).textured(0.06, 5.0);

/// An ellipsoid-swept segment along the X axis: a bar with rounded ends whose
/// cross-section runs from `ry0` x `rz0` to `ry1` x `rz1`. Flattening it
/// (rz << ry) gives the pointer inlay a broad top face that catches light,
/// where a round tube only ever shows a thin bright sliver; tapering it makes
/// the boss the teardrop it is on the real knob rather than a uniform stick.
#[allow(clippy::too_many_arguments)]
fn bar_into(
    mesh: &mut Mesh,
    x0: f32,
    x1: f32,
    ry0: f32,
    ry1: f32,
    rz0: f32,
    rz1: f32,
    around: u32,
    mat: u16,
    offset: Vec3,
    spin: f32,
) {
    let mut rows: Vec<Vec<u32>> = Vec::new();
    const CAP: u32 = 6;
    // (centre x, cross-section scale, x-normal weight, ry, rz)
    let mut spine: Vec<(f32, f32, f32, f32, f32)> = Vec::new();
    for i in 0..=CAP {
        let a = i as f32 / CAP as f32 * PI / 2.0;
        spine.push((x0 - ry0 * a.cos(), a.sin(), -a.cos(), ry0, rz0));
    }
    const BODY: u32 = 8;
    for i in 1..BODY {
        let t = i as f32 / BODY as f32;
        spine.push((x0 + (x1 - x0) * t, 1.0, 0.0, ry0 + (ry1 - ry0) * t, rz0 + (rz1 - rz0) * t));
    }
    for i in 0..=CAP {
        let a = (CAP - i) as f32 / CAP as f32 * PI / 2.0;
        spine.push((x1 + ry1 * a.cos(), a.sin(), a.cos(), ry1, rz1));
    }
    // The taper tilts the surface along x; without this the boss lights as if
    // it were a straight tube.
    let slope_y = (ry1 - ry0) / (x1 - x0);
    let slope_z = (rz1 - rz0) / (x1 - x0);
    for &(cx, scale, nx, ry, rz) in &spine {
        let row: Vec<u32> = (0..around)
            .map(|s| {
                let phi = s as f32 / around as f32 * 2.0 * PI;
                let (sn, cs) = phi.sin_cos();
                let id = mesh.verts.len() as u32;
                let taper = if nx == 0.0 {
                    -(slope_y * cs.abs() + slope_z * sn.abs())
                } else {
                    0.0
                };
                let n = v3(nx / ry + taper, scale * cs / ry, scale * sn / rz).normalise();
                mesh.verts.push(Vertex {
                    pos: (v3(cx, ry * scale * cs, rz * scale * sn) + offset).rotate_z(spin),
                    normal: n.rotate_z(spin),
                    ao: 1.0,
                    mat,
                });
                id
            })
            .collect();
        rows.push(row);
    }
    for i in 0..rows.len() - 1 {
        for s in 0..around as usize {
            let t = (s + 1) % around as usize;
            // Wound so the outward face is front-facing to a camera on +Z.
            mesh.tris.push([rows[i][s], rows[i][t], rows[i + 1][s]]);
            mesh.tris.push([rows[i][t], rows[i + 1][t], rows[i + 1][s]]);
        }
    }
}

/// A blade swept along X through a list of stations, each giving the
/// half-width, half-height and centre height at that point. The centre height
/// lets an inlay ride a sloping surface instead of being a flat plate buried
/// at one end and floating at the other. The cross-section is a superellipse,
/// so `fullness` above 2 gives the squared-off shoulders a moulded spade
/// pointer has rather than the soft oval an ellipse would.
#[allow(clippy::too_many_arguments)]
fn blade_into(
    mesh: &mut Mesh,
    stations: &[(f32, f32, f32, f32)],
    around: u32,
    fullness: f32,
    mat: u16,
    offset: Vec3,
    spin: f32,
) {
    let vert_start = mesh.verts.len();
    let tri_start = mesh.tris.len();
    let e = 2.0 / fullness;
    let mut rows: Vec<Vec<u32>> = Vec::new();
    for &(x, hw, h, cz) in stations {
        let row: Vec<u32> = (0..around)
            .map(|s| {
                let t = s as f32 / around as f32 * 2.0 * PI;
                let (sn, cs) = t.sin_cos();
                let y = hw * cs.abs().powf(e) * cs.signum();
                let z = h * sn.abs().powf(e) * sn.signum();
                let id = mesh.verts.len() as u32;
                mesh.verts.push(Vertex {
                    pos: (v3(x, y, z + cz) + offset).rotate_z(spin),
                    // Filled in by the smoothing pass below.
                    normal: v3(0.0, 0.0, 1.0),
                    ao: 1.0,
                    mat,
                });
                id
            })
            .collect();
        rows.push(row);
    }
    for i in 0..rows.len() - 1 {
        for s in 0..around as usize {
            let t = (s + 1) % around as usize;
            mesh.tris.push([rows[i][s], rows[i][t], rows[i + 1][s]]);
            mesh.tris.push([rows[i][t], rows[i + 1][t], rows[i + 1][s]]);
        }
    }
    smooth_normals(mesh, vert_start, tri_start);
}

pub struct KnobSpec {
    pub radius: f32,
    pub height: f32,
    pub flute_count: u32,
    pub flute_depth: f32,
    pub flute_sharpness: f32,
    pub segments: u32,
    /// Where the pointer sits, measured anticlockwise from due east.
    pub pointer_deg: f32,
}

impl Default for KnobSpec {
    fn default() -> Self {
        Self {
            radius: 21.0,
            height: 13.0,
            flute_count: 13,
            flute_depth: 0.130,
            flute_sharpness: 0.62,
            segments: 384,
            pointer_deg: 90.0,
        }
    }
}

/// The large Pultec knob: domed crown, recessed step, fluted collar, plain
/// flange, and a raised pointer with an ivory inlay.
pub fn pultec_knob_large(spec: &KnobSpec) -> Mesh {
    let r = spec.radius;
    let h = spec.height;
    let mut mesh = Mesh::default();
    let crown = mesh.material(CROWN);
    let black = mesh.material(BAKELITE);

    // (radius fraction, height, flute amount, sharp edge)
    // (radius fraction, height fraction, flute amount, sharp edge, glossy crown)
    let profile: &[(f32, f32, f32, bool, bool)] = &[
        // A shallow crown reflects the room only at its very rim, because a
        // dielectric returns 4% head-on. The dome has to actually curve for
        // the key to sweep across it the way it does on the real knob.
        (0.000, 1.000, 0.0, false, true),
        (0.300, 0.991, 0.0, false, true),
        (0.505, 0.968, 0.0, false, true),
        (0.615, 0.938, 0.0, false, true),
        (0.658, 0.922, 0.0, true, true),  // crown edge, catches the key
        (0.684, 0.898, 0.0, true, false), // chamfer down into the groove
        (0.706, 0.836, 0.0, true, false), // groove wall
        // The groove has to be wide enough to survive being drawn at 88px, or
        // it averages away to nothing and the crown loses its edge entirely.
        (0.748, 0.826, 0.0, false, false), // groove floor
        (0.792, 0.832, 0.0, false, false),
        (0.828, 0.866, 0.35, false, false), // collar rises
        (0.884, 0.880, 0.85, false, false), // collar crest
        (0.949, 0.864, 1.00, false, false),
        (0.986, 0.826, 1.00, true, false), // fluted collar edge
        (1.000, 0.818, 0.00, true, false), // plain flange top
        (1.000, 0.015, 0.00, true, false), // flange wall
        (0.930, 0.000, 0.00, false, false),
    ];

    let rings: Vec<Ring> = profile
        .iter()
        .map(|&(rf, zf, fl, sharp, glossy)| {
            let mat = if glossy { crown } else { black };
            let mut ring = Ring::new(rf * r, zf * h, mat).fluted(fl);
            if sharp {
                ring = ring.sharp();
            }
            ring
        })
        .collect();

    let flute = Flute {
        count: spec.flute_count,
        depth: spec.flute_depth,
        sharpness: spec.flute_sharpness,
        phase: 0.0,
        sides: 0,
    };
    revolve_into(&mut mesh, &rings, spec.segments, &flute);

    // Pointer: a raised boss running out across the crown, with a flat ivory
    // inlay let into its top face. The inlay is placed from the boss crown
    // height rather than a literal offset, so retuning the boss cannot bury it.
    let boss_z = 0.972 * h;
    let boss_ry = 0.118 * r;
    let boss_rz = 0.070 * r;
    let spin = spec.pointer_deg.to_radians();
    bar_into(
        &mut mesh,
        0.10 * r,
        0.86 * r,
        boss_ry * 0.72,
        boss_ry,
        boss_rz * 0.74,
        boss_rz,
        56,
        black,
        v3(0.0, 0.0, boss_z),
        spin,
    );

    let ivory = mesh.material(IVORY);
    let inlay_rz = 0.017 * r;
    // Sit the inlay's top face just proud of the boss crown.
    let inlay_z = (boss_z + boss_rz) - inlay_rz + 0.014 * r;
    bar_into(
        &mut mesh,
        0.31 * r,
        0.790 * r,
        0.030 * r,
        0.046 * r,
        inlay_rz,
        inlay_rz,
        40,
        ivory,
        v3(0.0, 0.0, inlay_z),
        spin,
    );

    mesh
}

/// The frequency-selector knob: a plain round skirt carrying a tapered spade
/// pointer that runs right across it, pointed at one end with a wide rounded
/// foot at the other.
///
/// Unlike the photographed sprite this is square and centred on the shaft, so
/// the pivot is simply the middle and rotation cannot make it wobble.
pub fn pultec_knob_pointer(spec: &KnobSpec) -> Mesh {
    let r = spec.radius;
    let mut mesh = Mesh::default();
    let gloss = mesh.material(CROWN);
    let matte = mesh.material(BAKELITE);

    // (radius fraction, height mm, sharp edge)
    let disc: &[(f32, f32, bool)] = &[
        (0.000, 0.307 * r, false),
        (0.500, 0.304 * r, false),
        (0.780, 0.297 * r, false),
        (0.900, 0.287 * r, false),
        (0.955, 0.266 * r, true), // chamfer
        (1.000, 0.229 * r, true), // rim
        (1.000, 0.014 * r, true), // wall
        (0.935, 0.000, false),
    ];
    let rings: Vec<Ring> = disc
        .iter()
        .map(|&(rf, z, sharp)| {
            let ring = Ring::new(rf * r, z, matte);
            if sharp {
                ring.sharp()
            } else {
                ring
            }
        })
        .collect();
    // A plain skirt: no flutes on this one.
    revolve_into(&mut mesh, &rings, spec.segments, &Flute { depth: 0.0, ..Flute::default() });

    // (distance along the blade, half width, half height), all as fractions of
    // the skirt radius. The widest point is half the skirt radius across,
    // measured off the reference.
    // The foot is blunt and nearly full width right to its end; tapering it
    // symmetrically turns the spade into an almond that reads as a bullet.
    // The sides then run parallel to a shoulder before the point, which is
    // the bottle profile the reference actually has.
    let stations: &[(f32, f32, f32)] = &[
        (-1.085, 0.270, 0.028),
        (-1.070, 0.335, 0.050),
        (-1.030, 0.360, 0.066),
        (-0.950, 0.367, 0.077),
        (-0.400, 0.369, 0.085),
        (0.100, 0.367, 0.086),
        (0.330, 0.352, 0.085),
        (0.560, 0.291, 0.080),
        (0.760, 0.219, 0.072),
        (0.940, 0.148, 0.060),
        (1.090, 0.086, 0.045),
        (1.200, 0.032, 0.024),
        (1.250, 0.005, 0.006),
    ];
    let scaled: Vec<(f32, f32, f32, f32)> = stations
        .iter()
        .map(|&(x, w, h)| (x * r, w * r, h * r, 0.0))
        .collect();
    blade_into(
        &mut mesh,
        &scaled,
        64,
        2.4,
        gloss,
        v3(0.0, 0.0, 0.300 * r),
        spec.pointer_deg.to_radians(),
    );

    mesh
}

/// The 1176 knobs: a fluted black skirt with a spun aluminium cap recessed
/// under a proud black rim, and an indicator rivet. The two sizes are the same
/// casting scaled, differing in flute count, how much of the face the cap
/// takes, and whether the rivet is brass or ivory.
pub struct Comp76Knob {
    /// Where the aluminium cap ends, as a fraction of the radius.
    pub cap: f32,
    /// Where the indicator sits, as a fraction of the radius.
    pub dot_at: f32,
    pub dot_radius: f32,
    pub brass_dot: bool,
}

impl Comp76Knob {
    pub const LARGE: Self =
        Self { cap: 0.585, dot_at: 0.767, dot_radius: 0.076, brass_dot: true };
    pub const SMALL: Self =
        Self { cap: 0.565, dot_at: 0.735, dot_radius: 0.088, brass_dot: false };
}

pub fn comp76_knob(spec: &KnobSpec, style: &Comp76Knob) -> Mesh {
    let r = spec.radius;
    let h = spec.height;
    let mut mesh = Mesh::default();
    let metal = mesh.material(ALUMINIUM);
    let black = mesh.material(BAKELITE);

    // (radius fraction, height fraction, flute amount, sharp edge, metal)
    let profile: &[(f32, f32, f32, bool, bool)] = &[
        (0.000, 0.930, 0.0, false, true),
        (0.420, 0.928, 0.0, false, true),
        (style.cap, 0.924, 0.0, true, true), // cap edge
        (style.cap + 0.013, 0.893, 0.0, true, false), // drops into the recess
        (style.cap + 0.029, 0.892, 0.0, true, false), // recess floor
        (style.cap + 0.043, 0.982, 0.0, true, false), // rim rises proud of the cap
        (0.700, 0.985, 0.0, false, false),
        (0.792, 0.978, 0.0, false, false),
        (0.848, 0.962, 0.30, false, false), // flutes fade in
        (0.902, 0.944, 0.75, false, false),
        (0.951, 0.910, 1.00, false, false),
        (0.990, 0.852, 1.00, true, false),
        (1.000, 0.844, 0.00, true, false), // plain flange
        (1.000, 0.015, 0.00, true, false),
        (0.930, 0.000, 0.00, false, false),
    ];
    let rings: Vec<Ring> = profile
        .iter()
        .map(|&(rf, zf, fl, sharp, is_metal)| {
            let mat = if is_metal { metal } else { black };
            let ring = Ring::new(rf * r, zf * h, mat).fluted(fl);
            if sharp {
                ring.sharp()
            } else {
                ring
            }
        })
        .collect();
    let flute = Flute {
        count: spec.flute_count,
        depth: spec.flute_depth,
        sharpness: spec.flute_sharpness,
        phase: 0.0,
        sides: 0,
    };
    revolve_into(&mut mesh, &rings, spec.segments, &flute);

    // The indicator, a flattened rivet sitting on the black rim.
    let dot_mat = if style.brass_dot { BRASS } else { IVORY };
    let brass = mesh.material(dot_mat);
    let dot_r = style.dot_radius * r;
    let spin = spec.pointer_deg.to_radians();
    bar_into(
        &mut mesh,
        -0.004 * r,
        0.004 * r,
        dot_r,
        dot_r,
        dot_r * 0.42,
        dot_r * 0.42,
        40,
        brass,
        v3(style.dot_at * r, 0.0, 0.985 * h),
        spin,
    );

    mesh
}

/// The small teardrop selector: one moulded piece, widest at the shaft and
/// tapering to a rounded point, with a cream stripe let into the neck.
pub fn pultec_knob_small(spec: &KnobSpec) -> Mesh {
    let r = spec.radius;
    let mut mesh = Mesh::default();
    let matte = mesh.material(BAKELITE);

    // (distance from the shaft, half width, half height), fractions of the
    // bulb radius. Taken from the reference silhouette, which is widest just
    // behind the shaft and runs out to a tip 3.3 radii away.
    let body: &[(f32, f32, f32)] = &[
        (-1.020, 0.120, 0.100),
        (-0.960, 0.330, 0.190),
        (-0.740, 0.676, 0.330),
        (-0.400, 0.904, 0.410),
        (-0.100, 0.990, 0.440),
        (0.020, 1.000, 0.450),
        (0.370, 0.973, 0.440),
        (0.880, 0.840, 0.410),
        (1.730, 0.553, 0.330),
        (2.600, 0.399, 0.260),
        (3.190, 0.277, 0.200),
        (3.340, 0.180, 0.150),
        (3.440, 0.040, 0.050),
    ];
    let spin = spec.pointer_deg.to_radians();
    let scaled: Vec<(f32, f32, f32, f32)> = body
        .iter()
        .map(|&(x, w, h)| (x * r, w * r, h * r, 0.0))
        .collect();
    blade_into(&mut mesh, &scaled, 72, 2.15, matte, v3(0.0, 0.0, 0.450 * r), spin);

    // The stripe rides the sloping neck, so its half-height is set from the
    // body surface at each station rather than being a constant offset.
    let ivory = mesh.material(IVORY);
    let surface = |x: f32| -> f32 {
        let mut prev = body[0];
        for &s in body {
            if s.0 >= x {
                let t = if (s.0 - prev.0).abs() < 1e-6 { 0.0 } else { (x - prev.0) / (s.0 - prev.0) };
                return 0.450 + prev.2 + (s.2 - prev.2) * t;
            }
            prev = s;
        }
        0.450 + body[body.len() - 1].2
    };
    // A thin plate riding just proud of the neck, not a fin standing on edge.
    let stripe: Vec<(f32, f32, f32, f32)> = [
        (2.020, 0.022),
        (2.110, 0.088),
        (2.600, 0.090),
        (3.010, 0.078),
        (3.090, 0.020),
    ]
    .iter()
    .map(|&(x, w): &(f32, f32)| {
        (x * r, w * r, 0.030 * r, (surface(x) - 0.018) * r)
    })
    .collect();
    blade_into(&mut mesh, &stripe, 40, 2.6, ivory, Vec3::ZERO, spin);

    mesh
}

fn ring_list(rows: &[(f32, f32, bool)], scale: f32, mat: u16, flute: f32) -> Vec<Ring> {
    rows.iter()
        .map(|&(r, z, sharp)| {
            let ring = Ring::new(r * scale, z * scale, mat).fluted(flute);
            if sharp {
                ring.sharp()
            } else {
                ring
            }
        })
        .collect()
}

/// The IN/OUT toggle: a knurled bushing, a hex nut, a domed shoulder and a
/// tapered bat with a ball end. Rendering the two positions from the same
/// geometry is the point - the photographs came from different shots, so the
/// bat changed size between them.
pub fn pultec_toggle(up: bool, segments: u32) -> Mesh {
    // Everything is in units of the nut's distance across the flats.
    let a = 6.0_f32;
    let mut mesh = Mesh::default();
    let chrome = mesh.material(CHROME);

    // Knurled bushing under the nut.
    let knurl = ring_list(
        &[
            (0.00, 0.155, false),
            (0.90, 0.155, true),
            (1.00, 0.128, true),
            (1.00, -0.020, true),
            (0.91, -0.055, false),
        ],
        a,
        chrome,
        1.0,
    );
    revolve_into(
        &mut mesh,
        &knurl,
        segments,
        &Flute { count: 46, depth: 0.030, sharpness: 1.0, phase: 0.0, sides: 0 },
    );

    // Hex nut. `sides` makes Ring::r the distance to a flat.
    let nut = ring_list(
        &[
            (0.00, 0.420, false),
            (0.86, 0.420, true),
            (1.00, 0.383, true),
            (1.00, 0.160, true),
            (0.94, 0.150, false),
        ],
        a,
        chrome,
        0.0,
    );
    revolve_into(
        &mut mesh,
        &nut,
        segments,
        &Flute { count: 0, depth: 0.0, sharpness: 1.0, phase: 0.0, sides: 6 },
    );

    // Domed shoulder the bat pivots out of.
    let dome = ring_list(
        &[
            (0.00, 0.700, false),
            (0.24, 0.676, false),
            (0.40, 0.596, false),
            (0.49, 0.487, false),
            (0.525, 0.425, true),
            (0.525, 0.400, true),
        ],
        a,
        chrome,
        0.0,
    );
    revolve_into(&mut mesh, &dome, segments, &Flute { depth: 0.0, ..Flute::default() });

    // The bat, built upright then tilted about its base.
    let bat_start = mesh.verts.len();
    let bat = ring_list(
        &[
            (0.000, 1.790, false),
            (0.115, 1.770, false),
            (0.192, 1.706, false),
            (0.225, 1.600, false),
            (0.200, 1.487, false),
            (0.142, 1.392, false),
            (0.125, 1.240, false),
            (0.142, 0.860, false),
            (0.172, 0.470, false),
            (0.196, 0.250, false),
            (0.205, 0.000, false),
        ],
        a,
        chrome,
        0.0,
    );
    revolve_into(&mut mesh, &bat, segments, &Flute { depth: 0.0, ..Flute::default() });
    // Screen up is world +Y, and rotate_x by a positive angle carries the top
    // of the bat toward -Y. So the thrown-up position needs a negative tilt.
    let tilt = if up { -31.0_f32 } else { 31.0_f32 };
    mesh.rotate_x_range(bat_start, tilt.to_radians());
    mesh.translate_range(bat_start, v3(0.0, 0.0, 0.600 * a));

    mesh
}

/// The power jewel: a faceted red lens in a hex chrome bezel. The concentric
/// steps are what make it read as moulded glass rather than a red dot.
pub fn pultec_lamp(segments: u32) -> Mesh {
    let a = 6.0_f32;
    let mut mesh = Mesh::default();
    let chrome = mesh.material(CHROME);
    let jewel = mesh.material(JEWEL);

    let bezel = ring_list(
        &[
            (0.00, 0.250, false),
            (0.86, 0.250, false),
            (0.93, 0.292, true),
            (0.975, 0.284, true),
            (1.00, 0.240, true),
            (1.00, 0.030, true),
            (0.93, 0.000, false),
        ],
        a,
        chrome,
        0.0,
    );
    revolve_into(
        &mut mesh,
        &bezel,
        segments,
        &Flute { count: 0, depth: 0.0, sharpness: 1.0, phase: 0.0, sides: 6 },
    );

    // A dome with concentric steps pressed into it.
    const STEPS: usize = 26;
    let mut lens: Vec<(f32, f32, bool)> = Vec::with_capacity(STEPS + 1);
    for i in 0..=STEPS {
        let t = i as f32 / STEPS as f32;
        let r = 0.895 * t;
        // Spherical cap, rippled.
        let dome = 0.610 - 0.268 * t * t;
        let ripple = 0.012 * (t * 34.0).cos() * (1.0 - t * 0.4);
        lens.push((r, dome + ripple, false));
    }
    let rings = ring_list(&lens, a, jewel, 0.0);
    revolve_into(&mut mesh, &rings, segments, &Flute { depth: 0.0, ..Flute::default() });

    mesh
}

/// Look a material up by name, for the `--map` option.
pub fn by_name(name: &str) -> Option<Material> {
    Some(match name {
        "bakelite" => BAKELITE,
        "crown" => CROWN,
        "ivory" => IVORY,
        "aluminium" => ALUMINIUM,
        "brass" => BRASS,
        "chrome" => CHROME,
        "jewel" => JEWEL,
        _ => return None,
    })
}
