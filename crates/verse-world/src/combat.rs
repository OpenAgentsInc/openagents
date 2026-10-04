//! Local chamber encounter and an agent controller over the same admitted actions as player input.
use super::play::{Ability, Game};
use glam::Vec3;
use std::collections::BTreeMap;
use verse_engine::director::{Action, Scene};

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct EnemyCast {
    pub actor: u64,
    pub life: verse_engine::core::LifeId,
    pub target_life: verse_engine::core::LifeId,
    pub position: Option<Vec3>,
    pub origin: Vec3,
    pub target: Vec3,
    pub started: f32,
    pub release: f32,
    /// Nominal area arrival; collision can resolve the flight sooner.
    pub impact: f32,
    pub damage: i32,
    pub radius: f32,
    pub boss: bool,
}
#[derive(Default, serde::Serialize, serde::Deserialize)]
pub struct Encounter {
    pub positions: BTreeMap<u64, Vec3>,
    pub casts: Vec<EnemyCast>,
    pub released: BTreeMap<u64, f32>,
    ready: BTreeMap<u64, f32>,
    pub used: BTreeMap<String, u32>,
    pub damage: i32,
    pub absorbed: i32,
    pub dodged: u32,
    pub enemy_casts: u32,
    pub ended: Option<f32>,
    pub enrage: Option<f32>,
    pub next_action: f32,
    pub opening: usize,
    pub actions: usize,
    pub boss_remaining: u32,
    pub boss_max: u32,
    pub kills: u32,
}

impl Game {
    /// Resets this local encounter while preserving command and event fences.
    pub fn restart_combat(&mut self, agent: bool) -> Result<(), String> {
        let mut fresh = Self::combat(self.scene.clone(), agent)?;
        fresh.adopt_restart_fences(self)?;
        fresh.time = fresh.scene.cut_at - if agent { 3. } else { 0. };
        *self = fresh;
        Ok(())
    }
    /// Starts a resettable combat encounter; manual and agent modes share all rules.
    pub fn combat(mut scene: Scene, agent: bool) -> Result<Self, String> {
        scene.duration = 200.0;
        for actor in &mut scene.actors {
            if actor.nameplate {
                actor.health = if actor.model == "claude" { 300_000 } else { 15 };
            }
        }
        // Combat dialogue and attacks replace the staged post-handoff reactions.
        scene
            .cues
            .retain(|cue| cue.at < scene.cut_at || !matches!(cue.action, Action::Yell { .. }));
        scene.cues.push(verse_engine::director::Cue {
            at: scene.cut_at + 0.4,
            actor: 3,
            action: Action::Yell {
                text: "Seal the chamber! Protect our ensouled master!".into(),
                animation: verse_engine::motion::State::Yell.into(),
            },
        });
        let mut game = Self::new(scene)?;
        let mut encounter = Encounter::default();
        for actor in &game.scene.actors {
            if actor.nameplate {
                encounter.positions.insert(actor.id, actor.position);
                encounter.ready.insert(
                    actor.id,
                    game.scene.cut_at + 2.0 + (actor.id % 6) as f32 * 0.8,
                );
            }
        }
        encounter.boss_max = 300_000;
        encounter.boss_remaining = 300_000;
        game.encounter = Some(encounter);
        game.control_handoff(agent)?;
        game.selected = 1;
        Ok(game)
    }
}
impl Encounter {
    pub fn validate(&self, game: &Game) -> Result<(), String> {
        if self.casts.len() > 128
            || self.positions.len() > 256
            || self.ready.len() > 256
            || self.released.len() > 256
        {
            return Err("Encounter checkpoint budget exceeded".into());
        }
        for cast in &self.casts {
            if game.actor_life(cast.actor) != Some(cast.life)
                || game.player_life() != cast.target_life
                || !cast.origin.is_finite()
                || cast.origin.abs().max_element() > 1_000_000.
                || !cast.target.is_finite()
                || cast.target.abs().max_element() > 1_000_000.
                || !cast.started.is_finite()
                || cast.started < 0.
                || cast.started > game.time
                || !cast.release.is_finite()
                || cast.release < cast.started
                || !cast.impact.is_finite()
                || !(0.05..=6.).contains(&(cast.impact - cast.release))
                || !(1..=10_000).contains(&cast.damage)
                || !cast.radius.is_finite()
                || !(0.05..=32.).contains(&cast.radius)
                || cast
                    .position
                    .is_some_and(|p| !p.is_finite() || p.abs().max_element() > 1_000_000.)
                || (cast.position.is_some() && game.time < cast.release)
            {
                return Err("Invalid hostile projectile checkpoint".into());
            }
            if let Some(position) = cast.position {
                let fraction =
                    ((game.time - cast.release) / (cast.impact - cast.release)).clamp(0., 1.);
                let expected = cast.origin.lerp(cast.target + Vec3::Y, fraction);
                if position.distance(expected) > 0.002 {
                    return Err("Hostile projectile position disagrees with its flight".into());
                }
            } else if game.time >= cast.release {
                return Err("Released hostile projectile has no position".into());
            }
        }
        Ok(())
    }

