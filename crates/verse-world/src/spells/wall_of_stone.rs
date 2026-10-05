//! Authored Wall of Stone panels in the chamber's shared rigid world.
use crate::{play::Game, wall_of_stone as rules};
use glam::DVec3;
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Effect {
    pub cast: u64,
    pub caster: u64,
    pub wall: rules::rig::Wall,
}

pub fn cast(game: &mut Game, context: super::Caster) -> Result<(), String> {
    context.validate(game)?;
    let forward = DVec3::new(
        -f64::from(context.yaw.sin()),
        0.,
        -f64::from(context.yaw.cos()),
    );
    let side = DVec3::new(forward.z, 0., -forward.x);
    let start = context.feet.as_dvec3() + forward * 4. - side * rules::Form::Thick.size().x;
    let panels = rules::shapes::straight(start, side, 2, rules::Form::Thick);
    raise(game, context, &panels)
}

pub fn raise(
    game: &mut Game,
    context: super::Caster,
    panels: &[rules::Placement],
) -> Result<(), String> {
    context.validate(game)?;
    let mut stone = vec![];
    let mut bodies = vec![];
    for collider in game.spells.world.colliders() {
        if game.spells.world[collider.body].kind != physics::BodyKind::Static
            || game.spells.world[collider.body].removed
        {
            continue;
        }
        if game
            .spells
            .props
            .iter()
            .any(|p| p.body == collider.body && p.spec.material != super::Material::Stone)
        {
            continue;
        }
        if let physics::Shape::Cuboid { half } = collider.shape {
            let (center, orientation) = collider.pose(&game.spells.world);
            stone.push(rules::validate::Stone {
                center,
                orientation,
                half,
            });
            bodies.push(collider.body);
        }
    }
    let plan = rules::validate::validate(panels, &stone, context.feet.as_dvec3())
        .map_err(|e| format!("Wall of Stone placement refused: {e:?}"))?;
    if game.spells.props.len() + panels.len() > super::MAX_PROPS {
        return Err("Stone panel budget exceeded".into());
    }
    let creatures = super::feather_fall::candidates(game);
    for target in creatures {
        let actor = u64::from(target.id);
        let creature = rules::creatures::Creature {
            feet: target.position - DVec3::Y * 0.9,
            radius: 0.35,
            height: 1.8,
        };
        let side = creature.feet - context.feet.as_dvec3();
        let mut destination = rules::creatures::push_out(panels, creature, side);
        if rules::creatures::enclosed(panels, &stone, creature) {
            let save = game.spells.dice_for(context.life.actor).save(
                actor,
                "Dexterity",
                0,
                context.save_dc,
            );
            let result = rules::creatures::resolve_enclosure(
                panels,
                &stone,
                creature,
                save.roll as i32,
                0,
                save.dc,
            );
            if let rules::creatures::Enclosure::Escaped { to, .. } = result {
                destination = Some(to);
            }
            game.spells.record(
                game.time,
                "Wall of Stone",
                format!("Enclosure: {result:?}"),
                Some(save),
            );
        }
        if let Some(feet) = destination {
            game.stone_displacement(actor, feet)?;
            game.spells.record(
                game.time,
                "Wall of Stone",
                format!("Creature {actor} displaced to {feet:?}"),
                None,
            );
        }
    }
    let cast = game.spells.begin_cast(context.life.actor, true)?;
    let first_prop = game.spells.props.len();
    let wall = rules::rig::Wall::raise(&mut game.spells.world, &plan, &bodies, game.time as f64);
    for panel in &wall.panels {
        game.spells.adopt_body(
            panel.body,
            "Stone panel",
            super::PropKind::SpellBody,
            Some(cast),
            context.life.instance,
        )?;
    }
    game.spells.walls.push(Effect {
        cast,
        caster: context.life.actor,
        wall,
    });
    for prop in &game.spells.props[first_prop..] {
        let half = prop.spec.dimensions * 0.5;
        game.query_scene.insert(physics::queries::MeshCollider {
            key: prop.query_key(),
            layers: 1,
            usage: physics::queries::Usage::Blocking,
            mesh: physics::queries::Mesh::from_box(-half, half)?,
        })?;
    }
    game.spells.sync_query_poses(&mut game.query_scene)?;
    game.spells.record(
        game.time,
        "Wall of Stone",
        format!(
            "Raised {} supported panels; AC 15; {} HP each",
            panels.len(),
            panels[0].form.hit_points()
        ),
        None,
    );
    Ok(())
}

