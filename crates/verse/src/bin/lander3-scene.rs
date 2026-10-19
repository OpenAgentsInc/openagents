//! Exports the Grid scene and sampled robot animations for `/lander3`.
//!
//! Regenerate after changing the Grid pack, gates, patrols, or atmosphere:
//! `cargo run -p verse --no-default-features --bin lander3-scene`.

use std::collections::BTreeMap;
use std::path::PathBuf;

use base64::Engine as _;
use glam::{Mat4, Quat};
use serde::Serialize;
use verse::grid_pack;
use verse::grid_robot::{self, Pose};
use verse::zones::{Gate, gate};
use verse_engine::assets::{Model, Pack, Topology};
use verse_engine::motion::Selection;

/// Skin samples per second for the robots' clips and patrols.
const RATE: f32 = 24.0;

fn main() -> Result<(), String> {
    let destination = std::env::args_os().nth(1).map_or_else(
        || {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../openagents-web/static/lander3-scene.json")
        },
        PathBuf::from,
    );
    let json = serde_json::to_string(&scene()?).map_err(|error| error.to_string())?;
    std::fs::write(&destination, json)
        .map_err(|error| format!("cannot write {}: {error}", destination.display()))?;
    println!("Wrote the Grid scene to {}", destination.display());
    Ok(())
}

/// Everything the page draws.
#[derive(Serialize)]
struct Scene {
    /// The source revision of the pinned Grid pack.
    source_revision: String,
    /// The Grid's clear color, linear RGB.
    background: [f32; 3],
    /// Each pack texture's address, by its pack slot.
    textures: Vec<String>,
    models: BTreeMap<String, ModelOut>,
    /// The pack's placements: the floor, the Gym, its boards.
    statics: Vec<Placed>,
    /// The arches the Grid stands, and the Everglade portal's view.
    gates: Vec<Placed>,
    robots: Robots,
}

#[derive(Serialize)]
struct ModelOut {
    /// Bones in the skin, or zero for a rigid model.
    bones: usize,
    surfaces: Vec<SurfaceOut>,
}

/// One surface: unlit, its texture times its tint. Arrays are base64 of
/// little-endian values.
#[derive(Serialize)]
struct SurfaceOut {
    lines: bool,
    tint: [f32; 3],
    texture: usize,
    /// `f32` x, y, z per vertex.
    positions: String,
    /// `f32` u, v per vertex.
    uvs: String,
    /// `u32` per index.
    indices: String,
    /// `u16` four joints per vertex, for a skinned model.
    #[serde(skip_serializing_if = "Option::is_none")]
    joints: Option<String>,
    /// `f32` four weights per vertex, for a skinned model.
    #[serde(skip_serializing_if = "Option::is_none")]
    weights: Option<String>,
}

#[derive(Serialize)]
struct Placed {
    model: String,
    /// Column-major, as three.js `Matrix4.fromArray` reads it.
    matrix: [f32; 16],
}

#[derive(Serialize)]
struct Robots {
    /// The model drawn near the eye, and the one beyond `far_meters`.
    near: &'static str,
    far: &'static str,
    far_meters: f32,
    scale: f32,
    /// Samples per second, for the clips and the patrols.
    rate: f32,
    /// The clips the robots play, by name; `patrols` names them by index.
    clips: Vec<String>,
    /// Per model, per clip in `clips`: its length in seconds and its skin
    /// matrices, 16 `f32` per bone per sample.
    poses: BTreeMap<&'static str, Vec<ClipOut>>,
    /// Each robot's round: its seconds and its samples, five `f32` each
    /// (x, z, heading, clip index, clip time).
    patrols: Vec<Patrol>,
}

#[derive(Serialize)]
struct ClipOut {
    duration: f32,
    frames: String,
}

#[derive(Serialize)]
struct Patrol {
    round: f32,
    samples: String,
}

fn base64_of<T: Copy>(values: &[T], bytes: impl Fn(T) -> Vec<u8>) -> String {
    let raw: Vec<u8> = values.iter().flat_map(|v| bytes(*v)).collect();
    base64::engine::general_purpose::STANDARD.encode(raw)
}

fn floats(values: &[f32]) -> String {
    base64_of(values, |v| v.to_le_bytes().to_vec())
}

