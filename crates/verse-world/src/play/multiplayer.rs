//! Controlled adventurers sharing one chamber authority and physics clock.
use super::*;
use crate::{Admission, Command, Controller, Intent};
use verse_engine::core::LifeId;
/// How far from an obstructed spawn a respawn may stand, in meters.
pub const RESPAWN_RADIUS: f64 = 3.;

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct Player {
    pub(crate) admission: Admission,
    #[serde(default)]
    pub(crate) source: u32,
    #[serde(default)]
    pub definition: crate::content::Character,
    pub(crate) character: physics::character::Character,
    #[serde(alias = "position")]
    pub player: Vec3,
    #[serde(alias = "previous")]
    pub(crate) previous_player: Vec3,
    #[serde(default)]
    pub(crate) spawn: Vec3,
    pub yaw: f32,
    #[serde(default)]
    pub selected: u64,
    #[serde(default)]
    pub(crate) catalog_ready: BTreeMap<u8, f32>,
    #[serde(alias = "pending_move")]
    pub(crate) pending_movement: Option<[f32; 2]>,
    #[serde(default, alias = "held_move")]
    pub(crate) held_movement: crate::movement::Held,
    pub(crate) pending_jump: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) frame_clock: Option<crate::movement::frames::Clock>,
    pub controls: Controls,
    pub casting: Option<Casting>,
    pub bow_ready: f32,
    pub last_cast: Option<(Ability, f32)>,
    pub(crate) motion_clock: f32,
    pub(crate) locomotion: [f32; 2],
    pub moving: bool,
    #[serde(default)]
    pub(crate) died_at: Option<f32>,
    #[serde(skip)]
    pub(crate) player_trajectory: Vec<[f32; 3]>,
}
impl Player {
    pub(super) fn new(admission: Admission, source: u32, spawn: Vec3) -> Self {
        Self {
            admission,
            source,
            definition: Default::default(),
            character: physics::character::Character::new(spawn.as_dvec3()),
            player: spawn,
            previous_player: spawn,
            spawn,
            yaw: std::f32::consts::PI,
            selected: 0,
            catalog_ready: BTreeMap::new(),
            pending_movement: None,
            held_movement: Default::default(),
            pending_jump: false,
            frame_clock: None,
            controls: Controls::default(),
            casting: None,
            bow_ready: 0.,
            last_cast: None,
            motion_clock: 0.,
            locomotion: [0.; 2],
            moving: false,
            died_at: None,
            player_trajectory: vec![],
        }
    }
    pub(super) fn frame(
        &self,
        frame: &mut verse_engine::director::ActorFrame,
        hp: u32,
        time: f32,
        collision: bool,
    ) {
        frame.actor.position = self.player;
        frame.actor.yaw = self.yaw;
        frame.life = Some(self.admission.actor());
        frame.health = hp;
        frame.animation_time = if self.moving { self.motion_clock } else { time };
        let state = if hp == 0 {
            frame.animation_time = self.died_at.map_or(0.8, |at| time - at);
            State::Death
        } else if self.moving {
            if self.locomotion[0].abs() > self.locomotion[1].abs() {
                if self.locomotion[0] < 0. {
                    State::StrafeLeft
                } else {
                    State::StrafeRight
                }
            } else if self.locomotion[1] < 0. {
                State::Backpedal
            } else {
                State::Run
            }
        } else if self
            .last_cast
            .is_some_and(|(a, t)| a == Ability::Bow && time - t < BOW_STANCE)
        {
            State::BowReady
        } else {
            State::CombatReady
        };
        frame.animation = state.into();
        if hp == 0 {
            return;
        }
        if collision && self.character.support.is_none() {
            frame.animation = State::Airborne.into();
            frame.animation_time = 0.2;
        }
        if let Some(c) = &self.casting {
            frame.animation = State::Cast.into();
            frame.animation_time = time - c.started;
        }
        if let Some((a, t)) = self.last_cast {
            if time - t < 1. {
                frame.animation = if a == Ability::Bow {
                    State::BowRelease
                } else {
                    State::SpellRelease
                }
                .into();
                frame.animation_time = time - t;
            }
        }
    }
}
#[derive(Clone)]
pub(crate) struct PortablePlayer {
    definition: crate::content::Character,
    catalog_remaining: BTreeMap<u8, f32>,
    dice: Option<crate::spells::Dice>,
    combat: crate::rules::transfer::Portable,
    controls: Controls,
    bow_remaining: f32,
    time: f32,
    yaw: f32,
    appearance: verse_engine::director::Actor,
}
impl Game {
    pub(crate) fn take_transfer_player(&mut self, actor: u64) -> Result<PortablePlayer, String> {
        let source = self
            .player_source(actor)
            .ok_or("Transfer character is missing")?;
        if self.simulation.private_snapshot(source)?.player.hp == 0 {
            return Err("Defeated characters cannot transfer".into());
        }
        self.take_resident_player(actor)
    }
    pub(crate) fn take_resident_player(&mut self, actor: u64) -> Result<PortablePlayer, String> {
        if actor == self.player_actor() {
            return self.take_primary_player();
        }
        let player = self
            .additional_players
            .get(&actor)
            .ok_or("Resident character is missing")?;
        let appearance = self
            .scene
            .actors
            .iter()
            .find(|a| a.id == actor)
            .cloned()
            .ok_or("Transfer appearance is missing")?;
        let portable = PortablePlayer {
            definition: player.definition.clone(),
            catalog_remaining: player
                .catalog_ready
                .iter()
                .map(|(slot, at)| (*slot, (*at - self.time).max(0.)))
                .collect(),
            dice: self.spells.caster_dice.get(&actor).cloned(),
            combat: self.simulation.take_player(player.source)?,
            controls: player.controls.clone(),
            bow_remaining: (player.bow_ready - self.time).max(0.),
            time: self.time,
            yaw: player.yaw,
            appearance,
        };
        let life = player.admission.actor();
        let source = player.source;
        self.spells.end_concentration(actor)?;
        let casts: Vec<_> = self
            .spells
            .owned
            .iter()
            .filter(|o| o.caster == actor)
            .map(|o| o.cast)
            .collect();
        for cast in casts {
            self.spells.end_cast(cast)?;
        }
        self.primary.controls.forget_actor(source);
        for p in self.additional_players.values_mut() {
            p.controls.forget_actor(source);
        }
        self.clear_social_seat(life);
        if let Some(encounter) = &mut self.encounter {
            encounter.casts.retain(|c| c.target_life != life);
        }
        self.additional_players.remove(&actor);
        self.prune_caster_dice();
        self.observed_health.remove(&source);
        self.scene.actors.retain(|a| a.id != actor);
        let physical = physics::queries::Life {
            instance: life.instance,
            entity: life.actor,
            generation: life.generation,
        };
        self.bodies.retire_actor(physical)?;
        self.blockers.remove(physical)?;
        self.sync_bodies(0.)?;
        self.checkpoint()?;
        Ok(portable)
    }
    fn take_primary_player(&mut self) -> Result<PortablePlayer, String> {
        if !self.primary_resident {
            return Err("Primary character is absent".into());
        }
        let life = self.player_life();
        let appearance = self
            .scene
            .actors
            .iter()
            .find(|a| a.id == life.actor)
            .cloned()
            .ok_or("Primary appearance is missing")?;
        let portable = PortablePlayer {
            definition: self.primary.definition.clone(),
            catalog_remaining: self
                .primary
                .catalog_ready
                .iter()
                .map(|(slot, at)| (*slot, (*at - self.time).max(0.)))
                .collect(),
            dice: Some(self.spells.dice.clone()),
            combat: self.simulation.take_player(0)?,
            controls: self.primary.controls.clone(),
            bow_remaining: (self.primary.bow_ready - self.time).max(0.),
            time: self.time,
            yaw: self.primary.yaw,
            appearance,
        };
        self.spells.end_concentration(life.actor)?;
        let casts: Vec<_> = self
            .spells
            .owned
            .iter()
            .filter(|o| o.caster == life.actor)
            .map(|o| o.cast)
            .collect();
        for cast in casts {
            self.spells.end_cast(cast)?;
        }
        for player in self.additional_players.values_mut() {
            player.controls.forget_actor(0);
        }
        self.clear_social_seat(life);
        if let Some(encounter) = &mut self.encounter {
            encounter.casts.retain(|c| c.target_life != life);
        }
        self.control_handoff(false)?;
        self.primary.controls = Controls::default();
        self.primary.casting = None;
        self.primary.last_cast = None;
        self.primary.bow_ready = 0.;
        self.observed_health.remove(&0);
        self.primary_resident = false;
        let physical = physics::queries::Life {
            instance: life.instance,
            entity: life.actor,
            generation: life.generation,
        };
        self.bodies.retire_actor(physical)?;
        self.blockers.remove(physical)?;
        self.sync_bodies(0.)?;
        self.checkpoint()?;
        Ok(portable)
    }
    pub(crate) fn put_transfer_player(
        &mut self,
        life: LifeId,
        portable: PortablePlayer,
    ) -> Result<(), String> {
        let p = self
            .additional_players
            .get_mut(&life.actor)
            .ok_or("Destination transfer player is missing")?;
        if p.admission.actor() != life {
            return Err("Destination transfer life is stale".into());
        }
        portable.definition.validate()?;
        p.definition = portable.definition;
        p.catalog_ready = portable
            .catalog_remaining
            .into_iter()
            .map(|(slot, remaining)| (slot, self.time + remaining))
            .collect();
        if let Some(dice) = portable.dice {
            self.spells.caster_dice.insert(life.actor, dice);
        }
        p.controls = portable
            .controls
            .transfer_cooldowns(portable.time, self.time);
        p.bow_ready = self.time + portable.bow_remaining;
        p.yaw = portable.yaw;
        let dead = portable.combat.defeated();
        p.died_at = dead.then_some(self.time);
        self.simulation.put_player(p.source, portable.combat)?;
        self.simulation
            .teleport_chamber_actor(p.source, p.player.to_array(), p.yaw)?;
        let appearance = self
            .scene
            .actors
            .iter_mut()
            .find(|a| a.id == life.actor)
            .ok_or("Destination transfer appearance is missing")?;
        let mut moved = portable.appearance;
        moved.id = life.actor;
        moved.position = p.player;
        moved.yaw = p.yaw;
        *appearance = moved;
        self.sync_bodies(0.)?;
        self.checkpoint()?;
        Ok(())
    }
}
impl Game {
    pub fn movement_baseline(
        &self,
        life: LifeId,
    ) -> Result<Option<crate::movement::Baseline>, String> {
        let (admission, character, yaw, pending, dead) = if life.actor == self.player_actor() {
            (
                &self.primary.admission,
                self.primary.character,
                self.primary.yaw,
                self.primary.pending_movement.is_some()
                    || self.primary.pending_jump
                    || self.agent_controlled,
                self.snapshot().player.hp == 0,
            )
        } else {
            let player = self
                .additional_players
                .get(&life.actor)
                .ok_or("Unknown movement character")?;
            (
                &player.admission,
                player.character,
                player.yaw,
                player.pending_movement.is_some() || player.pending_jump,
                self.simulation.snapshot_for(player.source)?.player.hp == 0,
            )
        };
        if admission.actor() != life {
            return Err("Movement baseline life mismatch".into());
        }
        if !self.unlocked() || pending || dead || self.colliders.is_empty() {
            return Ok(None);
        }
        let held = if life.actor == self.player_actor() {
            self.primary.held_movement
        } else {
            self.additional_players[&life.actor].held_movement
        };
        let target = crate::spells::Target::Actor(life.actor);
        let held_by_spell = self
            .spells
            .telekinesis
            .iter()
            .any(|effect| effect.target == target && effect.grip.grip.is_some())
            || self
                .spells
                .proxies
                .iter()
                .any(|proxy| proxy.actor == life.actor && proxy.held && !proxy.ended);
        let levitated = self
            .spells
            .levitations
            .iter()
            .any(|effect| effect.target == target && effect.state.holding());
        let policy = crate::movement::Policy {
            walking_scale: if held_by_spell || levitated {
                0.
            } else {
                crate::spells::black_tentacles::speed_scale(
                    &self.spells,
                    life.actor,
                    character.feet,
                ) as f32
            },
            jump_allowed: !(held_by_spell || levitated),
        };
        let clock = if life.actor == self.player_actor() {
            self.primary.frame_clock.as_ref()
        } else {
            self.additional_players[&life.actor].frame_clock.as_ref()
        };
        let baseline = crate::movement::Baseline {
            profile: if clock.is_some() {
                crate::movement::Profile::Frames
            } else {
                crate::movement::Profile::Arrival
            },
            world_step: self.physics_steps,
            life,
            epoch: admission.epoch(),
            applied_sequence: clock.map_or(admission.accepted_sequence(), |clock| {
                clock.applied_sequence
            }),
            physics_step: clock.map_or(self.physics_steps, |clock| clock.step),
            held,
            policy,
            character,
            yaw,
        };
        baseline.validate()?;
        Ok(Some(baseline))
    }

