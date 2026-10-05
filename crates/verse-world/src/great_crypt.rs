//! The great crypt's cultist fight: the chamber's combat rules, abilities,
//! enemy AI, and respawn, staged in a larger crypt around a ritual
//! (`docs/verse/zone-rules.md`, Great crypt).
//!
//! The crypt is `scripts/blender/great_crypt.py`'s hall in
//! `assets/verse/generated/great_crypt/`, furnished with the crypt lab's
//! props by [`LAYOUT`]. Its collision profile, [`PROFILE`], compiles the
//! hall's explicit collision boxes and every standing prop's footprint into
//! the colliders the authority walks, paths, aims, and keeps the camera
//! inside with.
//!
//! The fight is [`Game`]'s chamber encounter with a [`Ritual`] on top:
//!
//! - Six acolytes chant around the summoning circle on the dais while the
//!   ritual's clock runs. Each one that dies or is drawn into the fight
//!   slows it. They join the fight when the player comes near or hurts
//!   them.
//! - The High Priest stands below the circle. Awake, he throws a volley of
//!   three shadow bolts; at half health he calls every chapel's cultists at
//!   once.
//! - Pairs of cultists wait in the four side chapels and come out in waves.
//! - Claude, on the circle, sleeps until the ritual completes (he wakes
//!   empowered, as if enraged), the High Priest dies, or the player strikes
//!   him. Killing him ends the fight.

use std::collections::{BTreeMap, BTreeSet};
use std::f32::consts::{FRAC_PI_2, PI};
use std::sync::OnceLock;

use glam::{DVec3, Mat4, Vec3};
use serde::{Deserialize, Serialize};
use verse_engine::director::{Action, Actor, Cue, Scene};
use verse_engine::motion::State;

use crate::combat::{Definition, Encounter};
use crate::play::Game;

/// The great crypt's collision profile name in its scene.
pub const PROFILE: &str = "great-crypt-v1";

/// The plan, in world meters (glTF: +Y up, +Z toward the entrance). These
/// match `scripts/blender/great_crypt.py`.
pub const NAVE: f32 = 5.0;
pub const WALL: f32 = 10.5;
pub const ENTRY: f32 = 17.5;
pub const FAR: f32 = -21.5;
/// The deepest a chapel reaches beyond the aisle walls, m.
pub const CHAPEL_BACK: f32 = 14.1;
/// Chapel centers along z, on both sides.
pub const CHAPELS: [f32; 2] = [-6.75, 6.75];
/// The entrance landing: its front edge, top, and half width.
pub const LANDING: (f32, f32, f32) = (12.5, 1.5, 4.4);
/// The summoning circle's center on the dais, and its radius.
pub const CIRCLE: Vec3 = Vec3::new(0.0, 0.75, -17.8);
pub const CIRCLE_RADIUS: f32 = 2.2;
/// The tall window in the far wall.
pub const WINDOW: Vec3 = Vec3::new(0.0, 6.6, FAR);
/// Where the nave vault springs and how far it rises.
pub const SPRING: f32 = 7.0;
pub const RISE: f32 = 3.0;

/// Where the player arrives: on the landing, facing down the nave.
pub const SPAWN: Vec3 = Vec3::new(0.0, LANDING.1, 15.2);

/// The scene's actor IDs by role.
pub const BOSS: u64 = 1;
pub const PLAYER: u64 = 2;
/// The High Priest is actor 3, whom the chamber's encounter has call to
/// seal the chamber as the fight starts.
pub const LEADER: u64 = 3;
pub const CHANTERS: [u64; 6] = [4, 5, 6, 7, 8, 9];
pub const GUARDS: [u64; 6] = [10, 11, 12, 13, 14, 15];
/// The chapel cultists, two per chapel: near west, near east, far west,
/// far east.
pub const CHAPEL_CULTISTS: [u64; 8] = [16, 17, 18, 19, 20, 21, 22, 23];

