//! Thunderwave on physics.
//!
//! SRD 5.2.1: level 1 Evocation, casting time Action, range Self, a 15-foot
//! Cube originating from the caster. Each creature in the Cube makes a
//! Constitution saving throw: on a failure it takes 2d8 Thunder damage and
//! is pushed 10 feet away; on a success it takes half damage only.
//! Unsecured objects entirely within the Cube are pushed 10 feet away.
//!
//! The SRD outcome decides whether a push applies; the push itself is a
//! calibrated velocity ([`physics::character::Character::push_speed`] and
//! [`super::SpellWorld::prop_push_speed`]), so walls, crates, and other
//! creatures decide where the target actually stops.
use super::{FEET, SPELL_SAVE_DC, Target, Track};
use crate::play::Game;
use glam::{DVec3, Vec3};

pub const NAME: &str = "Thunderwave";
/// Edge of the Cube, m.
pub const CUBE: f64 = 15. * FEET;
/// Push distance, m.
pub const PUSH: f64 = 10. * FEET;
/// Damage dice on a failed save.
pub const DAMAGE_DICE: (u32, u32) = (2, 8);
/// Height of a character's center above its feet, m.
const CENTER: f64 = 0.9;

/// The Cube in the caster's frame: its origin is the center of the face
/// touching the caster, and it extends along the caster's facing.
#[derive(Clone, Copy, Debug)]
pub struct Cube {
    pub origin: DVec3,
    pub forward: DVec3,
}

impl Cube {
    pub fn new(caster_feet: DVec3, facing: DVec3) -> Self {
        Self {
            origin: caster_feet + DVec3::Y * CENTER,
            forward: DVec3::new(facing.x, 0., facing.z).normalize_or(DVec3::Z),
        }
    }
    pub fn contains(&self, p: DVec3) -> bool {
        let d = p - self.origin;
        let side = DVec3::new(-self.forward.z, 0., self.forward.x);
        let along = d.dot(self.forward);
        (-1e-9..=CUBE + 1e-9).contains(&along)
            && d.dot(side).abs() <= CUBE * 0.5 + 1e-9
            && d.y.abs() <= CUBE * 0.5 + 1e-9
    }
    /// Horizontal direction from the caster's center to a target's center.
    pub fn away(&self, center: DVec3) -> DVec3 {
        let d = center - self.origin;
        DVec3::new(d.x, 0., d.z).normalize_or(self.forward)
    }
}

