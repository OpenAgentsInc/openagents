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
//! out, the model stands in front of where the player arrives. A document
//! has one placement per zone, keeping its version, so placing a newer
//! version replaces the old one; the zone sees the change within a second
//! ([`Placements::due`]). The glTF is CAD Lab's (metres, Y up, a node per
//! solid), read as [`super::meteor_rover`] reads the rover's; the
//! model's lowest point rests on the ground.
//!
//! Native callers poll the file. Browser callers pass authenticated model
//! bytes to `stand_bytes` through `WorldRuntime::install_cad_models`.

use std::path::PathBuf;
#[cfg(not(target_arch = "wasm32"))]
use std::{path::Path, time::SystemTime};

use glam::{Quat, Vec3};
use serde::Deserialize;
use serde_json::Value;

#[cfg(not(target_arch = "wasm32"))]
use super::ZoneId;
use crate::pbr::LitVertex;

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
    spawn + crate::controller::forward(yaw) * AHEAD
}

/// One model in one zone.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Placement {
    /// The CAD Lab document, e.g. `rover`.
    pub doc: String,
    pub version: u64,
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
    Ok(Placed {
        placement: placement.clone(),
        origin,
        size: (hi - lo) * scale,
        vertices,
    })
}

/// The models placed in the current zone, kept in step with the file.
#[derive(Default)]
pub struct Placements {
    /// What was last read: the zone and the file's time and length.
    #[cfg(not(target_arch = "wasm32"))]
    seen: Option<(ZoneId, Option<SystemTime>, u64)>,
    clock: f32,
    pub placed: Vec<Placed>,
    /// Placements that couldn't stand, in plain words.
    pub problems: Vec<String>,
}

impl Placements {
    /// Whether it's time to look at the file again, `dt` seconds on.
    pub fn due(&mut self, dt: f32) -> bool {
        self.clock += dt;
        if self.clock >= POLL_SECONDS {
            self.clock = 0.0;
            return true;
        }
        false
    }