/// How long the acolytes must chant to complete the ritual, s.
pub const RITUAL_SECONDS: f32 = 150.0;
/// When each wave leaves its chapels, s after the fight starts.
pub const WAVES: [(f32, [u64; 4]); 2] = [(35.0, [16, 17, 18, 19]), (75.0, [20, 21, 22, 23])];
/// How near the player must come to wake a waiting actor, m.
const WAKE_CHANTER: f32 = 9.0;
const WAKE_CHAPEL: f32 = 6.5;
const WAKE_LEADER: f32 = 13.0;

/// The prop models from the crypt lab, in `assets/verse/generated/chamber`.
pub const CHAMBER_MODELS: &[&str] = &[
    "sarcophagus",
    "candelabrum_tall",
    "candelabrum_short",
    "floor_candles",
    "brazier",
    "cauldron_green",
    "cauldron_red",
    "cauldron_amber",
    "bookshelf",
    "jar_shelf",
    "chained_skeleton",
    "hanging_chains",
    "iron_cage",
    "slab_table",
    "alchemy_bench",
    "writing_desk",
    "lectern",
    "crate",
    "barrel",
    "specimen_jar",
    "specimen_jar_bones",
    "bone_scatter",
    "cobweb",
    "ritual_rug",
];
/// The great crypt's own models, in `assets/verse/generated/great_crypt`.
pub const CRYPT_MODELS: &[&str] = &[
    "great_crypt_hall",
    "summoning_circle",
    "broken_pillar",
    "rubble_pile",
];

/// The directory under `assets/verse/generated` that holds `model`.
#[must_use]
pub fn model_folder(model: &str) -> &'static str {
    if CRYPT_MODELS.contains(&model) {
        "great_crypt"
    } else {
        "chamber"
    }
}

/// Models that hang on walls or lie flat, which nothing runs into.
const UNBLOCKING: &[&str] = &[
    "cobweb",
    "ritual_rug",
    "summoning_circle",
    "bone_scatter",
    "hanging_chains",
];

const W: f32 = FRAC_PI_2;