    /// Delays new hostile casts for a bounded local scene fixture.
    pub fn postpone_casts_until(&mut self, until: f32) -> Result<(), String> {
        if !until.is_finite() || !(0. ..=600.).contains(&until) {
            return Err("Invalid hostile cast delay".into());
        }
        for ready in self.ready.values_mut() {
            *ready = ready.max(until);
        }
        Ok(())
    }

    pub fn reset_actor(&mut self, actor: u64, time: f32) {
        self.casts.retain(|c| c.actor != actor);
        self.released.remove(&actor);
        self.ready.insert(actor, time + 2.0);
    }

    pub fn step(&mut self, game: &mut Game, dt: f32) -> Result<(), String> {
        let frame = game.frame();
        let boss = frame
            .actors
            .iter()
            .find(|a| a.actor.model == "claude")
            .ok_or("Missing combat boss")?;
        self.boss_remaining = boss.health;
        self.kills = frame
            .actors
            .iter()
            .filter(|a| a.actor.model.starts_with("cultist") && a.health == 0)
            .count() as u32;
        if self.ended.is_some() {
            return Ok(());
        }
        if game.snapshot().player.hp == 0 || boss.health == 0 {
            self.ended = Some(game.time);
            if game.snapshot().player.hp == 0 {
                game.scene.cues.push(verse_engine::director::Cue {
                    at: game.time,
                    actor: 1,
                    action: Action::Yell {
                        text: "So close, adventurer. Yet the soul persists.".into(),
                        animation: verse_engine::motion::State::Yell.into(),
                    },
                });
            }
            game.casting = None;
            self.casts.clear();
            game.message = if boss.health == 0 {
                "Claude defeated"
            } else {
                "The adventurer has fallen"
            }
            .into();
            return Ok(());
        }
        let enraged = boss.health * 4 < self.boss_max;
        if enraged && self.enrage.is_none() {
            self.enrage = Some(game.time);
            game.scene.cues.push(verse_engine::director::Cue {
                at: game.time,
                actor: 1,
                action: Action::Yell {
                    text: "You almost unmade me. Now face my full wrath!".into(),
                    animation: verse_engine::motion::State::Yell.into(),
                },
            });
        }
        let mut keep = Vec::new();
        for mut cast in self.casts.drain(..) {
            if game.actor_life(cast.actor) != Some(cast.life)
                || game.player_life() != cast.target_life
            {
                continue;
            }
            let source = frame.actors.iter().find(|a| a.actor.id == cast.actor);
            let interrupted = source
                .is_none_or(|a| a.health == 0 || game.controls.prone(a.actor.position, game.time));
            if interrupted && game.time < cast.release {
                continue;
            }
            if game.time < cast.release {
                keep.push(cast);
                continue;
            }
            self.released.insert(cast.actor, cast.release);
            let command_start = game.time - dt;
            let begin = if cast.position.is_none() {
                cast.release
            } else {
                command_start.max(cast.release)
            };
            let end = game.time.min(cast.impact);
            let velocity = (cast.target + Vec3::Y - cast.origin) / (cast.impact - cast.release);
            let duration = (end - begin).max(0.);
            let steps = (duration * 120.).ceil().max(1.) as usize;
            let radius = if cast.boss { 0.18 } else { 0.08 };
            let mut position = cast.position.unwrap_or(cast.origin);
            let mut resolved = false;
            let mut boundaries: Vec<f32> = (0..=steps)
                .map(|step| begin + duration * step as f32 / steps as f32)
                .collect();
            let segments = game.player_motion_segments();
            if segments > 1 {
                boundaries.extend(
                    (1..segments)
                        .map(|step| command_start + dt * step as f32 / segments as f32)
                        .filter(|at| *at > begin && *at < end),
                );
            }
            boundaries.sort_by(f32::total_cmp);
            boundaries.dedup();
            if duration == 0. {
                boundaries.push(begin);
            }
            for window in boundaries.windows(2) {
                let (at, until) = (window[0], window[1]);
                let delta = velocity * (until - at);
                let from = if dt > 0. {
                    (at - command_start) / dt
                } else {
                    1.
                };
                let to = if dt > 0. {
                    (until - command_start) / dt
                } else {
                    1.
                };
                let wall = game.projectile_cover(position, delta, radius)?;
                let hit = physics::continuous::sphere_capsule(
                    position.as_dvec3(),
                    (position + delta).as_dvec3(),
                    radius,
                    game.player_motion_at(from).as_dvec3(),
                    game.player_motion_at(to).as_dvec3(),
                    0.35,
                    1.8,
                )?;
                if hit.is_some_and(|t| wall.is_none_or(|w| t < w)) {
                    let (damage, absorbed) = game.hostile_hit(cast.damage)?;
                    self.damage += damage;
                    self.absorbed += absorbed;
                    resolved = true;
                    break;
                }
                if wall.is_some() {
                    self.dodged += 1;
                    resolved = true;
                    break;
                }
                position += delta;
            }
            if resolved {
                continue;
            }
            cast.position = Some(position);
            if game.time >= cast.impact {
                let fraction = if dt > 0. {
                    (cast.impact - command_start) / dt
                } else {
                    1.
                };
                let player = game.player_motion_at(fraction);
                let delta = player - cast.target;
                if Vec3::new(delta.x, 0., delta.z).length() <= cast.radius
                    && game.attack_clear(position, player + Vec3::Y * 1.4)
                {
                    let (damage, absorbed) = game.hostile_hit(cast.damage)?;
                    self.damage += damage;
                    self.absorbed += absorbed;
                } else {
                    self.dodged += 1;
                }
            } else {
                keep.push(cast);
            }
        }
        self.casts = keep;
        for actor in frame
            .actors
            .iter()
            .filter(|a| a.actor.nameplate && a.health > 0)
        {
            let boss = actor.actor.model == "claude";
            if game.navigation_directed(actor.actor.id) {
                continue;
            }
            let delta = game.player - actor.actor.position;
            let distance = Vec3::new(delta.x, 0.0, delta.z).length();
            let blocked = game.controls.prone(actor.actor.position, game.time);
            // Control effects use combat-store IDs; read their admitted pose through Game.
            let blocked = blocked || game.hostile_held(actor.actor.id);
            let casting = self
                .casts
                .iter()
                .any(|c| c.actor == actor.actor.id && game.time < c.release);
            if !boss
                && !blocked
                && !casting
                && (distance > if actor.actor.id % 3 == 0 { 3.5 } else { 10.0 }
                    || !game.attack_clear(
                        actor.actor.position + Vec3::Y * 1.4,
                        game.player + Vec3::Y * 1.4,
                    ))
            {
                let position = self
                    .positions
                    .get_mut(&actor.actor.id)
                    .ok_or("Missing combat actor")?;
                *position = game.move_hostile(
                    actor.actor.id,
                    *position,
                    game.player,
                    dt * if actor.actor.id % 3 == 0 { 1.8 } else { 0.9 },
                )?;
            }
            if self.casts.len() >= 128
                || blocked
                || casting
                || game.time < self.ready[&actor.actor.id]
            {
                continue;
            }
            let origin = actor.actor.position + Vec3::Y * if boss { 4.0 } else { 1.4 };
            if !game.attack_clear(origin, game.player + Vec3::Y * 1.4) {
                continue;
            }
            let windup = if boss { 1.3 } else { 1.0 };
            let release = game.time + windup;
            self.casts.push(EnemyCast {
                actor: actor.actor.id,
                life: game
                    .actor_life(actor.actor.id)
                    .ok_or("Missing hostile life")?,
                target_life: game.player_life(),
                position: None,
                origin,
                target: game.player,
                started: game.time,
                release,
                impact: release + if boss { 0.65 } else { 0.8 },
                damage: if boss {
                    if enraged { 45 } else { 18 }
                } else {
                    8
                },
                radius: if boss {
                    if enraged { 4.5 } else { 3.2 }
                } else {
                    1.6
                },
                boss,
            });
            self.enemy_casts += 1;
            self.ready.insert(
                actor.actor.id,
                game.time
                    + if boss {
                        if enraged { 2.4 } else { 4.4 }
                    } else {
                        6.5 + (actor.actor.id % 3) as f32
                    },
            );
        }
        Ok(())
    }
}

