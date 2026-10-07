//! The Grid robot: an original black-and-white robot on the Universal rig
//! (`docs/verse/grid-robot.md`), the four patrols it walks around the Grid's
//! plaza, and a fifth that dances beside the Everglade arch.
//!
//! `scripts/blender/grid_robot.py` builds it from rigid parts, each bound to
//! one joint, and `scripts/blender/grid_robot_admit.py` admits a near and a
//! far level of detail under [`SOURCES`]. [`model`] turns a level into the
//! Grid's flat look: its armor as near-black facets in four shades, its
//! sharp panel edges as gray lines, and its glow parts as white, every
//! vertex skinned to its joint so the Universal Animation Library's clips
//! play on it.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use coder_ui::theme::Intensity;
use glam::{DMat4, DVec3, Mat4, Quat, Vec3};
use sha2::{Digest, Sha256};
use verse_engine::animation_graph::Authored;
use verse_engine::assets::{BoneKeys, Model, Pack, Surface, Topology, Vertex};
use verse_engine::motion::{Binding, Mode, Selection, State};

use crate::imported::{characters, flat};
use crate::palette;

/// The near model, feet at the origin facing +Z.
pub const ROBOT: &str = "grid/robot";
/// The far level of detail, drawn beyond [`FAR_METERS`] from the eye.
pub const ROBOT_FAR: &str = "grid/robot-far";
/// The admitted sources, relative to the repository root.
pub const SOURCES: &str = "assets/verse/characters/original/grid-robot";
/// The Universal Animation Library's gaits, relative to the repository root.
pub const GAITS: &str = "assets/verse/characters/quaternius/gaits.glb";
/// Eye distance beyond which the far level is drawn, in meters.
pub const FAR_METERS: f32 = 30.0;

pub const CLIP_IDLE: u16 = 0;
pub const CLIP_WALK: u16 = 2;
pub const CLIP_RUN: u16 = 3;
pub const CLIP_AIRBORNE: u16 = 4;
/// Dancing in place, on a loop.
pub const CLIP_DANCE: u16 = 5;
/// Each clip and the Universal clip it plays.
const CLIPS: [(u16, &str); 5] = [
    (CLIP_IDLE, "Idle_Loop"),
    (CLIP_WALK, "Walk_Loop"),
    (CLIP_RUN, "Jog_Fwd_Loop"),
    (CLIP_AIRBORNE, "Jump_Loop"),
    (CLIP_DANCE, "Dance_Loop"),
];
/// The loops whose last key is set to their first, so they cycle cleanly.
const LOOPS: [u16; 5] = [CLIP_IDLE, CLIP_WALK, CLIP_RUN, CLIP_AIRBORNE, CLIP_DANCE];
/// `Walk_Loop`'s length, s, and the ground its stride covers in that time,
/// m, as `characters::loop_distance` gives it.
pub const WALK_LOOP_SECONDS: f32 = 4.0 / 3.0;
pub const WALK_LOOP_METERS: f32 = 1.3;
/// `Dance_Loop`'s length, s.
pub const DANCE_SECONDS: f32 = 1.0;

/// Rotation tolerance when thinning a clip's keys, in quaternion components.
const ROTATION_TOLERANCE: f64 = 1.2e-3;
/// Translation tolerance when thinning a clip's keys, m.
const TRANSLATION_TOLERANCE: f64 = 3e-4;
/// Facets whose normals turn by more than this angle draw a line on their
/// shared edge: the cosine of 30 degrees, so the helmet's ten facets draw.
const EDGE_COSINE: f64 = 0.866;
/// The facet shades: the light direction each facet's shade is taken from,
/// and the four sRGB shades of armor and trim from shadow to light. The
/// second armor shade is the terminal's near-black tint.
const LIGHT: [f64; 3] = [0.35, 0.8, 0.5];
const ARMOR: [u32; 4] = [0x0c0c0d, 0x131314, 0x1a1a1a, 0x232325];
const TRIM: [u32; 4] = [0x1c1c1e, 0x262628, 0x313133, 0x3d3d40];

/// The admitted sources' directory, found from this crate.
#[must_use]
pub fn sources_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(SOURCES)
}

fn gaits() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(GAITS)
}

/// The robot's sources as the pack's inventory records them: every file the
/// manifest pins, checked against its digest, then the manifest and the
/// gaits. Returns the bundle's digest and size.
pub fn sources() -> Result<(String, u64), String> {
    let dir = sources_dir();
    let read = |path: &Path| std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()));
    let manifest = read(&dir.join("manifest.json"))?;
    let doc: serde_json::Value = serde_json::from_slice(&manifest).map_err(|e| e.to_string())?;
    if doc["schema"] != "openagents.verse.character-sources.v1" || doc["license"] != "CC0-1.0" {
        return Err("The Grid robot's manifest is not a CC0 character source".into());
    }
    let files = doc["files"]
        .as_object()
        .ok_or("The Grid robot's manifest lists no files")?;
    let mut parts = Vec::new();
    for (name, digest) in files {
        let bytes = read(&dir.join(name))?;
        if Some(format!("{:x}", Sha256::digest(&bytes)).as_str()) != digest.as_str() {
            return Err(format!("{name} differs from the Grid robot's manifest"));
        }
        parts.push(bytes);
    }
    parts.push(manifest);
    parts.push(read(&gaits())?);
    let parts: Vec<&[u8]> = parts.iter().map(Vec::as_slice).collect();
    Ok(crate::imported::inventory::bundle(&parts))
}