/// Where each model stands: name, x, y, z (world meters), and yaw. A model
/// faces +Z at yaw 0; a wall-hung model has its wall behind it.
pub const LAYOUT: &[(&str, f32, f32, f32, f32)] = &[
    ("great_crypt_hall", 0.0, 0.0, 0.0, 0.0),
    // The dais: the circle, candelabra at its corners, obelisk-flanked
    // bones under the window, and braziers at its foot.
    ("summoning_circle", 0.0, 0.75, -17.8, 0.0),
    ("candelabrum_tall", -2.95, 0.75, -15.05, 0.3),
    ("candelabrum_tall", 2.95, 0.75, -15.05, -0.4),
    ("candelabrum_tall", -1.7, 0.75, -21.05, 0.9),
    ("candelabrum_tall", 1.7, 0.75, -21.05, -0.7),
    ("bone_scatter", 0.0, 0.75, -21.0, 0.6),
    ("floor_candles", -2.6, 0.5, -13.95, 0.0),
    ("floor_candles", 2.6, 0.5, -13.95, 1.1),
    ("brazier", -3.3, 0.0, -11.9, 0.4),
    ("brazier", 3.3, 0.0, -11.9, -0.6),
    ("ritual_rug", 0.0, 0.0, -9.6, 0.0),
    ("cobweb", -WALL, 4.4, FAR, -W),
    ("cobweb", WALL, 4.4, FAR, PI),
    // The apse aisles beside the dais: cauldrons, cages, chains.
    ("cauldron_green", -7.6, 0.0, -18.2, 0.0),
    ("cauldron_red", 7.6, 0.0, -18.2, 2.0),
    ("cauldron_amber", 7.7, 0.0, -11.4, 1.2),
    ("iron_cage", -9.2, 0.0, -20.4, 0.4),
    ("iron_cage", 9.3, 0.0, -14.6, -0.3),
    ("chained_skeleton", -WALL, 0.0, -15.75, W),
    ("hanging_chains", WALL, 0.0, -20.0, -W),
    ("hanging_chains", -WALL, 0.0, -11.2, W),
    ("specimen_jar", -9.6, 0.0, -12.6, 0.3),
    ("specimen_jar_bones", 9.7, 0.0, -17.0, -0.6),
    // The west aisle: the study, bookshelves against the wall.
    ("bookshelf", -WALL + 0.28, 0.0, -11.0, W),
    ("bookshelf", -WALL + 0.28, 0.0, -2.6, W),
    ("jar_shelf", -WALL + 0.24, 0.0, -0.4, W),
    ("bookshelf", -WALL + 0.28, 0.0, 1.8, W),
    ("writing_desk", -8.3, 0.0, -1.1, W),
    ("lectern", -7.2, 0.0, 1.0, 1.1),
    ("candelabrum_tall", -8.9, 0.0, 3.1, 0.2),
    ("bone_scatter", -7.6, 0.0, -8.6, 2.4),
    // The east aisle: dissection and brewing.
    ("slab_table", 8.2, 0.0, -1.6, W),
    ("chained_skeleton", WALL, 0.0, -2.0, -W),
    ("specimen_jar_bones", 9.7, 0.0, 0.6, -0.5),
    ("specimen_jar", 9.6, 0.0, 1.5, 0.8),
    ("alchemy_bench", WALL - 0.4, 0.0, -10.2, -W),
    ("candelabrum_short", WALL - 0.45, 0.86, -10.6, 0.0),
    ("jar_shelf", WALL - 0.24, 0.0, 2.8, -W),
    ("candelabrum_tall", 8.8, 0.0, 3.4, -0.5),
    // Near the entrance, below the landing: storage.
    ("crate", -9.6, 0.0, 16.4, 0.2),
    ("crate", -8.85, 0.0, 16.5, -0.3),
    ("crate", -9.25, 0.7, 16.45, 0.5),
    ("barrel", -9.7, 0.0, 15.3, 0.0),
    ("barrel", -7.6, 0.0, 16.6, 1.3),
    ("crate", 9.6, 0.0, 16.4, -0.2),
    ("barrel", 9.7, 0.0, 15.3, 0.9),
    ("barrel", 8.95, 0.0, 16.5, 2.0),
    ("floor_candles", -8.2, 0.0, 13.6, 0.0),
    ("floor_candles", 8.4, 0.0, 13.2, 0.6),
    ("bone_scatter", 7.4, 0.0, 10.4, 4.0),
    ("cobweb", -WALL, 4.4, ENTRY, 0.0),
    ("cobweb", WALL, 4.4, ENTRY, W),
    // The landing: candelabra at the stair head, candles by the door.
    ("candelabrum_tall", -3.6, 1.5, 13.1, 0.3),
    ("candelabrum_tall", 3.6, 1.5, 13.1, -0.3),
    ("floor_candles", -2.9, 1.5, 16.7, 0.0),
    ("floor_candles", 2.9, 1.5, 16.7, 2.0),
    // The nave: broken piers and fallen vault stones for cover.
    ("broken_pillar", -2.6, 0.0, -6.2, 0.3),
    ("broken_pillar", 2.7, 0.0, 2.4, 1.9),
    ("rubble_pile", 2.9, 0.0, -8.6, 0.8),
    ("rubble_pile", -3.0, 0.0, 5.6, 2.4),
    ("floor_candles", 0.9, 0.0, -5.4, 0.0),
    ("floor_candles", -1.2, 0.0, 3.4, 1.0),
    // The four side chapels: a sarcophagus each, candles, and webs.
    ("sarcophagus", -13.25, 0.0, -6.75, W),
    ("candelabrum_tall", -12.2, 0.0, -8.15, 0.2),
    ("floor_candles", -12.3, 0.0, -5.35, 0.0),
    ("cobweb", -CHAPEL_BACK, 4.4, -8.45, -W),
    ("sarcophagus", 13.25, 0.0, -6.75, -W),
    ("candelabrum_tall", 12.2, 0.0, -5.35, -0.4),
    ("floor_candles", 12.3, 0.0, -8.15, 1.0),
    ("cobweb", CHAPEL_BACK, 4.4, -5.05, W),
    ("sarcophagus", -13.25, 0.0, 6.75, W),
    ("candelabrum_tall", -12.2, 0.0, 5.35, 0.6),
    ("floor_candles", -12.3, 0.0, 8.15, 2.0),
    ("cobweb", -CHAPEL_BACK, 4.4, 5.05, -W),
    ("sarcophagus", 13.25, 0.0, 6.75, -W),
    ("candelabrum_tall", 12.2, 0.0, 8.15, -0.1),
    ("floor_candles", 12.3, 0.0, 5.35, 0.5),
    ("bone_scatter", 13.3, 0.0, 8.0, 1.2),
    ("cobweb", CHAPEL_BACK, 4.4, 8.45, W),
];

