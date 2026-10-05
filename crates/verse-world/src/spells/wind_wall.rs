//! Wind Wall through the chamber's physics and projectile state.
use crate::{play::Game, wind_wall as rules};
use glam::{DVec2, DVec3, Vec3};
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Effect {
    pub cast: u64,
    pub caster: u64,
    pub wall: rules::WindWall,
    #[serde(default)]
    pub supported: std::collections::BTreeSet<physics::BodyId>,
}

pub fn cast(game: &mut Game, context: super::Caster) -> Result<(), String> {
    context.validate(game)?;
    let facing = DVec3::new(
        -f64::from(context.yaw.sin()),
        0.,
        -f64::from(context.yaw.cos()),
    );
    let center = context.feet.as_dvec3() + facing * 4.;
    let side = DVec2::new(-facing.z, facing.x);
    let path = rules::Wall::straight(DVec2::new(center.x, center.z), side, 30. * super::FEET);
    cast_path(game, context, path)
}

/// Admit a continuous authored path through the native placement validator.
pub fn cast_path(game: &mut Game, context: super::Caster, path: Vec<DVec2>) -> Result<(), String> {
    context.validate(game)?;
    let wall = rules::Wall::new(path, context.feet.as_dvec3(), |p| {
        ground(&game.spells.world, p)
    })
    .map_err(|e| format!("Wind Wall placement refused: {e:?}"))?;
    let victims: Vec<_> = super::feather_fall::candidates(game)
        .into_iter()
        .filter(|c| wall.in_area(c.position - DVec3::Y * 0.9, 0.35, 1.8))
        .collect();
    let cast = game.spells.begin_cast(context.life.actor, true)?;
    for target in victims {
        let save = game.spells.dice_for(context.life.actor).save(
            u64::from(target.id),
            "Strength",
            0,
            context.save_dc,
        );
        let (_, damage) = rules::appearance_damage(save.success, || {
            game.spells.dice_for(context.life.actor).roll(8) as i32
        });
        if let Some(source) = game.player_source(u64::from(target.id)) {
            let hp = game.simulation.snapshot_for(source)?.player.hp;
            game.simulation.player_damage_for(source, damage.min(hp))?;
        } else if let Some(source) = game.ids.get(&u64::from(target.id)).copied() {
            game.simulation.bow_impact(source, damage)?;
        }
        game.spells.record(
            game.time,
            "Wind Wall",
            format!("Appearance: {damage} bludgeoning; no later damage"),
            Some(save),
        );
    }
    let seed = game.spells.dice_for(context.life.actor).seed;
    let wall = rules::WindWall::raise(&mut game.spells.world, wall, game.time as f64, seed);
    game.spells.wind_walls.push(Effect {
        cast,
        caster: context.life.actor,
        supported: rules::supported(&game.spells.world, &wall.bodies),
        wall,
    });
    game.simulation.set_spell_wind_walls(
        game.spells
            .wind_walls
            .iter()
            .filter(|e| !e.wall.ended && f64::from(game.time) < e.wall.until)
            .map(|e| e.wall.wall.clone())
            .collect(),
    );
    Ok(())
}

pub fn ground(world: &physics::World, p: DVec2) -> Option<f64> {
    world
        .colliders()
        .iter()
        .filter(|c| world[c.body].kind == physics::BodyKind::Static && !world[c.body].removed)
        .filter_map(|c| {
            let (center, rotation) = c.pose(world);
            let physics::Shape::Cuboid { half } = c.shape else {
                return None;
            };
            if rotation.dot(glam::DQuat::IDENTITY).abs() < 0.999 {
                return None;
            }
            ((p.x - center.x).abs() <= half.x && (p.y - center.z).abs() <= half.z)
                .then_some(center.y + half.y)
        })
        .filter(|y| *y <= 2.)
        .max_by(f64::total_cmp)
}

/// Draw the grounded outline and updraft of each live wall.
pub fn guide_lines(game: &Game) -> Vec<(Vec3, Vec3, [f32; 4])> {
    game.spells
        .wind_walls
        .iter()
        .filter(|e| !e.wall.ended && f64::from(game.time) < e.wall.until)
        .flat_map(|active| wall_lines(&active.wall.wall, game.time))
        .collect()
}

/// One wall's grounded outline and the updraft streaks rising through it at
/// `time`, s, as colored segments. Zones without the chamber's game draw a
/// wall with these too.
pub fn wall_lines(wall: &rules::Wall, time: f32) -> Vec<(Vec3, Vec3, [f32; 4])> {
    let mut out = vec![];
    let outline = [0.55, 0.9, 1.0, 0.85];
    let streak = [0.85, 0.97, 1.0, 0.55];
    let (base, top) = (wall.base as f32, wall.top() as f32);
    let at = |p: DVec2, y: f32| Vec3::new(p.x as f32, y, p.y as f32);
    for pair in wall.path.windows(2) {
        let normal = (pair[1] - pair[0]).normalize().perp() * (rules::THICKNESS * 0.5);
        for side in [-1., 1.] {
            let (a, b) = (pair[0] + normal * side, pair[1] + normal * side);
            out.push((at(a, base + 0.02), at(b, base + 0.02), outline));
            out.push((at(a, top), at(b, top), outline));
        }
        let length = pair[0].distance(pair[1]);
        let count = (length / 0.5).ceil().max(1.) as usize;
        for i in 0..count {
            let p = pair[0].lerp(pair[1], (i as f64 + 0.5) / count as f64);
            let phase = (i as f32 * 0.618).fract();
            let height = top - base;
            let y = base + ((time * 2.2 + phase) * height / 2.).rem_euclid(height);
            let end = (y + 0.9).min(top);
            out.push((at(p, y), at(p, end), streak));
        }
    }
    for p in &wall.path {
        out.push((at(*p, base), at(*p, top), outline));
    }
    out
}
