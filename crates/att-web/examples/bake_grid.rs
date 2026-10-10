//! Bakes the Grid robot for the `/att` scene: `cargo run -p att-web
//! --example bake_grid` writes `crates/att-web/assets/grid-robot.bin`.
//!
//! It reads the far level of detail (`grid/robot-far`) from the pinned Grid
//! pack (`assets/verse/grid/pack.json`, built by `crates/verse/src/
//! grid_robot.rs`), poses it in the first frame of its idle clip, and writes
//! its shaded facets and edge lines in the compact form `crate::robot`
//! reads, so the page draws the same robot without the engine.
//!
//! Format (little endian): `GRB1`, the triangle count and the line count
//! (u32 each), the lines' gray (one byte, sRGB), then each triangle as its
//! gray (one byte, sRGB) and three corners, then each line as two corners.
//! A corner is three i16 millimetres.

use std::path::Path;

use glam::Vec3;
use verse_engine::assets::{Pack, Topology};
use verse_engine::motion::{Selection, State};

fn srgb(linear: f32) -> u8 {
    let c = linear.clamp(0.0, 1.0);
    let s = if c <= 0.003_130_8 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    };
    (s * 255.0).round() as u8
}

fn corner(out: &mut Vec<u8>, p: Vec3) {
    for v in p.to_array() {
        let mm = (v * 1000.0).round().clamp(-32_000.0, 32_000.0) as i16;
        out.extend_from_slice(&mm.to_le_bytes());
    }
}

fn main() -> Result<(), String> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let json = std::fs::read_to_string(root.join("../../assets/verse/grid/pack.json"))
        .map_err(|e| e.to_string())?;
    let pack: Pack = serde_json::from_str(&json).map_err(|e| e.to_string())?;
    let model = pack
        .models
        .get("grid/robot-far")
        .ok_or("the Grid pack has no grid/robot-far")?;
    let matrices =
        verse_engine::animation::pose_selected(model, Selection::Named(State::Idle), 0.0)?;
    let posed = |position: [f32; 3], joint: u32| {
        matrices[joint as usize].transform_point3(Vec3::from_array(position))
    };
    let mut triangles = Vec::new();
    let mut lines = Vec::new();
    let mut line_gray = 128;
    let (mut tri_count, mut line_count) = (0u32, 0u32);
    for surface in &model.surfaces {
        let at = |i: u32| {
            let v = &surface.vertices[i as usize];
            posed(v.position, v.joints[0])
        };
        match surface.topology {
            Topology::Lines => {
                line_gray = srgb(surface.tint[0]);
                for pair in surface.indices.chunks_exact(2) {
                    corner(&mut lines, at(pair[0]));
                    corner(&mut lines, at(pair[1]));
                    line_count += 1;
                }
            }
            _ => {
                for tri in surface.indices.chunks_exact(3) {
                    triangles.push(srgb(surface.tint[0]));
                    for i in tri {
                        corner(&mut triangles, at(*i));
                    }
                    tri_count += 1;
                }
            }
        }
    }
    let mut out = b"GRB1".to_vec();
    out.extend_from_slice(&tri_count.to_le_bytes());
    out.extend_from_slice(&line_count.to_le_bytes());
    out.push(line_gray);
    out.extend(triangles);
    out.extend(lines);
    let path = root.join("assets/grid-robot.bin");
    std::fs::write(&path, &out).map_err(|e| e.to_string())?;
    println!(
        "{}: {tri_count} triangles, {line_count} lines, {} bytes",
        path.display(),
        out.len()
    );
    Ok(())
}
