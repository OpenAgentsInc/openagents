//! Controlled adventurers sharing one chamber authority and physics clock.
use super::*;
use crate::{Admission, Command, Controller, Intent};
use verse_engine::core::LifeId;

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Player {
    pub(super) admission: Admission,
    pub(super) source: u32,
    pub(super) character: physics::character::Character,
    pub(super) position: Vec3,
    pub(super) previous: Vec3,
    spawn: Vec3,
    pub(super) yaw: f32,
    pending_move: Option<[f32; 2]>,
    pending_jump: bool,
    pub(super) controls: Controls,
    pub(super) casting: Option<Casting>,
    bow_ready: f32,
    last_cast: Option<(Ability, f32)>,
    motion_clock: f32,
    locomotion: [f32; 2],
    moving: bool,
    died_at: Option<f32>,
    #[serde(skip)]
    trajectory: Vec<[f32; 3]>,
}
impl Player {
    fn new(admission: Admission, source: u32, spawn: Vec3) -> Self {
        Self {
            admission,
            source,
            character: physics::character::Character::new(spawn.as_dvec3()),
            position: spawn,
            previous: spawn,
            spawn,
            yaw: std::f32::consts::PI,
            pending_move: None,
            pending_jump: false,
            controls: Controls::default(),
            casting: None,
            bow_ready: 0.,
            last_cast: None,
            motion_clock: 0.,
            locomotion: [0.; 2],
            moving: false,
            died_at: None,
            trajectory: vec![],
        }
    }
    pub(super) fn frame(
        &self,
        frame: &mut verse_engine::director::ActorFrame,
        hp: u32,
        time: f32,
        collision: bool,
    ) {
        frame.actor.position = self.position;
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
impl Game {
    /// Binds a trusted controller to a new adventurer in this same world.
    /// A network host must admit membership and select the spawn before calling this.
    pub fn add_player(&mut self, controller: Controller, spawn: Vec3) -> Result<LifeId, String> {
        if self.additional_players.len() >= 63
            || self.scene.actors.len() >= 256
            || self.bodies.records().count() >= 1024
            || !spawn.is_finite()
            || spawn.abs().max_element() > 10_000.
        {
            return Err("Controlled player admission budget exceeded".into());
        }
        let mut actor = self.next_player_actor;
        while self.scene.actors.iter().any(|a| a.id == actor)
            || self.bodies.records().any(|r| r.life.entity == actor)
        {
            actor = actor.checked_add(1).ok_or("Player actor IDs exhausted")?;
        }
        let next = actor.checked_add(1).ok_or("Player actor IDs exhausted")?;
        if next >= 1_000_000 {
            return Err("Player actor IDs exhausted".into());
        }
        let life = LifeId {
            instance: self.player_life().instance,
            actor,
            generation: 0,
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
        let source = self.simulation.spawn_player(spawn.to_array())?;
        appearance.id = actor;
        appearance.position = spawn;
        appearance.name = format!("Adventurer {actor}");
        appearance.nameplate = false;
        appearance.health = 200;
        self.scene.actors.push(appearance);
        let mut player = Player::new(Admission::new(life, controller), source, spawn);
        player.character = character;
        self.additional_players.insert(actor, player);
        self.next_player_actor = next;
        self.sync_bodies(0.)?;
        Ok(life)
    }
    pub fn player_admission(&self, actor: u64) -> Option<&Admission> {
        if actor == self.player_actor() {
            Some(&self.admission)
        } else {
            self.additional_players.get(&actor).map(|p| &p.admission)
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
    /// Extracts HUD values for this exact controlled life without exposing authority.
    pub fn player_hud(&self, life: LifeId) -> Result<crate::hud::Own, String> {
        let snapshot = self.player_snapshot(life)?;
        let (controls, bow_ready, casting) = if life == self.player_life() {
            (&self.controls, self.bow_ready, self.casting.as_ref())
        } else {
            let p = &self.additional_players[&life.actor];
            (&p.controls, p.bow_ready, p.casting.as_ref())
        };
        Ok(crate::hud::Own::extract(
            self, life, snapshot, controls, bow_ready, casting,
        ))
    }
    pub(super) fn player_source(&self, actor: u64) -> Option<u32> {
        if actor == self.player_actor() {
            Some(0)
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
            self.admission
                .handoff(controller)
                .map_err(|e| format!("Control handoff refused: {e:?}"))?;
            self.agent_controlled = controller == Controller(2);
            self.pending_movement = None;
            self.pending_jump = false;
        } else {
            let p = self.additional_players.get_mut(&life.actor).unwrap();
            p.admission
                .handoff(controller)
                .map_err(|e| format!("Control handoff refused: {e:?}"))?;
            p.pending_move = None;
            p.pending_jump = false;
        }
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
        if let Intent::Cast {
            aim,
            target,
            ability,
        } = &command.intent
        {
            if aim[1].abs() > 0.001
                || target.is_some_and(|life| self.lives.get(&life.actor) != Some(&life))
            {
                return Err("Invalid spell target or horizontal aim".into());
            }
            if matches!(ability, Ability::Spell(_)) {
                return Err("This catalog spell needs a shared-caster adapter".into());
            }
        }
        self.additional_players
            .get_mut(&actor)
            .unwrap()
            .admission
            .admit(controller, &command, self.authority_tick)
            .map_err(|e| format!("Command refused: {e:?}"))?;
        let mut p = self.additional_players.remove(&actor).unwrap();
        let result = match command.intent {
            Intent::Move { axes, yaw } => {
                p.pending_move = Some(axes);
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
                self.activate_player(&mut p, ability, target)
            }
        };
        self.additional_players.insert(actor, p);
        result
    }
    fn activate_player(
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
        let start = p.position + Vec3::Y * 1.4;
        if let Some(spell) = ability.utility() {
            if matches!(spell, Utility::Web | Utility::Grease)
                && target_position.is_some_and(|pos| !self.attack_clear(start, pos + Vec3::Y * 1.1))
            {
                return Err("Target is blocked by chamber geometry".into());
            }
            let teleport = if spell == Utility::MistyStep {
                let destination = if self.colliders.is_empty() {
                    p.position + direction * 9.144
                } else {
                    physics::character::slide(
                        &self.query_scene,
                        self.actor_filter(p.admission.actor()),
                        physics::character::Settings::default(),
                        p.position.as_dvec3(),
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
                p.position,
                direction,
                target_position,
                teleport,
                |_| true,
            )?;
            if spell == Utility::Thunderwave {
                crate::spells::thunderwave::resolve_for(
                    self,
                    p.admission.actor().actor,
                    p.position,
                    direction,
                )?;
            }
            if spell == Utility::MistyStep {
                p.position = destination;
                p.previous = destination;
                p.character = physics::character::Character::new(destination.as_dvec3());
                p.trajectory.clear();
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
                let axes = if dead {
                    p.pending_move = None;
                    [0.; 2]
                } else {
                    p.pending_move.take().unwrap_or([0.; 2])
                };
                let forward = Vec3::new(-p.yaw.sin(), 0., -p.yaw.cos());
                let right = Vec3::new(-forward.z, 0., forward.x);
                let input = Vec3::from([axes[0], 0., axes[1]]);
                let input = input / input.length().max(1.);
                let speed = if input.z < 0. { 4.1148 } else { 6.4008 };
                let velocity = (right * input.x + forward * input.z) * speed;
                if dead || velocity.length_squared() > 0. {
                    p.casting = None;
                }
                p.previous = p.position;
                p.locomotion = axes;
                let jump = std::mem::take(&mut p.pending_jump) && !dead;
                let mut fell = 0.;
                p.trajectory = vec![p.position.to_array()];
                if dead {
                    p.trajectory.push(p.position.to_array());
                } else if self.colliders.is_empty() {
                    p.position += velocity * dt;
                    p.position.x = p.position.x.clamp(-12., 12.);
                    p.position.z = p.position.z.clamp(-25., 12.);
                    p.character = physics::character::Character::new(p.position.as_dvec3());
                    p.trajectory.push(p.position.to_array());
                } else {
                    for step in 0..steps {
                        p.character.step(
                            &self.query_scene,
                            self.actor_filter(p.admission.actor()),
                            physics::character::Settings::default(),
                            velocity.as_dvec3(),
                            jump && step == 0,
                            self.physics_clock.dt,
                        )?;
                        fell += p.character.landed.unwrap_or(0.);
                        p.trajectory.push(p.character.feet.as_vec3().to_array());
                    }
                    p.position = p.character.feet.as_vec3();
                }
                let distance = p.position.distance(p.previous);
                p.moving = distance > 0.00001;
                p.motion_clock += distance / speed;
                self.simulation
                    .place_chamber_actor(p.source, p.position.to_array(), p.yaw)?;
                if p.trajectory.len() >= 2 {
                    self.simulation
                        .record_motion_path(p.source, p.trajectory.clone())?;
                }
                self.place_actor_body(p.admission.actor(), p.position, dt as f64)?;
                if !dead && fell > 0. {
                    let dice = crate::spells::fall_dice(fell);
                    if dice > 0 {
                        let amount = self.spells.dice.sum(dice, 6) as i32;
                        let hp = self.simulation.snapshot_for(p.source)?.player.hp;
                        let loss = amount.min(hp);
                        self.simulation.player_damage_for(p.source, loss)?;
                        self.player_damage_event(p.admission.actor(), p.source, loss, p.position)?;
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
        if self
            .simulation
            .player_resources(0)
            .is_some_and(|p| p.hp > 0)
        {
            result.push((self.player_life(), self.player));
        }
        result.extend(
            self.additional_players
                .values()
                .filter(|p| {
                    self.simulation
                        .player_resources(p.source)
                        .is_some_and(|p| p.hp > 0)
                })
                .map(|p| (p.admission.actor(), p.position)),
        );
        result
    }
    pub(crate) fn player_path_segments(&self, actor: u64) -> usize {
        if actor == self.player_actor() {
            self.player_motion_segments()
        } else {
            self.additional_players
                .get(&actor)
                .map_or(0, |p| p.trajectory.len().saturating_sub(1))
        }
    }
    pub(crate) fn player_path_at(&self, actor: u64, fraction: f32) -> Vec3 {
        if actor == self.player_actor() {
            return self.player_motion_at(fraction);
        }
        let p = &self.additional_players[&actor];
        if p.trajectory.len() < 2 {
            return p.position;
        }
        let segment = fraction.clamp(0., 1.) * (p.trajectory.len() - 1) as f32;
        let index = (segment.floor() as usize).min(p.trajectory.len() - 2);
        Vec3::from(p.trajectory[index])
            .lerp(Vec3::from(p.trajectory[index + 1]), segment - index as f32)
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
        let (source, position) = (p.source, p.position);
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
        let mut character = physics::character::Character::new(p.spawn.as_dvec3());
        if !self.colliders.is_empty() {
            character.teleport(
                &self.query_scene,
                self.actor_filter(life),
                physics::character::Settings::default(),
                p.spawn.as_dvec3(),
            )?;
        }
        let physical = physics::queries::Life {
            instance: life.instance,
            entity: life.actor,
            generation: life.generation,
        };
        let (source, spawn) = (p.source, p.spawn);
        let mut blockers = self.blockers.clone();
        if blockers.remove(physical)? && self.navigation.is_some() {
            self.replace_blockers(blockers)?;
        }
        self.simulation
            .revive_player(source, spawn.to_array(), std::f32::consts::PI)?;
        self.bodies.remove(physical);
        self.spells.end_concentration(life.actor)?;
        let next = admission.actor();
        let mut p = Player::new(admission, source, spawn);
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
    pub(crate) fn shared_prone(&self, position: Vec3) -> bool {
        self.controls.prone(position, self.time)
            || self
                .additional_players
                .values()
                .any(|p| p.controls.prone(position, self.time))
    }
    /// Resolves a combat-store caster ID to its exact current controlled life.
    pub fn projectile_caster_life(&self, source: u32) -> Option<LifeId> {
        if source == 0 {
            Some(self.player_life())
        } else {
            self.additional_players
                .values()
                .find(|p| p.source == source)
                .map(|p| p.admission.actor())
        }
    }
    /// Read-only effect projection for every controlled adventurer.
    pub fn controlled_effects(&self) -> impl Iterator<Item = (LifeId, Vec3, &Controls)> + '_ {
        std::iter::once((self.player_life(), self.player, &self.controls)).chain(
            self.additional_players
                .values()
                .map(|p| (p.admission.actor(), p.position, &p.controls)),
        )
    }
    pub(crate) fn rebuild_players_after_restart(&mut self, previous: &Game) -> Result<(), String> {
        for (actor, p) in &previous.additional_players {
            let mut admission = p.admission.clone();
            admission
                .respawn()
                .map_err(|e| format!("Restart player refused: {e:?}"))?;
            let source = self.simulation.spawn_player(p.spawn.to_array())?;
            self.additional_players
                .insert(*actor, Player::new(admission, source, p.spawn));
        }
        self.next_player_actor = previous.next_player_actor;
        self.sync_bodies(0.)?;
        Ok(())
    }
    pub(super) fn validate_players(&self) -> Result<(), String> {
        if self.additional_players.len() > 63 || self.next_player_actor >= 1_000_000 {
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
            p.character.validate()?;
            p.controls.validate()?;
            if *actor >= self.next_player_actor
                || p.admission.actor().actor != *actor
                || p.admission.actor().instance != self.player_life().instance
                || *actor == self.player_actor()
                || self.lives.contains_key(actor)
                || !sources.insert(p.source)
                || !p.position.is_finite()
                || !p.previous.is_finite()
                || p.previous.abs().max_element() > 10_000.
                || !p.spawn.is_finite()
                || p.spawn.abs().max_element() > 10_000.
                || p.position.abs().max_element() > 10_000.
                || p.character.feet.as_vec3() != p.position
                || !p.yaw.is_finite()
                || !p.motion_clock.is_finite()
                || p.motion_clock < 0.
                || !p.bow_ready.is_finite()
                || p.bow_ready < 0.
                || p.pending_move
                    .is_some_and(|a| a.iter().any(|v| !v.is_finite() || v.abs() > 1.))
                || p.locomotion.iter().any(|v| !v.is_finite() || v.abs() > 1.)
                || p.died_at
                    .is_some_and(|at| !at.is_finite() || at < 0. || at > self.time)
                || p.last_cast.is_some_and(|(a, at)| {
                    matches!(a, Ability::Spell(_)) || !at.is_finite() || at < 0. || at > self.time
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
                    .is_none_or(|a| Vec3::from(a.pos) != p.position)
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
        g.character.add_velocity(glam::DVec3::X * 5.);
        g.additional_players
            .get_mut(&life.actor)
            .unwrap()
            .character
            .add_velocity(glam::DVec3::X * 5.);
        let positions = (g.player, g.actor_position(life.actor).unwrap());
        g.hostile_hit(200).unwrap();
        g.hostile_hit_player(life, 200).unwrap();
        steps(&mut g, 15);
        assert_eq!(positions, (g.player, g.actor_position(life.actor).unwrap()));
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
        let start = g.player;
        steps(&mut g, 1);
        assert!(g.player.z > start.z);
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
        assert!(g.casting.is_some());
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
        assert!(g.add_player(Controller(11), g.player).is_err());
        assert_eq!(before, g.checkpoint().unwrap());
        let mut bad: serde_json::Value = serde_json::from_slice(&before).unwrap();
        bad["world"]["additional_players"][life.actor.to_string()]["admission"]["actor"]["instance"] =
            9.into();
        assert!(Game::restore(&serde_json::to_vec(&bad).unwrap()).is_err());
    }
}
