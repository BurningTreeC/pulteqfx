//! Ambient occlusion baked per vertex by ray casting against the mesh itself.
//!
//! This is what makes the recess under a knob crown read as a recess rather
//! than a painted ring, and it only has to be computed once because the
//! geometry is rigid: the knob turns, but it turns with its own shadowing.

use crate::mesh::Mesh;
use crate::vec::{v3, Vec3};
use std::f32::consts::PI;

/// Uniform grid over the triangles so a ray only tests what is near it.
struct Grid {
    min: Vec3,
    cell: f32,
    dim: [usize; 3],
    buckets: Vec<Vec<u32>>,
}

impl Grid {
    fn build(mesh: &Mesh, target_per_axis: usize) -> Self {
        let (mut min, mut max) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
        for v in &mesh.verts {
            min = v3(min.x.min(v.pos.x), min.y.min(v.pos.y), min.z.min(v.pos.z));
            max = v3(max.x.max(v.pos.x), max.y.max(v.pos.y), max.z.max(v.pos.z));
        }
        let extent = max - min;
        let longest = extent.x.max(extent.y).max(extent.z).max(1e-6);
        let cell = longest / target_per_axis as f32;
        let dim = [
            ((extent.x / cell).ceil() as usize + 1).max(1),
            ((extent.y / cell).ceil() as usize + 1).max(1),
            ((extent.z / cell).ceil() as usize + 1).max(1),
        ];
        let mut grid = Grid { min, cell, dim, buckets: vec![Vec::new(); dim[0] * dim[1] * dim[2]] };
        for (i, t) in mesh.tris.iter().enumerate() {
            let p: Vec<Vec3> = t.iter().map(|v| mesh.verts[*v as usize].pos).collect();
            let lo = v3(
                p[0].x.min(p[1].x).min(p[2].x),
                p[0].y.min(p[1].y).min(p[2].y),
                p[0].z.min(p[1].z).min(p[2].z),
            );
            let hi = v3(
                p[0].x.max(p[1].x).max(p[2].x),
                p[0].y.max(p[1].y).max(p[2].y),
                p[0].z.max(p[1].z).max(p[2].z),
            );
            let (a, b) = (grid.cell_of(lo), grid.cell_of(hi));
            for z in a[2]..=b[2] {
                for y in a[1]..=b[1] {
                    for x in a[0]..=b[0] {
                        let idx = grid.index([x, y, z]);
                        grid.buckets[idx].push(i as u32);
                    }
                }
            }
        }
        grid
    }

    fn cell_of(&self, p: Vec3) -> [usize; 3] {
        let f = (p - self.min) / self.cell;
        [
            (f.x.floor().max(0.0) as usize).min(self.dim[0] - 1),
            (f.y.floor().max(0.0) as usize).min(self.dim[1] - 1),
            (f.z.floor().max(0.0) as usize).min(self.dim[2] - 1),
        ]
    }

    fn index(&self, c: [usize; 3]) -> usize {
        (c[2] * self.dim[1] + c[1]) * self.dim[0] + c[0]
    }
}

/// Moller-Trumbore, front and back faces alike since we only need occlusion.
fn hit(orig: Vec3, dir: Vec3, a: Vec3, b: Vec3, c: Vec3, max_t: f32) -> bool {
    const EPS: f32 = 1e-7;
    let (e1, e2) = (b - a, c - a);
    let p = dir.cross(e2);
    let det = e1.dot(p);
    if det.abs() < EPS {
        return false;
    }
    let inv = 1.0 / det;
    let tv = orig - a;
    let u = tv.dot(p) * inv;
    if !(0.0..=1.0).contains(&u) {
        return false;
    }
    let q = tv.cross(e1);
    let v = dir.dot(q) * inv;
    if v < 0.0 || u + v > 1.0 {
        return false;
    }
    let t = e2.dot(q) * inv;
    t > EPS && t < max_t
}

