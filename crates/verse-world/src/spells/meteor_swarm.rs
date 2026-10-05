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

pub fn cast(game: &mut Game, context: super::Caster) -> Result<(), String> {
    context.validate(game)?;
    let point = game
        .actor_position(context.selected)
        .unwrap_or(context.feet + glam::Vec3::new(-context.yaw.sin(), 0., -context.yaw.cos()) * 10.)
        .as_dvec3();
    cast_at(
        game,
        context,
        [
            point,
            point + DVec3::X * 4.,
            point - DVec3::X * 4.,
            point + DVec3::Z * 4.,
        ],
    )
}

pub fn cast_at(game: &mut Game, context: super::Caster, points: [DVec3; 4]) -> Result<(), String> {
    context.validate(game)?;
    if game.scene.collision_profile.as_deref() == Some("original-chamber-v1") {
        return Err("Meteor Swarm requires open sky; leave the indoor chamber".into());
    }
    let caster = context.life.actor;
    let origin = context.feet.as_dvec3() + DVec3::Y * 1.4;
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
    let flight = if game.scene.collision_profile.as_deref() == Some(crate::playground::PROFILE) {
        rules::Flight::PLAYGROUND
    } else {
        rules::Flight::STANDARD
    };
    let super::SpellWorld {
        world,
        dice,
        caster_dice,
        primary_caster,
        ..
    } = &mut game.spells;
    let dice = super::dice::for_caster(dice, caster_dice, *primary_caster, context.life.actor);
    let swarm = rules::MeteorSwarm::cast_with_flight(
        world,
        origin,
        points,
        super::GRAVITY,
        flight,
        context.save_dc,
        |_| true,
        &mut |s| dice.roll(s),
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
    game.spells.record(
        game.time,
        "Meteor Swarm",
        format!(
            "Four meteors: {:.0} m descent at {:.0} m/s; {} fire + {} bludgeoning",
            flight.height, flight.speed, swarm.damage.fire, swarm.damage.bludgeoning
        ),
        None,
    );
    game.spells.meteors.push(Effect {
        cast,
        caster,
        swarm,
        objects,
    });
    Ok(())
}

/// Bounded Meteor Swarm presentation shared with the native renderer.
pub const METEOR_TRAIL_INSTANCES: usize = 7;
pub const METEOR_TRAIL_SPACING: f32 = 0.03;
pub const BLAST_SHOW: f32 = 1.2;
pub const BLAST_GROW: f32 = 0.3;
pub const BLAST_COLUMN_INSTANCES: usize = 5;
pub const BLAST_SMOKE_INSTANCES: usize = 3;
pub const MAX_SCORCHES: usize = 8;

pub fn scenario() -> crate::playground::Scenario {
    use crate::playground::{Cue, Scenario, Shot, Step, creature};
    use glam::Vec3;
    Scenario {
        key: "meteor-swarm",
        title: "Meteor Swarm",
        srd: rules::SRD_LINE,
        seed: 459,
        live: 10.,
        speed: 1,
        replay: (3.0, 4.0),
        setup: |scene, _| {
            for (id, name, x, z) in [
                (101, "Dummy A1 (overlap)", -4., -16.),
                (102, "Dummy A2", -9., -19.),
                (103, "Dummy B1 (overlap)", 8., -16.),
                (104, "Dummy C1", -15., 8.),
                (105, "Dummy C2", -7., 10.),
                (106, "Dummy D (overhang)", 17., 12.),
            ] {
                // Enough health to take the full damage and stay standing.
                scene
                    .actors
                    .push(creature(id, name, "dummy", Vec3::new(x, 0., z), 0., 300));
            }
            Ok(())
        },
        populate: |game, _| {
            use super::{PropKind, PropSpec};
            use glam::Vec3;
            let crate_spec = PropSpec::reference(PropKind::Crate);
            // Props are few and large: every piece and every flame is a
            // render instance, and a frame holds at most 256.
            // A crate pyramid (2, 1) beside the first point.
            for row in 0..2 {
                for i in 0..2 - row {
                    let x = (i as f32 - (1 - row) as f32 * 0.5) * 0.61;
                    game.spawn_prop(
                        &format!("Crate {}-{}", row + 1, i + 1),
                        crate_spec.clone(),
                        Vec3::new(-4.4 + x, 0.3 + 0.602 * row as f32, -19.0),
                        0.,
                    )?;
                }
            }
            // A barrel pyramid (2, 1) east of the third point.
            let barrel = PropSpec::reference(PropKind::Barrel);
            for (name, x, y) in [
                ("Barrel 1-1", 6.275, 0.45),
                ("Barrel 1-2", 6.925, 0.45),
                ("Barrel 2-1", 6.6, 1.352),
            ] {
                game.spawn_prop(name, barrel.clone(), Vec3::new(x, y, -20.3), 0.)?;
            }
            // A wooden fence of two boards west of it.
            let board = PropSpec {
                dimensions: glam::DVec3::new(0.12, 1.1, 1.2),
                mass: 16.,
                ..PropSpec::reference(PropKind::Crate)
            };
            for n in 0..2 {
                game.spawn_prop(
                    &format!("Fence board {}", n + 1),
                    board.clone(),
                    Vec3::new(1.4, 0.55, -21.6 + 1.2 * n as f32),
                    0.,
                )?;
            }
            // A tower of six 45 cm sandstone blocks standing on the
            // second point, so its meteor strikes the top and blows the
            // tower apart from above. Mortared blocks this thick are
            // sturdier than the SRD's resilient Small object, so they take
            // the damage without breaking and stay whole on screen.
            let block = PropSpec {
                size: super::Size::Small,
                dimensions: glam::DVec3::splat(0.45),
                // Sandstone, about 2,000 kg per cubic meter.
                mass: 182.,
                hit_points: Some(300),
                ..PropSpec::reference(PropKind::StoneBlock)
            };
            for n in 0..6 {
                game.spawn_prop(
                    &format!("Tower block {}", n + 1),
                    block.clone(),
                    Vec3::new(-10.5, 0.225 + 0.452 * n as f32, 9.),
                    0.,
                )?;
            }
            // An overhang on two posts over the last dummy.
            let post = PropSpec {
                dimensions: glam::DVec3::new(0.5, 3., 0.5),
                mass: 1_500.,
                // Sturdy enough to stand through the blast.
                hit_points: Some(5_000),
                ..PropSpec::reference(PropKind::StoneBlock)
            }
            .secured();
            for (n, (x, z)) in [(14.9, 13.), (17.9, 9.8)].into_iter().enumerate() {
                game.spawn_prop(
                    &format!("Post {}", n + 1),
                    post.clone(),
                    Vec3::new(x, 1.5, z),
                    0.,
                )?;
            }
            let slab = PropSpec {
                dimensions: glam::DVec3::new(3.6, 0.4, 3.6),
                mass: 6_000.,
                size: super::Size::Large,
                hit_points: Some(5_000),
                ..PropSpec::reference(PropKind::StoneBlock)
            }
            .secured();
            game.spawn_prop("Overhang", slab, Vec3::new(16.4, 3.2, 11.4), 0.)?;
            Ok(())
        },
        script: || {
            vec![
                Cue {
                    at: 0.5,
                    step: Step::Face(0.),
                },
                Cue {
                    at: 1.0,
                    step: Step::Cast(crate::play::Ability::SpellCommand(
                        super::command::Command::Meteors([
                            [-4000, 0, -19000],
                            [-10500, 0, 9000],
                            [4000, 0, -21000],
                            [16400, 0, 11400],
                        ]),
                    )),
                },
            ]
        },
        // Meteors spawn at 1.0, 1.5, 2.0, and 2.5 s and fall for about
        // 1.8 s: A lands at 2.8 s, C on the tower top at about 3.25 s, B at
        // 3.8 s, and D on the overhang at about 4.2 s.
        camera: || {
            let shoulder = (Vec3::new(9., 3., 5.), Vec3::new(-1., 1.5, -14.));
            // Low at the north end, looking up the hall at the south wall,
            // so meteors fall in front of lit stone onto A and B.
            let hall = (Vec3::new(2., 1.4, 12.), Vec3::new(-1., 9., -22.));
            let tower = (Vec3::new(-14., 2.4, 13.5), Vec3::new(-10.5, 1.8, 9.));
            let south = (Vec3::new(-1., 7.5, -5.5), Vec3::new(-1., 1., -19.));
            let overhang = (Vec3::new(9., 4.5, 4.), Vec3::new(16.3, 2.4, 11.4));
            let wide = (Vec3::new(16., 7., -10.), Vec3::new(-1., 1.5, -18.5));
            [
                (0., shoulder),
                (0.9, shoulder),
                (1.0, hall),
                (3.0, hall),
                (3.02, tower),
                (3.6, tower),
                (3.62, south),
                (4.05, south),
                (4.07, overhang),
                (5.4, overhang),
                (5.42, tower),
                (7.0, tower),
                (7.02, wide),
                (10., wide),
            ]
            .into_iter()
            .map(|(at, (eye, target))| Shot { at, eye, target })
            .collect()
        },
        // The second impact: C on the tower, at 0.25x.
        replay_camera: (Vec3::new(-14., 2.4, 13.5), Vec3::new(-10.5, 1.8, 9.)),
        check: |game| {
            let swarm = &game
                .spells
                .meteors
                .first()
                .ok_or("Meteor Swarm was not cast")?
                .swarm;
            if swarm.impacts.len() != 4 || !(101..=106).all(|id| swarm.affected.contains(&id)) {
                return Err("Expected four impacts and damage to all six dummies".into());
            }
            for prefix in ["Overhang", "Tower block"] {
                if !swarm.impacts.iter().any(|impact| {
                    impact.obstructed
                        && impact.struck.is_some_and(|body| {
                            game.spells
                                .props
                                .iter()
                                .any(|p| p.body == body && p.name.starts_with(prefix))
                        })
                }) {
                    return Err(format!("Meteor did not strike {prefix}"));
                }
            }
            let error = game.spells.ledger_error();
            if error.linear > super::LEDGER_TOLERANCE || error.angular > super::LEDGER_TOLERANCE {
                return Err(format!("Meteor momentum residual: {error:?}"));
            }
            Ok(())
        },
    }
}