#[derive(Deserialize)]
struct FootprintFile {
    boxes: Vec<BoxSpec>,
}

#[derive(Deserialize)]
struct BoxSpec {
    #[allow(dead_code)]
    name: String,
    center: [f32; 3],
    half_extents: [f32; 3],
}

macro_rules! footprint {
    ($folder:literal, $name:literal) => {
        (
            $name,
            include_str!(concat!(
                "../../../assets/verse/generated/",
                $folder,
                "/",
                $name,
                ".footprint.json"
            )),
        )
    };
}

const FOOTPRINTS: &[(&str, &str)] = &[
    footprint!("great_crypt", "great_crypt_hall"),
    footprint!("great_crypt", "summoning_circle"),
    footprint!("great_crypt", "broken_pillar"),
    footprint!("great_crypt", "rubble_pile"),
    footprint!("chamber", "sarcophagus"),
    footprint!("chamber", "candelabrum_tall"),
    footprint!("chamber", "candelabrum_short"),
    footprint!("chamber", "floor_candles"),
    footprint!("chamber", "brazier"),
    footprint!("chamber", "cauldron_green"),
    footprint!("chamber", "cauldron_red"),
    footprint!("chamber", "cauldron_amber"),
    footprint!("chamber", "bookshelf"),
    footprint!("chamber", "jar_shelf"),
    footprint!("chamber", "chained_skeleton"),
    footprint!("chamber", "hanging_chains"),
    footprint!("chamber", "iron_cage"),
    footprint!("chamber", "slab_table"),
    footprint!("chamber", "alchemy_bench"),
    footprint!("chamber", "writing_desk"),
    footprint!("chamber", "lectern"),
    footprint!("chamber", "crate"),
    footprint!("chamber", "barrel"),
    footprint!("chamber", "specimen_jar"),
    footprint!("chamber", "specimen_jar_bones"),
    footprint!("chamber", "bone_scatter"),
    footprint!("chamber", "cobweb"),
    footprint!("chamber", "ritual_rug"),
];

fn footprints(name: &str) -> Result<Vec<BoxSpec>, String> {
    let text = FOOTPRINTS
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, text)| *text)
        .ok_or_else(|| format!("The great crypt has no footprint for {name}"))?;
    serde_json::from_str::<FootprintFile>(text)
        .map(|file| file.boxes)
        .map_err(|e| format!("{name}.footprint.json: {e}"))
}

/// The world bounds of a footprint box placed at `at`, turned by `yaw`.
fn placed_box(at: Vec3, yaw: f32, center: [f32; 3], half: [f32; 3]) -> physics::kinematic::Aabb {
    let turn = Mat4::from_translation(at) * Mat4::from_rotation_y(yaw);
    let mut min = Vec3::splat(f32::INFINITY);
    let mut max = Vec3::splat(f32::NEG_INFINITY);
    for corner in 0..8 {
        let sign = |bit: usize| if corner & bit == 0 { -1.0 } else { 1.0 };
        let p = turn.transform_point3(Vec3::new(
            center[0] + sign(1) * half[0],
            center[1] + sign(2) * half[1],
            center[2] + sign(4) * half[2],
        ));
        min = min.min(p);
        max = max.max(p);
    }
    physics::kinematic::Aabb {
        min: min.as_dvec3(),
        max: max.as_dvec3(),
    }
}