/// The robot's `variant` (`lod0` or `lod1`) as a Grid model named `name`:
/// flat surfaces on the white texel at `white`, skinned to the Universal
/// rig, with the idle, walk, run, airborne, and dance clips.
pub fn model(dir: &Path, white: usize, variant: &str, name: &str) -> Result<Model, String> {
    if !["lod0", "lod1"].contains(&variant) {
        return Err("Unknown Grid robot variant".into());
    }
    let mut scratch = Pack {
        inventory: None,
        version: 1,
        source_revision: String::new(),
        models: BTreeMap::new(),
        textures: Vec::new(),
        placements: Vec::new(),
    };
    let path = sources_dir().join(format!("grid-robot.{variant}.gltf"));
    let mut imported = characters::import(&mut scratch, dir, &path)?;
    for (id, clip) in CLIPS {
        characters::retarget_clip(&mut imported, &gaits(), id, clip)?;
    }
    let skin = imported.skin.as_mut().ok_or("The Grid robot has no skin")?;
    // The importer stores vertices in the chamber's basis; the Grid draws
    // in the sources' own frame (meters, +Y up, facing +Z).
    let inverse = DMat4::from_cols_array(&skin.basis.map(f64::from)).inverse();
    skin.basis = Mat4::IDENTITY.to_cols_array();
    let rest: Vec<_> = skin.rest.clone();
    let mut clips = std::mem::take(&mut imported.clips);
    for clip in &mut clips {
        if LOOPS.contains(&clip.id) {
            close_loop(&mut clip.bones);
        }
        for keys in &mut clip.bones {
            let at = rest[keys.bone];
            thin(&mut keys.rotation, at.rotation, ROTATION_TOLERANCE);
            thin(&mut keys.translation, at.translation, TRANSLATION_TOLERANCE);
            thin(&mut keys.scale, at.scale, TRANSLATION_TOLERANCE);
        }
        clip.bones
            .retain(|k| !(k.rotation.is_empty() && k.translation.is_empty() && k.scale.is_empty()));
    }
    let surfaces = flatten(&imported.surfaces, inverse, white)?;
    let height = surfaces
        .iter()
        .flat_map(|s| s.vertices.iter())
        .map(|v| v.position[1])
        .fold(0.0, f32::max);
    let bind = |clip: u16| Binding {
        clip,
        mode: Mode::Loop,
        transition_seconds: 0.2,
    };
    let mut model = Model {
        graph: None,
        markers: vec![],
        states: BTreeMap::from([
            (State::Idle, bind(CLIP_IDLE)),
            (State::Walk, bind(CLIP_WALK)),
            (State::Run, bind(CLIP_RUN)),
            (State::Airborne, bind(CLIP_AIRBORNE)),
        ]),
        skin: imported.skin,
        source: name.into(),
        source_sha256: String::new(),
        surfaces,
        bones: imported.bones,
        clips,
        height,
        attachments: vec![],
    };
    model.graph = Some(Authored::from_bindings(&model));
    model.validate_animation()?;
    Ok(model)
}

/// Sets each track's last key to its first, so a loop closes exactly.
fn close_loop(bones: &mut [BoneKeys]) {
    fn close<const N: usize>(keys: &mut [(f32, [f32; N])]) {
        if let Some(first) = keys.first().map(|k| k.1) {
            keys.last_mut().unwrap().1 = first;
        }
    }
    for keys in bones {
        close(&mut keys.translation);
        close(&mut keys.rotation);
        close(&mut keys.scale);
    }
}

/// Drops the keys a straight line between their neighbors reproduces within
/// `tolerance`, and the whole track when every key holds `rest`.
fn thin<const N: usize>(keys: &mut Vec<(f32, [f32; N])>, rest: [f32; N], tolerance: f64) {
    let near = |a: [f32; N], b: [f32; N]| {
        a.iter()
            .zip(b)
            .all(|(x, y)| (f64::from(*x) - f64::from(y)).abs() <= tolerance)
    };
    if keys.iter().all(|k| near(k.1, rest)) {
        keys.clear();
        return;
    }
    if keys.len() < 3 {
        return;
    }
    let mut kept = vec![keys[0]];
    let mut anchor = 0;
    let mut end = 1;
    while end < keys.len() {
        let next = end + 1;
        let fits = next < keys.len()
            && (anchor + 1..next).all(|i| {
                let (t0, a) = keys[anchor];
                let (t1, b) = keys[next];
                let f = (f64::from(keys[i].0) - f64::from(t0)) / (f64::from(t1) - f64::from(t0));
                let line: [f32; N] = std::array::from_fn(|c| {
                    (f64::from(a[c]) + (f64::from(b[c]) - f64::from(a[c])) * f) as f32
                });
                near(line, keys[i].1)
            });
        if fits {
            end = next;
        } else {
            kept.push(keys[end]);
            anchor = end;
            end += 1;
        }
    }
    *keys = kept;
}

/// What a source material is in the Grid's look.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Kind {
    Armor,
    Trim,
    Glow,
    Pale,
}

fn kind(surface: &Surface) -> Kind {
    let emission = surface.material.emissive_factor[0];
    if emission > 0.9 {
        Kind::Glow
    } else if emission > 0.0 {
        Kind::Pale
    } else if surface.tint[0] < 0.04 {
        Kind::Armor
    } else {
        Kind::Trim
    }
}

/// A surface whose vertices are shared by position and joint.
struct Welded {
    surface: Surface,
    index: BTreeMap<([u32; 3], u32), u32>,
}

