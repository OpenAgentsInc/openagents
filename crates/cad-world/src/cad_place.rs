//! CAD Lab models placed in Verse zones (#11642, docs/cad-lab/README.md,
//! "Into Verse"): `cad_place` (or `openagents cad place`) writes a version's
//! glTF, its zone, place and heading into a placements file, and the zone
//! loads every placement named for it and stands each model on the ground
//! there.
//!
//! The file (`verse-cad-placements.v1`) is plain data:
//!
//! ```json
//! {"kind": "verse-cad-placements.v1", "placements": [
//!   {"doc": "rover", "version": 38, "zone": "verse-everglade",
//!    "glb": "/…/rover/v38/model.glb", "at": [3, 0, -4], "heading_deg": 30, "scale": 1}]}
//! ```
//!
//! `at` is x and z in the zone (metres) and a height above the ground; left
//! out, the model stands in front of where the player arrives. A copy
//! has a stable id, defaulting to its document. A newer version with the
//! same id replaces that copy; the zone sees the change within a second
//! (`verse::zones::cad_place::Placements`). The glTF is CAD Lab's (metres, Y up, a node per
//! solid), read by the shared CAD physics module; the
//! model's lowest point rests on the ground.
//!
//! Native callers poll the file. Browser callers pass authenticated model
//! bytes to `stand_bytes` through `WorldRuntime::install_cad_models`.

#[cfg(not(target_arch = "wasm32"))]
use std::path::Path;
use std::path::PathBuf;

use crate::cad_physics::Model;
use glam::{Quat, Vec3};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::sync::Arc;

use crate::LitVertex;

/// The placements file's kind.
pub const KIND: &str = "verse-cad-placements.v1";
/// The largest glTF read, bytes.
pub const MAX_GLB_BYTES: u64 = 64 << 20;
/// The most triangles a zone's placed models may have together.
pub const MAX_TRIANGLES: usize = 150_000;
/// How often the file is looked at for changes, s.
pub const POLL_SECONDS: f32 = 1.0;
/// How far in front of the arrival point an unplaced model stands, m.
pub const AHEAD: f32 = 8.0;

/// A default placement in front of the zone's arrival point.
pub fn ahead(spawn: Vec3, yaw: f32) -> Vec3 {
    spawn + crate::shapes::forward(yaw) * AHEAD
}

/// One model in one zone.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct Placement {
    /// Separate copies use different ids; older files use the document id.
    #[serde(default)]
    pub id: Option<String>,
    /// The CAD Lab document, e.g. `rover`.
    pub doc: String,
    pub version: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generation: Option<String>,
    /// The zone's world id, e.g. `verse-everglade`.
    pub zone: String,
    /// The version's glTF (`model.glb`).
    pub glb: PathBuf,
    /// x, height above the ground, z; m.
    #[serde(default)]
    pub at: Option<[f32; 3]>,
    /// Which way it faces, degrees about +Y (0 faces +X, the CAD model's
    /// forward).
    #[serde(default)]
    pub heading_deg: f32,
    #[serde(default = "one")]
    pub scale: f32,
}

impl Placement {
    pub fn key(&self) -> &str {
        self.id.as_deref().unwrap_or(&self.doc)
    }
}

fn one() -> f32 {
    1.0
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Deserialize)]
struct File {
    kind: String,
    #[serde(default)]
    placements: Vec<Placement>,
}

/// The placements file: `$VERSE_CAD_PLACEMENTS`, else
/// `$CAD_LAB_DIR/.verse/placements.json`, else CAD Lab's own folder
/// (`~/work/film-out/lab/cad/.verse/placements.json`).
#[cfg(not(target_arch = "wasm32"))]
#[must_use]
pub fn path() -> PathBuf {
    if let Some(file) = std::env::var_os("VERSE_CAD_PLACEMENTS") {
        return PathBuf::from(file);
    }
    let root = std::env::var_os("CAD_LAB_DIR").map_or_else(
        || {
            std::env::var_os("HOME")
                .map(PathBuf::from)
                .unwrap_or_default()
                .join("work/film-out/lab/cad")
        },
        PathBuf::from,
    );
    root.join(".verse").join("placements.json")
}

/// The placements in `path` (none when there is no file).
///
/// # Errors
/// The file isn't a placements file.
#[cfg(not(target_arch = "wasm32"))]
pub fn read(path: &Path) -> Result<Vec<Placement>, String> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(format!("{}: {e}", path.display())),
    };
    let file: File = serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
    if file.kind != KIND {
        return Err(format!("{} is {}, not {KIND}", path.display(), file.kind));
    }
    Ok(file.placements)
}