/// What a character runs into, stands on, and the camera stays inside: the
/// hall's collision boxes (walls, piers, arcades, ceilings, the dais, the
/// landing, its stairs and parapets) and every standing prop's footprint.
///
/// # Errors
///
/// Returns a message when a footprint file does not parse.
pub fn colliders() -> Result<Vec<physics::kinematic::Aabb>, String> {
    static COLLIDERS: OnceLock<Result<Vec<physics::kinematic::Aabb>, String>> = OnceLock::new();
    COLLIDERS
        .get_or_init(|| {
            let mut out = Vec::new();
            for &(name, x, y, z, yaw) in LAYOUT {
                if UNBLOCKING.contains(&name) {
                    continue;
                }
                for spec in footprints(name)? {
                    out.push(placed_box(
                        Vec3::new(x, y, z),
                        yaw,
                        spec.center,
                        spec.half_extents,
                    ));
                }
            }
            Ok(out)
        })
        .clone()
}

/// The walkable navigation the hostiles path over: the nave, the aisles,
/// the chapels, the dais steps, the stairs, and the landing.
pub(crate) fn navigation(
    instance: u64,
) -> Result<std::sync::Arc<physics::walkable::Navigation>, String> {
    use physics::walkable::{Config, Navigation};
    static COMPILED: OnceLock<Result<std::sync::Arc<Navigation>, String>> = OnceLock::new();
    let template = COMPILED
        .get_or_init(|| {
            let scene = crate::room::profile_query_scene(Some(PROFILE), 0)?;
            Ok(std::sync::Arc::new(Navigation::compile(
                &scene,
                Config {
                    instance: 0,
                    layers: 1,
                    min: DVec3::new(-14.5, -0.01, -22.0),
                    max: DVec3::new(14.5, 3.5, 18.0),
                    cell: 0.5,
                    character: physics::character::Settings::default(),
                    work_budget: 80_000_000,
                },
            )?))
        })
        .clone()?;
    Ok(if instance == 0 {
        template
    } else {
        std::sync::Arc::new(template.bind_instance(instance))
    })
}

/// The yaw at which an actor at `from` faces `to`.
fn facing(from: Vec3, to: Vec3) -> f32 {
    let d = to - from;
    (-d.x).atan2(-d.z)
}

/// Where each chanter kneels: around the circle, facing it.
#[must_use]
pub fn chanter_position(index: usize) -> Vec3 {
    let angle = index as f32 * std::f32::consts::TAU / CHANTERS.len() as f32;
    CIRCLE + Vec3::new(angle.cos(), 0.0, angle.sin()) * 2.75
}

