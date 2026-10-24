//! Coarse local-preview kernel. The JSON bridge is a prototype, not the full v1 ABI.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet, HashSet, VecDeque};
use std::f32::consts::{FRAC_PI_2, PI, TAU};
use std::ops::Deref;
use std::sync::Arc;

mod battle_participant;
mod building;
mod collision_scene;
pub use collision_scene::CollisionScene;
mod enemies;
#[cfg(test)]
mod enemy_tests;
#[cfg(test)]
mod forecast_tests;
mod gameplay;
#[cfg(test)]
mod gameplay_tests;
#[cfg(test)]
mod mmorpg_tests;
mod quick_combat;
mod quick_mmorpg;
mod static_collision;
pub use building::{Building, BuildingPlan};

const DT: f32 = 1.0 / 60.0;
const RADIUS: f32 = 0.3;
const HEIGHT: f32 = 1.8;
const MAX_ENTITIES: usize = 16_384;
const MAX_COLLIDERS: usize = 32_768;
const MAX_MESSAGE: usize = 4 * 1024 * 1024;
const EPS: f32 = 0.001;
// Adapted from OpenAgents Verse's Everglade controller; see third_party/openagents-camera/NOTICE.
const RUN_SPEED: f32 = 6.4008;
const BACKPEDAL_SPEED: f32 = 4.1148;
const SPRINT_MULT: f32 = 1.6;
const GRAVITY: f32 = 9.81 * 1.6;
const JUMP_VELOCITY: f32 = 6.2;

