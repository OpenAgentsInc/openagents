//! The collision bake for a mesh from the asset pipeline (#11657).
//! Usage: collision_bake MODEL.obj OUT.json [CELL_M]
//!
//! Reads a closed triangle mesh as Wavefront OBJ in metres (Blender:
//! File > Export > Wavefront, triangulated, +Y up), splits it into convex
//! pieces with CoACD (offline, `pip install coacd`, MIT), and writes the
//! pieces as `collision.v1` JSON beside a `.sdf` distance grid of the
//! mesh, the same output the CAD `collision` export writes for a part.
use std::path::PathBuf;

use glam::DVec3;
use serde_json::json;

fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let input = PathBuf::from(args.next().ok_or("Expected a mesh, MODEL.obj")?);
    let output = PathBuf::from(args.next().ok_or("Expected an output, OUT.json")?);
    let mut options = cad::collision::Options::default();
    if let Some(cell) = args.next() {
        options.cell = cell
            .parse()
            .map_err(|_| format!("{cell} is not a cell size in m"))?;
    }
    let text = std::fs::read_to_string(&input).map_err(|e| format!("{}: {e}", input.display()))?;
    let mut vertices = Vec::new();
    let mut triangles = Vec::new();
    for line in text.lines() {
        let mut words = line.split_whitespace();
        match words.next() {
            Some("v") => {
                let p: Vec<f64> = words.take(3).filter_map(|w| w.parse().ok()).collect();
                if p.len() == 3 {
                    vertices.push(DVec3::new(p[0], p[1], p[2]));
                }
            }
            Some("f") => {
                // `f a b c ...` with optional /texture/normal; fans polygons.
                let ids: Vec<u32> = words
                    .filter_map(|w| w.split('/').next()?.parse::<u32>().ok())
                    .map(|i| i - 1)
                    .collect();
                for k in 1..ids.len().saturating_sub(1) {
                    triangles.push([ids[0], ids[k], ids[k + 1]]);
                }
            }
            _ => {}
        }
    }
    let split = cad::collision::decompose(&vertices, &triangles, &options.coacd, &options.python)?;
    let hulls: Vec<Vec<[f64; 3]>> = split
        .pieces
        .iter()
        .filter_map(|piece| physics::ConvexHull::new(piece).ok())
        .map(|hull| hull.vertices().iter().map(|p| p.to_array()).collect())
        .collect();
    let span = vertices
        .iter()
        .fold((DVec3::INFINITY, DVec3::NEG_INFINITY), |(lo, hi), v| {
            (lo.min(*v), hi.max(*v))
        });
    let finest = (span.1 - span.0 + DVec3::splat(2.0 * options.pad)).max_element()
        / f64::from(physics::SdfGrid::MAX_AXIS - 2);
    let grid = physics::SdfGrid::bake(&vertices, &triangles, options.cell.max(finest), options.pad)
        .map_err(|e| e.to_string())?;
    let sdf = output.with_extension("sdf");
    std::fs::write(&sdf, grid.to_bytes()).map_err(|e| e.to_string())?;
    let report = json!({
        "format": cad::collision::FORMAT,
        "units": "m",
        "source": input.file_name().map(|n| n.to_string_lossy()),
        "decomposition": {
            "tool": "CoACD",
            "version": split.version,
            "license": "MIT",
            "source": "https://github.com/SarahWeiii/CoACD",
            "params": options.coacd,
        },
        "solids": [{ "hulls": hulls }],
        "sdf": {
            "file": sdf.file_name().map(|n| n.to_string_lossy()),
            "version": physics::SdfGrid::VERSION,
            "cell": grid.cell(),
            "dims": grid.dims(),
            "origin": grid.origin().to_array(),
            "sha256": grid.hash(),
        },
    });
    let text = serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?;
    std::fs::write(&output, text).map_err(|e| e.to_string())?;
    eprintln!(
        "{} convex pieces, a {:?} grid; wrote {} and {}",
        hulls.len(),
        grid.dims(),
        output.display(),
        sdf.display()
    );
    Ok(())
}
