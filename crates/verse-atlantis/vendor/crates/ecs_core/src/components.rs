//! ECS component definitions for destructibles and voxel chunk meshes.
//!
//! These components are shared across server and client crates. The server owns
//! authoritative mutation for `VoxelProxy` and emits `ChunkDirty`/`ChunkMesh`
//! updates; the client consumes `ChunkMesh` for GPU upload.

use glam::{DVec3, UVec3, Vec3};
use std::collections::HashMap;

/// Opaque entity identifier (server-assigned). Stable across replication.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "replication", derive(serde::Serialize, serde::Deserialize))]
pub struct EntityId(pub u64);

/// Stable identifier for a destructible object (shared across client/server).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "replication", derive(serde::Serialize, serde::Deserialize))]
pub struct DestructibleId(pub u64);

/// Destructible tag with material for mass/density.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Destructible {
    pub id: u64,
    pub material: core_materials::MaterialId,
}

/// The CPU voxel proxy metadata for a destructible.
#[derive(Debug, Clone)]
pub struct VoxelProxy {
    pub meta: voxel_proxy::VoxelProxyMeta,
}

/// List of dirty chunk coordinates since last mesh/collider rebuild.
#[derive(Debug, Clone, Default, PartialEq)]
#[cfg_attr(feature = "replication", derive(serde::Serialize, serde::Deserialize))]
pub struct ChunkDirty(pub Vec<UVec3>);

/// A single CPU mesh buffer for a chunk (positions, normals, indices).
#[derive(Debug, Clone, Default, PartialEq)]
#[cfg_attr(feature = "replication", derive(serde::Serialize, serde::Deserialize))]
pub struct MeshCpu {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub indices: Vec<u32>,
}

impl MeshCpu {
    /// Validate CPU mesh invariants.
    pub fn validate(&self) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.positions.len() == self.normals.len(),
            "pos/normal len mismatch"
        );
        anyhow::ensure!(
            self.indices.len().is_multiple_of(3),
            "indices not multiple of 3"
        );
        Ok(())
    }
}

/// Mapping from chunk coords to mesh for a given destructible proxy.
#[derive(Debug, Clone, Default, PartialEq)]
#[cfg_attr(feature = "replication", derive(serde::Serialize, serde::Deserialize))]
pub struct ChunkMesh {
    pub map: HashMap<(u32, u32, u32), MeshCpu>,
}

/// Carve request produced by projectile/destructible intersection on the server.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "replication", derive(serde::Serialize, serde::Deserialize))]
pub struct CarveRequest {
    pub did: u64,
    pub center_m: DVec3,
    pub radius_m: f64,
    pub seed: u64,
    pub impact_id: u32,
}

/// Canonical chunk key type (local chunk coords).
pub type ChunkKey = (u32, u32, u32);

/// Helper to build stable keys for per-destructible chunk maps.
pub fn chunk_key(did: DestructibleId, c: UVec3) -> (DestructibleId, u32, u32, u32) {
    (did, c.x, c.y, c.z)
}

/// Runtime-selectable input/controller profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InputProfile {
    #[default]
    ActionCombat,
    ClassicCursor,
}

/// High-level controller mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ControllerMode {
    #[default]
    Mouselook,
    Cursor,
}

/// Read-only camera pose for renderer consumption.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CameraPose {
    pub eye: Vec3,
    pub look_dir: Vec3,
    pub up: Vec3,
    pub yaw: f32,
    pub pitch: f32,
}

impl Default for CameraPose {
    fn default() -> Self {
        Self {
            eye: Vec3::ZERO,
            look_dir: Vec3::Z,
            up: Vec3::Y,
            yaw: 0.0,
            pitch: 0.0,
        }
    }
}

/// Input commands emitted by the client controller; server consumes later.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum InputCommand {
    AtWillLMB,
    AtWillRMB,
    EncounterQ,
    EncounterE,
    EncounterR,
    Dodge,
    ClassMechanic,
    CursorToggle,
}

/// Unique boss identifier (for indices/telemetry).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "replication", derive(serde::Serialize, serde::Deserialize))]
pub struct BossId(pub u32);

/// Boss tag containing a stable id for lookups.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "replication", derive(serde::Serialize, serde::Deserialize))]
pub struct Boss {
    pub id: BossId,
}

/// Display name for an entity (unique bosses, NPCs, etc.).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "replication", derive(serde::Serialize, serde::Deserialize))]
pub struct Name(pub String);

/// Marker for entities that must be unique in a scene.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[cfg_attr(feature = "replication", derive(serde::Serialize, serde::Deserialize))]
pub struct Unique;

/// Armor Class component.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "replication", derive(serde::Serialize, serde::Deserialize))]
pub struct ArmorClass {
    pub ac: i32,
}

/// Saving throw modifiers by ability.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "replication", derive(serde::Serialize, serde::Deserialize))]
pub struct SavingThrows {
    pub str_mod: i8,
    pub dex_mod: i8,
    pub con_mod: i8,
    pub int_mod: i8,
    pub wis_mod: i8,
    pub cha_mod: i8,
}

/// Common damage types used for resistances and vulnerabilities.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "replication", derive(serde::Serialize, serde::Deserialize))]
pub enum DamageType {
    Acid,
    Bludgeoning,
    Cold,
    Fire,
    Force,
    Lightning,
    Necrotic,
    Piercing,
    Poison,
    Psychic,
    Radiant,
    Slashing,
    Thunder,
}

