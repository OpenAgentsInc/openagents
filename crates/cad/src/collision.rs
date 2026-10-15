//! The collision bake: what Verse's physics needs from a built document
//! (#11657).
//!
//! - **Convex pieces** for each solid, so a non-convex part can be a
//!   dynamic body made of several hull colliders
//!   ([`physics::World::add_compound_body`]). The pieces come from CoACD
//!   (<https://github.com/SarahWeiii/CoACD>, MIT licence, Wei et al.,
//!   SIGGRAPH 2022), run offline as `python3` with the `coacd` package
//!   (`pip install coacd`); it is never linked into the engine. The tool's
//!   version and every parameter are written into the output.
//! - **A signed-distance grid** of all the solids together, for static
//!   non-convex geometry such as a container or terrain
//!   ([`physics::SdfGrid`]), in its versioned file format.
//!
//! The shape of the bake follows Genesis
//! (<https://github.com/Genesis-Embodied-AI/Genesis>, genesis-world
//! `df30b61a`, Apache-2.0): `utils/mesh.py` `convex_decompose` runs CoACD
//! and caches the pieces, and `utils/sdf.py` precomputes a grid per
//! geometry. It is credited here; none of its code is translated.
//!
//! Both are in metres (the document is in millimetres). The same document
//! and options give the same bytes: the grid bake is deterministic, and
//! CoACD runs with a fixed seed.

use glam::DVec3;
use serde::Serialize;
use serde_json::{Value, json};

use crate::build::Solid;

/// The output's format name.
pub const FORMAT: &str = "collision.v1";

/// CoACD's parameters, passed through by name (see its README).
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Coacd {
    /// Concavity threshold: smaller cuts finer, into more pieces.
    pub threshold: f64,
    /// At most this many pieces; -1 for no limit.
    pub max_convex_hull: i32,
    pub preprocess_mode: String,
    pub preprocess_resolution: u32,
    pub resolution: u32,
    pub mcts_nodes: u32,
    pub mcts_iterations: u32,
    pub mcts_max_depth: u32,
    pub merge: bool,
    pub max_ch_vertex: u32,
    pub seed: u32,
}

impl Default for Coacd {
    fn default() -> Self {
        Self {
            threshold: 0.05,
            max_convex_hull: 16,
            preprocess_mode: "auto".into(),
            preprocess_resolution: 50,
            resolution: 2000,
            mcts_nodes: 20,
            mcts_iterations: 150,
            mcts_max_depth: 3,
            merge: true,
            max_ch_vertex: 64,
            seed: 0,
        }
    }
}

/// How to bake.
#[derive(Clone, Debug, PartialEq)]
pub struct Options {
    pub coacd: Coacd,
    /// The Python that has the `coacd` package.
    pub python: String,
    /// Grid spacing, m; the grid is coarsened if it would pass the
    /// physics crate's resolution cap.
    pub cell: f64,
    /// Margin around the solids, m.
    pub pad: f64,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            coacd: Coacd::default(),
            python: std::env::var("OPENAGENTS_COACD_PYTHON").unwrap_or_else(|_| "python3".into()),
            cell: 0.005,
            pad: 0.02,
        }
    }
}

/// A solid's triangles in metres.
#[must_use]
pub fn mesh(solid: &Solid) -> (Vec<DVec3>, Vec<[u32; 3]>) {
    let vertices = solid
        .mesh
        .positions
        .iter()
        .map(|p| DVec3::new(p.x, p.y, p.z) * 1e-3)
        .collect();
    (vertices, solid.mesh.triangles.clone())
}

/// One CoACD run: the tool's version and the pieces' points.
#[derive(Clone, Debug, PartialEq)]
pub struct Decomposition {
    pub version: String,
    pub pieces: Vec<Vec<DVec3>>,
}

