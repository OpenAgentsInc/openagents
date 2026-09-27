#![deny(clippy::unwrap_used, clippy::expect_used)]
//! In‑process NPC state and simple melee AI/collision avoidance.
//!
//! Also hosts simple voxel destructible helpers (see `destructible` module):
//! - Grid raycast via Amanatides & Woo DDA
//! - Carve impact sphere + spawn debris with seeded RNG

use ecs_core::components as ec;
pub mod actor;
pub mod combat;
pub use actor::*;
pub use combat::*;
use glam::Vec3;
pub mod destructible;
pub mod ecs;
pub mod jobs;
pub mod scene_build;
pub mod systems;
pub mod zones;
// (destructible module disabled)

// ----------------------------------------------------------------------------
// Specs (tuning tables)
// ----------------------------------------------------------------------------
#[derive(Debug, Clone, Copy)]
pub struct SpellSpec {
    pub cost: i32,
    pub cd_s: f32,
    pub gcd_s: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct SpellsSpec {
    pub firebolt: SpellSpec,
    pub fireball: SpellSpec,
    pub magic_missile: SpellSpec,
}

#[derive(Debug, Clone, Copy)]
pub struct EffectsSpec {
    pub fireball_burn_dps: i32,
    pub fireball_burn_s: f32,
    pub mm_slow_mul: f32,
    pub mm_slow_s: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct HomingSpec {
    pub mm_turn_rate: f32,
    pub mm_max_range_m: f32,
    pub reacquire: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct Specs {
    pub spells: SpellsSpec,
    pub effects: EffectsSpec,
    pub homing: HomingSpec,
}

impl Default for Specs {
    fn default() -> Self {
        Self {
            spells: SpellsSpec {
                firebolt: SpellSpec {
                    cost: 0,
                    cd_s: 0.30,
                    gcd_s: 0.30,
                },
                fireball: SpellSpec {
                    cost: 5,
                    cd_s: 2.00,
                    gcd_s: 0.50,
                },
                magic_missile: SpellSpec {
                    cost: 2,
                    cd_s: 1.50,
                    gcd_s: 0.30,
                },
            },
            effects: EffectsSpec {
                fireball_burn_dps: 6,
                fireball_burn_s: 3.0,
                mm_slow_mul: 0.7,
                mm_slow_s: 2.0,
            },
            homing: HomingSpec {
                mm_turn_rate: 3.5,
                mm_max_range_m: 35.0,
                reacquire: true,
            },
        }
    }
}

// Legacy NPC types removed. Use ActorStore (Zombie/Boss kinds).

/// Stored boss stats built from data_runtime config (no ECS world yet).
#[derive(Debug, Clone)]
pub struct NivitaStats {
    pub name: String,
    pub ac: i32,
    pub abilities: ec::Abilities,
    pub saves: ec::SavingThrows,
    pub defenses: ec::Defenses,
    pub legendary: ec::LegendaryResist,
    pub spellbook: ec::Spellbook,
    pub radius: f32,
    pub height: f32,
    pub team: Option<String>,
    pub team_id: Option<u32>,
}

/// Minimal boss status used by clients.
#[derive(Debug, Clone)]
pub struct BossStatus {
    pub name: String,
    pub ac: i32,
    pub hp: i32,
    pub max: i32,
    pub pos: Vec3,
}

// Legacy Wizard removed. Use ActorStore (Wizard kind).

/// Projectile kind enum.
///
/// IMPORTANT: The server is authoritative over all projectile tuning
/// (speed, lifetime, AoE radius, damage). Clients must never supply
/// gameplay parameters — they only request a kind.
#[derive(Debug, Clone, Copy)]
pub enum ProjKind {
    Firebolt,
    Fireball,
    MagicMissile,
}

/// Pending projectile input; schedule spawns ECS projectiles from this queue.
#[derive(Debug, Clone)]
pub struct PendingProjectile {
    pub pos: Vec3,
    pub dir: Vec3,
    pub kind: ProjKind,
    pub owner: Option<ActorId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SpellId {
    Firebolt,
    Fireball,
    MagicMissile,
}

#[derive(Debug, Clone)]
pub struct CastCmd {
    pub pos: Vec3,
    pub dir: Vec3,
    pub spell: SpellId,
    pub caster: Option<ActorId>,
}

/// Server-side resolved projectile parameters used for spawning and collision.
#[derive(Debug, Clone, Copy)]
pub struct ProjectileSpec {
    pub speed_mps: f32,
    pub life_s: f32,
    pub aoe_radius_m: f32,
    pub damage: i32,
    pub arming_delay_s: f32,
    pub carves_destructibles: bool,
    pub carve_radius_m: f32,
}

#[inline]
#[allow(dead_code)]
fn segment_hits_circle_xz(p0: Vec3, p1: Vec3, center: Vec3, radius: f32) -> bool {
    let a = glam::Vec2::new(p0.x, p0.z);
    let b = glam::Vec2::new(p1.x, p1.z);
    let c = glam::Vec2::new(center.x, center.z);
    let ab = b - a;
    let len2 = ab.length_squared();
    if len2 <= 1e-12 {
        return (a - c).length_squared() <= radius * radius;
    }
    let t = ((c - a).dot(ab) / len2).clamp(0.0, 1.0);
    let closest = a + ab * t;
    (closest - c).length_squared() <= radius * radius
}

// Legacy NPC ctor removed.

// Legacy hit events removed.

#[derive(Debug, Default)]
pub struct ServerState {
    /// Unique boss handle if spawned (e.g., Nivita).
    pub nivita_actor_id: Option<ActorId>,
    /// Snapshot of Nivita's boss stats/components for replication/logging.
    pub nivita_stats: Option<NivitaStats>,
    /// Pending projectile spawns from input (consumed by schedule)
    pub pending_projectiles: Vec<PendingProjectile>,
    /// Pending casts (server-authoritative gating)
    pub pending_casts: Vec<CastCmd>,
    /// New authoritative ECS world (phase 1)
    pub ecs: ecs::WorldEcs,
    pub factions: FactionState,
    /// Cached PC actor id (spawned during sync)
    pub pc_actor: Option<ActorId>,
    /// Tuning tables for spells, effects, and homing.
    pub specs: Specs,
    /// Cached archetype specs (loaded once).
    pub specs_arche: data_runtime::specs::archetypes::ArchetypeSpecDb,
    /// Cached projectile specs (loaded once).
    pub specs_proj: data_runtime::specs::projectiles::ProjectileSpecDb,
    /// Frame-local hit effects emitted by projectile collisions (drained by platform).
    pub fx_hits: Vec<net_core::snapshot::HitFx>,
    /// Frame-local HUD toasts emitted by systems (drained by platform).
    pub hud_toasts: Vec<u8>,
    // Destructible ECS runtime
    pub destruct_registry: crate::destructible::state::DestructibleRegistry,
    pub destruct_instances: Vec<scene_build::DestructibleWorldAabb>,
    pub destruct_bootstrap_instances_outstanding: bool,
}

impl ServerState {
    // Legacy AoE removed; production path is in ECS schedule systems.
    pub fn new() -> Self {
        let specs_arche =
            data_runtime::specs::archetypes::ArchetypeSpecDb::load_default().unwrap_or_default();
        let specs_proj =
            data_runtime::specs::projectiles::ProjectileSpecDb::load_default().unwrap_or_default();
        Self {
            nivita_actor_id: None,
            nivita_stats: None,
            pending_projectiles: Vec::new(),
            pending_casts: Vec::new(),
            ecs: ecs::WorldEcs::default(),
            factions: FactionState::default(),
            pc_actor: None,
            specs: Specs::default(),
            specs_arche,
            specs_proj,
            fx_hits: Vec::new(),
            hud_toasts: Vec::new(),
            destruct_registry: crate::destructible::state::DestructibleRegistry::default(),
            destruct_instances: Vec::new(),
            destruct_bootstrap_instances_outstanding: false,
        }
    }
    /// Provide world AABBs for all known destructible instances as net records.
    pub fn all_destructible_instances(&self) -> Vec<net_core::snapshot::DestructibleInstance> {
        self.destruct_instances
            .iter()
            .map(|d| net_core::snapshot::DestructibleInstance {
                did: d.did,
                world_min: d.world_min,
                world_max: d.world_max,
            })
            .collect()
    }

    /// Drain queued chunk mesh deltas accumulated during the last tick.
    pub fn drain_destruct_mesh_deltas(&mut self) -> Vec<net_core::snapshot::ChunkMeshDelta> {
        let mut v = Vec::new();
        std::mem::swap(&mut v, &mut self.destruct_registry.pending_mesh_deltas);
        v
    }
    // Legacy actor rebuild removed. Actors are authoritative.

    /// Apply AoE to actors, skipping self-damage for PC-owned sources and flipping factions when PC hits Wizards.
    #[allow(dead_code)]
    fn apply_aoe_at_actors(
        &mut self,
        x: f32,
        z: f32,
        r2: f32,
        damage: i32,
        source: Option<ActorId>,
    ) -> usize {
        let src_team = source.and_then(|id| self.ecs.get(id).map(|a| a.faction));
        let mut hits = 0usize;
        for a in self.ecs.iter_mut() {
            // Skip self-damage for PC-owned AoE
            if let (Some(crate::actor::Faction::Pc), Some(src)) = (src_team, source)
                && a.id == src
            {
                continue;
            }
            let dx = a.tr.pos.x - x;
            let dz = a.tr.pos.z - z;
            if dx * dx + dz * dz <= r2 && a.hp.alive() {
                a.hp.hp = (a.hp.hp - damage).max(0);
                hits += 1;
                if let Some(crate::actor::Faction::Pc) = src_team
                    && a.faction == crate::actor::Faction::Wizards
                {
                    self.factions.pc_vs_wizards_hostile = true;
                }
            }
        }
        hits
    }

    /// Public helper for tests: return spatial broad-phase candidate count for a segment in XZ.
    pub fn spatial_candidates_for_segment(&self, a: glam::Vec2, b: glam::Vec2, pad: f32) -> usize {
        let mut grid = crate::ecs::schedule::SpatialGrid::default();
        grid.rebuild(self);
        grid.query_segment(a, b, pad).len()
    }
    /// Mirror wizard positions into ActorStore and ensure PC/NPC wizard actors exist.
    pub fn sync_wizards(&mut self, wiz_pos: &[Vec3]) {
        // PC (index 0)
        if let Some(p0) = wiz_pos.first().copied() {
            // If missing or dead, (re)spawn the PC actor so casts always have a valid caster
            let need_spawn = match self.pc_actor {
                None => true,
                Some(id) => match self.ecs.get(id) {
                    None => true,
                    Some(a) => !a.hp.alive(),
                },
            };
            if need_spawn {
                let id = self.ecs.spawn(
                    ActorKind::Wizard,
                    crate::actor::Faction::Pc,
                    Transform {
                        pos: p0,
                        yaw: 0.0,
                        radius: 0.7,
                    },
                    Health { hp: 100, max: 100 },
                );
                self.pc_actor = Some(id);
                // Attach basic casting resources to PC
                if let Some(pc) = self.ecs.get_mut(id) {
                    pc.pool = Some(ecs::ResourcePool {
                        mana: 20,
                        max: 20,
                        regen_per_s: 1.0,
                        mana_frac: 0.0,
                    });
                    use std::collections::HashMap;
                    pc.cooldowns = Some(ecs::Cooldowns {
                        gcd_s: 0.30,
                        gcd_ready: 0.0,
                        per_spell: HashMap::new(),
                    });
                    pc.spellbook = Some(ecs::Spellbook {
                        known: vec![SpellId::Firebolt, SpellId::Fireball, SpellId::MagicMissile],
                    });
                }
            } else if let Some(id) = self.pc_actor
                && let Some(a) = self.ecs.get_mut(id)
            {
                a.tr.pos = p0;
            }
        }
        // Extra wizard positions correspond to NPC wizards (Faction::Wizards)
        let need = wiz_pos.len().saturating_sub(1);
        let mut npc_ids: Vec<ActorId> = self
            .ecs
            .iter()
            .filter(|a| a.kind == ActorKind::Wizard && a.faction == crate::actor::Faction::Wizards)
            .map(|a| a.id)
            .collect();
        while npc_ids.len() < need {
            let id = self.ecs.spawn(
                ActorKind::Wizard,
                crate::actor::Faction::Wizards,
                Transform {
                    pos: Vec3::ZERO,
                    yaw: 0.0,
                    radius: 0.7,
                },
                Health { hp: 100, max: 100 },
            );
            // Attach basic casting resources to NPC wizard for AI
            if let Some(w) = self.ecs.get_mut(id) {
                if w.pool.is_none() {
                    w.pool = Some(ecs::ResourcePool {
                        mana: 20,
                        max: 20,
                        regen_per_s: 0.5,
                        mana_frac: 0.0,
                    });
                }
                if w.cooldowns.is_none() {
                    use std::collections::HashMap;
                    w.cooldowns = Some(ecs::Cooldowns {
                        gcd_s: 0.30,
                        gcd_ready: 0.0,
                        per_spell: HashMap::new(),
                    });
                }
                if w.spellbook.is_none() {
                    w.spellbook = Some(ecs::Spellbook {
                        known: vec![SpellId::Firebolt, SpellId::Fireball, SpellId::MagicMissile],
                    });
                }
            }
            npc_ids.push(id);
        }
        for (i, id) in npc_ids.into_iter().enumerate() {
            if let Some(p) = wiz_pos.get(i + 1).copied()
                && let Some(a) = self.ecs.get_mut(id)
            {
                a.tr.pos = p;
            }
        }
    }
    // Projectiles are spawned via schedule ingestion using pending_projectiles.
    /// Convenience enqueue for PC-owned projectile input; schedule consumes.
    pub fn spawn_projectile_from_pc(&mut self, pos: Vec3, dir: Vec3, kind: ProjKind) {
        let owner = self.pc_actor;
        self.pending_projectiles.push(PendingProjectile {
            pos,
            dir,
            kind,
            owner,
        });
    }

    /// Enqueue a cast; cast system will validate and translate to projectiles.
    pub fn enqueue_cast(&mut self, pos: Vec3, dir: Vec3, spell: SpellId) {
        let caster = self.pc_actor; // local demo assumes one PC caster
        if std::env::var("RA_LOG_CASTS")
            .map(|v| v == "1")
            .unwrap_or(false)
        {
            log::info!("srv: enqueue_cast {:?} (caster={:?})", spell, caster);
        }
        self.pending_casts.push(CastCmd {
            pos,
            dir,
            spell,
            caster,
        });
    }

    /// Convenience: enqueue a projectile owned by a specific caster.
    pub fn spawn_projectile_from(&mut self, caster: ActorId, pos: Vec3, dir: Vec3, kind: ProjKind) {
        self.pending_projectiles.push(PendingProjectile {
            pos,
            dir,
            kind,
            owner: Some(caster),
        });
    }
    /// Spawn the PC actor at a chosen position if missing; attach casting resources.
    pub fn spawn_pc_at(&mut self, pos: Vec3) -> ActorId {
        if let Some(id) = self.pc_actor
            && self.ecs.get(id).is_some()
        {
            return id;
        }
        let id = self.ecs.spawn(
            ActorKind::Wizard,
            crate::actor::Faction::Pc,
            Transform {
                pos,
                yaw: 0.0,
                radius: 0.7,
            },
            Health { hp: 100, max: 100 },
        );
        self.pc_actor = Some(id);
        if let Some(pc) = self.ecs.get_mut(id) {
            pc.pool = Some(ecs::ResourcePool {
                mana: 20,
                max: 20,
                regen_per_s: 1.0,
                mana_frac: 0.0,
            });
            use std::collections::HashMap;
            pc.cooldowns = Some(ecs::Cooldowns {
                gcd_s: 0.30,
                gcd_ready: 0.0,
                per_spell: HashMap::new(),
            });
            pc.spellbook = Some(ecs::Spellbook {
                known: vec![SpellId::Firebolt, SpellId::Fireball, SpellId::MagicMissile],
            });
            pc.move_speed = Some(ecs::MoveSpeed { mps: 5.0 });
        }
        id
    }

    /// Set a movement intent on the PC actor (consumed by schedule at start of tick).
    pub fn apply_move_intent(&mut self, dx: f32, dz: f32, run: bool) {
        if let Some(id) = self.pc_actor
            && let Some(pc) = self.ecs.get_mut(id)
        {
            pc.intent_move = Some(crate::ecs::IntentMove { dx, dz, run });
        }
    }
    /// Set an aim/yaw intent on the PC actor.
    pub fn apply_aim_intent(&mut self, yaw: f32) {
        if let Some(id) = self.pc_actor
            && let Some(pc) = self.ecs.get_mut(id)
        {
            pc.intent_aim = Some(crate::ecs::IntentAim { yaw });
        }
    }
    /// Resolve server-authoritative projectile spec. Falls back to baked defaults
    /// when the DB cannot be loaded. Public for tests and tools.
    pub fn projectile_spec(&self, kind: ProjKind) -> ProjectileSpec {
        let db = Some(&self.specs_proj);
        match kind {
            ProjKind::Firebolt => {
                let s = db
                    .as_ref()
                    .and_then(|db| db.actions.get("AtWillLMB"))
                    .cloned()
                    .unwrap_or(data_runtime::specs::projectiles::ProjectileSpec {
                        speed_mps: 40.0,
                        radius_m: 0.2,
                        damage: 10,
                        life_s: 1.5,
                        arming_delay_s: 0.08,
                        carves_destructibles: false,
                        carve_radius_m: 0.0,
                    });
                ProjectileSpec {
                    speed_mps: s.speed_mps,
                    life_s: s.life_s,
                    aoe_radius_m: 0.0,
                    damage: s.damage,
                    arming_delay_s: s.arming_delay_s,
                    carves_destructibles: false,
                    carve_radius_m: 0.0,
                }
            }
            ProjKind::Fireball => {
                let s = db
                    .as_ref()
                    .and_then(|db| db.actions.get("EncounterQ"))
                    .cloned()
                    .unwrap_or(data_runtime::specs::projectiles::ProjectileSpec {
                        speed_mps: 30.0,
                        radius_m: 6.0,
                        damage: 28,
                        life_s: 1.5,
                        arming_delay_s: 0.10,
                        carves_destructibles: true,
                        carve_radius_m: 2.0,
                    });
                ProjectileSpec {
                    speed_mps: s.speed_mps,
                    life_s: s.life_s,
                    aoe_radius_m: s.radius_m.max(0.0),
                    damage: s.damage.max(0),
                    arming_delay_s: s.arming_delay_s,
                    carves_destructibles: s.carves_destructibles,
                    carve_radius_m: if s.carves_destructibles {
                        s.carve_radius_m.max(0.0)
                    } else {
                        0.0
                    },
                }
            }
            ProjKind::MagicMissile => {
                // Close-range, medium speed, short TTL; damage light per hit
                let s = self
                    .specs_proj
                    .actions
                    .get("MagicMissile")
                    .cloned()
                    .unwrap_or(data_runtime::specs::projectiles::ProjectileSpec {
                        speed_mps: 28.0,
                        radius_m: 0.5,
                        damage: 7,
                        life_s: 1.0,
                        arming_delay_s: 0.08,
                        carves_destructibles: false,
                        carve_radius_m: 0.0,
                    });
                ProjectileSpec {
                    speed_mps: s.speed_mps,
                    life_s: s.life_s,
                    aoe_radius_m: s.radius_m.max(0.0),
                    damage: s.damage.max(0),
                    arming_delay_s: s.arming_delay_s,
                    carves_destructibles: s.carves_destructibles,
                    carve_radius_m: if s.carves_destructibles {
                        s.carve_radius_m.max(0.0)
                    } else {
                        0.0
                    },
                }
            }
        }
    }

    fn spell_cost_cooldown(&self, spell: SpellId) -> (i32, f32, f32) {
        match spell {
            SpellId::Firebolt => {
                let s = &self.specs.spells.firebolt;
                (s.cost, s.cd_s, s.gcd_s)
            }
            SpellId::Fireball => {
                let s = &self.specs.spells.fireball;
                (s.cost, s.cd_s, s.gcd_s)
            }
            SpellId::MagicMissile => {
                let s = &self.specs.spells.magic_missile;
                (s.cost, s.cd_s, s.gcd_s)
            }
        }
    }
    /// Step server-authoritative systems: NPC AI/melee, wizard casts, projectile
    /// integration/collision. Collisions reduce HP for both NPCs and wizards.
    /// Wizard positions are no longer mirrored here; movement/aim are applied via intents.
    pub fn step_authoritative(&mut self, dt: f32) {
        // Run ECS schedule
        let mut ctx = crate::ecs::schedule::Ctx {
            dt,
            time_s: 0.0,
            ..Default::default()
        };
        let mut sched = crate::ecs::schedule::Schedule;
        sched.run(self, &mut ctx);
        // Drain per-tick visual hits into server buffer for platform to replicate
        if !ctx.fx_hits.is_empty() {
            self.fx_hits.append(&mut ctx.fx_hits);
        }
        if !ctx.hud_toasts.is_empty() {
            self.hud_toasts.append(&mut ctx.hud_toasts);
        }
        // Destructible runtime disabled
    }
    /// Spawn an Undead actor (legacy NPC replacement)
    pub fn spawn_undead(&mut self, pos: Vec3, radius: f32, hp: i32) -> ActorId {
        let mut pos = push_out_of_pc_bubble(self, pos);
        pos = push_out_of_destructibles(self, pos);
        pos = push_out_of_actors(self, pos, radius);
        let id = self.ecs.spawn(
            ActorKind::Zombie,
            crate::actor::Faction::Undead,
            Transform {
                pos,
                yaw: 0.0,
                radius,
            },
            Health { hp, max: hp },
        );
        // Defaults for undead
        if let Some(a) = self.ecs.get_mut(id) {
            let spec = self.specs_arche.entries.get("Undead").cloned().unwrap_or(
                data_runtime::specs::archetypes::ArchetypeSpec {
                    radius_m: radius,
                    move_speed_mps: 2.0,
                    aggro_radius_m: 25.0,
                    attack_radius_m: 0.35,
                    melee_damage: 5,
                    melee_cooldown_s: 0.6,
                },
            );
            a.move_speed = Some(ecs::MoveSpeed {
                mps: spec.move_speed_mps,
            });
            a.aggro = Some(ecs::AggroRadius {
                m: spec.aggro_radius_m,
            });
            a.attack = Some(ecs::AttackRadius {
                m: spec.attack_radius_m,
            });
            a.melee = Some(ecs::Melee {
                damage: spec.melee_damage,
                cooldown_s: spec.melee_cooldown_s,
                ready_in_s: 0.0,
            });
        }
        id
    }
    /// Spawn a Death Knight (boss-like hostile). Not unique by design.
    pub fn spawn_death_knight(&mut self, pos: Vec3) -> ActorId {
        let mut pos = push_out_of_pc_bubble(self, pos);
        pos = push_out_of_destructibles(self, pos);
        pos = push_out_of_actors(self, pos, 1.0);
        let id = self.ecs.spawn(
            ActorKind::Boss,
            crate::actor::Faction::Undead,
            Transform {
                pos,
                yaw: 0.0,
                radius: 1.0,
            },
            Health { hp: 400, max: 400 },
        );
        if let Some(a) = self.ecs.get_mut(id) {
            a.name = Some("Death Knight".to_string());
            let spec = self
                .specs_arche
                .entries
                .get("DeathKnight")
                .cloned()
                .unwrap_or(data_runtime::specs::archetypes::ArchetypeSpec {
                    radius_m: 1.0,
                    move_speed_mps: 2.2,
                    aggro_radius_m: 40.0,
                    attack_radius_m: 0.45,
                    melee_damage: 18,
                    melee_cooldown_s: 0.9,
                });
            a.move_speed = Some(ecs::MoveSpeed {
                mps: spec.move_speed_mps,
            });
            a.aggro = Some(ecs::AggroRadius {
                m: spec.aggro_radius_m,
            });
            a.attack = Some(ecs::AttackRadius {
                m: spec.attack_radius_m,
            });
            a.melee = Some(ecs::Melee {
                damage: spec.melee_damage,
                cooldown_s: spec.melee_cooldown_s,
                ready_in_s: 0.0,
            });
            // Give DK a basic spell kit (Fireball + MagicMissile)
            a.spellbook = Some(ecs::Spellbook {
                known: vec![SpellId::Fireball, SpellId::MagicMissile],
            });
            a.pool = Some(ecs::ResourcePool {
                mana: 40,
                max: 40,
                regen_per_s: 0.3,
                mana_frac: 0.0,
            });
            use std::collections::HashMap;
            a.cooldowns = Some(ecs::Cooldowns {
                gcd_s: 0.40,
                gcd_ready: 0.0,
                per_spell: HashMap::new(),
            });
        }
        id
    }

    // Destructible runtime disabled

    // Destructible runtime disabled
    /// Spawn an NPC wizard (hostile to Undead) for demo or scripted scenes.
    pub fn spawn_wizard_npc(&mut self, pos: Vec3) -> ActorId {
        let mut pos = push_out_of_pc_bubble(self, pos);
        pos = push_out_of_destructibles(self, pos);
        pos = push_out_of_actors(self, pos, 0.7);
        let id = self.ecs.spawn(
            ActorKind::Wizard,
            crate::actor::Faction::Wizards,
            Transform {
                pos,
                yaw: 0.0,
                radius: 0.7,
            },
            Health { hp: 100, max: 100 },
        );
        if let Some(w) = self.ecs.get_mut(id) {
            // Stationary caster by default; give casting resources
            use std::collections::HashMap;
            w.pool = Some(ecs::ResourcePool {
                mana: 30,
                max: 30,
                regen_per_s: 0.5,
                mana_frac: 0.0,
            });
            w.cooldowns = Some(ecs::Cooldowns {
                gcd_s: 0.30,
                gcd_ready: 0.0,
                per_spell: HashMap::new(),
            });
            w.spellbook = Some(ecs::Spellbook {
                known: vec![SpellId::Firebolt, SpellId::Fireball, SpellId::MagicMissile],
            });
            // Apply archetype radius if present
            let spec = self.specs_arche.entries.get("WizardNPC").cloned();
            if let Some(sp) = spec {
                w.tr.radius = sp.radius_m;
            }
            // Clamp to minimum collision radius to avoid tunneling misses
            w.tr.radius = w.tr.radius.max(0.7);
        }
        id
    }
    /// Spawn the unique boss "Nivita, Lady of Undertide" if not present.
    /// Returns the NPC id if spawned or already present.
    pub fn spawn_nivita_unique(&mut self, pos: Vec3) -> Option<ActorId> {
        if let Some(id) = self.nivita_actor_id {
            return Some(id);
        }
        let cfg = match data_runtime::configs::npc_unique::load_nivita() {
            Ok(c) => c,
            Err(e) => {
                log::warn!("server: failed to load nivita config: {e:#}");
                return None;
            }
        };
        let hp_mid = (cfg.hp_range.0 + cfg.hp_range.1) / 2;
        let radius = cfg.radius_m.unwrap_or(0.9);
        // Respect PC safety bubble when placing the boss
        let pos = push_out_of_pc_bubble(self, pos);
        let id = self.ecs.spawn(
            ActorKind::Boss,
            crate::actor::Faction::Undead,
            Transform {
                pos,
                yaw: 0.0,
                radius,
            },
            Health {
                hp: hp_mid,
                max: hp_mid,
            },
        );
        if let Some(a) = self.ecs.get_mut(id) {
            a.name = Some(cfg.name.clone());
            a.move_speed = Some(ecs::MoveSpeed { mps: 2.6 });
            a.aggro = Some(ecs::AggroRadius { m: 35.0 });
            a.attack = Some(ecs::AttackRadius { m: 0.35 });
            a.melee = Some(ecs::Melee {
                damage: 12,
                cooldown_s: 0.8,
                ready_in_s: 0.0,
            });
        }
        // Build and store boss stats snapshot for replication/logging
        let ab = ec::Abilities {
            str: cfg.abilities.str,
            dex: cfg.abilities.dex,
            con: cfg.abilities.con,
            int: cfg.abilities.int,
            wis: cfg.abilities.wis,
            cha: cfg.abilities.cha,
            prof: cfg.abilities.proficiency,
        };
        let mod_of = |v: i8| ((v as i16 - 10) / 2) as i8;
        let saves = if let Some(s) = cfg.saves.as_ref() {
            ec::SavingThrows {
                str_mod: s.str,
                dex_mod: s.dex,
                con_mod: s.con,
                int_mod: s.int,
                wis_mod: s.wis,
                cha_mod: s.cha,
            }
        } else {
            ec::SavingThrows {
                str_mod: mod_of(ab.str),
                dex_mod: mod_of(ab.dex),
                con_mod: mod_of(ab.con),
                int_mod: mod_of(ab.int) + ab.prof,
                wis_mod: mod_of(ab.wis) + ab.prof,
                cha_mod: mod_of(ab.cha) + ab.prof,
            }
        };
        let resist: Vec<ec::DamageType> = cfg
            .defenses
            .resistances
            .iter()
            .filter_map(|s| ecs_core::parse::parse_damage_type(s))
            .collect();
        let immune: Vec<ec::Condition> = cfg
            .defenses
            .immunities
            .iter()
            .filter_map(|s| ecs_core::parse::parse_condition(s))
            .collect();
        let reset = match cfg.legendary.resets.as_deref() {
            Some("per_encounter") => ec::ResetRule::PerEncounter,
            _ => ec::ResetRule::LongRest,
        };
        let lres = ec::LegendaryResist::new(cfg.legendary.resist_per_day, reset);
        let spell_ids = |v: &[String]| v.iter().map(|s| ec::SpellId(s.clone())).collect();
        let book = ec::Spellbook {
            cantrips: spell_ids(&cfg.spellbook.cantrips),
            level_1_3: spell_ids(&cfg.spellbook.level_1_3),
            level_4_5: spell_ids(&cfg.spellbook.level_4_5),
            signature: spell_ids(&cfg.spellbook.signature),
        };
        let team_id = match cfg.team.as_deref() {
            Some("enemy_raid") => Some(2u32),
            Some("players") => Some(1u32),
            _ => None,
        };
        self.nivita_stats = Some(NivitaStats {
            name: cfg.name.clone(),
            ac: i32::from(cfg.defenses.ac),
            abilities: ab,
            saves,
            defenses: ec::Defenses { resist, immune },
            legendary: lres,
            spellbook: book,
            radius,
            height: cfg.height_m.unwrap_or(1.9),
            team: cfg.team.clone(),
            team_id,
        });
        self.nivita_actor_id = Some(id);
        log::info!(
            "server: spawned unique boss '{}' (hp={}..{}, ac={}) as {:?}",
            cfg.name,
            cfg.hp_range.0,
            cfg.hp_range.1,
            cfg.defenses.ac,
            id
        );
        metrics::counter!("boss.nivita.spawns_total").increment(1);
        Some(id)
    }
    /// Lightweight status for UI/replication.
    pub fn nivita_status(&self) -> Option<BossStatus> {
        let id = self.nivita_actor_id?;
        let n = self.ecs.get(id)?;
        let stats = self.nivita_stats.as_ref()?;
        Some(BossStatus {
            name: stats.name.clone(),
            ac: stats.ac,
            hp: n.hp.hp,
            max: n.hp.max,
            pos: n.tr.pos,
        })
    }
    pub fn ring_spawn(&mut self, count: usize, radius: f32, hp: i32) {
        for i in 0..count {
            let a = (i as f32) / (count as f32) * std::f32::consts::TAU;
            let pos = Vec3::new(radius * a.cos(), 0.6, radius * a.sin());
            if is_safe_from_pc(self, pos) {
                if std::env::var("RA_LOG_SPAWNS").ok().as_deref() == Some("1") {
                    log::info!("spawn undead at {:?}", pos);
                }
                let _ = self.spawn_undead(pos, 0.95, hp);
            }
        }
    }
    /// Build a consolidated `TickSnapshot` for clients. Until wizard/projectile state
    /// lives here, we include wizard positions from the caller and compute NPC yaw toward
    /// the nearest wizard.
    // Legacy TickSnapshot removed; actor-centric snapshot is canonical.
    pub fn tick_snapshot_actors(&self, tick: u64) -> net_core::snapshot::ActorSnapshot {
        // Only true actors (non-projectiles) are included in the actor list.
        // Projectiles are carried separately under `projectiles` and must never
        // be represented as NPC spawns to the client.
        let actors: Vec<net_core::snapshot::ActorRep> = self
            .ecs
            .iter()
            .filter(|a| a.projectile.is_none())
            .map(|a| net_core::snapshot::ActorRep {
                id: a.id.0,
                kind: match a.kind {
                    ActorKind::Wizard => 0,
                    ActorKind::Zombie => 1,
                    ActorKind::Boss => 2,
                },
                faction: match a.faction {
                    crate::actor::Faction::Pc => 0,
                    crate::actor::Faction::Wizards => 1,
                    crate::actor::Faction::Undead => 2,
                    crate::actor::Faction::Neutral => 3,
                },
                archetype_id: match a.kind {
                    ActorKind::Wizard => 1,
                    ActorKind::Zombie => 2,
                    ActorKind::Boss => 3,
                },
                name_id: if a.name.is_some() { 1 } else { 0 },
                unique: if Some(a.id) == self.nivita_actor_id {
                    1
                } else {
                    0
                },
                pos: [a.tr.pos.x, a.tr.pos.y, a.tr.pos.z],
                yaw: a.tr.yaw,
                radius: a.tr.radius,
                hp: a.hp.hp,
                max: a.hp.max,
                alive: a.hp.alive(),
            })
            .collect();
        let mut projectiles: Vec<net_core::snapshot::ProjectileRep> = Vec::new();
        for c in self.ecs.iter() {
            if let (Some(proj), Some(vel)) = (c.projectile.as_ref(), c.velocity.as_ref()) {
                projectiles.push(net_core::snapshot::ProjectileRep {
                    id: c.id.0,
                    kind: match proj.kind {
                        ProjKind::Firebolt => 0,
                        ProjKind::Fireball => 1,
                        ProjKind::MagicMissile => 2,
                    },
                    pos: [c.tr.pos.x, c.tr.pos.y, c.tr.pos.z],
                    vel: [vel.v.x, vel.v.y, vel.v.z],
                });
            }
        }
        if std::env::var("RA_LOG_SNAPSHOTS")
            .map(|v| v == "1")
            .unwrap_or(false)
        {
            log::info!(
                "snapshot_v2: tick={} actors={} projectiles={}",
                tick,
                actors.len(),
                projectiles.len()
            );
        }
        net_core::snapshot::ActorSnapshot {
            v: 2,
            tick,
            actors,
            projectiles,
        }
    }

    // clone_for_snapshot removed
    // Move toward nearest wizard and attack when in range. Returns (wizard_idx, damage) per hit.
    /*
        pub fn step_npc_ai(&mut self, _dt: f32, _wizards: &[Vec3]) -> Vec<(usize, i32)> { return Vec::new(); }
    /*
            #[cfg(not(target_arch = "wasm32"))]
            let _t0 = std::time::Instant::now();
            #[cfg(target_arch = "wasm32")]
            let _t0 = web_time::Instant::now();
            if wizards.is_empty() {
                let ms = _t0.elapsed().as_secs_f64() * 1000.0;
                metrics::histogram!("tick.ms").record(ms);
                return Vec::new();
            }
            let wizard_r = 0.7f32;
            let melee_pad = 0.35f32;
            let attack_cd = 1.5f32;
            let attack_anim_time = 0.8f32;
            let mut hits = Vec::new();
            let mut chosen: Vec<Option<usize>> = vec![None; self.npcs.len()];
            for (idx, n) in self.npcs.iter_mut().enumerate() {
                if !n.alive {
                    continue;
                }
                n.attack_cooldown = (n.attack_cooldown - dt).max(0.0);
                n.attack_anim = (n.attack_anim - dt).max(0.0);
                let mut best_i = 0usize;
                let mut best_d2 = f32::INFINITY;
                for (i, w) in wizards.iter().enumerate() {
                    let dx = w.x - n.pos.x;
                    let dz = w.z - n.pos.z;
                    let d2 = dx * dx + dz * dz;
                    if d2 < best_d2 {
                        best_d2 = d2;
                        best_i = i;
                    }
                }
                chosen[idx] = Some(best_i);
                let target = wizards[best_i];
                let to = Vec3::new(target.x - n.pos.x, 0.0, target.z - n.pos.z);
                let dist = to.length();
                let contact = n.radius + wizard_r + melee_pad;
                if dist > contact + 0.02 {
                    let step = (n.speed * dt).min(dist - contact);
                    if step > 1e-4 {
                        n.pos += to.normalize() * step;
                    }
                }
            }
            #[cfg(not(target_arch = "wasm32"))]
            let _c0 = std::time::Instant::now();
            #[cfg(target_arch = "wasm32")]
            let _c0 = web_time::Instant::now();
            self.resolve_collisions(wizards);
            let coll_ms = _c0.elapsed().as_secs_f64() * 1000.0;
            metrics::histogram!("collider.ms").record(coll_ms);
            for (idx, n) in self.npcs.iter_mut().enumerate() {
                if !n.alive {
                    continue;
                }
                if let Some(best_i) = chosen[idx] {
                    let target = wizards[best_i];
                    let to = Vec3::new(target.x - n.pos.x, 0.0, target.z - n.pos.z);
                    let dist = to.length();
                    let contact = n.radius + wizard_r + melee_pad;
                    if dist <= contact + 0.05 && n.attack_cooldown <= 0.0 {
                        hits.push((best_i, n.damage));
                        n.attack_cooldown = attack_cd;
                        n.attack_anim = attack_anim_time;
                    }
                }
            }
            let ms = _t0.elapsed().as_secs_f64() * 1000.0;
            metrics::histogram!("tick.ms").record(ms);
            hits
        }
        fn resolve_collisions(&mut self, _wizards: &[Vec3]) { }
    */
            let nlen = self.npcs.len();
            for i in 0..nlen {
                if !self.npcs[i].alive {
                    continue;
                }
                for j in (i + 1)..nlen {
                    if !self.npcs[j].alive {
                        continue;
                    }
                    let mut dx = self.npcs[j].pos.x - self.npcs[i].pos.x;
                    let mut dz = self.npcs[j].pos.z - self.npcs[i].pos.z;
                    let d2 = dx * dx + dz * dz;
                    let min_d = self.npcs[i].radius + self.npcs[j].radius;
                    if d2 < min_d * min_d {
                        let mut d = d2.sqrt();
                        if d < 1e-4 {
                            dx = 1.0;
                            dz = 0.0;
                            d = 1e-4;
                        }
                        dx /= d;
                        dz /= d;
                        let overlap = min_d - d;
                        let push = overlap * 0.5;
                        self.npcs[i].pos.x -= dx * push;
                        self.npcs[i].pos.z -= dz * push;
                        self.npcs[j].pos.x += dx * push;
                        self.npcs[j].pos.z += dz * push;
                    }
                }
            }
            let wiz_r = 0.7f32;
            for n in &mut self.npcs {
                if !n.alive {
                    continue;
                }
                for w in wizards {
                    let mut dx = n.pos.x - w.x;
                    let mut dz = n.pos.z - w.z;
                    let d2 = dx * dx + dz * dz;
                    let min_d = n.radius + wiz_r;
                    if d2 < min_d * min_d {
                        let mut d = d2.sqrt();
                        if d < 1e-4 {
                            dx = 1.0;
                            dz = 0.0;
                            d = 1e-4;
                        }
                        dx /= d;
                        dz /= d;
                        let overlap = min_d - d;
                        n.pos.x += dx * overlap;
                        n.pos.z += dz * overlap;
                    }
                }
            }
        }
        */
}

const SAFE_SPAWN_RADIUS_M: f32 = 10.0;

fn is_safe_from_pc(srv: &ServerState, pos: Vec3) -> bool {
    if let Some(pc) = srv.pc_actor
        && let Some(pc_c) = srv.ecs.get(pc)
    {
        let dx = pos.x - pc_c.tr.pos.x;
        let dz = pos.z - pc_c.tr.pos.z;
        return dx * dx + dz * dz >= SAFE_SPAWN_RADIUS_M * SAFE_SPAWN_RADIUS_M;
    }
    true
}

fn push_out_of_pc_bubble(srv: &ServerState, mut pos: Vec3) -> Vec3 {
    if let Some(pc) = srv.pc_actor
        && let Some(pc_c) = srv.ecs.get(pc)
    {
        let pcpos = pc_c.tr.pos;
        let dx = pos.x - pcpos.x;
        let dz = pos.z - pcpos.z;
        let d2 = dx * dx + dz * dz;
        let r = SAFE_SPAWN_RADIUS_M;
        if d2 < r * r {
            let mut dir = glam::Vec3::new(dx, 0.0, dz);
            let len = dir.length();
            if len < 1e-3 {
                dir = glam::Vec3::Z;
            } else {
                dir /= len;
            }
            pos = pcpos + dir * r;
        }
    }
    pos
}

// ============================================================================
// Tests – actors only
// ============================================================================

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests_actor {
    use super::*;
    use glam::Vec3;

    #[test]
    fn spawn_from_dir_scales_speed() {
        let mut srv = ServerState::new();
        // enqueue one projectile and run ingest
        srv.pending_projectiles.push(PendingProjectile {
            pos: Vec3::ZERO,
            dir: Vec3::new(0.0, 0.0, 1.0),
            kind: ProjKind::Firebolt,
            owner: None,
        });
        let mut ctx = crate::ecs::schedule::Ctx::default();
        let mut sched = crate::ecs::schedule::Schedule;
        sched.run(&mut srv, &mut ctx);
        // ensure a projectile entity exists with velocity > 20 m/s
        let mut found = false;
        for c in srv.ecs.iter() {
            if let Some(v) = c.velocity.as_ref()
                && v.v.z > 20.0
            {
                found = true;
                break;
            }
        }
        assert!(found, "no projectile with expected speed found");
    }

    #[test]
    fn aoe_hits_wizard_and_undead_and_clamps() {
        let mut s = ServerState::new();
        // PC + one NPC wizard
        s.sync_wizards(&[Vec3::new(0.0, 0.6, 0.0), Vec3::new(5.9, 0.6, 0.0)]);
        // One undead inside radius
        let _z = s.spawn_undead(Vec3::new(0.0, 0.6, 2.0), 0.9, 20);
        // Centered explosion radius 6.0, damage 28
        let hits = s.apply_aoe_at_actors(0.0, 0.0, 36.0, 28, None);
        assert!(hits >= 2, "should hit at least wizard+undead");
        // Check actor hp decreased for those in range
        let mut damaged = 0;
        for a in s.ecs.iter() {
            if (a.tr.pos.x.powi(2) + a.tr.pos.z.powi(2)) <= 36.0 && a.hp.hp < a.hp.max {
                damaged += 1;
            }
        }
        assert!(damaged >= 2);
    }

    #[test]
    fn aoe_boundary_inclusive() {
        let mut s = ServerState::new();
        s.sync_wizards(&[Vec3::new(6.0, 0.6, 0.0), Vec3::new(6.01, 0.6, 0.0)]);
        let _ = s.apply_aoe_at_actors(0.0, 0.0, 36.0, 10, None);
        // Find actors exactly on boundary vs outside
        let on = s
            .ecs
            .iter()
            .find(|a| (a.tr.pos.x - 6.0).abs() < 1e-3)
            .unwrap();
        let out = s
            .ecs
            .iter()
            .find(|a| (a.tr.pos.x - 6.01).abs() < 1e-3)
            .unwrap();
        assert!(on.hp.hp < on.hp.max, "boundary actor should be hit");
        assert_eq!(out.hp.hp, out.hp.max, "outside actor should not be hit");
    }

    #[test]
    fn aoe_skips_dead_targets_and_pc_self() {
        let mut s = ServerState::new();
        s.sync_wizards(&[Vec3::new(0.0, 0.6, 0.0), Vec3::new(0.5, 0.6, 0.5)]);
        // Mark PC dead then apply AoE sourced by PC; PC should not be damaged further
        if let Some(pc) = s.pc_actor
            && let Some(a) = s.ecs.get_mut(pc)
        {
            a.hp.hp = 0;
        }
        let src = s.pc_actor;
        let _ = s.apply_aoe_at_actors(0.0, 0.0, 4.0, 5, src);
        if let Some(pc) = s.pc_actor {
            assert_eq!(s.ecs.get(pc).unwrap().hp.hp, 0);
        }
    }

    #[test]
    fn mana_regen_accumulates_fraction_and_ticks() {
        let mut s = ServerState::new();
        let pc = s.spawn_pc_at(Vec3::new(0.0, 0.6, 0.0));
        {
            let a = s.ecs.get_mut(pc).unwrap();
            let p = a.pool.as_mut().unwrap();
            p.mana = 10;
            p.mana_frac = 0.0;
            p.regen_per_s = 1.0;
        }
        for _ in 0..60 {
            s.step_authoritative(1.0 / 60.0);
        }
        let mana = s.ecs.get(pc).unwrap().pool.as_ref().unwrap().mana;
        assert_eq!(mana, 11, "PC mana should regen by ~1 over one second");
    }
}

/// If `pos` lies inside any registered destructible world AABB, push it outside along the
/// nearest axis by a small margin (0.5 m). Returns possibly adjusted position.
pub fn push_out_of_destructibles(srv: &ServerState, mut pos: Vec3) -> Vec3 {
    let margin = 0.5f32;
    for inst in &srv.destruct_instances {
        let min = glam::Vec3::from(inst.world_min);
        let max = glam::Vec3::from(inst.world_max);
        let inside = pos.x >= min.x
            && pos.x <= max.x
            && pos.y >= min.y
            && pos.y <= max.y
            && pos.z >= min.z
            && pos.z <= max.z;
        if !inside {
            continue;
        }
        // Distances to each face
        let dx_min = (pos.x - min.x).abs();
        let dx_max = (max.x - pos.x).abs();
        let dz_min = (pos.z - min.z).abs();
        let dz_max = (max.z - pos.z).abs();
        // Choose shortest escape
        let (axis, dir) = {
            // Track minimum distance for branch clarity (only used for comparisons)
            #[allow(unused_assignments)]
            let mut best = dx_min;
            let mut axis = 0; // 0=x, 1=z
            let mut dir = -1.0; // -1: toward min, +1: toward max
            if dx_max < best {
                axis = 0;
                dir = 1.0;
                best = dx_max;
            }
            if dz_min < best {
                axis = 1;
                dir = -1.0;
                best = dz_min;
            }
            // no need to update `best` after final branch; we only use `axis`/`dir`
            if dz_max < best {
                axis = 1;
                dir = 1.0;
            }
            (axis, dir)
        };
        match axis {
            0 => {
                // Push along X
                pos.x = if dir < 0.0 {
                    min.x - margin
                } else {
                    max.x + margin
                };
            }
            _ => {
                // Push along Z
                pos.z = if dir < 0.0 {
                    min.z - margin
                } else {
                    max.z + margin
                };
            }
        }
    }
    pos
}

/// Push a spawn position out of nearby actors to avoid stacking. Uses a simple
/// iterative nudge away from the nearest overlapping actor within 2m.
pub fn push_out_of_actors(srv: &ServerState, mut pos: Vec3, radius: f32) -> Vec3 {
    let pad = 0.1f32;
    for _ in 0..8 {
        let mut moved = false;
        for a in srv.ecs.iter() {
            let dx = pos.x - a.tr.pos.x;
            let dz = pos.z - a.tr.pos.z;
            let d2 = dx * dx + dz * dz;
            let need = radius + a.tr.radius + pad;
            if d2 < need * need {
                let d = d2.sqrt().max(1e-3);
                let dir = glam::vec3(dx / d, 0.0, dz / d);
                let push = need - d;
                pos += dir * push;
                moved = true;
            }
        }
        if !moved {
            break;
        }
    }
    pos
}