    /// Reads `file` again when it or the zone changed, standing every
    /// placement for `zone`. Returns whether anything changed.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn refresh(
        &mut self,
        zone: ZoneId,
        file: &Path,
        ground: impl Fn(f32, f32) -> f32,
        default_at: Vec3,
    ) -> bool {
        let meta = std::fs::metadata(file).ok();
        let stamp = (
            zone,
            meta.as_ref().and_then(|m| m.modified().ok()),
            meta.as_ref().map_or(0, std::fs::Metadata::len),
        );
        if self.seen == Some(stamp) {
            return false;
        }
        self.seen = Some(stamp);
        self.placed.clear();
        self.problems.clear();
        let list = match read(file) {
            Ok(list) => list,
            Err(e) => {
                self.problems.push(e);
                return true;
            }
        };
        let mut triangles = 0;
        for placement in list.iter().filter(|p| p.zone == zone.world_id()) {
            match stand(placement, &ground, default_at) {
                Ok(placed) if triangles + placed.vertices.len() / 3 <= MAX_TRIANGLES => {
                    triangles += placed.vertices.len() / 3;
                    self.placed.push(placed);
                }
                Ok(_) => self.problems.push(format!(
                    "{} v{} would take {zone:?} past {MAX_TRIANGLES} placed triangles",
                    placement.doc, placement.version
                )),
                Err(e) => self.problems.push(e),
            }
        }
        for problem in &self.problems {
            eprintln!("verse: a CAD model can't stand here: {problem}");
        }
        true
    }

    /// Every placed model's triangles.
    #[must_use]
    pub fn lit(&self) -> Vec<LitVertex> {
        self.placed
            .iter()
            .flat_map(|p| p.vertices.iter().copied())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ROVER: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../assets/verse/cad/rover.glb"
    );

    fn file(dir: &Path, placements: &serde_json::Value) -> PathBuf {
        std::fs::create_dir_all(dir).unwrap();
        let path = dir.join("placements.json");
        std::fs::write(
            &path,
            serde_json::json!({"kind": KIND, "placements": placements}).to_string(),
        )
        .unwrap();
        path
    }

    #[test]
    fn default_placements_are_in_front_of_the_arriving_player() {
        for yaw in [0., 1., 2., 3.] {
            let spawn = Vec3::new(0., 0., -20.);
            let delta = ahead(spawn, yaw) - spawn;
            assert!((delta.length() - AHEAD).abs() < 1e-5);
            assert!(delta.dot(crate::controller::forward(yaw)) > 7.99);
        }
    }

    #[test]
    fn supplied_bytes_stand_without_a_local_file_and_refuse_bad_settings() {
        let bytes = std::fs::read(ROVER).unwrap();
        let mut placement:Placement=serde_json::from_value(serde_json::json!({"doc":"rover","version":1,"zone":"verse-everglade","glb":"/cad-world/model.glb","at":[3.,0.,4.],"scale":2.})).unwrap();
        let placed = stand_bytes(&placement, &bytes, |_, _| 1., Vec3::ZERO).unwrap();
        assert_eq!(placed.origin, Vec3::new(3., 1., 4.));
        assert!(placed.vertices.len() > 100);
        assert!((placed.size.y - 2.244).abs() < 0.02);
        placement.scale = f32::NAN;
        assert!(stand_bytes(&placement, &bytes, |_, _| 0., Vec3::ZERO).is_err());
        placement.scale = 1.;
        assert!(stand_bytes(&placement, b"broken", |_, _| 0., Vec3::ZERO).is_err());
    }

    #[test]
    fn a_placed_version_stands_on_the_ground_and_a_newer_one_replaces_it() {
        let dir = std::env::temp_dir().join(format!("verse-cad-place-{}", std::process::id()));
        let ground = |x: f32, z: f32| 0.1 * x + 0.05 * z;
        let path = file(
            &dir,
            &serde_json::json!([
                {"doc": "rover", "version": 38, "zone": "verse-everglade", "glb": ROVER, "at": [4.0, 0.0, -2.0], "heading_deg": 90.0},
                {"doc": "rover", "version": 38, "zone": "verse-coast", "glb": ROVER}
            ]),
        );
        let mut places = Placements::default();
        assert!(places.refresh(ZoneId::Everglade, &path, ground, Vec3::ZERO));
        assert_eq!(places.placed.len(), 1, "{:?}", places.problems);
        let rover = &places.placed[0];
        assert_eq!(rover.origin, Vec3::new(4.0, 0.3, -2.0));
        // The rover v38 is 909 × 1122 × 980 mm (X forward, Z up, Y left).
        assert!((rover.size.y - 1.122).abs() < 0.01, "{}", rover.size);
        let lowest = rover
            .vertices
            .iter()
            .map(|v| v.pos[1])
            .fold(f32::MAX, f32::min);
        assert!((lowest - 0.3).abs() < 1e-4, "{lowest}");
        // Turned a quarter: its length now lies along Z.
        let (lo, hi) = rover.vertices.iter().fold(
            (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)),
            |(lo, hi), v| (lo.min(Vec3::from(v.pos)), hi.max(Vec3::from(v.pos))),
        );
        assert!(((hi.z - lo.z) - 0.909).abs() < 0.01, "{lo} {hi}");
        assert!(!places.refresh(ZoneId::Everglade, &path, ground, Vec3::ZERO));
        // Another zone sees only its own.
        assert!(places.refresh(ZoneId::Coast, &path, ground, Vec3::new(1.0, 2.0, 3.0)));
        assert_eq!(places.placed[0].origin, Vec3::new(1.0, 2.0, 3.0));
        // A newer version replaces the old in the file; the zone follows.
        std::thread::sleep(std::time::Duration::from_millis(20));
        file(
            &dir,
            &serde_json::json!([{"doc": "rover", "version": 39, "zone": "verse-everglade", "glb": ROVER, "scale": 0.5}]),
        );
        assert!(places.refresh(ZoneId::Everglade, &path, ground, Vec3::ZERO));
        assert_eq!(places.placed.len(), 1);
        assert_eq!(places.placed[0].placement.version, 39);
        assert!((places.placed[0].size.y - 0.561).abs() < 0.01);
        // A missing glTF is a plain problem, not a crash.
        file(
            &dir,
            &serde_json::json!([{"doc": "gone", "version": 1, "zone": "verse-everglade", "glb": dir.join("none.glb")}]),
        );
        assert!(places.refresh(ZoneId::Everglade, &path, ground, Vec3::ZERO));
        assert!(places.placed.is_empty() && places.problems.len() == 1);
        let _ = std::fs::remove_dir_all(dir);
    }
}