/// The fight's scene: the boss on the circle, the adventurer on the landing,
/// the acolytes around the circle, the High Priest below it, guards in the
/// nave, and cultists waiting in the chapels.
#[must_use]
pub fn scene() -> Scene {
    let actor = |id, name: &str, model: &str, position: Vec3, yaw, scale, health| Actor {
        id,
        name: name.into(),
        model: model.into(),
        position,
        yaw,
        scale,
        health,
        nameplate: id != PLAYER,
        friendly: false,
    };
    let mut actors = vec![
        actor(BOSS, "Claude", "claude", CIRCLE, 0.0, 2.0, 420),
        actor(PLAYER, "Adventurer", "adventurer", SPAWN, 0.0, 1.0, 100),
    ];
    for (index, id) in CHANTERS.into_iter().enumerate() {
        let at = chanter_position(index);
        actors.push(actor(
            id,
            "Acolyte of Anthropic",
            "cultist-acolyte",
            at,
            facing(at, CIRCLE),
            1.0,
            12,
        ));
    }
    let priest = Vec3::new(0.0, 0.5, -14.15);
    actors.push(actor(
        LEADER,
        "High Priest of Anthropic",
        "cultist-leader",
        priest,
        0.0,
        1.2,
        110,
    ));
    let variants = [
        "cultist",
        "cultist-female",
        "cultist-peasant",
        "cultist-peasant-female",
    ];
    let guards = [
        Vec3::new(-1.0, 0.0, -3.4),
        Vec3::new(-3.0, 0.0, -2.6),
        Vec3::new(-1.9, 0.0, -1.6),
        Vec3::new(1.4, 0.0, 6.6),
        Vec3::new(3.0, 0.0, 6.0),
        Vec3::new(2.2, 0.0, 7.6),
    ];
    for (k, (id, at)) in GUARDS.into_iter().zip(guards).enumerate() {
        actors.push(actor(
            id,
            "Cultist of Anthropic",
            variants[k % 4],
            at,
            facing(at, SPAWN),
            1.0,
            15,
        ));
    }
    // Near west, near east, far west, far east.
    let chapels = [
        (-1.0, CHAPELS[1]),
        (1.0, CHAPELS[1]),
        (-1.0, CHAPELS[0]),
        (1.0, CHAPELS[0]),
    ];
    for (k, id) in CHAPEL_CULTISTS.into_iter().enumerate() {
        let (side, z) = chapels[k / 2];
        let at = Vec3::new(side * 11.6, 0.0, z + if k % 2 == 0 { -0.6 } else { 0.6 });
        actors.push(actor(
            id,
            "Cultist of Anthropic",
            variants[(k + 1) % 4],
            at,
            facing(at, Vec3::new(0.0, 0.0, z)),
            1.0,
            15,
        ));
    }
    let yell = |at, actor, text: &str, animation: State| Cue {
        at,
        actor,
        action: Action::Yell {
            text: text.into(),
            animation: animation.into(),
        },
    };
    Scene {
        collision_profile: Some(PROFILE.into()),
        version: 1,
        duration: 600.0,
        origin: [0.0; 3],
        cut_at: 0.5,
        actors,
        cues: vec![
            yell(
                1.2,
                CHANTERS[0],
                "The circle wakes! Keep chanting, he is almost ensouled!",
                State::Affirm,
            ),
            yell(
                4.5,
                LEADER,
                "An intruder on the stair. Faithful, hold the nave!",
                State::Yell,
            ),
        ],
    }
}

/// The ritual on top of the chamber encounter: who waits, how far the
/// chant has come, and whether Claude is awake.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Ritual {
    /// Actors not yet in the fight: chanting acolytes, the High Priest,
    /// the chapels' cultists, and the sleeping boss.
    pub waiting: BTreeSet<u64>,
    /// Seconds of chant so far, out of [`RITUAL_SECONDS`].
    pub progress: f32,
    /// When each wave was called, by index into [`WAVES`].
    pub called: BTreeMap<usize, f32>,
    /// When Claude woke, and whether the completed ritual empowered him.
    pub awakened: Option<f32>,
    pub empowered: bool,
    /// Whether the High Priest has called the chapels to him.
    pub rallied: bool,
}

impl Ritual {
    /// A ritual that has not begun: everyone but the guards waits.
    #[must_use]
    pub fn new() -> Self {
        let mut waiting: BTreeSet<u64> = CHANTERS.into_iter().collect();
        waiting.extend(CHAPEL_CULTISTS);
        waiting.insert(LEADER);
        waiting.insert(BOSS);
        Self {
            waiting,
            ..Self::default()
        }
    }

    /// Whether `actor` still waits outside the fight: it neither moves nor
    /// casts.
    #[must_use]
    pub fn holds(&self, actor: u64) -> bool {
        self.waiting.contains(&actor)
    }

    /// Whether `actor` is an acolyte still chanting.
    #[must_use]
    pub fn chanting(&self, actor: u64) -> bool {
        CHANTERS.contains(&actor) && self.waiting.contains(&actor) && self.awakened.is_none()
    }

