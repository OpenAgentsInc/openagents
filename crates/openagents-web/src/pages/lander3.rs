//! `/lander3`: the Grid from overhead, drawn with three.js (#12180).
//!
//! `/grid` runs the Verse's wgpu renderer as wasm. This page draws the same
//! Grid with three.js instead, from the same sources: the server reads the
//! pinned Grid pack built into the `verse` crate (`assets/verse/grid/`), the
//! arches the Grid stands ([`verse::zones::Gate`] under the `zones::gate`
//! switches), the robots' patrols ([`verse::grid_robot`]), and the Grid's
//! clear color ([`verse::runtime::WorldRuntime::bare`]). It hands them to
//! the browser as one JSON file, `/lander3/scene.json`, with the robots'
//! skin matrices sampled by the engine's own [`verse_engine::animation`].
//!
//! ```text
//! /lander3                      the page: a canvas and its controls
//! /lander3/scene.json           the Grid, from the verse crate
//! /lander3/texture/{file}       the pack's textures, by their pack names
//! /lander3/three/{file}         three.js 0.184.0, served from this site
//! /lander3/lander3.js           the page's script (static/lander3.js)
//! /lander3/lander3.css          the page's controls
//! ```

use std::collections::BTreeMap;
use std::sync::OnceLock;

use axum::Router;
use axum::extract::Path as UrlPath;
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use base64::Engine as _;
use glam::{Mat4, Quat};
use serde::Serialize;
use verse::grid_pack;
use verse::grid_robot::{self, Pose};
use verse::zones::{Gate, gate};
use verse_engine::assets::{Model, Pack, Topology};
use verse_engine::motion::Selection;

use crate::App;
use crate::layout::escape;

/// The page's policy: the site's, with its module script and the
/// same-origin reads of the scene, the textures, and three.js. No wasm, no
/// inline script, nothing from another origin.
pub(crate) const LANDER3_POLICY: &str = "default-src 'none'; style-src 'self'; font-src 'self'; \
img-src 'self'; script-src 'self'; connect-src 'self'; \
base-uri 'none'; form-action 'self'; frame-ancestors 'none'";

/// The vendored three.js build: the module and the core it imports as
/// `./three.core.min.js` (`static/vendor/three-0.184.0/`).
const THREE: [(&str, &str); 2] = [
    (
        "three.module.min.js",
        include_str!("../../static/vendor/three-0.184.0/three.module.min.js"),
    ),
    (
        "three.core.min.js",
        include_str!("../../static/vendor/three-0.184.0/three.core.min.js"),
    ),
];

/// The pinned pack's texture files, by the names its manifest gives them.
/// A test holds each to the manifest's SHA-256.
const TEXTURES: [(&str, &[u8]); 2] = [
    (
        "verse-flat-white.png",
        include_bytes!("../../../../assets/verse/grid/verse-flat-white.png"),
    ),
    (
        grid_pack::PORTAL_VIEW,
        include_bytes!("../../../../assets/verse/grid/everglade-portal.png"),
    ),
];

const SCRIPT: &str = include_str!("../../static/lander3.js");
const STYLES: &str = include_str!("../../static/lander3.css");

/// Skin samples per second for the robots' clips and patrols.
const RATE: f32 = 24.0;

/// Static files: their name is not versioned, so they are cached briefly.
const CACHE: &str = "public, max-age=300";

pub(crate) fn routes() -> Router<App> {
    Router::new()
        .route("/lander3", get(page))
        .route("/lander3/scene.json", get(scene_file))
        .route("/lander3/texture/{file}", get(texture))
        .route("/lander3/three/{file}", get(three))
        .route("/lander3/lander3.js", get(script))
        .route("/lander3/lander3.css", get(styles))
}

/// The page: the canvas filling the window, the controls over its corner,
/// and the status line over its foot. The heading is for screen readers.
fn document() -> String {
    let title = escape("The Grid from above");
    format!(
        "<!doctype html><html lang=\"en\" class=\"stage\"><head><meta charset=\"utf-8\">\
<meta name=\"viewport\" content=\"width=device-width, initial-scale=1, viewport-fit=cover\">\
<meta name=\"color-scheme\" content=\"dark\"><title>{title} \u{b7} OpenAgents</title>\
<link rel=\"icon\" type=\"image/svg+xml\" href=\"/favicon.svg\">\
<link rel=\"stylesheet\" href=\"/static/legacy-demo.css\">\
<link rel=\"stylesheet\" href=\"/lander3/lander3.css\"></head><body><main id=\"content\">\
<h1 class=\"unseen\">{title}</h1>\
<div class=\"glade\" id=\"lander3\" data-scene=\"/lander3/scene.json\">\
<canvas id=\"lander3-canvas\" tabindex=\"0\" aria-label=\"The Grid seen from above. \
Drag to look around; arrow keys move, Q and E turn, R and F tilt, plus and minus zoom.\"></canvas>\
<div class=\"lander3-panel\"><p class=\"lander3-title\">The Grid from above</p>\
<p class=\"lander3-hint\">Drag to look around. Right-drag, Shift-drag, or two fingers to move. \
Scroll or pinch to zoom.</p>\
<div class=\"lander3-buttons\">\
<button type=\"button\" id=\"lander3-in\" disabled>Zoom in</button>\
<button type=\"button\" id=\"lander3-out\" disabled>Zoom out</button>\
<button type=\"button\" id=\"lander3-fit\" disabled>Show everything</button>\
<button type=\"button\" id=\"lander3-above\" disabled>Look from above</button>\
<button type=\"button\" id=\"lander3-robots\" aria-pressed=\"false\" disabled>Pause robots</button>\
</div><p class=\"lander3-hint\"><a href=\"/grid\">Walk the Grid</a></p></div>\
<p class=\"glade-status\" id=\"lander3-status\" aria-live=\"polite\">Loading the Grid…</p>\
<noscript><p class=\"glade-status\">Turn on JavaScript to see the Grid.</p></noscript></div>\
<script type=\"module\" src=\"/lander3/lander3.js\"></script></main></body></html>"
    )
}

