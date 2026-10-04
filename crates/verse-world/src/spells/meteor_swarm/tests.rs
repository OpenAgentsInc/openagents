use super::*;
use crate::play::Ability;
use crate::playground::{Run, creature, hall};

fn settle(game: &mut Game, seconds: f32) {
    for _ in 0..(seconds * 30.) as u32 {
        game.tick(1. / 30., [0.; 2]).unwrap();
    }
}

/// The playground hall with dummies at `dummies` and the wizard at the
/// caster spawn, facing -z.
fn hall_game(dummies: &[(u64, Vec3)]) -> Game {
    let hall = hall().unwrap();
    let mut scene = hall.scene.clone();
    scene.actors[0].position = hall.spawn("caster").unwrap();
    for (id, at) in dummies {
        scene
            .actors
            .push(creature(*id, "Dummy", "dummy", *at, 0., 400));
    }
    let mut game = Game::new(scene).unwrap();
    game.face(0.).unwrap();
    game.tick(1. / 30., [0.; 2]).unwrap();
    game
}

#[test]
fn a_point_under_an_authored_ceiling_is_refused() {
    let ceiling = physics::kinematic::Aabb {
        min: DVec3::new(-50., 8., -50.),
        max: DVec3::new(50., 9., 50.),
    };
    let floor = physics::kinematic::Aabb {
        min: DVec3::new(-200., -1., -200.),
        max: DVec3::new(200., 0., 200.),
    };
    let covered = SpellWorld::new(&[floor, ceiling], 1);
    let open = SpellWorld::new(&[floor], 1);
    let point = DVec3::new(0., 0., -20.);
    assert!(!sky_clear(&covered, DVec3::ZERO, point));
    assert!(sky_clear(&open, DVec3::ZERO, point));
    let hostiles = [(2, point)];
    let refused = choose_points(DVec3::ZERO, -DVec3::Z, &hostiles, |p| {
        sky_clear(&covered, DVec3::ZERO, p)
    })
    .unwrap_err();
    assert!(refused.contains("open sky"), "{refused}");
}

#[test]
fn points_aim_at_groups_keep_the_caster_outside_and_fill_to_four() {
    let hostiles = [
        (2, DVec3::new(-2., 0., -20.)),
        (3, DVec3::new(2., 0., -20.)),
        (4, DVec3::new(0., 0., -40.)),
        // Too close: a Sphere on it would reach the caster.
        (5, DVec3::new(0., 0., -5.)),
    ];
    let points = choose_points(DVec3::ZERO, -DVec3::Z, &hostiles, |_| true).unwrap();
    // The pair first, at its centroid; then the lone dummy, beside it.
    assert!(points[0].distance(DVec3::new(0., 0., -20.)) < 1e-9);
    assert!(points[1].distance(DVec3::new(0., 0., -40.)) <= STANDOFF + 1e-9);
    for (i, p) in points.iter().enumerate() {
        assert!(DVec3::new(p.x, 0., p.z).length() >= SAFE_DISTANCE, "{p}");
        for q in &points[..i] {
            assert!(p.distance(*q) >= SPACING);
        }
    }
    assert!(choose_points(DVec3::ZERO, -DVec3::Z, &hostiles[3..], |_| true).is_err());
}