/// Resolves an admitted Thunderwave from the caster's place and facing.
pub(crate) fn resolve(game: &mut Game, facing: Vec3) -> Result<(), String> {
    let caster = game.player_actor();
    let cube = Cube::new(game.player.as_dvec3(), facing.as_dvec3());
    let origin = game.player + Vec3::Y * 1.4;
    let colliders = game.colliders.clone();
    // Total cover from static geometry blocks the wave.
    let visible = |p: Vec3| {
        physics::kinematic::sweep_box(
            origin.as_dvec3(),
            DVec3::splat(0.12),
            (p - origin).as_dvec3(),
            &colliders,
        )
        .is_ok_and(|hit| hit.is_none())
    };
    let snapshot = game.snapshot();
    let creatures: Vec<(u64, u32, Vec3)> = game
        .ids
        .iter()
        .filter_map(|(actor, id)| {
            snapshot
                .actors
                .iter()
                .find(|a| a.id == *id && a.alive && a.faction != "player")
                .map(|a| (*actor, *id, Vec3::from(a.pos)))
        })
        .filter(|(_, _, feet)| {
            cube.contains(feet.as_dvec3() + DVec3::Y * CENTER) && visible(*feet + Vec3::Y * 1.1)
        })
        .collect();
    let cast = game.spells.begin_cast(caster, false)?;
    let _ = cast;
    for (actor, id, feet) in creatures {
        let name = game.actor_name(actor);
        let modifier = super::constitution_modifier(
            &game
                .scene
                .actors
                .iter()
                .find(|a| a.id == actor)
                .map(|a| a.model.clone())
                .unwrap_or_default(),
        );
        let save = game
            .spells
            .dice
            .save(actor, "Constitution", modifier, SPELL_SAVE_DC);
        let rolled = game.spells.dice.sum(DAMAGE_DICE.0, DAMAGE_DICE.1) as i32;
        let damage = if save.success { rolled / 2 } else { rolled };
        game.simulation.bow_impact(id, damage)?;
        let center = feet.as_dvec3() + DVec3::Y * CENTER;
        let direction = cube.away(center);
        game.spells.record(
            game.time,
            NAME,
            format!(
                "{name}: CON save {} {:+} = {} vs DC {} {}; {damage} thunder{}",
                save.roll,
                save.modifier,
                save.total,
                save.dc,
                if save.success { "succeeds" } else { "fails" },
                if save.success { "" } else { ", pushed 10 ft" }
            ),
            Some(save.clone()),
        );
        game.spells.track(Track {
            label: name,
            target: Target::Actor(actor),
            spell: NAME.into(),
            at: game.time,
            start: feet.as_dvec3(),
            requested: if save.success { 0. } else { PUSH },
        });
        if save.success {
            continue;
        }
        if game.colliders.is_empty() {
            // Scenes without collision keep the kinematic shove.
            let push = (direction * PUSH).as_vec3();
            game.controls.displace(id, push);
            game.simulation
                .place_chamber_actor(id, (feet + push).to_array(), 0.)?;
            continue;
        }
        let character = game
            .npc_characters
            .entry(actor)
            .or_insert_with(|| physics::character::Character::new(feet.as_dvec3()));
        if character.feet.as_vec3() != feet {
            *character = physics::character::Character::new(feet.as_dvec3());
        }
        character.add_velocity(direction * physics::character::Character::push_speed(PUSH));
        game.controls.displace(id, Vec3::ZERO);
    }
    for index in 0..game.spells.props.len() {
        let prop = &game.spells.props[index];
        if prop.removed {
            continue;
        }
        let name = prop.name.clone();
        let secured = prop.spec.secured;
        let inside = game
            .spells
            .prop_corners(index)
            .iter()
            .all(|corner| cube.contains(*corner));
        let center = game.spells.prop_center(index);
        if !inside || !visible(center.as_vec3()) {
            continue;
        }
        game.spells.track(Track {
            label: name.clone(),
            target: Target::Prop(index),
            spell: NAME.into(),
            at: game.time,
            start: center,
            requested: if secured { 0. } else { PUSH },
        });
        if secured {
            game.spells
                .record(game.time, NAME, format!("{name}: secured, not moved"), None);
            continue;
        }
        game.spells
            .push_prop(index, cube.away(center), PUSH, "thunderwave")?;
    }
    Ok(())
}