fn model_out(model: &Model) -> ModelOut {
    let skinned = model.skin.is_some();
    let surfaces = model
        .surfaces
        .iter()
        .map(|surface| {
            let positions: Vec<f32> = surface.vertices.iter().flat_map(|v| v.position).collect();
            let uvs: Vec<f32> = surface.vertices.iter().flat_map(|v| v.uv).collect();
            SurfaceOut {
                lines: matches!(surface.topology, Topology::Lines),
                tint: surface.tint,
                texture: surface.texture,
                positions: floats(&positions),
                uvs: floats(&uvs),
                indices: base64_of(&surface.indices, |v| v.to_le_bytes().to_vec()),
                joints: skinned.then(|| {
                    let joints: Vec<u16> = surface
                        .vertices
                        .iter()
                        .flat_map(|v| v.joints.map(|j| j as u16))
                        .collect();
                    base64_of(&joints, |v| v.to_le_bytes().to_vec())
                }),
                weights: skinned.then(|| {
                    let weights: Vec<f32> =
                        surface.vertices.iter().flat_map(|v| v.weights).collect();
                    floats(&weights)
                }),
            }
        })
        .collect();
    ModelOut {
        bones: model.skin.as_ref().map_or(0, |skin| skin.names.len()),
        surfaces,
    }
}

/// The arches a Grid with zone storage stands: each one its switch in
/// `zones::gate` opens, with the Everglade portal's view in its arch. The
/// RITUAL arch needs a desktop's pinned chamber, so it has none here.
fn gates() -> Vec<Placed> {
    let layout = verse::blocks::Layout::grid();
    let everglade = Gate::everglade(&layout);
    [
        (
            grid_pack::ARCH_LAGRANGE,
            gate::GRID_PORTAL_OPEN,
            Gate::grid(&layout),
        ),
        (
            grid_pack::ARCH_EVERGLADE,
            gate::GRID_EVERGLADE_OPEN,
            everglade,
        ),
        (
            grid_pack::PORTAL_EVERGLADE,
            gate::GRID_EVERGLADE_OPEN,
            everglade,
        ),
        (
            grid_pack::ARCH_LAGRANGE2,
            gate::GRID_LAGRANGE2_OPEN,
            Gate::lagrange2(&layout),
        ),
    ]
    .into_iter()
    .filter(|(_, open, _)| *open)
    .map(|(model, _, gate)| Placed {
        model: model.into(),
        matrix: Mat4::from_rotation_translation(Quat::from_rotation_y(gate.yaw), gate.at)
            .to_cols_array(),
    })
    .collect()
}

/// A clip's length: the clip its selection plays on `model`.
fn duration(model: &Model, selection: Selection) -> Option<f32> {
    let clip = match selection {
        Selection::Named(state) => model.states.get(&state)?.clip,
        Selection::Legacy(id) => id,
    };
    model
        .clips
        .iter()
        .find(|c| c.id == clip)
        .map(|c| c.duration)
}

fn samples(seconds: f32) -> usize {
    ((seconds * RATE).ceil() as usize).max(1)
}

fn robots(pack: &Pack) -> Result<Robots, String> {
    // Each robot's round: the four patrols, then the dancer, whose loop is
    // its dance.
    let mut rounds: Vec<(f32, Box<dyn Fn(f64) -> Pose>)> = (0..grid_robot::ROUTES.len())
        .map(|route| {
            let walk: Box<dyn Fn(f64) -> Pose> =
                Box::new(move |clock| grid_robot::patroller(route, clock));
            (grid_robot::round_seconds(route), walk)
        })
        .collect();
    rounds.push((grid_robot::DANCE_SECONDS, Box::new(grid_robot::dancer)));

    let mut clips: Vec<Selection> = Vec::new();
    let mut patrols = Vec::new();
    for (round, pose) in &rounds {
        let mut values = Vec::new();
        for n in 0..samples(*round) {
            let pose = pose(f64::from(n as f32 / RATE));
            let index = match clips.iter().position(|c| *c == pose.animation) {
                Some(index) => index,
                None => {
                    clips.push(pose.animation);
                    clips.len() - 1
                }
            };
            values.extend([pose.pos.x, pose.pos.z, pose.yaw, index as f32, pose.time]);
        }
        patrols.push(Patrol {
            round: *round,
            samples: floats(&values),
        });
    }

    let mut poses = BTreeMap::new();
    for name in [grid_robot::ROBOT, grid_robot::ROBOT_FAR] {
        let model = pack
            .models
            .get(name)
            .ok_or_else(|| format!("the pack has no {name}"))?;
        let mut out = Vec::new();
        for selection in &clips {
            let duration = duration(model, *selection)
                .ok_or_else(|| format!("{name} has no clip for {selection:?}"))?;
            let mut frames = Vec::new();
            for n in 0..samples(duration) {
                let matrices = verse_engine::animation::pose_selected(
                    model,
                    *selection,
                    (n as f32 / RATE).min(duration),
                )?;
                frames.extend(matrices.iter().flat_map(Mat4::to_cols_array));
            }
            out.push(ClipOut {
                duration,
                frames: floats(&frames),
            });
        }
        poses.insert(name, out);
    }

    Ok(Robots {
        near: grid_robot::ROBOT,
        far: grid_robot::ROBOT_FAR,
        far_meters: grid_robot::FAR_METERS,
        scale: grid_robot::SCALE,
        rate: RATE,
        clips: clips.iter().map(|c| format!("{c:?}")).collect(),
        poses,
        patrols,
    })
}