#[test]
fn a_cast_in_the_hall_hits_each_dummy_once_and_moves_none() {
    // Two dummies 9 m apart stand in both of the first two Spheres.
    let dummies = [
        (2, Vec3::new(-4., 0., -16.)),
        (3, Vec3::new(4., 0., -21.)),
        (4, Vec3::new(-12., 0., 8.)),
    ];
    let mut game = hall_game(&dummies);
    let mana = game.snapshot().player.mana;
    game.activate(Ability::Spell(SLOT)).unwrap();
    assert_eq!(game.snapshot().player.mana, mana - COST);
    assert_eq!(game.spells.meteor_swarm.casts.len(), 1);
    settle(&mut game, 4.);
    let state = &game.spells.meteor_swarm;
    assert!(state.casts.is_empty());
    assert_eq!(state.impacts.len(), METEORS);
    for impact in &state.impacts {
        assert_eq!(impact.radius, RADIUS);
        assert!(!impact.obstructed, "{impact:?}");
    }
    let hits: Vec<u64> = state
        .impacts
        .iter()
        .flat_map(|i| i.creatures.iter().map(|c| c.id))
        .collect();
    for (id, at) in dummies {
        assert_eq!(hits.iter().filter(|h| **h == id).count(), 1, "{id}");
        assert!(game.actor_position(id).unwrap().distance(at) < 1e-4);
        let hit = state
            .impacts
            .iter()
            .flat_map(|i| &i.creatures)
            .find(|c| c.id == id)
            .unwrap();
        // Half of each type on a save: at most 60 of each from 20d6.
        if hit.save.success {
            assert!(hit.damage.fire <= 60 && hit.damage.bludgeoning <= 60);
        } else {
            assert!(hit.damage.fire >= 20 && hit.damage.bludgeoning >= 20);
        }
        let health = game
            .frame()
            .actors
            .iter()
            .find(|a| a.actor.id == id)
            .unwrap()
            .health;
        assert_eq!(health as i32, 400 - hit.damage.total());
    }
    let error = game.spells.ledger_error();
    assert!(error.linear < super::super::LEDGER_TOLERANCE, "{error:?}");
}

#[test]
fn crates_in_the_sphere_take_the_damage_break_and_burn() {
    let mut game = hall_game(&[(2, Vec3::new(-4., 0., -16.))]);
    let spec = super::super::PropSpec::reference(super::super::PropKind::Crate);
    let near = game
        .spawn_prop("Near crate", spec.clone(), Vec3::new(-1., 0.3, -15.), 0.)
        .unwrap();
    let anvil = game
        .spawn_prop(
            "Anvil",
            super::super::PropSpec {
                hit_points: Some(1_000),
                ..super::super::PropSpec::reference(super::super::PropKind::Anvil)
            },
            Vec3::new(-7., 0.2, -16.),
            0.,
        )
        .unwrap();
    settle(&mut game, 0.5);
    let anvil_start = game.spells.prop_center(anvil);
    game.activate(Ability::Spell(SLOT)).unwrap();
    settle(&mut game, 3.5);
    assert!(game.spells.props[near].removed, "the crate broke");
    let debris: Vec<_> = game
        .spells
        .props
        .iter()
        .filter(|p| p.name.starts_with("Near crate debris") && !p.removed)
        .collect();
    assert_eq!(debris.len(), mechanics::DEBRIS_CHUNKS);
    let tick = game.spells.world.tick;
    assert!(
        game.spells
            .meteor_swarm
            .objects
            .iter()
            .filter(|o| debris.iter().any(|d| d.body == o.body))
            .all(|o| o.burning(tick))
    );
    // The anvil survives the full damage, taken once, and the blast moves it.
    let anvil_body = game.spells.props[anvil].body;
    let hits: Vec<_> = game
        .spells
        .meteor_swarm
        .impacts
        .iter()
        .flat_map(|i| &i.objects)
        .filter(|o| o.body == anvil_body)
        .collect();
    assert!(!hits.is_empty());
    assert_eq!(hits.iter().filter(|h| h.damage.is_some()).count(), 1);
    assert!(hits.iter().all(|h| h.impulse.length() > 0.));
    let damage = hits.iter().find_map(|h| h.damage).unwrap();
    assert_eq!(
        game.spells.props[anvil].hit_points,
        Some(1_000 - damage.total())
    );
    assert!(game.spells.prop_center(anvil).distance(anvil_start) > 0.01);
    assert!(
        game.spells
            .ledger
            .external
            .contains_key(mechanics::LEDGER_TERM)
    );
    let error = game.spells.ledger_error();
    assert!(error.linear < super::super::LEDGER_TOLERANCE, "{error:?}");
}

#[test]
fn a_checkpoint_mid_fall_replays_identically() {
    let mut run = Run::new(scenario()).unwrap();
    while run.game.time < 2.2 {
        run.advance().unwrap();
    }
    assert!(!run.game.spells.meteor_swarm.casts.is_empty());
    assert!(run.game.spells.meteor_swarm.impacts.is_empty());
    let saved = run.game.checkpoint().unwrap();
    let mut restored = Game::restore(&saved).unwrap();
    for _ in 0..90 {
        run.game.tick(1. / 30., [0.; 2]).unwrap();
        restored.tick(1. / 30., [0.; 2]).unwrap();
    }
    assert_eq!(run.game.spells.meteor_swarm.impacts.len(), METEORS);
    assert_eq!(
        run.game.checkpoint().unwrap(),
        restored.checkpoint().unwrap()
    );
}

