use super::*;
use crate::play::Ability;
use crate::playground::{Run, creature, hall};
use std::f32::consts::{FRAC_PI_2, PI};

const LEVITATE: Ability = Ability::Spell(SLOT);
/// A dummy parked far from every test, because the chamber authority needs
/// one creature.
const FAR: Vec3 = Vec3::new(20., 0., -20.);

/// The playground hall with the wizard at `caster`, facing +z, and dummies.
fn hall_game(caster: Vec3, dummies: &[(u64, Vec3)]) -> Game {
    let hall = hall().unwrap();
    let mut scene = hall.scene.clone();
    scene.actors[0].position = caster;
    scene
        .actors
        .push(creature(99, "Far dummy", "dummy", FAR, 0., 100));
    for (id, at) in dummies {
        scene
            .actors
            .push(creature(*id, "Dummy", "dummy", *at, 0., 100));
    }
    let mut game = Game::new(scene).unwrap();
    game.face(PI).unwrap();
    game.tick(1. / 30., [0.; 2]).unwrap();
    game
}

fn settle(game: &mut Game, seconds: f32) {
    for _ in 0..(seconds * 30.) as u32 {
        game.tick(1. / 30., [0.; 2]).unwrap();
    }
}

fn logged(game: &Game, needle: &str) -> bool {
    game.spells.log.iter().any(|r| r.text.contains(needle))
}

#[test]
fn aiming_at_nothing_lifts_the_caster_twenty_feet_and_input_alone_does_not_move_it() {
    let start = Vec3::new(-6., 0., 4.);
    let mut game = hall_game(start, &[]);
    game.activate(LEVITATE).unwrap();
    assert!(logged(&game, "Wizard (self): rises up to 20 ft"));
    let (mut highest, mut fastest) = (0_f64, 0_f64);
    for _ in 0..(8 * 30) {
        game.tick(1. / 30., [0.; 2]).unwrap();
        highest = highest.max(f64::from(game.player.y));
        fastest = fastest.max(game.character.vertical_speed);
    }
    assert!(highest <= MAX_RISE + 1e-3, "{highest}");
    assert!(fastest <= mechanics::RISE_SPEED + 1e-9, "{fastest}");
    assert!(
        (f64::from(game.player.y) - MAX_RISE).abs() < 0.02,
        "{}",
        game.player.y
    );
    // Nothing within 5 ft: walking input does nothing.
    let before = game.player;
    for _ in 0..60 {
        game.tick(1. / 30., [0., 1.]).unwrap();
    }
    let moved = Vec3::new(game.player.x - before.x, 0., game.player.z - before.z).length();
    assert!(moved < 1e-4, "{moved}");
    assert!(game.character.external.length() < 1e-9);
}

#[test]
fn a_push_off_from_a_wall_is_capped_at_climbing_speed_and_the_drift_persists() {
    // The stone wall's west face is at x = 10.
    let start = Vec3::new(9.5, 0., -3.);
    let mut game = hall_game(start, &[]);
    // Facing west, away from the wall.
    game.face(FRAC_PI_2).unwrap();
    game.tick(1. / 30., [0.; 2]).unwrap();
    game.activate(LEVITATE).unwrap();
    settle(&mut game, 1.);
    for _ in 0..3 {
        game.tick(1. / 30., [0., 1.]).unwrap();
    }
    let speed = game.character.external.length();
    assert!(speed <= CLIMB_SPEED + 1e-9, "{speed}");
    assert!(speed > CLIMB_SPEED * 0.99, "{speed}");
    assert!(game.character.external.x < 0.);
    assert!(logged(&game, "pushes off"));
    let x = game.player.x;
    settle(&mut game, 2.);
    let drift = game.character.external.length();
    let expected = speed * (-mechanics::DRIFT_DAMPING * 2.).exp();
    assert!((drift - expected).abs() < 0.01, "{drift} vs {expected}");
    assert!(x - game.player.x > 5., "{} -> {}", x, game.player.x);
}

#[test]
fn the_end_of_concentration_floats_the_target_down_without_damage() {
    let mut game = hall_game(Vec3::new(-6., 0., 4.), &[]);
    game.activate(LEVITATE).unwrap();
    settle(&mut game, 8.);
    let top = game.player.y;
    let health = game.snapshot().player.hp;
    let player = game.player_actor();
    game.spells.end_concentration(player).unwrap();
    let mut fastest = 0_f64;
    for _ in 0..(4 * 30) {
        game.tick(1. / 30., [0.; 2]).unwrap();
        fastest = fastest.max(-game.character.vertical_speed);
    }
    assert!(fastest <= FEATHER_FALL_SPEED + 1e-9, "{fastest}");
    assert!(fastest > FEATHER_FALL_SPEED * 0.99, "{fastest}");
    assert!(
        top > 6. && game.player.y.abs() < 1e-3,
        "{top} -> {}",
        game.player.y
    );
    assert!(!game.spells.log.iter().any(|r| r.spell == "Falling"));
    assert_eq!(game.snapshot().player.hp, health);
    assert!(logged(&game, "lands gently, no falling damage"));
    assert!(game.spells.levitations.active.is_empty());
    assert!(game.character.gravity.is_none());
}

