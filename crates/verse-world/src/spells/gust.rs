//! Gust of Wind's chamber adapter.
use crate::{gust as rules, play::Game};
use glam::DVec3;
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Effect {
    pub cast: u64,
    pub caster: u64,
    pub gust: rules::Gust,
}

pub fn cast(game: &mut Game) -> Result<(), String> {
    let caster = game.player_actor();
    let forward = DVec3::new(-f64::from(game.yaw.sin()), 0., -f64::from(game.yaw.cos()));
    let gust = rules::Gust::cast(
        caster as u32,
        game.player.as_dvec3(),
        forward,
        game.time as f64,
    )?;
    let cast = game.spells.begin_cast(caster, true)?;
    game.spells.gusts.push(Effect { cast, caster, gust });
    game.spells.record(
        game.time,
        "Gust of Wind",
        "60 by 10 ft Line; Strength saves on entry and every 6 s".into(),
        None,
    );
    Ok(())
}

pub fn update(game: &mut Game) -> Result<(), String> {
    let creatures: Vec<_> = super::feather_fall::candidates(game)
        .iter()
        .map(|c| rules::Creature {
            id: c.id,
            feet: c.position - DVec3::Y * 0.9,
            strength: 0,
        })
        .collect();
    let mut events = vec![];
    let player = game.player_actor();
    for effect in &mut game.spells.gusts {
        if effect.caster == player {
            effect.gust.follow(game.player.as_dvec3());
        }
        events.extend(
            effect
                .gust
                .flames(game.time as f64, &mut game.spells.flames, &mut || {
                    game.spells.dice.roll(100)
                }),
        );
        events.extend(
            effect
                .gust
                .creatures_with(game.time as f64, &creatures, &mut |id| {
                    game.spells
                        .dice
                        .save(u64::from(id), "Strength", 0, super::SPELL_SAVE_DC)
                        .roll as i32
                }),
        );
    }
    for event in events {
        match event {
            rules::Event::Push {
                creature,
                direction,
                distance,
            } => {
                let actor = u64::from(creature);
                let speed = physics::character::Character::push_speed(distance);
                if let Some(start) = game.actor_position(actor) {
                    game.spells.track(super::Track {
                        label: game.actor_name(actor),
                        target: super::Target::Actor(actor),
                        spell: "Gust of Wind".into(),
                        at: game.time,
                        start: start.as_dvec3(),
                        requested: distance,
                    });
                }
                if let Some(proxy) = game
                    .spells
                    .proxies
                    .iter()
                    .find(|p| p.actor == actor && p.held && !p.ended)
                {
                    let body = proxy.body;
                    let at = game.spells.world[body].pos;
                    let impulse = direction * speed * game.spells.world[body].mass;
                    game.spells.world.wake(body);
                    game.spells.world[body].apply_impulse_at(impulse, at);
                    game.spells
                        .ledger
                        .add_impulse("gust:creature-proxy", impulse, at);
                } else {
                    game.spell_velocity(actor, direction * speed);
                }
            }
            _ => game
                .spells
                .record(game.time, "Gust of Wind", format!("{event:?}"), None),
        }
    }
    Ok(())
}

/// Charge double movement for the component that approaches the caster.
pub fn movement(spells: &super::SpellWorld, feet: DVec3, mut velocity: DVec3) -> DVec3 {
    for effect in &spells.gusts {
        velocity = effect.gust.approach(spells.time, feet, velocity);
    }
    velocity
}