/// The Grid as the page draws it.
fn scene() -> Result<Scene, String> {
    let pack = grid_pack::embedded()?;
    let statics = grid_pack::placements(&pack)
        .into_iter()
        .map(|instance| Placed {
            model: instance.model,
            matrix: instance.transform.to_cols_array(),
        })
        .collect();
    let gates = gates();
    // The models the page places: the statics, the arches, the robots. The
    // figure and the spade stand for players and agents, who are not here.
    let robot_models = [grid_robot::ROBOT, grid_robot::ROBOT_FAR];
    let models = pack
        .models
        .iter()
        .filter(|(name, _)| {
            pack.placements.iter().any(|p| &p.model == *name)
                || gates.iter().any(|g| &g.model == *name)
                || robot_models.contains(&name.as_str())
        })
        .map(|(name, model)| (name.clone(), model_out(model)))
        .collect();
    Ok(Scene {
        source_revision: pack.source_revision.clone(),
        background: verse::runtime::WorldRuntime::bare().atmosphere().color,
        textures: pack
            .textures
            .iter()
            .map(|t| format!("/lander3/texture/{}", t.file))
            .collect(),
        models,
        statics,
        gates,
        robots: robots(&pack)?,
    })
}

#[cfg(test)]
mod tests {
    use base64::Engine as _;
    use serde_json::Value;

    use super::*;

    #[test]
    fn the_web_scene_matches_the_grid() {
        let generated = serde_json::to_string(&scene().unwrap()).unwrap();
        let generated: Value = serde_json::from_str(&generated).unwrap();
        let saved: Value = serde_json::from_str(include_str!(
            "../../../openagents-web/static/lander3-scene.json"
        ))
        .unwrap();
        compare(&generated, &saved, "$", false);
    }

    fn compare(actual: &Value, expected: &Value, path: &str, approximate: bool) {
        match (actual, expected) {
            (Value::Object(actual), Value::Object(expected)) => {
                assert_eq!(actual.len(), expected.len(), "{path}: field count");
                for (key, expected) in expected {
                    let path = format!("{path}.{key}");
                    let actual = actual.get(key).unwrap_or_else(|| panic!("missing {path}"));
                    if matches!(key.as_str(), "frames" | "samples") {
                        compare_samples(actual, expected, &path, key == "samples");
                    } else {
                        compare(
                            actual,
                            expected,
                            &path,
                            matches!(key.as_str(), "matrix" | "round"),
                        );
                    }
                }
            }
            (Value::Array(actual), Value::Array(expected)) => {
                assert_eq!(actual.len(), expected.len(), "{path}: item count");
                for (index, (actual, expected)) in actual.iter().zip(expected).enumerate() {
                    compare(actual, expected, &format!("{path}[{index}]"), approximate);
                }
            }
            (Value::Number(actual), Value::Number(expected))
                if approximate && actual.is_f64() && expected.is_f64() =>
            {
                close(actual.as_f64().unwrap(), expected.as_f64().unwrap(), path);
            }
            _ => assert!(
                actual == expected,
                "{path} differs; regenerate with cargo run -p verse --no-default-features --bin lander3-scene"
            ),
        }
    }

    fn compare_samples(actual: &Value, expected: &Value, path: &str, patrol: bool) {
        let decode = |value: &Value| {
            base64::engine::general_purpose::STANDARD
                .decode(value.as_str().unwrap())
                .unwrap()
        };
        let actual = decode(actual);
        let expected = decode(expected);
        assert_eq!(actual.len(), expected.len(), "{path}: sample byte count");
        assert_eq!(actual.len() % 4, 0, "{path}: incomplete float");
        for (index, (actual, expected)) in actual
            .chunks_exact(4)
            .zip(expected.chunks_exact(4))
            .enumerate()
        {
            let actual = f32::from_le_bytes(actual.try_into().unwrap());
            let expected = f32::from_le_bytes(expected.try_into().unwrap());
            let path = format!("{path}[{index}]");
            if patrol && index % 5 == 3 {
                assert_eq!(actual, expected, "{path}: clip index");
            } else {
                close(f64::from(actual), f64::from(expected), &path);
            }
        }
    }

    fn close(actual: f64, expected: f64, path: &str) {
        // Trigonometry can round differently across CPU architectures.
        let tolerance = 1e-5 * actual.abs().max(expected.abs()).max(1.0);
        assert!(
            actual.is_finite() && expected.is_finite() && (actual - expected).abs() <= tolerance,
            "{path}: {actual} differs from {expected}; regenerate with cargo run -p verse --no-default-features --bin lander3-scene"
        );
    }
}
