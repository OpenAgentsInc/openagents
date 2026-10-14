//! Exports: STEP and 3MF from the kernel, glTF (binary, `.glb`) written
//! here from the solids' triangles for the web viewer and Verse.

use std::path::Path;

use serde_json::{Value, json};

use crate::build::{Built, Solid};

/// The formats a document exports to.
pub const FORMATS: [&str; 4] = ["glb", "3mf", "step", "stl"];

/// Writes the built document as `format` to `path`.
///
/// # Errors
/// The format is unknown, or the kernel or the file system refused.
pub fn write(built: &mut Built, format: &str, path: &Path) -> Result<u64, String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    match format {
        "glb" => {
            let bytes = glb(&built.solids);
            std::fs::write(path, &bytes).map_err(|e| e.to_string())?;
            Ok(bytes.len() as u64)
        }
        "3mf" | "step" | "stl" => {
            let report = built
                .session
                .execute(
                    "file.export",
                    &json!({"path": path.to_string_lossy(), "format": format}),
                )
                .map_err(|e| e.to_string())?;
            Ok(report["bytes"].as_u64().unwrap_or(0))
        }
        other => Err(format!("I can export {}, not {other}.", FORMATS.join(", "))),
    }
}

fn pad4(bytes: &mut Vec<u8>, with: u8) {
    while bytes.len() % 4 != 0 {
        bytes.push(with);
    }
}

/// The solids as one binary glTF: a node per solid (named instance/body),
/// in metres with Y up (glTF's convention), one material per colour and
/// finish (a finish sets the roughness: glossy plastic shines).
#[must_use]
pub fn glb(solids: &[Solid]) -> Vec<u8> {
    let mut bin: Vec<u8> = Vec::new();
    let mut views = Vec::new();
    let mut accessors = Vec::new();
    let mut meshes = Vec::new();
    let mut nodes = Vec::new();
    let mut materials: Vec<([u8; 3], Option<&str>)> = Vec::new();
    for solid in solids {
        let m = &solid.mesh;
        if m.triangles.is_empty() {
            continue;
        }
        let key = (solid.color, solid.finish.as_deref());
        let material = match materials.iter().position(|m| *m == key) {
            Some(i) => i,
            None => {
                materials.push(key);
                materials.len() - 1
            }
        };
        // Model space (mm, Z up) to glTF (m, Y up): (x, y, z) → (x, z, -y).
        let to =
            |x: f64, y: f64, z: f64| [x as f32 / 1000.0, z as f32 / 1000.0, -y as f32 / 1000.0];
        let mut lo = [f32::MAX; 3];
        let mut hi = [f32::MIN; 3];
        let start = bin.len();
        for p in &m.positions {
            let q = to(p.x, p.y, p.z);
            for k in 0..3 {
                lo[k] = lo[k].min(q[k]);
                hi[k] = hi[k].max(q[k]);
                bin.extend_from_slice(&q[k].to_le_bytes());
            }
        }
        let positions_view = views.len();
        views.push(json!({"buffer": 0, "byteOffset": start, "byteLength": bin.len() - start, "target": 34962}));
        let start = bin.len();
        let normals: Vec<[f64; 3]> = if m.normals.len() == m.positions.len() {
            m.normals.iter().map(|n| [n.x, n.y, n.z]).collect()
        } else {
            vec![[0.0, 0.0, 1.0]; m.positions.len()]
        };
        for n in &normals {
            let q = to(n[0] * 1000.0, n[1] * 1000.0, n[2] * 1000.0);
            let len = (q[0] * q[0] + q[1] * q[1] + q[2] * q[2]).sqrt().max(1e-12);
            for x in q {
                bin.extend_from_slice(&(x / len).to_le_bytes());
            }
        }
        let normals_view = views.len();
        views.push(json!({"buffer": 0, "byteOffset": start, "byteLength": bin.len() - start, "target": 34962}));
        let start = bin.len();
        for t in &m.triangles {
            for i in t {
                bin.extend_from_slice(&i.to_le_bytes());
            }
        }
        let index_view = views.len();
        views.push(json!({"buffer": 0, "byteOffset": start, "byteLength": bin.len() - start, "target": 34963}));
        let a = accessors.len();
        accessors.push(json!({"bufferView": positions_view, "componentType": 5126, "count": m.positions.len(), "type": "VEC3", "min": lo, "max": hi}));
        accessors.push(json!({"bufferView": normals_view, "componentType": 5126, "count": m.positions.len(), "type": "VEC3"}));
        accessors.push(json!({"bufferView": index_view, "componentType": 5125, "count": m.triangles.len() * 3, "type": "SCALAR"}));
        meshes.push(json!({"name": solid.body, "primitives": [{"attributes": {"POSITION": a, "NORMAL": a + 1}, "indices": a + 2, "material": material}]}));
        nodes.push(
            json!({"name": format!("{}/{}", solid.instance, solid.body), "mesh": meshes.len() - 1,
                          "extras": {"instance": solid.instance, "part": solid.part}}),
        );
    }
    let materials: Vec<Value> = materials
        .iter()
        .map(|(c, finish)| {
            let lin = |x: u8| (f64::from(x) / 255.0).powf(2.2);
            // Without a finish, the plain look the rover has always had.
            let (metallic, rough) = match finish.and_then(crate::doc::roughness) {
                Some(r) => (0.0, r),
                None => (0.1, 0.6),
            };
            let name = format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2]);
            json!({"pbrMetallicRoughness": {"baseColorFactor": [lin(c[0]), lin(c[1]), lin(c[2]), 1.0], "metallicFactor": metallic, "roughnessFactor": rough},
                   "name": finish.map_or(name.clone(), |f| format!("{name} {f}"))})
        })
        .collect();
    let node_ids: Vec<usize> = (0..nodes.len()).collect();
    pad4(&mut bin, 0);
    let gltf = json!({
        "asset": {"version": "2.0", "generator": "OpenAgents CAD Lab"},
        "scene": 0,
        "scenes": [{"nodes": node_ids}],
        "nodes": nodes,
        "meshes": meshes,
        "materials": materials,
        "accessors": accessors,
        "bufferViews": views,
        "buffers": [{"byteLength": bin.len()}],
    });
    let mut text = gltf.to_string().into_bytes();
    pad4(&mut text, b' ');
    let total = 12 + 8 + text.len() + 8 + bin.len();
    let mut out = Vec::with_capacity(total);
    out.extend_from_slice(b"glTF");
    out.extend_from_slice(&2u32.to_le_bytes());
    out.extend_from_slice(&(total as u32).to_le_bytes());
    out.extend_from_slice(&(text.len() as u32).to_le_bytes());
    out.extend_from_slice(b"JSON");
    out.extend_from_slice(&text);
    out.extend_from_slice(&(bin.len() as u32).to_le_bytes());
    out.extend_from_slice(b"BIN\0");
    out.extend_from_slice(&bin);
    out
}