const OPENING: [Ability; 10] = [
    Ability::Light,
    Ability::Web,
    Ability::Grease,
    Ability::Fireball,
    Ability::Shield,
    Ability::MagicMissile,
    Ability::Bow,
    Ability::FireBolt,
    Ability::Thunderwave,
    Ability::MistyStep,
];
/// Observes health, incoming attacks, distance, mana, and cooldowns before choosing an action.
pub fn drive(game: &mut Game, dt: f32) -> Result<[f32; 2], String> {
    let Some(encounter) = &game.encounter else {
        return Ok([0.0; 2]);
    };
    if encounter.ended.is_some() || game.snapshot().player.hp == 0 {
        return Ok([0.0; 2]);
    }
    let opening = encounter.opening;
    let threatened = encounter
        .casts
        .iter()
        .any(|c| c.impact - game.time < 1.2 && game.player.distance(c.target) < c.radius);
    let ready = game.time >= encounter.next_action;
    let frame = game.frame();
    let closest = frame
        .actors
        .iter()
        .filter(|a| a.actor.nameplate && a.health > 0 && a.actor.model != "claude")
        .min_by(|a, b| {
            a.actor
                .position
                .distance_squared(game.player)
                .total_cmp(&b.actor.position.distance_squared(game.player))
        });
    let target = if opening < 3 || opening == 8 {
        closest
    } else {
        frame
            .actors
            .iter()
            .find(|a| a.actor.model == "claude" && a.health > 0)
    };
    if let Some(target) = target {
        game.selected = target.actor.id;
    }
    let position = target.map_or(Vec3::ZERO, |a| a.actor.position);
    let delta = position - game.player;
    if delta.length_squared() > 0.01 {
        game.yaw = (-delta.x).atan2(-delta.z);
    }
    let camera_delta = (game.yaw - game.camera.yaw + std::f32::consts::PI)
        .rem_euclid(std::f32::consts::TAU)
        - std::f32::consts::PI;
    game.camera.yaw += camera_delta * (dt * 3.0).min(1.0);
    game.camera.pitch = 0.16;
    game.camera.distance = 11.5;
    let distance = Vec3::new(delta.x, 0.0, delta.z).length();
    if game.casting.is_some() {
        return Ok([0.0; 2]);
    }
    if ready {
        let mut candidates = Vec::new();
        if threatened && game.controls.shield <= 0 {
            candidates.push(Ability::Shield);
        }
        if opening < OPENING.len() {
            if OPENING[opening] != Ability::Thunderwave || distance <= 4.0 {
                candidates.push(OPENING[opening]);
            }
        } else {
            if threatened && distance < 8.0 {
                candidates.push(Ability::MistyStep);
            }
            if closest.is_some_and(|a| a.actor.position.distance(game.player) < 4.0) {
                candidates.push(Ability::Thunderwave);
            }
            candidates.extend_from_slice(&[Ability::Fireball, Ability::MagicMissile]);
            candidates.push(if game.encounter.as_ref().unwrap().actions % 3 == 0 {
                Ability::Bow
            } else {
                Ability::FireBolt
            });
        }
        for ability in candidates {
            let previous_aim = (game.selected, game.yaw);
            if ability == Ability::Fireball {
                let cluster = frame
                    .actors
                    .iter()
                    .filter(|a| {
                        a.actor.nameplate
                            && a.health > 0
                            && a.actor.position.distance(game.player) <= 45.72
                            && game.attack_clear(
                                game.player + Vec3::Y * 1.4,
                                a.actor.position + Vec3::Y * 1.1,
                            )
                    })
                    .map(|candidate| {
                        let count = frame
                            .actors
                            .iter()
                            .filter(|other| {
                                other.actor.nameplate
                                    && other.health > 0
                                    && other.actor.model != "claude"
                                    && other.actor.position.distance(candidate.actor.position)
                                        <= 6.096
                                    && game.attack_clear(
                                        candidate.actor.position + Vec3::Y * 1.1,
                                        other.actor.position + Vec3::Y * 1.1,
                                    )
                            })
                            .count();
                        (candidate, count)
                    })
                    .max_by(|(a, ac), (b, bc)| ac.cmp(bc).then(b.actor.id.cmp(&a.actor.id)));
                if let Some((target, _)) = cluster.filter(|(_, count)| *count >= 2) {
                    game.selected = target.actor.id;
                    let delta = target.actor.position - game.player;
                    game.yaw = (-delta.x).atan2(-delta.z);
                }
            }
            if game.activate(ability).is_ok() {
                let e = game.encounter.as_mut().unwrap();
                if opening < OPENING.len() && ability == OPENING[opening] {
                    e.opening += 1;
                }
                e.actions += 1;
                e.next_action = game.time + 0.95;
                return Ok([0.0; 2]);
            }
            (game.selected, game.yaw) = previous_aim;
        }
    }
    let forward = if (opening == 8 && distance > 3.5)
        || (opening < 3 && distance > 12.0)
        || (opening >= 10 && distance > 10.5)
    {
        0.65
    } else {
        0.0
    };
    let strafe = if threatened && forward == 0.0 && game.controls.shield <= 0 {
        if (game.time / 3.0) as u32 % 2 == 0 {
            0.5
        } else {
            -0.5
        }
    } else {
        0.0
    };
    Ok([strafe, forward])
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn moving_cultists_walk_and_death_playback_never_rewinds() {
        let scene =
            Scene::from_json(include_bytes!("../../../assets/verse/original/ritual.json")).unwrap();
        let mut game = Game::combat(scene, true).unwrap();
        for _ in 0..180 {
            game.tick(0.1, [0.0; 2]).unwrap();
        }
        let mut walked = false;
        let mut cast = false;
        let mut deaths = BTreeMap::new();
        let mut saw_death = false;
        for _ in 0..2400 {
            game.tick(1.0 / 30.0, [0.0; 2]).unwrap();
            for a in game
                .frame()
                .actors
                .into_iter()
                .filter(|a| a.actor.model.starts_with("cultist"))
            {
                if game.unlocked()
                    && a.health > 0
                    && game.encounter.as_ref().is_some_and(|e| e.ended.is_none())
                {
                    assert_ne!(
                        a.animation,
                        verse_engine::motion::State::Idle.into(),
                        "Living cultist must keep a combat pose"
                    );
                }
                walked |= a.animation == verse_engine::motion::State::Walk.into();
                cast |= a.animation == verse_engine::motion::State::Cast.into();
                if a.health > 0 {
                    deaths.remove(&a.actor.id);
                }
                if a.animation == verse_engine::motion::State::Death.into() {
                    saw_death = true;
                    assert!(a.visible, "Corpse must survive ECS removal");
                    assert!(!a.actor.nameplate, "Corpse must hide its nameplate");
                    if let Some(previous) = deaths.insert(a.actor.id, a.animation_time) {
                        assert!(a.animation_time >= previous);
                    }
                }
            }
        }
        assert!(walked && cast && saw_death);
    }
    #[test]
    fn floating_numbers_account_for_health_loss() {
        let scene =
            Scene::from_json(include_bytes!("../../../assets/verse/original/ritual.json")).unwrap();
        let mut game = Game::combat(scene, true).unwrap();
        let mut health = BTreeMap::new();
        let mut lost = 0;
        let mut serial = 0;
        let (mut incoming, mut outgoing) = (0, 0);
        for _ in 0..180 {
            game.tick(0.1, [0.0; 2]).unwrap();
        }
        for _ in 0..3600 {
            game.tick(1.0 / 30.0, [0.0; 2]).unwrap();
            for actor in game.snapshot().actors.iter().filter(|a| a.id != 0) {
                let previous = health.insert(actor.id, actor.hp).unwrap_or(actor.max_hp);
                lost += (previous - actor.hp).max(0);
            }
            for number in game.damage_numbers.iter().filter(|n| n.serial > serial) {
                if number.incoming {
                    incoming += number.amount;
                } else {
                    outgoing += number.amount;
                }
            }
            serial = game
                .damage_numbers
                .iter()
                .map(|n| n.serial)
                .max()
                .unwrap_or(serial);
            if game
                .encounter
                .as_ref()
                .unwrap()
                .ended
                .is_some_and(|at| game.time - at >= 5.0)
            {
                break;
            }
        }
        assert_eq!(
            incoming,
            game.snapshot().player.max_hp - game.snapshot().player.hp
        );
        assert_eq!(outgoing, lost);
        let e = game.encounter.as_ref().unwrap();
        assert!(e.ended.is_some());
        assert!(e.used.len() == Ability::ALL.len());
    }
    fn run() -> (Game, u32) {
        let scene =
            Scene::from_json(include_bytes!("../../../assets/verse/original/ritual.json")).unwrap();
        let mut game = Game::combat(scene, true).unwrap();
        for _ in 0..180 {
            game.tick(0.1, [0.0; 2]).unwrap();
        }
        let mut peak_kills = 0;
        for _ in 0..6000 {
            game.tick(1.0 / 30.0, [0.0; 2]).unwrap();
            peak_kills = peak_kills.max(game.encounter.as_ref().unwrap().kills);
            if game.encounter.as_ref().unwrap().ended.is_some() {
                break;
            }
        }
        for _ in 0..150 {
            game.tick(1.0 / 30.0, [0.0; 2]).unwrap();
        }
        (game, peak_kills)
    }
    #[test]
    fn agent_fights_with_the_full_kit_against_high_health_claude() {
        let (game, peak_kills) = run();
        let e = game.encounter.as_ref().unwrap();
        eprintln!(
            "COMBAT time={} hp={} boss={}/{} kills={} damage={} absorbed={} dodged={} abilities={:?}",
            game.time,
            game.snapshot().player.hp,
            e.boss_remaining,
            e.boss_max,
            e.kills,
            e.damage,
            e.absorbed,
            e.dodged,
            e.used
        );
        assert!(e.enemy_casts > 10);
        assert!(
            peak_kills >= 9,
            "The controller should nearly clear the cultists before defeat"
        );
        assert!(e.damage > 0 && e.absorbed > 0 && e.dodged > 0);
        assert!(e.ended.is_some());
        assert_eq!(game.snapshot().player.hp, 0);
        assert!(e.boss_remaining > 0 && e.boss_remaining < e.boss_max);
        for ability in Ability::ALL {
            assert!(
                e.used.contains_key(ability.label()),
                "Missing {}",
                ability.label()
            );
        }
    }
    #[test]
    fn direct_agent_mode_entry_uses_the_same_admitted_combat_rules() {
        let scene =
            Scene::from_json(include_bytes!("../../../assets/verse/original/ritual.json")).unwrap();
        let mut game = Game::combat(scene, true).unwrap();
        game.time = game.scene.cut_at - 3.0;
        for _ in 0..4000 {
            game.tick(1.0 / 30.0, [0.0; 2]).unwrap();
            if game.encounter.as_ref().unwrap().ended.is_some() {
                break;
            }
        }
        let e = game.encounter.as_ref().unwrap();
        assert_eq!(game.snapshot().player.hp, 0);
        assert!(e.boss_remaining > 0 && e.boss_remaining < e.boss_max);
        for ability in Ability::ALL {
            assert!(e.used.contains_key(ability.label()));
        }
        assert!(game.activate(Ability::Shield).is_err());
    }
    #[test]
    fn manual_combat_never_selects_or_casts_for_the_player() {
        let scene =
            Scene::from_json(include_bytes!("../../../assets/verse/original/ritual.json")).unwrap();
        let mut game = Game::combat(scene, false).unwrap();
        for _ in 0..1200 {
            game.tick(1.0 / 30.0, [0.0; 2]).unwrap();
        }
        assert!(game.encounter.as_ref().unwrap().used.is_empty());
        assert!(game.snapshot().player.hp < 200);
    }
    #[test]
    fn original_character_variants_fight_after_the_cinematic_handoff() {
        let mut scene =
            Scene::from_json(include_bytes!("../../../assets/verse/original/ritual.json")).unwrap();
        let variants = [
            "cultist",
            "cultist-female",
            "cultist-peasant",
            "cultist-peasant-female",
        ];
        for actor in &mut scene.actors {
            if actor.model == "cultist" {
                actor.model = variants[actor.id as usize % variants.len()].into();
            }
        }
        let mut game = Game::combat(scene, false).unwrap();
        while game.time < game.scene.cut_at {
            assert!(!game.unlocked());
            game.tick(1.0 / 30.0, [0.0; 2]).unwrap();
            assert_eq!(game.snapshot().player.hp, 200);
        }
        assert!(game.unlocked());
        game.activate(Ability::Shield).unwrap();
        for _ in 0..1200 {
            game.tick(1.0 / 30.0, [0.0; 2]).unwrap();
        }
        let encounter = game.encounter.as_ref().unwrap();
        assert!(encounter.enemy_casts > 0);
        assert!(encounter.absorbed > 0);
        assert!(encounter.damage > 0);
        assert_eq!(game.snapshot().player.hp, 0);
        assert!(encounter.ended.is_some());
        assert_eq!(encounter.used.len(), 1);
        assert_eq!(encounter.boss_max, 300_000);
    }
}