/// Build a bounded layout using the same validator as the default barricade.
pub fn authored(
    game: &mut Game,
    context: super::Caster,
    shape: u8,
    from: [i32; 3],
    to: [i32; 3],
    count: u8,
    thin: bool,
) -> Result<(), String> {
    context.validate(game)?;
    if count == 0 || count > 10 {
        return Err("Stone layout count must be between 1 and 10".into());
    }
    let point = |p: [i32; 3]| DVec3::from_array(p.map(|v| f64::from(v) / 1000.));
    let (from, to) = (point(from), point(to));
    let form = if thin {
        rules::Form::Thin
    } else {
        rules::Form::Thick
    };
    let panels = match shape {
        0 => rules::shapes::straight(from, to - from, usize::from(count), form),
        1 => rules::shapes::bridge(from, to, usize::from(count), form),
        2 => rules::shapes::ramp(from, to, usize::from(count), rules::Form::Half),
        3 => rules::shapes::tower(from, usize::from(count), form),
        4 => rules::shapes::enclosure(from, form),
        _ => return Err("Unknown stone layout".into()),
    };
    raise(game, context, &panels)
}

/// Current panel bounds for the chamber's projectile cover sweep.
pub fn cover(spells: &super::SpellWorld) -> Vec<(physics::BodyId, physics::kinematic::Aabb)> {
    spells
        .walls
        .iter()
        .filter(|e| !e.wall.vanished)
        .flat_map(|e| &e.wall.panels)
        .filter(|p| !p.destroyed && !spells.world[p.body].removed)
        .filter_map(|p| {
            let c = spells.world.colliders().iter().find(|c| c.body == p.body)?;
            let physics::Shape::Cuboid { half } = c.shape else {
                return None;
            };
            let (center, rotation) = c.pose(&spells.world);
            let extent = rotation.mul_vec3(DVec3::X).abs() * half.x
                + rotation.mul_vec3(DVec3::Y).abs() * half.y
                + rotation.mul_vec3(DVec3::Z).abs() * half.z;
            Some((
                p.body,
                physics::kinematic::Aabb {
                    min: center - extent,
                    max: center + extent,
                },
            ))
        })
        .collect()
}

/// Apply projectile damage through the panel rig, retaining its fracture debris.
pub fn hit(
    spells: &mut super::SpellWorld,
    body: physics::BodyId,
    kind: crate::rules::ProjectileKind,
    time: f64,
) -> Result<(), String> {
    let (amount, damage) = match kind {
        crate::rules::ProjectileKind::Firebolt => (8, rules::DamageType::Fire),
        crate::rules::ProjectileKind::Fireball => (15, rules::DamageType::Fire),
        crate::rules::ProjectileKind::Bow => (6, rules::DamageType::Piercing),
        _ => (4, rules::DamageType::Force),
    };
    hit_damage(spells, body, amount, damage, time)
}

/// Apply a hostile flight's impact to the panel that stopped its sweep.
pub fn struck(
    game: &mut Game,
    point: DVec3,
    amount: i32,
    damage: rules::DamageType,
) -> Result<(), String> {
    let body = cover(&game.spells)
        .into_iter()
        .find(|(_, bounds)| point.distance(point.clamp(bounds.min, bounds.max)) <= 0.25)
        .map(|(body, _)| body);
    if let Some(body) = body {
        hit_damage(&mut game.spells, body, amount, damage, game.time as f64)?;
    }
    Ok(())
}

fn hit_damage(
    spells: &mut super::SpellWorld,
    body: physics::BodyId,
    amount: i32,
    damage: rules::DamageType,
    time: f64,
) -> Result<(), String> {
    let before = spells.boundary_snapshot();
    let mut debris = vec![];
    for effect in &mut spells.walls {
        if let Some(panel) = effect.wall.panel_of(body) {
            effect.wall.damage(
                &mut spells.world,
                panel,
                amount,
                damage,
                time,
                spells.dice.seed,
            );
            if let Some(prop) = spells.props.iter_mut().find(|p| p.body == body) {
                prop.hit_points = Some(effect.wall.panels[panel].hit_points);
                prop.removed = spells.world[body].removed;
            }
            debris.extend(
                effect
                    .wall
                    .debris
                    .iter()
                    .filter(|d| !d.removed)
                    .map(|d| d.body),
            );
        }
    }
    for body in debris {
        if spells.props.len() >= super::MAX_PROPS && !spells.props.iter().any(|p| p.body == body) {
            spells.world.remove_body(body);
            for effect in &mut spells.walls {
                for piece in &mut effect.wall.debris {
                    if piece.body == body {
                        piece.removed = true;
                    }
                }
            }
            continue;
        }
        let instance = spells.props.first().map_or(1, |p| p.life.instance);
        spells.adopt_body(
            body,
            "Stone fracture debris",
            super::PropKind::SpellBody,
            None,
            instance,
        )?;
    }
    spells.record_boundary(&before, "stone:fracture");
    Ok(())
}