/// One solid of a CAD Lab glTF (metres, Y up).
struct Solid {
    positions: Vec<Vec3>,
    normals: Vec<Vec3>,
    indices: Vec<u32>,
    color: [f32; 3],
}

/// CAD Lab's glTF (`export::glb`): a node per solid, positions, normals
/// and indices, a colour per material.
/// The CAD Lab rover's glTF (`assets/verse/cad/rover.glb`), known by its
/// length and SHA-256. Comparing against the file itself built all 5.7 MB
/// of it into every program that places CAD models, the browser's Verse
/// module among them (#12319).
const ROVER_LEN: usize = 5_748_500;
const ROVER_SHA256: &str = "d68343cfa8566db96739d8bcfae9bb967ffee3158cfe06f88d467619c4a75bda";

fn is_rover(bytes: &[u8]) -> bool {
    use sha2::Digest as _;
    bytes.len() == ROVER_LEN && {
        let digest = sha2::Sha256::digest(bytes);
        digest
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
            == ROVER_SHA256
    }
}

fn parse_glb(bytes: &[u8]) -> Option<Vec<Solid>> {
    let word = |at: usize| -> Option<usize> {
        Some(u32::from_le_bytes(bytes.get(at..at + 4)?.try_into().ok()?) as usize)
    };
    if bytes.get(..4)? != b"glTF" {
        return None;
    }
    let json_len = word(12)?;
    let json: Value = serde_json::from_slice(bytes.get(20..20 + json_len)?).ok()?;
    let bin = 20 + json_len + 8;
    let floats = |accessor: &Value| -> Option<Vec<f32>> {
        let view = &json["bufferViews"][accessor["bufferView"].as_u64()? as usize];
        let start = bin + view["byteOffset"].as_u64().unwrap_or(0) as usize;
        let count = accessor["count"].as_u64()? as usize * 3;
        let raw = bytes.get(start..start + count * 4)?;
        Some(
            raw.chunks_exact(4)
                .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
                .collect(),
        )
    };
    let mut out = Vec::new();
    for node in json["nodes"].as_array()? {
        let mesh = &json["meshes"][node["mesh"].as_u64()? as usize];
        let primitive = &mesh["primitives"][0];
        let accessor = |name: &str| {
            &json["accessors"][primitive["attributes"][name].as_u64().unwrap_or(0) as usize]
        };
        let positions = floats(accessor("POSITION"))?;
        let normals = floats(accessor("NORMAL"))?;
        let index_accessor = &json["accessors"][primitive["indices"].as_u64()? as usize];
        let view = &json["bufferViews"][index_accessor["bufferView"].as_u64()? as usize];
        let start = bin + view["byteOffset"].as_u64().unwrap_or(0) as usize;
        let count = index_accessor["count"].as_u64()? as usize;
        let indices = bytes
            .get(start..start + count * 4)?
            .chunks_exact(4)
            .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect();
        let material = &json["materials"][primitive["material"].as_u64().unwrap_or(0) as usize];
        let factor = &material["pbrMetallicRoughness"]["baseColorFactor"];
        let color = [0, 1, 2].map(|k| factor[k].as_f64().unwrap_or(0.6) as f32);
        let triple = |v: &[f32]| {
            v.chunks_exact(3)
                .map(|c| Vec3::new(c[0], c[1], c[2]))
                .collect()
        };
        out.push(Solid {
            positions: triple(&positions),
            normals: triple(&normals),
            indices,
            color,
        });
    }
    Some(out)
}

/// A model standing in a zone.
pub struct Placed {
    pub placement: Placement,
    /// Where its base rests, world m.
    pub origin: Vec3,
    /// Its width, height and depth, m.
    pub size: Vec3,
    /// Its triangles in the world, lit physically.
    pub vertices: Vec<LitVertex>,
    pub model: Arc<Model>,
    pub fingerprint: String,
    pub geometry: String,
}

/// Stands `placement` in the world: on `ground(x, z)`, or at `default_at`
/// when it names no place.
///
/// # Errors
/// The glTF can't be read or holds no solids.
#[cfg(not(target_arch = "wasm32"))]
pub fn stand(
    placement: &Placement,
    ground: impl Fn(f32, f32) -> f32,
    default_at: Vec3,
) -> Result<Placed, String> {
    let shown = placement.glb.display();
    let size = std::fs::metadata(&placement.glb)
        .map_err(|e| format!("{shown}: {e}"))?
        .len();
    if size > MAX_GLB_BYTES {
        return Err(format!("{shown} is larger than {MAX_GLB_BYTES} bytes"));
    }
    let bytes = std::fs::read(&placement.glb).map_err(|e| format!("{shown}: {e}"))?;
    stand_bytes(placement, &bytes, ground, default_at)
}

