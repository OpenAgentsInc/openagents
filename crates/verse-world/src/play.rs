//! Owned chamber authority and read-only cinematic presentation.
mod caster;
mod framed_movement;
mod multiplayer;
pub mod social;
use crate::rules::{Simulation, Snapshot, Spell};
use crate::utilities::{Controls, Utility};
use glam::Vec3;
use std::collections::BTreeMap;
use verse_engine::director::{Action, Frame, Scene};
use verse_engine::motion::State;

/// Checkpoint revision. v24 stores every controlled actor in the shared record.
pub const RULES_REVISION: &str = "verse-chamber-owned-v24";
/// Seed of the chamber's spell dice; scenarios may reseed before acting.
pub const SPELL_SEED: u64 = 0x5EED_0451;

/// Seconds the player keeps the bow drawn after a shot before stowing it.
pub const BOW_STANCE: f32 = 4.0;
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Ability {
    Bow,
    FireBolt,
    MagicMissile,
    Fireball,
    MistyStep,
    Thunderwave,
    Web,
    Grease,
    Light,
    Shield,
    /// A second-row spell from [`crate::spells::CATALOG`], by slot.
    Spell(u8),
    SpellCommand(crate::spells::command::Command),
}
impl Ability {
    /// The second action-bar row, hotkeys Shift+1 through Shift+0.
    pub const ROW_TWO: [Self; 10] = [
        Self::Spell(0),
        Self::Spell(1),
        Self::Spell(2),
        Self::Spell(3),
        Self::Spell(4),
        Self::Spell(5),
        Self::Spell(6),
        Self::Spell(7),
        Self::Spell(8),
        Self::Spell(9),
    ];
    /// The registered spell behind a second-row slot.
    pub fn catalog(self) -> Option<&'static crate::spells::SpellDef> {
        match self {
            Self::Spell(slot) => crate::spells::spell_in_slot(slot),
            Self::SpellCommand(command) => crate::spells::spell_in_slot(command.slot()),
            _ => None,
        }
    }
    pub const ALL: [Self; 10] = [
        Self::Bow,
        Self::FireBolt,
        Self::MagicMissile,
        Self::Fireball,
        Self::MistyStep,
        Self::Thunderwave,
        Self::Web,
        Self::Grease,
        Self::Light,
        Self::Shield,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Bow => "Shoot Bow",
            Self::FireBolt => "Fire Bolt",
            Self::MagicMissile => "Magic Missile",
            Self::Fireball => "Fireball",
            Self::MistyStep => "Misty Step",
            Self::Thunderwave => "Thunderwave",
            Self::Web => "Web",
            Self::Grease => "Grease",
            Self::Light => "Light",
            Self::Shield => "Shield",
            Self::Spell(_) | Self::SpellCommand(_) => {
                self.catalog().map_or("Empty slot", |s| s.label)
            }
        }
    }
    pub fn icon(self) -> &'static str {
        match self {
            Self::Bow => "bow-icon",
            Self::FireBolt => "fire-bolt-icon",
            Self::MagicMissile => "magic-missile-icon",
            Self::Fireball => "fireball-icon",
            Self::MistyStep => "misty-step-icon",
            Self::Thunderwave => "thunderwave-icon",
            Self::Web => "web-icon",
            Self::Grease => "grease-icon",
            Self::Light => "light-icon",
            Self::Shield => "shield-icon",
            Self::Spell(_) | Self::SpellCommand(_) => {
                self.catalog().map_or("spell-slot-empty", |s| s.icon)
            }
        }
    }
    pub fn utility(self) -> Option<Utility> {
        match self {
            Self::MistyStep => Some(Utility::MistyStep),
            Self::Thunderwave => Some(Utility::Thunderwave),
            Self::Web => Some(Utility::Web),
            Self::Grease => Some(Utility::Grease),
            Self::Light => Some(Utility::Light),
            Self::Shield => Some(Utility::Shield),
            _ => None,
        }
    }
    pub fn description(self) -> &'static str {
        match self {
            Self::MistyStep => "Blink forward 30 feet",
            Self::Thunderwave => "Close wave: damage and push",
            Self::Web => "Target area: restrain for 12 seconds",
            Self::Grease => "Target area: knock down for 10 seconds",
            Self::Light => "Place a light on the chamber floor",
            Self::Shield => "Absorb 18 damage for four seconds",
            Self::Spell(_) | Self::SpellCommand(_) => self.catalog().map_or("", |s| s.description),
            _ => "Attack the selected target",
        }
    }
    pub fn spell(self) -> Option<Spell> {
        match self {
            Self::Bow
            | Self::MistyStep
            | Self::Thunderwave
            | Self::Web
            | Self::Grease
            | Self::Light
            | Self::Shield
            | Self::Spell(_)
            | Self::SpellCommand(_) => None,
            Self::FireBolt => Some(Spell::Firebolt),
            Self::MagicMissile => Some(Spell::MagicMissile),
            Self::Fireball => Some(Spell::Fireball),
        }
    }
}
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Casting {
    pub target_life: verse_engine::core::LifeId,
    pub aim: Vec3,
    pub ability: Ability,
    pub started: f32,
    pub ends: f32,
    pub origin: Vec3,
    pub direction: Vec3,
}
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct DamageNumber {
    pub actor: u64,
    pub amount: i32,
    pub at: f32,
    pub position: Vec3,
    pub incoming: bool,
    pub serial: u64,
}
#[derive(Clone, serde::Serialize, serde::Deserialize)]
struct NavigationGoal {
    life: verse_engine::core::LifeId,
    target: Vec3,
    speed: f32,
    #[serde(default)]
    direct: bool,
}
#[derive(Clone, serde::Serialize, serde::Deserialize)]
struct Route {
    life: verse_engine::core::LifeId,
    target: Vec3,
    planned_at: f32,
    blocker_revision: u64,
    points: Vec<glam::DVec3>,
    cursor: usize,
    stuck_steps: u32,
    refusal: Option<String>,
}
fn primary_present() -> bool {
    true
}
fn is_primary_present(value: &bool) -> bool {
    *value
}
#[derive(Clone, serde::Serialize)]
pub struct Game {
    #[serde(flatten)]
    pub(crate) primary: multiplayer::Player,
    #[serde(
        default = "primary_present",
        skip_serializing_if = "is_primary_present"
    )]
    primary_resident: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    social: Option<social::State>,
    additional_players: BTreeMap<u64, multiplayer::Player>,
    next_player_actor: u64,
    #[serde(default)]
    migration_generation: u64,
    pub(crate) npc_characters: BTreeMap<u64, physics::character::Character>,
    routes: BTreeMap<u64, Route>,
    navigation_goals: BTreeMap<u64, NavigationGoal>,
    blockers: physics::walkable::Blockers,
    bodies: physics::lifetimes::Bodies,
    #[serde(skip)]
    pub motor_recovery: crate::movement::RecoveryObservations,
    #[serde(skip)]
    pub movement_expiry: crate::movement::frames::ExpiryObservations,
    #[serde(default)]
    navigation_scheduler: physics::walkable::Scheduler,
    #[serde(skip)]
    navigation_scratch: physics::walkable::SearchScratch,
    #[serde(skip)]
    navigation_crowd: physics::walkable::Crowd,
    #[serde(skip)]
    navigation_cover: BTreeMap<(u8, u64), (glam::DVec3, glam::DVec3)>,
    pub navigation_plans: u64,
    pub navigation_budget_refusals: u64,
    #[serde(skip)]
    navigation: Option<std::sync::Arc<physics::walkable::Navigation>>,
    pub physics_clock: physics::FixedStep,
    pub physics_steps: u64,
    clock_origin: Option<f64>,
    previous_npc: BTreeMap<u64, Vec3>,
    #[serde(skip)]
    pub(crate) query_scene: physics::queries::Scene,
    pub events: Vec<crate::events::Event>,
    event_serial: u64,
    emitted_cues: std::collections::BTreeSet<usize>,
    #[serde(skip)]
    pub(crate) colliders: Vec<physics::kinematic::Aabb>,
    pub scene: Scene,
    pub encounter: Option<super::combat::Encounter>,
    pub agent_controlled: bool,
    pub time: f32,
    pub authority_tick: u64,
    pub camera: super::controls::Camera,
    pub message: String,
    pub(crate) simulation: Simulation,
    /// Physics the spells act through: dynamic props, fields, dice, ledger.
    pub spells: crate::spells::SpellWorld,
    pub(crate) ids: BTreeMap<u64, u32>,
    pub(crate) lives: BTreeMap<u64, verse_engine::core::LifeId>,
    pub impacts: Vec<(Vec3, f32, u8)>,
    pub damage_numbers: Vec<DamageNumber>,
    damage_serial: u64,
    observed_health: BTreeMap<u32, i32>,
    npc_motion_clock: BTreeMap<u64, f32>,
    npc_motion: BTreeMap<u64, Vec3>,
    npc_yaw: BTreeMap<u64, f32>,
    npc_deaths: BTreeMap<u64, (f32, Vec3)>,
}
// Decode the primary record separately: Serde's flatten content reader rejects
// integer map keys in controls and cooldowns. The public JSON remains flat.
#[derive(serde::Deserialize)]
struct GameWire {
    pub(crate) primary: multiplayer::Player,
    #[serde(
        default = "primary_present",
        skip_serializing_if = "is_primary_present"
    )]
    primary_resident: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    social: Option<social::State>,
    additional_players: BTreeMap<u64, multiplayer::Player>,
    next_player_actor: u64,
    #[serde(default)]
    migration_generation: u64,
    pub(crate) npc_characters: BTreeMap<u64, physics::character::Character>,
    routes: BTreeMap<u64, Route>,
    navigation_goals: BTreeMap<u64, NavigationGoal>,
    blockers: physics::walkable::Blockers,
    bodies: physics::lifetimes::Bodies,
    #[serde(skip)]
    pub motor_recovery: crate::movement::RecoveryObservations,
    #[serde(skip)]
    pub movement_expiry: crate::movement::frames::ExpiryObservations,
    #[serde(default)]
    navigation_scheduler: physics::walkable::Scheduler,
    #[serde(skip)]
    navigation_scratch: physics::walkable::SearchScratch,
    #[serde(skip)]
    navigation_crowd: physics::walkable::Crowd,
    #[serde(skip)]
    navigation_cover: BTreeMap<(u8, u64), (glam::DVec3, glam::DVec3)>,
    pub navigation_plans: u64,
    pub navigation_budget_refusals: u64,
    #[serde(skip)]
    navigation: Option<std::sync::Arc<physics::walkable::Navigation>>,
    pub physics_clock: physics::FixedStep,
    pub physics_steps: u64,
    clock_origin: Option<f64>,
    previous_npc: BTreeMap<u64, Vec3>,
    #[serde(skip)]
    pub(crate) query_scene: physics::queries::Scene,
    pub events: Vec<crate::events::Event>,
    event_serial: u64,
    emitted_cues: std::collections::BTreeSet<usize>,
    #[serde(skip)]
    pub(crate) colliders: Vec<physics::kinematic::Aabb>,
    pub scene: Scene,
    pub encounter: Option<super::combat::Encounter>,
    pub agent_controlled: bool,
    pub time: f32,
    pub authority_tick: u64,
    pub camera: super::controls::Camera,
    pub message: String,
    pub(crate) simulation: Simulation,
    /// Physics the spells act through: dynamic props, fields, dice, ledger.
    pub spells: crate::spells::SpellWorld,
    pub(crate) ids: BTreeMap<u64, u32>,
    pub(crate) lives: BTreeMap<u64, verse_engine::core::LifeId>,
    pub impacts: Vec<(Vec3, f32, u8)>,
    pub damage_numbers: Vec<DamageNumber>,
    damage_serial: u64,
    observed_health: BTreeMap<u32, i32>,
    npc_motion_clock: BTreeMap<u64, f32>,
    npc_motion: BTreeMap<u64, Vec3>,
    npc_yaw: BTreeMap<u64, f32>,
    npc_deaths: BTreeMap<u64, (f32, Vec3)>,
}
impl<'de> serde::Deserialize<'de> for Game {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let mut value = serde_json::Value::deserialize(deserializer)?;
        let object = value
            .as_object_mut()
            .ok_or_else(|| serde::de::Error::custom("World checkpoint must be an object"))?;
        let mut primary = serde_json::Map::new();
        for key in [
            "admission",
            "source",
            "definition",
            "character",
            "player",
            "previous_player",
            "spawn",
            "yaw",
            "selected",
            "catalog_ready",
            "pending_movement",
            "held_movement",
            "pending_jump",
            "frame_clock",
            "controls",
            "casting",
            "bow_ready",
            "last_cast",
            "motion_clock",
            "locomotion",
            "moving",
            "died_at",
        ] {
            if let Some(value) = object.remove(key) {
                primary.insert(key.into(), value);
            }
        }
        object.insert("primary".into(), serde_json::Value::Object(primary));
        let wire: GameWire = serde_json::from_value(value).map_err(serde::de::Error::custom)?;
        Ok(Self {
            primary: wire.primary,
            primary_resident: wire.primary_resident,
            social: wire.social,
            additional_players: wire.additional_players,
            next_player_actor: wire.next_player_actor,
            migration_generation: wire.migration_generation,
            npc_characters: wire.npc_characters,
            routes: wire.routes,
            navigation_goals: wire.navigation_goals,
            blockers: wire.blockers,
            bodies: wire.bodies,
            motor_recovery: wire.motor_recovery,
            movement_expiry: wire.movement_expiry,
            navigation_scheduler: wire.navigation_scheduler,
            navigation_scratch: wire.navigation_scratch,
            navigation_crowd: wire.navigation_crowd,
            navigation_cover: wire.navigation_cover,
            navigation_plans: wire.navigation_plans,
            navigation_budget_refusals: wire.navigation_budget_refusals,
            navigation: wire.navigation,
            physics_clock: wire.physics_clock,
            physics_steps: wire.physics_steps,
            clock_origin: wire.clock_origin,
            previous_npc: wire.previous_npc,
            query_scene: wire.query_scene,
            events: wire.events,
            event_serial: wire.event_serial,
            emitted_cues: wire.emitted_cues,
            colliders: wire.colliders,
            scene: wire.scene,
            encounter: wire.encounter,
            agent_controlled: wire.agent_controlled,
            time: wire.time,
            authority_tick: wire.authority_tick,
            camera: wire.camera,
            message: wire.message,
            simulation: wire.simulation,
            spells: wire.spells,
            ids: wire.ids,
            lives: wire.lives,
            impacts: wire.impacts,
            damage_numbers: wire.damage_numbers,
            damage_serial: wire.damage_serial,
            observed_health: wire.observed_health,
            npc_motion_clock: wire.npc_motion_clock,
            npc_motion: wire.npc_motion,
            npc_yaw: wire.npc_yaw,
            npc_deaths: wire.npc_deaths,
        })
    }
}

pub use multiplayer::Player as ActorState;
impl std::ops::Deref for Game {
    type Target = ActorState;
    fn deref(&self) -> &Self::Target {
        &self.primary
    }
}
impl std::ops::DerefMut for Game {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.primary
    }
}

impl Game {
    /// Enables collision-query measurements without changing gameplay or checkpoint state.
    pub fn enable_query_profiling(&self) {
        self.query_scene.enable_profiling();
    }
    pub fn query_profile(&self) -> Option<physics::queries::QueryProfile> {
        self.query_scene.query_profile()
    }

