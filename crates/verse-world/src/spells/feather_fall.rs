//! Feather Fall through the chamber's admitted reaction and fixed-step controllers.
use super::{SpellWorld, Target, Track};
use crate::{feather_fall as rules, play::Game};
use glam::DVec3;

pub fn candidates(game: &Game) -> Vec<rules::Candidate> {
    game.player_actors()
        .into_iter()
        .chain(
            game.scene
                .actors
                .iter()
                .filter(|a| a.nameplate && !a.friendly)
                .map(|a| a.id),
        )
        .filter_map(|actor| {
            let id = u32::try_from(actor).ok()?;
            let character = game.actor_character(actor)?;
            Some(rules::Candidate {
                id,
                kind: rules::Kind::Creature,
                position: character.feet + DVec3::Y * 0.9,
                velocity: character.external + DVec3::Y * character.vertical_speed,
                supported: !character.airborne(),
            })
        })
        .collect()
}

pub fn cast(game: &mut Game) -> Result<(), String> {
    let candidates = visible_falling(game);
    let caster = game.player.as_dvec3() + DVec3::Y * 0.9;
    let chosen =
        rules::decide(caster, &candidates).ok_or("No visible falling creatures within 60 feet")?;
    cast_on(game, &chosen)
}

pub fn cast_on(game: &mut Game, chosen: &[u32]) -> Result<(), String> {
    let candidates = candidates(game);
    let caster = game.player.as_dvec3() + DVec3::Y * 0.9;
    for id in chosen {
        let candidate = candidates
            .iter()
            .find(|c| c.id == *id)
            .ok_or("Unknown Feather Fall target")?;
        if physics::kinematic::sweep_box(
            caster,
            DVec3::splat(0.01),
            candidate.position - caster,
            &game.colliders,
        )?
        .is_some()
        {
            return Err("Feather Fall target is behind cover".into());
        }
    }
    let effect = rules::FeatherFall::cast(
        game.player_actor() as u32,
        caster,
        &candidates,
        chosen,
        game.time as f64,
    )
    .map_err(|e| format!("Feather Fall refused: {e:?}"))?;
    game.spells.begin_cast(game.player_actor(), false)?;
    for id in chosen {
        let actor = u64::from(*id);
        game.spells.track(Track {
            label: game.actor_name(actor),
            target: Target::Actor(actor),
            spell: "Feather Fall".into(),
            at: game.time,
            start: game.actor_position(actor).unwrap().as_dvec3(),
            requested: 0.,
        });
    }
    game.spells.record(
        game.time,
        "Feather Fall",
        format!("Warded {} falling creatures for up to 60 s", chosen.len()),
        None,
    );
    for old in &mut game.spells.feather_falls {
        old.wards.retain(|ward| !chosen.contains(&ward.target));
    }
    game.spells
        .feather_falls
        .retain(|old| !old.ended(game.time as f64));
    game.spells.feather_falls.push(effect);
    Ok(())
}

/// Set the next step's terminal speed from gradual drag; horizontal motion is unchanged.
pub fn prepare(
    spells: &mut SpellWorld,
    actor: u64,
    character: &mut physics::character::Character,
    time: f64,
    dt: f64,
) {
    spells.feather_falls.retain(|effect| !effect.ended(time));
    let Some(id) = u32::try_from(actor).ok() else {
        return;
    };
    if let Some(effect) = spells
        .feather_falls
        .iter_mut()
        .find(|effect| effect.holds(id, time))
    {
        let before = character.vertical_speed;
        let gravity = character.gravity.map_or(1., |g| g.scale)
            * physics::character::Settings::default().gravity;
        if let Some(drag) = effect.drag(id, 75., before, before - gravity * dt, dt, time) {
            character.gravity = Some(physics::character::GravityOverride {
                scale: character.gravity.map_or(1., |g| g.scale),
                terminal: (-drag.vertical_speed).max(rules::DESCENT_CAP).min(55.),
            });
        }
    } else if character.gravity.is_some_and(|g| g.scale == 1.) {
        character.gravity = None;
    }
}

