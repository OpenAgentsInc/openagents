//! Portable adapter over the retained Ruins of Atlantis Wizard Woods simulation.
//!
//! Gameplay runs in the original server_core ECS schedule. This adapter owns
//! validated host inputs and finite snapshots, not a replacement combat system.

use glam::Vec3;
use serde::Serialize;
use server_core::{ActorKind, Faction, ServerState, SpellId};

pub mod scene;

pub use client_core::controller::PlayerController as Controller;
pub use client_core::input::InputState as MovementInput;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Spell {
    Firebolt,
    MagicMissile,
    Fireball,
}
impl Spell {
    pub const ALL: [Self; 3] = [Self::Firebolt, Self::MagicMissile, Self::Fireball];
    fn source(self) -> SpellId {
        match self {
            Self::Firebolt => SpellId::Firebolt,
            Self::MagicMissile => SpellId::MagicMissile,
            Self::Fireball => SpellId::Fireball,
        }
    }
    fn label(self) -> &'static str {
        match self {
            Self::Firebolt => "Firebolt",
            Self::MagicMissile => "Magic missile",
            Self::Fireball => "Fireball",
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct Player {
    pub hp: i32,
    pub max_hp: i32,
    pub mana: i32,
    pub max_mana: i32,
}
#[derive(Clone, Debug, Serialize)]
pub struct Ability {
    pub id: Spell,
    pub label: &'static str,
    pub cost: i32,
    pub ready: bool,
    pub cooldown_remaining: f32,
}
#[derive(Clone, Debug, Serialize)]
pub struct Actor {
    pub id: u32,
    pub kind: &'static str,
    pub faction: &'static str,
    pub pos: [f32; 3],
    pub yaw: f32,
    pub hp: i32,
    pub max_hp: i32,
    pub alive: bool,
}
#[derive(Clone, Debug, Serialize)]
pub struct Projectile {
    pub id: u32,
    pub kind: Spell,
    pub pos: [f32; 3],
    pub vel: [f32; 3],
}
#[derive(Clone, Debug, Serialize)]
pub struct Effect {
    pub kind: u8,
    pub pos: [f32; 3],
}
#[derive(Clone, Debug, Default, Serialize)]
pub struct Counters {
    /// Valid player input requests, including requests later refused by source gates.
    pub casts: u64,
    /// Newly observed projectiles from all casters, including NPCs.
    pub projectiles: u64,
    /// Original source impact events, rather than inferred damage or kills.
    pub hits: u64,
}

/// Original world-space destructible mesh. Kept outside mobile state packets.
#[derive(Clone, Debug)]
pub struct RuinChunk {
    pub did: u64,
    pub chunk: [u32; 3],
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub indices: Vec<u32>,
}
#[derive(Clone, Debug, Serialize)]
pub struct Snapshot {
    pub elapsed: f32,
    pub player: Player,
    pub abilities: Vec<Ability>,
    pub actors: Vec<Actor>,
    pub projectiles: Vec<Projectile>,
    pub effects: Vec<Effect>,
    pub counters: Counters,
}

#[derive(Debug)]
pub struct Simulation {
    source: ServerState,
    elapsed: f32,
    effects: Vec<Effect>,
    counters: Counters,
    seen_projectiles: std::collections::HashSet<u32>,
    ruins: Vec<RuinChunk>,
    ruin_revision: u64,
}
impl Simulation {
    pub fn new(player_position: [f32; 3]) -> Result<Self, String> {
        validate_position(player_position)?;
        let mut source = ServerState::new();
        source.spawn_pc_at(Vec3::from(player_position));
        server_core::zones::boot_with_zone(&mut source, "wizard_woods");
        Ok(Self {
            source,
            elapsed: 0.0,
            effects: Vec::new(),
            counters: Counters::default(),
            seen_projectiles: std::collections::HashSet::new(),
            ruins: Vec::new(),
            ruin_revision: 0,
        })
    }

    /// Starts the retained combat schedule with caller-authored stationary hostiles.
    pub fn chamber(
        player_position: [f32; 3],
        hostiles: &[([f32; 3], i32)],
    ) -> Result<(Self, Vec<u32>), String> {
        validate_position(player_position)?;
        if hostiles.len() > 256 {
            return Err("Too many chamber actors".into());
        }
        let mut source = ServerState::new();
        source.spawn_pc_at(player_position.into());
        let mut ids = Vec::new();
        for (position, hp) in hostiles {
            validate_position(*position)?;
            if !(1..=1_000_000).contains(hp) {
                return Err("Invalid chamber health".into());
            }
            let id = source.ecs.spawn(
                ActorKind::Wizard,
                Faction::Undead,
                server_core::Transform {
                    pos: (*position).into(),
                    yaw: 0.0,
                    radius: 0.65,
                },
                server_core::Health { hp: *hp, max: *hp },
            );
            ids.push(id.0);
        }
        Ok((
            Self {
                source,
                elapsed: 0.0,
                effects: vec![],
                counters: Counters::default(),
                seen_projectiles: Default::default(),
                ruins: vec![],
                ruin_revision: 0,
            },
            ids,
        ))
    }
    pub fn place_chamber_actor(
        &mut self,
        id: u32,
        position: [f32; 3],
        yaw: f32,
    ) -> Result<(), String> {
        validate_position(position)?;
        if !yaw.is_finite() {
            return Err("Invalid chamber facing".into());
        }
        let actor = self
            .source
            .ecs
            .get_mut(server_core::ActorId(id))
            .ok_or("Unknown chamber actor")?;
        actor.tr.pos = position.into();
        actor.tr.yaw = yaw;
        Ok(())
    }
    /// Applies a directed cinematic bow impact to the same health state as spells.
    pub fn bow_impact(&mut self, id: u32, damage: i32) -> Result<(), String> {
        if !(1..=10000).contains(&damage) {
            return Err("Invalid bow damage".into());
        }
        let actor = self
            .source
            .ecs
            .get_mut(server_core::ActorId(id))
            .ok_or("Unknown bow target")?;
        actor.hp.hp = actor.hp.hp.saturating_sub(damage).max(0);
        Ok(())
    }

    pub fn tick(&mut self, dt: f32, player_position: [f32; 3], yaw: f32) -> Result<(), String> {
        validate_position(player_position)?;
        if !dt.is_finite() || !(0.0..=0.1).contains(&dt) || !yaw.is_finite() {
            return Err("Invalid Ruins frame input".into());
        }
        if let Some(id) = self.source.pc_actor
            && let Some(pc) = self.source.ecs.get_mut(id)
        {
            pc.tr.pos = Vec3::from(player_position);
            pc.tr.yaw = yaw;
        }
        self.source.step_authoritative(dt);
        self.elapsed += dt;
        self.effects = self
            .source
            .fx_hits
            .drain(..)
            .map(|hit| Effect {
                kind: hit.kind,
                pos: hit.pos,
            })
            .collect();
        self.counters.hits = self.counters.hits.saturating_add(self.effects.len() as u64);
        self.source.hud_toasts.clear();
        let projectiles: std::collections::HashSet<_> = self
            .source
            .ecs
            .iter()
            .filter(|actor| actor.projectile.is_some())
            .map(|actor| actor.id.0)
            .collect();
        self.counters.projectiles = self
            .counters
            .projectiles
            .saturating_add(projectiles.difference(&self.seen_projectiles).count() as u64);
        self.seen_projectiles = projectiles;
        for mesh in self.source.drain_destruct_mesh_deltas() {
            let chunk = [mesh.chunk.0, mesh.chunk.1, mesh.chunk.2];
            self.ruins
                .retain(|old| old.did != mesh.did || old.chunk != chunk);
            if !mesh.indices.is_empty() {
                self.ruins.push(RuinChunk {
                    did: mesh.did,
                    chunk,
                    positions: mesh.positions,
                    normals: mesh.normals,
                    indices: mesh.indices,
                });
            }
            self.ruin_revision = self.ruin_revision.saturating_add(1);
        }
        Ok(())
    }

    /// Current original voxel meshes, updated under the source's per-tick budget.
    pub fn ruins(&self) -> &[RuinChunk] {
        &self.ruins
    }

    pub fn ruin_revision(&self) -> u64 {
        self.ruin_revision
    }

    /// Test the current source voxel grid at a rendered world position.
    ///
    /// This applies the source proxy transform, including its retained origin
    /// convention. It does not replace the original controller or slide solver.
    pub fn solid_at(&self, position: [f32; 3]) -> bool {
        if validate_position(position).is_err() {
            return false;
        }
        self.source.destruct_registry.proxies.values().any(|proxy| {
            let object = proxy
                .object_from_world
                .transform_point3(Vec3::from(position));
            let voxel = (object.as_dvec3() - proxy.grid.origin_m()) / proxy.grid.voxel_m().0;
            let dims = proxy.grid.dims().as_dvec3();
            voxel.cmpge(glam::DVec3::ZERO).all()
                && voxel.cmplt(dims).all()
                && proxy
                    .grid
                    .is_solid(voxel.x as u32, voxel.y as u32, voxel.z as u32)
        })
    }

    pub fn cast(
        &mut self,
        spell: Spell,
        origin: [f32; 3],
        direction: [f32; 3],
    ) -> Result<(), String> {
        validate_position(origin)?;
        let direction = Vec3::from(direction);
        if !direction.is_finite()
            || !direction.length_squared().is_finite()
            || direction.length_squared() < 1e-6
        {
            return Err("Invalid Ruins cast direction".into());
        }
        if !self
            .source
            .pc_actor
            .and_then(|id| self.source.ecs.get(id))
            .is_some_and(|pc| pc.hp.alive())
        {
            return Err("The Ruins player is defeated".into());
        }
        if self.source.pending_casts.len() >= 16 {
            return Err("Wait for the next Ruins frame".into());
        }
        self.source
            .enqueue_cast(Vec3::from(origin), direction.normalize(), spell.source());
        self.counters.casts = self.counters.casts.saturating_add(1);
        Ok(())
    }

    pub fn snapshot(&self) -> Snapshot {
        let pc = self.source.pc_actor.and_then(|id| self.source.ecs.get(id));
        let player = pc.map_or(
            Player {
                hp: 0,
                max_hp: 100,
                mana: 0,
                max_mana: 20,
            },
            |actor| Player {
                hp: actor.hp.hp,
                max_hp: actor.hp.max,
                mana: actor.pool.as_ref().map_or(0, |pool| pool.mana),
                max_mana: actor.pool.as_ref().map_or(0, |pool| pool.max),
            },
        );
        let abilities = Spell::ALL
            .into_iter()
            .map(|id| {
                let cost = match id {
                    Spell::Firebolt => self.source.specs.spells.firebolt.cost,
                    Spell::MagicMissile => self.source.specs.spells.magic_missile.cost,
                    Spell::Fireball => self.source.specs.spells.fireball.cost,
                };
                let cooldown_remaining =
                    pc.and_then(|pc| pc.cooldowns.as_ref()).map_or(0.0, |cd| {
                        cd.gcd_ready
                            .max(cd.per_spell.get(&id.source()).copied().unwrap_or(0.0))
                    });
                let ready = pc.is_some_and(|pc| pc.hp.alive() && pc.stunned.is_none())
                    && player.mana >= cost
                    && cooldown_remaining <= 0.0;
                Ability {
                    id,
                    label: id.label(),
                    cost,
                    ready,
                    cooldown_remaining,
                }
            })
            .collect();
        let actors = self
            .source
            .ecs
            .iter()
            .filter(|a| a.projectile.is_none())
            .map(|a| Actor {
                id: a.id.0,
                kind: match a.kind {
                    ActorKind::Wizard => "wizard",
                    ActorKind::Zombie => "zombie",
                    ActorKind::Boss => "boss",
                },
                faction: match a.faction {
                    Faction::Pc => "player",
                    Faction::Wizards => "wizards",
                    Faction::Undead => "undead",
                    Faction::Neutral => "neutral",
                },
                pos: a.tr.pos.to_array(),
                yaw: a.tr.yaw,
                hp: a.hp.hp,
                max_hp: a.hp.max,
                alive: a.hp.alive(),
            })
            .collect();
        let projectiles = self
            .source
            .ecs
            .iter()
            .filter_map(|a| {
                a.projectile.as_ref().map(|p| Projectile {
                    id: a.id.0,
                    kind: match p.kind {
                        server_core::ProjKind::Firebolt => Spell::Firebolt,
                        server_core::ProjKind::Fireball => Spell::Fireball,
                        server_core::ProjKind::MagicMissile => Spell::MagicMissile,
                    },
                    pos: a.tr.pos.to_array(),
                    vel: a.velocity.as_ref().map_or([0.0; 3], |v| v.v.to_array()),
                })
            })
            .collect();
        Snapshot {
            elapsed: self.elapsed,
            player,
            abilities,
            actors,
            projectiles,
            effects: self.effects.clone(),
            counters: self.counters.clone(),
        }
    }
}

fn validate_position(position: [f32; 3]) -> Result<(), String> {
    if !position.iter().all(|v| v.is_finite() && v.abs() <= 10000.0) {
        return Err("Invalid Ruins world position".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty() -> Simulation {
        let mut simulation = Simulation::new([0.0, 0.6, 0.0]).unwrap();
        let mut source = ServerState::new();
        source.spawn_pc_at(Vec3::new(0.0, 0.6, 0.0));
        simulation.source = source;
        simulation
    }

    #[test]
    fn boot_retains_original_population_and_stats() {
        let simulation = Simulation::new([0.0, 0.6, 0.0]).unwrap();
        let snapshot = simulation.snapshot();
        assert_eq!(snapshot.actors.len(), 42);
        assert_eq!(
            snapshot
                .actors
                .iter()
                .filter(|a| a.kind == "zombie")
                .count(),
            35
        );
        assert_eq!(
            snapshot.actors.iter().filter(|a| a.kind == "boss").count(),
            2
        );
        assert_eq!(snapshot.player.hp, 100);
        assert_eq!(snapshot.player.mana, 20);
        assert_eq!(
            snapshot
                .abilities
                .iter()
                .map(|a| a.cost)
                .collect::<Vec<_>>(),
            [0, 2, 5]
        );
        assert!(snapshot.abilities.iter().all(|a| a.ready));
    }

    #[test]
    fn original_npcs_move_and_cast_without_player_requests() {
        let pos = [0.0, 0.6, 0.0];
        let mut simulation = Simulation::new(pos).unwrap();
        let before = simulation.snapshot();
        for _ in 0..300 {
            simulation.tick(0.016, pos, 0.0).unwrap();
        }
        let after = simulation.snapshot();
        assert_eq!(after.counters.casts, 0);
        assert!(after.counters.projectiles > 0);
        assert!(after.actors.iter().any(|a| a.faction == "undead"
            && before.actors.iter().any(|b| b.id == a.id && b.pos != a.pos)));
        assert!(serde_json::to_vec(&after).unwrap().len() < 64 * 1024);
    }

    #[test]
    fn fireball_uses_source_mana_cooldown_and_damage() {
        let mut simulation = empty();
        let target = simulation.source.ecs.spawn(
            ActorKind::Wizard,
            Faction::Wizards,
            server_core::Transform {
                pos: Vec3::new(0.0, 0.6, 6.0),
                yaw: 0.0,
                radius: 0.7,
            },
            server_core::Health { hp: 100, max: 100 },
        );
        simulation
            .cast(Spell::Fireball, [0.0, 0.6, 0.0], [0.0, 0.0, 1.0])
            .unwrap();
        simulation.tick(0.016, [0.0, 0.6, 0.0], 0.0).unwrap();
        let first = simulation.snapshot();
        assert_eq!(first.player.mana, 15);
        assert!(!first.abilities[2].ready);
        assert!(first.abilities[2].cooldown_remaining > 1.9);
        assert_eq!(first.counters.projectiles, 1);
        simulation
            .cast(Spell::Fireball, [0.0, 0.6, 0.0], [0.0, 0.0, 1.0])
            .unwrap();
        simulation.tick(0.016, [0.0, 0.6, 0.0], 0.0).unwrap();
        assert_eq!(simulation.snapshot().counters.projectiles, 1);
        assert_eq!(simulation.snapshot().player.mana, 15);
        for _ in 0..60 {
            simulation.tick(0.016, [0.0, 0.6, 0.0], 0.0).unwrap();
        }
        assert!(
            simulation
                .source
                .ecs
                .get(target)
                .is_none_or(|actor| actor.hp.hp < 100)
        );
    }

    #[test]
    fn terrain_height_cast_hits_source_placeholder_height_via_xz_collision() {
        let mut simulation = empty();
        let target = simulation.source.ecs.spawn(
            ActorKind::Wizard,
            Faction::Wizards,
            server_core::Transform {
                pos: Vec3::new(0.0, 0.6, 6.0),
                yaw: 0.0,
                radius: 0.7,
            },
            server_core::Health { hp: 100, max: 100 },
        );
        let height = scene::Terrain::bundled().height(0.0, 0.0);
        simulation
            .cast(Spell::Fireball, [0.0, height + 1.4, 0.25], [0.0, 0.0, 1.0])
            .unwrap();
        for _ in 0..60 {
            simulation.tick(0.016, [0.0, height, 0.0], 0.0).unwrap();
        }
        assert!(
            simulation
                .source
                .ecs
                .get(target)
                .is_none_or(|actor| actor.hp.hp < 100)
        );
    }

    #[test]
    fn magic_missile_retains_three_projectiles() {
        let mut simulation = empty();
        simulation
            .cast(Spell::MagicMissile, [0.0, 0.6, 0.0], [0.0, 0.0, 1.0])
            .unwrap();
        simulation.tick(0.016, [0.0, 0.6, 0.0], 0.0).unwrap();
        let snapshot = simulation.snapshot();
        assert_eq!(snapshot.player.mana, 18);
        assert_eq!(snapshot.projectiles.len(), 3);
        assert!(
            snapshot
                .projectiles
                .iter()
                .all(|p| p.kind == Spell::MagicMissile)
        );
    }

    #[test]
    fn invalid_inputs_do_not_mutate_the_simulation() {
        let mut simulation = empty();
        let before = serde_json::to_vec(&simulation.snapshot()).unwrap();
        assert!(simulation.tick(f32::NAN, [0.0; 3], 0.0).is_err());
        assert!(simulation.tick(0.2, [0.0; 3], 0.0).is_err());
        assert!(
            simulation
                .tick(0.016, [f32::INFINITY, 0.0, 0.0], 0.0)
                .is_err()
        );
        assert!(
            simulation
                .cast(Spell::Fireball, [0.0; 3], [0.0; 3])
                .is_err()
        );
        assert!(
            simulation
                .cast(Spell::Fireball, [0.0; 3], [f32::MAX; 3])
                .is_err()
        );
        assert_eq!(before, serde_json::to_vec(&simulation.snapshot()).unwrap());
        for _ in 0..16 {
            simulation
                .cast(Spell::Fireball, [0.0; 3], [0.0, 0.0, 1.0])
                .unwrap();
        }
        assert!(
            simulation
                .cast(Spell::Fireball, [0.0; 3], [0.0, 0.0, 1.0])
                .is_err()
        );
    }

    #[test]
    fn player_death_does_not_trigger_legacy_auto_respawn() {
        let mut simulation = empty();
        let player = simulation.source.pc_actor.unwrap();
        simulation.source.ecs.get_mut(player).unwrap().hp.hp = 0;
        assert!(
            simulation
                .cast(Spell::Fireball, [0.0; 3], [0.0, 0.0, 1.0])
                .is_err()
        );
        for _ in 0..140 {
            simulation.tick(0.016, [0.0, 0.6, 0.0], 0.0).unwrap();
        }
        assert!(simulation.source.ecs.get(player).is_none());
        assert!(
            simulation
                .cast(Spell::Fireball, [0.0; 3], [0.0, 0.0, 1.0])
                .is_err()
        );
        assert_eq!(simulation.snapshot().player.hp, 0);
        assert!(simulation.snapshot().abilities.iter().all(|a| !a.ready));
    }

    #[test]
    fn ruin_chunks_are_original_finite_meshes_under_source_budget() {
        let mut simulation = Simulation::new([0.0, 0.6, 0.0]).unwrap();
        assert_eq!(simulation.ruin_revision(), 0);
        for _ in 0..8 {
            simulation.tick(0.016, [0.0, 0.6, 0.0], 0.0).unwrap();
        }
        assert!(simulation.ruin_revision() > 0);
        assert!(!simulation.ruins().is_empty());
        for mesh in simulation.ruins() {
            assert_eq!(mesh.positions.len(), mesh.normals.len());
            assert_eq!(mesh.indices.len() % 3, 0);
            assert!(mesh.positions.iter().flatten().all(|v| v.is_finite()));
            assert!(
                mesh.indices
                    .iter()
                    .all(|i| (*i as usize) < mesh.positions.len())
            );
        }
        assert!(!simulation.solid_at([100.0; 3]));
        assert!(!simulation.solid_at([f32::NAN; 3]));
        assert!(
            (-32..32).any(|x| (0..16).any(|y| (0..64).any(|z| simulation.solid_at([
                x as f32 * 0.5 + 0.25,
                y as f32 * 0.5 + 0.25,
                z as f32 * 0.5 + 0.25
            ]))))
        );
    }

    #[test]
    fn controller_uses_original_run_sprint_and_jump_constants() {
        let mut controller = Controller::new(Vec3::ZERO);
        controller.update(
            &MovementInput {
                forward: true,
                ..MovementInput::default()
            },
            0.1,
            Vec3::Z,
        );
        assert!((controller.pos.z - 0.64008).abs() < 0.00001);
        controller.pos = Vec3::ZERO;
        controller.update(
            &MovementInput {
                forward: true,
                run: true,
                jump_pressed: true,
                ..MovementInput::default()
            },
            0.1,
            Vec3::Z,
        );
        assert!((controller.pos.z - 0.832104).abs() < 0.00001);
        assert!((controller.pos.y - 0.46).abs() < 0.00001);
        assert!(controller.airborne());
        controller.update(&MovementInput::default(), 0.1, Vec3::Z);
        assert!((controller.pos.y - (0.46 + (4.6 - 9.81 * 0.1) * 0.1)).abs() < 0.00001);
    }
}