impl Welded {
    fn new(white: usize, tint: [f32; 3], topology: Topology) -> Self {
        Self {
            surface: flat::surface(white, tint, topology),
            index: BTreeMap::new(),
        }
    }

    fn vertex(&mut self, position: [f32; 3], joint: u32) -> u32 {
        let surface = &mut self.surface;
        *self
            .index
            .entry((position.map(f32::to_bits), joint))
            .or_insert_with(|| {
                surface.vertices.push(Vertex {
                    position,
                    normal: [0., 1., 0.],
                    uv: [0.5, 0.5],
                    joints: [joint, 0, 0, 0],
                    weights: [1., 0., 0., 0.],
                });
                (surface.vertices.len() - 1) as u32
            })
    }
}

/// The imported surfaces in the Grid's flat look: shaded facets, edge
/// lines, and glow, in that draw order.
fn flatten(imported: &[Surface], inverse: DMat4, white: usize) -> Result<Vec<Surface>, String> {
    let shade = |hex: u32| palette::neutral(palette::linear(hex));
    let mut facets: BTreeMap<(Kind, usize), Welded> = BTreeMap::new();
    let mut glow: BTreeMap<Kind, Welded> = BTreeMap::new();
    // Every facet's edges by welded position and joint, with the normals
    // of the facets that share it, for the edge lines.
    type Key = ([u32; 3], u32);
    let mut edges: BTreeMap<(Key, Key), Vec<DVec3>> = BTreeMap::new();
    let light = DVec3::from_array(LIGHT).normalize();
    for surface in imported {
        let kind = kind(surface);
        for triangle in surface.indices.chunks_exact(3) {
            let corners = [triangle[0], triangle[1], triangle[2]].map(|i| {
                let v = &surface.vertices[i as usize];
                let p = inverse.transform_point3(DVec3::from_array(v.position.map(f64::from)));
                (p.as_vec3().to_array(), v.joints[0])
            });
            if corners.iter().any(|c| c.1 != corners[0].1) {
                return Err("A Grid robot facet spans two joints".into());
            }
            let joint = corners[0].1;
            let [a, b, c] = corners.map(|c| DVec3::from_array(c.0.map(f64::from)));
            let normal = (b - a).cross(c - a).normalize_or_zero();
            let target = match kind {
                Kind::Glow | Kind::Pale => glow.entry(kind).or_insert_with(|| {
                    let step = if kind == Kind::Glow {
                        Intensity::Full
                    } else {
                        Intensity::ThreeQuarters
                    };
                    Welded::new(
                        white,
                        palette::neutral(palette::amber(step)),
                        Topology::Triangles,
                    )
                }),
                Kind::Armor | Kind::Trim => {
                    let lit = (normal.dot(light) * 0.5 + 0.5).clamp(0.0, 0.999);
                    let level = (lit * 4.0) as usize;
                    let shades = if kind == Kind::Armor { ARMOR } else { TRIM };
                    for (p, q) in [(0, 1), (1, 2), (2, 0)] {
                        let (p, q) = (
                            (corners[p].0.map(f32::to_bits), joint),
                            (corners[q].0.map(f32::to_bits), joint),
                        );
                        edges
                            .entry(if p < q { (p, q) } else { (q, p) })
                            .or_default()
                            .push(normal);
                    }
                    facets.entry((kind, level)).or_insert_with(|| {
                        Welded::new(white, shade(shades[level]), Topology::Triangles)
                    })
                }
            };
            for (position, joint) in corners {
                let index = target.vertex(position, joint);
                target.surface.indices.push(index);
            }
        }
    }
    let mut lines = Welded::new(
        white,
        palette::neutral(palette::amber(Intensity::Half)),
        Topology::Lines,
    );
    for ((p, q), normals) in &edges {
        let sharp = normals.len() != 2 || normals[0].dot(normals[1]) < EDGE_COSINE;
        if sharp {
            for (bits, joint) in [p, q] {
                let index = lines.vertex(bits.map(f32::from_bits), *joint);
                lines.surface.indices.push(index);
            }
        }
    }
    let mut out: Vec<Surface> = facets.into_values().map(|w| w.surface).collect();
    out.push(lines.surface);
    out.extend(glow.into_values().map(|w| w.surface));
    Ok(out)
}

/// How much larger than its rig the robot stands in the Grid: about 2.8 m
/// tall, a machine over the line figures. The instance transform scales it
/// about its feet, so they stay on the ground.
pub const SCALE: f32 = 1.5;
/// The robot's body radius at [`SCALE`], m: its shoulders and pauldrons.
pub const BODY_RADIUS: f32 = 0.45 * SCALE;
/// How near a player's center comes to a robot's, m.
pub const REACH: f32 = BODY_RADIUS + crate::controller::RADIUS;

/// The four patrols: each robot walks between its route's ends, world x and
/// z, m, one in each quadrant around the spawn. The two ahead flank the
/// stack and the dominoes outside the side arches; the two behind run
/// diagonally off the spawn's back corners. Each keeps clear of the arches,
/// their approaches, the Gym, the pillar, the blocks, and the others.
pub const ROUTES: [[[f32; 2]; 2]; 4] = [
    [[13.0, 6.0], [13.0, 16.0]],
    [[-13.0, 16.0], [-13.0, 6.0]],
    [[16.0, -8.0], [8.0, -14.0]],
    [[-8.0, -14.0], [-16.0, -8.0]],
];
/// Seconds a robot stands idle at each end of its route, turning back over
/// the first [`TURN_SECONDS`].
const PAUSE_SECONDS: f32 = 2.5;
const TURN_SECONDS: f32 = 1.2;
/// The fraction of a round between one patroller's start and the next's:
/// near a quarter, but not one, so no two pause together.
const PHASE: f64 = 0.287;
/// Ground speed while walking: the walk clip's stride at [`SCALE`].
pub const WALK_SPEED: f32 = WALK_LOOP_METERS * SCALE / WALK_LOOP_SECONDS;