/// Stands an authenticated CAD model supplied by a browser or native caller.
pub fn stand_bytes(
    placement: &Placement,
    bytes: &[u8],
    ground: impl Fn(f32, f32) -> f32,
    default_at: Vec3,
) -> Result<Placed, String> {
    if bytes.len() as u64 > MAX_GLB_BYTES {
        return Err("This CAD model is too large.".into());
    }
    let mut model = Model::read(bytes)?;
    let solids = parse_glb(bytes)
        .filter(|s| !s.is_empty())
        .ok_or("This file is not a CAD Lab model.")?;
    if placement
        .at
        .is_some_and(|p| p.iter().any(|v| !v.is_finite() || v.abs() > 5000.))
        || !placement.heading_deg.is_finite()
        || !(0.01..=100.).contains(&placement.scale)
    {
        return Err("Choose a finite place, heading, and scale for the model.".into());
    }
    if solids
        .iter()
        .flat_map(|s| s.positions.iter().chain(&s.normals))
        .any(|p| !p.is_finite())
        || solids.iter().map(|s| s.indices.len() / 3).sum::<usize>() > MAX_TRIANGLES
    {
        return Err("This CAD model has invalid or too many triangles.".into());
    }
    let scale = if placement.scale.is_finite() && placement.scale > 0.0 {
        placement.scale
    } else {
        1.0
    };
    let (mut lo, mut hi) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
    for p in solids.iter().flat_map(|s| &s.positions) {
        lo = lo.min(*p);
        hi = hi.max(*p);
    }
    let origin = match placement.at {
        Some([x, y, z]) => Vec3::new(x, ground(x, z) + y, z),
        None => default_at,
    };
    let turn = Quat::from_rotation_y(placement.heading_deg.to_radians());
    let place = |p: Vec3| origin + turn * ((p - Vec3::new(0.0, lo.y, 0.0)) * scale);
    let mut vertices = Vec::new();
    for s in &solids {
        for tri in s.indices.chunks_exact(3) {
            let [a, b, c] = [tri[0], tri[1], tri[2]].map(|i| i as usize);
            let (Some(&pa), Some(&pb), Some(&pc)) =
                (s.positions.get(a), s.positions.get(b), s.positions.get(c))
            else {
                continue;
            };
            let tangent = (turn * (pb - pa)).normalize_or(Vec3::X);
            for (p, i) in [(pa, a), (pb, b), (pc, c)] {
                let n = s.normals.get(i).copied().unwrap_or(Vec3::Y);
                vertices.push(LitVertex {
                    pos: place(p).to_array(),
                    normal: (turn * n).normalize_or(Vec3::Y).to_array(),
                    tangent: tangent.to_array(),
                    local: p.to_array(),
                    color: s.color,
                    params: [0.15, 0.55, 0.0, 1.0],
                });
            }
        }
    }
    if is_rover(bytes) {
        let mut spec: Value =
            serde_json::from_str(include_str!("../../../assets/verse/cad/rover.json"))
                .map_err(|e| e.to_string())?;
        for joint in spec["joints"].as_array_mut().into_iter().flatten() {
            let (limits, servo) =
                crate::cad_physics::rover_joint_settings(joint["id"].as_str().unwrap_or_default());
            if let Some(l) = limits {
                joint["limits"] = serde_json::json!([l.lower.to_degrees(), l.upper.to_degrees()]);
            }
            if let Some(s) = servo {
                joint["servo"] = serde_json::to_value(s).map_err(|e| e.to_string())?;
            }
            joint["pin_radius_m"] = serde_json::json!(crate::cad_physics::rover_pin_radius(
                joint["id"].as_str().unwrap_or_default()
            ));
        }
        model.solids =
            crate::cad_physics::parse_glb(bytes, &spec).ok_or("This rover has no solids.")?;
        model.spec = spec;
    }
    let mut hash = Sha256::new();
    hash.update(bytes);
    let geometry = format!("{:x}", hash.clone().finalize());
    hash.update(serde_json::to_vec(placement).map_err(|e| e.to_string())?);
    Ok(Placed {
        model: Arc::new(model),
        fingerprint: format!("{:x}", hash.finalize()),
        geometry,
        placement: placement.clone(),
        origin,
        size: (hi - lo) * scale,
        vertices,
    })
}

#[cfg(test)]
mod rover_tests {
    #[test]
    fn the_rover_is_known_by_its_length_and_digest() {
        let rover = include_bytes!("../../../assets/verse/cad/rover.glb");
        assert!(super::is_rover(rover));
        let mut other = rover.to_vec();
        other[100] ^= 1;
        assert!(!super::is_rover(&other));
        assert!(!super::is_rover(b"glTF"));
    }
}