#[cfg(test)]
mod obstruction_tests {
    use super::*;
    fn original_scene() -> Scene {
        Scene::from_json(include_bytes!("../../../assets/verse/original/ritual.json")).unwrap()
    }
    #[test]
    fn cover_blocks_a_released_hostile_impact() {
        let mut game = Game::combat(original_scene(), false).unwrap();
        game.time = 21.;
        game.player = Vec3::new(13., 0., -13.);
        let mut encounter = game.encounter.take().unwrap();
        for ready in encounter.ready.values_mut() {
            *ready = f32::INFINITY;
        }
        encounter.casts.push(EnemyCast {
            actor: 2,
            life: game.actor_life(2).unwrap(),
            target_life: game.player_life(),
            position: None,
            origin: Vec3::new(17., 1.4, -13.),
            target: game.player,
            started: 19.,
            release: 20.,
            impact: 21.,
            damage: 8,
            radius: 1.6,
            boss: false,
        });
        encounter.step(&mut game, 0.1).unwrap();
        assert_eq!(game.snapshot().player.hp, 200);
        assert_eq!(encounter.damage, 0);
        assert_eq!(encounter.dodged, 1);
    }
    #[test]
    fn an_obstructed_cultist_routes_instead_of_casting_through_a_column() {
        let mut scene = original_scene();
        scene
            .actors
            .iter_mut()
            .find(|a| a.model == "adventurer")
            .unwrap()
            .position = Vec3::new(13., 0., -13.);
        scene
            .actors
            .iter_mut()
            .find(|a| a.id == 2)
            .unwrap()
            .position = Vec3::new(17., 0., -13.);
        let mut game = Game::combat(scene, false).unwrap();
        game.time = 20.;
        game.tick(0.1, [0.; 2]).unwrap();
        assert!(
            !game
                .encounter
                .as_ref()
                .unwrap()
                .casts
                .iter()
                .any(|c| c.actor == 2)
        );
        for _ in 0..40 {
            game.tick(0.1, [0.; 2]).unwrap();
        }
        let cultist = game
            .frame()
            .actors
            .into_iter()
            .find(|a| a.actor.id == 2)
            .unwrap();
        assert!((cultist.actor.position.z + 13.).abs() > 1.);
    }
}