/// One step of a patrol.
#[derive(Clone, Copy, Debug)]
enum Step {
    /// Walks from one end to the other.
    Walk { from: [f32; 2], to: [f32; 2] },
    /// Stands idle, turning from one heading to another.
    Stand { at: [f32; 2], from: f32, to: f32 },
}

impl Step {
    fn seconds(self) -> f32 {
        match self {
            Self::Walk { from, to } => (to[0] - from[0]).hypot(to[1] - from[1]) / WALK_SPEED,
            Self::Stand { .. } => PAUSE_SECONDS,
        }
    }
}

fn heading(from: [f32; 2], to: [f32; 2]) -> f32 {
    (to[0] - from[0]).atan2(to[1] - from[1])
}

/// A patrol: walk out, pause and turn, walk back, pause and turn.
fn steps(route: usize) -> [Step; 4] {
    let [a, b] = ROUTES[route];
    let (out, back) = (heading(a, b), heading(b, a));
    [
        Step::Walk { from: a, to: b },
        Step::Stand {
            at: b,
            from: out,
            to: back,
        },
        Step::Walk { from: b, to: a },
        Step::Stand {
            at: a,
            from: back,
            to: out,
        },
    ]
}

/// One full round of patrol `route`, s.
#[must_use]
pub fn round_seconds(route: usize) -> f32 {
    steps(route).iter().map(|s| s.seconds()).sum()
}

/// Where a robot stands and what it plays at one moment.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pose {
    /// Feet, on the ground.
    pub pos: Vec3,
    /// Heading about +Y; zero faces +Z.
    pub yaw: f32,
    pub animation: Selection,
    /// The clip's time, s.
    pub time: f32,
    /// Ground speed, m/s.
    pub speed: f32,
}

/// The patroller on `route`'s pose `clock` seconds in. Each starts
/// [`PHASE`] of a round after the one before, so no two move in step.
#[must_use]
pub fn patroller(route: usize, clock: f64) -> Pose {
    let round = f64::from(round_seconds(route));
    let clock = clock + round * PHASE * route as f64;
    let mut t = clock.rem_euclid(round) as f32;
    let idle = (clock % 1000.0) as f32;
    let steps = steps(route);
    for step in steps {
        let length = step.seconds();
        if t >= length {
            t -= length;
            continue;
        }
        return match step {
            Step::Walk { from, to } => {
                let f = t / length;
                Pose {
                    pos: Vec3::new(
                        from[0] + (to[0] - from[0]) * f,
                        0.0,
                        from[1] + (to[1] - from[1]) * f,
                    ),
                    yaw: heading(from, to),
                    animation: Selection::Named(State::Walk),
                    time: t,
                    speed: WALK_SPEED,
                }
            }
            Step::Stand { at, from, to } => {
                let turn = (t / TURN_SECONDS).min(1.0);
                let eased = turn * turn * (3.0 - 2.0 * turn);
                let delta = crate::controller::wrap(to - from);
                Pose {
                    pos: ground(at),
                    yaw: crate::controller::wrap(from + delta * eased),
                    animation: Selection::Named(State::Idle),
                    time: idle,
                    speed: 0.0,
                }
            }
        };
    }
    let [a, b] = ROUTES[route];
    Pose {
        pos: ground(a),
        yaw: heading(a, b),
        animation: Selection::Named(State::Idle),
        time: idle,
        speed: 0.0,
    }
}

fn ground(at: [f32; 2]) -> Vec3 {
    Vec3::new(at[0], 0.0, at[1])
}

/// Pushes a player's feet at `player` out of reach of a robot at `robot`.
pub fn keep_clear(player: &mut Vec3, robot: Vec3) {
    let (dx, dz) = (player.x - robot.x, player.z - robot.z);
    let distance = dx.hypot(dz);
    if !distance.is_finite() || distance >= REACH {
        return;
    }
    let (nx, nz) = if distance > 1e-4 {
        (dx / distance, dz / distance)
    } else {
        (0.0, -1.0)
    };
    player.x = robot.x + nx * REACH;
    player.z = robot.z + nz * REACH;
}

/// Where the dancer stands in the Everglade arch's frame: outside the pillar
/// on the side away from the spawn's line, and a step toward the spawn so it
/// reads from the approach. Across, through, m; the opening spans
/// [`crate::zones::gate::OPENING_HALF`] either side of the middle.
pub const DANCER_BESIDE_ARCH: [f32; 2] = [3.6, -1.0];

/// The Everglade arch's place in the Grid, whether or not the arch is drawn.
fn everglade_arch() -> crate::zones::Gate {
    crate::zones::Gate::everglade(&crate::blocks::Layout::grid())
}

/// Where the dancer stands, on the ground.
#[must_use]
pub fn dancer_spot() -> Vec3 {
    let arch = everglade_arch();
    let beside = Vec3::new(DANCER_BESIDE_ARCH[0], 0.0, DANCER_BESIDE_ARCH[1]);
    let at = arch.at + Quat::from_rotation_y(arch.yaw) * beside;
    Vec3::new(at.x, 0.0, at.z)
}

