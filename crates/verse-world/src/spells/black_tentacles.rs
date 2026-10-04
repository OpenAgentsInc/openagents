//! Articulated Black Tentacles in the chamber's shared rigid world.
use crate::{black_tentacles as rules, play::Game};
use glam::DVec3;
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Effect {
    pub cast: u64,
    pub caster: u64,
    pub spell: rules::BlackTentacles,
}

pub fn cast(game: &mut Game) -> Result<(), String> {
    let point = game
        .actor_position(game.selected)
        .unwrap_or(game.player + glam::Vec3::new(-game.yaw.sin(), 0., -game.yaw.cos()) * 6.)
        .as_dvec3();
    cast_at(game, point)
}

/// Admit an explicitly chosen ground point through the native spell rules.
pub fn cast_at(game: &mut Game, point: glam::DVec3) -> Result<(), String> {
    let center = rules::place(game.player.as_dvec3(), point, |x, z| {
        super::wind_wall::ground(&game.spells.world, glam::DVec2::new(x, z))
    })
    .map_err(|e| format!("Black Tentacles refused: {e:?}"))?;
    if game.spells.props.len() + rules::GRID * rules::GRID * rules::SEGMENTS > super::MAX_PROPS {
        return Err("Tentacle body budget exceeded".into());
    }
    let caster = game.player_actor();
    let actors: Vec<_> = super::feather_fall::candidates(game)
        .iter()
        .map(|c| u64::from(c.id))
        .collect();
    if game.spells.proxies.len() + actors.len() > 256 {
        return Err("Creature proxy budget exceeded".into());
    }
    let cast = game.spells.begin_cast(caster, true)?;
    let mut targets = vec![];
    for actor in actors {
        let body = super::proxies::add(game, cast, actor)?;
        game.spells.proxies.last_mut().unwrap().held = false;
        targets.push(rules::Target {
            body,
            kind: rules::Kind::Creature {
                strength: 0,
                athletics: 0,
            },
        });
    }
    targets.extend(
        game.spells
            .props
            .iter()
            .filter(|p| !p.removed && p.spec.kind != super::PropKind::Tentacle)
            .map(|p| rules::Target {
                body: p.body,
                kind: rules::Kind::Prop {
                    secured: p.spec.secured,
                },
            }),
    );
    let identities: std::collections::BTreeMap<_, _> = game
        .spells
        .proxies
        .iter()
        .map(|p| (p.body, p.actor))
        .collect();
    let (spell, events) = rules::BlackTentacles::cast_with(
        &mut game.spells.world,
        center,
        DVec3::new(0., -super::GRAVITY, 0.),
        super::SPELL_SAVE_DC,
        game.spells.dice.seed,
        &targets,
        &mut |body, sides| {
            if let Some(actor) = body.and_then(|b| identities.get(&b)) {
                game.spells
                    .dice
                    .save(*actor, "Strength", 0, super::SPELL_SAVE_DC)
                    .roll
            } else {
                game.spells.dice.roll(sides)
            }
        },
    );
    for event in events {
        record(&mut game.spells, event);
    }
    let instance = game.player_life().instance;
    for tentacle in &spell.tentacles {
        for body in &tentacle.segments {
            game.spells.adopt_body(
                *body,
                "Tentacle segment",
                super::PropKind::Tentacle,
                Some(cast),
                instance,
            )?;
        }
    }
    game.spells.tentacles.push(Effect {
        cast,
        caster,
        spell,
    });
    Ok(())
}

pub fn record(spells: &mut super::SpellWorld, event: rules::Event) {
    if let rules::Event::Save {
        body,
        damage: Some(damage),
        ..
    } = event
    {
        if let Some(proxy) = spells.proxies.iter().find(|p| p.body == body) {
            spells
                .damage
                .push((proxy.actor, damage, "Black Tentacles".into()));
        }
    }
    spells.record(
        spells.time as f32,
        "Black Tentacles",
        format!("{event:?}"),
        None,
    );
}

/// Resolve one creature's Athletics action against its current restraint.
pub fn escape(game: &mut Game, actor: u64) -> Result<(), String> {
    if game
        .spells
        .escape_ready
        .get(&actor)
        .is_some_and(|at| *at > game.time as f64)
    {
        return Err("Escape action is cooling down".into());
    }
    let body = game
        .spells
        .proxies
        .iter()
        .find(|p| p.actor == actor && p.held && !p.ended)
        .ok_or("Creature is not Restrained")?
        .body;
    let dc = game
        .spells
        .tentacles
        .iter()
        .find(|e| e.spell.restrained(body))
        .ok_or("No tentacle restraint")?
        .spell
        .dc;
    let save = game.spells.dice.save(actor, "Athletics", 0, dc);
    let event = game
        .spells
        .tentacles
        .iter_mut()
        .find_map(|e| {
            e.spell
                .escape(&mut game.spells.world, body, save.roll as i32, 0)
        })
        .ok_or("No tentacle restraint")?;
    game.spells
        .escape_ready
        .insert(actor, game.time as f64 + super::ROUND as f64);
    game.spells.record(
        game.time,
        "Black Tentacles",
        format!("Athletics escape: {event:?}"),
        Some(save),
    );
    Ok(())
}

/// Movement stops while restrained and slows over the tentacle square.
pub fn speed_scale(spells: &super::SpellWorld, actor: u64, feet: DVec3) -> f64 {
    if spells
        .proxies
        .iter()
        .any(|p| p.actor == actor && spells.tentacles.iter().any(|e| e.spell.restrained(p.body)))
    {
        return 0.;
    }
    spells
        .tentacles
        .iter()
        .map(|e| e.spell.speed_scale(feet))
        .fold(1., f64::min)
}