#[test]
fn leaving_sixty_feet_ends_the_spell_and_releases_concentration() {
    let start = Vec3::new(2., 0., 0.);
    let mut game = hall_game(start, &[]);
    let crate_ = game
        .spawn_prop(
            "Crate",
            super::super::PropSpec::reference(super::super::PropKind::Crate),
            start + Vec3::new(0., 0.3, 3.),
            0.,
        )
        .unwrap();
    game.activate(LEVITATE).unwrap();
    assert!(logged(&game, "Crate: rises up to 20 ft"));
    let player = game.player_actor();
    assert!(game.spells.concentration.contains_key(&player));
    settle(&mut game, 1.);
    // Walk south, away from the crate, until it is out of range.
    game.face(0.).unwrap();
    for _ in 0..(4 * 30) {
        game.tick(1. / 30., [0., 1.]).unwrap();
    }
    assert!(logged(&game, "Crate: out of the 60-ft range"));
    assert!(!game.spells.concentration.contains_key(&player));
    settle(&mut game, 4.);
    assert!(game.spells.levitations.active.is_empty());
    let rest = game.spells.prop_center(crate_).y;
    assert!((rest - 0.3).abs() < 0.02, "{rest}");
}

#[test]
fn heavy_and_secured_objects_are_refused_and_a_saving_creature_is_unaffected() {
    use super::super::{PropKind, PropSpec};
    let start = Vec3::new(2., 0., 0.);
    let mut game = hall_game(start, &[(2, start + Vec3::new(3., 0., 3.))]);
    let mut anvil = PropSpec::reference(PropKind::Anvil);
    anvil.mass = 300.;
    game.spawn_prop("Anvil + load", anvil, start + Vec3::new(0., 0.2, 3.), 0.)
        .unwrap();
    game.activate(LEVITATE).unwrap();
    assert!(logged(&game, "Anvil + load: refused, too heavy: 661 lb"));
    assert!(game.spells.levitations.active.is_empty());
    game.spells.dice.force_save(2, 20).unwrap();
    game.face((-3_f32).atan2(-3.)).unwrap();
    settle(&mut game, 2.);
    game.activate(LEVITATE).unwrap();
    assert!(logged(
        &game,
        "Dummy: CON save 20 +0 = 20 vs DC 15 succeeds; unaffected"
    ));
    assert!(game.spells.levitations.active.is_empty());
    assert!(game.spells.concentration.is_empty());
}

#[test]
fn a_checkpoint_mid_drift_replays_identically() {
    let mut run = Run::new(scenario()).unwrap();
    while run.game.time < 3. {
        run.advance().unwrap();
    }
    let dummy = &run.game.npc_characters[&101];
    assert!(dummy.external.length() > 1., "{dummy:?}");
    assert!(dummy.feet.y > 1.);
    let saved = run.game.checkpoint().unwrap();
    let mut restored = Game::restore(&saved).unwrap();
    for _ in 0..(6 * 30) {
        run.game.tick(1. / 30., [0.; 2]).unwrap();
        restored.tick(1. / 30., [0.; 2]).unwrap();
    }
    assert!(logged(&run.game, "Dummy A (willing): bounces"));
    assert_eq!(
        run.game.checkpoint().unwrap(),
        restored.checkpoint().unwrap()
    );
}

#[test]
fn a_levitated_cultist_hangs_helpless_and_lands_without_damage() {
    let scene = verse_engine::director::Scene::from_json(include_bytes!(
        "../../../../../assets/verse/original/ritual.json"
    ))
    .unwrap();
    let mut game = Game::new(scene).unwrap();
    for _ in 0..201 {
        game.tick(0.1, [0.; 2]).unwrap();
    }
    let cultist = game
        .scene
        .actors
        .iter()
        .find(|a| a.model.starts_with("cultist"))
        .map(|a| a.id)
        .unwrap();
    let at = game.actor_position(cultist).unwrap();
    game.player = at - Vec3::Z * 3.;
    game.face(PI).unwrap();
    game.tick(1. / 30., [0.; 2]).unwrap();
    let ground = game.actor_position(cultist).unwrap().y;
    game.spells.dice.force_save(cultist, 1).unwrap();
    game.activate(LEVITATE).unwrap();
    assert!(logged(&game, "fails; rises up to 20 ft"));
    settle(&mut game, 5.);
    let aloft = game.actor_position(cultist).unwrap();
    assert!(aloft.y - ground > 4., "{} -> {}", ground, aloft.y);
    assert!(game.hostile_held(cultist));
    let player = game.player_actor();
    game.spells.end_concentration(player).unwrap();
    settle(&mut game, 4.);
    assert!(!game.hostile_held(cultist));
    assert!(!game.spells.levitations.holds(cultist));
    assert!(!game.spells.log.iter().any(|r| r.spell == "Falling"));
}