/// Split a closed mesh into convex pieces with CoACD.
///
/// # Errors
///
/// Python or the `coacd` package is missing, or CoACD failed.
#[cfg(not(target_arch = "wasm32"))]
pub fn decompose(
    vertices: &[DVec3],
    triangles: &[[u32; 3]],
    coacd: &Coacd,
    python: &str,
) -> Result<Decomposition, String> {
    use std::io::Write;
    use std::process::{Command, Stdio};
    let request = json!({
        "vertices": vertices.iter().map(|v| v.to_array()).collect::<Vec<_>>(),
        "triangles": triangles,
        "params": coacd,
    });
    let mut child = Command::new(python)
        .args(["-c", include_str!("coacd.py")])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("I couldn't start {python} for CoACD: {e}"))?;
    child
        .stdin
        .take()
        .ok_or("CoACD has no input")?
        .write_all(request.to_string().as_bytes())
        .map_err(|e| e.to_string())?;
    let out = child.wait_with_output().map_err(|e| e.to_string())?;
    if !out.status.success() {
        let why = String::from_utf8_lossy(&out.stderr);
        let hint = if why.contains("No module named") {
            " Install it with `pip install coacd`."
        } else {
            ""
        };
        return Err(format!("CoACD failed: {}{hint}", why.trim()));
    }
    let reply: Value = serde_json::from_slice(&out.stdout).map_err(|e| e.to_string())?;
    let pieces = reply["parts"]
        .as_array()
        .ok_or("CoACD returned no parts")?
        .iter()
        .map(|part| {
            part["vertices"]
                .as_array()
                .map(|vs| {
                    vs.iter()
                        .filter_map(|v| {
                            let a = v.as_array()?;
                            Some(DVec3::new(a[0].as_f64()?, a[1].as_f64()?, a[2].as_f64()?))
                        })
                        .collect()
                })
                .unwrap_or_default()
        })
        .collect();
    Ok(Decomposition {
        version: reply["version"].as_str().unwrap_or("unknown").to_owned(),
        pieces,
    })
}

/// The grid of every solid together, at `options.cell` or the finest
/// spacing within the physics crate's cap.
///
/// # Errors
///
/// No solid has triangles.
pub fn grid(solids: &[Solid], options: &Options) -> Result<physics::SdfGrid, String> {
    let mut vertices = Vec::new();
    let mut triangles = Vec::new();
    for solid in solids {
        let (v, t) = mesh(solid);
        let base = u32::try_from(vertices.len()).map_err(|e| e.to_string())?;
        vertices.extend(v);
        triangles.extend(t.iter().map(|t| t.map(|i| i + base)));
    }
    let (lo, hi) = vertices
        .iter()
        .fold((DVec3::INFINITY, DVec3::NEG_INFINITY), |(lo, hi), v| {
            (lo.min(*v), hi.max(*v))
        });
    let span = (hi - lo + DVec3::splat(2.0 * options.pad)).max_element();
    let finest = span / f64::from(physics::SdfGrid::MAX_AXIS - 2);
    physics::SdfGrid::bake(&vertices, &triangles, options.cell.max(finest), options.pad)
        .map_err(|e| e.to_string())
}

/// The bake: the decomposition of each solid as JSON, and the grid of them
/// all.
///
/// # Errors
///
/// CoACD is unavailable or failed, or the solids have no triangles.
#[cfg(not(target_arch = "wasm32"))]
pub fn bake(solids: &[Solid], options: &Options) -> Result<(Value, physics::SdfGrid), String> {
    let mut version = String::from("unknown");
    let mut out = Vec::new();
    for solid in solids {
        let (v, t) = mesh(solid);
        let split = decompose(&v, &t, &options.coacd, &options.python)?;
        version = split.version;
        // Each piece as the hull the engine will build from it: its
        // corners, in a fixed order.
        let mut hulls = Vec::new();
        let mut dropped = 0;
        for piece in &split.pieces {
            match physics::ConvexHull::new(piece) {
                Ok(hull) => hulls.push(
                    hull.vertices()
                        .iter()
                        .map(|p| p.to_array())
                        .collect::<Vec<_>>(),
                ),
                Err(_) => dropped += 1,
            }
        }
        out.push(json!({
            "instance": solid.instance,
            "part": solid.part,
            "body": solid.body,
            "hulls": hulls,
            "dropped_flat_pieces": dropped,
        }));
    }
    let grid = grid(solids, options)?;
    let report = json!({
        "format": FORMAT,
        "units": "m",
        "decomposition": {
            "tool": "CoACD",
            "version": version,
            "license": "MIT",
            "source": "https://github.com/SarahWeiii/CoACD",
            "params": options.coacd,
        },
        "solids": out,
        "sdf": {
            "version": physics::SdfGrid::VERSION,
            "cell": grid.cell(),
            "dims": grid.dims(),
            "origin": grid.origin().to_array(),
            "sha256": grid.hash(),
        },
    });
    Ok((report, grid))
}