    fn fence_world_generation(&mut self, previous: &Self) -> Result<(), String> {
        let generation = previous
            .lives
            .values()
            .map(|life| life.generation)
            .chain(
                previous
                    .spells
                    .props
                    .iter()
                    .map(|prop| prop.life.generation),
            )
            .chain([previous.migration_generation])
            .max()
            .unwrap_or(0)
            .checked_add(1)
            .ok_or("World generation exhausted")?;
        self.migration_generation = generation;
        self.spells.generation = generation;
        self.navigation_scheduler = Default::default();
        self.navigation_goals.clear();
        self.routes.clear();
        for prop in &mut self.spells.props {
            self.query_scene.remove(prop.query_key());
            prop.life.generation = generation;
        }
        self.spells.insert_query_colliders(&mut self.query_scene)?;
        Ok(())
    }
    pub(super) fn adopt_restart_fences(&mut self, previous: &Self) -> Result<(), String> {
        self.fence_world_generation(previous)?;
        self.primary.admission = previous.primary.admission.clone();
        self.primary
            .admission
            .respawn()
            .map_err(|e| format!("Restart refused: {e:?}"))?;
        self.control_handoff(self.agent_controlled)?;
        self.lives = previous
            .lives
            .iter()
            .map(|(actor, life)| Ok((*actor, life.next()?)))
            .collect::<Result<_, String>>()?;
        self.authority_tick = previous.authority_tick;
        self.event_serial = previous.event_serial;
        self.events = previous.events.clone();
        self.sync_bodies(0.)?;
        Ok(())
    }
    fn event(
        &mut self,
        actor: Option<verse_engine::core::LifeId>,
        kind: crate::events::Kind,
    ) -> Result<(), String> {
        self.event_serial = self
            .event_serial
            .checked_add(1)
            .ok_or("World event IDs exhausted")?;
        if self.events.len() == 512 {
            self.events.remove(0);
        }
        self.events.push(crate::events::Event {
            instance: self.primary.admission.actor().instance,
            serial: self.event_serial,
            tick: self.authority_tick,
            time: self.time,
            actor,
            kind,
        });
        Ok(())
    }
    fn timeline_events(&mut self) -> Result<(), String> {
        let due: Vec<_> = self
            .scene
            .cues
            .iter()
            .enumerate()
            .filter(|(index, cue)| cue.at <= self.time && !self.emitted_cues.contains(index))
            .map(|(index, cue)| (index, cue.clone()))
            .collect();
        for (index, cue) in due {
            let life = self.actor_life(cue.actor).or_else(|| {
                (cue.actor == self.primary.admission.actor().actor)
                    .then_some(self.primary.admission.actor())
            });
            match cue.action {
                Action::Yell { text, .. } => {
                    self.event(life, crate::events::Kind::Dialogue { text })?
                }
                Action::CameraCut => self.event(None, crate::events::Kind::CameraHandoff)?,
                Action::Bow { .. } => {}
            }
            self.emitted_cues.insert(index);
        }
        Ok(())
    }
    pub fn player_life(&self) -> verse_engine::core::LifeId {
        self.primary.admission.actor()
    }
    pub(super) fn player_motion_segments(&self) -> usize {
        self.primary.player_trajectory.len().saturating_sub(1)
    }
    pub(super) fn player_motion_at(&self, fraction: f32) -> Vec3 {
        if self.primary.player_trajectory.len() < 2 {
            return self.primary.player;
        }
        let segment = fraction.clamp(0., 1.) * (self.primary.player_trajectory.len() - 1) as f32;
        let index = (segment.floor() as usize).min(self.primary.player_trajectory.len() - 2);
        Vec3::from(self.primary.player_trajectory[index]).lerp(
            Vec3::from(self.primary.player_trajectory[index + 1]),
            segment - index as f32,
        )
    }
    pub(super) fn projectile_cover(
        &self,
        start: Vec3,
        delta: Vec3,
        radius: f64,
    ) -> Result<Option<f64>, String> {
        // Standing Wall of Stone panels are cover too.
        let mut cover = self.colliders.clone();
        cover.extend(
            crate::spells::wall_of_stone::cover(&self.spells)
                .into_iter()
                .map(|(_, bounds)| bounds),
        );
        Ok(physics::kinematic::sweep_box(
            start.as_dvec3(),
            glam::DVec3::splat(radius),
            delta.as_dvec3(),
            &cover,
        )?
        .map(|h| h.fraction))
    }
    pub fn actor_life(&self, actor: u64) -> Option<verse_engine::core::LifeId> {
        self.player_admission(actor)
            .map(|a| a.actor())
            .or_else(|| self.lives.get(&actor).copied())
    }
    /// Saves pending combat, controller fences, timers, and presentation clocks.
    pub fn checkpoint(&self) -> Result<Vec<u8>, String> {
        self.checkpoint_parts().map(|(_, bytes)| bytes)
    }
    pub(crate) fn checkpoint_parts(&self) -> Result<(serde_json::Value, Vec<u8>), String> {
        self.validate_social()?;
        self.validate_roles()?;
        self.simulation.validate()?;
        self.validate_players()?;
        self.validate_clock()?;
        self.bodies.validate()?;
        self.validate_body_bindings()?;
        if let Some(encounter) = &self.encounter {
            encounter.validate(self)?;
        }
        let value = serde_json::json!({
            "version": 1, "rules_revision": RULES_REVISION, "world": self,
        });
        let bytes = serde_json::to_vec(&value).map_err(|e| e.to_string())?;
        if bytes.len() > 2 * 1024 * 1024 {
            return Err("World checkpoint budget exceeded".into());
        }
        Ok((value, bytes))
    }
    /// Restores a versioned checkpoint and rebuilds static collision from its profile.
    pub fn restore(bytes: &[u8]) -> Result<Self, String> {
        #[derive(serde::Deserialize)]
        struct Saved {
            version: u32,
            rules_revision: String,
            world: Game,
        }
        if bytes.len() > 2 * 1024 * 1024 {
            return Err("World checkpoint budget exceeded".into());
        }
        let saved: Saved = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
        if saved.version != 1
            || (saved.rules_revision != RULES_REVISION
                && saved.rules_revision != "verse-chamber-owned-v23"
                && saved.rules_revision != "verse-chamber-owned-v22"
                && saved.rules_revision != "verse-chamber-owned-v21"
                && saved.rules_revision != "verse-chamber-owned-v20"
                && saved.rules_revision != "verse-chamber-owned-v19"
                && saved.rules_revision != "verse-chamber-owned-v18"
                && !(saved.rules_revision == "verse-chamber-owned-v16"
                    && saved.world.scene.actors.iter().all(|a| !a.friendly)))
        {
            return Err("Unsupported world checkpoint".into());
        }
        if saved.rules_revision != RULES_REVISION
            && saved.rules_revision != "verse-chamber-owned-v23"
            && saved.rules_revision != "verse-chamber-owned-v22"
            && saved.rules_revision != "verse-chamber-owned-v21"
            && saved.world.social.is_some()
        {
            return Err("Legacy rules cannot contain hosted social state".into());
        }
        if saved.rules_revision != RULES_REVISION
            && saved.rules_revision != "verse-chamber-owned-v23"
            && saved.rules_revision != "verse-chamber-owned-v22"
            && !saved.world.primary_resident
        {
            return Err("Legacy rules cannot contain an absent primary character".into());
        }
        let mut world = saved.world;
        world.spells.primary_caster = world.primary.admission.actor().actor;
        if saved.rules_revision != RULES_REVISION {
            world.primary.catalog_ready = std::mem::take(&mut world.spells.ready);
            world.primary.spawn = world
                .scene
                .actors
                .iter()
                .find(|actor| actor.id == world.primary.admission.actor().actor)
                .map_or(world.primary.player, |actor| actor.position);
        }
        world.validate_social()?;
        world.validate_clock()?;
        world.primary.held_movement.validate(world.physics_steps)?;
        world.bodies.validate()?;
        world.validate_body_bindings()?;
        if let Some(encounter) = &world.encounter {
            encounter.validate(&world)?;
        }
        world.validate_roles()?;
        world.simulation.validate()?;
        world.validate_players()?;
        world.primary.controls.validate()?;
        world.blockers.validate()?;
        if world.navigation_goals.iter().any(|(actor, goal)| {
            world.lives.get(actor) != Some(&goal.life)
                || !goal.target.is_finite()
                || !goal.speed.is_finite()
                || !(0.01..=6.4).contains(&goal.speed)
        }) {
            return Err("Invalid directed navigation checkpoint".into());
        }
        if world.blockers.instance != world.primary.admission.actor().instance
            || world.routes.iter().any(|(id, route)| {
                world.lives.get(id) != Some(&route.life)
                    || !route.target.is_finite()
                    || !route.planned_at.is_finite()
                    || route.planned_at > world.time
                    || route.blocker_revision > world.blockers.revision
                    || route.points.len() > 1024
                    || route.points.iter().any(|p| !p.is_finite())
                    || route.cursor > route.points.len()
                    || route.refusal.as_ref().is_some_and(|text| text.len() > 128)
            })
        {
            return Err("Invalid world navigation checkpoint".into());
        }
        world
            .navigation_scheduler
            .validate(world.primary.admission.actor().instance)?;
        if world
            .navigation_scheduler
            .tick()
            .is_some_and(|tick| tick > world.authority_tick)
        {
            return Err("Navigation scheduler belongs to a future authority tick".into());
        }
        if world.navigation_scheduler.pending_lives().any(|life| {
            world.lives.get(&life.entity).is_none_or(|current| {
                current.instance != life.instance || current.generation != life.generation
            })
        }) {
            return Err("Navigation scheduler contains a stale actor life".into());
        }
        world.primary.character.validate()?;
        for character in world.npc_characters.values() {
            character.validate()?;
        }
        if !world.time.is_finite()
            || world.time < 0.
            || !world.primary.player.is_finite()
            || !world.primary.yaw.is_finite()
            || world.ids.len() > 256
            || world.ids.len() != world.lives.len()
            || (world.social.is_none()
                && !world.ids.contains_key(&world.primary.selected)
                && !world
                    .spells
                    .props
                    .iter()
                    .any(|p| p.life.entity == world.primary.selected))
            || world
                .scene
                .actors
                .iter()
                .filter(|a| a.nameplate)
                .any(|a| !world.ids.contains_key(&a.id))
            || world.ids.keys().any(|id| {
                !world
                    .scene
                    .actors
                    .iter()
                    .any(|a| a.id == *id && a.nameplate)
            })
            || world.damage_numbers.len() > 64
            || world.events.len() > 512
            || world
                .events
                .windows(2)
                .any(|events| events[0].serial >= events[1].serial)
            || world
                .emitted_cues
                .iter()
                .any(|index| *index >= world.scene.cues.len())
            || world.events.iter().any(|e| {
                e.instance != world.primary.admission.actor().instance
                    || e.serial > world.event_serial
                    || e.tick > world.authority_tick
                    || !e.time.is_finite()
            })
            || world.ids.keys().any(|id| {
                world.lives.get(id).is_none_or(|life| {
                    life.actor != *id || life.instance != world.primary.admission.actor().instance
                })
            })
            || world
                .scene
                .actors
                .iter()
                .find(|a| a.model == "adventurer")
                .is_none_or(|a| a.id != world.primary.admission.actor().actor)
        {
            return Err("Invalid world checkpoint state".into());
        }
        world.colliders = world.static_bounds()?;
        world.query_scene = if world.colliders.is_empty() {
            Default::default()
        } else {
            world.static_queries()?
        };
        world
            .spells
            .validate(world.primary.admission.actor().instance)?;
        if !world.primary.character.feet.is_finite()
            || !world.primary.character.vertical_speed.is_finite()
            || world.npc_characters.iter().any(|(id, character)| {
                !world.lives.contains_key(id)
                    || !character.feet.is_finite()
                    || !character.vertical_speed.is_finite()
            })
            || world
                .previous_npc
                .iter()
                .any(|(id, pose)| !world.lives.contains_key(id) || !pose.is_finite())
            || !world.primary.previous_player.is_finite()
            || world.physics_clock.dt != 1. / 120.
            || world.physics_clock.max_steps != 12
            || !world.physics_clock.accumulator.is_finite()
            || !(0. ..world.physics_clock.dt).contains(&world.physics_clock.accumulator)
            || !world.physics_clock.dropped.is_finite()
            || world.physics_clock.dropped < 0.
        {
            return Err("Invalid world movement checkpoint".into());
        }
        world.navigation = crate::room::profile_navigation(
            world.scene.collision_profile.as_deref(),
            world.primary.admission.actor().instance,
        )?;
        if let Some(settings) = world
            .encounter
            .as_ref()
            .and_then(|e| e.authored_settings.clone())
        {
            if let Some(region) = &settings.navigation {
                world.configure_authored_navigation(region, &settings.blockers)?;
            }
        }
        world.navigation_cover = world.navigation_obstacles();
        for collider in world.blockers.colliders()? {
            world.colliders.push(physics::kinematic::Aabb {
                min: collider
                    .mesh
                    .triangles()
                    .iter()
                    .flat_map(|t| t.0)
                    .reduce(glam::DVec3::min)
                    .unwrap(),
                max: collider
                    .mesh
                    .triangles()
                    .iter()
                    .flat_map(|t| t.0)
                    .reduce(glam::DVec3::max)
                    .unwrap(),
            });
            world.query_scene.insert(collider)?;
        }
        world.simulation.set_spell_cover(
            world.colliders.clone(),
            crate::spells::wall_of_stone::cover(&world.spells),
        );
        world
            .spells
            .insert_query_colliders(&mut world.query_scene)?;
        world.sync_actor_colliders()?;
        Ok(world)
    }
    fn validate_clock(&self) -> Result<(), String> {
        if let Some(clock) = &self.primary.frame_clock {
            clock.validate(
                self.physics_steps,
                self.primary.admission.actor(),
                self.primary.admission.epoch(),
                self.primary.admission.accepted_sequence(),
            )?;
        }
        if self.spells.generation != self.migration_generation {
            return Err("World and prop generation namespace disagree".into());
        }
        let elapsed = self.physics_steps as f64 * self.physics_clock.dt;
        if self.snapshot().elapsed != elapsed as f32
            || self.clock_origin.is_none() && self.physics_steps != 0
            || self.clock_origin.is_some_and(|origin| {
                !origin.is_finite()
                    || origin.abs() > 1_000_000.
                    || (origin + elapsed) as f32 != self.time
            })
        {
            return Err("World and combat clocks disagree".into());
        }
        Ok(())
    }
    fn move_player(&self, position: Vec3, delta: Vec3) -> Result<Vec3, String> {
        if self.colliders.is_empty() {
            let mut p = position + delta;
            p.x = p.x.clamp(-12., 12.);
            p.z = p.z.clamp(-25., 12.);
            return Ok(p);
        }
        Ok(physics::character::slide(
            &self.query_scene,
            self.actor_filter(self.primary.admission.actor()),
            physics::character::Settings::default(),
            position.as_dvec3(),
            delta.as_dvec3(),
            true,
        )?
        .as_vec3())
    }

    /// Directs a living NPC through planning and collision, without placing it.
    pub fn direct_npc_navigation(
        &mut self,
        life: verse_engine::core::LifeId,
        target: Vec3,
        speed: f32,
    ) -> Result<(), String> {
        if self.actor_life(life.actor) != Some(life)
            || !target.is_finite()
            || target.abs().max_element() > 1_000_000.
            || !speed.is_finite()
            || !(0.01..=6.4).contains(&speed)
            || self.ids.get(&life.actor).is_none_or(|id| {
                !self
                    .snapshot()
                    .actors
                    .iter()
                    .any(|a| a.id == *id && a.alive)
            })
        {
            return Err("Invalid directed NPC navigation intent".into());
        }
        if let Some(encounter) = &mut self.encounter {
            encounter
                .casts
                .retain(|cast| cast.life != life || self.time >= cast.release);
        }
        self.routes.remove(&life.actor);
        self.navigation_goals.insert(
            life.actor,
            NavigationGoal {
                life,
                target,
                speed,
                direct: false,
            },
        );
        Ok(())
    }
    /// Walk through the swept controller over authored or spell-built surfaces.
    pub(crate) fn direct_npc_walk(
        &mut self,
        life: verse_engine::core::LifeId,
        target: Vec3,
        speed: f32,
    ) -> Result<(), String> {
        self.direct_npc_navigation(life, target, speed)?;
        self.navigation_goals.get_mut(&life.actor).unwrap().direct = true;
        Ok(())
    }
    pub fn clear_npc_navigation(&mut self, life: verse_engine::core::LifeId) -> bool {
        if !self
            .navigation_goals
            .get(&life.actor)
            .is_some_and(|goal| goal.life == life)
        {
            return false;
        }
        self.navigation_goals.remove(&life.actor);
        self.routes.remove(&life.actor);
        true
    }
    pub(super) fn navigation_directed(&self, actor: u64) -> bool {
        self.navigation_goals.contains_key(&actor)
    }
    fn validate_body_bindings(&self) -> Result<(), String> {
        if self.bodies.records().filter(|r| r.actor).count()
            != self.ids.len() + usize::from(self.primary_resident) + self.additional_players.len()
            || self.bodies.instance != self.primary.admission.actor().instance
        {
            return Err("Checkpoint physics body ownership disagrees".into());
        }
        if self
            .blockers
            .active_bounds()
            .any(|(life, _, _)| self.bodies.get(life).is_none())
        {
            return Err("Checkpoint blocker has no owned body".into());
        }
        let snapshot = self.snapshot();
        for r in self.bodies.records() {
            if !r.actor {
                if self.actor_life(r.life.entity).is_some() {
                    return Err("Checkpoint prop uses an actor ID".into());
                }
                let active = self
                    .blockers
                    .active_bounds()
                    .find(|(life, _, _)| *life == r.life);
                match r.phase {
                    physics::lifetimes::Phase::Alive => {
                        let physics::lifetimes::Hull::Box { half } = r.hull else {
                            return Err("Invalid prop collision hull".into());
                        };
                        if active.is_none_or(|(_, min, max)| {
                            (min - (r.body.pos - half)).abs().max_element() > 1e-8
                                || (max - (r.body.pos + half)).abs().max_element() > 1e-8
                        }) {
                            return Err("Checkpoint prop collision mask disagrees".into());
                        }
                    }
                    physics::lifetimes::Phase::Removed if active.is_none() => {}
                    _ => return Err("Invalid prop body phase".into()),
                }
                continue;
            }

            let (expected, source) = if r.life.entity == self.primary.admission.actor().actor {
                (Some(self.primary.admission.actor()), Some(0))
            } else {
                (
                    self.actor_life(r.life.entity),
                    self.player_source(r.life.entity)
                        .or_else(|| self.ids.get(&r.life.entity).copied()),
                )
            };
            let alive =
                source.is_some_and(|id| snapshot.actors.iter().any(|a| a.id == id && a.alive));
            if expected.is_none_or(|life| {
                life.instance != r.life.instance || life.generation != r.life.generation
            }) || r.damage_enabled() != alive
            {
                return Err("Checkpoint physics body life disagrees".into());
            }
            let active = self
                .blockers
                .active_bounds()
                .find(|(life, _, _)| *life == r.life);
            match r.phase {
                physics::lifetimes::Phase::Corpse { .. }
                    if self.scene.collision_profile.as_deref() == Some("original-chamber-v1") =>
                {
                    let physics::lifetimes::Hull::Box { half } = r.hull else {
                        return Err("Invalid corpse collision hull".into());
                    };
                    if active.is_none_or(|(_, min, max)| {
                        (min - (r.body.pos - half)).abs().max_element() > 1e-8
                            || (max - (r.body.pos + half)).abs().max_element() > 1e-8
                    }) {
                        return Err("Checkpoint corpse collision mask disagrees".into());
                    }
                }
                _ if active.is_some() => {
                    return Err("Checkpoint actor collision mask disagrees".into());
                }
                _ => {}
            }
        }
        Ok(())
    }
    pub fn physics_bodies(&self) -> &physics::lifetimes::Bodies {
        &self.bodies
    }
    pub(crate) fn actor_filter(
        &self,
        life: verse_engine::core::LifeId,
    ) -> physics::queries::Filter {
        let mut filter = physics::queries::Filter::blocking(life.instance);
        filter.ignore = Some(physics::queries::Life {
            instance: life.instance,
            entity: life.actor,
            generation: life.generation,
        });
        filter
    }
    fn sync_actor_colliders(&mut self) -> Result<(), String> {
        use physics::{
            lifetimes::{Hull, Phase},
            queries::{Capsule, CapsuleCollider, Pose, Usage},
        };
        let keys: Vec<_> = self.query_scene.capsule_keys().collect();
        for key in keys {
            self.query_scene.remove_capsule(key);
        }
        if self.colliders.is_empty() {
            return Ok(());
        }
        for record in self
            .bodies
            .records()
            .filter(|r| r.actor && r.phase == Phase::Alive)
        {
            let Hull::UprightCapsule { radius, height } = record.hull else {
                return Err("Living actor requires a capsule body".into());
            };
            self.query_scene.insert_capsule(CapsuleCollider {
                key: record.key(),
                capsule: Capsule {
                    a: glam::DVec3::Y * (radius - height * 0.5),
                    b: glam::DVec3::Y * (height * 0.5 - radius),
                    radius,
                },
                layers: 2,
                usage: Usage::Blocking,
            })?;
            self.query_scene.set_pose(
                record.key(),
                Pose {
                    position: record.body.pos,
                    rotation: record.body.orientation,
                },
            )?;
        }
        Ok(())
    }
    fn place_actor_body(
        &mut self,
        life: verse_engine::core::LifeId,
        feet: Vec3,
        dt: f64,
    ) -> Result<(), String> {
        let physical = physics::queries::Life {
            instance: life.instance,
            entity: life.actor,
            generation: life.generation,
        };
        let Some(record) = self.bodies.get(physical) else {
            return Ok(());
        };
        if record.phase != physics::lifetimes::Phase::Alive {
            return Ok(());
        }
        let key = record.key();
        let position = feet.as_dvec3() + glam::DVec3::Y * 0.9;
        self.bodies.place(physical, position, dt.min(0.1))?;
        if self.query_scene.pose(key).is_some() {
            self.query_scene.set_pose(
                key,
                physics::queries::Pose {
                    position,
                    rotation: glam::DQuat::IDENTITY,
                },
            )?;
        }
        Ok(())
    }
    fn sync_bodies(&mut self, dt: f32) -> Result<(), String> {
        use physics::lifetimes::{Hull, Phase};
        let snapshot = self.snapshot();
        let mut actors = if self.primary_resident {
            vec![(self.primary.admission.actor(), 0, self.primary.player)]
        } else {
            vec![]
        };
        actors.extend(
            self.additional_players
                .values()
                .map(|p| (p.admission.actor(), p.source, p.player)),
        );
        actors.extend(self.ids.iter().map(|(actor, source)| {
            let position = snapshot
                .actors
                .iter()
                .find(|a| a.id == *source)
                .map(|a| Vec3::from(a.pos))
                .or_else(|| self.npc_deaths.get(actor).map(|(_, p)| *p))
                .unwrap_or(Vec3::ZERO);
            (self.lives[actor], *source, position)
        }));
        let mut next = self.blockers.clone();
        let mut changed = false;
        for expired in self.bodies.expire(self.time as f64)? {
            changed |= next.remove(expired)?;
        }
        for (life, source, feet) in actors {
            let physical = physics::queries::Life {
                instance: life.instance,
                entity: life.actor,
                generation: life.generation,
            };
            let alive = snapshot.actors.iter().any(|a| a.id == source && a.alive);
            if self.bodies.get(physical).is_none() {
                if !alive {
                    continue;
                }
                self.bodies.spawn(
                    physical,
                    feet.as_dvec3() + glam::DVec3::Y * 0.9,
                    Hull::UprightCapsule {
                        radius: 0.35,
                        height: 1.8,
                    },
                )?;
            }
            let phase = self.bodies.get(physical).unwrap().phase;
            if alive && phase == Phase::Alive {
                self.bodies.place(
                    physical,
                    feet.as_dvec3() + glam::DVec3::Y * 0.9,
                    (dt as f64).min(0.1),
                )?;
            } else if !alive && phase == Phase::Alive {
                let died = self
                    .npc_deaths
                    .get(&life.actor)
                    .map_or(self.time, |(at, _)| *at);
                let min = feet.as_dvec3() + glam::DVec3::new(-0.35, 0., -0.8);
                let max = feet.as_dvec3() + glam::DVec3::new(0.35, 0.24, 0.8);
                self.bodies
                    .corpse(physical, min, max, (died + 60.) as f64)?;
                if self.navigation.is_some() {
                    next.upsert(physical, min, max)?;
                    changed = true;
                }
                self.routes.remove(&life.actor);
                self.navigation_goals.remove(&life.actor);
            }
        }
        if changed {
            self.replace_blockers(next)?;
        }
        self.sync_actor_colliders()?;
        Ok(())
    }
    /// Current tick's reserved plans, expansions, collision work, and queued actors.
    pub fn navigation_work(&self) -> (usize, usize, usize, usize) {
        let (plans, nodes, work) = self.navigation_scheduler.used();
        (plans, nodes, work, self.navigation_scheduler.pending())
    }
    /// Compiles an author preview over the authority's current collision scene.
    pub(crate) fn configure_authored_navigation(
        &mut self,
        region: &crate::content::NavigationRegion,
        blockers: &BTreeMap<u64, crate::content::Bounds>,
    ) -> Result<(), String> {
        region.validate()?;
        let instance = self.player_life().instance;
        let mut geometry = self.static_queries()?;
        for (id, bounds) in blockers {
            geometry.insert(physics::queries::MeshCollider {
                key: physics::queries::ColliderKey {
                    life: physics::queries::Life {
                        instance,
                        entity: 2_000_000 + *id,
                        generation: self.player_life().generation,
                    },
                    shape: 0,
                },
                layers: 1,
                usage: physics::queries::Usage::Blocking,
                mesh: physics::queries::Mesh::from_box(bounds.min.into(), bounds.max.into())?,
            })?;
        }
        let navigation = physics::walkable::Navigation::compile(
            &geometry,
            physics::walkable::Config {
                instance,
                layers: 1,
                min: region.min.into(),
                max: region.max.into(),
                cell: region.cell,
                character: physics::character::Settings::default(),
                work_budget: 1_000_000,
            },
        )?;
        self.navigation = Some(std::sync::Arc::new(navigation));
        Ok(())
    }
    pub fn navigation_preview(
        &self,
        min: glam::DVec3,
        max: glam::DVec3,
        cell: f64,
    ) -> Result<physics::walkable::Navigation, String> {
        physics::walkable::Navigation::compile(
            &self.query_scene,
            physics::walkable::Config {
                instance: self.player_life().instance,
                layers: 1,
                min,
                max,
                cell,
                character: physics::character::Settings::default(),
                work_budget: 1_000_000,
            },
        )
    }
    /// Returns owned diagnostic geometry without granting mutation authority.
    pub fn collision_geometry(&self) -> Result<physics::queries::SceneSnapshot, String> {
        self.query_scene.snapshot(self.player_life().instance)
    }
    pub fn navigation_blockers(&self) -> &physics::walkable::Blockers {
        &self.blockers
    }
    /// Changes a trusted world prop; player commands cannot call this mutation.
    pub fn set_navigation_blocker(
        &mut self,
        life: physics::queries::Life,
        min: glam::DVec3,
        max: glam::DVec3,
    ) -> Result<(), String> {
        if self.actor_life(life.entity).is_some() {
            return Err("World prop cannot replace an actor collision body".into());
        }
        let mut next = self.blockers.clone();
        next.upsert(life, min, max)?;
        let mut bodies = self.bodies.clone();
        bodies.upsert_prop(life, min, max)?;
        self.replace_blockers(next)?;
        self.bodies = bodies;
        Ok(())
    }
    pub fn remove_navigation_blocker(
        &mut self,
        life: physics::queries::Life,
    ) -> Result<bool, String> {
        if self.actor_life(life.entity).is_some() {
            return Err("World prop cannot remove an actor collision body".into());
        }
        let mut next = self.blockers.clone();
        if !next.remove(life)? {
            return Ok(false);
        }
        self.replace_blockers(next)?;
        self.bodies.remove(life);
        Ok(true)
    }
    fn replace_blockers(&mut self, next: physics::walkable::Blockers) -> Result<(), String> {
        if self.navigation.is_none() {
            return Err("World profile has no compiled navigation".into());
        }
        let mut scene = self.static_queries()?;
        let mut bounds = self.static_bounds()?;
        for collider in next.colliders()? {
            bounds.push(physics::kinematic::Aabb {
                min: collider
                    .mesh
                    .triangles()
                    .iter()
                    .flat_map(|t| t.0)
                    .reduce(glam::DVec3::min)
                    .unwrap(),
                max: collider
                    .mesh
                    .triangles()
                    .iter()
                    .flat_map(|t| t.0)
                    .reduce(glam::DVec3::max)
                    .unwrap(),
            });
            scene.insert(collider)?;
        }
        self.spells.insert_query_colliders(&mut scene)?;
        scene.continue_profiling(&self.query_scene);
        self.query_scene = scene;
        self.colliders = bounds;
        self.blockers = next;
        self.simulation.set_colliders(self.colliders.clone());
        self.sync_actor_colliders()?;
        self.refresh_navigation_obstacles()?;
        Ok(())
    }
    fn navigation_obstacles(&self) -> BTreeMap<(u8, u64), (glam::DVec3, glam::DVec3)> {
        self.blockers
            .active_bounds()
            .map(|(life, min, max)| ((0, life.entity), (min, max)))
            .chain(
                crate::spells::wall_of_stone::cover(&self.spells)
                    .into_iter()
                    .map(|(id, bounds)| ((1, id.0 as u64), (bounds.min, bounds.max))),
            )
            .collect()
    }
    fn refresh_navigation_obstacles(&mut self) -> Result<(), String> {
        let next = self.navigation_obstacles();
        let Some(navigation) = &self.navigation else {
            self.navigation_cover = next;
            return Ok(());
        };
        let mut changed = std::collections::BTreeSet::new();
        for (id, bounds) in self.navigation_cover.iter().chain(next.iter()) {
            if self.navigation_cover.get(id) != next.get(id) {
                changed.extend(navigation.affected_tiles(bounds.0, bounds.1)?);
            }
        }
        if !changed.is_empty() {
            let snapshot = self.snapshot();
            let positions: BTreeMap<_, _> = self
                .ids
                .iter()
                .filter_map(|(actor, id)| {
                    snapshot
                        .actors
                        .iter()
                        .find(|source| source.id == *id)
                        .map(|source| (*actor, Vec3::from(source.pos).as_dvec3()))
                })
                .collect();
            let mut invalid = vec![];
            for (actor, route) in &mut self.routes {
                let start = positions
                    .get(actor)
                    .copied()
                    .unwrap_or(route.target.as_dvec3());
                let mut points = route.points[route.cursor..].to_vec();
                points.push(route.target.as_dvec3());
                if navigation
                    .route_tiles(start, &points)?
                    .iter()
                    .any(|tile| changed.contains(tile))
                {
                    invalid.push(*actor);
                } else {
                    route.blocker_revision = self.blockers.revision;
                }
            }
            for actor in invalid {
                self.routes.remove(&actor);
            }
        }
        for route in self.routes.values_mut() {
            route.blocker_revision = self.blockers.revision;
        }
        self.navigation_cover = next;
        Ok(())
    }
    pub(super) fn move_hostile(
        &mut self,
        actor: u64,
        position: Vec3,
        target: Vec3,
        distance: f32,
    ) -> Result<Vec3, String> {
        if !position.is_finite()
            || !target.is_finite()
            || !distance.is_finite()
            || !(0. ..=1.).contains(&distance)
        {
            return Err("Invalid hostile movement request".into());
        }
        if self.colliders.is_empty() {
            let mut p = position + (target - position).normalize_or_zero() * distance;
            p.x = p.x.clamp(-11., 11.);
            p.z = p.z.clamp(-24., 10.);
            return Ok(p);
        }
        if self
            .navigation_goals
            .get(&actor)
            .is_some_and(|goal| goal.direct)
            || (self.navigation.is_none()
                && self.scene.collision_profile.as_deref() == Some(crate::playground::PROFILE))
        {
            let delta = Vec3::new(target.x - position.x, 0., target.z - position.z);
            return Ok(position + delta.normalize_or_zero() * delta.length().min(distance));
        }
        let life = self
            .actor_life(actor)
            .ok_or("Hostile movement life is missing")?;
        let instance = self.primary.admission.actor().instance;
        let ignore = Some(physics::queries::Life {
            instance,
            entity: actor,
            generation: life.generation,
        });
        self.refresh_navigation_obstacles()?;
        let replan = self.routes.get(&actor).is_none_or(|route| {
            route.life != life
                || route.blocker_revision != self.blockers.revision
                || self.time - route.planned_at >= 0.5
                    && (route.target.distance(target) > 0.5
                        || route.stuck_steps >= 8
                        || route.points.is_empty())
        });
        if replan {
            self.navigation_scheduler.begin_tick(self.authority_tick);
            let budget = physics::walkable::Budget {
                nodes: 16_384,
                ..Default::default()
            };
            if !self.navigation_scheduler.request(ignore.unwrap(), budget)? {
                return Ok(position);
            }
            self.navigation_plans = self
                .navigation_plans
                .checked_add(1)
                .ok_or("Navigation plan counter exhausted")?;
            let navigation_blockers = self.blockers.with_obstacles(
                crate::spells::wall_of_stone::cover(&self.spells)
                    .into_iter()
                    .map(|(_, bounds)| bounds),
            )?;
            let result = self
                .navigation
                .as_ref()
                .ok_or("Compiled navigation is missing")?
                .path_with_scratch(
                    &self.query_scene,
                    &navigation_blockers,
                    instance,
                    position.as_dvec3(),
                    target.as_dvec3(),
                    ignore,
                    budget,
                    &mut self.navigation_scratch,
                );
            let (points, refusal) = match result {
                Ok(Some(path)) => (path.points, None),
                Ok(None) => (vec![], Some("No walkable path".into())),
                Err(error)
                    if matches!(
                        error.as_str(),
                        "Navigation path work budget exceeded"
                            | "Navigation collision work budget exceeded"
                    ) =>
                {
                    self.navigation_budget_refusals = self
                        .navigation_budget_refusals
                        .checked_add(1)
                        .ok_or("Navigation refusal counter exhausted")?;
                    (vec![], Some(error))
                }
                Err(error) => return Err(error),
            };
            self.routes.insert(
                actor,
                Route {
                    life,
                    target,
                    planned_at: self.time,
                    blocker_revision: self.blockers.revision,
                    points,
                    cursor: 0,
                    stuck_steps: 0,
                    refusal,
                },
            );
        }
        let route = self.routes.get_mut(&actor).unwrap();
        while route.points.get(route.cursor).is_some_and(|p| {
            let delta = *p - position.as_dvec3();
            glam::DVec3::new(delta.x, 0., delta.z).length()
                < if route.cursor + 1 == route.points.len() {
                    0.001
                } else {
                    0.08
                }
                && delta.y.abs() < 0.12
        }) {
            route.cursor += 1;
        }
        let Some(waypoint) = route.points.get(route.cursor).copied() else {
            return Ok(position);
        };
        let delta = waypoint - position.as_dvec3();
        let horizontal = glam::DVec3::new(delta.x, 0., delta.z);
        let movement = horizontal.normalize_or_zero() * horizontal.length().min(distance as f64);
        let movement = self.navigation_crowd.steer(
            physics::walkable::CrowdAgent {
                life: ignore.unwrap(),
                feet: position.as_dvec3(),
                radius: 0.35,
            },
            movement,
        )?;
        let steps = (movement.length() / 0.05).ceil().max(1.) as usize;
        let mut character = physics::character::Character::new(position.as_dvec3());
        let mut filter = physics::queries::Filter::blocking(instance);
        filter.ignore = ignore;
        for _ in 0..steps {
            if !self.motor_recovery.observe(character.step_contained(
                &self.query_scene,
                filter,
                physics::character::Settings::default(),
                movement * (120. / steps as f64),
                false,
                1. / 120.,
            )?) {
                break;
            }
        }
        if character.feet.distance(position.as_dvec3()) < distance as f64 * 0.1 {
            route.stuck_steps = route.stuck_steps.saturating_add(1);
        } else {
            route.stuck_steps = 0;
        }
        Ok(character.feet.as_vec3())
    }
    pub(super) fn attack_clear(&self, start: Vec3, end: Vec3) -> bool {
        physics::kinematic::sweep_box(
            start.as_dvec3(),
            glam::DVec3::splat(0.12),
            (end - start).as_dvec3(),
            &self
                .colliders
                .iter()
                .copied()
                .chain(
                    crate::spells::wall_of_stone::cover(&self.spells)
                        .into_iter()
                        .map(|(_, b)| b),
                )
                .collect::<Vec<_>>(),
        )
        .is_ok_and(|hit| hit.is_none())
    }
    fn camera_eye(&self, anchor: Vec3, desired: Vec3) -> Vec3 {
        let delta = (desired - anchor).as_dvec3();
        match physics::kinematic::sweep_box(
            anchor.as_dvec3(),
            glam::DVec3::splat(0.15),
            delta,
            &self.colliders,
        ) {
            Ok(Some(hit)) => {
                anchor
                    + (delta * (hit.fraction - 1e-4 / delta.length().max(1e-12)).max(0.)).as_vec3()
            }
            Ok(None) => desired,
            Err(_) => anchor,
        }
    }
    /// Returns the owned rules profile's cooldown duration for action-button swipes.
    pub fn cooldown_duration(&self, spell: crate::rules::Spell) -> f32 {
        self.simulation.cooldown_duration(spell)
    }

