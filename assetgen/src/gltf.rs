//! Minimal GLB reader: positions, indices and per-primitive materials.
//!
//! Enough to bring in an externally modelled control and render it with the
//! same rig as the parametric parts, so the two can be compared honestly.
//! Vertex normals are used when the export carries them, since only the
//! exporter knows which edges are meant to be creases; without them they are
//! derived from the faces, which rounds every edge off.

use crate::mesh::{smooth_normals, Material, Mesh, Vertex};
use crate::vec::{v3, Vec3};
use serde_json::Value;

/// Component types we can read an index buffer from.
const U16: u64 = 5123;
const U32: u64 = 5125;
const F32: u64 = 5126;

fn accessor_slice<'a>(json: &Value, bin: &'a [u8], index: usize) -> Option<(&'a [u8], u64, usize, usize)> {
    let acc = json["accessors"].get(index)?;
    let count = acc["count"].as_u64()? as usize;
    let component = acc["componentType"].as_u64()?;
    let kind = acc["type"].as_str()?;
    let per = match kind {
        "SCALAR" => 1,
        "VEC2" => 2,
        "VEC3" => 3,
        "VEC4" => 4,
        _ => return None,
    };
    let view = json["bufferViews"].get(acc["bufferView"].as_u64()? as usize)?;
    let offset = view["byteOffset"].as_u64().unwrap_or(0) as usize
        + acc["byteOffset"].as_u64().unwrap_or(0) as usize;
    let width = match component {
        U16 => 2,
        U32 | F32 => 4,
        _ => return None,
    };
    let len = count * per * width;
    Some((bin.get(offset..offset + len)?, component, count, per))
}

fn read_positions(json: &Value, bin: &[u8], index: usize) -> Vec<Vec3> {
    let Some((bytes, component, count, per)) = accessor_slice(json, bin, index) else {
        return Vec::new();
    };
    if component != F32 || per != 3 {
        return Vec::new();
    }
    (0..count)
        .map(|i| {
            let at = |k: usize| {
                let o = (i * 3 + k) * 4;
                f32::from_le_bytes([bytes[o], bytes[o + 1], bytes[o + 2], bytes[o + 3]])
            };
            v3(at(0), at(1), at(2))
        })
        .collect()
}

/// Unit normals, if the export supplies them.
fn read_normals(json: &Value, bin: &[u8], index: usize) -> Vec<Vec3> {
    read_positions(json, bin, index)
        .into_iter()
        .map(|n| n.normalise())
        .collect()
}

fn read_indices(json: &Value, bin: &[u8], index: usize) -> Vec<u32> {
    let Some((bytes, component, count, _)) = accessor_slice(json, bin, index) else {
        return Vec::new();
    };
    (0..count)
        .map(|i| match component {
            U16 => u16::from_le_bytes([bytes[i * 2], bytes[i * 2 + 1]]) as u32,
            U32 => u32::from_le_bytes([
                bytes[i * 4],
                bytes[i * 4 + 1],
                bytes[i * 4 + 2],
                bytes[i * 4 + 3],
            ]),
            _ => 0,
        })
        .collect()
}

/// Turn a glTF material into ours. The exports carry sensible base colour,
/// metalness and roughness, so they are taken as given; the surface texture is
/// ours, since glTF has no equivalent.
fn material_from(
    json: &Value,
    index: Option<usize>,
    fallback: Material,
    overrides: &[(String, Material)],
) -> Material {
    let Some(i) = index else { return fallback };
    let Some(m) = json["materials"].get(i) else {
        return fallback;
    };
    // A named override wins outright. The exports describe a lens as a
    // slightly metallic bright red, which renders as painted plastic; ours is
    // tuned against the reference and is lit from behind.
    if let Some(name) = m["name"].as_str() {
        if let Some((_, mat)) = overrides.iter().find(|(n, _)| n == name) {
            return *mat;
        }
    }
    let pbr = &m["pbrMetallicRoughness"];
    let base = pbr["baseColorFactor"]
        .as_array()
        .map(|a| {
            v3(
                a[0].as_f64().unwrap_or(0.5) as f32,
                a[1].as_f64().unwrap_or(0.5) as f32,
                a[2].as_f64().unwrap_or(0.5) as f32,
            )
        })
        .unwrap_or(fallback.base);
    let metallic = pbr["metallicFactor"].as_f64().unwrap_or(0.0) as f32;
    let rough = pbr["roughnessFactor"].as_f64().unwrap_or(0.5) as f32;
    Material { base, roughness: rough, metallic, ..fallback }
}