/// Conditions for immunities and status effects (subset for MVP).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "replication", derive(serde::Serialize, serde::Deserialize))]
pub enum Condition {
    Blinded,
    Charmed,
    Deafened,
    Frightened,
    Grappled,
    Incapacitated,
    Invisible,
    Paralyzed,
    Petrified,
    Poisoned,
    Prone,
    Restrained,
    Stunned,
    Unconscious,
}

/// List of damage resistances (half damage) for an entity.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "replication", derive(serde::Serialize, serde::Deserialize))]
pub struct Resistances {
    pub damage: Vec<DamageType>,
}

/// List of condition immunities for an entity.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "replication", derive(serde::Serialize, serde::Deserialize))]
pub struct Immunities {
    pub conditions: Vec<Condition>,
}

/// Legendary Resistances (per-day charges).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "replication", derive(serde::Serialize, serde::Deserialize))]
pub struct LegendaryResistances {
    pub per_day: u8,
    pub remaining: u8,
}

impl LegendaryResistances {
    pub fn new(per_day: u8) -> Self {
        Self {
            per_day,
            remaining: per_day,
        }
    }
}

/// How legendary resources reset.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "replication", derive(serde::Serialize, serde::Deserialize))]
pub enum ResetRule {
    LongRest,
    PerEncounter,
}

/// Legendary Resistances with reset rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "replication", derive(serde::Serialize, serde::Deserialize))]
pub struct LegendaryResist {
    pub per_day: u8,
    pub left: u8,
    pub reset: ResetRule,
}

impl LegendaryResist {
    pub fn new(per_day: u8, reset: ResetRule) -> Self {
        Self {
            per_day,
            left: per_day,
            reset,
        }
    }
}

/// Raw ability scores with proficiency bonus.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "replication", derive(serde::Serialize, serde::Deserialize))]
pub struct Abilities {
    pub str: i8,
    pub dex: i8,
    pub con: i8,
    pub int: i8,
    pub wis: i8,
    pub cha: i8,
    pub prof: i8,
}

/// Combined defenses: damage resistances and condition immunities.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "replication", derive(serde::Serialize, serde::Deserialize))]
pub struct Defenses {
    pub resist: Vec<DamageType>,
    pub immune: Vec<Condition>,
}

/// Spell identifier newtype.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "replication", derive(serde::Serialize, serde::Deserialize))]
pub struct SpellId(pub String);

/// Minimal spellbook buckets for MVP boss wiring.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "replication", derive(serde::Serialize, serde::Deserialize))]
pub struct Spellbook {
    pub cantrips: Vec<SpellId>,
    pub level_1_3: Vec<SpellId>,
    pub level_4_5: Vec<SpellId>,
    pub signature: Vec<SpellId>,
}

/// Health component for damage/death application.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "replication", derive(serde::Serialize, serde::Deserialize))]
pub struct Health {
    pub hp: i32,
    pub max: i32,
}

/// Faction affiliation (used for friendly fire and aggro).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "replication", derive(serde::Serialize, serde::Deserialize))]
pub struct Faction {
    pub id: u32,
}

/// Linear velocity for movement integration (server-side).
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "replication", derive(serde::Serialize, serde::Deserialize))]
pub struct Velocity {
    pub lin: Vec3,
}

impl Default for Velocity {
    fn default() -> Self {
        Self { lin: Vec3::ZERO }
    }
}

/// NPC parameters and transient cooldowns (server-side AI/melee).
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "replication", derive(serde::Serialize, serde::Deserialize))]
pub struct Npc {
    pub radius: f32,
    pub speed_mps: f32,
    pub damage: i32,
    pub attack_cooldown_s: f32,
}

impl Default for Npc {
    fn default() -> Self {
        Self {
            radius: 0.9,
            speed_mps: 2.0,
            damage: 5,
            attack_cooldown_s: 0.0,
        }
    }
}

/// Collision shape for broad/narrow collision in server systems.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "replication", derive(serde::Serialize, serde::Deserialize))]
pub enum CollisionShape {
    Sphere {
        center: Vec3,
        radius: f32,
    },
    CapsuleY {
        center: Vec3,
        radius: f32,
        half_height: f32,
    },
    Aabb {
        min: Vec3,
        max: Vec3,
    },
}

/// Projectile component for authoritative integration and collision.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "replication", derive(serde::Serialize, serde::Deserialize))]
pub struct Projectile {
    pub radius_m: f32,
    pub damage: i32,
    pub life_s: f32,
    pub owner: EntityId,
    pub pos: Vec3,
    pub vel: Vec3,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunk_mesh_insert_and_validate() {
        let mut cm = ChunkMesh::default();
        let key = (0u32, 1, 2);
        let m = MeshCpu {
            positions: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            normals: vec![[0.0, 1.0, 0.0]; 3],
            indices: vec![0, 1, 2],
        };
        m.validate().expect("valid tri");
        cm.map.insert(key, m.clone());
        assert!(cm.map.contains_key(&key));
        assert_eq!(cm.map.get(&key).unwrap().indices.len(), 3);
    }

    #[test]
    fn legendary_resistances_init() {
        let lr = LegendaryResistances::new(3);
        assert_eq!(lr.per_day, 3);
        assert_eq!(lr.remaining, 3);
    }
}
