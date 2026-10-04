//! Meteor Swarm's swept impacts, object damage, and burning in the chamber.
use crate::{meteor_swarm as rules, play::Game};
use glam::DVec3;
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Effect {
    pub cast: u64,
    pub caster: u64,
    pub swarm: rules::MeteorSwarm,
    pub objects: Vec<rules::Unattended>,
}

pub fn cast(game: &mut Game) -> Result<(), String> {
    let point = game
        .actor_position(game.selected)
        .unwrap_or(game.player + glam::Vec3::new(-game.yaw.sin(), 0., -game.yaw.cos()) * 10.)
        .as_dvec3();
    cast_at(
        game,
        [
            point,
            point + DVec3::X * 4.,
            point - DVec3::X * 4.,
            point + DVec3::Z * 4.,
        ],
    )
}

pub fn cast_at(game: &mut Game, points: [DVec3; 4]) -> Result<(), String> {
    if game.scene.collision_profile.as_deref() == Some("original-chamber-v1") {
        return Err("Meteor Swarm requires open sky; leave the indoor chamber".into());
    }
    let caster = game.player_actor();
    let origin = game.player.as_dvec3() + DVec3::Y * 1.4;
    rules::validate(origin, &points, |p| {
        physics::kinematic::sweep_box(
            origin,
            DVec3::splat(0.01),
            p + DVec3::Y * 0.1 - origin,
            &game.colliders,
        )
        .is_ok_and(|hit| hit.is_none())
    })
    .map_err(|e| format!("Meteor Swarm refused: {e:?}"))?;
    let breakable = game
        .spells
        .props
        .iter()
        .filter(|p| !p.removed && p.hit_points.is_some())
        .count();
    if game.spells.props.len() + 4 + breakable * rules::DEBRIS_CHUNKS > super::MAX_PROPS {
        return Err("Meteor debris budget exceeded".into());
    }
    let before = game.spells.boundary_snapshot();
    let swarm = rules::MeteorSwarm::cast(
        &mut game.spells.world,
        origin,
        points,
        super::GRAVITY,
        super::SPELL_SAVE_DC,
        |_| true,
        &mut |s| game.spells.dice.roll(s),
    )
    .map_err(|e| format!("Meteor Swarm refused: {e:?}"))?;
    let cast = game.spells.begin_cast(caster, false)?;
    let objects = game
        .spells
        .props
        .iter()
        .filter(|p| !p.removed)
        .map(|p| rules::Unattended::new(p.body, p.hit_points, p.spec.flammable))
        .collect();
    game.spells.record_boundary(&before, "meteor:spawn");
    game.spells.meteors.push(Effect {
        cast,
        caster,
        swarm,
        objects,
    });
    Ok(())
}