/// The dancer `clock` seconds in: in place beside the Everglade arch, facing
/// the spawn, dancing on a loop.
#[must_use]
pub fn dancer(clock: f64) -> Pose {
    let pos = dancer_spot();
    let spawn = crate::world::SPAWN;
    Pose {
        pos,
        yaw: heading([pos.x, pos.z], [spawn.x, spawn.z]),
        animation: Selection::Legacy(CLIP_DANCE),
        time: clock.rem_euclid(f64::from(DANCE_SECONDS)) as f32,
        speed: 0.0,
    }
}

/// The robot's engine instance at `pose`, the far level when the eye is
/// beyond [`FAR_METERS`].
#[must_use]
pub fn instance(pose: &Pose, eye: Vec3) -> verse_engine::presentation::Instance {
    let model = if pose.pos.distance(eye) > FAR_METERS {
        ROBOT_FAR
    } else {
        ROBOT
    };
    verse_engine::presentation::Instance {
        mount: None,
        actor: None,
        model: model.into(),
        transform: Mat4::from_scale_rotation_translation(
            Vec3::splat(SCALE),
            Quat::from_rotation_y(pose.yaw),
            pose.pos,
        ),
        animation: pose.animation,
        time: pose.time.max(0.0),
        animation_epoch: None,
        emission: Vec3::ZERO,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::controller::InputState;
    use crate::grid_pack;
    use crate::runtime::WorldRuntime;

    fn triangles(model: &Model) -> usize {
        model
            .surfaces
            .iter()
            .filter(|s| s.topology == Topology::Triangles)
            .map(|s| s.indices.len() / 3)
            .sum()
    }

    /// Every vertex's position under `clip` at `time`, with its joint.
    fn posed(model: &Model, selection: Selection, time: f32) -> Vec<(Vec3, usize)> {
        let matrices = verse_engine::animation::pose_selected(model, selection, time).unwrap();
        model
            .surfaces
            .iter()
            .flat_map(|s| s.vertices.iter())
            .map(|v| {
                let joint = v.joints[0] as usize;
                (
                    matrices[joint].transform_point3(Vec3::from_array(v.position)),
                    joint,
                )
            })
            .collect()
    }

    /// The distance from `point` to the segment from `a` to `b`, on the
    /// ground.
    fn to_segment(point: Vec3, a: Vec3, b: Vec3) -> f32 {
        let flat = |v: Vec3| Vec3::new(v.x, 0.0, v.z);
        let (point, a, b) = (flat(point), flat(a), flat(b));
        let ab = b - a;
        let t = ((point - a).dot(ab) / ab.length_squared().max(1e-6)).clamp(0.0, 1.0);
        point.distance(a + ab * t)
    }

    /// Points every 0.1 m along `route`.
    fn along(route: usize) -> Vec<Vec3> {
        let [a, b] = ROUTES[route].map(ground);
        let n = (a.distance(b) / 0.1).ceil() as usize;
        (0..=n).map(|i| a.lerp(b, i as f32 / n as f32)).collect()
    }

    #[test]
    fn the_sources_match_their_manifest_and_the_pack_admits_both_levels() {
        sources().unwrap();
        let pack = grid_pack::embedded().unwrap();
        let near = &pack.models[ROBOT];
        let far = &pack.models[ROBOT_FAR];
        assert!(
            (2_000..=7_000).contains(&triangles(near)),
            "{}",
            triangles(near)
        );
        assert!(triangles(far) <= 1_500 && triangles(far) * 2 < triangles(near));
        let inventory = pack.inventory.as_ref().unwrap();
        let robot = inventory
            .assets
            .iter()
            .find(|a| a.id.as_str() == "verse:source:grid-robot")
            .unwrap();
        for name in [ROBOT, ROBOT_FAR] {
            let asset = inventory
                .assets
                .iter()
                .find(|a| a.id.as_str() == format!("verse:model:{name}"))
                .unwrap();
            assert!(asset.dependencies.contains(&robot.id));
        }
        for model in [near, far] {
            assert!(model.surfaces.iter().all(|s| s.unlit && s.texture == 0));
            let lines = model
                .surfaces
                .iter()
                .filter(|s| s.topology == Topology::Lines)
                .count();
            assert_eq!(lines, 1, "one surface of panel edges");
            // Black armor, gray edges, and white glow: every tint is neutral.
            assert!(
                model
                    .surfaces
                    .iter()
                    .all(|s| s.tint[0] == s.tint[1] && s.tint[1] == s.tint[2])
            );
            assert!(model.surfaces.iter().any(|s| s.tint == [1.0; 3]));
            assert!((1.8..1.95).contains(&model.height), "{}", model.height);
            // Only the clips the Grid plays: no kneeling or working a control.
            let mut ids: Vec<u16> = model.clips.iter().map(|c| c.id).collect();
            ids.sort_unstable();
            assert_eq!(
                ids,
                [CLIP_IDLE, CLIP_WALK, CLIP_RUN, CLIP_AIRBORNE, CLIP_DANCE]
            );
        }
    }

    #[test]
    fn every_part_rides_one_universal_joint_at_full_weight() {
        let pack = grid_pack::load_pinned().unwrap();
        let base: serde_json::Value = serde_json::from_slice(
            &std::fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join(
                "../../assets/verse/characters/quaternius/base/Superhero_Male_FullBody.gltf",
            ))
            .unwrap(),
        )
        .unwrap();
        let universal: Vec<&str> = base["skins"][0]["joints"]
            .as_array()
            .unwrap()
            .iter()
            .map(|j| {
                base["nodes"][j.as_u64().unwrap() as usize]["name"]
                    .as_str()
                    .unwrap()
            })
            .collect();
        assert_eq!(universal.len(), 65);
        for name in [ROBOT, ROBOT_FAR] {
            let model = &pack.models[name];
            let skin = model.skin.as_ref().unwrap();
            for joint in &universal {
                assert!(
                    skin.names.iter().any(|n| n == joint),
                    "{name} lacks {joint}"
                );
            }
            assert_eq!(skin.basis, Mat4::IDENTITY.to_cols_array());
            for v in model.surfaces.iter().flat_map(|s| s.vertices.iter()) {
                assert_eq!(v.weights, [1., 0., 0., 0.]);
                let joint = skin.names[v.joints[0] as usize].as_str();
                assert!(universal.contains(&joint), "a vertex rides {joint}");
            }
        }
    }

    #[test]
    fn the_clips_play_and_the_feet_stay_on_the_ground() {
        let pack = grid_pack::load_pinned().unwrap();
        let model = &pack.models[ROBOT];
        let names = &model.skin.as_ref().unwrap().names;
        let feet: Vec<usize> = ["foot_l", "foot_r", "ball_l", "ball_r"]
            .iter()
            .map(|n| names.iter().position(|x| x == n).unwrap())
            .collect();
        let walk = model.clips.iter().find(|c| c.id == CLIP_WALK).unwrap();
        assert!((walk.duration - WALK_LOOP_SECONDS).abs() < 1e-3);
        assert_eq!(characters::loop_distance(4), WALK_LOOP_METERS);
        let dance = model.clips.iter().find(|c| c.id == CLIP_DANCE).unwrap();
        assert!((dance.duration - DANCE_SECONDS).abs() < 1e-3);
        let rest = posed(model, Selection::Named(State::Idle), 0.0);
        let lowest = rest.iter().map(|p| p.0.y).fold(f32::MAX, f32::min);
        assert!(lowest.abs() < 0.01, "standing soles at {lowest}");
        for selection in [
            Selection::Named(State::Idle),
            Selection::Named(State::Walk),
            Selection::Legacy(CLIP_DANCE),
        ] {
            let mut planted = f32::MAX;
            for i in 0..=24 {
                let time = i as f32 / 24.0 * 1.3;
                // At the Grid's scale, about the feet.
                let points: Vec<_> = posed(model, selection, time)
                    .into_iter()
                    .map(|(p, j)| (p * SCALE, j))
                    .collect();
                let soles = points
                    .iter()
                    .filter(|p| feet.contains(&p.1))
                    .map(|p| p.0.y)
                    .fold(f32::MAX, f32::min);
                // A toe may dip a little as the foot rolls; nothing sinks.
                assert!(soles > -0.04, "{selection:?} at {time}: soles at {soles}");
                planted = planted.min(soles.abs());
                let body = points.iter().map(|p| p.0.y).fold(f32::MAX, f32::min);
                assert!(body > -0.04, "{selection:?} sinks to {body}");
            }
            assert!(planted < 0.03, "{selection:?} never plants a foot");
        }
        // The loops close: their first and last poses match.
        for id in LOOPS {
            let clip = model.clips.iter().find(|c| c.id == id).unwrap();
            let a = posed(model, Selection::Legacy(id), 0.0);
            let b = posed(model, Selection::Legacy(id), clip.duration - 1e-4);
            let gap = a
                .iter()
                .zip(&b)
                .map(|(p, q)| p.0.distance(q.0))
                .fold(0.0, f32::max);
            assert!(gap < 0.01, "clip {id} jumps {gap} m at its loop");
        }
    }

    #[test]
    fn the_robot_stands_half_again_as_tall_and_its_body_fits_its_radius() {
        let pack = grid_pack::load_pinned().unwrap();
        let model = &pack.models[ROBOT];
        let tall = model.height * SCALE;
        assert!((2.7..2.95).contains(&tall), "{tall} m tall");
        let at = Pose {
            pos: Vec3::new(3.0, 0.0, 4.0),
            yaw: 0.0,
            animation: Selection::Named(State::Idle),
            time: 0.0,
            speed: 0.0,
        };
        let transform = instance(&at, at.pos).transform;
        assert!(transform.transform_point3(Vec3::ZERO).distance(at.pos) < 1e-6);
        assert!(
            (transform.transform_point3(Vec3::Y * model.height).y - tall).abs() < 1e-4,
            "the feet stay put and the head rises"
        );
        // Standing, the body stays within its radius, and walking, the upper
        // body does; only the swinging forearms and hands, and the striding
        // legs, reach past it.
        let names = &model.skin.as_ref().unwrap().names;
        let arms: Vec<usize> = names
            .iter()
            .enumerate()
            .filter(|(_, n)| {
                [
                    "hand", "lowerarm", "thumb", "index", "middle", "ring", "pinky",
                ]
                .iter()
                .any(|p| n.starts_with(p))
            })
            .map(|(i, _)| i)
            .collect();
        let legs: Vec<usize> = names
            .iter()
            .enumerate()
            .filter(|(_, n)| {
                ["thigh", "calf", "foot", "ball"]
                    .iter()
                    .any(|p| n.starts_with(p))
            })
            .map(|(i, _)| i)
            .collect();
        for selection in [Selection::Named(State::Idle), Selection::Named(State::Walk)] {
            let walking = selection == Selection::Named(State::Walk);
            for i in 0..=8 {
                let widest = posed(model, selection, i as f32 / 8.0)
                    .iter()
                    .filter(|p| !arms.contains(&p.1) && !(walking && legs.contains(&p.1)))
                    .map(|p| p.0.x.hypot(p.0.z) * SCALE)
                    .fold(0.0, f32::max);
                assert!(widest < BODY_RADIUS + 0.1, "{selection:?}: {widest} m wide");
            }
        }
    }

    #[test]
    fn four_robots_patrol_and_one_dances_in_the_grid_plaza_only() {
        let mut grid = WorldRuntime::bare();
        assert!(WorldRuntime::new().robots().is_empty());
        let start = grid.robots();
        assert_eq!(ROUTES.len(), 4);
        assert_eq!(start.len(), ROUTES.len() + 1);
        let dancers = start
            .iter()
            .filter(|r| r.animation == Selection::Legacy(CLIP_DANCE))
            .count();
        assert_eq!(dancers, 1);
        for _ in 0..90 {
            grid.tick_unoccupied(1.0 / 30.0);
        }
        let later = grid.robots();
        for route in 0..ROUTES.len() {
            assert_eq!(start[route], patroller(route, 0.0));
            assert!(later[route].pos.distance(patroller(route, 3.0).pos) < 1e-3);
        }
        assert_eq!(later[ROUTES.len()].pos, dancer_spot());
        let instances = crate::grid_frame::dynamic(&grid, &[], &[]);
        let drawn = instances
            .iter()
            .filter(|i| i.model.as_str().starts_with("grid/robot"))
            .count();
        assert_eq!(drawn, 5);
    }

    #[test]
    fn the_patrols_walk_and_idle_continuously_out_of_step() {
        for route in 0..ROUTES.len() {
            let round = f64::from(round_seconds(route));
            assert!(round > 10.0 && round < 40.0, "{round}");
            let mut previous = patroller(route, 0.0);
            let mut kinds = std::collections::BTreeSet::new();
            for i in 1..=(round * 30.0) as usize + 30 {
                let now = patroller(route, i as f64 / 30.0);
                let step = now.pos.distance(previous.pos);
                assert!(step <= WALK_SPEED / 30.0 + 1e-3, "jump of {step} m at {i}");
                let turn = crate::controller::wrap(now.yaw - previous.yaw).abs();
                assert!(turn < 0.25, "snap of {turn} rad at {i}");
                kinds.insert(format!("{:?}", now.animation));
                previous = now;
            }
            // Only walking and idling: no kneeling or working a control.
            let expected: std::collections::BTreeSet<String> = ["Named(Idle)", "Named(Walk)"]
                .map(String::from)
                .into_iter()
                .collect();
            assert_eq!(kinds, expected);
        }
        // No two move in step.
        for clock in [0.0, 5.0, 11.0] {
            let phases: std::collections::BTreeSet<_> = (0..ROUTES.len())
                .map(|r| patroller(r, clock))
                .map(|p| format!("{:?}{:.1}", p.animation, p.time))
                .collect();
            assert_eq!(phases.len(), ROUTES.len(), "in step at {clock}");
        }
    }

    #[test]
    fn the_routes_keep_clear_of_the_plaza_and_each_other() {
        let layout = crate::blocks::Layout::grid();
        let spawn = crate::world::SPAWN;
        let arches = [
            crate::zones::Gate::grid(&layout),
            crate::zones::Gate::everglade(&layout),
            crate::zones::Gate::ritual(&layout),
        ];
        let walls = crate::world::GymSite::GRID.walls();
        let pillar = crate::pillar::AT.as_vec3();
        let ball = crate::ball::START.as_vec3();
        let stack = layout
            .at(crate::blocks::STACK_AT[0], crate::blocks::STACK_AT[1], 0.0)
            .as_vec3();
        // The dominoes' arc, from its start to a quarter turn along.
        let [side, ahead] = crate::blocks::DOMINOES_AT;
        let dominoes: Vec<Vec3> = (0..=16)
            .map(|i| {
                let theta = std::f64::consts::FRAC_PI_2 * f64::from(i) / 16.0;
                layout
                    .at(
                        side + 4.0 - 4.0 * theta.cos(),
                        ahead + 4.0 * theta.sin(),
                        0.0,
                    )
                    .as_vec3()
            })
            .collect();
        let clear = 2.0 * BODY_RADIUS + 1.5;
        for route in 0..ROUTES.len() {
            for at in along(route) {
                for arch in arches {
                    assert!(at.distance(arch.at) > 4.0, "{route} at {at}: an arch");
                    assert!(
                        at.distance(arch.front().0) > 4.0,
                        "{route} at {at}: an approach"
                    );
                    // The walk from the spawn to each arch.
                    assert!(
                        to_segment(at, spawn, arch.at) > 3.0,
                        "{route} at {at}: a path"
                    );
                }
                for wall in &walls {
                    let inside = (wall.min[0] - 2.0..wall.max[0] + 2.0).contains(&at.x)
                        && (wall.min[1] - 2.0..wall.max[1] + 2.0).contains(&at.z);
                    assert!(!inside, "{route} at {at}: a Gym wall");
                }
                for (thing, point) in [
                    ("the spawn", spawn),
                    ("the pillar", pillar),
                    ("the ball", ball),
                    ("the stack", stack),
                    ("the dancer", dancer_spot()),
                ] {
                    assert!(at.distance(point) > clear, "{route} at {at}: {thing}");
                }
                for domino in &dominoes {
                    assert!(at.distance(*domino) > clear, "{route} at {at}: a domino");
                }
                // The routes never cross or come near each other.
                for other in (0..ROUTES.len()).filter(|o| *o != route) {
                    let [a, b] = ROUTES[other].map(ground);
                    assert!(to_segment(at, a, b) > 4.0, "{route} nears {other}");
                }
            }
        }
    }

    #[test]
    fn the_dance_is_in_the_pack_and_dances_in_place() {
        let pack = grid_pack::embedded().unwrap();
        for name in [ROBOT, ROBOT_FAR] {
            let dance = pack.models[name]
                .clips
                .iter()
                .find(|c| c.id == CLIP_DANCE)
                .unwrap_or_else(|| panic!("{name} lacks the dance"));
            assert!((dance.duration - DANCE_SECONDS).abs() < 1e-3);
        }
        // The pelvis sways but stays over the feet: no drift off the spot.
        let model = &pack.models[ROBOT];
        let names = &model.skin.as_ref().unwrap().names;
        let pelvis = names.iter().position(|n| n == "pelvis").unwrap();
        let center = |time: f32| {
            let points = posed(model, Selection::Legacy(CLIP_DANCE), time);
            let hips: Vec<Vec3> = points
                .iter()
                .filter(|p| p.1 == pelvis)
                .map(|p| p.0)
                .collect();
            hips.iter().sum::<Vec3>() / hips.len() as f32 * SCALE
        };
        let rest = center(0.0);
        for i in 0..=24 {
            let at = center(i as f32 / 24.0 * DANCE_SECONDS);
            let drift = Vec3::new(at.x - rest.x, 0.0, at.z - rest.z).length();
            assert!(drift < 0.3, "the dancer's hips drift {drift} m");
        }
    }

    #[test]
    fn the_dancer_stands_beside_the_everglade_arch_and_clear_of_every_path() {
        let arch = everglade_arch();
        let spot = dancer_spot();
        let near = spot.distance(arch.at);
        assert!((2.5..5.0).contains(&near), "{near} m from the arch");
        // Outside the opening and its pillars (1.9 m out, 0.175 m half
        // wide), with room for a player to pass between.
        let local = arch.local(spot);
        assert!(
            local.x.abs() - REACH > 2.075
                && local.x.abs() - crate::zones::gate::OPENING_HALF > REACH,
            "{local}"
        );
        // On the approach side, level with the arch: beside it, not behind.
        assert!((-2.0..0.0).contains(&local.z), "{local}");
        // Clear of the walk from the spawn to the opening, and of where a
        // player comes back through.
        let spawn = crate::world::SPAWN;
        assert!(to_segment(spot, spawn, arch.at) > 2.0 * REACH);
        assert!(spot.distance(arch.front().0) > 2.0 * REACH);
        let layout = crate::blocks::Layout::grid();
        for gate in [
            crate::zones::Gate::grid(&layout).at,
            crate::zones::Gate::ritual(&layout).at,
        ] {
            assert!(spot.distance(gate) > 3.0, "too near an arch at {gate}");
        }
        // It dances in place, facing the spawn, on a loop.
        let a = dancer(0.25);
        let b = dancer(0.25 + f64::from(DANCE_SECONDS) * 7.0);
        assert_eq!(a.pos, spot);
        assert_eq!(a.animation, Selection::Legacy(CLIP_DANCE));
        assert!((a.time - b.time).abs() < 1e-3 && a.time < DANCE_SECONDS);
        let facing = Quat::from_rotation_y(a.yaw) * Vec3::Z;
        assert!(facing.dot((spawn - spot).normalize()) > 0.99);
    }

    #[test]
    fn a_player_walks_through_the_everglade_arch_past_the_dancer() {
        let arch = everglade_arch();
        let dancer = dancer_spot();
        let world =
            |x: f32, z: f32| arch.at + Quat::from_rotation_y(arch.yaw) * Vec3::new(x, 0.0, z);
        for x in [-1.5, 0.0, 1.5] {
            for z in [-3.0, -1.5, 0.0, 1.5] {
                let at = world(x, z);
                let gap = Vec3::new(at.x - dancer.x, 0.0, at.z - dancer.z).length();
                assert!(gap > REACH, "{x}, {z}: {gap}");
            }
        }
    }

    #[test]
    fn the_far_level_draws_beyond_its_distance() {
        let at = patroller(0, 0.0);
        assert_eq!(instance(&at, at.pos + Vec3::Z * 5.0).model, ROBOT);
        assert_eq!(
            instance(&at, at.pos + Vec3::Z * (FAR_METERS + 1.0)).model,
            ROBOT_FAR
        );
    }

    #[test]
    fn the_player_cannot_walk_through_any_robot() {
        for k in 0..=ROUTES.len() {
            let mut grid = WorldRuntime::bare();
            let robot = grid.robots()[k];
            grid.player.pos = robot.pos - Vec3::Z * 0.3;
            grid.tick(&InputState::default(), 1.0 / 60.0);
            let robot = grid.robots()[k];
            let gap = Vec3::new(
                grid.player.pos.x - robot.pos.x,
                0.0,
                grid.player.pos.z - robot.pos.z,
            )
            .length();
            assert!(
                gap > REACH - 0.05,
                "the player stands {gap} m from robot {k}'s center"
            );
        }
    }
}
