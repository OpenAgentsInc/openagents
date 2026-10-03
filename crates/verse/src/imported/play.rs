//! Player input after the cinematic handoff, backed by retained Ruins combat.
use glam::Vec3;
use std::collections::BTreeMap;
use verse_engine::director::{Action, Frame, Scene};
use verse_ruins::chamber_spells::{Controls, Utility};
use verse_ruins::{Simulation, Snapshot, Spell};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
}
impl Ability {
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
            | Self::Shield => None,
            Self::FireBolt => Some(Spell::Firebolt),
            Self::MagicMissile => Some(Spell::MagicMissile),
            Self::Fireball => Some(Spell::Fireball),
        }
    }
}
#[derive(Clone, Debug)]
pub struct Arrow {
    pub start: Vec3,
    pub end: Vec3,
    pub fired: f32,
    pub impact: f32,
    pub target: u64,
}
#[derive(Clone, Debug)]
pub struct Casting {
    pub aim: Vec3,
    pub ability: Ability,
    pub started: f32,
    pub ends: f32,
    pub origin: Vec3,
    pub direction: Vec3,
}
#[derive(Clone, Debug)]
pub struct DamageNumber {
    pub actor: u64,
    pub amount: i32,
    pub at: f32,
    pub position: Vec3,
    pub incoming: bool,
    pub serial: u64,
}
pub struct Game {
    colliders: Vec<physics::kinematic::Aabb>,
    pub scene: Scene,
    pub encounter: Option<super::combat::Encounter>,
    pub agent_controlled: bool,
    pub time: f32,
    pub player: Vec3,
    pub yaw: f32,
    pub camera: super::controls::Camera,
    pub selected: u64,
    pub message: String,
    simulation: Simulation,
    pub controls: Controls,
    ids: BTreeMap<u64, u32>,
    arrows: Vec<Arrow>,
    pub bow_ready: f32,
    pub last_cast: Option<(Ability, f32)>,
    pub casting: Option<Casting>,
    pub impacts: Vec<(Vec3, f32, u8)>,
    pub damage_numbers: Vec<DamageNumber>,
    damage_serial: u64,
    observed_health: BTreeMap<u32, i32>,
    pub moving: bool,
    locomotion: [f32; 2],
    motion_clock: f32,
    npc_motion_clock: BTreeMap<u64, f32>,
    npc_motion: BTreeMap<u64, Vec3>,
    npc_yaw: BTreeMap<u64, f32>,
    npc_deaths: BTreeMap<u64, (f32, Vec3)>,
}
impl Game {
    fn move_player(&self, position: Vec3, delta: Vec3) -> Result<Vec3, String> {
        if self.colliders.is_empty() {
            let mut p = position + delta;
            p.x = p.x.clamp(-12., 12.);
            p.z = p.z.clamp(-25., 12.);
            return Ok(p);
        }
        let center = position.as_dvec3() + glam::DVec3::Y * 0.9;
        let moved = physics::kinematic::move_and_slide(
            center,
            glam::DVec3::new(0.35, 0.9, 0.35),
            delta.as_dvec3(),
            &self.colliders,
        )?;
        Ok((moved - glam::DVec3::Y * 0.9).as_vec3())
    }
    pub(super) fn move_hostile(
        &self,
        position: Vec3,
        target: Vec3,
        distance: f32,
    ) -> Result<Vec3, String> {
        if self.colliders.is_empty() {
            let mut p = position + (target - position).normalize_or_zero() * distance;
            p.x = p.x.clamp(-11., 11.);
            p.z = p.z.clamp(-24., 10.);
            return Ok(p);
        }
        let center = position.as_dvec3() + glam::DVec3::Y * 0.9;
        let goal = glam::DVec3::new(target.x as f64, center.y, target.z as f64);
        let half = glam::DVec3::new(0.35, 0.9, 0.35);
        let Some(waypoint) =
            physics::navigation::next_waypoint(center, goal, half, &self.colliders)?
        else {
            return Ok(position);
        };
        let delta = waypoint - center;
        let movement = delta.normalize_or_zero() * delta.length().min(distance as f64);
        Ok(
            (physics::kinematic::move_and_slide(center, half, movement, &self.colliders)?
                - glam::DVec3::Y * 0.9)
                .as_vec3(),
        )
    }
    pub(super) fn attack_clear(&self, start: Vec3, end: Vec3) -> bool {
        physics::kinematic::sweep_box(
            start.as_dvec3(),
            glam::DVec3::splat(0.12),
            (end - start).as_dvec3(),
            &self.colliders,
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
    /// Uses the retained simulation's cooldown tuning for action-button swipes.
    pub fn cooldown_duration(&self, spell: verse_ruins::Spell) -> f32 {
        self.simulation.cooldown_duration(spell)
    }

    pub fn new(mut scene: Scene) -> Result<Self, String> {
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
        let (simulation, source_ids) = Simulation::chamber(player.to_array(), &targets)?;
        let ids: BTreeMap<_, _> = actors
            .iter()
            .zip(source_ids)
            .map(|(a, id)| (a.id, id))
            .collect();
        let selected = *ids
            .keys()
            .find(|id| **id != 1)
            .or_else(|| ids.keys().next())
            .ok_or("Missing hostile actors")?;
        let observed_health = simulation
            .snapshot()
            .actors
            .iter()
            .map(|a| (a.id, a.hp.max(0)))
            .collect();
        let colliders = match scene.collision_profile.as_deref() {
            None => vec![],
            Some("original-chamber-v1") => super::original::colliders(),
            Some(_) => return Err("Unsupported scene collision profile".into()),
        };
        Ok(Self {
            colliders,
            scene,
            encounter: None,
            agent_controlled: false,
            time: 0.0,
            player,
            yaw: std::f32::consts::PI,
            camera: super::controls::Camera::default(),
            selected,
            message: String::new(),
            simulation,
            controls: Controls::default(),
            ids,
            arrows: vec![],
            bow_ready: 0.0,
            last_cast: None,
            casting: None,
            impacts: vec![],
            damage_numbers: vec![],
            damage_serial: 0,
            observed_health,
            moving: false,
            locomotion: [0.0; 2],
            motion_clock: 0.,
            npc_motion_clock: BTreeMap::new(),
            npc_motion: BTreeMap::new(),
            npc_yaw: BTreeMap::new(),
            npc_deaths: BTreeMap::new(),
        })
    }
    pub fn unlocked(&self) -> bool {
        self.time >= self.scene.cut_at
    }
    pub fn snapshot(&self) -> Snapshot {
        self.simulation.snapshot()
    }
    pub fn frame(&self) -> Frame {
        let mut frame = self.scene.frame(self.time);
        if !self.unlocked() {
            return frame;
        }
        let snapshot = self.snapshot();
        for a in &mut frame.actors {
            if self.time > self.scene.duration {
                a.animation_time = self.time + a.actor.id as f32 * 0.19;
            }
            if a.actor.model == "adventurer" {
                a.animation_time = if self.moving {
                    self.motion_clock
                } else {
                    self.time
                };
                a.actor.position = self.player;
                a.actor.yaw = self.yaw;
                if snapshot.player.hp == 0 {
                    a.animation = 1;
                    a.animation_time = self
                        .encounter
                        .as_ref()
                        .and_then(|e| e.ended)
                        .map_or(0.8, |at| (self.time - at).min(2.0));
                    continue;
                }
                a.animation = if self.moving {
                    if self.scene.collision_profile.is_some()
                        && self.locomotion[0].abs() > self.locomotion[1].abs()
                    {
                        if self.locomotion[0] < 0. { 14 } else { 15 }
                    } else if self.locomotion[1] < 0.0 {
                        13
                    } else {
                        5
                    }
                } else {
                    109
                };
                if let Some(cast) = &self.casting {
                    a.animation = 52;
                    a.animation_time = self.time - cast.started;
                }
                if let Some((ability, at)) = self.last_cast {
                    if self.time - at < 1.0 {
                        a.animation = if ability == Ability::Bow { 46 } else { 53 };
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
                            4
                        } else if a.actor.model.starts_with("cultist") && e.ended.is_none() {
                            if a.actor.id % 3 == 0 { 25 } else { 51 }
                        } else {
                            0
                        };
                        a.animation_time = if a.animation == 4 {
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
                            a.animation = 52;
                            a.animation_time = self.time - cast.started;
                        } else if e
                            .released
                            .get(&a.actor.id)
                            .is_some_and(|at| self.time - at < 0.7)
                        {
                            a.animation = 53;
                            a.animation_time = self.time - e.released[&a.actor.id];
                        }
                        let direction = self.player - a.actor.position;
                        a.actor.yaw = self
                            .npc_yaw
                            .get(&a.actor.id)
                            .copied()
                            .unwrap_or_else(|| (-direction.x).atan2(-direction.z));
                    }
                    if self.controls.held(*id) {
                        a.animation = if a.actor.model.starts_with("cultist")
                            && self.encounter.as_ref().is_some_and(|e| e.ended.is_none())
                        {
                            if a.actor.id % 3 == 0 { 25 } else { 51 }
                        } else {
                            0
                        };
                        a.animation_time = self.time + a.actor.id as f32 * 0.19;
                    }
                    if self.controls.prone(a.actor.position, self.time) {
                        a.animation = 100;
                        a.animation_time = 1.0;
                    }
                    if !source.alive {
                        a.animation = 1;
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
                    a.animation = 1;
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
        let anchor = self.player + Vec3::Y * if self.agent_controlled { 3.0 } else { 1.4 };
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
            .arrows
            .iter()
            .map(|a| verse_engine::director::Projectile {
                position: a.start.lerp(
                    a.end,
                    ((self.time - a.fired) / (a.impact - a.fired)).clamp(0.0, 1.0),
                ),
                direction: (a.end - a.start).normalize(),
            })
            .collect();
        frame
    }
    pub fn tick(&mut self, dt: f32, movement: [f32; 2]) -> Result<(), String> {
        if !dt.is_finite() || !(0.0..=0.1).contains(&dt) || movement.iter().any(|x| !x.is_finite())
        {
            return Err("Invalid play input".into());
        }
        let previous = self
            .encounter
            .as_ref()
            .map(|e| e.positions.clone())
            .unwrap_or_default();
        self.time += dt;
        if !self.unlocked() {
            return Ok(());
        }
        self.respawn_cultists()?;
        let movement = if self.agent_controlled {
            super::combat::drive(self, dt)?
        } else {
            movement
        };
        let dead = self.snapshot().player.hp == 0;
        let movement = if dead { [0.0; 2] } else { movement };
        let forward = Vec3::new(-self.yaw.sin(), 0.0, -self.yaw.cos());
        let right = Vec3::new(-forward.z, 0.0, forward.x);
        let input = Vec3::new(
            movement[0].clamp(-1.0, 1.0),
            0.0,
            movement[1].clamp(-1.0, 1.0),
        );
        let input = input / input.length().max(1.0);
        let speed = if input.z < 0.0 { 4.1148 } else { 6.4008 };
        let delta = (right * input.x + forward * input.z) * dt * speed;
        self.moving = delta.length_squared() > 0.0;
        self.locomotion = movement;
        if self.moving && self.casting.take().is_some() {
            self.message = "Cast interrupted by movement".into();
        }
        let previous_player = self.player;
        self.player = self.move_player(self.player, delta)?;
        let travelled = self.player.distance(previous_player);
        self.motion_clock += travelled / speed;
        self.moving = travelled > 0.00001;
        let source_actors = self.snapshot().actors;
        for a in self.scene.frame(self.time).actors {
            if let Some(id) = self.ids.get(&a.actor.id) {
                if !source_actors.iter().any(|a| a.id == *id) {
                    continue;
                }
                let desired = self.controls.position(
                    *id,
                    self.encounter
                        .as_ref()
                        .and_then(|e| e.positions.get(&a.actor.id))
                        .copied()
                        .unwrap_or(a.actor.position),
                    self.time,
                );
                let position = if self.colliders.is_empty() {
                    desired
                } else {
                    let previous = Vec3::from(
                        source_actors
                            .iter()
                            .find(|actor| actor.id == *id)
                            .unwrap()
                            .pos,
                    );
                    self.move_player(previous, desired - previous)?
                };
                self.simulation
                    .place_chamber_actor(*id, position.to_array(), a.actor.yaw)?;
            }
        }
        let mut remaining = Vec::new();
        for arrow in std::mem::take(&mut self.arrows) {
            if !self.attack_clear(arrow.start, arrow.end) {
                continue;
            }
            if self.time >= arrow.impact {
                if source_actors.iter().any(|a| {
                    a.id == self.ids[&arrow.target]
                        && a.alive
                        && self.attack_clear(arrow.start, Vec3::from(a.pos) + Vec3::Y * 1.1)
                }) {
                    self.simulation.bow_impact(self.ids[&arrow.target], 6)?;
                }
                self.impacts.push((arrow.end, self.time, 0));
            } else {
                remaining.push(arrow);
            }
        }
        self.arrows = remaining;
        if self.casting.as_ref().is_some_and(|c| self.time >= c.ends) {
            let cast = self.casting.take().unwrap();
            if self.attack_clear(cast.origin, cast.aim) {
                self.simulation.cast(
                    cast.ability.spell().unwrap(),
                    cast.origin.to_array(),
                    cast.direction.to_array(),
                )?;
                self.last_cast = Some((cast.ability, self.time));
            } else {
                self.message = "Cast blocked by chamber geometry".into();
            }
        }
        self.simulation.tick(dt, self.player.to_array(), self.yaw)?;
        for effect in self.snapshot().effects {
            self.impacts
                .push((effect.pos.into(), self.time, effect.kind));
        }
        if let Some(mut encounter) = self.encounter.take() {
            encounter.step(self, dt)?;
            self.encounter = Some(encounter);
        }
        if dt > 0.0 {
            if let Some(e) = &self.encounter {
                for (id, position) in &e.positions {
                    *self.npc_motion_clock.entry(*id).or_default() +=
                        position.distance(previous.get(id).copied().unwrap_or(*position)) / 2.4;
                    self.npc_motion.insert(
                        *id,
                        (*position - previous.get(id).copied().unwrap_or(*position)) / dt,
                    );
                }
            }
        }
        let snapshot = self.snapshot();
        if self.encounter.is_some() {
            for actor in &self.scene.actors {
                let Some(source) = self
                    .ids
                    .get(&actor.id)
                    .and_then(|id| snapshot.actors.iter().find(|s| s.id == *id))
                else {
                    continue;
                };
                if !source.alive {
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
                    self.player - Vec3::from(source.pos)
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
                self.damage_number(actor, lost, position, false);
            }
        }
        self.damage_numbers.retain(|n| self.time - n.at < 1.35);
        self.impacts.retain(|(_, at, _)| self.time - at < 0.6);
        Ok(())
    }
    fn respawn_cultists(&mut self) -> Result<(), String> {
        let due: Vec<_> = self
            .scene
            .actors
            .iter()
            .filter(|a| {
                a.model.starts_with("cultist")
                    && self
                        .npc_deaths
                        .get(&a.id)
                        .is_some_and(|(at, _)| self.time - at >= 60.0)
            })
            .cloned()
            .collect();
        for actor in due {
            let old = self.ids[&actor.id];
            let source = self
                .simulation
                .spawn_chamber_actor(actor.position.to_array(), actor.health as i32)?;
            self.ids.insert(actor.id, source);
            self.observed_health.remove(&old);
            self.observed_health.insert(source, actor.health as i32);
            self.controls.forget_actor(old);
            self.npc_deaths.remove(&actor.id);
            self.npc_motion.remove(&actor.id);
            self.npc_motion_clock.remove(&actor.id);
            self.npc_yaw.remove(&actor.id);
            self.arrows.retain(|a| a.target != actor.id);
            self.damage_numbers.retain(|n| n.actor != actor.id);
            if let Some(e) = &mut self.encounter {
                e.positions.insert(actor.id, actor.position);
                e.reset_actor(actor.id, self.time);
            }
        }
        Ok(())
    }
    fn damage_number(&mut self, actor: u64, amount: i32, position: Vec3, incoming: bool) {
        if amount <= 0 {
            return;
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
    }
    fn record_ability(&mut self, ability: Ability) {
        if let Some(e) = &mut self.encounter {
            *e.used.entry(ability.label().into()).or_default() += 1;
        }
    }
    pub fn hostile_held(&self, id: u64) -> bool {
        self.ids
            .get(&id)
            .is_some_and(|source| self.controls.held(*source))
    }
    pub fn hostile_hit(&mut self, damage: i32) -> Result<(i32, i32), String> {
        if self.snapshot().player.hp == 0 {
            return Ok((0, 0));
        }
        let absorbed = self.controls.absorb(damage, self.time);
        let lost = (damage - absorbed).min(self.snapshot().player.hp);
        self.simulation.chamber_player_damage(lost)?;
        if lost > 0 {
            let actor = self
                .scene
                .actors
                .iter()
                .find(|a| a.model == "adventurer")
                .map_or(14, |a| a.id);
            self.damage_number(actor, lost, self.player, true);
            self.impacts.push((self.player + Vec3::Y, self.time, 3));
        }
        Ok((lost, absorbed))
    }
    pub fn cycle_target(&mut self) {
        let live: Vec<_> = self
            .frame()
            .actors
            .iter()
            .filter(|a| a.actor.nameplate && a.health > 0)
            .map(|a| a.actor.id)
            .collect();
        if !live.is_empty() {
            let index = live
                .iter()
                .position(|id| *id == self.selected)
                .map_or(0, |i| (i + 1) % live.len());
            self.selected = live[index];
        }
    }
    pub fn activate(&mut self, ability: Ability) -> Result<(), String> {
        if !self.unlocked() {
            return Err("Wait for the cinematic camera handoff".into());
        }
        if self.snapshot().player.hp <= 0 {
            return Err("The adventurer is dead".into());
        }
        if self.casting.is_some() {
            return Err("A spell is already being cast".into());
        }
        if let Some(spell) = ability.utility() {
            let target = self
                .frame()
                .actors
                .into_iter()
                .find(|a| a.actor.id == self.selected && a.health > 0)
                .map(|a| a.actor.position);
            if matches!(spell, Utility::Web | Utility::Grease)
                && target.is_some_and(|p| {
                    !self.attack_clear(self.player + Vec3::Y * 1.4, p + Vec3::Y * 1.1)
                })
            {
                return Err("Target is blocked by chamber geometry".into());
            }
            let direction = Vec3::new(-self.yaw.sin(), 0.0, -self.yaw.cos());
            let teleport = if spell == Utility::MistyStep && !self.colliders.is_empty() {
                Some(self.move_player(self.player, direction * 9.144)?)
            } else {
                None
            };
            let colliders = &self.colliders;
            let origin = self.player + Vec3::Y * 1.4;
            let destination = self.controls.cast_with_visibility(
                &mut self.simulation,
                spell,
                self.time,
                self.player,
                direction,
                target,
                teleport,
                |position| {
                    physics::kinematic::sweep_box(
                        origin.as_dvec3(),
                        glam::DVec3::splat(0.12),
                        (position + Vec3::Y * 1.1 - origin).as_dvec3(),
                        colliders,
                    )
                    .is_ok_and(|hit| hit.is_none())
                },
            )?;
            self.player = destination;
            self.record_ability(ability);
            self.last_cast = Some((ability, self.time));
            self.message = format!("{}: {}", ability.label(), ability.description());
            return Ok(());
        }
        let target = self
            .frame()
            .actors
            .into_iter()
            .find(|a| a.actor.id == self.selected && a.health > 0)
            .ok_or("Select a living target")?;
        let start = self.player + Vec3::Y * 1.4;
        let end = target.actor.position + Vec3::Y * 1.1;
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
            let state = self.snapshot();
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
                self.simulation.cast(
                    spell,
                    start.to_array(),
                    (end - start).normalize().to_array(),
                )?;
            } else {
                self.casting = Some(Casting {
                    aim: end,
                    ability,
                    started: self.time,
                    ends: self.time + 1.0,
                    origin: start,
                    direction: (end - start).normalize(),
                });
            }
        } else {
            if self.time < self.bow_ready {
                return Err("Bow is cooling down".into());
            }
            self.bow_ready = self.time + 1.0;
            self.arrows.push(Arrow {
                start,
                end,
                fired: self.time,
                impact: self.time + start.distance(end) / 24.0,
                target: self.selected,
            });
        }
        self.record_ability(ability);
        self.last_cast = Some((ability, self.time));
        self.message = ability.label().into();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn game() -> Game {
        Game::new(
            Scene::from_json(include_bytes!(
                "../../../../assets/verse/wow/anthropic.json"
            ))
            .unwrap(),
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
            // Allow the retained ECS to remove the old corpse before respawning.
            for _ in 0..30 {
                g.tick(0.1, [0.0; 2]).unwrap();
            }
            g.time = died + 59.9;
            g.tick(0.0, [0.0; 2]).unwrap();
            assert_eq!(g.ids[&cultist.id], old);
            g.time = died + 60.0;
            g.tick(0.0, [0.0; 2]).unwrap();
            assert_ne!(g.ids[&cultist.id], old);
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
            assert!(alive.actor.nameplate && alive.animation != 1);
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
        g.player = target.actor.position + Vec3::Z;
        g.yaw = 0.0;
        g.activate(Ability::Thunderwave).unwrap();
        g.tick(0.01, [0.0; 2]).unwrap();
        assert!(
            g.damage_numbers
                .iter()
                .any(|n| n.actor == target.actor.id && n.amount == 9 && !n.incoming)
        );
        let count = g.damage_numbers.len();
        g.tick(0.01, [0.0; 2]).unwrap();
        assert_eq!(g.damage_numbers.len(), count);
    }
    #[test]
    fn floating_incoming_damage_excludes_absorption_and_caps_lethal_hits() {
        let mut g = game();
        g.time = 30.0;
        g.controls.shield = 18;
        g.controls.shield_until = 34.0;
        assert_eq!(g.hostile_hit(45).unwrap(), (27, 18));
        assert_eq!(g.damage_numbers.len(), 1);
        assert_eq!(g.damage_numbers[0].amount, 27);
        assert!(g.damage_numbers[0].incoming);
        assert_eq!(g.hostile_hit(1000).unwrap(), (73, 0));
        assert_eq!(g.damage_numbers[1].amount, 73);
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
        let before = g.player;
        g.tick(0.1, [0.0, 1.0]).unwrap();
        assert!((g.player.distance(before) - 6.4008 * 0.1).abs() < 0.001);
        let before = g.player;
        g.tick(0.1, [0.0, -1.0]).unwrap();
        assert!((g.player.distance(before) - 4.1148 * 0.1).abs() < 0.001);
        let before = g.player;
        g.tick(0.1, [1.0, 1.0]).unwrap();
        assert!((g.player.distance(before) - 6.4008 * 0.1).abs() < 0.001);
        let projection = g.frame().view_projection(1280.0 / 720.0);
        let before = g.player;
        g.tick(0.1, [1.0, 0.0]).unwrap();
        assert!(
            (projection * (g.player - before).extend(0.0)).x > 0.0,
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
        g.selected = u64::MAX;
        g.activate(Ability::Light).unwrap();
        assert!(g.controls.light.is_some());
        let before = g.player;
        g.activate(Ability::MistyStep).unwrap();
        assert!(g.player.distance(before) > 9.0);
        assert_eq!(g.snapshot().player.mana, 18);
        assert!(g.activate(Ability::MistyStep).is_err());
        g.selected = 2;
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
            100
        );
        g.player = rooted - Vec3::Z * 3.0;
        let hp = g
            .frame()
            .actors
            .iter()
            .find(|a| a.actor.id == 2)
            .unwrap()
            .health;
        g.activate(Ability::Thunderwave).unwrap();
        assert_eq!(
            g.frame()
                .actors
                .iter()
                .find(|a| a.actor.id == 2)
                .unwrap()
                .health,
            hp - 9
        );
        assert_eq!(g.snapshot().player.hp, 100);
        g.tick(0.1, [0.0; 2]).unwrap();
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
    fn fireball_resolves_against_imported_hostiles() {
        let scene = Scene::from_json(include_bytes!(
            "../../../../assets/verse/wow/anthropic.json"
        ))
        .unwrap();
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
        let scene = Scene::from_json(include_bytes!(
            "../../../../assets/verse/wow/anthropic.json"
        ))
        .unwrap();
        let mut game = Game::new(scene).unwrap();
        for _ in 0..201 {
            game.tick(0.1, [0.0, 0.0]).unwrap();
        }
        game.activate(Ability::Fireball).unwrap();
        assert!(game.casting.is_some());
        game.tick(0.05, [1.0, 0.0]).unwrap();
        assert!(game.casting.is_none());
        assert_eq!(game.snapshot().player.mana, 20);
        let health = game
            .frame()
            .actors
            .iter()
            .find(|a| a.actor.id == game.selected)
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
                .find(|a| a.actor.id == game.selected)
                .unwrap()
                .health,
            health - 6
        );
        game.tick(0.05, [0.0, 0.0]).unwrap();
        assert_eq!(
            game.frame()
                .actors
                .iter()
                .find(|a| a.actor.id == game.selected)
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
        let scene = Scene::from_json(include_bytes!(
            "../../../../assets/verse/original/ritual.json"
        ))
        .unwrap();
        let mut game = Game::new(scene).unwrap();
        game.time = 21.;
        game
    }
    #[test]
    fn original_player_stops_at_column_and_slides_along_its_face() {
        let mut g = game();
        g.player = Vec3::new(13., 0., -13.);
        g.yaw = -std::f32::consts::FRAC_PI_2;
        for _ in 0..20 {
            g.tick(0.1, [0., 1.]).unwrap();
        }
        assert!(g.player.x < 13.951 && g.player.x > 13.94);
        let stopped_clock = g.motion_clock;
        g.tick(0.1, [0., 1.]).unwrap();
        assert!(!g.moving);
        assert!((g.motion_clock - stopped_clock).abs() < 0.00001);
        let slid = g.move_player(g.player, Vec3::new(2., 0., 2.)).unwrap();
        assert!(slid.z > -11.01 && slid.x < 13.951);
    }
    #[test]
    fn owned_locomotion_selects_backward_and_strafe_states_from_actual_motion() {
        let mut g = game();
        g.player = Vec3::new(0., 0., -16.);
        for (movement, clip) in [
            ([0., -1.], 13),
            ([-1., 0.], 14),
            ([1., 0.], 15),
            ([0., 1.], 5),
        ] {
            let previous = g.motion_clock;
            g.tick(0.05, movement).unwrap();
            let player = g
                .frame()
                .actors
                .into_iter()
                .find(|a| a.actor.model == "adventurer")
                .unwrap();
            assert_eq!(player.animation, clip);
            assert!(g.motion_clock > previous);
            assert_eq!(player.animation_time, g.motion_clock);
        }
    }
    #[test]
    fn blink_uses_collision_admitted_destination_and_matching_effect() {
        let mut g = game();
        g.player = Vec3::new(13., 0., -13.);
        g.yaw = -std::f32::consts::FRAC_PI_2;
        g.activate(Ability::MistyStep).unwrap();
        assert!(g.player.x < 13.951 && g.player.x > 13.94);
        assert_eq!(g.controls.areas.last().unwrap().position, g.player);
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
        g.player = Vec3::new(20.8, 0., -7.);
        g.camera.yaw = std::f32::consts::FRAC_PI_2;
        g.camera.pitch = 0.;
        g.camera.distance = 10.;
        let blocked = g.frame();
        assert!(blocked.eye.x < 21.351 && blocked.eye.x > 21.34);
        assert_eq!(g.camera.distance, 10.);
        g.player = Vec3::new(0., 0., -7.);
        let clear = g.frame();
        assert!((clear.eye.x - 10.).abs() < 1e-4);
        assert_eq!(g.camera.distance, 10.);
        let floor = g.camera_eye(Vec3::Y * 1.4, Vec3::new(0., -3., -2.));
        assert!(floor.y >= 0.15 && floor.y < 0.151);
    }
    #[test]
    fn original_hostile_routes_around_a_real_chamber_column() {
        let g = game();
        let mut position = Vec3::new(13., 0., -13.);
        let target = Vec3::new(17., 0., -13.);
        let mut detoured = false;
        for _ in 0..150 {
            position = g.move_hostile(position, target, 0.09).unwrap();
            detoured |= (position.z + 13.).abs() > 1.;
            assert!(!(position.x > 13.95 && position.x < 16.05 && (position.z + 13.).abs() < 1.05));
        }
        assert!(detoured && position.distance(target) < 0.02);
    }
    #[test]
    fn obstructed_player_attacks_do_not_spend_resources_or_start_casts() {
        let mut g = game();
        g.player = Vec3::new(13., 0., -13.);
        g.selected = 2;
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
        assert!(g.casting.is_none());
        assert_eq!(g.bow_ready, 0.);
        assert!(g.arrows.is_empty());
    }
    fn intervening_wall() -> physics::kinematic::Aabb {
        physics::kinematic::Aabb {
            min: glam::DVec3::new(-3., 0., -15.1),
            max: glam::DVec3::new(3., 8., -14.9),
        }
    }
    #[test]
    fn delayed_bow_and_spell_recheck_obstruction() {
        let mut g = game();
        g.activate(Ability::Bow).unwrap();
        g.colliders.push(intervening_wall());
        let hp = g
            .frame()
            .actors
            .into_iter()
            .find(|a| a.actor.id == g.selected)
            .unwrap()
            .health;
        for _ in 0..12 {
            g.tick(0.1, [0.; 2]).unwrap();
        }
        assert_eq!(
            g.frame()
                .actors
                .into_iter()
                .find(|a| a.actor.id == g.selected)
                .unwrap()
                .health,
            hp
        );
        assert!(g.arrows.is_empty());
        let mut g = game();
        g.activate(Ability::MagicMissile).unwrap();
        g.colliders.push(intervening_wall());
        for _ in 0..12 {
            g.tick(0.1, [0.; 2]).unwrap();
        }
        assert_eq!(g.snapshot().player.mana, 20);
        assert_eq!(g.snapshot().counters.casts, 0);
    }
    #[test]
    fn thunderwave_does_not_damage_a_cultist_behind_a_column() {
        let mut g = game();
        g.player = Vec3::new(13., 0., -13.);
        g.yaw = -std::f32::consts::FRAC_PI_2;
        g.selected = 2;
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