/// The playground recording: a 15-crate pyramid whose near crates sit in the
/// Cube, three dummies (one fails, one succeeds on a forced roll, one fails
/// with a wall 1.5 m behind it), and a secured crate beside a loose one.
pub fn scenario() -> crate::playground::Scenario {
    use crate::playground::{Cue, Scenario, Shot, Step, creature};
    use std::f32::consts::{FRAC_PI_2, PI};
    Scenario {
        key: "thunderwave",
        title: NAME,
        srd: "Level 1 Evocation | Range Self (15-ft Cube) | CON save | 2d8 Thunder, push 10 ft",
        seed: 451,
        live: 14.,
        replay: (1.4, 3.4),
        setup: |scene, hall| {
            let station = hall.spawn("wall_station")?;
            for (id, name, z) in [
                (101, "Dummy A", 1.7),
                (102, "Dummy B (wall)", 0.),
                (103, "Dummy C", -1.7),
            ] {
                scene.actors.push(creature(
                    id,
                    name,
                    "dummy",
                    station + Vec3::Z * z,
                    FRAC_PI_2,
                    100,
                ));
            }
            Ok(())
        },
        populate: |game, hall| {
            let caster = hall.spawn("caster")?;
            let spec = super::PropSpec::reference(super::PropKind::Crate);
            for row in 0..5 {
                for i in 0..5 - row {
                    let x = -1.2 - 0.305 * row as f32 - 0.61 * i as f32;
                    let y = 0.3 + 0.6 * row as f32 + 0.002 * row as f32;
                    game.spawn_prop(
                        &format!("Crate {}-{}", row + 1, i + 1),
                        spec.clone(),
                        caster + Vec3::new(x, y, 2.5),
                        0.,
                    )?;
                }
            }
            game.spawn_prop(
                "Secured crate",
                spec.clone().secured(),
                caster + Vec3::new(-0.8, 0.3, -2.5),
                0.,
            )?;
            game.spawn_prop("Loose crate", spec, caster + Vec3::new(0.8, 0.3, -2.5), 0.)?;
            // A fails, C succeeds, and B fails into the wall.
            for (dummy, roll) in [(101, 4), (102, 3), (103, 18)] {
                game.spells.dice.force_save(dummy, roll)?;
            }
            Ok(())
        },
        script: || {
            vec![
                Cue {
                    at: 0.5,
                    step: Step::Face(PI),
                },
                Cue {
                    at: 1.5,
                    step: Step::Cast(crate::play::Ability::Thunderwave),
                },
                Cue {
                    at: 5.5,
                    step: Step::Face(-FRAC_PI_2),
                },
                Cue {
                    at: 6.0,
                    step: Step::Cast(crate::play::Ability::Thunderwave),
                },
                Cue {
                    at: 10.0,
                    step: Step::Face(0.),
                },
                Cue {
                    at: 10.5,
                    step: Step::Cast(crate::play::Ability::Thunderwave),
                },
            ]
        },
        camera: || {
            vec![
                Shot {
                    at: 0.,
                    eye: Vec3::new(10.0, 4.6, 6.0),
                    target: Vec3::new(4.0, 1.9, 1.2),
                },
                Shot {
                    at: 4.2,
                    eye: Vec3::new(10.0, 4.6, 6.0),
                    target: Vec3::new(4.0, 1.9, 1.2),
                },
                Shot {
                    at: 5.4,
                    eye: Vec3::new(6.5, 4.5, -6.5),
                    target: Vec3::new(8.6, 0.6, 0.5),
                },
                Shot {
                    at: 8.8,
                    eye: Vec3::new(6.5, 4.5, -6.5),
                    target: Vec3::new(8.6, 0.6, 0.5),
                },
                Shot {
                    at: 9.8,
                    eye: Vec3::new(8.6, 4.0, 1.8),
                    target: Vec3::new(6.0, 0.3, -3.8),
                },
                Shot {
                    at: 14.,
                    eye: Vec3::new(8.6, 4.0, 1.8),
                    target: Vec3::new(6.0, 0.3, -3.8),
                },
            ]
        },
        replay_camera: (Vec3::new(9.5, 3.2, 1.0), Vec3::new(3.6, 0.9, 3.6)),
        check: |game| {
            let moved = |label: &str| {
                game.spells
                    .tracks
                    .iter()
                    .find(|t| t.label == label)
                    .and_then(|t| crate::playground::measure(game, t))
                    .map(|(_, d)| d)
                    .ok_or(format!("{label} was not in the wave"))
            };
            let near = |label: &str, want: f64, tolerance: f64| -> Result<(), String> {
                let got = moved(label)?;
                if (got - want).abs() > tolerance {
                    return Err(format!("{label} moved {got:.3} m, expected {want:.3} m"));
                }
                Ok(())
            };
            near("Dummy A", PUSH, PUSH * 0.05)?;
            near("Dummy C", 0., 0.02)?;
            near("Dummy B (wall)", 1.5, 0.1)?;
            near("Secured crate", 0., 1e-6)?;
            near("Loose crate", PUSH, PUSH * 0.05)?;
            let pushed: Vec<_> = game
                .spells
                .tracks
                .iter()
                .filter(|t| t.label.starts_with("Crate ") && t.requested > 0.)
                .collect();
            if pushed.is_empty() || pushed.len() > 6 {
                return Err(format!(
                    "{} pyramid crates were inside the Cube",
                    pushed.len()
                ));
            }
            let toppled = (0..game.spells.props.len())
                .filter(|i| {
                    let p = &game.spells.props[*i];
                    p.name.starts_with("Crate ")
                        && !pushed.iter().any(|t| t.target == Target::Prop(*i))
                        && game.spells.world[p.body]
                            .orientation
                            .angle_between(glam::DQuat::IDENTITY)
                            > 0.3
                })
                .count();
            if toppled < 3 {
                return Err(format!("Only {toppled} pyramid crates toppled"));
            }
            let error = game.spells.ledger_error();
            if error.linear > super::LEDGER_TOLERANCE || error.angular > super::LEDGER_TOLERANCE {
                return Err(format!("Ledger residual {error:?}"));
            }
            Ok(())
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::play::Ability;
    use crate::playground::{creature, hall};

    /// The playground hall with dummies at `dummies` and the wizard at
    /// `caster`, facing +z.
    fn hall_game(caster: Vec3, dummies: &[(u64, Vec3)]) -> Game {
        let hall = hall().unwrap();
        let mut scene = hall.scene.clone();
        scene.actors[0].position = caster;
        for (id, at) in dummies {
            scene
                .actors
                .push(creature(*id, "Dummy", "dummy", *at, 0., 100));
        }
        let mut game = Game::new(scene).unwrap();
        game.face(std::f32::consts::PI).unwrap();
        game.tick(1. / 30., [0.; 2]).unwrap();
        game
    }

    fn settle(game: &mut Game, seconds: f32) {
        for _ in 0..(seconds * 30.) as u32 {
            game.tick(1. / 30., [0.; 2]).unwrap();
        }
    }

    #[test]
    fn a_creature_that_saves_takes_half_damage_and_stays() {
        let caster = Vec3::new(-4., 0., 0.);
        let mut game = hall_game(caster, &[(2, caster + Vec3::Z * 2.)]);
        game.spells.dice.force_save(2, 20).unwrap();
        game.activate(Ability::Thunderwave).unwrap();
        settle(&mut game, 1.5);
        let save = game.spells.log[0].save.clone().unwrap();
        assert!(save.success && save.roll == 20);
        assert!(
            game.actor_position(2)
                .unwrap()
                .distance(caster + Vec3::Z * 2.)
                < 1e-4
        );
        let health = game
            .frame()
            .actors
            .iter()
            .find(|a| a.actor.id == 2)
            .unwrap()
            .health;
        assert!((92..=99).contains(&health), "{health}");
    }

    #[test]
    fn a_failed_save_slides_ten_feet_and_a_wall_stops_the_next() {
        let caster = Vec3::new(-4., 0., 0.);
        let mut game = hall_game(caster, &[(2, caster + Vec3::Z * 2.)]);
        game.spells.dice.force_save(2, 1).unwrap();
        game.activate(Ability::Thunderwave).unwrap();
        settle(&mut game, 1.5);
        let moved = game
            .actor_position(2)
            .unwrap()
            .distance(caster + Vec3::Z * 2.);
        assert!((moved as f64 - PUSH).abs() < PUSH * 0.05, "{moved}");
        // The stone wall's west face is at x = 10; 1.5 m behind a dummy at 8.15.
        let mut game = hall_game(Vec3::new(5.75, 0., 0.), &[(2, Vec3::new(8.15, 0., 0.))]);
        game.face(-std::f32::consts::FRAC_PI_2).unwrap();
        game.tick(1. / 30., [0.; 2]).unwrap();
        game.spells.dice.force_save(2, 1).unwrap();
        game.activate(Ability::Thunderwave).unwrap();
        settle(&mut game, 1.5);
        let x = game.actor_position(2).unwrap().x;
        assert!((x - 9.65).abs() < 0.01, "{x}");
    }

    #[test]
    fn a_prop_partly_inside_gets_no_impulse_and_a_secured_prop_stays() {
        let caster = Vec3::new(-4., 0., 0.);
        let mut game = hall_game(caster, &[(2, Vec3::new(20., 0., -20.))]);
        let spec = super::super::PropSpec::reference(super::super::PropKind::Crate);
        // Straddles the far face of the Cube, 4.572 m ahead.
        let partly = game
            .spawn_prop("partly", spec.clone(), caster + Vec3::new(0., 0.3, 4.5), 0.)
            .unwrap();
        let secured = game
            .spawn_prop(
                "secured",
                spec.clone().secured(),
                caster + Vec3::new(1., 0.3, 2.),
                0.,
            )
            .unwrap();
        let loose = game
            .spawn_prop("loose", spec, caster + Vec3::new(-1., 0.3, 2.), 0.)
            .unwrap();
        settle(&mut game, 0.5);
        let before: Vec<_> = (0..3).map(|i| game.spells.prop_center(i)).collect();
        game.activate(Ability::Thunderwave).unwrap();
        assert!(!game.spells.ledger.external.is_empty());
        let pushed = game.spells.ledger.external["spell:thunderwave"].linear;
        assert!((pushed.length() - 20. * game.spells.prop_push_speed(loose, PUSH)).abs() < 1e-9);
        settle(&mut game, 2.);
        assert!(game.spells.prop_center(partly).distance(before[partly]) < 1e-3);
        assert!(game.spells.prop_center(secured).distance(before[secured]) < 1e-9);
        let moved = game.spells.prop_center(loose) - before[loose];
        let moved = DVec3::new(moved.x, 0., moved.z).length();
        assert!((moved - PUSH).abs() < PUSH * 0.05, "{moved}");
        let error = game.spells.ledger_error();
        assert!(error.linear < super::super::LEDGER_TOLERANCE, "{error:?}");
    }

    #[test]
    fn a_shove_off_the_ledge_deals_falling_damage() {
        // The ledge is 6 m (19.7 ft) tall: 1d6 on landing.
        let caster = Vec3::new(-19., 6., -2.);
        let mut game = hall_game(caster, &[(2, Vec3::new(-16.6, 6., -2.))]);
        game.face(-std::f32::consts::FRAC_PI_2).unwrap();
        game.tick(1. / 30., [0.; 2]).unwrap();
        game.spells.dice.force_save(2, 1).unwrap();
        game.activate(Ability::Thunderwave).unwrap();
        settle(&mut game, 3.);
        let fall = game
            .spells
            .log
            .iter()
            .find(|r| r.spell == "Falling")
            .expect("The dummy fell");
        assert!(fall.text.contains("1d6"), "{}", fall.text);
        assert!(game.actor_position(2).unwrap().y.abs() < 1e-3);
    }

    #[test]
    fn checkpoints_replay_identically_with_props_in_flight() {
        let mut run = crate::playground::Run::new(scenario()).unwrap();
        while run.game.time < 1.6 {
            run.advance().unwrap();
        }
        assert!(
            run.game
                .spells
                .props
                .iter()
                .any(|p| run.game.spells.world[p.body].vel.length() > 1.)
        );
        let saved = run.game.checkpoint().unwrap();
        let mut restored = Game::restore(&saved).unwrap();
        for _ in 0..60 {
            run.game.tick(1. / 30., [0.; 2]).unwrap();
            restored.tick(1. / 30., [0.; 2]).unwrap();
        }
        assert_eq!(
            run.game.checkpoint().unwrap(),
            restored.checkpoint().unwrap()
        );
    }

    #[test]
    fn the_chamber_wave_still_spares_a_cultist_behind_a_column_and_knocks_cultists_together() {
        let scene = verse_engine::director::Scene::from_json(include_bytes!(
            "../../../../assets/verse/original/ritual.json"
        ))
        .unwrap();
        let mut game = Game::new(scene).unwrap();
        for _ in 0..201 {
            game.tick(0.1, [0.; 2]).unwrap();
        }
        // Two cultists in a line: the near one is shoved into the far one.
        let ids: Vec<u64> = game
            .scene
            .actors
            .iter()
            .filter(|a| a.model.starts_with("cultist"))
            .map(|a| a.id)
            .take(2)
            .collect();
        let near = game.actor_position(ids[0]).unwrap();
        let far = near + Vec3::new(0., 0., 1.6);
        game.simulation
            .place_chamber_actor(game.ids[&ids[1]], far.to_array(), 0.)
            .unwrap();
        game.npc_characters.remove(&ids[1]);
        game.player = near - Vec3::Z * 1.5;
        game.face(std::f32::consts::PI).unwrap();
        game.tick(1. / 30., [0.; 2]).unwrap();
        let far = game.actor_position(ids[1]).unwrap();
        game.spells.dice.force_save(ids[0], 1).unwrap();
        game.spells.dice.force_save(ids[1], 20).unwrap();
        game.activate(Ability::Thunderwave).unwrap();
        settle(&mut game, 2.);
        assert!(game.actor_position(ids[1]).unwrap().distance(far) > 0.3);
    }
}