/// Load every primitive of a GLB into one mesh, scaled by `scale` and with the
/// bottom of its bounding box placed on z = 0.
pub fn load(
    path: &str,
    scale: f32,
    fallback: Material,
    overrides: &[(String, Material)],
    hide: &[String],
) -> Result<Mesh, String> {
    let data = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
    if data.len() < 20 || &data[0..4] != b"glTF" {
        return Err(format!("{path}: not a GLB"));
    }
    let json_len = u32::from_le_bytes([data[12], data[13], data[14], data[15]]) as usize;
    let json: Value = serde_json::from_slice(&data[20..20 + json_len])
        .map_err(|e| format!("{path}: bad JSON chunk: {e}"))?;
    // The binary chunk follows, after its own 8-byte header.
    let bin_start = 20 + json_len + 8;
    let bin = data.get(bin_start..).unwrap_or(&[]);

    let mut mesh = Mesh::default();
    let empty = vec![];
    for m in json["meshes"].as_array().unwrap_or(&empty) {
        for prim in m["primitives"].as_array().unwrap_or(&empty) {
            let Some(pos_idx) = prim["attributes"]["POSITION"].as_u64() else {
                continue;
            };
            // Nothing here is transparent, so a glazing pane would render as an
            // opaque sheet over the dial it is meant to cover.
            let named = prim["material"]
                .as_u64()
                .and_then(|i| json["materials"].get(i as usize))
                .and_then(|m| m["name"].as_str())
                .unwrap_or("");
            if hide.iter().any(|h| h == named) {
                continue;
            }
            let positions = read_positions(&json, bin, pos_idx as usize);
            if positions.is_empty() {
                continue;
            }
            let normals = prim["attributes"]["NORMAL"]
                .as_u64()
                .map(|i| read_normals(&json, bin, i as usize))
                .filter(|n| n.len() == positions.len())
                .unwrap_or_default();
            let indices = prim["indices"]
                .as_u64()
                .map(|i| read_indices(&json, bin, i as usize))
                .unwrap_or_else(|| (0..positions.len() as u32).collect());

            let mat = material_from(
                &json,
                prim["material"].as_u64().map(|i| i as usize),
                fallback,
                overrides,
            );
            let mat_id = mesh.material(mat);
            let vert_start = mesh.verts.len();
            let tri_start = mesh.tris.len();
            let base = vert_start as u32;
            let supplied = !normals.is_empty();
            for (i, p) in positions.into_iter().enumerate() {
                mesh.verts.push(Vertex {
                    pos: p * scale,
                    normal: if supplied { normals[i] } else { v3(0.0, 0.0, 1.0) },
                    ao: 1.0,
                    mat: mat_id,
                });
            }
            // Whole triangles only; a trailing partial one would be a
            // malformed accessor and is dropped with the remainder.
            for t in indices.as_chunks::<3>().0 {
                mesh.tris.push([base + t[0], base + t[1], base + t[2]]);
            }
            if !supplied {
                smooth_normals(&mut mesh, vert_start, tri_start);
            }
        }
    }
    if mesh.verts.is_empty() {
        return Err(format!("{path}: no geometry"));
    }
    // Sit it on the panel.
    let low = mesh.verts.iter().map(|v| v.pos.z).fold(f32::MAX, f32::min);
    for v in &mut mesh.verts {
        v.pos.z -= low;
    }
    Ok(mesh)
}