    pub(crate) fn hostile_actor(&self, actor: u64) -> bool {
        self.scene
            .actors
            .iter()
            .any(|a| a.id == actor && a.nameplate && !a.friendly)
    }

    fn validate_roles(&self) -> Result<(), String> {
        if self
            .scene
            .actors
            .iter()
            .any(|a| a.friendly && (!a.nameplate || a.model == "adventurer"))
        {
            return Err("Invalid friendly scene actor".into());
        }
        let snapshot = self.simulation.snapshot();
        for actor in self.scene.actors.iter().filter(|a| a.nameplate) {
            let source = self
                .ids
                .get(&actor.id)
                .and_then(|id| snapshot.actors.iter().find(|s| s.id == *id));
            if match source {
                Some(source) => {
                    source.faction != if actor.friendly { "friendly" } else { "undead" }
                }
                None => actor.friendly || !self.npc_deaths.contains_key(&actor.id),
            } {
                return Err("Scene role disagrees with simulation faction".into());
            }
        }
        Ok(())
    }

    pub fn new(scene: Scene) -> Result<Self, String> {
        Self::new_in(scene, 0)
    }

    /// Creates a chamber in the instance selected by its trusted host.
    pub fn new_in(scene: Scene, instance: u64) -> Result<Self, String> {
        Self::new_owned(scene, instance, None)
    }
    fn new_owned(
        mut scene: Scene,
        instance: u64,
        social: Option<social::State>,
    ) -> Result<Self, String> {
        if scene
            .actors
            .iter()
            .any(|a| a.friendly && (!a.nameplate || a.model == "adventurer"))
        {
            return Err("Invalid friendly scene actor".into());
        }
        scene
            .cues
            .retain(|c| !matches!(c.action, Action::Bow { .. }));
        let player = scene
            .actors
            .iter()
            .find(|a| a.model == "adventurer")
            .ok_or("Missing adventurer")?
            .position;
        let actors: Vec<_> = scene.actors.iter().filter(|a| a.nameplate).collect();
        let targets: Vec<_> = actors
            .iter()
            .map(|a| (a.position.to_array(), a.health as i32))
            .collect();
        let (mut simulation, source_ids) = Simulation::chamber(player.to_array(), &targets)?;
        let ids: BTreeMap<_, _> = actors
            .iter()
            .zip(source_ids)
            .map(|(a, id)| (a.id, id))
            .collect();
        for actor in actors.iter().filter(|a| a.friendly) {
            simulation.mark_friendly(ids[&actor.id])?;
        }
        let selected = ids
            .keys()
            .find(|id| **id != 1 && actors.iter().any(|a| a.id == **id && !a.friendly))
            .or_else(|| {
                ids.keys()
                    .find(|id| actors.iter().any(|a| a.id == **id && !a.friendly))
            })
            .copied()
            .or_else(|| social.as_ref().map(|_| 0))
            .ok_or("Missing hostile actors")?;
        let observed_health = simulation
            .snapshot()
            .actors
            .iter()
            .map(|a| (a.id, a.hp.max(0)))
            .collect();
        let colliders = social.as_ref().map_or_else(
            || crate::room::profile_colliders(scene.collision_profile.as_deref()),
            |s| s.profile.bounds(),
        )?;
        simulation.set_colliders(colliders.clone());
        let query_scene = if colliders.is_empty() {
            Default::default()
        } else {
            social.as_ref().map_or_else(
                || crate::room::profile_query_scene(scene.collision_profile.as_deref(), instance),
                |s| s.profile.geometry_in(instance)?.compile(instance),
            )?
        };
        let mut spells = crate::spells::SpellWorld::new(&colliders, SPELL_SEED);
        spells.primary_caster = scene
            .actors
            .iter()
            .find(|a| a.model == "adventurer")
            .unwrap()
            .id;
        let next_player_actor = scene
            .actors
            .iter()
            .map(|a| a.id)
            .filter(|id| *id < 1_000_000)
            .max()
            .unwrap_or(0)
            .checked_add(1)
            .ok_or("Player actor IDs exhausted")?;
        let mut world = Self {
            primary: multiplayer::Player::new(
                crate::Admission::new(
                    verse_engine::core::LifeId {
                        instance,
                        actor: scene
                            .actors
                            .iter()
                            .find(|a| a.model == "adventurer")
                            .unwrap()
                            .id,
                        generation: 0,
                    },
                    crate::Controller(1),
                ),
                0,
                player,
            ),
            social,
            additional_players: BTreeMap::new(),
            next_player_actor,
            primary_resident: true,
            migration_generation: 0,
            npc_characters: BTreeMap::new(),
            routes: BTreeMap::new(),
            navigation_goals: BTreeMap::new(),
            blockers: physics::walkable::Blockers::new(instance),
            bodies: physics::lifetimes::Bodies::new(instance),
            motor_recovery: Default::default(),
            movement_expiry: Default::default(),
            navigation_scheduler: Default::default(),
            navigation_scratch: Default::default(),
            navigation_crowd: Default::default(),
            navigation_cover: Default::default(),
            navigation_plans: 0,
            navigation_budget_refusals: 0,
            navigation: crate::room::profile_navigation(
                scene.collision_profile.as_deref(),
                instance,
            )?,
            physics_clock: physics::FixedStep::new(1. / 120., 12),
            clock_origin: None,
            physics_steps: 0,
            previous_npc: BTreeMap::new(),
            query_scene,
            events: vec![],
            event_serial: 0,
            emitted_cues: Default::default(),

            colliders,
            scene,
            encounter: None,
            agent_controlled: false,
            time: 0.0,
            authority_tick: 0,
            camera: super::controls::Camera::default(),
            message: String::new(),
            simulation,
            spells,
            lives: ids
                .keys()
                .map(|id| {
                    (
                        *id,
                        verse_engine::core::LifeId {
                            instance,
                            actor: *id,
                            generation: 0,
                        },
                    )
                })
                .collect(),
            ids,
            impacts: vec![],
            damage_numbers: vec![],
            damage_serial: 0,
            observed_health,
            npc_motion_clock: BTreeMap::new(),
            npc_motion: BTreeMap::new(),
            npc_yaw: BTreeMap::new(),
            npc_deaths: BTreeMap::new(),
        };
        world.primary.selected = selected;
        world.sync_bodies(0.)?;
        if world.scene.collision_profile.as_deref() == Some("original-chamber-v1") {
            for (name, kind, position) in [
                (
                    "Loose ritual crate",
                    crate::spells::PropKind::Crate,
                    Vec3::new(-6., 0.3, -4.),
                ),
                (
                    "Loose ritual barrel",
                    crate::spells::PropKind::Barrel,
                    Vec3::new(6., 0.45, -4.),
                ),
            ] {
                world.spawn_prop(name, crate::spells::PropSpec::reference(kind), position, 0.)?;
            }
        }
        Ok(world)
    }
    pub fn unlocked(&self) -> bool {
        self.time >= self.scene.cut_at
    }
    pub fn snapshot(&self) -> Snapshot {
        let mut snapshot = self.simulation.snapshot();
        if !self.primary_resident {
            snapshot.actors.retain(|a| a.id != 0);
        }
        snapshot
    }
    pub fn frame(&self) -> Frame {
        let mut frame = self.scene.frame(self.time);
        if !self.primary_resident {
            frame.actors.retain(|a| a.actor.id != self.player_actor());
        }
        for actor in &mut frame.actors {
            actor.life = self.actor_life(actor.actor.id);
        }
        if !self.unlocked() {
            for actor in &mut frame.actors {
                if let Some(source) = self.player_source(actor.actor.id) {
                    if let Ok(snapshot) = self.simulation.snapshot_for(source) {
                        actor.actor.health = snapshot.player.max_hp as u32;
                        actor.health = snapshot.player.hp as u32;
                    }
                }
            }
            return frame;
        }
        // The cinematic director stops its clock at the authored endpoint.
        // Live authority presentation continues with combat and the owned HUD.
        frame.time = self.time;
        let snapshot = self.snapshot();
        for a in &mut frame.actors {
            if self.scene.collision_profile.as_deref() == Some(crate::playground::PROFILE)
                && matches!(a.animation, verse_engine::motion::Selection::Legacy(_))
            {
                a.animation = verse_engine::motion::State::Idle.into();
            }
            if self.time > self.scene.duration {
                a.animation_time = self.time + a.actor.id as f32 * 0.19;
            }
            if let Some(p) = self.additional_players.get(&a.actor.id) {
                let hp = snapshot
                    .actors
                    .iter()
                    .find(|a| a.id == p.source)
                    .map_or(0, |a| a.hp.max(0) as u32);
                a.actor.health = snapshot
                    .actors
                    .iter()
                    .find(|a| a.id == p.source)
                    .map_or(200, |a| a.max_hp as u32);
                p.frame(a, hp, self.time, !self.colliders.is_empty());
                continue;
            }
            if a.actor.id == self.player_actor() {
                a.health = snapshot.player.hp.max(0) as u32;
                a.actor.health = snapshot.player.max_hp as u32;
                a.animation_time = if self.primary.moving {
                    self.primary.motion_clock
                } else {
                    self.time
                };
                a.actor.position = self.primary.player;
                a.actor.yaw = self.primary.yaw;
                if snapshot.player.hp == 0 {
                    a.animation = State::Death.into();
                    a.animation_time = self
                        .encounter
                        .as_ref()
                        .and_then(|e| e.ended)
                        .map_or(0.8, |at| (self.time - at).min(2.0));
                    continue;
                }
                a.animation = if self.primary.moving {
                    if self.scene.collision_profile.is_some()
                        && self.primary.locomotion[0].abs() > self.primary.locomotion[1].abs()
                    {
                        if self.primary.locomotion[0] < 0. {
                            State::StrafeLeft
                        } else {
                            State::StrafeRight
                        }
                    } else if self.primary.locomotion[1] < 0.0 {
                        State::Backpedal
                    } else {
                        State::Run
                    }
                } else if self.primary.last_cast.is_some_and(|(ability, at)| {
                    ability == Ability::Bow && self.time - at < BOW_STANCE
                }) {
                    State::BowReady
                } else {
                    State::CombatReady
                }
                .into();
                if !self.colliders.is_empty() && self.primary.character.support.is_none() {
                    a.animation = State::Airborne.into();
                    a.animation_time = 0.2;
                }
                if let Some(cast) = &self.primary.casting {
                    a.animation = State::Cast.into();
                    a.animation_time = self.time - cast.started;
                }
                if let Some((ability, at)) = self.primary.last_cast {
                    if self.time - at < 1.0 {
                        a.animation = if ability == Ability::Bow {
                            State::BowRelease
                        } else {
                            State::SpellRelease
                        }
                        .into();
                        a.animation_time = self.time - at;
                    }
                }
            } else if let Some(id) = self.ids.get(&a.actor.id) {
                if let Some(source) = snapshot.actors.iter().find(|s| s.id == *id) {
                    a.health = source.hp.max(0) as u32;
                    a.actor.position = source.pos.into();
                    if let Some(e) = &self.encounter {
                        a.animation = if self
                            .npc_motion
                            .get(&a.actor.id)
                            .is_some_and(|v| v.length_squared() > 0.01)
                        {
                            State::Walk
                        } else if !a.actor.friendly
                            && a.actor.model.starts_with("cultist")
                            && e.ended.is_none()
                        {
                            if a.actor.id % 3 == 0 {
                                State::CombatReadyAlternate
                            } else {
                                State::CombatReady
                            }
                        } else {
                            State::Idle
                        }
                        .into();
                        a.animation_time = if a.animation == State::Walk.into() {
                            self.npc_motion_clock
                                .get(&a.actor.id)
                                .copied()
                                .unwrap_or(0.)
                        } else {
                            self.time + a.actor.id as f32 * 0.19
                        };
                        if let Some(cast) = e
                            .casts
                            .iter()
                            .find(|c| c.actor == a.actor.id && self.time < c.release)
                        {
                            a.animation = State::Cast.into();
                            a.animation_time = self.time - cast.started;
                        } else if e
                            .released
                            .get(&a.actor.id)
                            .is_some_and(|at| self.time - at < 0.7)
                        {
                            a.animation = State::SpellRelease.into();
                            a.animation_time = self.time - e.released[&a.actor.id];
                        }
                        let direction = self.primary.player - a.actor.position;
                        if !a.actor.friendly {
                            a.actor.yaw = self
                                .npc_yaw
                                .get(&a.actor.id)
                                .copied()
                                .unwrap_or_else(|| (-direction.x).atan2(-direction.z));
                        }
                    }
                    // In the great crypt, acolytes chant around the circle
                    // and those still waiting hold their authored pose.
                    if let Some(ritual) = self.encounter.as_ref().and_then(|e| e.ritual.as_ref()) {
                        if ritual.chanting(a.actor.id) {
                            a.animation = State::Cast.into();
                            a.animation_time = self.time + a.actor.id as f32 * 0.37;
                            let to = crate::great_crypt::CIRCLE - a.actor.position;
                            a.actor.yaw = (-to.x).atan2(-to.z);
                        } else if ritual.holds(a.actor.id) {
                            a.animation = State::Idle.into();
                            a.animation_time = self.time + a.actor.id as f32 * 0.19;
                            if let Some(authored) =
                                self.scene.actors.iter().find(|s| s.id == a.actor.id)
                            {
                                a.actor.yaw = authored.yaw;
                            }
                        }
                    }
                    if self.encounter.is_none() && self.navigation_directed(a.actor.id) {
                        a.animation = if self
                            .npc_motion
                            .get(&a.actor.id)
                            .is_some_and(|v| v.length_squared() > 0.01)
                        {
                            State::Walk
                        } else {
                            State::BowReady
                        }
                        .into();
                        a.animation_time = if a.animation == State::Walk.into() {
                            self.npc_motion_clock
                                .get(&a.actor.id)
                                .copied()
                                .unwrap_or(0.)
                        } else {
                            self.time
                        };
                        if let Some(yaw) = self.npc_yaw.get(&a.actor.id) {
                            a.actor.yaw = *yaw;
                        }
                    }
                    if !a.actor.friendly && self.hostile_held(a.actor.id) {
                        a.animation = if a.actor.model.starts_with("cultist")
                            && self.encounter.as_ref().is_some_and(|e| e.ended.is_none())
                        {
                            if a.actor.id % 3 == 0 {
                                State::CombatReadyAlternate
                            } else {
                                State::CombatReady
                            }
                        } else {
                            State::Idle
                        }
                        .into();
                        a.animation_time = self.time + a.actor.id as f32 * 0.19;
                    }
                    if !self.colliders.is_empty()
                        && self
                            .npc_characters
                            .get(&a.actor.id)
                            .is_some_and(|c| c.knocked() || c.airborne() && c.vertical_speed < -1.)
                    {
                        a.animation = State::Airborne.into();
                        a.animation_time = 0.2;
                    }
                    if !a.actor.friendly && self.shared_prone(a.actor.position) {
                        a.animation = State::Prone.into();
                        a.animation_time = 1.0;
                    }
                    if !source.alive {
                        a.animation = State::Death.into();
                        a.animation_time = self
                            .npc_deaths
                            .get(&a.actor.id)
                            .map_or(0.0, |(at, _)| self.time - at);
                    }
                } else {
                    a.health = 0;
                    a.actor.yaw = self
                        .npc_yaw
                        .get(&a.actor.id)
                        .copied()
                        .unwrap_or(a.actor.yaw);
                    a.animation = State::Death.into();
                    a.animation_time = self
                        .npc_deaths
                        .get(&a.actor.id)
                        .map_or(2.0, |(at, _)| self.time - at);
                    if let Some((_, position)) = self.npc_deaths.get(&a.actor.id) {
                        a.actor.position = *position;
                    } else if let Some(e) = &self.encounter {
                        if let Some(position) = e.positions.get(&a.actor.id) {
                            a.actor.position = *position;
                        }
                    }
                }
            }
        }
        for actor in &mut frame.actors {
            if actor.health == 0 {
                actor.actor.nameplate = false;
            }
        }
        let direction = self.camera.direction();
        let anchor = self.primary.player + Vec3::Y * if self.agent_controlled { 3.0 } else { 1.4 };
        let mut desired = anchor - direction * self.camera.distance;
        desired.y = desired.y.max(0.25);
        frame.eye = if self.colliders.is_empty() {
            desired
        } else {
            self.camera_eye(anchor, desired)
        };
        frame.target = frame.eye + direction * 20.0;
        if frame.eye.distance(anchor) < 0.2 {
            for actor in &mut frame.actors {
                if actor.actor.model == "adventurer" {
                    actor.visible = false;
                }
            }
        }
        frame.projectiles = self
            .snapshot()
            .projectiles
            .iter()
            .filter(|p| p.kind == crate::rules::ProjectileKind::Bow)
            .map(|p| verse_engine::director::Projectile {
                position: p.pos.into(),
                direction: Vec3::from(p.vel).normalize_or_zero(),
            })
            .collect();
        frame
    }
    pub fn tick(&mut self, dt: f32, movement: [f32; 2]) -> Result<(), String> {
        if !dt.is_finite() || !(0.0..=0.1).contains(&dt) || movement.iter().any(|x| !x.is_finite())
        {
            return Err("Invalid play input".into());
        }
        let actor_positions = self.snapshot().actors;
        self.previous_npc = self
            .ids
            .iter()
            .filter_map(|(id, sim_id)| {
                actor_positions
                    .iter()
                    .find(|actor| actor.id == *sim_id)
                    .map(|actor| (*id, Vec3::from(actor.pos)))
            })
            .collect();
        if dt > 0. {
            self.authority_tick = self
                .authority_tick
                .checked_add(1)
                .ok_or("Authority tick exhausted")?;
        }
        let requested = dt;
        let active = if self.unlocked() {
            dt
        } else {
            self.time += dt;
            self.timeline_events()?;
            if !self.unlocked() {
                return Ok(());
            }
            let active = (self.time - self.scene.cut_at).clamp(0., dt);
            self.time = self.scene.cut_at;
            active
        };
        let elapsed = self.physics_steps as f64 * self.physics_clock.dt;
        // Trusted fixture placement can rebase scene time without changing combat time.
        if self
            .clock_origin
            .is_none_or(|origin| (origin + elapsed) as f32 != self.time)
        {
            self.clock_origin = Some(self.time as f64 - elapsed);
        }
        let physics_steps = self.physics_clock.advance(active as f64);
        self.physics_steps = self
            .physics_steps
            .checked_add(physics_steps as u64)
            .ok_or("Physics step counter exhausted")?;
        let dt = (physics_steps as f64 * self.physics_clock.dt) as f32;
        self.time =
            (self.clock_origin.unwrap() + self.physics_steps as f64 * self.physics_clock.dt) as f32;
        self.timeline_events()?;
        let caster_positions: BTreeMap<_, _> = self
            .player_actors()
            .into_iter()
            .filter_map(|actor| self.actor_position(actor).map(|p| (actor, p.as_dvec3())))
            .collect();
        for effect in &mut self.spells.telekinesis {
            if let Some(position) = caster_positions.get(&effect.caster) {
                effect.caster_position = *position + glam::DVec3::Y * 0.9;
            }
        }
        for effect in &mut self.spells.levitations {
            if let Some(position) = caster_positions.get(&effect.caster) {
                effect.caster_position = *position;
            }
        }
        self.spells.creatures = crate::spells::feather_fall::candidates(self)
            .iter()
            .map(|c| crate::meteor_swarm::Creature {
                id: c.id,
                feet: c.position - glam::DVec3::Y * 0.9,
                radius: 0.35,
                height: 1.8,
                dexterity: 0,
            })
            .collect();
        crate::spells::gust::update(self)?;
        self.simulation.set_spell_wind_walls(
            self.spells
                .wind_walls
                .iter()
                .filter(|e| !e.wall.ended && f64::from(self.time) < e.wall.until)
                .map(|e| e.wall.wall.clone())
                .collect(),
        );
        if requested > 0. && physics_steps == 0 {
            return Ok(());
        }
        self.respawn_cultists()?;
        self.sync_bodies(0.)?;
        let dead = self.snapshot().player.hp == 0;
        if dead {
            self.primary.casting = None;
        }
        if !dead
            && (self.agent_controlled
                || (self.primary.pending_movement.is_none()
                    && self.primary.admission.controller() == crate::Controller(1)))
        {
            let movement = if self.agent_controlled {
                super::combat::drive(self, dt)?
            } else {
                movement
            };
            let command = self
                .admission
                .command(
                    self.authority_tick,
                    crate::Intent::Move {
                        axes: movement.map(|v| v.clamp(-1., 1.)),
                        yaw: self.primary.yaw,
                    },
                )
                .map_err(|e| format!("Movement refused: {e:?}"))?;
            self.submit(self.primary.admission.controller(), command)?;
        }
        self.expire_primary_frames(dead)?;
        let movement = if dead {
            self.primary.pending_movement = None;
            self.primary.held_movement = Default::default();
            [0.; 2]
        } else {
            let start = self.physics_steps - physics_steps as u64;
            if let Some(axes) = self.primary.pending_movement.take() {
                self.primary.held_movement.refresh(axes, start)?;
            }
            self.primary.held_movement.axes(start)
        };
        let walk = crate::movement::walk(movement, self.primary.yaw)?;
        let speed = walk.speed;
        // Black Tentacles' square is Difficult Terrain: half speed.
        let terrain = crate::spells::black_tentacles::speed_scale(
            &self.spells,
            self.player_actor(),
            self.primary.player.as_dvec3(),
        ) as f32;
        let delta = walk.direction * dt * speed * terrain;
        self.primary.moving = delta.length_squared() > 0.0;
        self.primary.locomotion = movement;
        if self.primary.moving && self.primary.casting.take().is_some() {
            self.message = "Cast interrupted by movement".into();
        }
        let previous_player = self.primary.player;
        self.primary.previous_player = self.primary.player;
        let jump = std::mem::take(&mut self.primary.pending_jump) && !dead;
        // Jumping while Telekinesis holds something lets go instead.
        let mut player_path = vec![];
        if self.colliders.is_empty() {
            self.primary.player = self.move_player(self.primary.player, delta)?;
            player_path.extend([previous_player.to_array(), self.primary.player.to_array()]);
        } else {
            if self.primary.character.feet.as_vec3() != self.primary.player {
                self.primary.character =
                    physics::character::Character::new(self.primary.player.as_dvec3());
            }
            let framed = self.primary.frame_clock.is_some();
            let frame_work = if dead {
                Vec::new()
            } else if let Some(clock) = &mut self.primary.frame_clock {
                crate::movement::frames::expand(&clock.take(self.physics_steps)?)
            } else {
                Vec::new()
            };
            let steps = if dead {
                0
            } else if framed {
                frame_work.len() as u32
            } else {
                physics_steps
            };
            if frame_work
                .iter()
                .any(|step| step.held.axes(step.at).iter().any(|v| *v != 0.))
            {
                self.primary.casting = None;
            }
            let velocity = if dt > 0. {
                (delta / dt).as_dvec3()
            } else {
                glam::DVec3::ZERO
            };
            player_path.push(self.primary.character.feet.as_vec3().to_array());
            let filter = self.actor_filter(self.primary.admission.actor());
            let mut fell = 0.;
            for step in 0..steps {
                let (velocity, jump_step) = if framed {
                    let input = frame_work[step as usize];
                    self.primary.held_movement = input.held;
                    self.primary.yaw = input.yaw;
                    self.primary.locomotion = input.held.axes(input.at);
                    (
                        crate::movement::walk(self.primary.locomotion, self.primary.yaw)?
                            .direction
                            .as_dvec3()
                            * f64::from(
                                crate::movement::walk(self.primary.locomotion, self.primary.yaw)?
                                    .speed
                                    * terrain,
                            ),
                        input.jump,
                    )
                } else {
                    (velocity, jump && step == 0)
                };
                let spell_time = if framed {
                    self.time as f64 - f64::from(dt) * f64::from(steps - step) / f64::from(steps)
                } else {
                    self.time as f64 - (steps - step) as f64 * self.physics_clock.dt
                };
                crate::spells::feather_fall::prepare(
                    &mut self.spells,
                    self.primary.admission.actor().actor,
                    &mut self.primary.character,
                    spell_time,
                    self.physics_clock.dt,
                );
                let velocity = crate::spells::levitate::prepare(
                    &mut self.spells,
                    self.primary.admission.actor().actor,
                    &mut self.primary.character,
                    velocity,
                    spell_time,
                    self.physics_clock.dt,
                );
                let velocity = crate::spells::telekinesis::prepare(
                    &self.spells,
                    self.primary.admission.actor().actor,
                    &mut self.primary.character,
                    velocity,
                );
                let velocity = crate::spells::proxies::prepare(
                    &self.spells,
                    self.primary.admission.actor().actor,
                    &mut self.primary.character,
                    velocity,
                );
                let velocity = crate::spells::gust::movement(
                    &self.spells,
                    self.primary.character.feet,
                    velocity,
                );
                let before_bounce = self.primary.character.external;
                if !self
                    .motor_recovery
                    .observe(self.primary.character.step_contained(
                        &self.query_scene,
                        filter,
                        physics::character::Settings::default(),
                        velocity,
                        jump_step,
                        self.physics_clock.dt,
                    )?)
                {
                    break;
                }
                crate::spells::levitate::bounce(
                    &self.spells,
                    self.primary.admission.actor().actor,
                    &mut self.primary.character,
                    before_bounce,
                );
                fell += self.primary.character.landed.unwrap_or(0.);
                player_path.push(self.primary.character.feet.as_vec3().to_array());
            }
            self.primary.player = self.primary.character.feet.as_vec3();
            if !dead {
                self.fall_damage(None, fell)?;
            }
        }
        self.place_actor_body(
            self.primary.admission.actor(),
            self.primary.player,
            dt as f64,
        )?;
        let travelled = self.primary.player.distance(previous_player);
        self.primary.motion_clock += travelled / speed;
        self.primary.moving = travelled > 0.00001;
        self.primary.player_trajectory = player_path.clone();
        if player_path.len() >= 2 {
            self.simulation.place_chamber_actor(
                0,
                self.primary.player.to_array(),
                self.primary.yaw,
            )?;
            self.simulation.record_motion_path(0, player_path)?;
        }
        self.step_additional(physics_steps as u32, dt)?;
        let source_actors = self.snapshot().actors;
        self.navigation_scheduler.begin_tick(self.authority_tick);
        self.navigation_scheduler.retain(|life| {
            self.lives.get(&life.entity).is_some_and(|current| {
                current.instance == life.instance && current.generation == life.generation
            })
        });
        self.navigation_crowd
            .rebuild(self.ids.iter().filter_map(|(actor, id)| {
                let source = source_actors.iter().find(|source| source.id == *id)?;
                let life = self.lives.get(actor)?;
                Some(physics::walkable::CrowdAgent {
                    life: physics::queries::Life {
                        instance: life.instance,
                        entity: life.actor,
                        generation: life.generation,
                    },
                    feet: Vec3::from(source.pos).as_dvec3(),
                    radius: 0.35,
                })
            }))?;
        let mut falls = vec![];
        for a in self.scene.frame(self.time).actors {
            if let Some(id) = self.ids.get(&a.actor.id).copied() {
                if !source_actors.iter().any(|a| a.id == id) {
                    continue;
                }
                let mut authored = self
                    .encounter
                    .as_ref()
                    .and_then(|e| e.positions.get(&a.actor.id))
                    .copied()
                    .unwrap_or(a.actor.position);
                let terrain = crate::spells::black_tentacles::speed_scale(
                    &self.spells,
                    a.actor.id,
                    Vec3::from(
                        source_actors
                            .iter()
                            .find(|actor| actor.id == id)
                            .unwrap()
                            .pos,
                    )
                    .as_dvec3(),
                ) as f32;
                let navigated = self.navigation_goals.contains_key(&a.actor.id);
                if let Some(goal) = self.navigation_goals.get(&a.actor.id) {
                    let (target, speed) = (goal.target, goal.speed);
                    let position = Vec3::from(
                        source_actors
                            .iter()
                            .find(|actor| actor.id == id)
                            .unwrap()
                            .pos,
                    );
                    authored =
                        self.move_hostile(a.actor.id, position, target, speed * dt * terrain)?;
                    if let Some(encounter) = &mut self.encounter {
                        encounter.positions.insert(a.actor.id, authored);
                    }
                }
                let mut desired = authored;
                if !a.actor.friendly {
                    desired = self.primary.controls.position(id, authored, self.time);
                    for p in self.additional_players.values_mut() {
                        desired = p.controls.position(id, desired, self.time);
                    }
                    // Restrained creatures stand still and the tentacles'
                    // square halves speed; the authored place follows.
                    if terrain < 1. && !navigated {
                        let previous = Vec3::from(
                            source_actors
                                .iter()
                                .find(|actor| actor.id == id)
                                .unwrap()
                                .pos,
                        );
                        let slowed = previous + (desired - previous) * terrain;
                        if slowed != desired {
                            self.primary.controls.displace(id, slowed - desired);
                            desired = slowed;
                        }
                    }
                }
                let mut npc_path = vec![];
                let life = self.lives[&a.actor.id];
                let filter = self.actor_filter(life);
                let position = if self.colliders.is_empty() {
                    desired
                } else {
                    let previous = Vec3::from(
                        source_actors
                            .iter()
                            .find(|actor| actor.id == id)
                            .unwrap()
                            .pos,
                    );
                    let character = self
                        .npc_characters
                        .entry(a.actor.id)
                        .or_insert_with(|| physics::character::Character::new(previous.as_dvec3()));
                    if character.feet.as_vec3() != previous {
                        *character = physics::character::Character::new(previous.as_dvec3());
                    }
                    let displacement = (desired - previous).as_dvec3();
                    let velocity = if dt > 0. {
                        displacement / dt as f64
                    } else {
                        glam::DVec3::ZERO
                    };
                    // A knocked character does not walk; its authored place
                    // follows wherever the shove leaves it.
                    // A creature held by Telekinesis is moved by the grip.
                    let knocked = character.knocked();
                    let velocity = if knocked {
                        glam::DVec3::ZERO
                    } else {
                        glam::DVec3::new(velocity.x, 0., velocity.z).clamp_length_max(99.)
                    };
                    npc_path.push(character.feet.as_vec3().to_array());
                    let mut fell = 0.;
                    for step in 0..physics_steps {
                        let spell_time = self.time as f64
                            - (physics_steps - step) as f64 * self.physics_clock.dt;
                        crate::spells::feather_fall::prepare(
                            &mut self.spells,
                            a.actor.id,
                            character,
                            spell_time,
                            self.physics_clock.dt,
                        );
                        let velocity = crate::spells::levitate::prepare(
                            &mut self.spells,
                            a.actor.id,
                            character,
                            velocity,
                            spell_time,
                            self.physics_clock.dt,
                        );
                        let velocity = crate::spells::telekinesis::prepare(
                            &self.spells,
                            a.actor.id,
                            character,
                            velocity,
                        );
                        let velocity = crate::spells::proxies::prepare(
                            &self.spells,
                            a.actor.id,
                            character,
                            velocity,
                        );
                        let velocity =
                            crate::spells::gust::movement(&self.spells, character.feet, velocity);
                        let before_bounce = character.external;
                        if !self.motor_recovery.observe(character.step_contained(
                            &self.query_scene,
                            filter,
                            physics::character::Settings::default(),
                            velocity,
                            false,
                            self.physics_clock.dt,
                        )?) {
                            break;
                        }
                        crate::spells::levitate::bounce(
                            &self.spells,
                            a.actor.id,
                            character,
                            before_bounce,
                        );
                        fell += character.landed.unwrap_or(0.);
                        npc_path.push(character.feet.as_vec3().to_array());
                    }
                    let feet = character.feet.as_vec3();
                    if (knocked || character.knocked()) && !navigated {
                        self.primary.controls.displace(id, feet - previous);
                    }
                    if fell > 0. {
                        falls.push((a.actor.id, fell));
                    }
                    feet
                };
                self.simulation
                    .place_chamber_actor(id, position.to_array(), a.actor.yaw)?;
                self.place_actor_body(life, position, dt as f64)?;
                if npc_path.len() >= 2 {
                    self.simulation.record_motion_path(id, npc_path)?;
                }
            }
        }
        for (actor, height) in falls {
            self.fall_damage(Some(actor), height)?;
        }
        if physics_steps > 0 {
            self.step_spells(physics_steps as usize)?;
        }
        if self
            .primary
            .casting
            .as_ref()
            .is_some_and(|c| self.time >= c.ends)
        {
            let cast = self.primary.casting.take().unwrap();
            if self.actor_life(cast.target_life.actor) != Some(cast.target_life)
                || self.ids.get(&cast.target_life.actor).is_none_or(|id| {
                    self.snapshot()
                        .actors
                        .iter()
                        .find(|a| a.id == *id)
                        .is_none_or(|a| !a.alive)
                })
            {
                self.message = "Cast target life ended".into();
            } else if self.attack_clear(cast.origin, cast.aim) {
                self.simulation.cast(
                    cast.ability.spell().unwrap(),
                    cast.origin.to_array(),
                    cast.direction.to_array(),
                )?;
                self.primary.last_cast = Some((cast.ability, self.time));
            } else {
                self.message = "Cast blocked by chamber geometry".into();
            }
        }
        self.simulation.tick_at(
            dt,
            (self.physics_steps as f64 * self.physics_clock.dt) as f32,
            self.primary.player.to_array(),
            self.primary.yaw,
        )?;
        for effect in self.snapshot().effects {
            self.impacts
                .push((effect.pos.into(), self.time, effect.kind));
        }
        if let Some(mut encounter) = self.encounter.take() {
            encounter.step(self, dt)?;
            self.encounter = Some(encounter);
        }
        let snapshot = self.snapshot();
        if dt > 0. {
            for (id, sim_id) in &self.ids {
                if let Some(source) = snapshot.actors.iter().find(|actor| actor.id == *sim_id) {
                    let position = Vec3::from(source.pos);
                    let previous = self.previous_npc.get(id).copied().unwrap_or(position);
                    let delta = if source.alive {
                        position - previous
                    } else {
                        Vec3::ZERO
                    };
                    *self.npc_motion_clock.entry(*id).or_default() += delta.length() / 2.4;
                    self.npc_motion.insert(*id, delta / dt);
                }
            }
        }
        if self.encounter.is_some() || !self.navigation_goals.is_empty() {
            for actor in &self.scene.actors {
                let Some(source) = self
                    .ids
                    .get(&actor.id)
                    .and_then(|id| snapshot.actors.iter().find(|s| s.id == *id))
                else {
                    continue;
                };
                if !source.alive || actor.friendly {
                    continue;
                }
                let motion = self
                    .npc_motion
                    .get(&actor.id)
                    .copied()
                    .unwrap_or(Vec3::ZERO);
                let direction = if motion.length_squared() > 0.01 {
                    motion
                } else {
                    self.living_players()
                        .into_iter()
                        .min_by(|a, b| {
                            a.1.distance_squared(Vec3::from(source.pos))
                                .total_cmp(&b.1.distance_squared(Vec3::from(source.pos)))
                                .then_with(|| a.0.actor.cmp(&b.0.actor))
                        })
                        .map_or(self.primary.player, |(_, p)| p)
                        - Vec3::from(source.pos)
                };
                let target = (-direction.x).atan2(-direction.z);
                let yaw = self.npc_yaw.entry(actor.id).or_insert(actor.yaw);
                let delta = (target - *yaw + std::f32::consts::PI)
                    .rem_euclid(std::f32::consts::TAU)
                    - std::f32::consts::PI;
                *yaw += delta.clamp(-5.0 * dt, 5.0 * dt);
            }
        }
        for (actor, source) in &self.ids {
            let state = snapshot.actors.iter().find(|s| s.id == *source);
            if state.is_none_or(|s| !s.alive) {
                let position = state.map(|s| Vec3::from(s.pos)).or_else(|| {
                    self.encounter
                        .as_ref()
                        .and_then(|e| e.positions.get(actor).copied())
                });
                if let Some(position) = position {
                    self.npc_deaths
                        .entry(*actor)
                        .or_insert((self.time, position));
                }
            }
        }
        let after_damage = self.snapshot();
        for (actor, source) in self.ids.clone() {
            let after = after_damage.actors.iter().find(|a| a.id == source);
            let hp = after.map_or(0, |a| a.hp.max(0));
            let previous = self.observed_health.insert(source, hp).unwrap_or(hp);
            let lost = previous - hp;
            if lost > 0 {
                let position = after
                    .map(|a| Vec3::from(a.pos))
                    .or_else(|| self.npc_deaths.get(&actor).map(|(_, p)| *p))
                    .unwrap_or_else(|| {
                        self.scene
                            .actors
                            .iter()
                            .find(|a| a.id == actor)
                            .unwrap()
                            .position
                    });
                self.damage_number(actor, lost, position, false)?;
            }
        }
        self.damage_numbers.retain(|n| self.time - n.at < 1.35);
        self.impacts.retain(|(_, at, _)| self.time - at < 0.6);
        self.sync_bodies(dt)?;
        self.record_movement_confirmations()?;
        Ok(())
    }
    fn respawn_cultists(&mut self) -> Result<(), String> {
        let due: Vec<_> = self
            .scene
            .actors
            .iter()
            .filter(|a| {
                !a.friendly
                    && a.model.starts_with("cultist")
                    && self
                        .npc_deaths
                        .get(&a.id)
                        .is_some_and(|(at, _)| self.time - at >= 60.0)
            })
            .cloned()
            .collect();
        for actor in due {
            let next_life = self.lives[&actor.id].next()?;
            let old = self.ids[&actor.id];
            let source = self
                .simulation
                .spawn_chamber_actor(actor.position.to_array(), actor.health as i32)?;
            self.ids.insert(actor.id, source);
            self.lives.insert(actor.id, next_life);
            self.event(Some(next_life), crate::events::Kind::Respawn)?;
            self.observed_health.remove(&old);
            self.observed_health.insert(source, actor.health as i32);
            self.primary.controls.forget_actor(old);
            for p in self.additional_players.values_mut() {
                p.controls.forget_actor(old);
            }
            self.npc_deaths.remove(&actor.id);
            self.npc_characters.remove(&actor.id);
            self.routes.remove(&actor.id);
            self.navigation_goals.remove(&actor.id);
            self.previous_npc.remove(&actor.id);
            self.npc_motion.remove(&actor.id);
            self.npc_motion_clock.remove(&actor.id);
            self.npc_yaw.remove(&actor.id);
            self.damage_numbers.retain(|n| n.actor != actor.id);
            if let Some(e) = &mut self.encounter {
                e.positions.insert(actor.id, actor.position);
                e.reset_actor(actor.id, self.time);
            }
        }
        Ok(())
    }
    fn damage_number(
        &mut self,
        actor: u64,
        amount: i32,
        position: Vec3,
        incoming: bool,
    ) -> Result<(), String> {
        if amount <= 0 {
            return Ok(());
        }
        let life = if incoming {
            Some(self.primary.admission.actor())
        } else {
            self.actor_life(actor)
        };
        self.event(life, crate::events::Kind::Damage { amount, incoming })?;
        let dead = if incoming {
            self.snapshot().player.hp == 0
        } else {
            self.ids.get(&actor).is_some_and(|id| {
                self.snapshot()
                    .actors
                    .iter()
                    .find(|a| a.id == *id)
                    .is_none_or(|a| !a.alive)
            })
        };
        if dead {
            self.event(life, crate::events::Kind::Death)?;
        }
        self.damage_serial += 1;
        if self.damage_numbers.len() >= 64 {
            self.damage_numbers.remove(0);
        }
        self.damage_numbers.push(DamageNumber {
            actor,
            amount,
            at: self.time,
            position,
            incoming,
            serial: self.damage_serial,
        });
        Ok(())
    }
    fn record_ability(&mut self, ability: Ability) {
        if let Some(e) = &mut self.encounter {
            *e.used.entry(ability.label().into()).or_default() += 1;
        }
    }
    /// Wounds a scene actor directly, as a test's stand-in for a hit.
    #[cfg(test)]
    pub(crate) fn wound(&mut self, actor: u64, damage: i32) -> Result<(), String> {
        let id = *self.ids.get(&actor).ok_or("Unknown scene actor")?;
        self.simulation.bow_impact(id, damage)
    }
    pub fn hostile_held(&self, id: u64) -> bool {
        self.ids.get(&id).is_some_and(|source| {
            self.primary.controls.held(*source)
                || self
                    .additional_players
                    .values()
                    .any(|p| p.controls.held(*source))
        })
    }
    pub fn hostile_hit(&mut self, damage: i32) -> Result<(i32, i32), String> {
        if self.snapshot().player.hp == 0 {
            return Ok((0, 0));
        }
        let absorbed = self.primary.controls.absorb(damage, self.time);
        let lost = (damage - absorbed).min(self.snapshot().player.hp);
        self.simulation.chamber_player_damage(lost)?;
        if lost > 0 {
            let actor = self
                .scene
                .actors
                .iter()
                .find(|a| a.model == "adventurer")
                .map_or(14, |a| a.id);
            self.damage_number(actor, lost, self.primary.player, true)?;
            self.impacts
                .push((self.primary.player + Vec3::Y, self.time, 3));
        }
        Ok((lost, absorbed))
    }
    pub fn cycle_target(&mut self) {
        let live: Vec<_> = self
            .frame()
            .actors
            .iter()
            .filter(|a| a.actor.nameplate && !a.actor.friendly && a.health > 0)
            .map(|a| a.actor.id)
            .collect();
        if !live.is_empty() {
            let index = live
                .iter()
                .position(|id| *id == self.primary.selected)
                .map_or(0, |i| (i + 1) % live.len());
            self.primary.selected = live[index];
        }
    }
    pub fn activate(&mut self, ability: Ability) -> Result<(), String> {
        let tick = self.authority_tick;
        let intent = crate::Intent::Cast {
            ability,
            target: self.lives.get(&self.primary.selected).copied(),
            aim: [-self.primary.yaw.sin(), 0., -self.primary.yaw.cos()],
        };
        let command = self
            .admission
            .command(tick, intent)
            .map_err(|e| format!("Command refused: {e:?}"))?;
        self.submit(self.primary.admission.controller(), command)
    }
    /// Turns the adventurer through an admitted movement command.
    pub fn face(&mut self, yaw: f32) -> Result<(), String> {
        let command = self
            .admission
            .command(
                self.authority_tick,
                crate::Intent::Move { axes: [0.; 2], yaw },
            )
            .map_err(|e| format!("Turn refused: {e:?}"))?;
        self.submit(self.primary.admission.controller(), command)
    }
    /// Scene actor ID of the adventurer.
    pub(crate) fn player_actors(&self) -> Vec<u64> {
        self.primary_resident
            .then_some(self.player_actor())
            .into_iter()
            .chain(self.additional_players.keys().copied())
            .collect()
    }
    pub fn player_actor(&self) -> u64 {
        self.primary.admission.actor().actor
    }
    /// Feet position of a living scene actor, the adventurer included.
    pub fn actor_position(&self, actor: u64) -> Option<Vec3> {
        if actor == self.player_actor() {
            return self.primary_resident.then_some(self.primary.player);
        }
        if let Some(p) = self.additional_players.get(&actor) {
            return Some(p.player);
        }
        let id = self.ids.get(&actor)?;
        self.snapshot()
            .actors
            .into_iter()
            .find(|a| a.id == *id)
            .map(|a| Vec3::from(a.pos))
    }
    /// The movement controller of a scene actor, the adventurer included.
    pub(crate) fn spell_velocity(&mut self, actor: u64, velocity: glam::DVec3) {
        if actor == self.player_actor() {
            self.primary.character.add_velocity(velocity);
        } else if let Some(player) = self.additional_players.get_mut(&actor) {
            player.character.add_velocity(velocity);
        } else if let Some(character) = self.npc_characters.get_mut(&actor) {
            character.add_velocity(velocity);
        }
    }
    pub(crate) fn stone_displacement(
        &mut self,
        actor: u64,
        feet: glam::DVec3,
    ) -> Result<(), String> {
        let life = self
            .actor_life(actor)
            .ok_or("Stone displacement target is missing")?;
        let filter = self.actor_filter(life);
        let controller = if actor == self.player_actor() {
            &mut self.primary.character
        } else if let Some(player) = self.additional_players.get_mut(&actor) {
            &mut player.character
        } else {
            self.npc_characters
                .get_mut(&actor)
                .ok_or("Stone displacement controller is missing")?
        };
        controller.teleport(
            &self.query_scene,
            filter,
            physics::character::Settings::default(),
            feet,
        )?;
        if actor == self.player_actor() {
            self.primary.player = feet.as_vec3();
        } else if let Some(player) = self.additional_players.get_mut(&actor) {
            player.player = feet.as_vec3();
            self.simulation.place_chamber_actor(
                player.source,
                player.player.to_array(),
                player.yaw,
            )?;
        } else if let Some(source) = self.ids.get(&actor).copied() {
            self.simulation
                .place_chamber_actor(source, feet.as_vec3().to_array(), 0.)?;
        }
        Ok(())
    }
    pub fn actor_character(&self, actor: u64) -> Option<&physics::character::Character> {
        if actor == self.player_actor() {
            Some(&self.primary.character)
        } else if let Some(p) = self.additional_players.get(&actor) {
            Some(&p.character)
        } else {
            self.npc_characters.get(&actor)
        }
    }
    /// Adds trusted world setup: a dynamic prop centered on `center`.
    /// Player commands cannot call this mutation.
    pub fn spawn_prop(
        &mut self,
        name: &str,
        spec: crate::spells::PropSpec,
        center: Vec3,
        yaw: f32,
    ) -> Result<usize, String> {
        if self.colliders.is_empty() {
            return Err("Dynamic props need a collision profile".into());
        }
        let index = self.spells.props.len() as u64;
        let life = physics::queries::Life {
            instance: self.primary.admission.actor().instance,
            entity: crate::spells::PROP_ENTITY_BASE + index,
            generation: self.migration_generation,
        };
        let index = self
            .spells
            .add_prop(life, name, spec, center.as_dvec3(), yaw as f64, None)?;
        let prop = self.spells.props[index].clone();
        let half = prop.spec.dimensions * 0.5;
        let offset = -prop.spec.center_of_mass;
        self.query_scene.insert(physics::queries::MeshCollider {
            key: prop.query_key(),
            layers: 1,
            usage: physics::queries::Usage::Blocking,
            mesh: physics::queries::Mesh::from_box(offset - half, offset + half)?,
        })?;
        self.spells.sync_query_poses(&mut self.query_scene)?;
        Ok(index)
    }
    /// Applies SRD falling damage for a landing `height` meters below the
    /// arc's peak: 1d6 Bludgeoning per 10 feet, at most 20d6.
    pub(crate) fn fall_damage(&mut self, actor: Option<u64>, height: f64) -> Result<(), String> {
        if actor.is_some_and(|id| self.scene.actors.iter().any(|a| a.id == id && a.friendly)) {
            return Ok(());
        }
        if height > 0. {
            let target = actor.unwrap_or(self.player_actor());
            let protected = self
                .spells
                .feather_falls
                .iter_mut()
                .any(|effect| effect.land(target as u32, self.time as f64).is_some());
            let gentle = self.spells.levitations.iter_mut().any(|effect| {
                if effect.target == crate::spells::Target::Actor(target)
                    && matches!(effect.state.phase, crate::levitate::Phase::Descending(_))
                {
                    effect.state.land();
                    true
                } else {
                    false
                }
            });
            if protected || gentle {
                self.spells.record(
                    self.time,
                    "Feather Fall",
                    format!("{} landed: 0 falling damage", self.actor_name(target)),
                    None,
                );
                return Ok(());
            }
        }
        let dice = crate::spells::fall_dice(height);
        if dice == 0 {
            return Ok(());
        }
        let damage = self.spells.dice.sum(dice, 6) as i32;
        let name = match actor {
            None => {
                let lost = damage.min(self.snapshot().player.hp);
                self.simulation.chamber_player_damage(lost)?;
                let id = self.player_actor();
                self.damage_number(id, lost, self.primary.player, true)?;
                "Adventurer".to_string()
            }
            Some(actor) => {
                let Some(id) = self.ids.get(&actor).copied() else {
                    return Ok(());
                };
                if !self.snapshot().actors.iter().any(|a| a.id == id && a.alive) {
                    return Ok(());
                }
                self.simulation.bow_impact(id, damage)?;
                self.actor_name(actor)
            }
        };
        self.spells.record(
            self.time,
            "Falling",
            format!(
                "{name} fell {:.0} ft: {dice}d6 = {damage} bludgeoning",
                height / crate::spells::FEET
            ),
            None,
        );
        Ok(())
    }
    pub fn actor_name(&self, actor: u64) -> String {
        self.scene
            .actors
            .iter()
            .find(|a| a.id == actor)
            .map_or_else(|| format!("Actor {actor}"), |a| a.name.clone())
    }
    pub(crate) fn actor_model(&self, actor: u64) -> &str {
        self.scene
            .actors
            .iter()
            .find(|a| a.id == actor)
            .map_or("", |a| a.model.as_str())
    }
    fn living_npcs(&self) -> Vec<u64> {
        let snapshot = self.snapshot();
        self.ids
            .iter()
            .filter(|(_, id)| {
                snapshot
                    .actors
                    .iter()
                    .any(|a| a.id == **id && a.alive && a.faction == "undead")
            })
            .map(|(actor, _)| *actor)
            .collect()
    }
    /// Steps the spell world on the chamber clock: field forces on
    /// characters, the rigid props, character contacts, and prop colliders.
    fn step_spells(&mut self, steps: usize) -> Result<(), String> {
        if self.colliders.is_empty() {
            return Ok(());
        }
        let dt = steps as f64 * self.physics_clock.dt;
        let living = self.living_npcs();
        if !self.spells.fields.is_empty() {
            let fields = crate::spells::Fields {
                gravity: glam::DVec3::ZERO,
                fields: &self.spells.fields,
            };
            let push = |c: &mut physics::character::Character| {
                let accel = fields.spell_accel(c.feet + glam::DVec3::Y * 0.9);
                if accel != glam::DVec3::ZERO {
                    c.add_velocity(accel * dt);
                }
            };
            if self
                .simulation
                .player_resources(0)
                .is_some_and(|p| p.hp > 0)
            {
                push(&mut self.primary.character);
            }
            for p in self.additional_players.values_mut() {
                if self
                    .simulation
                    .player_resources(p.source)
                    .is_some_and(|p| p.hp > 0)
                {
                    push(&mut p.character);
                }
            }
            for actor in &living {
                if let Some(c) = self.npc_characters.get_mut(actor) {
                    push(c);
                }
            }
        }
        for (body, kind) in self.simulation.take_spell_hits() {
            crate::spells::wall_of_stone::hit(&mut self.spells, body, kind, self.time as f64)?;
        }
        self.simulation.set_spell_gusts(
            self.spells
                .gusts
                .iter()
                .filter(|e| e.gust.active(self.time as f64))
                .map(|e| e.gust.clone())
                .collect(),
        );
        crate::spells::reverse_gravity::entries(self)?;
        self.spells.begin_tick();
        let controller_boundary = self.spells.boundary_snapshot();
        for proxy in &self.spells.proxies {
            if proxy.held && !proxy.ended {
                self.spells.world[proxy.body].kind = physics::BodyKind::Dynamic;
                self.spells.world.wake(proxy.body);
            } else if !proxy.held && !proxy.ended {
                if let Some(c) = self.actor_character(proxy.actor).copied() {
                    let body = &mut self.spells.world[proxy.body];
                    body.pos = c.feet + glam::DVec3::Y * 0.9;
                    body.vel = glam::DVec3::ZERO;
                    body.kind = physics::BodyKind::Kinematic;
                }
            }
        }
        self.spells
            .record_boundary(&controller_boundary, "creature:controller");
        self.spells.step(steps as u32, self.time)?;
        for (actor, damage, spell) in std::mem::take(&mut self.spells.damage) {
            if let Some(source) = self.player_source(actor) {
                self.simulation.player_damage_for(source, damage)?;
            } else if let Some(source) = self.ids.get(&actor).copied() {
                self.simulation.bow_impact(source, damage)?;
            }
            self.spells.record(
                self.time,
                &spell,
                format!("{}: {damage} damage", self.actor_name(actor)),
                None,
            );
        }
        let proxies: Vec<_> = self
            .spells
            .telekinesis
            .iter()
            .filter_map(|effect| {
                let crate::spells::Target::Actor(actor) = effect.target else {
                    return None;
                };
                let body = effect.proxy?;
                Some((
                    actor,
                    body,
                    self.spells.world[body].pos - glam::DVec3::Y * 0.9,
                    self.spells.world[body].vel,
                    effect.grip.grip.is_some(),
                ))
            })
            .chain(
                self.spells
                    .proxies
                    .iter()
                    .filter(|p| p.held || p.ended)
                    .map(|p| {
                        (
                            p.actor,
                            p.body,
                            self.spells.world[p.body].pos - glam::DVec3::Y * 0.9,
                            self.spells.world[p.body].vel,
                            !p.ended,
                        )
                    }),
            )
            .collect();
        for (actor, body, feet, velocity, held) in proxies {
            let life = self
                .actor_life(actor)
                .ok_or("Telekinesis creature life is missing")?;
            let filter = self.actor_filter(life);
            let c = if actor == self.primary.admission.actor().actor {
                &mut self.primary.character
            } else if let Some(player) = self.additional_players.get_mut(&actor) {
                &mut player.character
            } else {
                self.npc_characters
                    .get_mut(&actor)
                    .ok_or("Telekinesis controller is missing")?
            };
            let peak = c.peak.unwrap_or(c.feet.y).max(feet.y);
            c.gravity = Some(physics::character::GravityOverride {
                scale: 0.,
                terminal: 55.,
            });
            c.external = glam::DVec3::ZERO;
            c.vertical_speed = 0.;
            let desired = (feet - c.feet) / dt;
            for _ in 0..steps {
                c.vertical_speed = 0.;
                c.add_velocity(glam::DVec3::Y * desired.y);
                if !self.motor_recovery.observe(c.step_contained(
                    &self.query_scene,
                    filter,
                    physics::character::Settings::default(),
                    glam::DVec3::new(desired.x, 0., desired.z).clamp_length_max(99.),
                    false,
                    self.physics_clock.dt,
                )?) {
                    break;
                }
            }
            c.peak = Some(peak);
            if !held {
                c.gravity = None;
                c.add_velocity(velocity);
            }
            let position = c.feet.as_vec3();
            if actor == self.player_actor() {
                self.primary.player = position;
            } else if let Some(player) = self.additional_players.get_mut(&actor) {
                player.player = position;
                self.simulation.place_chamber_actor(
                    player.source,
                    position.to_array(),
                    player.yaw,
                )?;
            } else if let Some(id) = self.ids.get(&actor).copied() {
                self.simulation
                    .place_chamber_actor(id, position.to_array(), 0.)?;
            }
            if !held {
                let lost =
                    physics::Momentum::of(&self.spells.world[body], self.spells.ledger.origin);
                self.spells.ledger.add(
                    "telekinesis:proxy",
                    physics::Momentum {
                        linear: -lost.linear,
                        angular: -lost.angular,
                    },
                );
                self.spells.world.remove_body(body);
                for effect in &mut self.spells.telekinesis {
                    if effect.proxy == Some(body) {
                        effect.proxy = None;
                    }
                }
                self.spells.proxies.retain(|p| p.body != body);
            }
        }
        let masses: BTreeMap<u64, f64> = std::iter::once(self.player_actor())
            .chain(self.additional_players.keys().copied())
            .chain(living.iter().copied())
            .map(|a| {
                (
                    a,
                    crate::spells::model_size(self.actor_model(a)).creature_mass(),
                )
            })
            .collect();
        let player = self.player_actor();
        let mut movers = vec![];
        if self
            .simulation
            .player_resources(0)
            .is_some_and(|p| p.hp > 0)
        {
            movers.push(crate::spells::Mover {
                actor: player,
                mass: masses[&player],
                character: &mut self.primary.character,
            });
        }
        for (actor, p) in self.additional_players.iter_mut() {
            if self
                .simulation
                .player_resources(p.source)
                .is_some_and(|p| p.hp > 0)
            {
                movers.push(crate::spells::Mover {
                    actor: *actor,
                    mass: masses[actor],
                    character: &mut p.character,
                });
            }
        }
        for (actor, character) in self.npc_characters.iter_mut() {
            if living.contains(actor) {
                movers.push(crate::spells::Mover {
                    actor: *actor,
                    mass: masses[actor],
                    character,
                });
            }
        }
        self.spells.couple(&mut movers)?;
        couple_characters(&mut movers);
        self.prune_caster_dice();
        self.spells.insert_query_colliders(&mut self.query_scene)?;
        self.refresh_navigation_obstacles()?;
        self.simulation.set_spell_cover(
            self.colliders.clone(),
            crate::spells::wall_of_stone::cover(&self.spells),
        );
        if self.snapshot().player.hp == 0 {
            self.spells.end_concentration(player)?;
        }
        for (actor, p) in &self.additional_players {
            if self.simulation.snapshot_for(p.source)?.player.hp == 0 {
                self.spells.end_concentration(*actor)?;
            }
        }
        Ok(())
    }
    pub fn jump(&mut self) -> Result<(), String> {
        let command = self
            .admission
            .command(self.authority_tick, crate::Intent::Jump)
            .map_err(|e| format!("Jump refused: {e:?}"))?;
        self.submit(self.primary.admission.controller(), command)
    }
    /// Produces a read-only pose between the previous and current authority ticks.
    pub fn interpolated_frame(&self, alpha: f32) -> Result<Frame, String> {
        if !alpha.is_finite() || !(0. ..=1.).contains(&alpha) {
            return Err("Invalid presentation interpolation".into());
        }
        let mut frame = self.frame();
        if self.unlocked() {
            let pose = self
                .primary
                .previous_player
                .lerp(self.primary.player, alpha);
            let delta = pose - self.primary.player;
            for actor in &mut frame.actors {
                if actor.actor.id == self.player_actor() {
                    actor.actor.position = pose;
                } else if let Some(p) = self.additional_players.get(&actor.actor.id) {
                    actor.actor.position = p.previous_player.lerp(p.player, alpha);
                } else if actor.health > 0 {
                    if let Some(previous) = self.previous_npc.get(&actor.actor.id) {
                        actor.actor.position = previous.lerp(actor.actor.position, alpha);
                    }
                }
            }
            frame.eye += delta;
            frame.target += delta;
        }
        Ok(frame)
    }
    /// Revives the adventurer at an authored spawn without resetting the encounter.
    pub fn respawn_player(&mut self) -> Result<(), String> {
        if !self.primary_resident {
            return Err("Primary character is absent".into());
        }
        if self.snapshot().player.hp != 0 {
            return Err("The adventurer is still alive".into());
        }
        let authored = self
            .scene
            .actors
            .iter()
            .find(|a| a.model == "adventurer")
            .ok_or("Missing adventurer spawn")?;
        let spawn = authored.position;
        let yaw = authored.yaw;
        let mut character = physics::character::Character::new(spawn.as_dvec3());
        character.teleport(
            &self.query_scene,
            self.actor_filter(self.player_life()),
            physics::character::Settings::default(),
            spawn.as_dvec3(),
        )?;
        let old = self.player_life();
        let physical = physics::queries::Life {
            instance: old.instance,
            entity: old.actor,
            generation: old.generation,
        };
        let mut admission = self.primary.admission.clone();
        admission
            .respawn()
            .map_err(|e| format!("Respawn refused: {e:?}"))?;
        admission
            .handoff(crate::Controller(1))
            .map_err(|e| format!("Respawn handoff refused: {e:?}"))?;
        let mut blockers = self.blockers.clone();
        if blockers.remove(physical)? && self.navigation.is_some() {
            self.replace_blockers(blockers)?;
        }
        self.simulation.respawn_player(spawn.to_array(), yaw)?;
        self.clear_social_seat(old);
        self.bodies.remove(physical);
        self.primary.admission = admission;
        self.primary.frame_clock = None;
        self.agent_controlled = false;
        self.primary.player = spawn;
        self.primary.previous_player = spawn;
        self.primary.player_trajectory.clear();
        self.primary.character = character;
        self.primary.yaw = yaw;
        // Behind the revived character, looking where it faces.
        self.camera = super::controls::Camera {
            yaw,
            ..Default::default()
        };
        self.primary.pending_movement = None;
        self.primary.held_movement = Default::default();
        self.primary.pending_jump = false;
        self.primary.casting = None;
        self.primary.last_cast = None;
        self.primary.bow_ready = self.time;
        self.primary.controls = Default::default();
        self.impacts.clear();
        self.damage_numbers.retain(|n| n.actor != old.actor);
        self.observed_health.insert(0, self.snapshot().player.hp);
        self.primary.moving = false;
        self.primary.locomotion = [0.; 2];
        self.primary.motion_clock = 0.;
        if let Some(encounter) = &mut self.encounter {
            encounter.casts.retain(|c| c.target_life != old);
            encounter.ended = None;
            encounter.next_action = self.time;
        }
        self.sync_bodies(0.)?;
        self.message = "The adventurer returns".into();
        self.event(Some(self.player_life()), crate::events::Kind::Respawn)?;
        Ok(())
    }
    /// Fences queued commands when switching between human and agent control.
    pub fn control_handoff(&mut self, agent: bool) -> Result<(), String> {
        self.primary
            .admission
            .handoff(crate::Controller(if agent { 2 } else { 1 }))
            .map_err(|e| format!("Control handoff refused: {e:?}"))?;
        self.clear_social_seat(self.player_life());
        self.agent_controlled = agent;
        self.primary.frame_clock = None;
        self.primary.pending_movement = None;
        self.primary.held_movement = Default::default();
        self.primary.pending_jump = false;
        Ok(())
    }
    /// Applies a controller command through the same local admission boundary.
    pub fn submit(
        &mut self,
        controller: crate::Controller,
        command: crate::Command<Ability>,
    ) -> Result<(), String> {
        let life = command.actor;
        let before = matches!(command.intent, crate::Intent::Cast { .. }).then(|| self.clone());
        let fence = command.clone();
        let cast_label = match command.intent {
            crate::Intent::Cast { ability, .. } => Some(ability.label().to_string()),
            _ => None,
        };
        let result = self.submit_owned(controller, command).and_then(|()| {
            if let Some(label) = cast_label {
                self.event(Some(life), crate::events::Kind::Ability { label })?;
            }
            Ok(())
        });
        if result.is_ok() {
            self.clear_social_seat(life);
        } else if let Some(before) = before {
            let admitted = before
                .player_admission(life.actor)
                .zip(self.player_admission(life.actor))
                .is_some_and(|(previous, current)| {
                    current.actor() == life
                        && (current.epoch() > previous.epoch()
                            || current.epoch() == previous.epoch()
                                && current.accepted_sequence() > previous.accepted_sequence())
                });
            *self = before;
            if admitted {
                let tick = self.authority_tick;
                // Refusals roll back gameplay, while the admitted envelope stays
                // consumed so a transport cannot replay it as a new command.
                self.actor_state_mut(life)
                    .unwrap()
                    .admission
                    .admit(controller, &fence, tick)
                    .map_err(|e| format!("Refused command fence disagrees: {e:?}"))?;
            }
        }
        result
    }
    fn submit_owned(
        &mut self,
        controller: crate::Controller,
        command: crate::Command<Ability>,
    ) -> Result<(), String> {
        if self.social.is_some() && matches!(command.intent, crate::Intent::Cast { .. }) {
            return Err("Combat is unavailable in social worlds".into());
        }
        if command.actor.actor != self.player_actor() {
            return self.submit_additional(controller, command);
        }
        if !self.unlocked() || self.snapshot().player.hp == 0 {
            return Err("The adventurer cannot act in the current state".into());
        }
        if self.primary.frame_clock.is_some()
            && matches!(
                command.intent,
                crate::Intent::Move { .. } | crate::Intent::Jump
            )
        {
            return Err("This controller requires movement intervals".into());
        }
        if matches!(command.intent, crate::Intent::Jump) {
            self.primary
                .admission
                .admit(controller, &command, self.authority_tick)
                .map_err(|e| format!("Command refused: {e:?}"))?;
            self.primary.pending_jump = true;
            return Ok(());
        }
        if let crate::Intent::Move { axes, yaw } = command.intent {
            let tick = self.authority_tick;
            self.primary
                .admission
                .admit(controller, &command, tick)
                .map_err(|e| format!("Command refused: {e:?}"))?;
            self.primary.yaw = yaw;
            self.primary.pending_movement = Some(axes);
            return Ok(());
        }
        let crate::Intent::Cast {
            ability,
            target,
            aim,
        } = command.intent
        else {
            return Err("Invalid chamber command".into());
        };
        if aim[1].abs() > 0.001 {
            return Err("The chamber requires horizontal aim".into());
        }
        if target.is_some_and(|life| {
            self.lives.get(&life.actor) != Some(&life) || !self.hostile_actor(life.actor)
        }) {
            return Err("Target life is stale".into());
        }
        let tick = self.authority_tick;
        self.primary
            .admission
            .admit(controller, &command, tick)
            .map_err(|e| format!("Command refused: {e:?}"))?;
        if let Some(target) = target {
            self.primary.selected = target.actor;
        }
        self.primary.yaw = (-aim[0]).atan2(-aim[2]);
        self.activate_admitted(ability)
    }
    fn activate_admitted(&mut self, ability: Ability) -> Result<(), String> {
        if self.social.is_some() {
            return Err("Combat is unavailable in social worlds".into());
        }
        if !self.unlocked() {
            return Err("Wait for the cinematic camera handoff".into());
        }
        if self.snapshot().player.hp <= 0 {
            return Err("The adventurer is dead".into());
        }
        if matches!(ability, Ability::Spell(_) | Ability::SpellCommand(_)) {
            let context = self.caster_context(self.player_life())?;
            return self.activate_catalog(context, ability);
        }
        let mut state = self.primary.clone();
        let target = self.lives.get(&state.selected).copied();
        self.activate_player(&mut state, ability, target)?;
        self.primary = state;
        self.message = ability.label().into();
        Ok(())
    }
}