pub fn scenario() -> crate::playground::Scenario {
    use crate::play::Ability;
    use crate::playground::{Cue, Scenario, Shot, Step, creature};
    use glam::Vec3;
    Scenario {
        key: "feather-fall",
        title: "Feather Fall",
        srd: "Level 1 Transmutation | Reaction | Range 60 ft | up to five falling creatures | 60 ft/round",
        seed: 455,
        live: 13.,
        speed: 1,
        replay: (0.9, 2.9),
        setup: |scene, _| {
            let tower = (60. * super::FEET) as f32 + 0.002;
            scene.actors[0].position = Vec3::new(0., tower, -3.);
            scene.actors[0].yaw = std::f32::consts::PI;
            for i in 0..5 {
                scene.actors.push(creature(
                    101 + i,
                    if i < 3 { "Warded dummy" } else { "Normal fall" },
                    "dummy",
                    Vec3::new((i as f32 - 2.) * 0.8, tower, -1.1),
                    0.,
                    200,
                ));
            }
            Ok(())
        },
        populate: |game, _| {
            let height = 60. * super::FEET;
            let mut spec = super::PropSpec::reference(super::PropKind::StoneBlock).secured();
            spec.dimensions = DVec3::new(6., height, 3.);
            spec.mass = 100_000.;
            game.spawn_prop(
                "60 ft tower",
                spec,
                Vec3::new(0., height as f32 * 0.5, -2.5),
                0.,
            )?;
            for actor in 101..106 {
                game.spells.dice.force_save(actor, 2)?;
            }
            Ok(())
        },
        script: || {
            vec![
                Cue {
                    at: 0.4,
                    step: Step::Face(std::f32::consts::PI),
                },
                Cue {
                    at: 0.5,
                    step: Step::Cast(Ability::Thunderwave),
                },
                Cue {
                    at: 1.2,
                    step: Step::Cast(Ability::SpellCommand(super::command::Command::Targets {
                        slot: 3,
                        targets: [101, 102, 103, 0, 0],
                        count: 3,
                    })),
                },
            ]
        },
        camera: || {
            vec![
                Shot {
                    at: 0.,
                    eye: Vec3::new(12., 25., 12.),
                    target: Vec3::new(0., 14., -1.),
                },
                Shot {
                    at: 9.,
                    eye: Vec3::new(12., 8., 15.),
                    target: Vec3::new(0., 5., 3.),
                },
            ]
        },
        replay_camera: (Vec3::new(8., 22., 9.), Vec3::new(0., 17., 0.)),
        check: |game| {
            let protected = game
                .spells
                .log
                .iter()
                .filter(|r| r.text.contains("0 falling damage"))
                .count();
            let normal = game
                .spells
                .log
                .iter()
                .filter(|r| r.spell == "Falling")
                .count();
            if protected != 3 || normal != 2 {
                return Err(format!(
                    "Expected three protected and two normal landings; got {protected} and {normal}"
                ));
            }
            Ok(())
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn the_chamber_reaction_lands_three_unharmed_and_replays_identically() {
        let mut run = crate::playground::Run::new(scenario()).unwrap();
        while !run.done() {
            run.advance().unwrap();
        }
        assert_eq!(run.replay_identical, Some(true));
        (run.scenario.check)(run.result()).unwrap();
    }
}

/// Read-only reaction availability for the player's action bar.
pub fn reaction_available(game: &Game) -> bool {
    game.spells.ready.get(&3).is_none_or(|at| *at <= game.time)
        && game.snapshot().player.mana >= super::CATALOG[3].cost
        && !visible_falling(game).is_empty()
}
fn visible_falling(game: &Game) -> Vec<rules::Candidate> {
    let caster = game.player.as_dvec3() + DVec3::Y * 0.9;
    candidates(game)
        .into_iter()
        .filter(|c| {
            c.falling()
                && c.position.distance(caster) <= rules::RANGE
                && !game
                    .spells
                    .feather_falls
                    .iter()
                    .any(|e| e.holds(c.id, game.time as f64))
                && physics::kinematic::sweep_box(
                    caster,
                    DVec3::splat(0.01),
                    c.position - caster,
                    &game.colliders,
                )
                .is_ok_and(|h| h.is_none())
        })
        .collect()
}
