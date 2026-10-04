//! Rigid creature bodies used while an articulated spell controls motion.
use crate::play::Game;
use glam::DVec3;
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Proxy {
    pub actor: u64,
    pub body: physics::BodyId,
    pub cast: u64,
    pub held: bool,
    pub ended: bool,
}

pub fn add(game: &mut Game, cast: u64, actor: u64) -> Result<physics::BodyId, String> {
    if game.spells.proxies.len() >= 256 || game.spells.world.bodies().len() >= 8192 {
        return Err("Creature proxy budget exceeded".into());
    }
    let character = *game
        .actor_character(actor)
        .ok_or("Spell creature has no controller")?;
    let dimensions = DVec3::new(0.7, 1.8, 0.7);
    let mut body = physics::Body::new(
        75.,
        physics::Body::box_inertia(75., dimensions),
        character.feet + DVec3::Y * 0.9,
    );
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
        "spell:creature-proxy",
        physics::Momentum::of(&game.spells.world[body], game.spells.ledger.origin),
    );
    game.spells.proxies.push(Proxy {
        actor,
        body,
        cast,
        held: true,
        ended: false,
    });
    Ok(body)
}

pub fn prepare(
    spells: &super::SpellWorld,
    actor: u64,
    character: &mut physics::character::Character,
    input: DVec3,
) -> DVec3 {
    if spells
        .proxies
        .iter()
        .any(|p| p.actor == actor && p.held && !p.ended)
    {
        character.gravity = Some(physics::character::GravityOverride {
            scale: 0.,
            terminal: 55.,
        });
        character.external = DVec3::ZERO;
        character.vertical_speed = 0.;
        DVec3::ZERO
    } else {
        if spells
            .proxies
            .iter()
            .any(|p| p.actor == actor && !p.held && !p.ended)
        {
            character.gravity = None;
        }
        input
    }
}