    /// How far the ritual has come, 0 to 1.
    #[must_use]
    pub fn fraction(&self) -> f32 {
        (self.progress / RITUAL_SECONDS).clamp(0.0, 1.0)
    }

    /// How many acolytes are chanting.
    #[must_use]
    pub fn chanters(&self) -> usize {
        CHANTERS.iter().filter(|id| self.chanting(**id)).count()
    }

    /// Whether every wave has left its chapel.
    #[must_use]
    pub fn waves_called(&self) -> usize {
        self.called.len()
    }

    /// Admits a checkpointed ritual only when it names this scene's actors.
    pub(crate) fn validate(&self) -> Result<(), String> {
        let known = |id: &u64| {
            *id == BOSS || *id == LEADER || CHANTERS.contains(id) || CHAPEL_CULTISTS.contains(id)
        };
        if !self.waiting.iter().all(known)
            || !self.progress.is_finite()
            || !(0.0..=RITUAL_SECONDS).contains(&self.progress)
            || self.called.keys().any(|k| *k >= WAVES.len())
            || self.called.values().any(|t| !t.is_finite())
            || self.awakened.is_some_and(|t| !t.is_finite())
        {
            return Err("Invalid great crypt ritual".into());
        }
        Ok(())
    }

    fn wake(
        &mut self,
        encounter_ready: &mut BTreeMap<u64, f32>,
        actor: u64,
        time: f32,
        delay: f32,
    ) {
        if self.waiting.remove(&actor) {
            let ready = encounter_ready.entry(actor).or_insert(time);
            *ready = ready.max(time + delay);
        }
    }

    fn awaken(
        &mut self,
        ready: &mut BTreeMap<u64, f32>,
        game: &mut Game,
        empowered: bool,
        text: &str,
    ) {
        if self.awakened.is_some() {
            return;
        }
        self.awakened = Some(game.time);
        self.empowered = empowered;
        self.wake(ready, BOSS, game.time, 1.6);
        for id in CHANTERS {
            self.wake(ready, id, game.time, 1.0 + (id % 3) as f32 * 0.4);
        }
        game.scene.cues.push(Cue {
            at: game.time,
            actor: BOSS,
            action: Action::Yell {
                text: text.into(),
                animation: State::Yell.into(),
            },
        });
        game.message = if empowered {
            "The ritual is complete. Claude wakes empowered!"
        } else {
            "Claude wakes!"
        }
        .into();
    }