async fn page() -> Response {
    let mut response = axum::response::Html(document()).into_response();
    response.headers_mut().insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(LANDER3_POLICY),
    );
    response
}

async fn scene_file() -> Response {
    match tokio::task::spawn_blocking(scene_json).await {
        Ok(Ok(json)) => (
            [
                (header::CONTENT_TYPE, "application/json"),
                (header::CACHE_CONTROL, CACHE),
            ],
            json.as_str(),
        )
            .into_response(),
        _ => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

async fn texture(UrlPath(file): UrlPath<String>) -> Response {
    match TEXTURES.iter().find(|(name, _)| *name == file) {
        Some((_, bytes)) => (
            [
                (header::CONTENT_TYPE, "image/png"),
                (header::CACHE_CONTROL, CACHE),
            ],
            *bytes,
        )
            .into_response(),
        None => crate::not_found().await,
    }
}

async fn three(UrlPath(file): UrlPath<String>) -> Response {
    match THREE.iter().find(|(name, _)| *name == file) {
        Some((_, text)) => javascript(text),
        None => crate::not_found().await,
    }
}

async fn script() -> Response {
    javascript(SCRIPT)
}

async fn styles() -> Response {
    (
        [
            (header::CONTENT_TYPE, "text/css; charset=utf-8"),
            (header::CACHE_CONTROL, CACHE),
        ],
        STYLES,
    )
        .into_response()
}

fn javascript(text: &'static str) -> Response {
    (
        [
            (header::CONTENT_TYPE, "text/javascript; charset=utf-8"),
            (header::CACHE_CONTROL, CACHE),
        ],
        text,
    )
        .into_response()
}

/// The scene, built once.
fn scene_json() -> Result<&'static String, &'static String> {
    static SCENE: OnceLock<Result<String, String>> = OnceLock::new();
    SCENE
        .get_or_init(|| {
            let scene = scene()?;
            serde_json::to_string(&scene).map_err(|e| e.to_string())
        })
        .as_ref()
}

/// Everything the page draws.
#[derive(Serialize)]
struct Scene {
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
    use sha2::{Digest, Sha256};

    use super::*;

    #[test]
    fn the_scene_comes_from_the_pinned_pack() {
        let pack = grid_pack::embedded().unwrap();
        let scene = scene().unwrap();
        assert_eq!(scene.statics.len(), pack.placements.len());
        for placed in scene.statics.iter().chain(&scene.gates) {
            assert!(scene.models.contains_key(&placed.model), "{}", placed.model);
        }
        assert!(
            scene
                .gates
                .iter()
                .any(|g| g.model == grid_pack::ARCH_EVERGLADE)
        );
        assert!(
            scene
                .gates
                .iter()
                .any(|g| g.model == grid_pack::ARCH_LAGRANGE2)
        );
        assert_eq!(scene.textures.len(), pack.textures.len());
        // Every robot walks; each clip has a pose for each model.
        assert_eq!(scene.robots.patrols.len(), grid_robot::ROUTES.len() + 1);
        for clips in scene.robots.poses.values() {
            assert_eq!(clips.len(), scene.robots.clips.len());
        }
    }

    #[test]
    fn the_textures_are_the_packs_own() {
        let pack = grid_pack::embedded().unwrap();
        assert_eq!(TEXTURES.len(), pack.textures.len());
        for texture in &pack.textures {
            let (_, bytes) = TEXTURES
                .iter()
                .find(|(name, _)| *name == texture.file)
                .unwrap();
            assert_eq!(format!("{:x}", Sha256::digest(bytes)), texture.sha256);
        }
    }

    #[test]
    fn the_policy_allows_only_this_site() {
        assert!(LANDER3_POLICY.starts_with("default-src 'none'"));
        assert!(LANDER3_POLICY.contains("script-src 'self';"));
        assert!(!LANDER3_POLICY.contains("unsafe"));
        assert!(!LANDER3_POLICY.contains("http"));
    }

    #[test]
    fn three_imports_only_its_vendored_core() {
        let (_, module) = THREE[0];
        assert!(module.contains("from\"./three.core.min.js\""));
        assert!(!module.contains("http://") && !module.contains("https://cdn"));
    }
}
