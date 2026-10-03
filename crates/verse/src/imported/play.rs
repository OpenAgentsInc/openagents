//! Player input after the cinematic handoff, backed by retained Ruins combat.
use glam::Vec3;
use std::collections::BTreeMap;
use verse_ruins::chamber_spells::{Controls, Utility};
use verse_ruins::{Simulation, Snapshot, Spell};
use verse_wow::director::{Action, Frame, Scene};

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
}
impl Ability {
    pub const ALL: [Self; 9] = [
        Self::Bow,
        Self::FireBolt,
        Self::MagicMissile,
        Self::Fireball,
        Self::MistyStep,
        Self::Thunderwave,
        Self::Web,
        Self::Grease,
        Self::Light,
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
        }
    }
    pub fn utility(self) -> Option<Utility> {
        match self {
            Self::MistyStep => Some(Utility::MistyStep),
            Self::Thunderwave => Some(Utility::Thunderwave),
            Self::Web => Some(Utility::Web),
            Self::Grease => Some(Utility::Grease),
            Self::Light => Some(Utility::Light),
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
            | Self::Light => None,
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
    pub ability: Ability,
    pub started: f32,
    pub ends: f32,
    pub origin: Vec3,
    pub direction: Vec3,
}
pub struct Game {
    pub scene: Scene,
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
    pub moving: bool,
    locomotion: [f32; 2],
}
impl Game {
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
        Ok(Self {
            scene,
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
            moving: false,
            locomotion: [0.0; 2],
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
                a.animation_time = self.time;
                a.actor.position = self.player;
                a.actor.yaw = self.yaw;
                a.animation = if self.moving {
                    if self.locomotion[1] < 0.0 { 13 } else { 5 }
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
                    if self.controls.held(*id) {
                        a.animation = 0;
                        a.animation_time = 0.0;
                    }
                    if self.controls.prone(a.actor.position, self.time) {
                        a.animation = 100;
                        a.animation_time = 1.0;
                    }
                    if !source.alive {
                        a.animation = 1;
                        a.animation_time = 0.8;
                    }
                }
            }
        }
        let direction = self.camera.direction();
        let anchor = self.player + Vec3::Y * 1.4;
        frame.eye = anchor - direction * self.camera.distance;
        frame.eye.y = frame.eye.y.max(0.25);
        frame.target = frame.eye + direction * 20.0;
        if self.camera.distance < 0.2 {
            for actor in &mut frame.actors {
                if actor.actor.model == "adventurer" {
                    actor.visible = false;
                }
            }
        }
        frame.projectiles = self
            .arrows
            .iter()
            .map(|a| verse_wow::director::Projectile {
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
        self.time += dt;
        if !self.unlocked() {
            return Ok(());
        }
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
        self.player += delta;
        self.player.x = self.player.x.clamp(-12.0, 12.0);
        self.player.z = self.player.z.clamp(-25.0, 12.0);
        for a in self.scene.frame(self.time).actors {
            if let Some(id) = self.ids.get(&a.actor.id) {
                self.simulation.place_chamber_actor(
                    *id,
                    self.controls
                        .position(*id, a.actor.position, self.time)
                        .to_array(),
                    a.actor.yaw,
                )?;
            }
        }
        let mut remaining = Vec::new();
        for arrow in self.arrows.drain(..) {
            if self.time >= arrow.impact {
                self.simulation.bow_impact(self.ids[&arrow.target], 6)?;
                self.impacts.push((arrow.end, self.time, 0));
            } else {
                remaining.push(arrow);
            }
        }
        self.arrows = remaining;
        if self.casting.as_ref().is_some_and(|c| self.time >= c.ends) {
            let cast = self.casting.take().unwrap();
            self.simulation.cast(
                cast.ability.spell().unwrap(),
                cast.origin.to_array(),
                cast.direction.to_array(),
            )?;
            self.last_cast = Some((cast.ability, self.time));
        }
        self.simulation.tick(dt, self.player.to_array(), self.yaw)?;
        for effect in self.snapshot().effects {
            self.impacts
                .push((effect.pos.into(), self.time, effect.kind));
        }
        self.impacts.retain(|(_, at, _)| self.time - at < 0.6);
        Ok(())
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
            let direction = Vec3::new(-self.yaw.sin(), 0.0, -self.yaw.cos());
            self.player = self.controls.cast(
                &mut self.simulation,
                spell,
                self.time,
                self.player,
                direction,
                target,
            )?;
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
