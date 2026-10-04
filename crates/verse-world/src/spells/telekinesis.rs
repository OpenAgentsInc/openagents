//! Telekinesis grips in the chamber's rigid world, including creature proxies.
use super::{Target, Track};
use crate::{play::Game, telekinesis as rules};
use glam::DVec3;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Effect {
    pub cast: u64,
    pub caster: u64,
    pub caster_position: DVec3,
    pub target: Target,
    pub aim: DVec3,
    pub grip: rules::Telekinesis,
    pub proxy: Option<physics::BodyId>,
}

pub fn cast(game: &mut Game) -> Result<(), String> {
    let target = if game.selected >= super::PROP_ENTITY_BASE {
        Target::Prop(
            game.spells
                .props
                .iter()
                .position(|p| p.life.entity == game.selected && !p.removed)
                .ok_or("Unknown Telekinesis prop")?,
        )
    } else {
        Target::Actor(game.selected)
    };
    cast_on(game, target)
}

pub fn cast_on(game: &mut Game, target: Target) -> Result<(), String> {
    let caster = game.player_actor();
    let caster_position = game.player.as_dvec3() + DVec3::Y * 0.9;
    let (position, size, subject) = match target {
        Target::Actor(actor) => {
            if game
                .scene
                .actors
                .iter()
                .any(|a| a.id == actor && a.friendly)
            {
                return Err("Friendly scene creatures cannot be spell targets".into());
            }
            if game.actor_character(actor).is_none() {
                return Err("Unknown Telekinesis creature".into());
            }
            (
                game.actor_position(actor)
                    .ok_or("Unknown Telekinesis creature")?
                    .as_dvec3()
                    + DVec3::Y * 0.9,
                rules::Size::Medium,
                rules::Target::Creature { strength: 0 },
            )
        }
        Target::Prop(index) => {
            let prop = game
                .spells
                .props
                .get(index)
                .ok_or("Unknown Telekinesis prop")?;
            if prop.removed || prop.spec.secured {
                return Err("Telekinesis requires a movable target".into());
            }
            let size = match prop.spec.size {
                super::Size::Tiny => rules::Size::Tiny,
                super::Size::Small => rules::Size::Small,
                super::Size::Medium => rules::Size::Medium,
                super::Size::Large => rules::Size::Large,
                super::Size::Huge => rules::Size::Huge,
            };
            (game.spells.prop_center(index), size, rules::Target::Object)
        }
    };
    if position.distance(caster_position) > rules::RANGE {
        return Err("Telekinesis target is beyond 60 feet".into());
    }
    if physics::kinematic::sweep_box(
        caster_position,
        DVec3::splat(0.01),
        position - caster_position,
        &game.colliders,
    )?
    .is_some()
    {
        return Err("Telekinesis target is behind cover".into());
    }
    let save = if let Target::Actor(actor) = target {
        let save = game
            .spells
            .dice
            .save(actor, "Strength", 0, super::SPELL_SAVE_DC);
        let native = rules::Save::new(save.roll as i32, save.modifier, save.dc);
        game.spells.record(
            game.time,
            "Telekinesis",
            format!("Strength save: {} vs DC {}", save.total, save.dc),
            Some(save),
        );
        Some(native)
    } else {
        None
    };
    // Re-applying to the current grip renews its budget and creature timer.
    if let Some(effect) = game
        .spells
        .telekinesis
        .iter_mut()
        .find(|e| e.caster == caster && e.target == target && e.grip.grip.is_some())
    {
        let body = effect.grip.grip.unwrap().body;
        effect
            .grip
            .apply(
                &mut game.spells.world,
                caster_position,
                body,
                size,
                subject,
                save,
                &mut vec![],
            )
            .map_err(|e| format!("Telekinesis refused: {e:?}"))?;
        return Ok(());
    }
    let cast = game.spells.begin_cast(caster, true)?;
    if save.is_some_and(|s| s.success) {
        game.spells.record(
            game.time,
            "Telekinesis",
            "Save succeeds; no movement".into(),
            None,
        );
        return Ok(());
    }
    let (body, proxy) = match target {
        Target::Actor(actor) => {
            let character = *game.actor_character(actor).unwrap();
            let dimensions = DVec3::new(0.7, 1.8, 0.7);
            let mut body =
                physics::Body::new(75., physics::Body::box_inertia(75., dimensions), position);
            body.vel = character.external + DVec3::Y * character.vertical_speed;
            let body = game.spells.world.add(body);
            game.spells.world.add_collider(
                physics::Collider::new(
                    body,
                    physics::Shape::Cuboid {
                        half: dimensions * 0.5,
                    },
                )
                .with_material(super::Material::Straw.physics()),
            );
            game.spells.ledger.add(
                "telekinesis:proxy",
                physics::Momentum::of(&game.spells.world[body], game.spells.ledger.origin),
            );
            (body, Some(body))
        }
        Target::Prop(index) => (game.spells.props[index].body, None),
    };
    let mut grip = rules::Telekinesis::cast(
        &mut game.spells.world,
        position,
        DVec3::new(0., -super::GRAVITY, 0.),
    );
    grip.apply(
        &mut game.spells.world,
        caster_position,
        body,
        size,
        subject,
        save,
        &mut vec![],
    )
    .map_err(|e| format!("Telekinesis refused: {e:?}"))?;
    game.spells.telekinesis.push(Effect {
        cast,
        caster,
        caster_position,
        target,
        aim: position + DVec3::Y * (20. * super::FEET),
        grip,
        proxy,
    });
    game.spells.track(Track {
        label: match target {
            Target::Actor(actor) => game.actor_name(actor),
            Target::Prop(index) => game.spells.props[index].name.clone(),
        },
        target,
        spell: "Telekinesis".into(),
        at: game.time,
        start: position,
        requested: rules::MOVE_BUDGET,
    });
    Ok(())
}

/// A restrained creature cannot walk while the rigid grip controls it.
pub fn prepare(
    spells: &super::SpellWorld,
    actor: u64,
    character: &mut physics::character::Character,
    input: DVec3,
) -> DVec3 {
    if spells
        .telekinesis
        .iter()
        .any(|effect| effect.target == Target::Actor(actor) && effect.grip.grip.is_some())
    {
        character.gravity = Some(physics::character::GravityOverride {
            scale: 0.,
            terminal: 55.,
        });
        character.external = DVec3::ZERO;
        character.vertical_speed = 0.;
        DVec3::ZERO
    } else {
        input
    }
}