    pub(crate) fn account_spawn(&self) -> Result<Vec3, String> {
        let origin = self
            .scene
            .actors
            .iter()
            .find(|a| a.id == self.player_actor())
            .ok_or("Authored realm spawn is missing")?
            .position;
        let directions = [
            Vec3::X,
            -Vec3::X,
            Vec3::Z,
            -Vec3::Z,
            Vec3::new(1., 0., 1.).normalize(),
            Vec3::new(-1., 0., 1.).normalize(),
            Vec3::new(1., 0., -1.).normalize(),
            Vec3::new(-1., 0., -1.).normalize(),
        ];
        for distance in [1., 2., 4., 8.] {
            for direction in directions {
                let spawn = origin + direction * distance;
                let mut character = physics::character::Character::new(spawn.as_dvec3());
                if self.colliders.is_empty()
                    || character
                        .teleport(
                            &self.query_scene,
                            physics::queries::Filter::blocking(self.player_life().instance),
                            physics::character::Settings::default(),
                            spawn.as_dvec3(),
                        )
                        .is_ok()
                {
                    return Ok(spawn);
                }
            }
        }
        Err("Authored realm spawn has no vacant entry point".into())
    }
    /// Binds a trusted controller to a new adventurer in this same world.
    /// A network host must admit membership and select the spawn before calling this.
    pub fn add_player(&mut self, controller: Controller, spawn: Vec3) -> Result<LifeId, String> {
        if self.additional_players.len() + usize::from(self.primary_resident) >= 64
            || self.scene.actors.len() >= 256
            || !spawn.is_finite()
            || spawn.abs().max_element() > 10_000.
        {
            return Err("Controlled player admission budget exceeded".into());
        }
        // Reuse retired player slots with a fresh life, retaining a bounded
        // body fence per slot rather than a tombstone per past character.
        let authored_max = self
            .ids
            .keys()
            .copied()
            .chain([self.player_actor()])
            .filter(|id| *id < 1_000_000)
            .max()
            .unwrap_or(0);
        let retired = self
            .bodies
            .records()
            .find(|record| {
                !record.actor
                    && record.phase == physics::lifetimes::Phase::Removed
                    && matches!(record.hull, physics::lifetimes::Hull::UprightCapsule { .. })
                    && !self
                        .spells
                        .meteors
                        .iter()
                        .any(|e| e.caster == record.life.entity && !e.swarm.finished())
                    && record.life.entity > authored_max
                    && record.life.entity < self.next_player_actor
                    && !self.scene.actors.iter().any(|a| a.id == record.life.entity)
            })
            .map(|record| record.life);
        let (actor, generation) = if let Some(retired) = retired {
            (
                retired.entity,
                retired
                    .generation
                    .checked_add(1)
                    .ok_or("Player generations exhausted")?,
            )
        } else {
            if self.bodies.records().count() >= 1024 {
                return Err("Controlled player body budget exceeded".into());
            }
            let mut actor = self.next_player_actor;
            while self.scene.actors.iter().any(|a| a.id == actor)
                || self.bodies.records().any(|r| r.life.entity == actor)
            {
                actor = actor.checked_add(1).ok_or("Player actor IDs exhausted")?;
            }
            (actor, 0)
        };
        let next = self
            .next_player_actor
            .max(actor.checked_add(1).ok_or("Player actor IDs exhausted")?);
        if next >= 1_000_000 {
            return Err("Player actor IDs exhausted".into());
        }
        let life = LifeId {
            instance: self.player_life().instance,
            actor,
            generation,
        };
        let mut character = physics::character::Character::new(spawn.as_dvec3());
        if !self.colliders.is_empty() {
            character.teleport(
                &self.query_scene,
                self.actor_filter(life),
                physics::character::Settings::default(),
                spawn.as_dvec3(),
            )?;
        }
        let mut appearance = self
            .scene
            .actors
            .iter()
            .find(|a| a.id == self.player_actor())
            .ok_or("Missing player appearance")?
            .clone();
        let position = character.feet.as_vec3();
        let source = self.simulation.spawn_player(position.to_array())?;
        appearance.id = actor;
        appearance.position = position;
        appearance.name = format!("Adventurer {actor}");
        appearance.nameplate = false;
        appearance.health = 200;
        self.scene.actors.push(appearance);
        self.spells.caster_dice.remove(&actor);
        let mut player = Player::new(Admission::new(life, controller), source, spawn);
        player.character = character;
        player.player = position;
        player.previous_player = position;
        self.additional_players.insert(actor, player);
        self.next_player_actor = next;
        self.sync_bodies(0.)?;
        Ok(life)
    }
    pub fn player_admission(&self, actor: u64) -> Option<&Admission> {
        if actor == self.player_actor() {
            self.primary_resident.then_some(&self.primary.admission)
        } else {
            self.additional_players.get(&actor).map(|p| &p.admission)
        }
    }
    pub(crate) fn player_spawn(&self, actor: u64) -> Option<Vec3> {
        if actor == self.player_actor() {
            self.scene
                .actors
                .iter()
                .find(|a| a.model == "adventurer")
                .map(|a| a.position)
        } else {
            self.additional_players.get(&actor).map(|p| p.spawn)
        }
    }
    pub fn player_snapshot(&self, life: LifeId) -> Result<Snapshot, String> {
        if self
            .player_admission(life.actor)
            .is_none_or(|a| a.actor() != life)
        {
            return Err("Player life is stale".into());
        }
        self.simulation
            .snapshot_for(self.player_source(life.actor).unwrap())
    }
    pub(crate) fn player_private_snapshot(&self, life: LifeId) -> Result<Snapshot, String> {
        if self
            .player_admission(life.actor)
            .is_none_or(|a| a.actor() != life)
        {
            return Err("Player life is stale".into());
        }
        self.simulation
            .private_snapshot(self.player_source(life.actor).unwrap())
    }
    /// Extracts HUD values for this exact controlled life without exposing authority.
    pub fn player_hud(&self, life: LifeId) -> Result<crate::hud::Own, String> {
        let snapshot = self.player_private_snapshot(life)?;
        let (controls, bow_ready, casting) = if life == self.player_life() {
            (
                &self.primary.controls,
                self.primary.bow_ready,
                self.primary.casting.as_ref(),
            )
        } else {
            let p = &self.additional_players[&life.actor];
            (&p.controls, p.bow_ready, p.casting.as_ref())
        };
        Ok(crate::hud::Own::extract(
            self, life, snapshot, controls, bow_ready, casting,
        ))
    }
    pub(crate) fn equipment_limits(
        &mut self,
        actor: u64,
        hp: i32,
        mana: i32,
    ) -> Result<(), String> {
        let source = self
            .player_source(actor)
            .ok_or("Unknown controlled player")?;
        self.simulation.equipment_limits(source, hp, mana)?;
        let current = self.simulation.snapshot_for(source)?.player.hp;
        self.observed_health.insert(source, current);
        Ok(())
    }
    pub(crate) fn recover_player_resources(
        &mut self,
        actor: u64,
        health: u32,
        mana: u32,
    ) -> Result<(), String> {
        let source = self
            .player_source(actor)
            .ok_or("Unknown controlled player")?;
        self.simulation.recover_resources(source, health, mana)
    }
    pub(crate) fn player_source(&self, actor: u64) -> Option<u32> {
        if actor == self.player_actor() {
            self.primary_resident.then_some(0)
        } else {
            self.additional_players.get(&actor).map(|p| p.source)
        }
    }
    pub fn handoff_player(&mut self, life: LifeId, controller: Controller) -> Result<(), String> {
        if self
            .player_admission(life.actor)
            .is_none_or(|a| a.actor() != life)
        {
            return Err("Player life is stale".into());
        }
        if life.actor == self.player_actor() {
            self.primary
                .admission
                .handoff(controller)
                .map_err(|e| format!("Control handoff refused: {e:?}"))?;
            self.agent_controlled = controller == Controller(2);
            self.primary.pending_movement = None;
            self.primary.held_movement = Default::default();
            self.primary.pending_jump = false;
            self.primary.frame_clock = None;
        } else {
            let p = self.additional_players.get_mut(&life.actor).unwrap();
            p.admission
                .handoff(controller)
                .map_err(|e| format!("Control handoff refused: {e:?}"))?;
            p.pending_movement = None;
            p.held_movement = Default::default();
            p.pending_jump = false;
            p.frame_clock = None;
        }
        self.clear_social_seat(life);
        Ok(())
    }
    pub(super) fn submit_additional(
        &mut self,
        controller: Controller,
        command: Command<Ability>,
    ) -> Result<(), String> {
        let actor = command.actor.actor;
        let p = self
            .additional_players
            .get(&actor)
            .ok_or("Unknown controlled player")?;
        if !self.unlocked() || self.simulation.snapshot_for(p.source)?.player.hp == 0 {
            return Err("The adventurer cannot act in the current state".into());
        }
        if p.frame_clock.is_some() && matches!(command.intent, Intent::Move { .. } | Intent::Jump) {
            return Err("This controller requires movement intervals".into());
        }
        if let Intent::Cast { aim, target, .. } = &command.intent {
            if aim[1].abs() > 0.001
                || target.is_some_and(|life| {
                    self.lives.get(&life.actor) != Some(&life) || !self.hostile_actor(life.actor)
                })
            {
                return Err("Invalid spell target or horizontal aim".into());
            }
        }
        self.additional_players
            .get_mut(&actor)
            .unwrap()
            .admission
            .admit(controller, &command, self.authority_tick)
            .map_err(|e| format!("Command refused: {e:?}"))?;
        if let Intent::Cast {
            ability,
            target,
            aim,
        } = command.intent
        {
            if matches!(ability, Ability::Spell(_) | Ability::SpellCommand(_)) {
                let state = self.additional_players.get_mut(&actor).unwrap();
                state.yaw = (-aim[0]).atan2(-aim[2]);
                if let Some(target) = target {
                    state.selected = target.actor;
                }
                let context = self.caster_context(command.actor)?;
                return self.activate_catalog(context, ability);
            }
        }
        let mut p = self.additional_players.remove(&actor).unwrap();
        let result = match command.intent {
            Intent::Move { axes, yaw } => {
                p.pending_movement = Some(axes);
                p.yaw = yaw;
                Ok(())
            }
            Intent::Jump => {
                p.pending_jump = true;
                Ok(())
            }
            Intent::Cast {
                ability,
                target,
                aim,
            } => {
                p.yaw = (-aim[0]).atan2(-aim[2]);
                if let Some(target) = target {
                    p.selected = target.actor;
                }
                self.activate_player(&mut p, ability, target)
            }
        };
        self.additional_players.insert(actor, p);
        result
    }
    pub(super) fn activate_player(
        &mut self,
        p: &mut Player,
        ability: Ability,
        target: Option<LifeId>,
    ) -> Result<(), String> {
        if p.casting.is_some() {
            return Err("A spell is already being cast".into());
        }
        let target_position = target
            .and_then(|life| self.ids.get(&life.actor))
            .and_then(|source| {
                self.snapshot()
                    .actors
                    .into_iter()
                    .find(|a| a.id == *source && a.alive)
            })
            .map(|a| Vec3::from(a.pos));
        let direction = Vec3::new(-p.yaw.sin(), 0., -p.yaw.cos());
        let start = p.player + Vec3::Y * 1.4;
        if matches!(ability, Ability::FireBolt | Ability::Bow) {
            if let Some(prop) = self
                .spells
                .props
                .iter()
                .find(|p2| p2.life.entity == p.selected && !p2.removed)
            {
                let end = self.spells.world[prop.body].pos.as_vec3();
                if start.distance(end) > 36.576 {
                    return Err("Target is out of range".into());
                }
                let direction = (end - start).normalize_or_zero();
                if ability == Ability::FireBolt {
                    self.simulation.cast_at(
                        p.source,
                        crate::rules::Spell::Firebolt,
                        start.to_array(),
                        direction.to_array(),
                    )?;
                } else {
                    if self.time < p.bow_ready {
                        return Err("Bow is cooling down".into());
                    }
                    self.simulation.launch_bow_at(
                        p.source,
                        start.to_array(),
                        direction.to_array(),
                    )?;
                    p.bow_ready = self.time + 1.;
                }
                p.last_cast = Some((ability, self.time));
                self.record_ability(ability);
                return Ok(());
            }
        }
        if let Some(spell) = ability.utility() {
            if matches!(spell, Utility::Web | Utility::Grease)
                && target_position.is_some_and(|pos| !self.attack_clear(start, pos + Vec3::Y * 1.1))
            {
                return Err("Target is blocked by chamber geometry".into());
            }
            let teleport = if spell == Utility::MistyStep {
                let destination = if self.colliders.is_empty() {
                    p.player + direction * 9.144
                } else {
                    physics::character::slide(
                        &self.query_scene,
                        self.actor_filter(p.admission.actor()),
                        physics::character::Settings::default(),
                        p.player.as_dvec3(),
                        (direction * 9.144).as_dvec3(),
                        true,
                    )?
                    .as_vec3()
                };
                let mut check = p.character;
                if !self.colliders.is_empty() {
                    check.teleport(
                        &self.query_scene,
                        self.actor_filter(p.admission.actor()),
                        physics::character::Settings::default(),
                        destination.as_dvec3(),
                    )?;
                }
                Some(destination)
            } else {
                None
            };
            let destination = p.controls.cast_with_visibility_for(
                &mut self.simulation,
                p.source,
                spell,
                self.time,
                p.player,
                direction,
                target_position,
                teleport,
                |position| {
                    physics::kinematic::sweep_box(
                        start.as_dvec3(),
                        glam::DVec3::splat(0.12),
                        (position + Vec3::Y * 1.1 - start).as_dvec3(),
                        &self.colliders,
                    )
                    .is_ok_and(|hit| hit.is_none())
                },
            )?;
            if spell == Utility::Thunderwave {
                crate::spells::thunderwave::resolve_for(
                    self,
                    p.admission.actor().actor,
                    p.player,
                    direction,
                    p.definition.save_dc,
                )?;
            }
            if spell == Utility::MistyStep {
                p.player = destination;
                p.previous_player = destination;
                p.character = physics::character::Character::new(destination.as_dvec3());
                p.player_trajectory.clear();
                if p.frame_clock.take().is_some() {
                    p.admission
                        .handoff(p.admission.controller())
                        .map_err(|e| format!("Teleport control fence refused: {e:?}"))?;
                    p.held_movement = Default::default();
                    p.pending_movement = None;
                    p.pending_jump = false;
                }
                self.simulation
                    .teleport_chamber_actor(p.source, destination.to_array(), p.yaw)?;
                self.place_actor_body(p.admission.actor(), destination, 0.)?;
            }
        } else {
            let end = target_position.ok_or("Select a living target")? + Vec3::Y * 1.1;
            let range = if ability == Ability::Fireball {
                45.72
            } else {
                36.576
            };
            if start.distance(end) > range {
                return Err("Target is out of range".into());
            }
            if !self.attack_clear(start, end) {
                return Err("Target is blocked by chamber geometry".into());
            }
            if let Some(spell) = ability.spell() {
                let state = self.simulation.snapshot_for(p.source)?;
                let gate = state.abilities.iter().find(|a| a.id == spell).unwrap();
                if !gate.ready {
                    return Err(if state.player.mana < gate.cost {
                        "Not enough mana"
                    } else {
                        "Spell is cooling down"
                    }
                    .into());
                }
                if ability == Ability::FireBolt {
                    self.simulation.cast_at(
                        p.source,
                        spell,
                        start.to_array(),
                        (end - start).normalize().to_array(),
                    )?;
                } else {
                    p.casting = Some(Casting {
                        target_life: target.unwrap(),
                        aim: end,
                        ability,
                        started: self.time,
                        ends: self.time + 1.,
                        origin: start,
                        direction: (end - start).normalize(),
                    });
                }
            } else if ability == Ability::Bow {
                if self.time < p.bow_ready {
                    return Err("Bow is cooling down".into());
                }
                self.simulation.launch_bow_at(
                    p.source,
                    start.to_array(),
                    (end - start).normalize().to_array(),
                )?;
                p.bow_ready = self.time + 1.;
            } else {
                return Err("Unsupported shared ability".into());
            }
        }
        p.last_cast = Some((ability, self.time));
        self.record_ability(ability);
        Ok(())
    }
    pub(super) fn step_additional(&mut self, steps: u32, dt: f32) -> Result<(), String> {
        let actors: Vec<_> = self.additional_players.keys().copied().collect();
        for actor in actors {
            let mut p = self.additional_players.remove(&actor).unwrap();
            let result: Result<(), String> = (|| {
                let dead = self.simulation.snapshot_for(p.source)?.player.hp == 0;
                if p.frame_clock.is_some() && (dead || !self.unlocked()) {
                    p.admission
                        .handoff(p.admission.controller())
                        .map_err(|e| format!("Unavailable interval handoff refused: {e:?}"))?;
                    p.frame_clock = None;
                    p.pending_movement = None;
                    p.pending_jump = false;
                    p.held_movement = Default::default();
                } else if p
                    .frame_clock
                    .as_ref()
                    .is_some_and(|clock| clock.expired(self.physics_steps))
                {
                    self.movement_expiry
                        .record(p.frame_clock.as_ref().unwrap().expiry_sample(
                            actor,
                            p.admission.epoch(),
                            self.authority_tick,
                            self.physics_steps,
                            crate::movement::frames::ExpiryOrigin::AdditionalTick,
                        ));
                    p.admission
                        .handoff(p.admission.controller())
                        .map_err(|e| format!("Expired interval handoff refused: {e:?}"))?;
                    p.frame_clock = None;
                    p.pending_movement = None;
                    p.pending_jump = false;
                    p.held_movement = Default::default();
                }
                let axes = if dead {
                    p.pending_movement = None;
                    p.held_movement = Default::default();
                    [0.; 2]
                } else {
                    let start = self.physics_steps - steps as u64;
                    if let Some(axes) = p.pending_movement.take() {
                        p.held_movement.refresh(axes, start)?;
                    }
                    p.held_movement.axes(start)
                };
                let walk = crate::movement::walk(axes, p.yaw)?;
                let speed = walk.speed;
                let terrain = crate::spells::black_tentacles::speed_scale(
                    &self.spells,
                    actor,
                    p.player.as_dvec3(),
                ) as f32;
                let velocity = walk.direction * speed * terrain;
                if dead || velocity.length_squared() > 0. {
                    p.casting = None;
                }
                p.previous_player = p.player;
                p.locomotion = axes;
                let jump = std::mem::take(&mut p.pending_jump) && !dead;
                let mut fell = 0.;
                p.player_trajectory = vec![p.player.to_array()];
                if dead {
                    p.player_trajectory.push(p.player.to_array());
                } else if self.colliders.is_empty() {
                    p.player += velocity * dt;
                    p.player.x = p.player.x.clamp(-12., 12.);
                    p.player.z = p.player.z.clamp(-25., 12.);
                    p.character = physics::character::Character::new(p.player.as_dvec3());
                    p.player_trajectory.push(p.player.to_array());
                } else {
                    let framed = p.frame_clock.is_some();
                    let frame_work = if let Some(clock) = &mut p.frame_clock {
                        crate::movement::frames::expand(&clock.take(self.physics_steps)?)
                    } else {
                        Vec::new()
                    };
                    let count = if framed {
                        frame_work.len() as u32
                    } else {
                        steps
                    };
                    if frame_work
                        .iter()
                        .any(|step| step.held.axes(step.at).iter().any(|v| *v != 0.))
                    {
                        p.casting = None;
                    }
                    for step in 0..count {
                        let (velocity, jump_step) = if framed {
                            let input = frame_work[step as usize];
                            p.held_movement = input.held;
                            p.yaw = input.yaw;
                            p.locomotion = input.held.axes(input.at);
                            let walk = crate::movement::walk(p.locomotion, p.yaw)?;
                            (walk.direction * walk.speed * terrain, input.jump)
                        } else {
                            (velocity, jump && step == 0)
                        };
                        let spell_time = if framed {
                            self.time as f64
                                - f64::from(dt) * f64::from(count - step) / f64::from(count)
                        } else {
                            self.time as f64 - (steps - step) as f64 * self.physics_clock.dt
                        };
                        crate::spells::feather_fall::prepare(
                            &mut self.spells,
                            actor,
                            &mut p.character,
                            spell_time,
                            self.physics_clock.dt,
                        );
                        let spell_velocity = crate::spells::levitate::prepare(
                            &mut self.spells,
                            actor,
                            &mut p.character,
                            velocity.as_dvec3(),
                            spell_time,
                            self.physics_clock.dt,
                        );
                        let spell_velocity = crate::spells::telekinesis::prepare(
                            &self.spells,
                            actor,
                            &mut p.character,
                            spell_velocity,
                        );
                        let spell_velocity = crate::spells::proxies::prepare(
                            &self.spells,
                            actor,
                            &mut p.character,
                            spell_velocity,
                        );
                        let spell_velocity = crate::spells::gust::movement(
                            &self.spells,
                            p.character.feet,
                            spell_velocity,
                        );
                        let before_bounce = p.character.external;
                        let outcome = p.character.step_contained(
                            &self.query_scene,
                            self.actor_filter(p.admission.actor()),
                            physics::character::Settings::default(),
                            spell_velocity,
                            jump_step,
                            self.physics_clock.dt,
                        )?;
                        if !self.motor_recovery.observe(outcome) {
                            break;
                        }
                        crate::spells::levitate::bounce(
                            &self.spells,
                            actor,
                            &mut p.character,
                            before_bounce,
                        );
                        fell += p.character.landed.unwrap_or(0.);
                        p.player_trajectory
                            .push(p.character.feet.as_vec3().to_array());
                    }
                    p.player = p.character.feet.as_vec3();
                }
                let distance = p.player.distance(p.previous_player);
                p.moving = distance > 0.00001;
                p.motion_clock += distance / speed;
                self.simulation
                    .place_chamber_actor(p.source, p.player.to_array(), p.yaw)?;
                if p.player_trajectory.len() >= 2 {
                    self.simulation
                        .record_motion_path(p.source, p.player_trajectory.clone())?;
                }
                self.place_actor_body(p.admission.actor(), p.player, dt as f64)?;
                let warded = fell > 0.
                    && self
                        .spells
                        .feather_falls
                        .iter_mut()
                        .any(|effect| effect.land(actor as u32, self.time as f64).is_some());
                let gentle = fell > 0.
                    && self.spells.levitations.iter_mut().any(|e| {
                        if e.target == crate::spells::Target::Actor(actor) && e.state.gentle() {
                            e.state.land();
                            true
                        } else {
                            false
                        }
                    });
                if !dead && fell > 0. && !warded && !gentle {
                    let dice = crate::spells::fall_dice(fell);
                    if dice > 0 {
                        let amount =
                            self.spells.dice_for(p.admission.actor().actor).sum(dice, 6) as i32;
                        let hp = self.simulation.snapshot_for(p.source)?.player.hp;
                        let loss = amount.min(hp);
                        self.simulation.player_damage_for(p.source, loss)?;
                        self.player_damage_event(p.admission.actor(), p.source, loss, p.player)?;
                    }
                }
                if p.casting.as_ref().is_some_and(|c| self.time >= c.ends) {
                    let c = p.casting.take().unwrap();
                    let alive = self.ids.get(&c.target_life.actor).is_some_and(|source| {
                        self.snapshot()
                            .actors
                            .iter()
                            .any(|a| a.id == *source && a.alive)
                    });
                    if self.actor_life(c.target_life.actor) == Some(c.target_life)
                        && alive
                        && self.attack_clear(c.origin, c.aim)
                    {
                        self.simulation.cast_at(
                            p.source,
                            c.ability.spell().unwrap(),
                            c.origin.to_array(),
                            c.direction.to_array(),
                        )?;
                        p.last_cast = Some((c.ability, self.time));
                    }
                }
                if self.simulation.snapshot_for(p.source)?.player.hp == 0 {
                    p.died_at.get_or_insert(self.time);
                }
                Ok(())
            })();
            self.additional_players.insert(actor, p);
            result?;
        }
        Ok(())
    }
    pub(crate) fn living_players(&self) -> Vec<(LifeId, Vec3)> {
        let mut result = vec![];
        if self.primary_resident
            && self
                .simulation
                .player_resources(0)
                .is_some_and(|p| p.hp > 0)
        {
            result.push((self.player_life(), self.primary.player));
        }
        result.extend(
            self.additional_players
                .values()
                .filter(|p| {
                    self.simulation
                        .player_resources(p.source)
                        .is_some_and(|p| p.hp > 0)
                })
                .map(|p| (p.admission.actor(), p.player)),
        );
        result
    }
    pub(crate) fn player_path_segments(&self, actor: u64) -> usize {
        if actor == self.player_actor() {
            self.player_motion_segments()
        } else {
            self.additional_players
                .get(&actor)
                .map_or(0, |p| p.player_trajectory.len().saturating_sub(1))
        }
    }
    pub(crate) fn player_path_at(&self, actor: u64, fraction: f32) -> Vec3 {
        if actor == self.player_actor() {
            return self.player_motion_at(fraction);
        }
        let p = &self.additional_players[&actor];
        if p.player_trajectory.len() < 2 {
            return p.player;
        }
        let segment = fraction.clamp(0., 1.) * (p.player_trajectory.len() - 1) as f32;
        let index = (segment.floor() as usize).min(p.player_trajectory.len() - 2);
        Vec3::from(p.player_trajectory[index]).lerp(
            Vec3::from(p.player_trajectory[index + 1]),
            segment - index as f32,
        )
    }
    pub fn hostile_hit_player(&mut self, life: LifeId, damage: i32) -> Result<(i32, i32), String> {
        if !(0..=10_000).contains(&damage)
            || self
                .player_admission(life.actor)
                .is_none_or(|a| a.actor() != life)
        {
            return Err("Invalid hostile player damage".into());
        }
        if life == self.player_life() {
            return self.hostile_hit(damage);
        }
        let p = self.additional_players.get_mut(&life.actor).unwrap();
        let hp = self.simulation.snapshot_for(p.source)?.player.hp;
        if hp == 0 {
            return Ok((0, 0));
        }
        let absorbed = p.controls.absorb(damage, self.time);
        let loss = (damage - absorbed).min(hp);
        let (source, position) = (p.source, p.player);
        self.simulation.player_damage_for(source, loss)?;
        if loss == hp {
            p.died_at = Some(self.time);
            p.casting = None;
        }
        self.player_damage_event(life, source, loss, position)?;
        Ok((loss, absorbed))
    }
    fn player_damage_event(
        &mut self,
        life: LifeId,
        source: u32,
        amount: i32,
        position: Vec3,
    ) -> Result<(), String> {
        if amount == 0 {
            return Ok(());
        }
        self.event(
            Some(life),
            crate::events::Kind::Damage {
                amount,
                incoming: true,
            },
        )?;
        if self.simulation.snapshot_for(source)?.player.hp == 0 {
            self.event(Some(life), crate::events::Kind::Death)?;
        }
        self.damage_serial = self
            .damage_serial
            .checked_add(1)
            .ok_or("Damage serial exhausted")?;
        if self.damage_numbers.len() == 64 {
            self.damage_numbers.remove(0);
        }
        self.damage_numbers.push(DamageNumber {
            actor: life.actor,
            amount,
            at: self.time,
            position,
            incoming: true,
            serial: self.damage_serial,
        });
        Ok(())
    }
    pub fn respawn_controlled_player(
        &mut self,
        controller: Controller,
        life: LifeId,
    ) -> Result<LifeId, String> {
        let a = self
            .player_admission(life.actor)
            .ok_or("Unknown controlled player")?;
        if a.actor() != life || a.controller() != controller {
            return Err("Player respawn ownership is stale".into());
        }
        if life == self.player_life() {
            self.respawn_player()?;
            if controller != Controller(1) {
                self.handoff_player(self.player_life(), controller)?;
            }
            return Ok(self.player_life());
        }
        let p = &self.additional_players[&life.actor];
        if self.simulation.snapshot_for(p.source)?.player.hp != 0 {
            return Err("The adventurer is still alive".into());
        }
        let mut admission = p.admission.clone();
        admission
            .respawn()
            .map_err(|e| format!("Respawn refused: {e:?}"))?;
        let feet = self.respawn_feet(life, p.spawn.as_dvec3())?;
        let mut character = physics::character::Character::new(feet);
        if !self.colliders.is_empty() {
            character.teleport(
                &self.query_scene,
                self.actor_filter(life),
                physics::character::Settings::default(),
                feet,
            )?;
        }
        let physical = physics::queries::Life {
            instance: life.instance,
            entity: life.actor,
            generation: life.generation,
        };
        let (source, spawn) = (p.source, p.spawn);
        let definition = p.definition.clone();
        let mut blockers = self.blockers.clone();
        if blockers.remove(physical)? && self.navigation.is_some() {
            self.replace_blockers(blockers)?;
        }
        let revived = feet.as_vec3();
        self.simulation
            .revive_player(source, revived.to_array(), std::f32::consts::PI)?;
        self.clear_social_seat(life);
        self.bodies.remove(physical);
        self.spells.end_concentration(life.actor)?;
        let next = admission.actor();
        let mut p = Player::new(admission, source, spawn);
        p.player = revived;
        p.previous_player = revived;
        p.definition = definition;
        p.character = character;
        self.additional_players.insert(life.actor, p);
        if let Some(e) = &mut self.encounter {
            e.casts.retain(|c| c.target_life != life);
            e.ended = None;
        }
        self.sync_bodies(0.)?;
        self.event(Some(next), crate::events::Kind::Respawn)?;
        Ok(next)
    }
    /// Where a respawned adventurer stands: its spawn, or, while something
    /// stands on the spawn, the nearest clear walkable cell on the same floor
    /// within [`RESPAWN_RADIUS`] (#10559). The battle soak's frontline player
    /// stayed dead for minutes because hostiles and corpses held its spawn.
    pub(crate) fn respawn_feet(
        &self,
        life: LifeId,
        spawn: glam::DVec3,
    ) -> Result<glam::DVec3, String> {
        if self.colliders.is_empty() {
            return Ok(spawn);
        }
        let filter = self.actor_filter(life);
        let settings = physics::character::Settings::default();
        let clear = |feet: glam::DVec3| {
            physics::character::Character::new(feet)
                .teleport(&self.query_scene, filter, settings, feet)
                .is_ok()
        };
        if clear(spawn) {
            return Ok(spawn);
        }
        let obstructed = || "Character teleport endpoint is obstructed".to_string();
        let Some(navigation) = &self.navigation else {
            return Err(obstructed());
        };
        let distance =
            |feet: &glam::DVec3| glam::DVec2::new(feet.x - spawn.x, feet.z - spawn.z).length();
        let mut cells: Vec<_> = navigation
            .nodes()
            .iter()
            .map(|cell| cell.feet)
            .filter(|feet| {
                (feet.y - spawn.y).abs() <= settings.step_height && distance(feet) <= RESPAWN_RADIUS
            })
            .collect();
        cells.sort_by(|a, b| {
            distance(a)
                .total_cmp(&distance(b))
                .then(a.x.total_cmp(&b.x))
                .then(a.z.total_cmp(&b.z))
        });
        cells
            .into_iter()
            .find(|feet| clear(*feet))
            .ok_or_else(obstructed)
    }
    pub(crate) fn shared_prone(&self, position: Vec3) -> bool {
        self.primary.controls.prone(position, self.time)
            || self
                .additional_players
                .values()
                .any(|p| p.controls.prone(position, self.time))
    }
    /// Resolves a combat-store caster ID to its exact current controlled life.
    pub fn projectile_caster_life(&self, source: u32) -> Option<LifeId> {
        if source == 0 {
            self.primary_resident.then_some(self.player_life())
        } else {
            self.additional_players
                .values()
                .find(|p| p.source == source)
                .map(|p| p.admission.actor())
        }
    }
    /// Read-only effect projection for every controlled adventurer.
    pub fn controlled_effects(&self) -> impl Iterator<Item = (LifeId, Vec3, &Controls)> + '_ {
        self.primary_resident
            .then_some((
                self.player_life(),
                self.primary.player,
                &self.primary.controls,
            ))
            .into_iter()
            .chain(
                self.additional_players
                    .values()
                    .map(|p| (p.admission.actor(), p.player, &p.controls)),
            )
    }
    pub(crate) fn rebuild_players_after_restart(&mut self, previous: &Game) -> Result<(), String> {
        self.configure_character(self.player_life(), previous.primary.definition.clone())?;
        for (actor, p) in &previous.additional_players {
            let mut admission = p.admission.clone();
            admission
                .respawn()
                .map_err(|e| format!("Restart player refused: {e:?}"))?;
            let source = self.simulation.spawn_player(p.spawn.to_array())?;
            let life = admission.actor();
            self.additional_players
                .insert(*actor, Player::new(admission, source, p.spawn));
            self.configure_character(life, p.definition.clone())?;
        }
        self.next_player_actor = previous.next_player_actor;
        if !previous.primary_resident {
            self.take_primary_player()?;
        }
        self.sync_bodies(0.)?;
        Ok(())
    }
    /// Restarts target content while preserving persistent player identities.
    pub(crate) fn migrate_content(
        mut self,
        previous: &Game,
        spawns: &BTreeMap<u64, Vec3>,
    ) -> Result<Self, String> {
        if self.player_life().instance != previous.player_life().instance
            || self.player_actor() != previous.player_actor()
            || !self.additional_players.is_empty()
            || previous
                .additional_players
                .keys()
                .any(|id| self.scene.actors.iter().any(|a| a.id == *id))
        {
            return Err("Target content conflicts with persistent player identity".into());
        }
        self.fence_world_generation(previous)?;
        for life in self.lives.values_mut() {
            life.generation = self.migration_generation;
        }
        self.routes.clear();
        self.navigation_goals.clear();
        self.primary.admission = previous.primary.admission.clone();
        self.primary
            .admission
            .respawn()
            .map_err(|e| format!("Migration life refused: {e:?}"))?;
        self.authority_tick = previous.authority_tick;
        self.event_serial = previous.event_serial;
        self.events = previous.events.clone();
        self.sync_bodies(0.)?;
        self.configure_character(self.player_life(), previous.primary.definition.clone())?;
        let appearance = self
            .scene
            .actors
            .iter()
            .find(|a| a.id == self.player_actor())
            .ok_or("Missing migration player appearance")?
            .clone();
        for (actor, old) in &previous.additional_players {
            let spawn = spawns.get(actor).copied().unwrap_or(old.spawn);
            if !spawn.is_finite()
                || spawn.abs().max_element() > 10_000.
                || self.scene.actors.len() >= 256
            {
                return Err("Migrated player spawn or appearance budget is invalid".into());
            }
            let mut admission = old.admission.clone();
            admission
                .respawn()
                .map_err(|e| format!("Migration player refused: {e:?}"))?;
            let mut character = physics::character::Character::new(spawn.as_dvec3());
            if !self.colliders.is_empty() {
                character.teleport(
                    &self.query_scene,
                    self.actor_filter(admission.actor()),
                    physics::character::Settings::default(),
                    spawn.as_dvec3(),
                )?;
            }
            let source = self
                .simulation
                .spawn_player(character.feet.as_vec3().to_array())?;
            let mut player = Player::new(admission, source, spawn);
            player.character = character;
            player.player = character.feet.as_vec3();
            player.previous_player = player.player;
            let life = player.admission.actor();
            self.additional_players.insert(*actor, player);
            self.configure_character(life, old.definition.clone())?;
            let mut actor_appearance = appearance.clone();
            actor_appearance.id = *actor;
            actor_appearance.position = spawn;
            actor_appearance.name = format!("Adventurer {actor}");
            actor_appearance.nameplate = false;
            actor_appearance.health = 200;
            self.scene.actors.push(actor_appearance);
            self.sync_bodies(0.)?;
        }
        self.next_player_actor = self.next_player_actor.max(previous.next_player_actor);
        if !previous.primary_resident {
            self.take_primary_player()?;
        }
        self.checkpoint()?;
        Ok(self)
    }
    pub(super) fn validate_players(&self) -> Result<(), String> {
        if self.primary.source != 0 {
            return Err("Primary resource source disagrees".into());
        }
        for p in std::iter::once(&self.primary).chain(self.additional_players.values()) {
            p.definition.validate()?;
            if p.catalog_ready.len() > 9
                || p.catalog_ready.iter().any(|(slot, at)| {
                    crate::spells::spell_in_slot(*slot).is_none() || !at.is_finite() || *at < 0.
                })
            {
                return Err("Invalid actor catalog cooldowns".into());
            }
        }
        if self.spells.primary_caster != self.primary.admission.actor().actor {
            return Err("Primary dice ownership disagrees".into());
        }
        if self.primary_resident == self.simulation.primary_absent() {
            return Err("Primary character and simulation ownership disagree".into());
        }
        if !self.primary_resident
            && (self
                .simulation
                .player_resources(0)
                .is_none_or(|r| r.hp != 0)
                || self.agent_controlled
                || self.primary.pending_movement.is_some()
                || self.primary.pending_jump
                || self.primary.casting.is_some())
        {
            return Err("Absent primary character retains live authority".into());
        }
        if self.additional_players.len() + usize::from(self.primary_resident) > 64
            || self.next_player_actor >= 1_000_000
        {
            return Err("Player checkpoint capacity exceeded".into());
        }
        if self
            .scene
            .actors
            .iter()
            .filter(|a| a.model == "adventurer")
            .any(|a| a.id != self.player_actor() && !self.additional_players.contains_key(&a.id))
        {
            return Err("Player appearance has no controlled life".into());
        }
        let mut sources = std::collections::BTreeSet::from([0]);
        for (actor, p) in &self.additional_players {
            if let Some(clock) = &p.frame_clock {
                clock.validate(
                    self.physics_steps,
                    p.admission.actor(),
                    p.admission.epoch(),
                    p.admission.accepted_sequence(),
                )?;
            }
            p.character.validate()?;
            p.controls.validate()?;
            if *actor >= self.next_player_actor
                || p.admission.actor().actor != *actor
                || p.admission.actor().instance != self.player_life().instance
                || *actor == self.player_actor()
                || self.lives.contains_key(actor)
                || !sources.insert(p.source)
                || !p.player.is_finite()
                || !p.previous_player.is_finite()
                || p.previous_player.abs().max_element() > 10_000.
                || !p.spawn.is_finite()
                || p.spawn.abs().max_element() > 10_000.
                || p.player.abs().max_element() > 10_000.
                || p.character.feet.as_vec3() != p.player
                || !p.yaw.is_finite()
                || !p.motion_clock.is_finite()
                || p.motion_clock < 0.
                || !p.bow_ready.is_finite()
                || p.bow_ready < 0.
                || p.pending_movement
                    .is_some_and(|a| a.iter().any(|v| !v.is_finite() || v.abs() > 1.))
                || p.held_movement.validate(self.physics_steps).is_err()
                || p.locomotion.iter().any(|v| !v.is_finite() || v.abs() > 1.)
                || p.died_at
                    .is_some_and(|at| !at.is_finite() || at < 0. || at > self.time)
                || p.last_cast.is_some_and(|(a, at)| {
                    (matches!(a, Ability::Spell(_) | Ability::SpellCommand(_))
                        && a.catalog().is_none())
                        || !at.is_finite()
                        || at < 0.
                        || at > self.time
                })
                || self
                    .scene
                    .actors
                    .iter()
                    .find(|a| a.id == *actor)
                    .is_none_or(|a| a.nameplate || a.model != "adventurer")
                || self
                    .simulation
                    .snapshot_for(p.source)?
                    .actors
                    .iter()
                    .find(|a| a.id == p.source)
                    .is_none_or(|a| Vec3::from(a.pos) != p.player)
            {
                return Err("Invalid controlled player checkpoint".into());
            }
            if let Some(c) = &p.casting {
                if c.ability.spell().is_none()
                    || c.target_life.instance != self.player_life().instance
                    || !self.lives.contains_key(&c.target_life.actor)
                    || !c.started.is_finite()
                    || !c.ends.is_finite()
                    || c.started < 0.
                    || c.started > self.time
                    || c.ends < c.started
                    || !c.origin.is_finite()
                    || !c.aim.is_finite()
                    || !c.direction.is_finite()
                    || !(0.99..=1.01).contains(&c.direction.length_squared())
                {
                    return Err("Invalid player cast checkpoint".into());
                }
            }
        }
        if sources != self.simulation.player_ids().collect() {
            return Err("Player resource ownership disagrees".into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn authored_instances_fence_commands_targets_and_survive_reset() {
        let scene = Scene::from_json(include_bytes!(
            "../../../../assets/verse/original/ritual.json"
        ))
        .unwrap();
        let mut left = Game::combat_in(scene.clone(), false, 41).unwrap();
        let mut right = Game::combat_in(scene, false, 42).unwrap();
        for game in [&mut left, &mut right] {
            game.time = game.scene.cut_at;
            game.tick(0., [0.; 2]).unwrap();
        }
        assert_eq!(left.player_life().actor, right.player_life().actor);
        assert_ne!(left.player_life(), right.player_life());
        let foreign = left
            .admission
            .command(left.authority_tick, Intent::Jump)
            .unwrap();
        assert!(right.submit(Controller(1), foreign).is_err());
        let foreign_target = left.actor_life(left.selected).unwrap();
        let cast = right
            .admission
            .command(
                right.authority_tick,
                Intent::Cast {
                    ability: Ability::FireBolt,
                    target: Some(foreign_target),
                    aim: [0., 0., 1.],
                },
            )
            .unwrap();
        assert!(right.submit(Controller(1), cast).is_err());
        let extra = left
            .add_player(Controller(10), Vec3::new(3., 0., -22.))
            .unwrap();
        assert_eq!(extra.instance, 41);
        let bytes = left.checkpoint().unwrap();
        let mut restored = Game::restore(&bytes).unwrap();
        assert_eq!(bytes, restored.checkpoint().unwrap());
        for _ in 0..8 {
            left.tick(1. / 30., [0.; 2]).unwrap();
            restored.tick(1. / 30., [0.; 2]).unwrap();
        }
        assert_eq!(left.checkpoint().unwrap(), restored.checkpoint().unwrap());
        left.restart_combat(false).unwrap();
        assert_eq!(left.player_life().instance, 41);
        let revived = left.player_admission(extra.actor).unwrap().actor();
        assert_eq!(revived.instance, 41);
        assert_eq!(revived.generation, extra.generation + 1);
        assert!(left.player_snapshot(extra).is_err());
        assert!(left.lives.values().all(|life| life.instance == 41));
        assert_eq!(left.bodies.instance, 41);
        assert_eq!(left.blockers.instance, 41);
        Game::restore(&left.checkpoint().unwrap()).unwrap();
    }

    fn world() -> (Game, LifeId) {
        world_profile(true)
    }
    fn world_profile(defer_hostiles: bool) -> (Game, LifeId) {
        let scene = Scene::from_json(include_bytes!(
            "../../../../assets/verse/original/ritual.json"
        ))
        .unwrap();
        let mut g = Game::combat(scene, false).unwrap();
        g.time = g.scene.cut_at;
        g.tick(0., [0.; 2]).unwrap();
        if defer_hostiles {
            g.encounter
                .as_mut()
                .unwrap()
                .postpone_casts_until(600.)
                .unwrap();
        }
        let life = g
            .add_player(Controller(10), Vec3::new(3., 0., -22.))
            .unwrap();
        (g, life)
    }
    #[test]
    fn retired_player_slots_survive_turnover_beyond_the_body_history_budget() {
        let (mut game, mut life) = world();
        let original = life;
        let body_slots = game.bodies.records().count();
        let spawn = game.player_spawn(life.actor).unwrap();
        let stale = game
            .player_admission(life.actor)
            .unwrap()
            .command(
                game.authority_tick,
                Intent::Move {
                    axes: [1., 0.],
                    yaw: 0.,
                },
            )
            .unwrap();
        for generation in 1..=1100 {
            let portable = game.take_transfer_player(life.actor).unwrap();
            life = game.add_player(Controller(10), spawn).unwrap();
            game.put_transfer_player(life, portable).unwrap();
            assert_eq!((life.actor, life.generation), (original.actor, generation));
            assert_eq!(game.bodies.records().count(), body_slots);
        }
        let before = game.checkpoint().unwrap();
        assert!(game.submit(Controller(10), stale).is_err());
        assert_eq!(game.checkpoint().unwrap(), before);
        let recovered = Game::restore(&before).unwrap();
        assert_eq!(
            recovered.player_admission(life.actor).unwrap().actor(),
            life
        );
        assert_eq!(recovered.bodies.records().count(), body_slots);
    }

    #[test]
    fn movement_baselines_follow_the_spell_target_not_the_caster() {
        let (mut game, extra) = world();
        let primary = game.player_life();
        let context = game.caster_context(game.player_life()).unwrap();
        crate::spells::levitate::cast_on(
            &mut game,
            context,
            crate::spells::Target::Actor(primary.actor),
        )
        .unwrap();
        assert_eq!(
            game.movement_baseline(primary)
                .unwrap()
                .unwrap()
                .policy
                .walking_scale,
            0.
        );
        assert!(
            !game
                .movement_baseline(primary)
                .unwrap()
                .unwrap()
                .policy
                .jump_allowed
        );
        assert_eq!(
            game.movement_baseline(extra)
                .unwrap()
                .unwrap()
                .policy
                .walking_scale,
            1.
        );
        game.spells.levitations[0].target = crate::spells::Target::Actor(extra.actor);
        assert_eq!(
            game.movement_baseline(primary)
                .unwrap()
                .unwrap()
                .policy
                .walking_scale,
            1.
        );
        assert_eq!(
            game.movement_baseline(extra)
                .unwrap()
                .unwrap()
                .policy
                .walking_scale,
            0.
        );
        assert!(
            !game
                .movement_baseline(extra)
                .unwrap()
                .unwrap()
                .policy
                .jump_allowed
        );
        game.spells.levitations[0]
            .state
            .end(crate::levitate::End::Concentration);
        assert_eq!(
            game.movement_baseline(extra)
                .unwrap()
                .unwrap()
                .policy
                .walking_scale,
            1.
        );
    }

    #[test]
    fn held_remote_movement_survives_gaps_then_expires_and_replays_checkpoints() {
        let (mut game, extra) = world();
        let primary = game.player_life();
        game.handoff_player(primary, Controller(9)).unwrap();
        let starts = [
            game.actor_position(primary.actor).unwrap(),
            game.actor_position(extra.actor).unwrap(),
        ];
        for (life, controller) in [(primary, Controller(9)), (extra, Controller(10))] {
            let input = command(
                &game,
                life,
                Intent::Move {
                    axes: [1., 0.],
                    yaw: 0.,
                },
            );
            game.submit(controller, input).unwrap();
        }
        game.tick(1. / 30., [0.; 2]).unwrap();
        let bytes = game.checkpoint().unwrap();
        let mut restored = Game::restore(&bytes).unwrap();
        for _ in 1..15 {
            game.tick(1. / 30., [0.; 2]).unwrap();
            restored.tick(1. / 30., [0.; 2]).unwrap();
        }
        assert_eq!(game.checkpoint().unwrap(), restored.checkpoint().unwrap());
        for (life, start) in [(primary, starts[0]), (extra, starts[1])] {
            let end = game.actor_position(life.actor).unwrap();
            assert!((end.x - start.x - 6.4008 * 0.5).abs() < 0.001);
            let baseline = game.movement_baseline(life).unwrap().unwrap();
            assert_eq!(baseline.applied_sequence, 1);
            assert_eq!(baseline.held.axes(baseline.physics_step), [0.; 2]);
        }
        let stopped = [
            game.actor_position(primary.actor),
            game.actor_position(extra.actor),
        ];
        game.tick(1. / 30., [0.; 2]).unwrap();
        assert_eq!(
            stopped,
            [
                game.actor_position(primary.actor),
                game.actor_position(extra.actor)
            ]
        );
        // Prior spell profiles are refused; omitted optional held input starts stopped.
        let mut old: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        old["rules_revision"] = "verse-chamber-owned-v17".into();
        old["world"]
            .as_object_mut()
            .unwrap()
            .remove("held_movement");
        for player in old["world"]["additional_players"]
            .as_object_mut()
            .unwrap()
            .values_mut()
        {
            player.as_object_mut().unwrap().remove("held_movement");
        }
        assert!(Game::restore(&serde_json::to_vec(&old).unwrap()).is_err());
        old["rules_revision"] = crate::play::RULES_REVISION.into();
        let migrated = Game::restore(&serde_json::to_vec(&old).unwrap()).unwrap();
        assert_eq!(migrated.held_movement.until, 0);
        assert_eq!(
            migrated.additional_players[&extra.actor]
                .held_movement
                .until,
            0
        );
        let mut bad: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        bad["world"]["held_movement"]["until"] = u64::MAX.into();
        assert!(Game::restore(&serde_json::to_vec(&bad).unwrap()).is_err());
    }
    #[test]
    fn held_movement_stops_on_zero_input_handoff_death_and_respawn() {
        let (mut game, extra) = world();
        let primary = game.player_life();
        game.handoff_player(primary, Controller(9)).unwrap();
        for (life, controller) in [(primary, Controller(9)), (extra, Controller(10))] {
            let input = command(
                &game,
                life,
                Intent::Move {
                    axes: [1., 0.],
                    yaw: 0.,
                },
            );
            game.submit(controller, input).unwrap();
        }
        game.tick(1. / 30., [0.; 2]).unwrap();
        let input = command(
            &game,
            extra,
            Intent::Move {
                axes: [0.; 2],
                yaw: 0.,
            },
        );
        game.submit(Controller(10), input).unwrap();
        game.handoff_player(primary, Controller(11)).unwrap();
        let positions = [
            game.actor_position(primary.actor),
            game.actor_position(extra.actor),
        ];
        game.tick(1. / 30., [0.; 2]).unwrap();
        assert_eq!(
            positions,
            [
                game.actor_position(primary.actor),
                game.actor_position(extra.actor)
            ]
        );
        let input = command(
            &game,
            primary,
            Intent::Move {
                axes: [1., 0.],
                yaw: 0.,
            },
        );
        game.submit(Controller(11), input).unwrap();
        game.tick(1. / 30., [0.; 2]).unwrap();
        game.hostile_hit(10000).unwrap();
        game.tick(1. / 30., [0.; 2]).unwrap();
        assert_eq!(game.primary.held_movement.until, 0);
        game.respawn_player().unwrap();
        assert_eq!(game.primary.held_movement.until, 0);
        assert_eq!(game.player_life().generation, primary.generation + 1);
    }

    fn command(g: &Game, life: LifeId, intent: Intent<Ability>) -> Command<Ability> {
        g.player_admission(life.actor)
            .unwrap()
            .command(g.authority_tick, intent)
            .unwrap()
    }
    fn cast(g: &mut Game, life: LifeId, ability: Ability) {
        let position = g.actor_position(life.actor).unwrap();
        let target = g
            .frame()
            .actors
            .into_iter()
            .filter(|a| {
                a.actor.nameplate
                    && a.health > 0
                    && g.attack_clear(position + Vec3::Y * 1.4, a.actor.position + Vec3::Y * 1.1)
            })
            .min_by(|a, b| {
                a.actor
                    .position
                    .distance_squared(position)
                    .total_cmp(&b.actor.position.distance_squared(position))
            })
            .unwrap()
            .life;
        let aim = if ability == Ability::MistyStep {
            [1., 0., 0.]
        } else {
            [0., 0., 1.]
        };
        let c = command(
            g,
            life,
            Intent::Cast {
                ability,
                target,
                aim,
            },
        );
        g.submit(Controller(10), c).unwrap();
    }
    fn steps(g: &mut Game, count: usize) {
        for _ in 0..count {
            g.tick(1. / 30., [0.; 2]).unwrap();
        }
    }
    #[test]
    fn controlled_capsules_jump_stop_at_walls_and_admit_teleport_endpoints() {
        let (mut g, _) = world();
        let life = g
            .add_player(Controller(11), Vec3::new(20., 0., -7.))
            .unwrap();
        steps(&mut g, 1);
        let c = command(&g, life, Intent::Jump);
        g.submit(Controller(11), c).unwrap();
        steps(&mut g, 1);
        assert!(g.actor_position(life.actor).unwrap().y > 0.);
        steps(&mut g, 40);
        for _ in 0..20 {
            let c = command(
                &g,
                life,
                Intent::Move {
                    axes: [0., 1.],
                    yaw: -std::f32::consts::FRAC_PI_2,
                },
            );
            g.submit(Controller(11), c).unwrap();
            steps(&mut g, 1);
        }
        let position = g.actor_position(life.actor).unwrap();
        assert!(position.x <= 21.151 && position.x > 20.);
        let c = command(
            &g,
            life,
            Intent::Cast {
                ability: Ability::MistyStep,
                target: None,
                aim: [1., 0., 0.],
            },
        );
        g.submit(Controller(11), c).unwrap();
        assert!(g.actor_position(life.actor).unwrap().x <= 21.151);
        Game::restore(&g.checkpoint().unwrap()).unwrap();
    }
    #[test]
    fn dead_adventurers_do_not_drift_with_previous_external_motion() {
        let (mut g, life) = world();
        g.primary.character.add_velocity(glam::DVec3::X * 5.);
        g.additional_players
            .get_mut(&life.actor)
            .unwrap()
            .character
            .add_velocity(glam::DVec3::X * 5.);
        let positions = (g.primary.player, g.actor_position(life.actor).unwrap());
        g.hostile_hit(200).unwrap();
        g.hostile_hit_player(life, 200).unwrap();
        steps(&mut g, 15);
        assert_eq!(
            positions,
            (g.primary.player, g.actor_position(life.actor).unwrap())
        );
        Game::restore(&g.checkpoint().unwrap()).unwrap();
    }
    #[test]
    fn foreign_stale_and_handoff_commands_cannot_move_another_adventurer() {
        let (mut g, life) = world();
        let old = command(
            &g,
            life,
            Intent::Move {
                axes: [0., 1.],
                yaw: 0.,
            },
        );
        let before = g.checkpoint().unwrap();
        assert!(g.submit(Controller(11), old.clone()).is_err());
        assert_eq!(before, g.checkpoint().unwrap());
        g.handoff_player(life, Controller(11)).unwrap();
        g.handoff_player(life, Controller(10)).unwrap();
        assert!(g.submit(Controller(10), old).is_err());
        let c = command(
            &g,
            life,
            Intent::Move {
                axes: [0., 1.],
                yaw: std::f32::consts::PI,
            },
        );
        g.submit(Controller(10), c).unwrap();
        let c = command(
            &g,
            g.player_life(),
            Intent::Move {
                axes: [0., 1.],
                yaw: std::f32::consts::PI,
            },
        );
        g.submit(Controller(1), c).unwrap();
        let start = g.primary.player;
        steps(&mut g, 1);
        assert!(g.primary.player.z > start.z);
        assert!(g.actor_position(life.actor).unwrap().z > -22.);
        assert_eq!(g.physics_steps, 4);
        assert_eq!(g.player_path_segments(life.actor), 4);
        Game::restore(&g.checkpoint().unwrap()).unwrap();
    }
    #[test]
    fn both_adventurers_use_the_complete_original_kit_with_independent_shields() {
        for ability in Ability::ALL {
            let (mut g, life) = world();
            cast(&mut g, life, ability);
            steps(&mut g, 40);
            assert!(
                g.encounter
                    .as_ref()
                    .unwrap()
                    .used
                    .contains_key(ability.label())
            );
            assert_eq!(g.snapshot().player.mana, 20);
            Game::restore(&g.checkpoint().unwrap()).unwrap();
        }
        let (mut g, life) = world();
        g.activate(Ability::Shield).unwrap();
        cast(&mut g, life, Ability::Shield);
        assert_eq!(g.hostile_hit_player(life, 45).unwrap(), (27, 18));
        assert_eq!(g.snapshot().player.hp, 200);
        assert_eq!(g.hostile_hit_player(g.player_life(), 45).unwrap(), (27, 18));
        assert_eq!(g.player_snapshot(life).unwrap().player.hp, 173);
        assert_eq!(g.snapshot().player.hp, 173);
        assert_eq!(g.controlled_effects().count(), 2);
    }
    #[test]
    fn movement_interrupts_only_its_casters_windup_and_checkpoints_replay() {
        let (mut g, life) = world();
        cast(&mut g, life, Ability::Fireball);
        let primary_target = g.selected;
        g.activate(Ability::MagicMissile).unwrap();
        assert_eq!(g.selected, primary_target);
        let c = command(
            &g,
            life,
            Intent::Move {
                axes: [1., 0.],
                yaw: 0.,
            },
        );
        g.submit(Controller(10), c).unwrap();
        steps(&mut g, 1);
        assert!(g.additional_players[&life.actor].casting.is_none());
        assert!(g.primary.casting.is_some());
        let bytes = g.checkpoint().unwrap();
        let mut restored = Game::restore(&bytes).unwrap();
        steps(&mut g, 70);
        steps(&mut restored, 70);
        assert_eq!(g.checkpoint().unwrap(), restored.checkpoint().unwrap());
    }
    #[test]
    fn defeat_and_revival_fence_only_the_dead_players_life() {
        let (mut g, life) = world();
        g.hostile_hit(200).unwrap();
        steps(&mut g, 1);
        assert!(g.encounter.as_ref().unwrap().ended.is_none());
        let old = command(
            &g,
            life,
            Intent::Move {
                axes: [0., 1.],
                yaw: 0.,
            },
        );
        g.hostile_hit_player(life, 200).unwrap();
        steps(&mut g, 1);
        assert!(g.encounter.as_ref().unwrap().ended.is_some());
        assert!(g.respawn_controlled_player(Controller(11), life).is_err());
        let next = g.respawn_controlled_player(Controller(10), life).unwrap();
        assert_eq!(next.generation, life.generation + 1);
        assert!(g.submit(Controller(10), old).is_err());
        assert!(g.player_snapshot(life).is_err());
        assert_eq!(g.player_snapshot(next).unwrap().player.hp, 200);
        assert_eq!(g.snapshot().player.hp, 0);
        assert!(g.encounter.as_ref().unwrap().ended.is_none());
        Game::restore(&g.checkpoint().unwrap()).unwrap();
    }
    #[test]
    fn hostile_encounters_retarget_a_surviving_adventurer_and_replay() {
        let (mut g, life) = world_profile(false);
        g.hostile_hit(200).unwrap();
        steps(&mut g, 150);
        assert!(g.player_snapshot(life).unwrap().player.hp < 200);
        assert!(g.events.iter().any(|e| e.actor == Some(life)
            && matches!(e.kind, crate::events::Kind::Damage { incoming: true, .. })));
        let saved = g.checkpoint().unwrap();
        let mut restored = Game::restore(&saved).unwrap();
        steps(&mut g, 30);
        steps(&mut restored, 30);
        assert_eq!(g.checkpoint().unwrap(), restored.checkpoint().unwrap());
    }
    #[test]
    fn shared_reset_keeps_controllers_and_advances_every_player_life() {
        let (mut g, life) = world();
        cast(&mut g, life, Ability::Light);
        let old = command(&g, life, Intent::Jump);
        g.restart_combat(false).unwrap();
        let next = g.player_admission(life.actor).unwrap().actor();
        assert_eq!(next.generation, 1);
        assert_eq!(
            g.player_admission(life.actor).unwrap().controller(),
            Controller(10)
        );
        assert!(g.submit(Controller(10), old).is_err());
        assert_eq!(g.player_snapshot(next).unwrap().player.mana, 20);
        assert!(g.additional_players[&next.actor].controls.light.is_none());
        Game::restore(&g.checkpoint().unwrap()).unwrap();
    }
    #[test]
    fn corrupted_player_checkpoint_and_obstructed_spawn_are_refused() {
        let (mut g, life) = world();
        let before = g.checkpoint().unwrap();
        assert!(g.add_player(Controller(11), g.primary.player).is_err());
        assert_eq!(before, g.checkpoint().unwrap());
        let mut bad: serde_json::Value = serde_json::from_slice(&before).unwrap();
        bad["world"]["additional_players"][life.actor.to_string()]["admission"]["actor"]["instance"] =
            9.into();
        assert!(Game::restore(&serde_json::to_vec(&bad).unwrap()).is_err());
    }
}

#[cfg(test)]
mod vacant_anchor_tests {
    use super::*;
    #[test]
    fn primary_equipment_resources_move_without_leaking_into_the_vacant_anchor() {
        let mut game = super::super::social::tests::game(1951, super::super::social::Zone::Plaza);
        let primary = game.player_life();
        game.equipment_limits(primary.actor, 300, 30).unwrap();
        game.recover_player_resources(primary.actor, 100, 10)
            .unwrap();
        let portable = game.take_resident_player(primary.actor).unwrap();
        assert_eq!(game.snapshot().player.max_hp, 200);
        Game::restore(&game.checkpoint().unwrap()).unwrap();
        let life = game
            .add_player(Controller(10), Vec3::new(-3., 0., -3.))
            .unwrap();
        game.put_transfer_player(life, portable).unwrap();
        let resources = game.player_snapshot(life).unwrap().player;
        assert_eq!(
            (
                resources.hp,
                resources.max_hp,
                resources.mana,
                resources.max_mana
            ),
            (300, 300, 30, 30)
        );
        Game::restore(&game.checkpoint().unwrap()).unwrap();
    }
    #[test]
    fn vacant_primary_anchor_does_not_reduce_the_sixty_four_resident_limit() {
        let mut game = super::super::social::tests::game(1950, super::super::social::Zone::Plaza);
        let primary = game.player_life();
        game.take_resident_player(primary.actor).unwrap();
        for n in 0..64 {
            game.add_player(
                Controller(n + 10),
                Vec3::new(-10. + (n % 8) as f32, 0., -10. + (n / 8) as f32),
            )
            .unwrap();
        }
        assert_eq!(game.controlled_effects().count(), 64);
        assert_eq!(game.simulation.player_ids().count(), 65);
        assert!(game.player_admission(primary.actor).is_none());
        let checkpoint = game.checkpoint().unwrap();
        assert!(
            game.add_player(Controller(80), Vec3::new(-1., 0., -1.))
                .is_err()
        );
        assert_eq!(game.checkpoint().unwrap(), checkpoint);
        let restored = Game::restore(&checkpoint).unwrap();
        assert_eq!(restored.controlled_effects().count(), 64);
        assert!(restored.player_admission(primary.actor).is_none());
        assert!(game.simulation.revive_player(0, [0.; 3], 0.).is_err());
    }
}