#[cfg(test)]
mod hostile_flight_tests {
    use super::*;
    fn game() -> Game {
        let mut g = Game::combat(
            Scene::from_json(include_bytes!("../../../assets/verse/original/ritual.json")).unwrap(),
            false,
        )
        .unwrap();
        g.time = 21.;
        g.player = Vec3::new(0., 0., 0.);
        g
    }
    fn shot(g: &Game) -> EnemyCast {
        EnemyCast {
            actor: 2,
            life: g.actor_life(2).unwrap(),
            target_life: g.player_life(),
            position: None,
            origin: Vec3::new(0., 1.1, -4.),
            target: Vec3::ZERO,
            started: 20.,
            release: 20.5,
            impact: 21.5,
            damage: 8,
            radius: 1.6,
            boss: false,
        }
    }
    #[test]
    fn actual_hostile_flight_hits_before_scheduled_area_time_and_only_once() {
        let mut g = game();
        let mut e = g.encounter.take().unwrap();
        e.postpone_casts_until(100.).unwrap();
        e.casts.push(shot(&g));
        e.step(&mut g, 0.1).unwrap();
        assert_eq!(g.snapshot().player.hp, 200);
        for at in [21.1, 21.2, 21.3, 21.4] {
            g.time = at;
            e.step(&mut g, 0.1).unwrap();
        }
        assert_eq!(g.snapshot().player.hp, 192);
        assert!(e.casts.is_empty());
        g.time = 21.5;
        e.step(&mut g, 0.1).unwrap();
        assert_eq!(g.snapshot().player.hp, 192);
    }
    #[test]
    fn hostile_flight_shield_and_target_life_fences_precede_damage() {
        let mut g = game();
        let mut e = g.encounter.take().unwrap();
        e.postpone_casts_until(100.).unwrap();
        let mut stale = shot(&g);
        stale.target_life.generation += 1;
        e.casts.push(stale);
        e.step(&mut g, 0.1).unwrap();
        assert!(e.casts.is_empty());
        assert_eq!(g.snapshot().player.hp, 200);
        g.activate(Ability::Shield).unwrap();
        e.casts.push(shot(&g));
        e.step(&mut g, 0.1).unwrap();
        for at in [21.1, 21.2, 21.3, 21.4] {
            g.time = at;
            e.step(&mut g, 0.1).unwrap();
        }
        assert_eq!(g.snapshot().player.hp, 200);
        assert_eq!(e.absorbed, 8);
        assert!(e.casts.is_empty());
    }
    #[test]
    fn stored_hostile_position_replays_and_forged_flight_is_refused() {
        let mut g = game();
        let mut e = g.encounter.take().unwrap();
        e.postpone_casts_until(100.).unwrap();
        e.casts.push(shot(&g));
        e.step(&mut g, 0.1).unwrap();
        assert!(e.casts[0].position.is_some());
        g.encounter = Some(e);
        let bytes = g.checkpoint().unwrap();
        let restored = Game::restore(&bytes).unwrap();
        assert_eq!(bytes, restored.checkpoint().unwrap());
        let mut forged: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        forged["world"]["encounter"]["casts"][0]["position"] = serde_json::json!([100., 1., 0.]);
        assert!(Game::restore(&serde_json::to_vec(&forged).unwrap()).is_err());
    }
}
