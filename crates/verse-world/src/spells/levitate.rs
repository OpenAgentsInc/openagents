//! Levitate on the chamber's characters and loose rigid props.
use super::{SpellWorld, Target, Track};
use crate::{levitate as rules, play::Game};
use glam::DVec3;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Effect {
    pub cast: u64,
    pub caster: u64,
    pub caster_position: DVec3,
    pub target: Target,
    pub state: rules::Levitation,
}

pub fn cast(game: &mut Game) -> Result<(), String> {
    let target = if game.selected >= super::PROP_ENTITY_BASE {
        Target::Prop(
            game.spells
                .props
                .iter()
                .position(|p| p.life.entity == game.selected && !p.removed)
                .ok_or("Unknown Levitate prop")?,
        )
    } else if game.actor_character(game.selected).is_some() {
        Target::Actor(game.selected)
    } else {
        Target::Actor(game.player_actor())
    };
    cast_on(game, target)
}

pub fn cast_on(game: &mut Game, target: Target) -> Result<(), String> {
    let caster = game.player_actor();
    let caster_position = game.player.as_dvec3();
    let (position, subject) = match target {
        Target::Actor(actor) => {
            if game
                .scene
                .actors
                .iter()
                .any(|a| a.id == actor && a.friendly)
            {
                return Err("Friendly scene creatures cannot be spell targets".into());
            }
            let mass = super::model_size(game.actor_model(actor)).creature_mass();
            if mass > rules::WEIGHT_LIMIT {
                return Err(rules::Refusal::TooHeavy { mass }.reason());
            }
            (
                game.actor_position(actor)
                    .ok_or("Unknown Levitate target")?
                    .as_dvec3(),
                rules::Subject::Creature {
                    willing: actor == caster,
                    constitution: super::constitution_modifier(game.actor_model(actor)),
                },
            )
        }
        Target::Prop(index) => {
            let prop = game
                .spells
                .props
                .get(index)
                .ok_or("Unknown Levitate prop")?;
            if prop.removed {
                return Err("Levitate prop is removed".into());
            }
            (
                game.spells.prop_center(index),
                rules::Subject::Object {
                    mass: prop.spec.mass,
                    secured: prop.spec.secured,
                },
            )
        }
    };
    if physics::kinematic::sweep_box(
        caster_position + DVec3::Y * 1.4,
        DVec3::splat(0.01),
        position + DVec3::Y * 0.9 - caster_position - DVec3::Y * 1.4,
        &game.colliders,
    )?
    .is_some()
    {
        return Err("Levitate target is behind cover".into());
    }
    let target_id = match target {
        Target::Actor(actor) => actor,
        Target::Prop(index) => game.spells.props[index].life.entity,
    };
    let constitution = match subject {
        rules::Subject::Creature { constitution, .. } => constitution,
        _ => 0,
    };
    let mut save = None;
    let admission = rules::admit(subject, position.distance(caster_position), || {
        let rolled = game.spells.dice.save(
            target_id,
            "Constitution",
            constitution,
            super::SPELL_SAVE_DC,
        );
        let roll = rolled.roll as i32;
        save = Some(rolled);
        roll
    });
    if let Err(rules::Refusal::Saved(_)) = admission {
        game.spells.record(
            game.time,
            "Levitate",
            "Constitution save succeeds; no movement".into(),
            save,
        );
        return Ok(());
    }
    admission.map_err(|e| e.reason())?;
    let cast = game.spells.begin_cast(caster, true)?;
    game.spells.levitations.push(Effect {
        cast,
        caster,
        caster_position,
        target,
        state: rules::Levitation::new(
            position.y,
            rules::MAX_RISE,
            target == Target::Actor(caster),
            game.time as f64,
        ),
    });
    game.spells.track(Track {
        label: match target {
            Target::Actor(actor) => game.actor_name(actor),
            Target::Prop(index) => game.spells.props[index].name.clone(),
        },
        target,
        spell: "Levitate".into(),
        at: game.time,
        start: position,
        requested: rules::MAX_RISE,
    });
    game.spells.record(
        game.time,
        "Levitate",
        "Altitude hold: rise at most 20 ft; push off surfaces to move".into(),
        save,
    );
    Ok(())
}

/// Character movement remains a capsule sweep; the spell contributes only velocity.
pub fn prepare(
    spells: &mut SpellWorld,
    actor: u64,
    character: &mut physics::character::Character,
    input: DVec3,
    time: f64,
    dt: f64,
) -> DVec3 {
    let Some(effect) = spells.levitations.iter_mut().find(|e| {
        e.target == Target::Actor(actor) && !matches!(e.state.phase, rules::Phase::Done(_))
    }) else {
        return input;
    };
    effect
        .state
        .update(time, character.feet.distance(effect.caster_position));
    if effect.state.holding() {
        character.gravity = Some(physics::character::GravityOverride {
            scale: 0.,
            terminal: 55.,
        });
        let velocity = character.external + DVec3::Y * character.vertical_speed;
        let dv = DVec3::Y
            * effect
                .state
                .vertical_accel(character.feet.y, character.vertical_speed, 0., dt)
            * dt
            + effect.state.horizontal_accel(velocity) * dt;
        character.add_velocity(dv);
        let center = character.feet + DVec3::Y * 0.9;
        let surface = spells
            .world
            .colliders()
            .iter()
            .enumerate()
            .filter(|(_, c)| spells.world[c.body].kind == physics::BodyKind::Static)
            .filter_map(|(index, collider)| {
                let point = collider.closest_point(&spells.world, center);
                let offset = center - point;
                let normal = offset.normalize_or(DVec3::Y);
                let gap = (offset.length() - 0.35).max(0.);
                (normal.y.abs() < 0.7 && gap <= rules::REACH).then_some(rules::Surface {
                    point,
                    normal,
                    gap,
                    body: collider.body,
                    collider: physics::ColliderId(index as u32),
                })
            })
            .min_by(|a, b| a.gap.total_cmp(&b.gap));
        character.add_velocity(rules::push_off(
            input / rules::WALK_SPEED,
            velocity,
            surface.as_ref(),
        ));
        DVec3::ZERO
    } else if effect.state.gentle() {
        character.gravity = Some(physics::character::GravityOverride {
            scale: 1.,
            terminal: rules::FEATHER_FALL_SPEED,
        });
        input
    } else {
        character.gravity = None;
        input
    }
}

/// Restore the reflected component removed by the capsule's wall slide.
pub fn bounce(
    spells: &SpellWorld,
    actor: u64,
    character: &mut physics::character::Character,
    before: DVec3,
) {
    if !spells
        .levitations
        .iter()
        .any(|e| e.target == Target::Actor(actor) && e.state.holding())
        || character.support.is_some()
    {
        return;
    }
    let lost = before - character.external;
    if lost.length_squared() < 1e-8 {
        return;
    }
    let center = character.feet + DVec3::Y * 0.9;
    let restitution = spells
        .world
        .colliders()
        .iter()
        .filter(|c| {
            spells.world[c.body].kind == physics::BodyKind::Static && !spells.world[c.body].removed
        })
        .filter(|c| c.closest_point(&spells.world, center).distance(center) < 0.5)
        .map(|c| c.material.restitution)
        .max_by(f64::total_cmp)
        .unwrap_or(super::Material::Stone.physics().restitution);
    character.external -= lost * restitution;
}