/// March the grid along the ray, testing triangles until something is hit.
fn occluded(grid: &Grid, mesh: &Mesh, orig: Vec3, dir: Vec3, max_t: f32, seen: &mut [u32], tag: u32) -> bool {
    let mut cell = grid.cell_of(orig);
    let step = [
        if dir.x > 0.0 { 1i32 } else { -1 },
        if dir.y > 0.0 { 1i32 } else { -1 },
        if dir.z > 0.0 { 1i32 } else { -1 },
    ];
    let d = [dir.x, dir.y, dir.z];
    let o = [orig.x, orig.y, orig.z];
    let m = [grid.min.x, grid.min.y, grid.min.z];
    let mut t_max = [0.0f32; 3];
    let mut t_delta = [0.0f32; 3];
    for i in 0..3 {
        if d[i].abs() < 1e-12 {
            t_max[i] = f32::MAX;
            t_delta[i] = f32::MAX;
        } else {
            let boundary = m[i] + (cell[i] as f32 + if d[i] > 0.0 { 1.0 } else { 0.0 }) * grid.cell;
            t_max[i] = (boundary - o[i]) / d[i];
            t_delta[i] = (grid.cell / d[i]).abs();
        }
    }
    let mut travelled = 0.0f32;
    while travelled < max_t {
        for &tri in &grid.buckets[grid.index(cell)] {
            if seen[tri as usize] == tag {
                continue;
            }
            seen[tri as usize] = tag;
            let t = mesh.tris[tri as usize];
            let (a, b, c) = (
                mesh.verts[t[0] as usize].pos,
                mesh.verts[t[1] as usize].pos,
                mesh.verts[t[2] as usize].pos,
            );
            if hit(orig, dir, a, b, c, max_t) {
                return true;
            }
        }
        // Advance to the next cell.
        let axis = if t_max[0] < t_max[1] && t_max[0] < t_max[2] {
            0
        } else if t_max[1] < t_max[2] {
            1
        } else {
            2
        };
        travelled = t_max[axis];
        let next = cell[axis] as i32 + step[axis];
        if next < 0 || next as usize >= grid.dim[axis] {
            return false;
        }
        cell[axis] = next as usize;
        t_max[axis] += t_delta[axis];
    }
    false
}

/// Hammersley point set, so a rebuild produces byte-identical output.
fn hammersley(i: u32, n: u32) -> (f32, f32) {
    let mut bits = i;
    bits = bits.rotate_right(16);
    bits = ((bits & 0x5555_5555) << 1) | ((bits & 0xAAAA_AAAA) >> 1);
    bits = ((bits & 0x3333_3333) << 2) | ((bits & 0xCCCC_CCCC) >> 2);
    bits = ((bits & 0x0F0F_0F0F) << 4) | ((bits & 0xF0F0_F0F0) >> 4);
    bits = ((bits & 0x00FF_00FF) << 8) | ((bits & 0xFF00_FF00) >> 8);
    (i as f32 / n as f32, bits as f32 * 2.328_306_4e-10)
}

/// Bake occlusion into `mesh.verts[..].ao`.
///
/// `radius` bounds how far a ray looks; occlusion from the far side of the
/// part should not darken a face that is plainly in the open.
pub fn bake(mesh: &mut Mesh, samples: u32, radius: f32) {
    let grid = Grid::build(mesh, 32);
    let mut seen = vec![u32::MAX; mesh.tris.len()];
    let positions: Vec<(Vec3, Vec3)> = mesh.verts.iter().map(|v| (v.pos, v.normal)).collect();
    let mut out = vec![1.0f32; mesh.verts.len()];

    for (vi, (pos, normal)) in positions.iter().enumerate() {
        let n = *normal;
        let tangent = n.any_perpendicular();
        let bitangent = n.cross(tangent);
        // Lift off the surface so a ray does not immediately hit its own face.
        let orig = *pos + n * (radius * 1e-3);
        let mut open = 0u32;
        for s in 0..samples {
            let (u1, u2) = hammersley(s, samples);
            // Cosine-weighted hemisphere around the normal.
            let r = u1.sqrt();
            let phi = 2.0 * PI * u2;
            let dir = (tangent * (r * phi.cos()) + bitangent * (r * phi.sin())
                + n * (1.0 - u1).max(0.0).sqrt())
            .normalise();
            let tag = (vi as u32).wrapping_mul(samples).wrapping_add(s);
            if !occluded(&grid, mesh, orig, dir, radius, &mut seen, tag) {
                open += 1;
            }
        }
        out[vi] = open as f32 / samples as f32;
    }

    for (v, ao) in mesh.verts.iter_mut().zip(out) {
        v.ao = ao;
    }
}
