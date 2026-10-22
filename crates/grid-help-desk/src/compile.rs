//! Compiles the Blender export into [`crate::devices::PINNED`], and checks
//! that the pinned file is current. `UPDATE_FIXTURES=1 cargo test -p
//! grid-help-desk` rewrites it after `scripts/blender/help_desk_devices.py`
//! runs again.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use glam::{Mat4, Vec3};

use crate::devices::{Device, PINNED_PATH, PRESENT, QUIET};

/// The exported models, from the workspace root, by the name the desk uses.
pub const SOURCES: [(&str, &str); 2] = [
    ("macbook", "assets/verse/generated/help_desk/macbook.glb"),
    ("iphone", "assets/verse/generated/help_desk/iphone.glb"),
];

/// Faces meeting at more than this angle, degrees, draw their edge.
const CREASE: f32 = 32.0;
/// Corners closer than this, m, are one corner.
const WELD: f32 = 2e-5;
/// How far the display stands off the glass, m, so the depth test never
/// mixes the two.
const LIFT: f32 = 0.001;

/// Materials whose edges draw quietly: the keys and their wells, and the
/// speaker grilles.
fn quiet(material: &str) -> bool {
    material.starts_with("mb_keycap")
        || material.starts_with("mb_well")
        || material.starts_with("mb_grille")
}

pub fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// One exported model as line art.
pub fn compile(name: &str, path: &Path) -> Result<Device, String> {
    let (doc, buffers, _) = gltf::import(path).map_err(|e| e.to_string())?;
    let mut corners: Vec<[f32; 3]> = Vec::new();
    let mut welded: HashMap<[i64; 3], u16> = HashMap::new();
    let mut triangles: Vec<[u16; 3]> = Vec::new();
    let mut materials: Vec<String> = Vec::new();
    let mut screen: Vec<([f32; 3], [f32; 2])> = Vec::new();
    let mut screen_indices: Vec<u16> = Vec::new();
    let scene = doc
        .default_scene()
        .or_else(|| doc.scenes().next())
        .ok_or("no scene")?;
    let mut stack: Vec<(gltf::Node, Mat4)> = scene.nodes().map(|n| (n, Mat4::IDENTITY)).collect();
    while let Some((node, parent)) = stack.pop() {
        let transform = parent * Mat4::from_cols_array_2d(&node.transform().matrix());
        for child in node.children() {
            stack.push((child, transform));
        }
        let Some(mesh) = node.mesh() else { continue };
        for primitive in mesh.primitives() {
            let reader = primitive.reader(|b| Some(&buffers[b.index()]));
            let positions: Vec<Vec3> = reader
                .read_positions()
                .ok_or("a primitive has no positions")?
                .map(|p| transform.transform_point3(Vec3::from_array(p)))
                .collect();
            let indices: Vec<u32> = match reader.read_indices() {
                Some(i) => i.into_u32().collect(),
                None => (0..positions.len() as u32).collect(),
            };
            let material = primitive.material().name().unwrap_or("").to_owned();
            if material.starts_with("screen") {
                let uvs: Vec<[f32; 2]> = reader
                    .read_tex_coords(0)
                    .ok_or("the screen has no texture coordinates")?
                    .into_f32()
                    .collect();
                let base = screen.len() as u16;
                for (p, uv) in positions.iter().zip(uvs) {
                    screen.push((p.to_array(), uv));
                }
                screen_indices.extend(indices.iter().map(|i| base + *i as u16));
                continue;
            }
            let mut at = |p: Vec3| -> u16 {
                let key = (p / WELD).round().as_i64vec3().to_array();
                *welded.entry(key).or_insert_with(|| {
                    corners.push(p.to_array());
                    (corners.len() - 1) as u16
                })
            };
            let ids: Vec<u16> = positions.iter().map(|p| at(*p)).collect();
            for t in indices.chunks_exact(3) {
                let tri = [ids[t[0] as usize], ids[t[1] as usize], ids[t[2] as usize]];
                if tri[0] == tri[1] || tri[1] == tri[2] || tri[0] == tri[2] {
                    continue;
                }
                triangles.push(tri);
                materials.push(material.clone());
            }
        }
    }
    let normal = |t: &[u16; 3]| {
        let [a, b, c] = t.map(|i| Vec3::from_array(corners[usize::from(i)]));
        (b - a).cross(c - a).normalize_or_zero()
    };
    let mut sides: BTreeMap<(u16, u16), Vec<usize>> = BTreeMap::new();
    for (index, t) in triangles.iter().enumerate() {
        for k in 0..3 {
            let (a, b) = (t[k], t[(k + 1) % 3]);
            sides.entry((a.min(b), a.max(b))).or_default().push(index);
        }
    }
    let cos = CREASE.to_radians().cos();
    let mut edges = Vec::new();
    for ((a, b), faces) in &sides {
        let sharp = match faces.as_slice() {
            [f, g] => {
                materials[*f] != materials[*g]
                    || normal(&triangles[*f]).dot(normal(&triangles[*g])) < cos
            }
            _ => true,
        };
        if sharp {
            let tone = if faces.iter().all(|f| quiet(&materials[*f])) {
                QUIET
            } else {
                PRESENT
            };
            edges.push(([*a, *b], tone));
        }
    }
    // The display is flat: lift it along its own normal.
    if let [a, b, c, ..] = screen_indices.as_slice() {
        let [a, b, c] = [*a, *b, *c].map(|i| Vec3::from_array(screen[usize::from(i)].0));
        let up = (b - a).cross(c - a).normalize_or_zero() * LIFT;
        for (p, _) in &mut screen {
            *p = (Vec3::from_array(*p) + up).to_array();
        }
    }
    Ok(Device {
        name: name.into(),
        corners,
        triangles,
        edges,
        screen,
        screen_indices,
    })
}

/// Every device, compiled and encoded.
pub fn compiled() -> Result<Vec<u8>, String> {
    let root = root();
    let devices = SOURCES
        .iter()
        .map(|(name, path)| compile(name, &root.join(path)))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(crate::devices::encode(&devices))
}

#[test]
fn the_pinned_devices_are_compiled_from_the_blender_export() {
    let bytes = compiled().unwrap_or_else(|e| panic!("{e}"));
    let path = root().join(PINNED_PATH);
    if std::env::var_os("UPDATE_FIXTURES").is_some() {
        std::fs::write(&path, &bytes).unwrap_or_else(|e| panic!("{e}"));
    }
    let pinned = std::fs::read(&path).unwrap_or_else(|e| panic!("{e}"));
    assert!(
        pinned == bytes,
        "{PINNED_PATH} is stale: run UPDATE_FIXTURES=1 cargo test -p grid-help-desk"
    );
}
