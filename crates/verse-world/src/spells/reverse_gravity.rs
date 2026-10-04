//! Reverse Gravity's cylinder, fixed-object grabs, and impacts in the chamber.
use crate::{play::Game, reverse_gravity as rules};
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Effect {
    pub cast: u64,
    pub caster: u64,
    pub spell: rules::ReverseGravity,
}

pub fn cast(game: &mut Game) -> Result<(), String> {
    let point = game
        .actor_position(game.selected)
        .unwrap_or(game.player)
        .as_dvec3();
    cast_at(game, point)
}

/// Admit an explicitly chosen ground point through the native spell rules.
pub fn cast_at(game: &mut Game, point: glam::DVec3) -> Result<(), String> {
    let caster = game.player_actor();
    if point.distance(game.player.as_dvec3()) > rules::RANGE {
        return Err("Reverse Gravity point is beyond 100 feet".into());
    }
    let actors: Vec<_> = super::feather_fall::candidates(game)
        .iter()
        .filter(|c| rules::Cylinder::at(point).contains(c.position))
        .map(|c| u64::from(c.id))
        .collect();
    if game.spells.proxies.len() + actors.len() > 256 {
        return Err("Creature proxy budget exceeded".into());
    }
    let cast = game.spells.begin_cast(caster, true)?;
    for actor in &actors {
        super::proxies::add(game, cast, *actor)?;
    }
    let mut spell =
        rules::ReverseGravity::cast(&mut game.spells.world, game.player.as_dvec3(), point)
            .map_err(|e| format!("Reverse Gravity refused: {e:?}"))?;
    let ground = ground(&game.spells.world);
    for actor in actors {
        let body = game
            .spells
            .proxies
            .iter()
            .find(|p| p.actor == actor && p.cast == cast)
            .unwrap()
            .body;
        let save = game
            .spells
            .dice
            .save(actor, "Dexterity", 0, super::SPELL_SAVE_DC);
        let grabbed = spell.grab(
            &mut game.spells.world,
            body,
            rules::Save::new(save.roll as i32, save.modifier, save.dc),
            &ground,
        );
        game.spells.record(
            game.time,
            "Reverse Gravity",
            format!("{}: {grabbed:?}", game.actor_name(actor)),
            Some(save),
        );
    }
    game.spells.reversed.push(Effect {
        cast,
        caster,
        spell,
    });
    Ok(())
}

/// Admit a creature that crosses into a live cylinder after the initial cast.
pub fn entries(game: &mut Game) -> Result<(), String> {
    let candidates = super::feather_fall::candidates(game);
    let effects: Vec<_> = game
        .spells
        .reversed
        .iter()
        .enumerate()
        .filter(|(_, e)| e.spell.active())
        .map(|(i, e)| (i, e.cast, e.spell.gravity.cylinder))
        .collect();
    for (index, cast, cylinder) in effects {
        for candidate in &candidates {
            let actor = u64::from(candidate.id);
            if !cylinder.contains(candidate.position)
                || game
                    .spells
                    .proxies
                    .iter()
                    .any(|p| p.actor == actor && p.cast == cast)
            {
                continue;
            }
            let body = super::proxies::add(game, cast, actor)?;
            let save = game
                .spells
                .dice
                .save(actor, "Dexterity", 0, super::SPELL_SAVE_DC);
            let ground = ground(&game.spells.world);
            let effect = &mut game.spells.reversed[index];
            effect.spell.track(&game.spells.world, body);
            let grabbed = effect.spell.grab(
                &mut game.spells.world,
                body,
                rules::Save::new(save.roll as i32, save.modifier, save.dc),
                &ground,
            );
            game.spells.record(
                game.time,
                "Reverse Gravity",
                format!("Creature {actor} entered: {grabbed:?}"),
                Some(save),
            );
        }
    }
    Ok(())
}
fn ground(world: &physics::World) -> Vec<physics::BodyId> {
    world
        .colliders()
        .iter()
        .filter_map(|c| {
            let physics::Shape::Cuboid { half } = c.shape else {
                return None;
            };
            (world[c.body].kind == physics::BodyKind::Static
                && half.x > half.y
                && half.z > half.y
                && world[c.body].pos.y + half.y <= 0.1)
                .then_some(c.body)
        })
        .collect()
}

/// Replace props broken by ceiling strikes with momentum-preserving chunks.
pub(super) fn break_props(
    spells: &mut super::SpellWorld,
    bodies: &[physics::BodyId],
) -> Result<(), String> {
    if bodies.is_empty() {
        return Ok(());
    }
    let before = spells.boundary_snapshot();
    for body in bodies {
        let Some(index) = spells
            .props
            .iter()
            .position(|p| p.body == *body && !p.removed)
        else {
            continue;
        };
        let original = spells.props[index].clone();
        let mut objects = vec![crate::meteor_swarm::Unattended::new(
            *body,
            Some(0),
            original.spec.flammable,
        )];
        let chunks = crate::meteor_swarm::shatter(&mut spells.world, &mut objects, 0);
        spells.props[index].removed = true;
        for chunk in chunks {
            if spells.props.len() >= super::MAX_PROPS {
                spells.world.remove_body(chunk);
                continue;
            }
            let adopted = spells.adopt_body(
                chunk,
                "Gravity impact debris",
                original.spec.kind,
                None,
                original.life.instance,
            )?;
            spells.props[adopted].spec.material = original.spec.material;
            spells.props[adopted].spec.flammable = original.spec.flammable;
            spells.props[adopted].hit_points = None;
            for effect in &mut spells.reversed {
                if effect.spell.active() {
                    effect.spell.track(&spells.world, chunk);
                }
            }
        }
    }
    spells.record_boundary(&before, "gravity:fracture");
    Ok(())
}

/// Outlines the live cylinder so its extent is visible in the playground.
pub fn guide_lines(game: &Game) -> Vec<(glam::Vec3, glam::Vec3, [f32; 4])> {
    const SEGMENTS: usize = 48;
    let outline = [0.72, 0.55, 1.0, 0.85];
    let faint = [0.72, 0.55, 1.0, 0.35];
    let mut out = vec![];
    for active in game.spells.reversed.iter().filter(|a| a.spell.active()) {
        let c = &active.spell.gravity.cylinder;
        let (base, top) = (c.base.y as f32 + 0.03, c.top() as f32);
        let at = |angle: f32, y: f32| {
            glam::Vec3::new(
                c.base.x as f32 + c.radius as f32 * angle.cos(),
                y,
                c.base.z as f32 + c.radius as f32 * angle.sin(),
            )
        };
        let step = std::f32::consts::TAU / SEGMENTS as f32;
        for i in 0..SEGMENTS {
            let (a, b) = (i as f32 * step, (i + 1) as f32 * step);
            out.push((at(a, base), at(b, base), outline));
            out.push((at(a, top), at(b, top), outline));
            let middle = (base + top) * 0.5;
            out.push((at(a, middle), at(b, middle), faint));
        }
        for i in 0..4 {
            let a = i as f32 * std::f32::consts::FRAC_PI_2;
            out.push((at(a, base), at(a, top), outline));
        }
    }
    out
}
