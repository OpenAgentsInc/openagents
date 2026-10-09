//! Everglade's standing workbench and monitor in the Grid's flat palette.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use coder_ui::theme::Intensity;
use glam::{Mat4, Vec3};
use sha2::{Digest, Sha256};
use verse_engine::assets::Pack;

use crate::mesh::{Mesh, Vertex};
use crate::zones::everglade::{boards, layout::Board};

pub const MODEL: &str = "grid/workstation";
/// The three desks stand behind the spawn, clear of patrols and portals.
pub const SITES: [[f32; 3]; 3] = [[-4.0, 0.0, -26.0], [0.0, 0.0, -26.0], [4.0, 0.0, -26.0]];
/// Furniture scaled for the Grid robot's standing height.
pub const SCALE: f32 = 1.5;

fn source_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/verse/everglade/props")
}

/// Verifies the existing CC0 workbench sources and records their provenance.
pub fn sources() -> Result<(String, u64), String> {
    let dir = source_dir();
    let read = |name: &str| std::fs::read(dir.join(name)).map_err(|e| e.to_string());
    let manifest = read("manifest.json")?;
    let doc: serde_json::Value = serde_json::from_slice(&manifest).map_err(|e| e.to_string())?;
    if doc["license"] != "CC0-1.0" {
        return Err("The Grid workstation requires the admitted CC0 workbench".into());
    }
    let mut parts = vec![];
    for name in ["Workbench.gltf", "Workbench.bin"] {
        let bytes = read(name)?;
        if doc["files"][name].as_str() != Some(format!("{:x}", Sha256::digest(&bytes)).as_str()) {
            return Err(format!("{name} differs from the admitted workbench"));
        }
        parts.push(bytes);
    }
    parts.push(manifest);
    parts.push(read("license.txt")?);
    Ok(crate::imported::inventory::bundle(
        &parts.iter().map(Vec::as_slice).collect::<Vec<_>>(),
    ))
}

/// The original bench geometry, with Everglade's monitor and a keyboard.
pub fn mesh() -> Result<Mesh, String> {
    sources()?;
    let dir = source_dir();
    let gltf = gltf::Gltf::open(dir.join("Workbench.gltf")).map_err(|e| e.to_string())?;
    let buffer = std::fs::read(dir.join("Workbench.bin")).map_err(|e| e.to_string())?;
    let mut raw = Mesh::default();
    for node in gltf.nodes() {
        let Some(model) = node.mesh() else { continue };
        let transform = Mat4::from_scale(Vec3::splat(crate::zones::everglade::pose::DESK_SCALE))
            * Mat4::from_rotation_y(std::f32::consts::PI)
            * Mat4::from_cols_array_2d(&node.transform().matrix());
        for primitive in model.primitives() {
            let reader = primitive.reader(|b| (b.index() == 0).then_some(buffer.as_slice()));
            let positions: Vec<_> = reader
                .read_positions()
                .ok_or("The workbench has no positions")?
                .map(|p| transform.transform_point3(Vec3::from(p)))
                .collect();
            let indices = reader
                .read_indices()
                .ok_or("The workbench has no indices")?;
            for index in indices.into_u32() {
                raw.faces.push(Vertex {
                    pos: positions[index as usize].to_array(),
                    color: crate::palette::field(),
                    fog: 1.0,
                });
            }
        }
    }
    let monitor = Board {
        center: Vec3::new(0.0, 1.4, 0.3),
        facing: std::f32::consts::PI,
        size: [0.84, 0.5],
    };
    boards::monitor(&mut raw, &monitor);
    let mut mesh = outlines(raw);
    mesh.cube(
        Mat4::from_translation(Vec3::new(0.0, 1.025, -0.25))
            * Mat4::from_scale(Vec3::new(0.7, 0.04, 0.26)),
        Intensity::Half,
    );
    for row in 0..3 {
        for key in 0..10 {
            mesh.cube(
                Mat4::from_translation(Vec3::new(
                    -0.3 + key as f32 * 0.066,
                    1.05,
                    -0.32 + row as f32 * 0.065,
                )) * Mat4::from_scale(Vec3::new(0.045, 0.006, 0.04)),
                Intensity::Quarter,
            );
        }
    }
    // Abstract code strokes keep the screens free of lettering.
    for (row, length) in [0.44, 0.32, 0.5, 0.38, 0.24].into_iter().enumerate() {
        let y = 1.55 - row as f32 * 0.075;
        let x = -0.3 + (row % 2) as f32 * 0.04;
        mesh.line(
            Vec3::new(x, y, 0.29),
            Vec3::new(x + length, y, 0.29),
            Intensity::ThreeQuarters,
        );
    }
    mesh.neutralize();
    Ok(mesh)
}

/// Keeps silhouette and sharp edges without drawing triangle diagonals.
fn outlines(mut mesh: Mesh) -> Mesh {
    type Point = [i32; 3];
    let key = |p: Vec3| p.to_array().map(|v| (v * 100_000.0).round() as i32);
    let mut edges: BTreeMap<(Point, Point), (Vec3, Vec3, Vec<Vec3>)> = BTreeMap::new();
    for triangle in mesh.faces.chunks_exact_mut(3) {
        let [a, b, c] = [0, 1, 2].map(|i| Vec3::from(triangle[i].pos));
        let normal = (b - a).cross(c - a).normalize_or_zero();
        for v in triangle {
            v.color = crate::palette::field();
        }
        for (a, b) in [(a, b), (b, c), (c, a)] {
            let (ka, kb) = (key(a), key(b));
            let pair = if ka < kb { (ka, kb) } else { (kb, ka) };
            edges
                .entry(pair)
                .or_insert_with(|| (a, b, vec![]))
                .2
                .push(normal);
        }
    }
    for (_, (a, b, normals)) in edges {
        if normals.len() == 1 || normals.iter().any(|n| n.dot(normals[0]) < 0.866) {
            mesh.line(a, b, Intensity::ThreeQuarters);
        }
    }
    mesh
}

/// The baked desk placements on the legacy render path.
pub fn placed_mesh(pack: &Pack) -> Mesh {
    let mut out = Mesh::default();
    let Some(model) = pack.models.get(MODEL) else {
        return out;
    };
    for placed in crate::grid_pack::placements(pack)
        .into_iter()
        .filter(|p| p.model == MODEL)
    {
        for surface in &model.surfaces {
            let target = if surface.topology == verse_engine::assets::Topology::Lines {
                &mut out.lines
            } else {
                &mut out.faces
            };
            for index in &surface.indices {
                let v = &surface.vertices[*index as usize];
                target.push(Vertex {
                    pos: placed
                        .transform
                        .transform_point3(Vec3::from(v.position))
                        .to_array(),
                    color: surface.tint,
                    fog: 1.0,
                });
            }
        }
    }
    out
}
