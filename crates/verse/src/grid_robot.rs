//! The Grid robot: an original black-and-white robot on the Universal rig
//! (`docs/verse/grid-robot.md`), and the patrol one of it walks in the
//! Grid's plaza.
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
/// Reaching out to work a control.
pub const CLIP_INTERACT: u16 = 5;
/// Kneeling to fix something on the ground.
pub const CLIP_KNEEL: u16 = 6;
/// Each clip and the Universal clip it plays.
const CLIPS: [(u16, &str); 6] = [
    (CLIP_IDLE, "Idle_Loop"),
    (CLIP_WALK, "Walk_Loop"),
    (CLIP_RUN, "Jog_Fwd_Loop"),
    (CLIP_AIRBORNE, "Jump_Loop"),
    (CLIP_INTERACT, "Interact"),
    (CLIP_KNEEL, "Fixing_Kneeling"),
];
/// The loops whose last key is set to their first, so they cycle cleanly.
const LOOPS: [u16; 4] = [CLIP_IDLE, CLIP_WALK, CLIP_RUN, CLIP_AIRBORNE];
/// `Walk_Loop`'s length, s, and the ground its stride covers in that time,
/// m, as `characters::loop_distance` gives it.
pub const WALK_LOOP_SECONDS: f32 = 4.0 / 3.0;
pub const WALK_LOOP_METERS: f32 = 1.3;
/// `Interact` and `Fixing_Kneeling`'s lengths, s.
pub const INTERACT_SECONDS: f32 = 2.0;
pub const KNEEL_SECONDS: f32 = 5.2;

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
/// rig, with the idle, walk, run, airborne, interact, and kneel clips.
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

/// Where the patrol walks: from its west end, by the plaza's middle, to
/// its east end, by the Everglade arch. World x and z, m.
pub const PATROL_WEST: [f32; 2] = [5.5, -6.0];
pub const PATROL_EAST: [f32; 2] = [5.5, 3.0];
/// Seconds a turn in place takes.
const TURN_SECONDS: f32 = 0.8;
/// Ground speed while walking, which the walk clip's stride covers.
pub const WALK_SPEED: f32 = WALK_LOOP_METERS / WALK_LOOP_SECONDS;

/// One step of the patrol.
#[derive(Clone, Copy, Debug)]
enum Step {
    /// Walks from one end to the other.
    Walk { from: [f32; 2], to: [f32; 2] },
    /// Stands idle, turning from one heading to another over the first
    /// [`TURN_SECONDS`].
    Stand {
        at: [f32; 2],
        from: f32,
        to: f32,
        seconds: f32,
    },
    /// Plays a working clip once, facing `yaw`.
    Work {
        at: [f32; 2],
        clip: u16,
        yaw: f32,
        seconds: f32,
    },
}

impl Step {
    fn seconds(self) -> f32 {
        match self {
            Self::Walk { from, to } => (to[0] - from[0]).hypot(to[1] - from[1]) / WALK_SPEED,
            Self::Stand { seconds, .. } | Self::Work { seconds, .. } => seconds,
        }
    }
}

fn heading(from: [f32; 2], to: [f32; 2]) -> f32 {
    (to[0] - from[0]).atan2(to[1] - from[1])
}

/// The patrol: walk east, turn to the arch and kneel to tend it, turn
/// back, walk west, turn toward the plaza's middle and work a control, and
/// turn east again.
fn steps() -> [Step; 8] {
    let (west, east) = (PATROL_WEST, PATROL_EAST);
    let out = heading(west, east);
    let back = heading(east, west);
    let arch = std::f32::consts::FRAC_PI_2;
    let middle = -std::f32::consts::FRAC_PI_2;
    [
        Step::Walk {
            from: west,
            to: east,
        },
        Step::Stand {
            at: east,
            from: out,
            to: arch,
            seconds: 1.2,
        },
        Step::Work {
            at: east,
            clip: CLIP_KNEEL,
            yaw: arch,
            seconds: KNEEL_SECONDS,
        },
        Step::Stand {
            at: east,
            from: arch,
            to: back,
            seconds: 1.6,
        },
        Step::Walk {
            from: east,
            to: west,
        },
        Step::Stand {
            at: west,
            from: back,
            to: middle,
            seconds: 1.2,
        },
        Step::Work {
            at: west,
            clip: CLIP_INTERACT,
            yaw: middle,
            seconds: INTERACT_SECONDS,
        },
        Step::Stand {
            at: west,
            from: middle,
            to: out,
            seconds: 2.4,
        },
    ]
}

/// One full round of the patrol, s.
#[must_use]
pub fn round_seconds() -> f32 {
    steps().iter().map(|s| s.seconds()).sum()
}

/// Where the robot stands and what it plays at one moment of its patrol.
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

/// The robot's pose `clock` seconds into its patrol.
#[must_use]
pub fn pose(clock: f64) -> Pose {
    let round = f64::from(round_seconds());
    let mut t = clock.rem_euclid(round) as f32;
    let idle = clock as f32 % 1000.0;
    for step in steps() {
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
            Step::Stand { at, from, to, .. } => {
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
            Step::Work { at, clip, yaw, .. } => Pose {
                pos: ground(at),
                yaw,
                animation: Selection::Legacy(clip),
                // Once through: never past the clip's last key.
                time: t.min(length - 1e-3),
                speed: 0.0,
            },
        };
    }
    Pose {
        pos: ground(PATROL_WEST),
        yaw: 0.0,
        animation: Selection::Named(State::Idle),
        time: idle,
        speed: 0.0,
    }
}