    /// Advances the ritual one step: wakes actors that are hurt, near the
    /// player, or called; runs the chant; and wakes Claude.
    pub(crate) fn step(
        &mut self,
        ready: &mut BTreeMap<u64, f32>,
        game: &mut Game,
        frame: &verse_engine::director::Frame,
        dt: f32,
    ) {
        let now = game.time;
        let since = now - game.scene.cut_at;
        let players: Vec<Vec3> = game.living_players().into_iter().map(|(_, p)| p).collect();
        let near = |at: Vec3, reach: f32| {
            players
                .iter()
                .any(|p| Vec3::new(p.x - at.x, 0.0, p.z - at.z).length() <= reach)
        };
        let health = |id: u64| {
            frame
                .actors
                .iter()
                .find(|a| a.actor.id == id)
                .map(|a| (a.health, a.actor.health))
        };
        // Hurt, dead, or near: drawn into the fight.
        let waiting: Vec<u64> = self.waiting.iter().copied().collect();
        for id in waiting {
            let Some(actor) = frame.actors.iter().find(|a| a.actor.id == id) else {
                continue;
            };
            let (hp, max, at) = (actor.health, actor.actor.health, actor.actor.position);
            if id == BOSS {
                if hp < max && hp > 0 {
                    self.awaken(
                        ready,
                        game,
                        false,
                        "Who dares strike me before the rite is done?",
                    );
                }
                continue;
            }
            let reach = if CHANTERS.contains(&id) {
                WAKE_CHANTER
            } else if id == LEADER {
                WAKE_LEADER
            } else {
                WAKE_CHAPEL
            };
            if hp == 0 {
                self.waiting.remove(&id);
            } else if hp < max || near(at, reach) {
                self.wake(ready, id, now, 0.8);
            }
        }
        // The waves leave their chapels on the clock.
        for (index, (at, wave)) in WAVES.iter().enumerate() {
            if self.called.contains_key(&index) || since < *at {
                continue;
            }
            self.call(ready, game, index, *wave);
        }
        // The High Priest at half health calls every chapel at once.
        if let Some((hp, max)) = health(LEADER) {
            if !self.rallied && hp > 0 && hp * 2 <= max {
                self.rallied = true;
                game.scene.cues.push(Cue {
                    at: now,
                    actor: LEADER,
                    action: Action::Yell {
                        text: "Faithful, to me! Defend the rite!".into(),
                        animation: State::Yell.into(),
                    },
                });
                for (index, (_, wave)) in WAVES.iter().enumerate() {
                    if !self.called.contains_key(&index) {
                        self.call(ready, game, index, *wave);
                    }
                }
            }
            if hp == 0 && self.awakened.is_none() {
                self.awaken(
                    ready,
                    game,
                    false,
                    "My priest is fallen. Then I will finish this myself!",
                );
            }
        }
        // The chant, slowed by every acolyte no longer in it.
        if self.awakened.is_none() {
            let chanting = self.chanters();
            self.progress =
                (self.progress + dt * chanting as f32 / CHANTERS.len() as f32).min(RITUAL_SECONDS);
            if self.progress >= RITUAL_SECONDS {
                self.awaken(
                    ready,
                    game,
                    true,
                    "The rite is complete. I am ensouled, and I am awake!",
                );
            }
        }
    }

    fn call(
        &mut self,
        ready: &mut BTreeMap<u64, f32>,
        game: &mut Game,
        index: usize,
        wave: [u64; 4],
    ) {
        self.called.insert(index, game.time);
        for (k, id) in wave.into_iter().enumerate() {
            self.wake(ready, id, game.time, 1.5 + k as f32 * 0.3);
        }
        if let Some(first) = wave.into_iter().find(|id| game.actor_life(*id).is_some()) {
            game.scene.cues.push(Cue {
                at: game.time,
                actor: first,
                action: Action::Yell {
                    text: if index == 0 {
                        "Brothers, rise from the chapels!"
                    } else {
                        "For the ensouled master!"
                    }
                    .into(),
                    animation: State::Yell.into(),
                },
            });
        }
    }
}

/// The encounter's tuning for the great crypt: a longer fight, the
/// scene's authored health, and a short warmup.
#[must_use]
pub fn definition() -> Definition {
    Definition {
        duration: 600.0,
        warmup: 3.0,
        stagger: 0.6,
        ..Definition::default()
    }
}

/// Starts the great crypt's fight in `instance`, at the moment the player
/// takes control.
///
/// # Errors
///
/// Returns a message when the scene, its collision, or its navigation
/// cannot be built.
pub fn game(instance: u64) -> Result<Game, String> {
    let scene = scene();
    scene.validate()?;
    let mut game = Game::combat_authored_definition(scene, false, instance, definition())?;
    if let Some(encounter) = game.encounter.as_mut() {
        encounter.ritual = Some(Ritual::new());
    }
    game.time = game.scene.cut_at;
    game.selected = GUARDS[0];
    // Behind the player, looking down the nave toward the dais.
    game.camera.yaw = 0.0;
    Ok(game)
}

/// The ritual's state in `game`, when it is the great crypt's fight.
#[must_use]
pub fn ritual(game: &Game) -> Option<&Ritual> {
    game.encounter.as_ref().and_then(|e| e.ritual.as_ref())
}

/// Whether `encounter` holds `actor` out of the fight.
pub(crate) fn held(encounter: &Encounter, actor: u64) -> bool {
    encounter.ritual.as_ref().is_some_and(|r| r.holds(actor))
}

#[cfg(test)]
mod tests;