/// Render instances a frame of `game` needs, counted the way the native
/// chamber presentation draws them: one per character plus the bow, one
/// per live prop, an impact and six sparks per small cue, an impact and
/// sixteen sparks per blast, and four per spell projectile.
fn presentation_instances(game: &Game) -> usize {
    let actors = game.frame().actors.len() + 1;
    let props = game.spells.props.iter().filter(|p| !p.removed).count();
    let cues: usize = game
        .impacts
        .iter()
        .filter(|(_, at, _)| game.time - at < CUE_LIFETIME)
        .map(|(_, _, kind)| match *kind {
            3 => 0,
            BLAST_CUE => 17,
            _ => 7,
        })
        .sum();
    actors + props + cues + 4 * game.snapshot().projectiles.len()
}

#[test]
fn the_busiest_playground_frame_stays_within_the_instance_budget() {
    let mut run = Run::new(scenario()).unwrap();
    let mut busiest = (0, 0.);
    while !run.done() {
        run.advance().unwrap();
        let count = presentation_instances(&run.game);
        if count > busiest.0 {
            busiest = (count, run.game.time);
        }
    }
    // The engine refuses a frame over 256 instances; leave room for the
    // hall and anything else on screen.
    assert!(
        busiest.0 <= 200,
        "{} instances at {:.2} s",
        busiest.0,
        busiest.1
    );
    assert!(live_flame_cues(run.result()) <= MAX_FLAME_CUES);
}

#[test]
fn the_playground_scenario_passes_its_check() {
    let mut run = Run::new(scenario()).unwrap();
    while !run.done() {
        run.advance().unwrap();
    }
    (run.scenario.check)(run.result()).unwrap();
    assert_eq!(run.replay_identical, Some(true));
}

#[test]
fn the_spell_is_on_the_second_action_bar() {
    let spell = Ability::Spell(SLOT).catalog().unwrap();
    assert_eq!(spell.key, "meteor-swarm");
    assert_eq!(spell.icon, "meteor-swarm-icon");
    assert_eq!(spell.label, NAME);
    assert!(crate::playground::scenario("meteor-swarm").is_some());
}

#[test]
fn clustered_cultists_in_the_chamber_are_each_struck_once() {
    let scene = verse_engine::director::Scene::from_json(include_bytes!(
        "../../../../../assets/verse/original/ritual.json"
    ))
    .unwrap();
    let mut game = Game::new(scene).unwrap();
    for _ in 0..201 {
        game.tick(0.1, [0.; 2]).unwrap();
    }
    game.face(std::f32::consts::PI).unwrap();
    game.tick(1. / 30., [0.; 2]).unwrap();
    // The chamber's authored solids have no ceiling, so the sky is open.
    game.activate(Ability::Spell(SLOT)).unwrap();
    settle(&mut game, 4.);
    let state = &game.spells.meteor_swarm;
    assert_eq!(state.impacts.len(), METEORS);
    // The picker skips points whose flight meets authored scenery, so no
    // meteor detonates on a pillar or a wall.
    let authored = |body: BodyId| !game.spells.props.iter().any(|p| p.body == body);
    assert!(
        state
            .impacts
            .iter()
            .all(|i| !i.obstructed || i.struck.is_none_or(|b| !authored(b))),
        "{:?}",
        state.impacts
    );
    let hits: Vec<u64> = state
        .impacts
        .iter()
        .flat_map(|i| i.creatures.iter().map(|c| c.id))
        .collect();
    let cultists: Vec<u64> = game
        .scene
        .actors
        .iter()
        .filter(|a| a.model.starts_with("cultist"))
        .map(|a| a.id)
        .collect();
    let struck = cultists.iter().filter(|c| hits.contains(c)).count();
    assert!(struck >= 3, "{struck} cultists struck");
    for id in &hits {
        assert_eq!(hits.iter().filter(|h| *h == id).count(), 1);
    }
    assert!(!hits.contains(&game.player_actor()));
}