fn ground(at: [f32; 2]) -> Vec3 {
    Vec3::new(at[0], 0.0, at[1])
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
        transform: Mat4::from_rotation_translation(Quat::from_rotation_y(pose.yaw), pose.pos),
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
        for (id, seconds) in [
            (CLIP_INTERACT, INTERACT_SECONDS),
            (CLIP_KNEEL, KNEEL_SECONDS),
        ] {
            let clip = model.clips.iter().find(|c| c.id == id).unwrap();
            assert!((clip.duration - seconds).abs() < 1e-3);
        }
        let rest = posed(model, Selection::Named(State::Idle), 0.0);
        let lowest = rest.iter().map(|p| p.0.y).fold(f32::MAX, f32::min);
        assert!(lowest.abs() < 0.01, "standing soles at {lowest}");
        for selection in [
            Selection::Named(State::Idle),
            Selection::Named(State::Walk),
            Selection::Legacy(CLIP_INTERACT),
            Selection::Legacy(CLIP_KNEEL),
        ] {
            let mut planted = f32::MAX;
            for i in 0..=24 {
                let time = i as f32 / 24.0 * 1.3;
                let points = posed(model, selection, time);
                let soles = points
                    .iter()
                    .filter(|p| feet.contains(&p.1))
                    .map(|p| p.0.y)
                    .fold(f32::MAX, f32::min);
                // A toe may dip a little as the foot rolls; nothing sinks.
                assert!(soles > -0.03, "{selection:?} at {time}: soles at {soles}");
                planted = planted.min(soles.abs());
                // Kneeling presses the knee ball a few centimeters into the
                // floor; nothing else sinks.
                let floor = if selection == Selection::Legacy(CLIP_KNEEL) {
                    -0.05
                } else {
                    -0.03
                };
                let body = points.iter().map(|p| p.0.y).fold(f32::MAX, f32::min);
                assert!(body > floor, "{selection:?} sinks to {body}");
            }
            assert!(planted < 0.02, "{selection:?} never plants a foot");
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
    fn the_robot_patrols_the_grid_plaza_only() {
        let mut grid = WorldRuntime::bare();
        assert!(WorldRuntime::new().robot().is_none());
        let start = grid.robot().unwrap();
        assert_eq!(start.pos, Vec3::new(PATROL_WEST[0], 0.0, PATROL_WEST[1]));
        for _ in 0..90 {
            grid.tick_unoccupied(1.0 / 30.0);
        }
        let later = grid.robot().unwrap();
        assert!((later.pos.distance(start.pos) - 3.0 * WALK_SPEED).abs() < 0.01);
        let instances = crate::grid_frame::dynamic(&grid, &[], &[]);
        assert!(instances.iter().any(|i| i.model == ROBOT));
    }

    #[test]
    fn the_patrol_is_continuous_and_keeps_clear_of_the_arches_and_gym() {
        let round = f64::from(round_seconds());
        assert!(round > 20.0 && round < 60.0, "{round}");
        let layout = crate::blocks::Layout::grid();
        let gates = [
            crate::zones::Gate::grid(&layout).at,
            crate::zones::Gate::everglade(&layout).at,
            crate::zones::Gate::ritual(&layout).at,
        ];
        let walls = crate::world::GymSite::GRID.walls();
        let mut previous = pose(0.0);
        let mut kinds = std::collections::BTreeSet::new();
        for i in 1..=(round * 30.0) as usize + 30 {
            let now = pose(i as f64 / 30.0);
            let step = now.pos.distance(previous.pos);
            assert!(step <= WALK_SPEED / 30.0 + 1e-3, "jump of {step} m at {i}");
            let turn = crate::controller::wrap(now.yaw - previous.yaw).abs();
            assert!(turn < 0.25, "snap of {turn} rad at {i}");
            for gate in gates {
                assert!(now.pos.distance(gate) > 3.0, "too near an arch at {gate}");
            }
            for wall in &walls {
                let inside = (wall.min[0] - 1.0..wall.max[0] + 1.0).contains(&now.pos.x)
                    && (wall.min[1] - 1.0..wall.max[1] + 1.0).contains(&now.pos.z);
                assert!(!inside, "inside a Gym wall at {}", now.pos);
            }
            kinds.insert(format!("{:?}", now.animation));
            previous = now;
        }
        // It idles, walks, kneels, and works a control in each round.
        assert_eq!(kinds.len(), 4, "{kinds:?}");
    }

    #[test]
    fn the_far_level_draws_beyond_its_distance() {
        let at = pose(0.0);
        assert_eq!(instance(&at, at.pos + Vec3::Z * 5.0).model, ROBOT);
        assert_eq!(
            instance(&at, at.pos + Vec3::Z * (FAR_METERS + 1.0)).model,
            ROBOT_FAR
        );
    }

    #[test]
    fn the_player_cannot_walk_through_the_robot() {
        let mut grid = WorldRuntime::bare();
        let robot = grid.robot().unwrap();
        grid.player.pos = robot.pos - Vec3::Z * 0.3;
        grid.tick(&InputState::default(), 1.0 / 60.0);
        let robot = grid.robot().unwrap();
        let gap = Vec3::new(
            grid.player.pos.x - robot.pos.x,
            0.0,
            grid.player.pos.z - robot.pos.z,
        )
        .length();
        assert!(
            gap > 0.5,
            "the player stands {gap} m from the robot's center"
        );
    }
}
