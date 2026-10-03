//! Local chamber encounter and an agent controller over the same admitted actions as player input.
use super::play::{Ability, Game};
use glam::Vec3;
use std::collections::BTreeMap;
use verse_engine::director::{Action, Scene};

#[derive(Clone, Debug)]
pub struct EnemyCast {
    pub actor: u64,
    pub origin: Vec3,
    pub target: Vec3,
    pub started: f32,
    pub release: f32,
    pub impact: f32,
    pub damage: i32,
    pub radius: f32,
    pub boss: bool,
}
#[derive(Default)]
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
                animation: 60,
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
        game.agent_controlled = agent;
        game.selected = 1;
        Ok(game)
    }
}
impl Encounter {
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
            .filter(|a| a.actor.model == "cultist" && a.health == 0)
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
                        animation: 60,
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
                    animation: 60,
                },
            });
        }
        let mut keep = Vec::new();
        for cast in self.casts.drain(..) {
            let source = frame.actors.iter().find(|a| a.actor.id == cast.actor);
            let interrupted = source
                .is_none_or(|a| a.health == 0 || game.controls.prone(a.actor.position, game.time));
            if interrupted && game.time < cast.release {
                continue;
            }
            if game.time >= cast.release {
                self.released.insert(cast.actor, cast.release);
            }
            if game.time >= cast.impact {
                let delta = game.player - cast.target;
                if Vec3::new(delta.x, 0.0, delta.z).length() <= cast.radius {
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
            let delta = game.player - actor.actor.position;
            let distance = Vec3::new(delta.x, 0.0, delta.z).length();
            let blocked = game.controls.prone(actor.actor.position, game.time);
            // Roots are expressed in retained ECS identifiers; read the source root pose through Game.
            let blocked = blocked || game.hostile_held(actor.actor.id);
            let casting = self
                .casts
                .iter()
                .any(|c| c.actor == actor.actor.id && game.time < c.release);
            if !boss
                && !blocked
                && !casting
                && distance > if actor.actor.id % 3 == 0 { 3.5 } else { 10.0 }
            {
                let position = self
                    .positions
                    .get_mut(&actor.actor.id)
                    .ok_or("Missing combat actor")?;
                *position = game.move_hostile(
                    *position,
                    game.player,
                    dt * if actor.actor.id % 3 == 0 { 1.8 } else { 0.9 },
                )?;
            }
            if blocked || casting || game.time < self.ready[&actor.actor.id] {
                continue;
            }
            let windup = if boss { 1.3 } else { 1.0 };
            let release = game.time + windup;
            self.casts.push(EnemyCast {
                actor: actor.actor.id,
                origin: actor.actor.position + Vec3::Y * if boss { 4.0 } else { 1.4 },
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
            if game.activate(ability).is_ok() {
                let e = game.encounter.as_mut().unwrap();
                if opening < OPENING.len() && ability == OPENING[opening] {
                    e.opening += 1;
                }
                e.actions += 1;
                e.next_action = game.time + 0.95;
                return Ok([0.0; 2]);
            }
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
        let scene = Scene::from_json(include_bytes!(
            "../../../../assets/verse/wow/anthropic.json"
        ))
        .unwrap();
        let mut game = Game::combat(scene, true).unwrap();
        for _ in 0..180 {
            game.tick(0.1, [0.0; 2]).unwrap();
        }
        let mut walked = false;
        let mut cast = false;
        let mut deaths = BTreeMap::new();
        for _ in 0..2400 {
            game.tick(1.0 / 30.0, [0.0; 2]).unwrap();
            for a in game
                .frame()
                .actors
                .into_iter()
                .filter(|a| a.actor.model == "cultist")
            {
                if game.unlocked()
                    && a.health > 0
                    && game.encounter.as_ref().is_some_and(|e| e.ended.is_none())
                {
                    assert_ne!(a.animation, 0, "Living cultist must keep a combat pose");
                }
                walked |= a.animation == 4;
                cast |= a.animation == 52;
                if a.health > 0 {
                    deaths.remove(&a.actor.id);
                }
                if a.animation == 1 {
                    assert!(a.visible, "Corpse must survive ECS removal");
                    assert!(!a.actor.nameplate, "Corpse must hide its nameplate");
                    if let Some(previous) = deaths.insert(a.actor.id, a.animation_time) {
                        assert!(a.animation_time >= previous);
                    }
                }
            }
        }
        assert!(walked && cast && !deaths.is_empty());
    }
    #[test]
    fn floating_numbers_account_for_health_loss() {
        let scene = Scene::from_json(include_bytes!(
            "../../../../assets/verse/wow/anthropic.json"
        ))
        .unwrap();
        let mut game = Game::combat(scene, true).unwrap();
        let initial_enemy_hp: u32 = game
            .scene
            .actors
            .iter()
            .filter(|a| a.nameplate)
            .map(|a| a.health)
            .sum();
        let mut serial = 0;
        let (mut incoming, mut outgoing) = (0, 0);
        for _ in 0..180 {
            game.tick(0.1, [0.0; 2]).unwrap();
        }
        for _ in 0..1800 {
            game.tick(1.0 / 30.0, [0.0; 2]).unwrap();
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
        let remaining: u32 = game
            .frame()
            .actors
            .iter()
            .filter(|a| a.actor.model != "adventurer")
            .map(|a| a.health)
            .sum();
        assert_eq!(incoming, 100 - game.snapshot().player.hp);
        assert_eq!(outgoing, (initial_enemy_hp - remaining) as i32);
        let e = game.encounter.as_ref().unwrap();
        assert!(e.ended.is_some());
        assert!(e.used.len() == Ability::ALL.len());
    }
    fn run() -> Game {
        let scene = Scene::from_json(include_bytes!(
            "../../../../assets/verse/wow/anthropic.json"
        ))
        .unwrap();
        let mut game = Game::combat(scene, true).unwrap();
        for _ in 0..180 {
            game.tick(0.1, [0.0; 2]).unwrap();
        }
        for _ in 0..6000 {
            game.tick(1.0 / 30.0, [0.0; 2]).unwrap();
            if game.encounter.as_ref().unwrap().ended.is_some() {
                break;
            }
        }
        for _ in 0..150 {
            game.tick(1.0 / 30.0, [0.0; 2]).unwrap();
        }
        game
    }
    #[test]
    fn agent_fights_with_the_full_kit_against_high_health_claude() {
        let game = run();
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
        let scene = Scene::from_json(include_bytes!(
            "../../../../assets/verse/wow/anthropic.json"
        ))
        .unwrap();
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
        let scene = Scene::from_json(include_bytes!(
            "../../../../assets/verse/wow/anthropic.json"
        ))
        .unwrap();
        let mut game = Game::combat(scene, false).unwrap();
        for _ in 0..1200 {
            game.tick(1.0 / 30.0, [0.0; 2]).unwrap();
        }
        assert!(game.encounter.as_ref().unwrap().used.is_empty());
        assert!(game.snapshot().player.hp < 100);
    }
}