/// Knocked characters that reach another character exchange momentum
/// through one contact impulse using both reference masses.
fn couple_characters(movers: &mut [crate::spells::Mover<'_>]) {
    use crate::spells::{CHARACTER_RADIUS, CONTACT_GAP, CONTACT_RESTITUTION};
    for i in 0..movers.len() {
        for j in i + 1..movers.len() {
            let (left, right) = movers.split_at_mut(j);
            let (a, b) = (&mut left[i], &mut right[0]);
            if !a.character.knocked() && !b.character.knocked() {
                continue;
            }
            let mut d = b.character.feet - a.character.feet;
            d.y = 0.;
            let vertical = (b.character.feet.y - a.character.feet.y).abs();
            let distance = d.length();
            if distance > 2. * CHARACTER_RADIUS + CONTACT_GAP
                || distance < 1e-9
                || vertical > crate::spells::CHARACTER_HEIGHT
            {
                continue;
            }
            let normal = d / distance;
            let closing = (a.character.external - b.character.external).dot(normal);
            if closing <= 1e-4 {
                continue;
            }
            let j = (1. + CONTACT_RESTITUTION) * closing / (1. / a.mass + 1. / b.mass);
            a.character.external -= normal * (j / a.mass);
            b.character.external += normal * (j / b.mass);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn queued_ability_cannot_cross_control_handoff_or_spend_resources() {
        let mut g = Game::combat(game().scene, false).unwrap();
        g.time = g.scene.cut_at;
        let command = g
            .admission
            .command(
                g.authority_tick,
                crate::Intent::Cast {
                    ability: Ability::Shield,
                    target: None,
                    aim: [0., 0., -1.],
                },
            )
            .unwrap();
        let mana = g.snapshot().player.mana;
        assert!(g.submit(crate::Controller(99), command.clone()).is_err());
        assert_eq!(g.snapshot().player.mana, mana);
        g.control_handoff(true).unwrap();
        g.control_handoff(false).unwrap();
        assert!(g.submit(crate::Controller(1), command).is_err());
        assert_eq!(g.snapshot().player.mana, mana);
        assert!(g.encounter.as_ref().unwrap().used.is_empty());
        g.activate(Ability::Shield).unwrap();
        assert!(g.snapshot().player.mana < mana);
    }
    fn game() -> Game {
        Game::new(
            Scene::from_json(include_bytes!("../../../assets/verse/original/ritual.json")).unwrap(),
        )
        .unwrap()
    }
    #[test]
    fn cultists_respawn_sixty_seconds_after_each_death_with_fresh_state() {
        let mut g = Game::combat(game().scene, false).unwrap();
        g.time = 30.0;
        assert_eq!(g.encounter.as_ref().unwrap().boss_max, 300_000);
        assert_eq!(
            g.frame()
                .actors
                .iter()
                .find(|a| a.actor.model == "claude")
                .unwrap()
                .health,
            300_000
        );
        let cultist = g
            .scene
            .actors
            .iter()
            .find(|a| a.model.starts_with("cultist"))
            .unwrap()
            .clone();
        for _ in 0..2 {
            let old_life = g.lives[&cultist.id];
            let old = g.ids[&cultist.id];
            g.simulation.bow_impact(old, 1000).unwrap();
            g.tick(0.0, [0.0; 2]).unwrap();
            let died = g.npc_deaths[&cultist.id].0;
            assert!(
                !g.frame()
                    .actors
                    .iter()
                    .find(|a| a.actor.id == cultist.id)
                    .unwrap()
                    .actor
                    .nameplate
            );
            // Allow the combat store to remove the old corpse before respawning.
            for _ in 0..30 {
                g.tick(0.1, [0.0; 2]).unwrap();
            }
            g.time = died + 59.9;
            g.tick(0.0, [0.0; 2]).unwrap();
            assert_eq!(g.ids[&cultist.id], old);
            g.time = died + 60.0;
            g.tick(0.0, [0.0; 2]).unwrap();
            assert_ne!(g.ids[&cultist.id], old);
            assert_eq!(g.lives[&cultist.id], old_life.next().unwrap());
            g.tick(0.0, [0.0; 2]).unwrap();
            assert_eq!(
                g.snapshot()
                    .actors
                    .iter()
                    .find(|a| a.id == g.ids[&cultist.id])
                    .unwrap()
                    .hp,
                cultist.health as i32,
            );
            assert!(!g.npc_deaths.contains_key(&cultist.id));
            assert!(!g.observed_health.contains_key(&old));
            let frame = g.frame();
            let alive = frame
                .actors
                .iter()
                .find(|a| a.actor.id == cultist.id)
                .unwrap();
            assert_eq!(alive.health, 15);
            assert_eq!(alive.actor.position, cultist.position);
            assert!(alive.actor.nameplate && alive.animation != State::Death.into());
        }
    }
    #[test]
    fn manual_instant_spell_reports_damage_once() {
        let mut g = game();
        g.time = 30.0;
        let target = g
            .frame()
            .actors
            .into_iter()
            .find(|a| a.actor.model.starts_with("cultist"))
            .unwrap();
        g.primary.player = target.actor.position + Vec3::Z;
        g.primary.yaw = 0.0;
        g.activate(Ability::Thunderwave).unwrap();
        g.tick(0.01, [0.0; 2]).unwrap();
        let dealt = 100
            - g.frame()
                .actors
                .iter()
                .find(|a| a.actor.id == target.actor.id)
                .unwrap()
                .health as i32;
        assert!(dealt > 0);
        assert!(
            g.damage_numbers
                .iter()
                .any(|n| n.actor == target.actor.id && n.amount == dealt && !n.incoming)
        );
        let count = g.damage_numbers.len();
        g.tick(0.01, [0.0; 2]).unwrap();
        assert_eq!(g.damage_numbers.len(), count);
    }
    #[test]
    fn floating_incoming_damage_excludes_absorption_and_caps_lethal_hits() {
        let mut g = game();
        g.time = 30.0;
        g.primary.controls.shield = 18;
        g.primary.controls.shield_until = 34.0;
        assert_eq!(g.hostile_hit(45).unwrap(), (27, 18));
        assert_eq!(g.damage_numbers.len(), 1);
        assert_eq!(g.damage_numbers[0].amount, 27);
        assert!(g.damage_numbers[0].incoming);
        assert_eq!(g.hostile_hit(1000).unwrap(), (173, 0));
        assert_eq!(g.damage_numbers[1].amount, 173);
        assert_eq!(g.hostile_hit(45).unwrap(), (0, 0));
        assert_eq!(g.damage_numbers.len(), 2);
        for _ in 0..14 {
            g.tick(0.1, [0.0; 2]).unwrap();
        }
        assert!(g.damage_numbers.is_empty());
    }
    #[test]
    fn classic_movement_uses_backpedal_speed_and_never_boosts_diagonals() {
        let mut g = game();
        for _ in 0..201 {
            g.tick(0.1, [0.0; 2]).unwrap();
        }
        let before = g.primary.player;
        g.tick(0.1, [0.0, 1.0]).unwrap();
        assert!((g.primary.player.distance(before) - 6.4008 * 0.1).abs() < 0.001);
        let before = g.primary.player;
        g.tick(0.1, [0.0, -1.0]).unwrap();
        assert!((g.primary.player.distance(before) - 4.1148 * 0.1).abs() < 0.001);
        let before = g.primary.player;
        g.tick(0.1, [1.0, 1.0]).unwrap();
        assert!((g.primary.player.distance(before) - 6.4008 * 0.1).abs() < 0.001);
        let projection = g.frame().view_projection(1280.0 / 720.0);
        let before = g.primary.player;
        g.tick(0.1, [1.0, 0.0]).unwrap();
        assert!(
            (projection * (g.primary.player - before).extend(0.0)).x > 0.0,
            "Right strafe must move toward screen right"
        );
    }
    #[test]
    fn utility_spells_share_resources_and_change_presented_world_state() {
        let mut g = game();
        assert!(g.activate(Ability::Light).is_err());
        for _ in 0..201 {
            g.tick(0.1, [0.0; 2]).unwrap();
        }
        g.primary.selected = u64::MAX;
        g.activate(Ability::Light).unwrap();
        assert!(g.primary.controls.light.is_some());
        let before = g.primary.player;
        g.activate(Ability::MistyStep).unwrap();
        assert!(g.primary.player.distance(before) > 9.0);
        assert_eq!(g.snapshot().player.mana, 18);
        assert!(g.activate(Ability::MistyStep).is_err());
        g.primary.selected = 2;
        g.activate(Ability::Web).unwrap();
        g.tick(0.1, [0.0; 2]).unwrap();
        let rooted = g
            .frame()
            .actors
            .iter()
            .find(|a| a.actor.id == 2)
            .unwrap()
            .actor
            .position;
        for _ in 0..40 {
            g.tick(0.1, [0.0; 2]).unwrap();
        }
        assert_eq!(
            g.frame()
                .actors
                .iter()
                .find(|a| a.actor.id == 2)
                .unwrap()
                .actor
                .position,
            rooted
        );
        g.activate(Ability::Grease).unwrap();
        assert_eq!(
            g.frame()
                .actors
                .iter()
                .find(|a| a.actor.id == 2)
                .unwrap()
                .animation,
            State::Prone.into()
        );
        g.primary.player = rooted - Vec3::Z * 3.0;
        let hp = g
            .frame()
            .actors
            .iter()
            .find(|a| a.actor.id == 2)
            .unwrap()
            .health;
        g.spells.dice.force_save(2, 2).unwrap();
        g.activate(Ability::Thunderwave).unwrap();
        let after = g
            .frame()
            .actors
            .iter()
            .find(|a| a.actor.id == 2)
            .unwrap()
            .health;
        assert!((2..=16).contains(&(hp - after)), "{hp} {after}");
        assert_eq!(g.snapshot().player.hp, 200);
        for _ in 0..15 {
            g.tick(0.1, [0.0; 2]).unwrap();
        }
        assert!(
            g.frame()
                .actors
                .iter()
                .find(|a| a.actor.id == 2)
                .unwrap()
                .actor
                .position
                .distance(rooted)
                > 3.0
        );
    }
    #[test]
    fn handoff_enforces_input_gate_and_retains_spell_damage_and_mana() {
        let mut g = game();
        assert!(g.activate(Ability::Fireball).is_err());
        for _ in 0..201 {
            g.tick(0.1, [0.0, 0.0]).unwrap();
        }
        let before = g.snapshot().player.mana;
        g.activate(Ability::MagicMissile).unwrap();
        for _ in 0..21 {
            g.tick(0.05, [0.0, 0.0]).unwrap();
        }
        assert_eq!(g.snapshot().player.mana, before - 2);
        assert_eq!(g.snapshot().projectiles.len(), 3);
        assert!(g.activate(Ability::MagicMissile).is_err());
        for _ in 0..100 {
            g.tick(0.05, [0.0, 0.0]).unwrap();
        }
        assert!(g.snapshot().counters.hits > 0);
        assert!(
            g.frame()
                .actors
                .iter()
                .filter(|a| a.actor.nameplate)
                .any(|a| a.health < a.actor.health)
        );
        g.activate(Ability::Bow).unwrap();
        assert!(g.activate(Ability::Bow).is_err());
        assert_eq!(g.frame().projectiles.len(), 1);
    }
}

#[cfg(test)]
mod interruption_tests {
    use super::*;
    #[test]
    fn fireball_resolves_against_owned_hostiles() {
        let scene =
            Scene::from_json(include_bytes!("../../../assets/verse/original/ritual.json")).unwrap();
        let mut game = Game::new(scene).unwrap();
        for _ in 0..201 {
            game.tick(0.1, [0.0, 0.0]).unwrap();
        }
        game.activate(Ability::Fireball).unwrap();
        let mut saw_projectile = false;
        for _ in 0..80 {
            game.tick(0.05, [0.0, 0.0]).unwrap();
            saw_projectile |= !game.snapshot().projectiles.is_empty();
        }
        assert!(saw_projectile);
        assert!(
            game.frame()
                .actors
                .iter()
                .filter(|a| a.actor.nameplate)
                .any(|a| a.health < a.actor.health)
        );
    }
    #[test]
    fn movement_interrupts_before_spending_mana_and_bow_damage_applies_once() {
        let scene =
            Scene::from_json(include_bytes!("../../../assets/verse/original/ritual.json")).unwrap();
        let mut game = Game::new(scene).unwrap();
        for _ in 0..201 {
            game.tick(0.1, [0.0, 0.0]).unwrap();
        }
        game.activate(Ability::Fireball).unwrap();
        assert!(game.primary.casting.is_some());
        game.tick(0.05, [1.0, 0.0]).unwrap();
        assert!(game.primary.casting.is_none());
        assert_eq!(game.snapshot().player.mana, 20);
        let health = game
            .frame()
            .actors
            .iter()
            .find(|a| a.actor.id == game.primary.selected)
            .unwrap()
            .health;
        game.activate(Ability::Bow).unwrap();
        for _ in 0..30 {
            game.tick(0.05, [0.0, 0.0]).unwrap();
        }
        assert_eq!(
            game.frame()
                .actors
                .iter()
                .find(|a| a.actor.id == game.primary.selected)
                .unwrap()
                .health,
            health - 6
        );
        game.tick(0.05, [0.0, 0.0]).unwrap();
        assert_eq!(
            game.frame()
                .actors
                .iter()
                .find(|a| a.actor.id == game.primary.selected)
                .unwrap()
                .health,
            health - 6
        );
    }
}

#[cfg(test)]
mod original_collision_tests {
    use super::*;
    fn game() -> Game {
        let scene =
            Scene::from_json(include_bytes!("../../../assets/verse/original/ritual.json")).unwrap();
        let mut game = Game::new(scene).unwrap();
        game.time = 21.;
        game
    }
    #[test]
    fn original_player_stops_at_column_and_slides_along_its_face() {
        let mut g = game();
        g.primary.player = Vec3::new(13., 0., -13.);
        g.primary.yaw = -std::f32::consts::FRAC_PI_2;
        for _ in 0..20 {
            g.tick(0.1, [0., 1.]).unwrap();
        }
        assert!(g.primary.player.x < 13.951 && g.primary.player.x > 13.94);
        let stopped_clock = g.primary.motion_clock;
        g.tick(0.1, [0., 1.]).unwrap();
        assert!(!g.primary.moving);
        assert!((g.primary.motion_clock - stopped_clock).abs() < 0.00001);
        let slid = g
            .move_player(g.primary.player, Vec3::new(2., 0., 2.))
            .unwrap();
        assert!(slid.z > -11.01 && slid.x < 13.951);
    }
    #[test]
    fn owned_locomotion_selects_backward_and_strafe_states_from_actual_motion() {
        let mut g = game();
        g.primary.player = Vec3::new(0., 0., -16.);
        for (movement, clip) in [
            ([0., -1.], State::Backpedal),
            ([-1., 0.], State::StrafeLeft),
            ([1., 0.], State::StrafeRight),
            ([0., 1.], State::Run),
        ] {
            let previous = g.primary.motion_clock;
            g.tick(0.05, movement).unwrap();
            let player = g
                .frame()
                .actors
                .into_iter()
                .find(|a| a.actor.model == "adventurer")
                .unwrap();
            assert_eq!(player.animation, clip.into());
            assert!(g.primary.motion_clock > previous);
            assert_eq!(player.animation_time, g.primary.motion_clock);
        }
    }
    #[test]
    fn blink_uses_collision_admitted_destination_and_matching_effect() {
        let mut g = game();
        g.primary.player = Vec3::new(13., 0., -13.);
        g.primary.yaw = -std::f32::consts::FRAC_PI_2;
        g.activate(Ability::MistyStep).unwrap();
        assert!(g.primary.player.x < 13.951 && g.primary.player.x > 13.94);
        assert_eq!(
            g.primary.controls.areas.last().unwrap().position,
            g.primary.player
        );
        assert_eq!(g.snapshot().player.mana, 18);
    }
    #[test]
    fn scene_refuses_unknown_collision_profiles() {
        let mut scene = game().scene;
        scene.collision_profile = Some("unknown".into());
        assert!(Game::new(scene).is_err());
    }
    #[test]
    fn original_camera_shortens_at_walls_and_restores_requested_zoom_in_open_space() {
        let mut g = game();
        g.primary.player = Vec3::new(20.8, 0., -7.);
        g.camera.yaw = std::f32::consts::FRAC_PI_2;
        g.camera.pitch = 0.;
        g.camera.distance = 10.;
        let blocked = g.frame();
        assert!(blocked.eye.x < 21.351 && blocked.eye.x > 21.34);
        assert_eq!(g.camera.distance, 10.);
        g.primary.player = Vec3::new(0., 0., -7.);
        let clear = g.frame();
        assert!((clear.eye.x - 10.).abs() < 1e-4);
        assert_eq!(g.camera.distance, 10.);
        let floor = g.camera_eye(Vec3::Y * 1.4, Vec3::new(0., -3., -2.));
        assert!(floor.y >= 0.15 && floor.y < 0.151);
    }
    #[test]
    fn original_hostile_routes_around_a_real_chamber_column() {
        let mut g = game();
        let mut position = Vec3::new(13., 0., -13.);
        let target = Vec3::new(17., 0., -13.);
        let mut detoured = false;
        for _ in 0..150 {
            position = g.move_hostile(2, position, target, 0.09).unwrap();
            detoured |= (position.z + 13.).abs() > 1.;
            let dx = ((position.x - 15.).abs() - 0.7).max(0.);
            let dz = ((position.z + 13.).abs() - 0.7).max(0.);
            assert!(
                dx.hypot(dz) >= 0.35 - 1e-4,
                "Capsule crossed the column: {position:?}"
            );
        }
        assert!(detoured && position.distance(target) < 0.02);
    }
    #[test]
    fn obstructed_player_attacks_do_not_spend_resources_or_start_casts() {
        let mut g = game();
        g.primary.player = Vec3::new(13., 0., -13.);
        g.primary.selected = 2;
        g.simulation
            .place_chamber_actor(g.ids[&2], [17., 0., -13.], 0.)
            .unwrap();
        for ability in [
            Ability::Bow,
            Ability::FireBolt,
            Ability::MagicMissile,
            Ability::Fireball,
            Ability::Web,
            Ability::Grease,
        ] {
            assert!(g.activate(ability).is_err());
        }
        assert_eq!(g.snapshot().player.mana, 20);
        assert!(g.primary.casting.is_none());
        assert_eq!(g.primary.bow_ready, 0.);
        assert!(g.snapshot().projectiles.is_empty());
    }
    fn intervening_wall() -> physics::kinematic::Aabb {
        physics::kinematic::Aabb {
            min: glam::DVec3::new(-3., 0., -15.1),
            max: glam::DVec3::new(3., 8., -14.9),
        }
    }
    #[test]
    fn the_bow_is_drawn_after_a_shot_and_stowed_later() {
        use verse_engine::motion::State;
        let stance = |g: &Game| {
            g.frame()
                .actors
                .into_iter()
                .find(|a| a.actor.model == "adventurer")
                .unwrap()
                .animation
        };
        let mut g = game();
        g.tick(0.05, [0.0, 0.0]).unwrap();
        assert_eq!(stance(&g), State::CombatReady.into());
        g.activate(Ability::Bow).unwrap();
        while g.time < g.primary.last_cast.unwrap().1 + 1.5 {
            g.tick(0.05, [0.0, 0.0]).unwrap();
        }
        assert_eq!(stance(&g), State::BowReady.into());
        while g.time < g.primary.last_cast.unwrap().1 + BOW_STANCE + 0.1 {
            g.tick(0.05, [0.0, 0.0]).unwrap();
        }
        assert_eq!(stance(&g), State::CombatReady.into());
    }
    #[test]
    fn delayed_bow_and_spell_recheck_obstruction() {
        let mut g = game();
        g.activate(Ability::Bow).unwrap();
        let wall = intervening_wall();
        g.set_navigation_blocker(
            physics::queries::Life {
                instance: 0,
                entity: 9005,
                generation: 0,
            },
            wall.min,
            wall.max,
        )
        .unwrap();
        let hp = g
            .frame()
            .actors
            .into_iter()
            .find(|a| a.actor.id == g.primary.selected)
            .unwrap()
            .health;
        for _ in 0..12 {
            g.tick(0.1, [0.; 2]).unwrap();
        }
        assert_eq!(
            g.frame()
                .actors
                .into_iter()
                .find(|a| a.actor.id == g.primary.selected)
                .unwrap()
                .health,
            hp
        );
        assert!(g.snapshot().projectiles.is_empty());
        let mut g = game();
        g.activate(Ability::MagicMissile).unwrap();
        let wall = intervening_wall();
        g.set_navigation_blocker(
            physics::queries::Life {
                instance: 0,
                entity: 9005,
                generation: 0,
            },
            wall.min,
            wall.max,
        )
        .unwrap();
        for _ in 0..12 {
            g.tick(0.1, [0.; 2]).unwrap();
        }
        assert_eq!(g.snapshot().player.mana, 20);
        assert_eq!(g.snapshot().counters.casts, 0);
    }
    #[test]
    fn thunderwave_does_not_damage_a_cultist_behind_a_column() {
        let mut g = game();
        g.primary.player = Vec3::new(13., 0., -13.);
        g.primary.yaw = -std::f32::consts::FRAC_PI_2;
        g.primary.selected = 2;
        g.simulation
            .place_chamber_actor(g.ids[&2], [17., 0., -13.], 0.)
            .unwrap();
        g.activate(Ability::Thunderwave).unwrap();
        assert_eq!(
            g.frame()
                .actors
                .into_iter()
                .find(|a| a.actor.id == 2)
                .unwrap()
                .health,
            100
        );
        assert_eq!(g.snapshot().player.mana, 20 - Utility::Thunderwave.cost());
    }
}

#[cfg(test)]
mod checkpoint_tests {
    use super::*;
    #[test]
    fn mid_battle_checkpoint_replays_casts_projectiles_ai_and_defeat() {
        let scene =
            Scene::from_json(include_bytes!("../../../assets/verse/original/ritual.json")).unwrap();
        let mut game = Game::combat(scene, true).unwrap();
        while game.time < 31. {
            game.tick(1. / 30., [0.; 2]).unwrap();
        }
        let saved = game.checkpoint().unwrap();
        assert!(!game.encounter.as_ref().unwrap().casts.is_empty());
        assert!(
            game.events
                .iter()
                .any(|event| matches!(event.kind, crate::events::Kind::Dialogue { .. }))
        );
        let mut restored = Game::restore(&saved).unwrap();
        assert_eq!(game.checkpoint().unwrap(), restored.checkpoint().unwrap());
        for _ in 0..2400 {
            game.tick(1. / 30., [0.; 2]).unwrap();
            restored.tick(1. / 30., [0.; 2]).unwrap();
            assert_eq!(game.checkpoint().unwrap(), restored.checkpoint().unwrap());
        }
        assert_eq!(game.snapshot().player.hp, 0);
        assert!(Ability::ALL.iter().all(|ability| {
            game.encounter
                .as_ref()
                .unwrap()
                .used
                .contains_key(ability.label())
        }));
        assert!(game.lives.values().any(|life| life.generation > 0));
        assert!(
            game.events
                .windows(2)
                .all(|events| events[0].serial < events[1].serial)
        );
        assert!(
            game.events
                .iter()
                .any(|event| matches!(event.kind, crate::events::Kind::Respawn))
        );
        let incoming: i32 = game
            .events
            .iter()
            .filter_map(|event| match event.kind {
                crate::events::Kind::Damage {
                    amount,
                    incoming: true,
                } => Some(amount),
                _ => None,
            })
            .sum();
        assert_eq!(incoming, 200);
    }
    #[test]
    fn checkpoint_refuses_wrong_revision_and_invalid_health() {
        let scene =
            Scene::from_json(include_bytes!("../../../assets/verse/original/ritual.json")).unwrap();
        let game = Game::combat(scene, false).unwrap();
        let mut saved: serde_json::Value =
            serde_json::from_slice(&game.checkpoint().unwrap()).unwrap();
        saved["version"] = 9.into();
        assert!(Game::restore(&serde_json::to_vec(&saved).unwrap()).is_err());
        saved["version"] = 1.into();
        saved["world"]["simulation"]["players"]["0"]["resources"]["hp"] = (-5).into();
        assert!(Game::restore(&serde_json::to_vec(&saved).unwrap()).is_err());
    }
    #[test]
    fn admitted_remote_movement_is_solved_and_handoff_clears_it() {
        let scene =
            Scene::from_json(include_bytes!("../../../assets/verse/original/ritual.json")).unwrap();
        let mut game = Game::combat(scene, false).unwrap();
        game.time = game.scene.cut_at;
        let command = game
            .admission
            .command(
                game.authority_tick,
                crate::Intent::Move {
                    axes: [0., 1.],
                    yaw: 0.,
                },
            )
            .unwrap();
        game.submit(crate::Controller(1), command).unwrap();
        let before = game.primary.player;
        game.tick(1. / 30., [0.; 2]).unwrap();
        assert!(game.primary.player.distance(before) > 0.2);
        let command = game
            .admission
            .command(
                game.authority_tick,
                crate::Intent::Move {
                    axes: [0., 1.],
                    yaw: 0.,
                },
            )
            .unwrap();
        game.submit(crate::Controller(1), command).unwrap();
        game.control_handoff(false).unwrap();
        let before = game.primary.player;
        game.tick(1. / 30., [0.; 2]).unwrap();
        assert_eq!(game.primary.player, before);
    }
    #[test]
    fn restart_fences_commands_from_the_previous_encounter() {
        let scene =
            Scene::from_json(include_bytes!("../../../assets/verse/original/ritual.json")).unwrap();
        let mut game = Game::combat(scene, false).unwrap();
        game.time = game.scene.cut_at;
        let old = game
            .admission
            .command(
                game.authority_tick,
                crate::Intent::Move {
                    axes: [0., 1.],
                    yaw: 0.,
                },
            )
            .unwrap();
        let old_life = game.actor_life(2).unwrap();
        game.restart_combat(false).unwrap();
        assert!(game.submit(crate::Controller(1), old).is_err());
        assert_eq!(game.actor_life(2), Some(old_life.next().unwrap()));
        game.activate(Ability::Shield).unwrap();
    }
}

#[cfg(test)]
mod grounded_movement_tests {
    use super::*;
    fn game() -> Game {
        let scene =
            Scene::from_json(include_bytes!("../../../assets/verse/original/ritual.json")).unwrap();
        let mut game = Game::new(scene).unwrap();
        game.time = 30.;
        game
    }
    #[test]
    fn fractional_frames_retain_input_and_share_one_clock_across_replay() {
        let mut game = game();
        game.tick(0., [0.; 2]).unwrap();
        game.jump().unwrap();
        let start = game.primary.player;
        game.tick(1. / 240., [0., 1.]).unwrap();
        assert_eq!(game.time, 30.);
        assert_eq!(game.snapshot().elapsed, 0.);
        assert_eq!(game.primary.player, start);
        assert!(game.primary.pending_jump);
        let bytes = game.checkpoint().unwrap();
        let mut restored = Game::restore(&bytes).unwrap();
        for _ in 0..239 {
            game.tick(1. / 240., [0., 1.]).unwrap();
            restored.tick(1. / 240., [0., 1.]).unwrap();
            assert_eq!(game.checkpoint().unwrap(), restored.checkpoint().unwrap());
            assert_eq!(
                game.snapshot().elapsed,
                (game.physics_steps as f64 / 120.) as f32
            );
        }
        assert_eq!(game.physics_steps, 120);
        assert_eq!(game.time, 31.);
        assert_eq!(game.snapshot().elapsed, 1.);
        let mut forged: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        forged["world"]["physics_steps"] = serde_json::json!(1);
        assert!(Game::restore(&serde_json::to_vec(&forged).unwrap()).is_err());
    }
    #[test]
    fn four_substeps_jump_checkpoint_and_read_only_interpolation() {
        let mut game = game();
        game.tick(1. / 30., [0.; 2]).unwrap();
        assert_eq!(game.physics_steps, 4);
        game.jump().unwrap();
        game.tick(1. / 30., [0.; 2]).unwrap();
        assert!(game.primary.player.y > 0.1);
        assert_eq!(
            game.frame()
                .actors
                .iter()
                .find(|a| a.actor.model == "adventurer")
                .unwrap()
                .animation,
            State::Airborne.into()
        );
        let before = game.checkpoint().unwrap();
        let frame = game.interpolated_frame(0.5).unwrap();
        let position = frame
            .actors
            .iter()
            .find(|a| a.actor.model == "adventurer")
            .unwrap()
            .actor
            .position;
        assert!(position.y > game.primary.previous_player.y && position.y < game.primary.player.y);
        assert_eq!(before, game.checkpoint().unwrap());
        let mut restored = Game::restore(&before).unwrap();
        for _ in 0..120 {
            game.tick(1. / 30., [0.; 2]).unwrap();
            restored.tick(1. / 30., [0.; 2]).unwrap();
        }
        assert_eq!(game.checkpoint().unwrap(), restored.checkpoint().unwrap());
        assert!(game.primary.player.y < 0.001);
        game.jump().unwrap();
        game.control_handoff(false).unwrap();
        game.tick(1. / 30., [0.; 2]).unwrap();
        assert!(game.primary.player.y < 0.001);
    }
    #[test]
    fn frame_rates_produce_identical_grounded_authority() {
        let mut poses = vec![];
        for hz in [30, 60, 144] {
            let mut game = game();
            let mut schedule = verse_engine::core::FixedSchedule::new(30, 3).unwrap();
            for _ in 0..hz * 2 {
                let batch = schedule.advance(1. / hz as f64).unwrap();
                for _ in 0..batch.steps {
                    game.tick(batch.seconds, [0.25, 1.]).unwrap();
                }
            }
            assert_eq!(game.physics_steps, 240);
            assert_eq!(game.physics_clock.dropped, 0.);
            poses.push(game.primary.player);
        }
        assert_eq!(poses[0], poses[1]);
        assert_eq!(poses[1], poses[2]);
    }
}

#[cfg(test)]
mod compiled_navigation_tests {
    use super::*;
    fn game() -> Game {
        let scene =
            Scene::from_json(include_bytes!("../../../assets/verse/original/ritual.json")).unwrap();
        let mut game = Game::new(scene).unwrap();
        game.time = 30.;
        game
    }
    #[test]
    fn prop_invalidation_preserves_distant_routes_and_replans_local_changes() {
        let mut g = game();
        let start = Vec3::new(-2., 0., -15.);
        let target = Vec3::new(2., 0., -15.);
        g.simulation
            .place_chamber_actor(g.ids[&2], start.to_array(), 0.)
            .unwrap();
        g.sync_bodies(0.).unwrap();
        g.move_hostile(2, start, target, 0.04).unwrap();
        assert!(g.routes.contains_key(&2));
        let distant = physics::queries::Life {
            instance: g.primary.admission.actor().instance,
            entity: 900,
            generation: 0,
        };
        g.set_navigation_blocker(
            distant,
            glam::DVec3::new(-18., 0., 4.),
            glam::DVec3::new(-17., 2., 5.),
        )
        .unwrap();
        assert!(g.routes.contains_key(&2));
        assert_eq!(g.routes[&2].blocker_revision, g.blockers.revision);
        let local = physics::queries::Life {
            entity: 901,
            ..distant
        };
        g.set_navigation_blocker(
            local,
            glam::DVec3::new(-0.6, 0., -15.6),
            glam::DVec3::new(0.6, 3., -14.4),
        )
        .unwrap();
        assert!(!g.routes.contains_key(&2));
        g.move_hostile(2, start, target, 0.04).unwrap();
        assert!(g.routes[&2].refusal.is_none());
        let checkpoint = g.checkpoint().unwrap();
        let restored = Game::restore(&checkpoint).unwrap();
        assert_eq!(checkpoint, restored.checkpoint().unwrap());
    }
    #[test]
    fn exhausted_tick_defers_movement_and_queue_replays_after_checkpoint() {
        let mut g = game();
        let life = g.actor_life(2).unwrap();
        let physical = physics::queries::Life {
            instance: life.instance,
            entity: life.actor,
            generation: life.generation,
        };
        g.navigation_scheduler.begin_tick(g.authority_tick);
        for _ in 0..4 {
            assert!(
                g.navigation_scheduler
                    .request(
                        physical,
                        physics::walkable::Budget {
                            nodes: 16_384,
                            ..Default::default()
                        }
                    )
                    .unwrap()
            );
        }
        let start = Vec3::new(-2., 0., -15.);
        let target = Vec3::new(2., 0., -15.);
        assert_eq!(g.move_hostile(2, start, target, 0.04).unwrap(), start);
        assert_eq!(g.navigation_plans, 0);
        assert_eq!(g.navigation_work().3, 1);
        let mut restored = Game::restore(&g.checkpoint().unwrap()).unwrap();
        g.authority_tick += 1;
        restored.authority_tick += 1;
        assert_eq!(
            g.move_hostile(2, start, target, 0.04).unwrap(),
            restored.move_hostile(2, start, target, 0.04).unwrap()
        );
        assert_eq!(g.checkpoint().unwrap(), restored.checkpoint().unwrap());
        assert!(g.navigation_work().0 <= 4);
    }
    #[test]
    fn cultist_climbs_authored_stairs_without_teleporting() {
        let mut game = game();
        let mut position = Vec3::new(18., 0., -32.);
        let target = Vec3::new(18., 1.5, -25.);
        for _ in 0..300 {
            let next = game.move_hostile(2, position, target, 0.03).unwrap();
            assert!(Vec3::new(next.x - position.x, 0., next.z - position.z).length() <= 0.0301);
            position = next;
            game.time += 1. / 30.;
        }
        assert!(position.distance(target) < 0.02, "{position:?}");
        assert_eq!(game.navigation_budget_refusals, 0);
        assert!(game.navigation_plans < 5);
    }
    #[test]
    fn directed_intent_advances_in_authority_and_replays_through_stairs() {
        let mut scene =
            Scene::from_json(include_bytes!("../../../assets/verse/original/ritual.json")).unwrap();
        scene
            .actors
            .iter_mut()
            .find(|a| a.id == 2)
            .unwrap()
            .position = Vec3::new(18., 0., -32.);
        let mut game = Game::new(scene).unwrap();
        game.time = 30.;
        let life = game.actor_life(2).unwrap();
        let target = Vec3::new(18.8, 1.5, -25.);
        game.direct_npc_navigation(life, target, 1.2).unwrap();
        for _ in 0..120 {
            game.tick(1. / 30., [0.; 2]).unwrap();
        }
        let frame = game.frame();
        let walking = frame.actors.iter().find(|a| a.actor.id == 2).unwrap();
        assert_eq!(walking.animation, State::Walk.into());
        let mut restored = Game::restore(&game.checkpoint().unwrap()).unwrap();
        for _ in 0..180 {
            game.tick(1. / 30., [0.; 2]).unwrap();
            restored.tick(1. / 30., [0.; 2]).unwrap();
            assert_eq!(game.checkpoint().unwrap(), restored.checkpoint().unwrap());
        }
        let frame = game.frame();
        let arrived = frame.actors.iter().find(|a| a.actor.id == 2).unwrap();
        assert!(
            arrived.actor.position.distance(target) < 0.1,
            "{:?}",
            arrived.actor.position
        );
        assert!(!game.clear_npc_navigation(life.next().unwrap()));
        assert!(game.clear_npc_navigation(life));
        game.restart_combat(false).unwrap();
        assert!(game.direct_npc_navigation(life, target, 1.2).is_err());
    }
    #[test]
    fn blocker_replanning_collision_and_checkpoint_keep_the_same_life() {
        let mut game = game();
        let life = physics::queries::Life {
            instance: 0,
            entity: 9001,
            generation: 0,
        };
        game.set_navigation_blocker(
            life,
            glam::DVec3::new(-0.6, 0., -15.6),
            glam::DVec3::new(0.6, 3., -14.4),
        )
        .unwrap();
        let start = Vec3::new(-2., 0., -15.);
        let target = Vec3::new(2., 0., -15.);
        let mut position = start;
        let mut detoured = false;
        for _ in 0..180 {
            position = game.move_hostile(2, position, target, 0.04).unwrap();
            detoured |= (position.z + 15.).abs() > 0.95;
            game.time += 1. / 30.;
        }
        assert!(detoured && position.distance(target) < 0.02, "{position:?}");
        assert!(game.move_player(start, Vec3::X * 5.).unwrap().x < -0.94);
        let mut restored = Game::restore(&game.checkpoint().unwrap()).unwrap();
        assert_eq!(game.checkpoint().unwrap(), restored.checkpoint().unwrap());
        assert!(game.remove_navigation_blocker(life).unwrap());
        assert!(restored.remove_navigation_blocker(life).unwrap());
        let direct = game.move_hostile(2, start, target, 0.04).unwrap();
        let replay = restored.move_hostile(2, start, target, 0.04).unwrap();
        assert_eq!(direct, replay);
        assert_eq!(game.checkpoint().unwrap(), restored.checkpoint().unwrap());
        assert!(
            game.set_navigation_blocker(life, glam::DVec3::ZERO, glam::DVec3::ONE)
                .is_err()
        );
        let next = physics::queries::Life {
            generation: 1,
            ..life
        };
        game.set_navigation_blocker(
            next,
            glam::DVec3::new(-0.6, 0., -15.6),
            glam::DVec3::new(0.6, 3., -14.4),
        )
        .unwrap();
        assert!(!game.remove_navigation_blocker(life).unwrap());
        assert!(game.move_player(start, Vec3::X * 5.).unwrap().x < -0.94);
    }
}

#[cfg(test)]
mod body_lifetime_tests {
    use super::*;
    fn game() -> Game {
        Game::combat(
            Scene::from_json(include_bytes!("../../../assets/verse/original/ritual.json")).unwrap(),
            false,
        )
        .unwrap()
    }
    #[test]
    fn living_capsules_block_other_lives_ignore_self_and_rebuild_on_load() {
        let mut g = game();
        g.time = 30.;
        g.primary.player = Vec3::new(0., 0., -15.);
        g.simulation
            .place_chamber_actor(0, g.primary.player.to_array(), 0.)
            .unwrap();
        g.simulation
            .place_chamber_actor(g.ids[&2], [1.2, 0., -15.], 0.)
            .unwrap();
        g.sync_bodies(0.).unwrap();
        let stop = g.move_player(g.primary.player, Vec3::X * 4.).unwrap();
        assert!(stop.x <= 0.5001 && stop.x > 0.45, "{stop:?}");
        assert!(g.move_player(g.primary.player, -Vec3::X).unwrap().x < -0.99);
        let restored = Game::restore(&g.checkpoint().unwrap()).unwrap();
        assert_eq!(
            stop,
            restored
                .move_player(restored.primary.player, Vec3::X * 4.)
                .unwrap()
        );
        let life = g.actor_life(2).unwrap();
        g.place_actor_body(life, Vec3::new(-1.2, 0., -15.), 0.)
            .unwrap();
        let moved = g
            .move_hostile(2, Vec3::new(-1.2, 0., -15.), g.primary.player, 1.)
            .unwrap();
        assert!(moved.x <= -0.6999 && moved.x > -1.19, "{moved:?}");
    }
    #[test]
    fn corpse_collision_navigation_and_respawn_share_exact_life() {
        let mut g = game();
        g.time = 30.;
        let life = g.actor_life(2).unwrap();
        let physical = physics::queries::Life {
            instance: life.instance,
            entity: life.actor,
            generation: life.generation,
        };
        g.simulation.bow_impact(g.ids[&2], 1000).unwrap();
        g.tick(0., [0.; 2]).unwrap();
        assert!(matches!(
            g.bodies.get(physical).unwrap().phase,
            physics::lifetimes::Phase::Corpse { .. }
        ));
        assert!(!g.bodies.get(physical).unwrap().damage_enabled());
        assert!(
            g.blockers
                .active_bounds()
                .any(|(key, _, _)| key == physical)
        );
        assert!(
            g.query_scene
                .pose(physics::walkable::blocker_key(physical))
                .is_some()
        );
        assert!(
            !g.frame()
                .actors
                .iter()
                .find(|a| a.actor.id == 2)
                .unwrap()
                .actor
                .nameplate
        );
        let center = g.bodies.get(physical).unwrap().body.pos;
        let hits = g
            .query_scene
            .ray(
                center + glam::DVec3::Y,
                -glam::DVec3::Y,
                2.,
                physics::queries::Filter::blocking(physical.instance),
            )
            .unwrap();
        assert_eq!(hits.hits[0].collider.life, physical);
        let bytes = g.checkpoint().unwrap();
        let mut forged: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        forged["world"]["bodies"]["entries"]["2"]["phase"] = serde_json::json!("Alive");
        assert!(Game::restore(&serde_json::to_vec(&forged).unwrap()).is_err());
        let mut restored = Game::restore(&bytes).unwrap();
        assert_eq!(bytes, restored.checkpoint().unwrap());
        let until = g.npc_deaths[&2].0 + 60.;
        g.time = until;
        restored.time = until;
        g.tick(0., [0.; 2]).unwrap();
        restored.tick(0., [0.; 2]).unwrap();
        assert_eq!(g.checkpoint().unwrap(), restored.checkpoint().unwrap());
        let next = g.actor_life(2).unwrap();
        assert_eq!(next, life.next().unwrap());
        assert!(g.bodies.get(physical).is_none());
        assert!(
            !g.blockers
                .active_bounds()
                .any(|(key, _, _)| key == physical)
        );
        assert!(
            g.query_scene
                .pose(physics::walkable::blocker_key(physical))
                .is_none()
        );
        assert!(!g.bodies.remove(physical));
        let current = physics::queries::Life {
            generation: next.generation,
            ..physical
        };
        assert!(g.bodies.get(current).unwrap().selection_enabled());
    }
    #[test]
    fn restart_and_checkpoint_reject_stale_body_ownership() {
        let mut g = game();
        let old = g.primary.admission.actor();
        g.restart_combat(false).unwrap();
        let current = g.primary.admission.actor();
        assert_eq!(current.generation, old.generation + 1);
        let key = physics::queries::Life {
            instance: old.instance,
            entity: old.actor,
            generation: old.generation,
        };
        assert!(g.bodies.get(key).is_none());
        let mut saved: serde_json::Value =
            serde_json::from_slice(&g.checkpoint().unwrap()).unwrap();
        saved["world"]["bodies"]["instance"] = serde_json::json!(9);
        assert!(Game::restore(&serde_json::to_vec(&saved).unwrap()).is_err());
        assert!(
            g.set_navigation_blocker(key, glam::DVec3::ZERO, glam::DVec3::ONE)
                .is_err()
        );
    }
}

#[cfg(test)]
mod prop_body_tests {
    use super::*;
    #[test]
    fn prop_geometry_body_and_reused_life_replay_together() {
        let mut g = Game::new(
            Scene::from_json(include_bytes!("../../../assets/verse/original/ritual.json")).unwrap(),
        )
        .unwrap();
        let life = physics::queries::Life {
            instance: 0,
            entity: 9001,
            generation: 0,
        };
        let min = glam::DVec3::new(2., 0., -10.);
        let max = min + glam::DVec3::new(1., 2., 1.);
        g.set_navigation_blocker(life, min, max).unwrap();
        let body = g.physics_bodies().get(life).unwrap();
        assert!(!body.actor);
        assert!(!body.damage_enabled());
        assert!(!body.selection_enabled());
        assert_eq!(body.body.pos, (min + max) * 0.5);
        let moved_min = min + glam::DVec3::X;
        let moved_max = max + glam::DVec3::X * 2.;
        g.set_navigation_blocker(life, moved_min, moved_max)
            .unwrap();
        let body = g.physics_bodies().get(life).unwrap();
        assert_eq!(body.body.pos, (moved_min + moved_max) * 0.5);
        assert_eq!(
            body.hull,
            physics::lifetimes::Hull::Box {
                half: (moved_max - moved_min) * 0.5
            }
        );
        let before = g.checkpoint().unwrap();
        assert!(
            g.set_navigation_blocker(
                physics::queries::Life {
                    instance: 8,
                    ..life
                },
                min,
                max
            )
            .is_err()
        );
        assert_eq!(before, g.checkpoint().unwrap());
        let mut restored = Game::restore(&before).unwrap();
        assert_eq!(before, restored.checkpoint().unwrap());
        assert!(g.remove_navigation_blocker(life).unwrap());
        assert!(restored.remove_navigation_blocker(life).unwrap());
        assert!(g.set_navigation_blocker(life, min, max).is_err());
        let next = physics::queries::Life {
            generation: 1,
            ..life
        };
        g.set_navigation_blocker(next, min, max).unwrap();
        restored.set_navigation_blocker(next, min, max).unwrap();
        assert!(!g.remove_navigation_blocker(life).unwrap());
        assert_eq!(g.checkpoint().unwrap(), restored.checkpoint().unwrap());
        let mut forged: serde_json::Value =
            serde_json::from_slice(&g.checkpoint().unwrap()).unwrap();
        forged["world"]["bodies"]["entries"]
            .as_object_mut()
            .unwrap()
            .remove("9001");
        assert!(Game::restore(&serde_json::to_vec(&forged).unwrap()).is_err());
    }
}

#[cfg(test)]
mod player_respawn_tests {
    use super::*;
    fn game() -> Game {
        let scene =
            Scene::from_json(include_bytes!("../../../assets/verse/original/ritual.json")).unwrap();
        let mut game = Game::combat(scene, false).unwrap();
        game.time = game.scene.cut_at;
        game.tick(1. / 30., [0.; 2]).unwrap();
        game
    }
    #[test]
    fn respawn_preserves_hostiles_and_fences_old_commands_through_repeated_deaths() {
        let mut g = game();
        assert_eq!(g.snapshot().player.hp, 200);
        let before = g.checkpoint().unwrap();
        assert!(g.respawn_player().is_err());
        assert_eq!(before, g.checkpoint().unwrap());
        g.simulation.bow_impact(g.ids[&2], 1000).unwrap();
        g.simulation.bow_impact(g.ids[&1], 45).unwrap();
        g.tick(1. / 30., [0.; 2]).unwrap();
        for _ in 0..2 {
            let command = g
                .admission
                .command(
                    g.authority_tick,
                    crate::Intent::Move {
                        axes: [0., 1.],
                        yaw: 0.,
                    },
                )
                .unwrap();
            let old = g.player_life();
            g.primary.pending_movement = Some([0., 1.]);
            g.primary.pending_jump = true;
            g.hostile_hit(10000).unwrap();
            g.tick(1. / 30., [0.; 2]).unwrap();
            let npc_health: Vec<_> = g
                .snapshot()
                .actors
                .into_iter()
                .filter(|a| a.id != 0)
                .map(|a| (a.id, a.hp))
                .collect();
            let deadlines = g.npc_deaths.clone();
            let lives = g.lives.clone();
            g.respawn_player().unwrap();
            assert_eq!(g.player_life(), old.next().unwrap());
            assert_eq!(g.snapshot().player.hp, 200);
            assert_eq!(g.snapshot().player.mana, 20);
            assert!(!g.agent_controlled);
            assert!(
                g.primary.pending_movement.is_none()
                    && !g.primary.pending_jump
                    && g.primary.casting.is_none()
            );
            assert!(g.submit(crate::Controller(1), command).is_err());
            assert_eq!(g.lives, lives);
            assert_eq!(g.npc_deaths, deadlines);
            assert_eq!(
                g.snapshot()
                    .actors
                    .into_iter()
                    .filter(|a| a.id != 0)
                    .map(|a| (a.id, a.hp))
                    .collect::<Vec<_>>(),
                npc_health
            );
            assert!(
                !g.navigation_blockers()
                    .colliders()
                    .unwrap()
                    .iter()
                    .any(|c| c.key.life.entity == old.actor)
            );
            let physical = physics::queries::Life {
                instance: old.instance,
                entity: old.actor,
                generation: old.generation + 1,
            };
            assert_eq!(
                g.bodies.get(physical).unwrap().phase,
                physics::lifetimes::Phase::Alive
            );
            let mut restored = Game::restore(&g.checkpoint().unwrap()).unwrap();
            g.activate(Ability::Shield).unwrap();
            restored.activate(Ability::Shield).unwrap();
            for _ in 0..30 {
                g.tick(1. / 30., [0., 1.]).unwrap();
                restored.tick(1. / 30., [0., 1.]).unwrap();
                assert_eq!(g.checkpoint().unwrap(), restored.checkpoint().unwrap());
            }
        }
    }
    #[test]
    fn obstructed_spawn_is_refused_without_mutation_and_expired_corpses_can_respawn() {
        let mut g = game();
        g.hostile_hit(10000).unwrap();
        g.tick(1. / 30., [0.; 2]).unwrap();
        let spawn = g
            .scene
            .actors
            .iter()
            .find(|a| a.model == "adventurer")
            .unwrap()
            .position
            .as_dvec3();
        let prop = physics::queries::Life {
            instance: 0,
            entity: 999,
            generation: 0,
        };
        g.set_navigation_blocker(
            prop,
            spawn - glam::DVec3::splat(1.),
            spawn + glam::DVec3::splat(2.),
        )
        .unwrap();
        let before = g.checkpoint().unwrap();
        assert!(g.respawn_player().is_err());
        assert_eq!(before, g.checkpoint().unwrap());
        g.remove_navigation_blocker(prop).unwrap();
        for _ in 0..2000 {
            g.tick(1. / 30., [0.; 2]).unwrap();
        }
        assert_eq!(g.snapshot().player.hp, 0);
        g.respawn_player().unwrap();
        assert_eq!(g.snapshot().player.hp, 200);
        Game::restore(&g.checkpoint().unwrap()).unwrap();
    }
    #[test]
    fn restored_health_requires_the_current_player_maximum() {
        let g = game();
        let mut saved: serde_json::Value =
            serde_json::from_slice(&g.checkpoint().unwrap()).unwrap();
        saved["world"]["simulation"]["players"]["0"]["resources"]["max_hp"] = 100.into();
        assert!(Game::restore(&serde_json::to_vec(&saved).unwrap()).is_err());
    }
}

#[cfg(test)]
mod friendly_tests {
    use super::*;

    fn scene() -> Scene {
        let mut scene =
            Scene::from_json(include_bytes!("../../../assets/verse/original/ritual.json")).unwrap();
        let giver = scene.actors.iter_mut().find(|a| a.id == 2).unwrap();
        giver.friendly = true;
        giver.health = 123;
        scene
    }

    #[test]
    fn friendly_giver_stays_out_of_combat_and_survives_restore_and_reset() {
        let mut game = Game::combat(scene(), true).unwrap();
        assert!(!game.encounter.as_ref().unwrap().positions.contains_key(&2));
        for _ in 0..300 {
            game.tick(1. / 30., [0.; 2]).unwrap();
        }
        let giver = game
            .frame()
            .actors
            .into_iter()
            .find(|a| a.actor.id == 2)
            .unwrap();
        assert_eq!(giver.health, 123);
        assert_eq!(giver.animation, State::Idle.into());
        assert!(
            !game
                .encounter
                .as_ref()
                .unwrap()
                .casts
                .iter()
                .any(|c| c.actor == 2)
        );
        assert!(!game.living_npcs().contains(&2));
        let mut restored = Game::restore(&game.checkpoint().unwrap()).unwrap();
        assert_eq!(
            restored
                .snapshot()
                .actors
                .iter()
                .find(|a| a.id == restored.ids[&2])
                .unwrap()
                .faction,
            "friendly"
        );
        restored.restart_combat(false).unwrap();
        assert_eq!(
            restored
                .frame()
                .actors
                .iter()
                .find(|a| a.actor.id == 2)
                .unwrap()
                .health,
            123
        );
        restored.primary.selected = 2;
        let mana = restored.snapshot().player.mana;
        assert!(restored.activate(Ability::Bow).is_err());
        assert_eq!(restored.snapshot().player.mana, mana);
    }

    #[test]
    fn friendly_bodies_ignore_fields_roots_prone_and_falling_damage() {
        let mut game = Game::combat(scene(), false).unwrap();
        game.tick(0., [0.; 2]).unwrap();
        let position = game.actor_position(2).unwrap();
        let yaw = game.scene.actors.iter().find(|a| a.id == 2).unwrap().yaw;
        let source = game.ids[&2];
        let before = serde_json::to_vec(&game.spells.dice).unwrap();
        game.fall_damage(Some(2), 100.).unwrap();
        assert_eq!(serde_json::to_vec(&game.spells.dice).unwrap(), before);
        for spell in [Utility::Web, Utility::Grease] {
            game.primary
                .controls
                .cast(
                    &mut game.simulation,
                    spell,
                    game.time,
                    game.primary.player,
                    Vec3::Z,
                    Some(position),
                )
                .unwrap();
        }
        let cast = game.spells.begin_cast(game.player_actor(), false).unwrap();
        game.spells
            .add_field(crate::spells::SpellField {
                cast,
                spell: "Friendly field proof".into(),
                owner: game.player_actor(),
                area: crate::spells::Area::Sphere {
                    center: position.as_dvec3(),
                    radius: 10.,
                },
                acceleration: glam::DVec3::new(10., 10., 0.),
                expires: game.time + 10.,
                concentration: false,
            })
            .unwrap();
        for _ in 0..30 {
            game.tick(1. / 30., [0.; 2]).unwrap();
        }
        assert!(!game.primary.controls.held(source));
        let frame = game.frame();
        let giver = frame.actors.iter().find(|a| a.actor.id == 2).unwrap();
        assert_eq!(giver.health, 123);
        assert_eq!(giver.animation, State::Idle.into());
        assert_eq!(giver.actor.yaw, yaw);
        assert!(giver.actor.position.distance(position) < 0.01);
        let life = game.actor_life(2).unwrap();
        let body = game
            .bodies
            .get(physics::queries::Life {
                instance: life.instance,
                entity: life.actor,
                generation: life.generation,
            })
            .unwrap();
        assert_eq!(body.phase, physics::lifetimes::Phase::Alive);
        Game::restore(&game.checkpoint().unwrap()).unwrap();
    }

    #[test]
    fn authored_entrance_giver_is_reachable_without_changing_player_ids() {
        let scene = Scene::from_json(include_bytes!(
            "../../../assets/verse/original/ritual-quests.json"
        ))
        .unwrap();
        let progression: crate::service::progression::Config = serde_json::from_slice(
            include_bytes!("../../../assets/verse/original/ritual-progression.json"),
        )
        .unwrap();
        progression.validate().unwrap();
        assert!(
            progression
                .quests
                .iter()
                .all(|q| q.giver == Some(1_000_000))
        );
        let mut game = Game::combat(scene, false).unwrap();
        assert_eq!(game.next_player_actor, 15);
        game.tick(0., [0.; 2]).unwrap();
        let giver = game.actor_position(1_000_000).unwrap();
        assert!(giver.distance(game.primary.player) < 4.);
        assert!(game.attack_clear(game.primary.player + Vec3::Y * 1.4, giver + Vec3::Y * 1.1));
        assert_eq!(
            game.frame()
                .actors
                .iter()
                .find(|a| a.actor.id == 1_000_000)
                .unwrap()
                .health,
            200
        );
        Game::restore(&game.checkpoint().unwrap()).unwrap();
    }

    #[test]
    fn checkpoint_fences_scene_factions_and_legacy_friendly_roles() {
        let game = Game::new(scene()).unwrap();
        let value: serde_json::Value = serde_json::from_slice(&game.checkpoint().unwrap()).unwrap();
        let mut legacy = value.clone();
        legacy["rules_revision"] = "verse-chamber-owned-v16".into();
        assert!(Game::restore(&serde_json::to_vec(&legacy).unwrap()).is_err());
        let mut forged = value;
        forged["world"]["simulation"]["actors"][game.ids[&2].to_string()]["faction"] =
            "undead".into();
        assert!(Game::restore(&serde_json::to_vec(&forged).unwrap()).is_err());
        let hostile = Game::new(
            Scene::from_json(include_bytes!("../../../assets/verse/original/ritual.json")).unwrap(),
        )
        .unwrap();
        let mut old: serde_json::Value =
            serde_json::from_slice(&hostile.checkpoint().unwrap()).unwrap();
        old["rules_revision"] = "verse-chamber-owned-v16".into();
        Game::restore(&serde_json::to_vec(&old).unwrap()).unwrap();
    }
}

#[cfg(test)]
mod motor_containment_tests {
    use super::*;
    #[test]
    fn query_measurements_survive_dynamic_blocker_scene_replacement() {
        let scene =
            Scene::from_json(include_bytes!("../../../assets/verse/original/ritual.json")).unwrap();
        let mut game = Game::combat(scene, false).unwrap();
        game.enable_query_profiling();
        let filter = physics::queries::Filter::blocking(game.player_life().instance);
        game.query_scene
            .ray(glam::DVec3::Y * 2., -glam::DVec3::Y, 4., filter)
            .unwrap();
        game.set_navigation_blocker(
            physics::queries::Life {
                instance: game.player_life().instance,
                entity: 90003,
                generation: 0,
            },
            glam::DVec3::new(8., 0., -22.),
            glam::DVec3::new(8.5, 1., -21.5),
        )
        .unwrap();
        assert_eq!(game.query_profile().unwrap().ray.calls, 1);
        game.query_scene
            .ray(glam::DVec3::Y * 2., -glam::DVec3::Y, 4., filter)
            .unwrap();
        assert_eq!(game.query_profile().unwrap().ray.calls, 2);
    }

    #[test]
    fn blocked_player_recovery_does_not_stop_the_world_clock() {
        use physics::queries::{ColliderKey, Life, Mesh, MeshCollider, Usage};
        let scene =
            Scene::from_json(include_bytes!("../../../assets/verse/original/ritual.json")).unwrap();
        let mut game = Game::combat(scene, false).unwrap();
        game.time = game.scene.cut_at;
        game.tick(1. / 30., [0.; 2]).unwrap();
        let position = game.primary.player;
        let instance = game.player_life().instance;
        let owner = crate::Controller(9);
        let unrelated_spawn = position + Vec3::X * 8.;
        let unrelated = game.add_player(owner, unrelated_spawn).unwrap();
        let admission = game.player_admission(unrelated.actor).unwrap();
        game.submit(
            owner,
            crate::Command {
                actor: unrelated,
                epoch: admission.epoch(),
                sequence: admission.accepted_sequence() + 1,
                tick: game.authority_tick,
                intent: crate::Intent::Move {
                    axes: [1., 0.],
                    yaw: 0.,
                },
            },
        )
        .unwrap();
        for (entity, lo, hi) in [(90001, -2., 0.1), (90002, -0.1, 2.)] {
            game.query_scene
                .insert(MeshCollider {
                    key: ColliderKey {
                        life: Life {
                            instance,
                            entity,
                            generation: 0,
                        },
                        shape: 0,
                    },
                    layers: 1,
                    usage: Usage::Blocking,
                    mesh: Mesh::from_box(
                        position.as_dvec3() + glam::DVec3::new(lo, -1., -2.),
                        position.as_dvec3() + glam::DVec3::new(hi, 4., 2.),
                    )
                    .unwrap(),
                })
                .unwrap();
        }
        let tick = game.authority_tick;
        let time = game.time;
        for _ in 0..3 {
            game.tick(1. / 30., [1., 0.]).unwrap();
            assert_eq!(game.primary.player, position);
        }
        assert_eq!(game.authority_tick, tick + 3);
        assert!(game.time > time);
        assert!(
            game.actor_position(unrelated.actor)
                .unwrap()
                .distance(unrelated_spawn)
                > 0.1
        );
        assert_eq!(game.motor_recovery.blocks, 3);
        assert!(game.motor_recovery.last_diagnostic.is_some());
        // An already planned route can become obstructed before its motor step.
        // Exercise the navigation motor rather than allowing the planner to refuse it.
        game.routes.insert(
            1,
            Route {
                life: game.actor_life(1).unwrap(),
                target: position + Vec3::X,
                planned_at: game.time,
                blocker_revision: game.blockers.revision,
                points: vec![(position + Vec3::X).as_dvec3()],
                cursor: 0,
                stuck_steps: 0,
                refusal: None,
            },
        );
        assert_eq!(
            game.move_hostile(1, position, position + Vec3::X, 0.1)
                .unwrap(),
            position
        );
        assert_eq!(game.motor_recovery.blocks, 4);

        let checkpoint: serde_json::Value =
            serde_json::from_slice(&game.checkpoint().unwrap()).unwrap();
        assert!(checkpoint["world"].get("motor_recovery").is_none());
    }
}