fn one() -> f32 {
    1.0
}
fn room_height() -> f32 {
    3.0
}
fn unset_floor() -> f32 {
    f32::NAN
}
fn default_spawn() -> [f32; 3] {
    [0.0, 0.0, 8.0]
}
fn camera_probe_radius() -> f32 {
    0.12
}
fn yes() -> bool {
    true
}
fn spell_range() -> f32 {
    80.
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Entity {
    pub id: String,
    pub kind: String,
    pub position: [f32; 3],
    #[serde(default)]
    pub yaw: f32,
    #[serde(default = "one")]
    pub scale: f32,
    #[serde(default)]
    pub half_extents: Option<[f32; 3]>,
    /// Local collision center. Omission preserves the previous floor-pivot convention.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub collision_offset: Option<[f32; 3]>,
    #[serde(default)]
    pub collidable: bool,
    #[serde(default)]
    pub building_plan: Option<BuildingPlan>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Collider {
    pub id: String,
    pub min: [f32; 3],
    pub max: [f32; 3],
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Room {
    pub id: String,
    pub center: [f32; 2],
    pub size: [f32; 2],
    #[serde(default = "room_height")]
    pub height: f32,
    #[serde(default = "unset_floor")]
    pub floor_y: f32,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WallCollider {
    id: String,
    room_id: String,
    side: String,
    min: [f32; 3],
    max: [f32; 3],
    #[serde(skip_serializing_if = "Option::is_none")]
    local_min: Option<[f32; 3]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    local_max: Option<[f32; 3]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    transform: Option<building::Transform>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Input {
    #[serde(default)]
    forward: f32,
    #[serde(default)]
    right: f32,
    #[serde(default)]
    up: f32,
    #[serde(default)]
    yaw_delta: f32,
    #[serde(default)]
    pitch_delta: f32,
    #[serde(default)]
    turn: f32,
    #[serde(default)]
    sprint: bool,
    #[serde(default)]
    jump: bool,
    #[serde(default)]
    // Visual action buttons travel with the input; the host supplies their movement lock.
    #[allow(dead_code)]
    attack: bool,
    #[serde(default)]
    #[allow(dead_code)]
    dance: bool,
    #[serde(default)]
    movement_locked: bool,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum CameraMode {
    #[default]
    Fly,
    Walk,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Camera {
    #[serde(default = "default_spawn")]
    pub position: [f32; 3],
    #[serde(default)]
    pub yaw: f32,
    #[serde(default)]
    pub pitch: f32,
    #[serde(default)]
    pub mode: CameraMode,
}
impl Default for Camera {
    fn default() -> Self {
        Self {
            position: default_spawn(),
            yaw: 0.0,
            pitch: 0.0,
            mode: CameraMode::Fly,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum Operation {
    ReplaceContent {
        entities: Vec<Entity>,
        rooms: Vec<Room>,
        #[serde(rename = "removedWalls")]
        removed_walls: Vec<String>,
        authoring: Authoring,
        checkpoint: Option<Camera>,
        fallback: Option<Camera>,
    },
    AddEntity {
        entity: Entity,
        #[serde(default, rename = "removedWalls")]
        removed_walls: Vec<String>,
    },
    RemoveEntity {
        id: String,
    },
    MoveEntity {
        id: String,
        position: [f32; 3],
    },
    SetEntityTransform {
        id: String,
        position: Option<[f32; 3]>,
        yaw: Option<f32>,
        scale: Option<f32>,
    },
    AddRoom {
        room: Room,
        #[serde(default)]
        index: Option<usize>,
    },
    RemoveRoom {
        id: String,
    },
    RemoveWall {
        #[serde(rename = "wallId")]
        wall_id: String,
    },
    RestoreWall {
        #[serde(rename = "wallId")]
        wall_id: String,
    },
    SetAuthoring {
        state: Authoring,
    },
    Undo,
    Redo,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Command {
    command_id: String,
    base_revision: u64,
    operations: Vec<Operation>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Request {
    Init {
        #[serde(default)]
        entities: Vec<Entity>,
        #[serde(default)]
        colliders: Vec<Collider>,
        #[serde(default)]
        rooms: Vec<Room>,
        #[serde(default)]
        buildings: Vec<Building>,
        #[serde(default, rename = "removedWalls")]
        removed_walls: Vec<String>,
        #[serde(default)]
        spawn: Camera,
        #[serde(default, rename = "groundY")]
        ground_y: f32,
    },
    Step {
        elapsed: f32,
        #[serde(default)]
        input: Input,
    },
    SetCamera {
        position: [f32; 3],
        yaw: f32,
        pitch: f32,
        #[serde(default)]
        mode: CameraMode,
    },
    SetCharacterController {
        controller: Option<quick_mmorpg::Configuration>,
    },
    BattleParticipantInit {
        terrain: battle_participant::Terrain,
        class: af_mmorpg_core::Class,
        position: [f32; 3],
        yaw: f32,
    },
    BattleParticipantStep {
        elapsed: f32,
        input: af_mmorpg_core::Controls,
        alive: bool,
        #[serde(default, rename = "yawDelta")]
        yaw_delta: f32,
        #[serde(default, rename = "pitchDelta")]
        pitch_delta: f32,
        #[serde(default)]
        crowd: Option<battle_participant::CrowdContext>,
    },
    BattleParticipantRespawn {
        position: [f32; 3],
        yaw: f32,
    },
    BattleParticipantSnapshot,
    BattleParticipantClear,
    CombatInit {
        #[serde(rename = "worldScale")]
        world_scale: f32,
        zombies: Vec<quick_combat::Spec>,
    },
    CombatAttack {
        #[serde(rename = "attackId")]
        attack_id: String,
    },
    CombatReset,
    MmorpgPredictionInit {
        #[serde(rename = "worldScale")]
        world_scale: f32,
        class: af_mmorpg_core::Class,
        state: af_mmorpg_core::Prediction,
    },
    MmorpgPredictionStep {
        input: af_mmorpg_core::Controls,
        ticks: u16,
    },
    MmorpgPredictionReconcile {
        state: af_mmorpg_core::Prediction,
        inputs: Vec<PredictionInput>,
    },
    SetStaticCollision {
        triangles: Vec<static_collision::Vertices>,
        #[serde(default)]
        owners: Option<Vec<u32>>,
        #[serde(default)]
        append: bool,
    },
    GameplayInit {
        objects: Vec<gameplay::Object>,
        seed: Option<u32>,
        #[serde(default)]
        append: bool,
        #[serde(default = "yes")]
        complete: bool,
        #[serde(default, rename = "targetGroups")]
        target_groups: Option<gameplay::TargetGroups>,
    },
    GameplayAim {
        origin: [f32; 3],
        direction: [f32; 3],
        #[serde(default = "spell_range", rename = "maxDistance")]
        max_distance: f32,
    },
    GameplayInitEnemies {
        enemies: Vec<enemies::Spec>,
    },
    GameplayPrepareCast {
        #[serde(rename = "castId")]
        cast_id: String,
        ability: gameplay::Ability,
        origin: [f32; 3],
        direction: [f32; 3],
    },
    GameplayForecastStart {
        #[serde(rename = "castId")]
        cast_id: String,
        ability: gameplay::Ability,
        origin: [f32; 3],
        direction: [f32; 3],
        #[serde(default, rename = "expectedVersion")]
        expected_version: Option<u64>,
    },
    GameplayForecastContinue {
        #[serde(rename = "forecastId")]
        forecast_id: u64,
        #[serde(rename = "expectedVersion")]
        expected_version: u64,
    },
    GameplayForecastCancel {
        #[serde(rename = "forecastId")]
        forecast_id: u64,
    },
    GameplayCast {
        #[serde(rename = "castId")]
        cast_id: String,
        ability: gameplay::Ability,
        origin: [f32; 3],
        direction: [f32; 3],
        #[serde(rename = "expectedVersion")]
        expected_version: Option<u64>,
        #[serde(default, rename = "castleFragments")]
        castle_fragments: Option<Vec<[u32; 2]>>,
        #[serde(default, rename = "fragmentMasks")]
        fragment_masks: Option<Vec<gameplay::FragmentMask>>,
    },
    GameplayReset,
    GameplayCancelCast,
    GameplaySnapshot,
    QueryCamera {
        from: [f32; 3],
        to: [f32; 3],
        #[serde(default = "camera_probe_radius")]
        radius: f32,
    },
    Snapshot,
    Prepare {
        #[serde(flatten)]
        command: Command,
    },
    Command {
        #[serde(flatten)]
        command: Command,
    },
}

/// Creator-owned values share the same transaction and history as geometry.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Authoring {
    #[serde(default)]
    pub environment: BTreeMap<String, f64>,
    #[serde(default)]
    pub materials: BTreeMap<String, BTreeMap<String, f64>>,
    #[serde(default)]
    pub effect: Option<String>,
    #[serde(default)]
    pub material_sources: BTreeMap<String, MaterialSource>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub planet_source: Option<MaterialSource>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MaterialSource {
    pub name: String,
    pub sha256: String,
    pub text: String,
}
impl Authoring {
    fn validate(&self) -> Result<(), String> {
        if self.planet_source.as_ref().is_some_and(|s| {
            s.name != "planet-v1.json"
                || s.text.len() > 24 * 1024
                || s.text.contains('\0')
                || serde_json::from_str::<Value>(&s.text).is_err()
                || s.sha256.len() != 64
                || !s
                    .sha256
                    .bytes()
                    .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
        }) || self.material_sources.len() + usize::from(self.planet_source.is_some()) > 8
            || self.material_sources.iter().any(|(key, source)| {
                key.len() != 64
                    || !key
                        .bytes()
                        .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
                    || source.name != format!("material-v1-{key}.json")
                    || source.sha256.len() != 64
                    || !source
                        .sha256
                        .bytes()
                        .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
                    || source.text.len() > 24 * 1024
                    || source.text.contains('\0')
                    || serde_json::from_str::<Value>(&source.text).is_err()
            })
            || self.environment.len() > 24
            || self.materials.len() > 32
            || self
                .effect
                .as_ref()
                .is_some_and(|s| s.len() > 48_000 || s.contains('\0'))
            || self
                .environment
                .iter()
                .any(|(k, v)| !valid_id(k) || !v.is_finite() || v.abs() > 100_000.0)
            || self.materials.iter().any(|(id, params)| {
                !valid_id(id)
                    || params.len() > 8
                    || params
                        .iter()
                        .any(|(k, v)| !valid_id(k) || !v.is_finite() || v.abs() > 100_000.0)
            })
        {
            return Err("Keep material and lighting edits within the supported limits.".into());
        }
        Ok(())
    }
}

#[derive(Clone, Default)]
struct World {
    authoring: Authoring,
    entities: Vec<Entity>,
    rooms: Vec<Room>,
    buildings: Vec<Building>,
    colliders: Vec<Collider>,
    removed: HashSet<String>,
    ground: f32,
}

#[derive(Clone)]
struct HistoryEntry {
    forward: Vec<Operation>,
    inverse: Vec<Operation>,
}

#[derive(Clone)]
struct CachedCommand {
    id: String,
    fingerprint: Vec<u8>,
    revision: u64,
    error: Option<String>,
}

/// Shared derived outputs survive simple prop edits and prepare-only engine clones.
#[derive(Clone)]
struct Shared<T>(Arc<T>);
impl<T: Default> Default for Shared<T> {
    fn default() -> Self {
        Self(Arc::new(T::default()))
    }
}
impl<T> Deref for Shared<T> {
    type Target = T;
    fn deref(&self) -> &T {
        self.0.as_ref()
    }
}
impl<T: Serialize> Serialize for Shared<T> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.as_ref().serialize(serializer)
    }
}

/// Work that produced the last accepted derived authoring state; validation is separate.
#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeWork {
    entities_validated: usize,
    prop_records_rebuilt: usize,
    structural_rebuilds: usize,
    collision_records_rebuilt: usize,
}

type PropColliders = BTreeMap<String, Arc<Collider>>;
struct CompiledWorld {
    collision: Shared<Vec<Collider>>,
    props: PropColliders,
    walls: Shared<Vec<WallCollider>>,
    geometry: Shared<building::Geometry>,
    work: RuntimeWork,
}
impl CompiledWorld {
    fn can_occupy(&self, position: [f32; 3]) -> bool {
        can_occupy(position, &self.collision) && can_occupy_props(position, &self.props)
    }
    fn support_height(&self, position: [f32; 3], max_y: f32, ground: f32) -> f32 {
        let mut result = ground;
        for collider in self
            .collision
            .iter()
            .chain(self.props.values().map(AsRef::as_ref))
        {
            if collider.max[1] > max_y + EPS {
                continue;
            }
            let dx = position[0] - position[0].clamp(collider.min[0], collider.max[0]);
            let dz = position[2] - position[2].clamp(collider.min[2], collider.max[2]);
            if dx * dx + dz * dz < RADIUS * RADIUS {
                result = result.max(collider.max[1]);
            }
        }
        self.geometry
            .support_height(position, max_y)
            .map_or(result, |height| result.max(height))
    }
}

#[derive(Clone)]
struct SpellForecast {
    id: u64,
    cast_id: String,
    ability: gameplay::Ability,
    origin: [f32; 3],
    direction: [f32; 3],
    version: u64,
    revision: u64,
    geometry: Option<Arc<static_collision::StaticGeometry>>,
    collision_revision: u64,
    candidate: Option<gameplay::Forecast>,
    ready: Option<Value>,
    phase: &'static str,
    ticks: u16,
    accepted_new: bool,
    restarts: u8,
}

#[derive(Clone)]
struct PreparedContext {
    cast_id: String,
    version: u64,
    revision: u64,
    collision_revision: u64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PredictionInput {
    input: af_mmorpg_core::Controls,
    ticks: u16,
}
#[derive(Clone)]
struct MmorpgPrediction {
    world_scale: f32,
    class: af_mmorpg_core::Class,
    state: af_mmorpg_core::Prediction,
}

#[derive(Clone)]
pub struct Engine {
    world: World,
    camera: Camera,
    quick_controller: Option<quick_mmorpg::Controller>,
    quick_combat: Option<quick_combat::Combat>,
    mmorpg_prediction: Option<MmorpgPrediction>,
    battle_participant: Option<battle_participant::Participant>,
    velocity_y: f32,
    velocity: [f32; 3],
    ahead: f32,
    side: f32,
    grounded: bool,
    tick: u64,
    revision: u64,
    accumulator: f32,
    dropped: f32,
    jump_held: bool,
    jump_pending: bool,
    collision: Shared<Vec<Collider>>,
    prop_collision: PropColliders,
    walls: Shared<Vec<WallCollider>>,
    building_geometry: Shared<building::Geometry>,
    runtime_work: RuntimeWork,
    static_geometry: Arc<static_collision::StaticGeometry>,
    collision_revision: u64,
    prepared_contexts: VecDeque<PreparedContext>,
    gameplay: gameplay::Gameplay,
    gameplay_staging: Option<gameplay::Builder>,
    forecast: Option<SpellForecast>,
    next_forecast: u64,
    seen: VecDeque<CachedCommand>,
    undo: Vec<HistoryEntry>,
    redo: Vec<HistoryEntry>,
}

impl Default for Engine {
    fn default() -> Self {
        Self {
            world: World::default(),
            camera: Camera::default(),
            quick_controller: None,
            quick_combat: None,
            mmorpg_prediction: None,
            battle_participant: None,
            velocity_y: 0.0,
            velocity: [0.0; 3],
            ahead: 0.0,
            side: 0.0,
            grounded: false,
            tick: 0,
            revision: 0,
            accumulator: 0.0,
            dropped: 0.0,
            jump_held: false,
            jump_pending: false,
            collision: Shared::default(),
            prop_collision: BTreeMap::new(),
            walls: Shared::default(),
            building_geometry: Shared::default(),
            runtime_work: RuntimeWork::default(),
            static_geometry: Arc::default(),
            collision_revision: 0,
            prepared_contexts: VecDeque::new(),
            gameplay: gameplay::Gameplay::default(),
            gameplay_staging: None,
            forecast: None,
            next_forecast: 0,
            seen: VecDeque::new(),
            undo: vec![],
            redo: vec![],
        }
    }
}

fn finite(values: &[f32]) -> bool {
    values.iter().all(|v| v.is_finite() && v.abs() <= 100_000.0)
}
fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 96
        && id
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"-_.:".contains(&c))
}
fn valid_collider(c: &Collider) -> bool {
    valid_id(&c.id) && finite(&c.min) && finite(&c.max) && (0..3).all(|a| c.min[a] < c.max[a])
}
fn bounds(room: &Room, side: usize) -> (f32, f32, f32, bool) {
    let [x, z] = room.center;
    let [w, d] = room.size;
    match side {
        0 => (z - d / 2.0, x - w / 2.0, x + w / 2.0, true),
        1 => (x + w / 2.0, z - d / 2.0, z + d / 2.0, false),
        2 => (z + d / 2.0, x - w / 2.0, x + w / 2.0, true),
        _ => (x - w / 2.0, z - d / 2.0, z + d / 2.0, false),
    }
}
const SIDES: [&str; 4] = ["north", "east", "south", "west"];

fn subtract(parts: Vec<(f32, f32)>, cut: (f32, f32)) -> Vec<(f32, f32)> {
    let mut out = vec![];
    for (lo, hi) in parts {
        if cut.1 <= lo + EPS || cut.0 >= hi - EPS {
            out.push((lo, hi));
            continue;
        }
        if cut.0 > lo + EPS {
            out.push((lo, cut.0.min(hi)));
        }
        if cut.1 < hi - EPS {
            out.push((cut.1.max(lo), hi));
        }
    }
    out
}

fn validate_entities(world: &World) -> Result<(), String> {
    if world.entities.len() > MAX_ENTITIES {
        return Err("This world has reached its preview limit.".into());
    }
    let mut ids = HashSet::new();
    for e in &world.entities {
        if !valid_id(&e.id)
            || e.kind.is_empty()
            || e.kind.len() > 128
            || !ids.insert(e.id.clone())
            || !finite(&e.position)
            || !e.yaw.is_finite()
            || !e.scale.is_finite()
            || !(0.05..=20.0).contains(&e.scale)
        {
            return Err("An object has an invalid position, size, or name.".into());
        }
        if e.collision_offset
            .is_some_and(|offset| !finite(&offset) || offset.iter().any(|v| v.abs() > 1000.0))
        {
            return Err("An object's collision center is invalid.".into());
        }
        if let Some(half) = e.half_extents {
            if !finite(&half) || half.iter().any(|&x| x <= 0.0 || x > 1000.0) {
                return Err("An object's collision size is invalid.".into());
            }
        } else if e.collidable {
            return Err("A solid object needs collision bounds.".into());
        }
    }
    Ok(())
}

fn prop_collider(entity: &Entity) -> Result<Option<Collider>, String> {
    if !entity.collidable {
        return Ok(None);
    }
    let half = entity
        .half_extents
        .ok_or("A solid object needs collision bounds.")?;
    let offset = entity.collision_offset.unwrap_or([0.0, half[1], 0.0]);
    let (s, c) = entity.yaw.sin_cos();
    let center = [
        entity.position[0] + (offset[0] * c + offset[2] * s) * entity.scale,
        entity.position[1] + offset[1] * entity.scale,
        entity.position[2] + (-offset[0] * s + offset[2] * c) * entity.scale,
    ];
    let extent = [
        (half[0] * c.abs() + half[2] * s.abs()) * entity.scale,
        half[1] * entity.scale,
        (half[2] * c.abs() + half[0] * s.abs()) * entity.scale,
    ];
    let collider = Collider {
        id: format!("entity:{}", entity.id),
        min: std::array::from_fn(|i| center[i] - extent[i]),
        max: std::array::from_fn(|i| center[i] + extent[i]),
    };
    if !finite(&collider.min)
        || !finite(&collider.max)
        || (0..3).any(|i| collider.min[i] >= collider.max[i])
    {
        return Err("An object's collision bounds exceed the supported world size.".into());
    }
    Ok(Some(collider))
}

fn can_occupy_props(position: [f32; 3], props: &PropColliders) -> bool {
    props
        .values()
        .all(|c| can_occupy(position, std::slice::from_ref(c.as_ref())))
}

fn compile_world(world: &World) -> Result<CompiledWorld, String> {
    if world.entities.len() > MAX_ENTITIES
        || world.colliders.len() > MAX_COLLIDERS
        || world.rooms.len() > 128
    {
        return Err("This world has reached its preview limit.".into());
    }
    if !world.ground.is_finite() {
        return Err("The ground height must be finite.".into());
    }
    let mut ids = HashSet::new();
    let mut collision = vec![];
    for c in &world.colliders {
        if !valid_collider(c) || !ids.insert(c.id.clone()) {
            return Err("A collider has invalid bounds or a duplicate name.".into());
        }
        collision.push(c.clone());
    }
    validate_entities(world)?;
    let mut props = BTreeMap::new();
    for entity in &world.entities {
        if let Some(collider) = prop_collider(entity)? {
            props.insert(entity.id.clone(), Arc::new(collider));
        }
    }
    ids.clear();
    for (i, r) in world.rooms.iter().enumerate() {
        if !valid_id(&r.id)
            || !ids.insert(r.id.clone())
            || !finite(&r.center)
            || !finite(&r.size)
            || r.size.iter().any(|&v| !(2.0..=32.0).contains(&v))
            || !(2.0..=12.0).contains(&r.height)
            || !r.floor_y.is_finite()
            || (r.floor_y - world.ground).abs() > EPS
        {
            return Err(
                "A room needs a unique name, valid dimensions, and the current ground level."
                    .into(),
            );
        }
        for prev in &world.rooms[..i] {
            if (r.center[0] - prev.center[0]).abs() < (r.size[0] + prev.size[0]) / 2.0 - EPS
                && (r.center[1] - prev.center[1]).abs() < (r.size[1] + prev.size[1]) / 2.0 - EPS
            {
                return Err("That room would overlap another room.".into());
            }
        }
    }
    let mut walls = vec![];
    for (i, r) in world.rooms.iter().enumerate() {
        for side in 0..4 {
            let id = format!("{}:{}", r.id, SIDES[side]);
            if world.removed.contains(&id) {
                continue;
            }
            let (coord, lo, hi, horiz) = bounds(r, side);
            let mut parts = vec![(lo, hi)];
            if side == 2 {
                let mid = (lo + hi) * 0.5;
                parts = subtract(parts, (mid - 1.0, mid + 1.0));
            }
            for (j, other) in world.rooms.iter().enumerate() {
                if i == j {
                    continue;
                }
                let (oc, ol, oh, _) = bounds(other, (side + 2) % 4);
                if (coord - oc).abs() > EPS {
                    continue;
                }
                let a = lo.max(ol);
                let b = hi.min(oh);
                if b - a < 0.1 {
                    continue;
                }
                if j < i {
                    parts = subtract(parts, (a, b));
                } else if b - a >= 1.8 {
                    let mid = (a + b) * 0.5;
                    let half = ((b - a - 0.2) / 2.0).min(1.0);
                    parts = subtract(parts, (mid - half, mid + half));
                }
            }
            for (a, b) in parts {
                let (min, max) = if horiz {
                    (
                        [a, r.floor_y, coord - 0.09],
                        [b, r.floor_y + r.height, coord + 0.09],
                    )
                } else {
                    (
                        [coord - 0.09, r.floor_y, a],
                        [coord + 0.09, r.floor_y + r.height, b],
                    )
                };
                collision.push(Collider {
                    id: id.clone(),
                    min,
                    max,
                });
                walls.push(WallCollider {
                    id: id.clone(),
                    room_id: r.id.clone(),
                    side: SIDES[side].into(),
                    min,
                    max,
                    local_min: None,
                    local_max: None,
                    transform: None,
                });
            }
        }
    }
    if collision.len() + props.len() > MAX_COLLIDERS {
        return Err("This world has too many collision surfaces.".into());
    }
    let geometry = building::compile(world)?;
    walls.extend(geometry.walls.clone());
    let response_size = serde_json::to_vec(&json!({"entities":world.entities,"rooms":world.rooms,"buildings":world.buildings,
        "colliders":world.colliders,"wallColliders":walls,"removedWalls":world.removed,"supports":geometry.supports,"ramps":geometry.ramps}))
    .map_err(|_| "The world contains an invalid value.")?
    .len();
    if response_size > MAX_MESSAGE - 4096 {
        return Err("This world is too large for the current preview bridge.".into());
    }
    let work = RuntimeWork {
        entities_validated: world.entities.len(),
        prop_records_rebuilt: world
            .entities
            .iter()
            .filter(|e| e.building_plan.is_none())
            .count(),
        structural_rebuilds: 1,
        collision_records_rebuilt: collision.len() + props.len(),
    };
    Ok(CompiledWorld {
        collision: Shared(Arc::new(collision)),
        props,
        walls: Shared(Arc::new(walls)),
        geometry: Shared(Arc::new(geometry)),
        work,
    })
}

fn can_occupy(position: [f32; 3], colliders: &[Collider]) -> bool {
    can_occupy_height(position, colliders, HEIGHT)
}
fn can_occupy_height(position: [f32; 3], colliders: &[Collider], height: f32) -> bool {
    for c in colliders {
        if position[1] + height <= c.min[1] + EPS || position[1] >= c.max[1] - EPS {
            continue;
        }
        let dx = position[0] - position[0].clamp(c.min[0], c.max[0]);
        let dz = position[2] - position[2].clamp(c.min[2], c.max[2]);
        if dx * dx + dz * dz < RADIUS * RADIUS - EPS * EPS {
            return false;
        }
    }
    true
}

pub(crate) fn sweep_box(
    from: [f32; 3],
    to: [f32; 3],
    radius: f32,
    min: [f32; 3],
    max: [f32; 3],
) -> f32 {
    let mut enter = 0.0f32;
    let mut exit = 1.0f32;
    for axis in 0..3 {
        let delta = to[axis] - from[axis];
        let lo = min[axis] - radius;
        let hi = max[axis] + radius;
        if delta.abs() < 1e-8 {
            if from[axis] < lo || from[axis] > hi {
                return 1.0;
            }
        } else {
            let a = (lo - from[axis]) / delta;
            let b = (hi - from[axis]) / delta;
            enter = enter.max(a.min(b));
            exit = exit.min(a.max(b));
            if enter > exit {
                return 1.0;
            }
        }
    }
    (enter - 0.001).clamp(0.0, 1.0)
}

impl Engine {
    pub fn handle_json(&mut self, bytes: &[u8]) -> Value {
        if bytes.len() > MAX_MESSAGE {
            return self.error("resource_limit", "That request is too large.");
        }
        match serde_json::from_slice::<Request>(bytes) {
            Ok(request) => self.dispatch(request),
            Err(_) => self.error(
                "invalid_request",
                "The request has an unsupported shape or value.",
            ),
        }
    }

    fn remember_prepared(&mut self, cast_id: &str) {
        if let Some(entry) = self
            .prepared_contexts
            .iter_mut()
            .find(|entry| entry.cast_id == cast_id)
        {
            entry.version = self.gameplay.version;
            entry.revision = self.revision;
            entry.collision_revision = self.collision_revision;
            return;
        }
        self.prepared_contexts.push_back(PreparedContext {
            cast_id: cast_id.into(),
            version: self.gameplay.version,
            revision: self.revision,
            collision_revision: self.collision_revision,
        });
        while self.prepared_contexts.len() > 16 {
            self.prepared_contexts.pop_front();
        }
    }
    fn forecast_reply(forecast: &SpellForecast) -> Value {
        let mut reply = json!({"ok":true,"type":"gameplay_forecast","forecastId":forecast.id,
            "version":forecast.version,"status":if forecast.ready.is_some() { "ready" } else { "pending" },
            "phase":forecast.phase,"ticks":forecast.ticks,"restarts":forecast.restarts});
        if let Some(plan) = &forecast.ready {
            reply
                .as_object_mut()
                .expect("forecast response object")
                .extend(plan.as_object().expect("forecast plan object").clone());
        }
        reply
    }
    fn error(&self, code: &str, message: &str) -> Value {
        json!({"ok":false,"error":{"code":code,"message":message},"revision":self.revision,"tick":self.tick,"player":self.player()})
    }
    fn can_stand(&self, position: [f32; 3]) -> bool {
        self.can_stand_world(position)
            && !self
                .quick_combat
                .as_ref()
                .is_some_and(|combat| combat.blocks_player(position))
    }
    fn can_stand_world(&self, position: [f32; 3]) -> bool {
        self.can_stand_height(position, HEIGHT)
    }
    fn can_stand_height(&self, position: [f32; 3], height: f32) -> bool {
        can_occupy_height(position, &self.collision, height)
            && self
                .prop_collision
                .values()
                .all(|c| can_occupy_height(position, std::slice::from_ref(c.as_ref()), height))
            && can_occupy_height(position, self.gameplay.rubble_colliders(), height)
            && self.building_geometry.can_occupy_height(position, height)
            && self.static_geometry.can_occupy(position, RADIUS, height)
    }
    fn support_height(&self, position: [f32; 3], max_y: f32) -> f32 {
        let mut result = self.world.ground;
        for c in self
            .collision
            .iter()
            .chain(self.prop_collision.values().map(AsRef::as_ref))
            .chain(self.gameplay.rubble_colliders())
        {
            if c.max[1] > max_y + EPS {
                continue;
            }
            let dx = position[0] - position[0].clamp(c.min[0], c.max[0]);
            let dz = position[2] - position[2].clamp(c.min[2], c.max[2]);
            if dx * dx + dz * dz < RADIUS * RADIUS {
                result = result.max(c.max[1]);
            }
        }
        let result = self
            .building_geometry
            .support_height(position, max_y)
            .map_or(result, |h| result.max(h));
        self.static_geometry
            .support_height(position, max_y, RADIUS)
            .map_or(result, |h| result.max(h))
    }
    fn step_support_height(&self, position: [f32; 3], max_y: f32) -> f32 {
        let result = self.support_height(position, max_y);
        self.static_geometry
            .step_height(position, max_y, RADIUS)
            .map_or(result, |height| result.max(height))
    }
    fn player(&self) -> Value {
        let [x, y, z] = self.camera.position;
        json!({"position":[x,y,z],"eye":[x,y+1.62,z],"yaw":self.camera.yaw,"pitch":self.camera.pitch,"grounded":self.grounded,"mode":self.camera.mode,
            "velocity":self.velocity,"verticalSpeed":self.velocity_y,"speed":self.velocity[0].hypot(self.velocity[2]),"ahead":self.ahead,"side":self.side,
            "controller":self.quick_controller.as_ref().map(|value|json!({"profile":value.configuration.profile,"worldScale":value.configuration.world_scale,"velocity":value.velocity}))})
    }
    fn publish_prediction(&mut self, prediction: MmorpgPrediction, elapsed_ticks: u16) {
        let before = self.camera.position;
        self.camera.position = prediction.state.position;
        self.camera.yaw = prediction.state.yaw;
        self.camera.mode = CameraMode::Walk;
        self.velocity_y = prediction.state.vertical_speed;
        self.grounded = prediction.state.grounded;
        self.velocity = if elapsed_ticks > 0 {
            std::array::from_fn(|i| {
                (self.camera.position[i] - before[i]) / (DT * elapsed_ticks as f32)
            })
        } else {
            [0.; 3]
        };
        if let Some(controller) = &mut self.quick_controller {
            controller.velocity = prediction.state.velocity;
        }
        self.mmorpg_prediction = Some(prediction);
    }
    fn sync_gameplay_collision(&mut self) {
        if self.static_geometry.disabled_count() != self.gameplay.destroyed.len() {
            Arc::make_mut(&mut self.static_geometry).set_disabled(&self.gameplay.destroyed);
            self.collision_revision = self.collision_revision.saturating_add(1);
        }
    }
    fn gameplay_aim(
        &self,
        origin: [f32; 3],
        direction: [f32; 3],
        max_distance: f32,
    ) -> Result<Option<gameplay::Aim>, &'static str> {
        let length = direction.iter().map(|n| n * n).sum::<f32>().sqrt();
        if !finite(&origin)
            || !finite(&direction)
            || origin.iter().any(|v| v.abs() > 20_000.)
            || !length.is_finite()
            || length < 0.01
            || !max_distance.is_finite()
            || !(0.1..=80.).contains(&max_distance)
        {
            return Err("Choose a valid aiming direction within the town.");
        }
        let direction = direction.map(|v| v / length);
        let hit = self
            .static_geometry
            .raycast(origin, direction, max_distance);
        let mut best = self.gameplay.aim(
            origin,
            direction,
            max_distance,
            hit,
            self.world.ground,
            &self.static_geometry,
        );
        for collider in self
            .collision
            .iter()
            .chain(self.prop_collision.values().map(AsRef::as_ref))
            .chain(self.gameplay.rubble_colliders())
        {
            if let Some((distance, normal)) =
                gameplay::ray_box(origin, direction, collider.min, collider.max)
                && distance > 0.01
                && distance <= max_distance
                && best.as_ref().is_none_or(|hit| distance < hit.distance)
            {
                best = Some(gameplay::Aim {
                    point: std::array::from_fn(|i| origin[i] + direction[i] * distance),
                    normal,
                    distance,
                    node_id: None,
                    enemy_id: None,
                });
            }
        }
        let end = std::array::from_fn(|i| origin[i] + direction[i] * max_distance);
        let fraction = self.building_geometry.camera_sweep(origin, end, 0.0001);
        if fraction < 1.
            && best
                .as_ref()
                .is_none_or(|hit| fraction * max_distance < hit.distance)
        {
            let distance = fraction * max_distance;
            best = Some(gameplay::Aim {
                point: std::array::from_fn(|i| origin[i] + direction[i] * distance),
                normal: direction.map(|v| -v),
                distance,
                node_id: None,
                enemy_id: None,
            });
        }
        if let Some(hit) = self.gameplay.enemies.aim(
            origin,
            direction,
            best.as_ref().map_or(max_distance, |hit| hit.distance),
        ) && self.spell_path_clear(origin, hit.point)
        {
            best = Some(gameplay::Aim {
                point: hit.point,
                normal: hit.normal,
                distance: hit.distance,
                node_id: None,
                enemy_id: Some(hit.id),
            });
        }
        Ok(best)
    }
    fn spell_path_clear(&self, from: [f32; 3], to: [f32; 3]) -> bool {
        let delta: [f32; 3] = std::array::from_fn(|i| to[i] - from[i]);
        let distance = delta.iter().map(|v| v * v).sum::<f32>().sqrt();
        if distance <= 0.02 {
            return true;
        }
        let direction = delta.map(|v| v / distance);
        let end = std::array::from_fn(|i| from[i] + direction[i] * (distance - 0.02));
        if self
            .static_geometry
            .raycast(from, direction, distance - 0.02)
            .is_some()
            || self.building_geometry.camera_sweep(from, end, 0.0001) < 1.
        {
            return false;
        }
        !self
            .collision
            .iter()
            .chain(self.prop_collision.values().map(AsRef::as_ref))
            .chain(self.gameplay.rubble_colliders())
            .any(|collider| sweep_box(from, end, 0., collider.min, collider.max) < 1.)
    }
    fn apply_enemy_impacts(&mut self) {
        let impacts = self.gameplay.take_enemy_impacts();
        if impacts.is_empty() {
            return;
        }
        let mut enemies = std::mem::take(&mut self.gameplay.enemies);
        for impact in impacts {
            enemies.damage(&impact, |from, to| self.spell_path_clear(from, to));
        }
        self.gameplay.enemies = enemies;
    }
    fn advance_enemies(&mut self) {
        if self.gameplay.enemies.actors.is_empty() {
            return;
        }
        let mut enemies = std::mem::take(&mut self.gameplay.enemies);
        enemies.advance(|position, delta, velocity_y, grounded| {
            self.move_enemy(position, delta, velocity_y, grounded)
        });
        self.gameplay.enemies = enemies;
    }
    fn advance_quick_combat(&mut self) {
        let Some(mut combat) = self.quick_combat.take() else {
            return;
        };
        combat.advance(
            self.camera.position,
            self.camera.yaw,
            |position, delta, velocity_y, grounded| {
                self.move_capsule(
                    position,
                    delta,
                    velocity_y,
                    grounded,
                    quick_combat::ZOMBIE_HEIGHT,
                )
            },
            |from, to| self.spell_path_clear(from, to),
        );
        self.quick_combat = Some(combat);
    }
    fn move_enemy(
        &self,
        position: [f32; 3],
        delta: [f32; 3],
        velocity_y: f32,
        was_grounded: bool,
    ) -> ([f32; 3], f32, bool) {
        self.move_capsule(position, delta, velocity_y, was_grounded, HEIGHT)
    }
    fn move_capsule(
        &self,
        position: [f32; 3],
        delta: [f32; 3],
        velocity_y: f32,
        was_grounded: bool,
        height: f32,
    ) -> ([f32; 3], f32, bool) {
        let mut body = af_mmorpg_core::Prediction::new(position, 0.0);
        body.vertical_speed = velocity_y;
        body.grounded = was_grounded;
        af_mmorpg_core::move_body(&mut body, delta, height, self);
        (body.position, body.vertical_speed, body.grounded)
    }
    fn reply(&self, kind: &str, complete: bool) -> Value {
        let mut v = json!({"ok":true,"type":kind,"revision":self.revision,"tick":self.tick,"player":self.player(),"droppedSeconds":self.dropped,
            "canUndo":!self.undo.is_empty(),"canRedo":!self.redo.is_empty(),"gameplay":self.gameplay.snapshot()});
        v["combat"] = self
            .quick_combat
            .as_ref()
            .map_or(Value::Null, |combat| combat.snapshot());
        if let Some(prediction) = &self.mmorpg_prediction {
            v["prediction"] = json!(prediction.state);
        }
        if complete {
            let mut removed: Vec<_> = self.world.removed.iter().collect();
            removed.sort();
            v["runtimeWork"] = json!(self.runtime_work);
            v["authoring"] = json!(self.world.authoring);
            v["entities"] = json!(self.world.entities);
            v["rooms"] = json!(self.world.rooms);
            v["buildings"] = json!(self.world.buildings);
            v["supports"] = json!(self.building_geometry.supports);
            v["ramps"] = json!(self.building_geometry.ramps);
            v["colliders"] = json!(self.world.colliders);
            v["wallColliders"] = json!(self.walls);
            v["removedWalls"] = json!(removed);
            v["groundY"] = json!(self.world.ground);
        }
        v
    }

    pub fn dispatch(&mut self, request: Request) -> Value {
        match request {
            Request::Init {
                mut entities,
                colliders,
                mut rooms,
                mut buildings,
                removed_walls,
                spawn,
                ground_y,
            } => {
                for room in &mut rooms {
                    if room.floor_y.is_nan() {
                        room.floor_y = ground_y;
                    }
                }
                for e in &mut entities {
                    if let Some(p) = &mut e.building_plan {
                        p.normalize();
                    }
                }
                for b in &mut buildings {
                    b.plan.normalize();
                }
                let mut next = World {
                    entities,
                    colliders,
                    rooms,
                    buildings,
                    ground: ground_y,
                    removed: HashSet::new(),
                    authoring: Authoring::default(),
                };
                let Ok(base) = compile_world(&next) else {
                    return self.error(
                        "invalid_world",
                        "The scene contains invalid objects, rooms, or collision bounds.",
                    );
                };
                if removed_walls.len() > 2048
                    || removed_walls
                        .iter()
                        .any(|id| !base.walls.iter().any(|w| &w.id == id))
                {
                    return self.error(
                        "invalid_world",
                        "A saved wall edit refers to a missing wall.",
                    );
                }
                next.removed = removed_walls.into_iter().collect();
                let Ok(compiled) = compile_world(&next) else {
                    return self.error("invalid_world", "The saved building edits could not open.");
                };
                if !finite(&spawn.position)
                    || !spawn.yaw.is_finite()
                    || !spawn.pitch.is_finite()
                    || (spawn.mode == CameraMode::Walk
                        && (!compiled.can_occupy(spawn.position)
                            || !compiled.geometry.can_occupy(spawn.position)
                            || !self.static_geometry.can_occupy_all(
                                spawn.position,
                                RADIUS,
                                HEIGHT,
                            )
                            || spawn.position[1] < ground_y - EPS))
                {
                    return self.error(
                        "invalid_spawn",
                        "Choose an open starting position above the ground.",
                    );
                }
                let spawn = Camera {
                    yaw: (spawn.yaw + PI).rem_euclid(TAU) - PI,
                    pitch: spawn.pitch.clamp(-FRAC_PI_2 + 0.01, FRAC_PI_2 - 0.01),
                    ..spawn
                };
                let support =
                    compiled.support_height(spawn.position, spawn.position[1] + EPS, ground_y);
                let support = self
                    .static_geometry
                    .support_height(spawn.position, spawn.position[1] + EPS, RADIUS)
                    .map_or(support, |height| support.max(height));
                let grounded =
                    spawn.mode == CameraMode::Walk && (spawn.position[1] - support).abs() < EPS;
                *self = Self {
                    world: next,
                    camera: spawn,
                    collision: compiled.collision,
                    prop_collision: compiled.props,
                    walls: compiled.walls,
                    building_geometry: compiled.geometry,
                    runtime_work: compiled.work,
                    static_geometry: self.static_geometry.clone(),
                    grounded,
                    next_forecast: self.next_forecast,
                    collision_revision: self.collision_revision.saturating_add(1),
                    ..Self::default()
                };
                Arc::make_mut(&mut self.static_geometry).clear_disabled();
                self.grounded = self.camera.mode == CameraMode::Walk
                    && (self.camera.position[1]
                        - self.support_height(self.camera.position, self.camera.position[1] + EPS))
                    .abs()
                        < EPS;
                self.reply("init", true)
            }
            Request::SetCamera {
                position,
                yaw,
                pitch,
                mode,
            } => {
                if !finite(&position)
                    || !yaw.is_finite()
                    || !pitch.is_finite()
                    || (mode == CameraMode::Walk
                        && (position[1] < self.world.ground - EPS || !self.can_stand(position)))
                {
                    return self.error("invalid_pose", "Choose an open position above the ground.");
                }
                self.camera = Camera {
                    position,
                    yaw: (yaw + PI).rem_euclid(TAU) - PI,
                    pitch: pitch.clamp(-FRAC_PI_2 + 0.01, FRAC_PI_2 - 0.01),
                    mode,
                };
                self.velocity_y = 0.0;
                self.velocity = [0.0; 3];
                if let Some(controller) = &mut self.quick_controller {
                    controller.velocity = 0.0;
                }
                self.ahead = 0.0;
                self.side = 0.0;
                self.accumulator = 0.0;
                self.jump_held = false;
                self.jump_pending = false;
                self.grounded = mode == CameraMode::Walk
                    && (position[1] - self.support_height(position, position[1] + EPS)).abs() < EPS;
                self.reply("set_camera", false)
            }
            Request::SetCharacterController { controller } => {
                if let Some(configuration) = &controller
                    && let Err(message) = configuration.validate()
                {
                    return self.error("invalid_controller", message);
                }
                self.quick_controller = controller.map(quick_mmorpg::Controller::new);
                self.quick_combat = None;
                self.mmorpg_prediction = None;
                self.velocity = [0.0; 3];
                self.ahead = 0.0;
                self.side = 0.0;
                self.jump_held = false;
                self.jump_pending = false;
                self.reply("set_character_controller", false)
            }
            Request::BattleParticipantInit {
                terrain,
                class,
                position,
                yaw,
            } => match battle_participant::Participant::new(terrain, class, position, yaw) {
                Ok(participant) => {
                    self.battle_participant = Some(participant);
                    json!({"ok":true,"type":"battle_participant_init","participant":self.battle_participant.as_ref().unwrap().snapshot()})
                }
                Err(message) => self.error("invalid_battle_participant", message),
            },
            Request::BattleParticipantStep {
                elapsed,
                input,
                alive,
                yaw_delta,
                pitch_delta,
                crowd,
            } => {
                let Some(participant) = &mut self.battle_participant else {
                    return self.error(
                        "battle_participant_unavailable",
                        "Enter the battle before moving.",
                    );
                };
                match participant.step(elapsed, input, alive, yaw_delta, pitch_delta, crowd) {
                    Ok(()) => {
                        json!({"ok":true,"type":"battle_participant_step","participant":participant.snapshot()})
                    }
                    Err(message) => self.error("invalid_battle_participant", message),
                }
            }
            Request::BattleParticipantRespawn { position, yaw } => {
                let Some(participant) = &mut self.battle_participant else {
                    return self.error(
                        "battle_participant_unavailable",
                        "Enter the battle before moving.",
                    );
                };
                match participant.respawn(position, yaw) {
                    Ok(()) => {
                        json!({"ok":true,"type":"battle_participant_respawn","participant":participant.snapshot()})
                    }
                    Err(message) => self.error("invalid_battle_participant", message),
                }
            }
            Request::BattleParticipantSnapshot => {
                json!({"ok":true,"type":"battle_participant_snapshot","participant":self.battle_participant.as_ref().map(|value|value.snapshot())})
            }
            Request::BattleParticipantClear => {
                self.battle_participant = None;
                json!({"ok":true,"type":"battle_participant_clear","participant":null})
            }
            Request::CombatInit {
                world_scale,
                zombies,
            } => {
                if self.camera.mode != CameraMode::Walk
                    || !self.quick_controller.as_ref().is_some_and(|controller| {
                        controller.configuration.world_scale == world_scale
                    })
                {
                    return self.error(
                        "invalid_combat",
                        "Open the walking character before starting combat.",
                    );
                }
                let combat =
                    match quick_combat::Combat::new(world_scale, zombies, self.camera.clone()) {
                        Ok(combat) => combat,
                        Err(message) => return self.error("invalid_combat", message),
                    };
                if combat.actors.iter().any(|actor| {
                    actor.position[1] < self.world.ground - EPS
                        || !self.can_stand_height(actor.position, quick_combat::ZOMBIE_HEIGHT)
                }) {
                    return self.error(
                        "invalid_combat",
                        "Choose open zombie starting positions above the ground.",
                    );
                }
                self.quick_combat = Some(combat);
                self.mmorpg_prediction = None;
                self.reply("combat_init", false)
            }
            Request::MmorpgPredictionInit {
                world_scale,
                class,
                state,
            } => {
                if !world_scale.is_finite()
                    || !(0.01..=1.0).contains(&world_scale)
                    || !class.is_player()
                    || !state.valid()
                {
                    return self.error(
                        "invalid_prediction",
                        "Choose valid character prediction state.",
                    );
                }
                self.quick_combat = None;
                self.publish_prediction(
                    MmorpgPrediction {
                        world_scale,
                        class,
                        state,
                    },
                    0,
                );
                self.reply("mmorpg_prediction_init", false)
            }
            Request::MmorpgPredictionStep { input, ticks } => {
                if !input.valid() || !(1..=4).contains(&ticks) {
                    return self.error(
                        "invalid_prediction",
                        "Use one to four valid prediction steps.",
                    );
                }
                let Some(mut prediction) = self.mmorpg_prediction.take() else {
                    return self.error(
                        "prediction_unavailable",
                        "Join the shared world before moving.",
                    );
                };
                for _ in 0..ticks {
                    af_mmorpg_core::predict(
                        &mut prediction.state,
                        input,
                        prediction.class,
                        prediction.world_scale,
                        &collision_scene::PredictionSurface(self),
                    );
                }
                self.tick = self.tick.saturating_add(ticks as u64);
                self.ahead = input.forward;
                self.side = 0.;
                self.publish_prediction(prediction, ticks);
                self.reply("mmorpg_prediction_step", false)
            }
            Request::MmorpgPredictionReconcile { state, inputs } => {
                if !state.valid()
                    || inputs.len() > 120
                    || inputs
                        .iter()
                        .any(|v| !v.input.valid() || !(1..=4).contains(&v.ticks))
                    || inputs.iter().map(|v| v.ticks as u32).sum::<u32>() > 120
                {
                    return self.error("invalid_prediction", "Use a bounded character correction.");
                }
                let Some(mut prediction) = self.mmorpg_prediction.take() else {
                    return self.error(
                        "prediction_unavailable",
                        "Join the shared world before correcting movement.",
                    );
                };
                prediction.state = state;
                for pending in inputs {
                    for _ in 0..pending.ticks {
                        af_mmorpg_core::predict(
                            &mut prediction.state,
                            pending.input,
                            prediction.class,
                            prediction.world_scale,
                            &collision_scene::PredictionSurface(self),
                        );
                    }
                }
                self.publish_prediction(prediction, 0);
                self.reply("mmorpg_prediction_reconcile", false)
            }
            Request::CombatAttack { attack_id } => {
                let Some(combat) = self.quick_combat.as_mut() else {
                    return self.error(
                        "combat_unavailable",
                        "Open the zombie encounter before attacking.",
                    );
                };
                let (accepted, duplicate) = match combat.attack(&attack_id) {
                    Ok(result) => result,
                    Err(message) => return self.error("invalid_attack", message),
                };
                let mut reply = self.reply("combat_attack", false);
                reply["accepted"] = json!(accepted);
                reply["duplicate"] = json!(duplicate);
                reply
            }
            Request::CombatReset => {
                let Some(previous) = self.quick_combat.as_ref() else {
                    return self.error(
                        "combat_unavailable",
                        "Open the zombie encounter before restarting.",
                    );
                };
                let Ok(combat) = quick_combat::Combat::new(
                    self.quick_controller
                        .as_ref()
                        .expect("combat has a controller")
                        .configuration
                        .world_scale,
                    previous.specs.clone(),
                    previous.spawn.clone(),
                ) else {
                    return self.error("invalid_combat", "The encounter could not restart.");
                };
                if !self.can_stand_world(combat.spawn.position)
                    || combat.actors.iter().any(|actor| {
                        !self.can_stand_height(actor.position, quick_combat::ZOMBIE_HEIGHT)
                    })
                {
                    return self.error(
                        "combat_occupied",
                        "Clear the starting positions before restarting.",
                    );
                }
                self.camera = combat.spawn.clone();
                self.quick_combat = Some(combat);
                self.quick_controller
                    .as_mut()
                    .expect("combat has a controller")
                    .velocity = 0.0;
                self.velocity = [0.0; 3];
                self.velocity_y = 0.0;
                self.ahead = 0.0;
                self.side = 0.0;
                self.accumulator = 0.0;
                self.jump_held = false;
                self.jump_pending = false;
                self.grounded = (self.camera.position[1]
                    - self.support_height(self.camera.position, self.camera.position[1] + EPS))
                .abs()
                    < EPS;
                self.reply("combat_reset", false)
            }
            Request::SetStaticCollision {
                triangles,
                owners,
                append,
            } => {
                let mut replacement = static_collision::StaticGeometry::default();
                let geometry = if append {
                    Arc::make_mut(&mut self.static_geometry)
                } else {
                    replacement = static_collision::StaticGeometry::default();
                    &mut replacement
                };
                if let Err(message) = geometry.append_owned(triangles, owners) {
                    return self.error("invalid_collision", message);
                }
                if !append {
                    self.static_geometry = Arc::new(replacement);
                }
                self.collision_revision = self.collision_revision.saturating_add(1);
                let mut result = self.reply("set_static_collision", false);
                result["triangleCount"] = json!(self.static_geometry.len());
                result
            }
            Request::GameplayInit {
                objects,
                seed,
                append,
                complete,
                target_groups,
            } => {
                if target_groups.is_some() && !complete {
                    return self.error(
                        "invalid_gameplay",
                        "Add the castle group with the town's final chunk.",
                    );
                }
                if append {
                    let Some(builder) = &mut self.gameplay_staging else {
                        return self.error(
                            "invalid_gameplay",
                            "Start the town's breakable pieces before adding another chunk.",
                        );
                    };
                    if seed.is_some_and(|s| s != builder.seed) {
                        return self.error(
                            "invalid_gameplay",
                            "Use the same scene seed for every chunk.",
                        );
                    }
                    if let Err(message) = builder.append(objects) {
                        return self.error("invalid_gameplay", message);
                    }
                } else {
                    let mut builder = gameplay::Builder::new(seed.unwrap_or(1));
                    if let Err(message) = builder.append(objects) {
                        return self.error("invalid_gameplay", message);
                    }
                    self.gameplay_staging = Some(builder);
                }
                let count = self
                    .gameplay_staging
                    .as_ref()
                    .map_or(0, |builder| builder.len());
                if complete {
                    let mut candidate = match self
                        .gameplay_staging
                        .as_ref()
                        .expect("staged gameplay")
                        .finish(target_groups.unwrap_or_default())
                    {
                        Ok(candidate) => candidate,
                        Err(message) => return self.error("invalid_gameplay", message),
                    };
                    if self.camera.mode == CameraMode::Walk
                        && !self.static_geometry.can_occupy_all(
                            self.camera.position,
                            RADIUS,
                            HEIGHT,
                        )
                    {
                        return self.error(
                            "blocked_reset",
                            "Step away from the broken wall before rebuilding the town.",
                        );
                    }
                    candidate.version = self.gameplay.version.saturating_add(1);
                    self.gameplay = candidate;
                    self.prepared_contexts.clear();
                    self.gameplay_staging = None;
                    Arc::make_mut(&mut self.static_geometry).clear_disabled();
                }
                let mut result = self.reply("gameplay_init", false);
                result["objectCount"] = json!(count);
                result["complete"] = json!(complete);
                result
            }
            Request::GameplayInitEnemies { enemies } => {
                if self.gameplay_staging.is_some() {
                    return self.error("gameplay_busy", "Wait for the town to finish opening.");
                }
                let actors = match enemies::Enemies::new(enemies) {
                    Ok(actors) => actors,
                    Err(message) => return self.error("invalid_enemies", message),
                };
                if actors.actors.iter().any(|actor| {
                    actor.position[1] < self.world.ground - EPS || !self.can_stand(actor.position)
                }) {
                    return self.error(
                        "invalid_enemies",
                        "Place each enemy in a clear space above the ground.",
                    );
                }
                self.gameplay.enemies = actors;
                self.gameplay.version = self.gameplay.version.saturating_add(1);
                self.reply("gameplay_init_enemies", false)
            }
            Request::GameplayAim {
                origin,
                direction,
                max_distance,
            } => match self.gameplay_aim(origin, direction, max_distance) {
                Ok(aim) => json!({"ok":true,"type":"gameplay_aim","aim":aim}),
                Err(message) => self.error("invalid_aim", message),
            },
            Request::GameplayPrepareCast {
                cast_id,
                ability,
                origin,
                direction,
            } => {
                if self.gameplay_staging.is_some() {
                    return self.error(
                        "gameplay_busy",
                        "Wait for the town's breakable pieces to finish opening.",
                    );
                }
                let range = if ability == gameplay::Ability::LightningBolt {
                    30.48
                } else {
                    80.
                };
                let aim = match self.gameplay_aim(origin, direction, range) {
                    Ok(aim) => aim,
                    Err(message) => return self.error("invalid_aim", message),
                };
                match self.gameplay.prepare(
                    cast_id.clone(),
                    ability,
                    origin,
                    direction,
                    aim,
                    &self.static_geometry,
                ) {
                    Ok(mut plan) => {
                        self.remember_prepared(&cast_id);
                        plan["ok"] = json!(true);
                        plan["type"] = json!("gameplay_prepare_cast");
                        plan
                    }
                    Err(message) => self.error("invalid_spell", message),
                }
            }
            Request::GameplayForecastStart {
                cast_id,
                ability,
                origin,
                direction,
                expected_version,
            } => {
                if self.gameplay_staging.is_some()
                    || self
                        .forecast
                        .as_ref()
                        .is_some_and(|job| job.candidate.is_some())
                {
                    return self.error(
                        "gameplay_busy",
                        "Wait for the current spell to finish preparing.",
                    );
                }
                if expected_version.is_some_and(|version| version != self.gameplay.version) {
                    return self.error("stale_forecast", "The town changed. Aim and cast again.");
                }
                if self.next_forecast >= 9_007_199_254_740_991 {
                    return self.error(
                        "resource_limit",
                        "Reopen the world before preparing another spell.",
                    );
                }
                let range = if ability == gameplay::Ability::LightningBolt {
                    30.48
                } else {
                    80.
                };
                let aim = match self.gameplay_aim(origin, direction, range) {
                    Ok(aim) => aim,
                    Err(message) => return self.error("invalid_aim", message),
                };
                let candidate = match self.gameplay.start_forecast(
                    cast_id.clone(),
                    ability,
                    origin,
                    direction,
                    aim,
                ) {
                    Ok(candidate) => candidate,
                    Err(message) => return self.error("invalid_spell", message),
                };
                self.next_forecast += 1;
                let job = SpellForecast {
                    id: self.next_forecast,
                    cast_id,
                    ability,
                    origin,
                    direction,
                    version: self.gameplay.version,
                    revision: self.revision,
                    geometry: Some(self.static_geometry.clone()),
                    collision_revision: self.collision_revision,
                    accepted_new: !candidate.previously_prepared(),
                    candidate: Some(candidate),
                    ready: None,
                    phase: "cast",
                    ticks: 0,
                    restarts: 0,
                };
                let reply = Self::forecast_reply(&job);
                self.forecast = Some(job);
                reply
            }
            Request::GameplayForecastContinue {
                forecast_id,
                expected_version,
            } => {
                let Some(mut job) = self.forecast.take() else {
                    return self.error("invalid_forecast", "Prepare this spell before continuing.");
                };
                if job.id != forecast_id {
                    self.forecast = Some(job);
                    return self.error(
                        "invalid_forecast",
                        "Continue the current spell preparation.",
                    );
                }
                if job.version != expected_version {
                    self.forecast = Some(job);
                    return self.error(
                        "invalid_forecast",
                        "Continue this spell with its prepared world version.",
                    );
                }
                if job.version != self.gameplay.version
                    || job.revision != self.revision
                    || job.collision_revision != self.collision_revision
                    || self.gameplay_staging.is_some()
                {
                    if job.ready.is_some() && job.accepted_new {
                        self.gameplay.discard_prepared(&job.cast_id, job.version);
                        self.prepared_contexts
                            .retain(|entry| entry.cast_id != job.cast_id);
                    }
                    return self.error("stale_forecast", "The town changed. Aim and cast again.");
                }
                if job.ready.is_some() {
                    if !self.gameplay.is_prepared(&job.cast_id) {
                        return self
                            .error("invalid_forecast", "Prepare this spell before continuing.");
                    }
                    if self.gameplay.prepared_needs_refresh(&job.cast_id) {
                        if job.restarts >= 16 {
                            self.gameplay.discard_prepared(&job.cast_id, job.version);
                            self.prepared_contexts
                                .retain(|entry| entry.cast_id != job.cast_id);
                            return self.error(
                                "resource_limit",
                                "Wait for the falling spells to finish before casting again.",
                            );
                        }
                        let candidate = match self.gameplay.start_forecast(
                            job.cast_id.clone(),
                            job.ability,
                            job.origin,
                            job.direction,
                            None,
                        ) {
                            Ok(candidate) => candidate,
                            Err(message) => return self.error("invalid_spell", message),
                        };
                        job.candidate = Some(candidate);
                        job.geometry = Some(self.static_geometry.clone());
                        job.ready = None;
                        job.phase = "cast";
                        job.ticks = 0;
                        job.restarts += 1;
                        let reply = Self::forecast_reply(&job);
                        self.forecast = Some(job);
                        return reply;
                    }
                }
                if let Some(candidate) = &mut job.candidate {
                    let plan = match candidate
                        .advance(job.geometry.as_ref().expect("pending forecast geometry"))
                    {
                        Ok(plan) => plan,
                        Err(message) => return self.error("invalid_spell", message),
                    };
                    job.phase = candidate.phase();
                    job.ticks = candidate.ticks();
                    if let Some(plan) = plan {
                        if let Err(message) = self.gameplay.accept_forecast(candidate, true) {
                            return self.error("stale_forecast", message);
                        }
                        self.remember_prepared(&job.cast_id);
                        job.ready = Some(plan);
                        job.candidate = None;
                        job.geometry = None;
                    }
                }
                let reply = Self::forecast_reply(&job);
                self.forecast = Some(job);
                reply
            }
            Request::GameplayForecastCancel { forecast_id } => {
                if self
                    .forecast
                    .as_ref()
                    .is_some_and(|job| job.id != forecast_id)
                {
                    return self.error("invalid_forecast", "Cancel the current spell preparation.");
                }
                let cancelled = self.forecast.take();
                if let Some(job) = &cancelled {
                    if job.ready.is_some() && job.accepted_new {
                        self.gameplay.discard_prepared(&job.cast_id, job.version);
                        self.prepared_contexts
                            .retain(|entry| entry.cast_id != job.cast_id);
                    }
                }
                json!({"ok":true,"type":"gameplay_forecast_cancel","forecastId":forecast_id,"cancelled":cancelled.is_some()})
            }
            Request::GameplayCast {
                cast_id,
                ability,
                origin,
                direction,
                expected_version,
                castle_fragments,
                fragment_masks,
            } => {
                if self.gameplay_staging.is_some() {
                    return self.error(
                        "gameplay_busy",
                        "Wait for the town's breakable pieces to finish opening.",
                    );
                }
                let range = if ability == gameplay::Ability::LightningBolt {
                    30.48
                } else {
                    80.
                };
                let aim = match self.gameplay_aim(origin, direction, range) {
                    Ok(aim) => aim,
                    Err(message) => return self.error("invalid_aim", message),
                };
                if let Some(context) = self
                    .prepared_contexts
                    .iter()
                    .find(|entry| entry.cast_id == cast_id)
                {
                    if context.version != self.gameplay.version
                        || context.revision != self.revision
                        || context.collision_revision != self.collision_revision
                    {
                        return self
                            .error("stale_forecast", "The town changed. Aim and cast again.");
                    }
                } else if self.gameplay.is_prepared(&cast_id) {
                    return self.error(
                        "stale_forecast",
                        "Prepare this spell in the current world before casting.",
                    );
                }
                let was_prepared = self.gameplay.is_prepared(&cast_id);
                match self.gameplay.cast(
                    cast_id.clone(),
                    ability,
                    origin,
                    direction,
                    aim,
                    expected_version,
                    &self.static_geometry,
                    castle_fragments,
                    fragment_masks,
                ) {
                    Ok(cast) => {
                        self.forecast = None;
                        self.prepared_contexts.clear();
                        self.apply_enemy_impacts();
                        self.sync_gameplay_collision();
                        self.gameplay.refresh_rubble(
                            (self.camera.mode == CameraMode::Walk).then_some(self.camera.position),
                        );
                        let mut result = self.reply("gameplay_cast", false);
                        result["cast"] = json!(cast);
                        result
                    }
                    Err(message) => {
                        if !was_prepared && self.gameplay.is_prepared(&cast_id) {
                            self.remember_prepared(&cast_id);
                        }
                        self.error("invalid_spell", message)
                    }
                }
            }
            Request::GameplayReset => {
                if self.camera.mode == CameraMode::Walk
                    && !self
                        .static_geometry
                        .can_occupy_all(self.camera.position, RADIUS, HEIGHT)
                {
                    return self.error(
                        "blocked_reset",
                        "Step away from the broken wall before rebuilding the town.",
                    );
                }
                self.gameplay.reset();
                self.prepared_contexts.clear();
                self.forecast = None;
                self.gameplay_staging = None;
                Arc::make_mut(&mut self.static_geometry).clear_disabled();
                self.reply("gameplay_reset", false)
            }
            Request::GameplaySnapshot => self.reply("gameplay_snapshot", false),
            Request::GameplayCancelCast => {
                self.forecast = None;
                self.prepared_contexts.clear();
                self.gameplay.cancel_cast();
                self.reply("gameplay_cancel_cast", false)
            }
            Request::QueryCamera { from, to, radius } => {
                let distance = from
                    .iter()
                    .zip(to)
                    .map(|(a, b)| (b - a).powi(2))
                    .sum::<f32>()
                    .sqrt();
                if !finite(&from)
                    || !finite(&to)
                    || !radius.is_finite()
                    || !(0.01..=1.0).contains(&radius)
                    || distance > 80.0
                    || from.iter().chain(to.iter()).any(|v| v.abs() > 100_000.0)
                {
                    return self.error(
                        "invalid_input",
                        "The camera movement is outside its supported range.",
                    );
                }
                let mut fraction = self.static_geometry.sweep(from, to, radius);
                for collider in self
                    .collision
                    .iter()
                    .chain(self.prop_collision.values().map(AsRef::as_ref))
                    .chain(self.gameplay.rubble_colliders())
                {
                    fraction =
                        fraction.min(sweep_box(from, to, radius, collider.min, collider.max));
                }
                // Derived building walls retain their exact local bounds when rotated.
                fraction = fraction.min(self.building_geometry.camera_sweep(from, to, radius));
                json!({"ok":true,"type":"query_camera","fraction":fraction})
            }
            Request::Step { elapsed, input } => self.step(elapsed, input),
            Request::Snapshot => self.reply("snapshot", true),
            Request::Prepare { command } => self.clone().command(command),
            Request::Command { command } => self.command(command),
        }
    }

    fn step(&mut self, elapsed: f32, input: Input) -> Value {
        if !elapsed.is_finite()
            || !(0.0..=10.0).contains(&elapsed)
            || !finite(&[
                input.forward,
                input.right,
                input.up,
                input.yaw_delta,
                input.pitch_delta,
                input.turn,
            ])
            || [input.forward, input.right, input.up, input.turn]
                .iter()
                .any(|v| v.abs() > 1.0)
        {
            return self.error(
                "invalid_input",
                "Movement values must be finite and within range.",
            );
        }
        let source_controller =
            self.quick_controller.is_some() && self.camera.mode == CameraMode::Walk;
        if !source_controller {
            self.camera.yaw = (self.camera.yaw + input.yaw_delta + PI).rem_euclid(TAU) - PI;
            self.camera.pitch =
                (self.camera.pitch + input.pitch_delta).clamp(-FRAC_PI_2 + 0.01, FRAC_PI_2 - 0.01);
        }
        if !source_controller && input.jump && !self.jump_held {
            self.jump_pending = true;
        }
        self.accumulator += elapsed.min(0.25);
        self.dropped += (elapsed - 0.25).max(0.0);
        let mut ticks = 0;
        while self.accumulator + 1e-7 >= DT && ticks < 4 {
            self.tick_once(&input);
            self.accumulator = (self.accumulator - DT).max(0.0);
            ticks += 1;
        }
        if self.accumulator >= DT {
            let keep = self.accumulator % DT;
            self.dropped += self.accumulator - keep;
            self.accumulator = keep;
        }
        self.jump_held = input.jump;
        self.reply("step", false)
    }

    fn tick_once(&mut self, input: &Input) {
        self.gameplay.advance_events();
        self.apply_enemy_impacts();
        self.sync_gameplay_collision();
        self.gameplay.advance_debris(
            &self.static_geometry,
            self.world.ground,
            (self.camera.mode == CameraMode::Walk).then_some(self.camera.position),
        );
        self.advance_enemies();
        self.advance_quick_combat();
        if self
            .quick_combat
            .as_ref()
            .is_some_and(|combat| !combat.player.alive)
        {
            self.velocity = [0.0; 3];
            self.velocity_y = 0.0;
            self.ahead = 0.0;
            self.side = 0.0;
            if let Some(controller) = &mut self.quick_controller {
                controller.velocity = 0.0;
            }
            self.tick += 1;
            return;
        }
        let previous = self.camera.position;
        let fly = self.camera.mode == CameraMode::Fly;
        let quick_motion = if !fly {
            self.quick_controller.as_mut().map(|controller| {
                controller.advance(
                    input.forward,
                    input.turn,
                    input.sprint,
                    input.movement_locked,
                    DT,
                )
            })
        } else {
            None
        };
        let yaw_delta = quick_motion.map_or(input.turn * PI * DT, |motion| motion.1);
        self.camera.yaw = (self.camera.yaw + yaw_delta + PI).rem_euclid(TAU) - PI;
        let speed = if fly {
            if input.sprint { 8.0 } else { 3.5 }
        } else if input.forward < 0.0 {
            BACKPEDAL_SPEED
        } else if input.sprint && input.forward > 0.0 {
            RUN_SPEED * SPRINT_MULT
        } else {
            RUN_SPEED
        };
        let (sy, cy) = self.camera.yaw.sin_cos();
        let (sp, cp) = self.camera.pitch.sin_cos();
        let forward_scale = if fly { cp } else { 1.0 };
        let mut delta = [
            (sy * input.forward * forward_scale + cy * input.right) * speed * DT,
            if fly {
                (sp * input.forward + input.up) * speed * DT
            } else {
                0.0
            },
            (-cy * input.forward * forward_scale + sy * input.right) * speed * DT,
        ];
        if let Some((distance, _)) = quick_motion {
            delta = [sy * distance, 0.0, -cy * distance];
        }
        let distance = (delta.iter().map(|x| x * x).sum::<f32>()).sqrt();
        if quick_motion.is_none() && distance > speed * DT {
            for d in &mut delta {
                *d *= speed * DT / distance;
            }
        }
        if fly {
            for (p, d) in self.camera.position.iter_mut().zip(delta) {
                *p += d;
            }
            self.grounded = false;
        } else {
            let can_jump = self.grounded;
            self.grounded = false;
            if self.jump_pending && can_jump {
                self.velocity_y = JUMP_VELOCITY;
            }
            self.jump_pending = false;
            self.velocity_y = (self.velocity_y - GRAVITY * DT).max(-50.0);
            delta[1] = self.velocity_y * DT;
            let steps = ((delta.iter().map(|v| v.abs()).fold(0.0f32, f32::max) / 0.08).ceil()
                as usize)
                .max(1);
            let horizontal = delta[0] != 0.0 || delta[2] != 0.0;
            for _ in 0..steps {
                let mut together = self.camera.position;
                together[0] += delta[0] / steps as f32;
                together[2] += delta[2] / steps as f32;
                let support = if horizontal {
                    self.step_support_height(together, self.camera.position[1] + 0.25)
                } else {
                    self.support_height(together, self.camera.position[1] + 0.25)
                };
                let follows_support = (can_jump || self.grounded)
                    && self.velocity_y <= 0.0
                    && (support - self.camera.position[1]).abs() <= 0.3;
                if follows_support {
                    together[1] = support;
                }
                if self.can_stand(together) {
                    self.camera.position = together;
                    if follows_support {
                        self.grounded = true;
                        self.velocity_y = 0.0;
                    }
                } else {
                    for axis in [0, 2] {
                        let mut candidate = self.camera.position;
                        candidate[axis] += delta[axis] / steps as f32;
                        let support =
                            self.step_support_height(candidate, self.camera.position[1] + 0.25);
                        let follows_support = (can_jump || self.grounded)
                            && self.velocity_y <= 0.0
                            && (support - self.camera.position[1]).abs() <= 0.3;
                        if follows_support {
                            candidate[1] = support;
                        }
                        if self.can_stand(candidate) {
                            self.camera.position = candidate;
                            if follows_support {
                                self.grounded = true;
                                self.velocity_y = 0.0;
                            }
                        }
                    }
                }
                let mut candidate = self.camera.position;
                candidate[1] += if self.grounded && self.velocity_y == 0.0 {
                    0.0
                } else {
                    delta[1] / steps as f32
                };
                let support = self.support_height(candidate, self.camera.position[1] + EPS);
                if delta[1] <= 0.0
                    && candidate[1] <= support + EPS
                    && self.camera.position[1] >= support - EPS
                {
                    candidate[1] = support;
                    self.velocity_y = 0.0;
                    self.grounded = true;
                }
                if self.can_stand(candidate) {
                    self.camera.position = candidate;
                } else if delta[1] < 0.0
                    && !self.grounded
                    && let Some(slid) = self.static_geometry.slide_down(candidate, RADIUS, HEIGHT)
                    && self.can_stand(slid)
                {
                    self.camera.position = slid;
                } else {
                    self.velocity_y = 0.0;
                }
            }
        }
        self.velocity =
            std::array::from_fn(|axis| (self.camera.position[axis] - previous[axis]) / DT);
        self.ahead = input.forward;
        self.side = if quick_motion.is_some() {
            0.0
        } else {
            input.right
        };
        self.tick += 1;
    }

    fn command(&mut self, command: Command) -> Value {
        let fingerprint = serde_json::to_vec(&command).expect("typed command serializes");
        if let Some(seen) = self.seen.iter().find(|c| c.id == command.command_id) {
            if seen.fingerprint != fingerprint {
                return self.error(
                    "command_conflict",
                    "That command name was already used for a different edit.",
                );
            }
            if let Some(error) = &seen.error {
                return self.error("invalid_edit", error);
            }
            let original = seen.revision;
            let mut reply = self.reply("command", true);
            reply["duplicate"] = json!(true);
            reply["originalRevision"] = json!(original);
            return reply;
        }
        if !valid_id(&command.command_id)
            || command.operations.is_empty()
            || command.operations.len() > 8
        {
            return self.error(
                "invalid_command",
                "An edit needs a unique name and one to eight actions.",
            );
        }
        if command.base_revision != self.revision {
            return self.error("stale_revision", "The scene changed. Try that edit again.");
        }
        let history = command
            .operations
            .iter()
            .any(|o| matches!(o, Operation::Undo | Operation::Redo));
        let result = if history {
            if command.operations.len() != 1 {
                Err("Undo or redo must be its own edit.".into())
            } else {
                self.apply_history(matches!(command.operations[0], Operation::Redo))
            }
        } else {
            self.apply_operations(&command.operations, true)
        };
        let error = result.err();
        self.seen.push_back(CachedCommand {
            id: command.command_id,
            fingerprint,
            revision: self.revision,
            error: error.clone(),
        });
        while self.seen.len() > 64 {
            self.seen.pop_front();
        }
        match error {
            Some(message) => self.error("invalid_edit", &message),
            None => self.reply("command", true),
        }
    }

    fn compile_edit(
        &self,
        next: &World,
        operations: &[Operation],
    ) -> Result<CompiledWorld, String> {
        let mut changed = BTreeSet::new();
        for op in operations {
            match op {
                Operation::AddEntity {
                    entity,
                    removed_walls,
                } if entity.building_plan.is_none() && removed_walls.is_empty() => {
                    changed.insert(entity.id.clone());
                }
                Operation::RemoveEntity { id }
                | Operation::MoveEntity { id, .. }
                | Operation::SetEntityTransform { id, .. } => {
                    changed.insert(id.clone());
                }
                _ => return compile_world(next),
            }
        }
        if next.removed != self.world.removed
            || self
                .world
                .entities
                .iter()
                .chain(&next.entities)
                .any(|e| changed.contains(&e.id) && e.building_plan.is_some())
        {
            return compile_world(next);
        }
        validate_entities(next)?;
        let mut props = self.prop_collision.clone();
        let mut collision_records = 0;
        for id in &changed {
            let previous = props.remove(id);
            let collider = next
                .entities
                .iter()
                .find(|entity| &entity.id == id)
                .map(prop_collider)
                .transpose()?
                .flatten();
            if previous.is_some() || collider.is_some() {
                collision_records += 1;
            }
            if let Some(collider) = collider {
                props.insert(id.clone(), Arc::new(collider));
            }
        }
        if self.collision.len() + props.len() > MAX_COLLIDERS {
            return Err("This world has too many collision surfaces.".into());
        }
        let response_size = serde_json::to_vec(
            &json!({"entities":next.entities,"rooms":next.rooms,"buildings":next.buildings,
            "colliders":next.colliders,"wallColliders":self.walls,"removedWalls":next.removed,
            "supports":self.building_geometry.supports,"ramps":self.building_geometry.ramps}),
        )
        .map_err(|_| "The world contains an invalid value.")?
        .len();
        if response_size > MAX_MESSAGE - 4096 {
            return Err("This world is too large for the current preview bridge.".into());
        }
        Ok(CompiledWorld {
            collision: self.collision.clone(),
            props,
            walls: self.walls.clone(),
            geometry: self.building_geometry.clone(),
            work: RuntimeWork {
                entities_validated: next.entities.len(),
                prop_records_rebuilt: changed.len(),
                structural_rebuilds: 0,
                collision_records_rebuilt: collision_records,
            },
        })
    }

    fn apply_operations(&mut self, operations: &[Operation], record: bool) -> Result<(), String> {
        let replacing = operations
            .iter()
            .any(|o| matches!(o, Operation::ReplaceContent { .. }));
        if replacing && operations.len() != 1 {
            return Err("Open a saved world separately from other edits.".into());
        }
        let mut next = self.world.clone();
        let mut inverse = vec![];
        for operation in operations {
            let inv = apply(&mut next, operation)?;
            inverse.insert(0, inv);
        }
        let compiled = self.compile_edit(&next, operations)?;
        let mut camera = self.camera.clone();
        if let Some(Operation::ReplaceContent {
            checkpoint: Some(saved),
            fallback,
            ..
        }) = operations.first()
        {
            let safe = |c: &Camera| {
                finite(&c.position)
                    && c.yaw.is_finite()
                    && c.pitch.is_finite()
                    && c.position[1] >= next.ground - EPS
                    && compiled.can_occupy(c.position)
                    && compiled.geometry.can_occupy(c.position)
                    && self.static_geometry.can_occupy(c.position, RADIUS, HEIGHT)
            };
            camera = if safe(saved) {
                saved.clone()
            } else if let Some(fallback) = fallback.as_ref().filter(|c| safe(c)) {
                fallback.clone()
            } else {
                return Err(
                    "The saved place is blocked. Open from a clear position instead.".into(),
                );
            };
        }
        if camera.mode == CameraMode::Walk
            && (!compiled.can_occupy(camera.position)
                || !compiled.geometry.can_occupy(camera.position)
                || !self
                    .static_geometry
                    .can_occupy(camera.position, RADIUS, HEIGHT))
        {
            return Err("Step away before putting a wall or solid object there.".into());
        }
        if !replacing && camera.mode == CameraMode::Walk && self.grounded {
            let support = compiled.support_height(
                self.camera.position,
                self.camera.position[1] + 0.05,
                next.ground,
            );
            let support = self
                .static_geometry
                .support_height(self.camera.position, self.camera.position[1] + 0.05, RADIUS)
                .map_or(support, |height| support.max(height));
            if self.camera.position[1] - support > 0.25 {
                return Err("Step onto another floor before removing the one under you.".into());
            }
        }
        self.world = next;
        if replacing {
            self.camera = camera;
            self.velocity_y = 0.0;
            self.velocity = [0.; 3];
            self.ahead = 0.;
            self.side = 0.;
            self.grounded = false;
        }
        self.collision = compiled.collision;
        self.prop_collision = compiled.props;
        self.walls = compiled.walls;
        self.building_geometry = compiled.geometry;
        self.runtime_work = compiled.work;
        self.revision += 1;
        if replacing {
            self.undo.clear();
            self.redo.clear();
        } else if record {
            self.undo.push(HistoryEntry {
                forward: operations.to_vec(),
                inverse,
            });
            if self.undo.len() > 32 {
                self.undo.remove(0);
            }
            self.redo.clear();
        }
        Ok(())
    }

    fn apply_history(&mut self, redo: bool) -> Result<(), String> {
        let entry = if redo {
            self.redo.last()
        } else {
            self.undo.last()
        }
        .cloned()
        .ok_or_else(|| "There is no edit to undo or redo.".to_string())?;
        self.apply_operations(if redo { &entry.forward } else { &entry.inverse }, false)?;
        if redo {
            self.redo.pop();
            self.undo.push(entry);
        } else {
            self.undo.pop();
            self.redo.push(entry);
        }
        Ok(())
    }
}

fn apply(world: &mut World, op: &Operation) -> Result<Operation, String> {
    match op {
        Operation::ReplaceContent {
            entities,
            rooms,
            removed_walls,
            authoring,
            ..
        } => {
            authoring.validate()?;
            let inverse = Operation::ReplaceContent {
                entities: world.entities.clone(),
                rooms: world.rooms.clone(),
                removed_walls: world.removed.iter().cloned().collect(),
                authoring: world.authoring.clone(),
                checkpoint: None,
                fallback: None,
            };
            world.entities = entities.clone();
            world.rooms = rooms.clone();
            world.removed = removed_walls.iter().cloned().collect();
            world.authoring = authoring.clone();
            if !world.removed.is_empty() {
                let mut standing = world.clone();
                standing.removed.clear();
                let compiled = compile_world(&standing)?;
                if world
                    .removed
                    .iter()
                    .any(|id| !compiled.walls.iter().any(|wall| &wall.id == id))
                {
                    return Err("This save refers to an unavailable editable wall.".into());
                }
            }
            // Fixed scene collision and building definitions stay deployment-owned.
            Ok(inverse)
        }
        Operation::SetAuthoring { state } => {
            state.validate()?;
            let previous = std::mem::replace(&mut world.authoring, state.clone());
            Ok(Operation::SetAuthoring { state: previous })
        }
        Operation::AddEntity {
            entity,
            removed_walls,
        } => {
            if world.entities.iter().any(|e| e.id == entity.id) {
                return Err("That object already exists.".into());
            }
            let mut entity = entity.clone();
            if let Some(plan) = &mut entity.building_plan {
                plan.normalize();
            }
            if removed_walls
                .iter()
                .any(|wall| !wall.starts_with(&format!("{}:", entity.id)))
            {
                return Err("A restored wall belongs to another building.".into());
            }
            world.entities.push(entity.clone());
            if !removed_walls.is_empty() {
                let walls = compile_world(world)?.walls;
                if removed_walls
                    .iter()
                    .any(|id| !walls.iter().any(|wall| &wall.id == id))
                {
                    return Err("A restored edit refers to a missing building wall.".into());
                }
                world.removed.extend(removed_walls.iter().cloned());
            }
            Ok(Operation::RemoveEntity {
                id: entity.id.clone(),
            })
        }
        Operation::RemoveEntity { id } => {
            let i = world
                .entities
                .iter()
                .position(|e| &e.id == id)
                .ok_or("That object no longer exists.")?;
            Ok(Operation::AddEntity {
                entity: world.entities.remove(i),
                removed_walls: {
                    let prefix = format!("{id}:");
                    let mut removed: Vec<_> = world
                        .removed
                        .iter()
                        .filter(|w| w.starts_with(&prefix))
                        .cloned()
                        .collect();
                    removed.sort();
                    for wall in &removed {
                        world.removed.remove(wall);
                    }
                    removed
                },
            })
        }
        Operation::MoveEntity { id, position } => apply(
            world,
            &Operation::SetEntityTransform {
                id: id.clone(),
                position: Some(*position),
                yaw: None,
                scale: None,
            },
        ),
        Operation::SetEntityTransform {
            id,
            position,
            yaw,
            scale,
        } => {
            let entity = world
                .entities
                .iter_mut()
                .find(|e| &e.id == id)
                .ok_or("That object no longer exists.")?;
            let inverse = Operation::SetEntityTransform {
                id: id.clone(),
                position: Some(entity.position),
                yaw: Some(entity.yaw),
                scale: Some(entity.scale),
            };
            if let Some(p) = position {
                entity.position = *p;
            }
            if let Some(y) = yaw {
                entity.yaw = *y;
            }
            if let Some(s) = scale {
                entity.scale = *s;
            }
            Ok(inverse)
        }
        Operation::AddRoom { room, index } => {
            if world.rooms.iter().any(|r| r.id == room.id) {
                return Err("That room already exists.".into());
            }
            let mut room = room.clone();
            if room.floor_y.is_nan() {
                room.floor_y = world.ground;
            }
            let index = index.unwrap_or(world.rooms.len());
            if index > world.rooms.len() {
                return Err("That room order is invalid.".into());
            }
            world.rooms.insert(index, room.clone());
            Ok(Operation::RemoveRoom {
                id: room.id.clone(),
            })
        }
        Operation::RemoveRoom { id } => {
            if world
                .removed
                .iter()
                .any(|w| w.starts_with(&format!("{id}:")))
            {
                return Err("Restore that room's removed walls before removing the room.".into());
            }
            let i = world
                .rooms
                .iter()
                .position(|r| &r.id == id)
                .ok_or("That room no longer exists.")?;
            Ok(Operation::AddRoom {
                room: world.rooms.remove(i),
                index: Some(i),
            })
        }
        Operation::RemoveWall { wall_id } | Operation::RestoreWall { wall_id } => {
            if matches!(op, Operation::RemoveWall { .. }) {
                if !world.removed.contains(wall_id)
                    && !compile_world(world)?.walls.iter().any(|w| &w.id == wall_id)
                {
                    return Err("That side is already an opening or belongs to the adjacent room. Choose a standing wall.".into());
                }
                if !world.removed.insert(wall_id.clone()) {
                    return Err("That wall is already open.".into());
                }
                Ok(Operation::RestoreWall {
                    wall_id: wall_id.clone(),
                })
            } else {
                if !world.removed.remove(wall_id) {
                    return Err("That wall is already standing.".into());
                }
                Ok(Operation::RemoveWall {
                    wall_id: wall_id.clone(),
                })
            }
        }
        Operation::Undo | Operation::Redo => Err("Undo and redo cannot be nested.".into()),
    }
}

#[cfg(target_arch = "wasm32")]
mod wasm {
    use super::*;
    use std::cell::{Cell, RefCell};
    use std::collections::HashMap;
    thread_local! {
        static ENGINE: RefCell<Engine> = RefCell::new(Engine::default());
        static SLABS: RefCell<HashMap<u32,Box<[u8]>>> = RefCell::new(HashMap::new());
        static RESPONSE_LEN: Cell<u32> = const {Cell::new(0)};
    }
    fn store(bytes: Box<[u8]>) -> u32 {
        let ptr = bytes.as_ptr() as u32;
        SLABS.with(|slabs| slabs.borrow_mut().insert(ptr, bytes));
        ptr
    }
    #[unsafe(no_mangle)]
    pub extern "C" fn af_alloc(len: u32) -> u32 {
        if len == 0 || len as usize > MAX_MESSAGE {
            return 0;
        }
        let allowed = SLABS.with(|s| {
            let b = s.borrow();
            b.len() < 128
                && b.values().map(|x| x.len()).sum::<usize>() + len as usize <= 16 * 1024 * 1024
        });
        if !allowed {
            return 0;
        }
        store(vec![0; len as usize].into_boxed_slice())
    }
    #[unsafe(no_mangle)]
    pub extern "C" fn af_free(ptr: u32, len: u32) {
        SLABS.with(|s| {
            let mut slabs = s.borrow_mut();
            if slabs.get(&ptr).is_some_and(|b| b.len() == len as usize) {
                slabs.remove(&ptr);
            }
        });
    }
    #[unsafe(no_mangle)]
    pub extern "C" fn af_response_len() -> u32 {
        RESPONSE_LEN.with(Cell::get)
    }
    #[unsafe(no_mangle)]
    pub extern "C" fn af_request(ptr: u32, len: u32) -> u32 {
        RESPONSE_LEN.with(|v| v.set(0));
        let allowed = SLABS.with(|s| {
            let b = s.borrow();
            b.len() < 128
                && b.values().map(|x| x.len()).sum::<usize>() + MAX_MESSAGE <= 16 * 1024 * 1024
        });
        if !allowed {
            return 0;
        }
        let bytes = SLABS.with(|s| {
            s.borrow()
                .get(&ptr)
                .filter(|b| b.len() == len as usize)
                .map(|b| b.to_vec())
        });
        let Some(bytes) = bytes else {
            return 0;
        };
        let response = ENGINE.with(|e| e.borrow_mut().handle_json(&bytes));
        let bytes = serde_json::to_vec(&response).unwrap_or_else(|_| {
            b"{\"ok\":false,\"error\":{\"code\":\"serialization_failed\"}}".to_vec()
        });
        if bytes.len() > MAX_MESSAGE {
            return 0;
        }
        RESPONSE_LEN.with(|v| v.set(bytes.len() as u32));
        store(bytes.into_boxed_slice())
    }
}

#[cfg(test)]
mod prop_tests;
#[cfg(test)]
mod tests;
