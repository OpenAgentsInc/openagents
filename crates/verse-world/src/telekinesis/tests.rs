use super::*;
use crate::spells::{Dice, fall_dice};
use physics::trace::{Tolerance, Trace};
use physics::{Collider, NoField, Shape, Uniform};

const DT: f64 = 1. / 120.;
const DC: i32 = crate::spells::SPELL_SAVE_DC;
const DOWN: DVec3 = DVec3::new(0., -9.81, 0.);

fn static_box(world: &mut World, center: DVec3, half: DVec3) {
    let id = world.add(Body::new(1., DVec3::ONE, center).with_kind(BodyKind::Static));
    world.add_collider(Collider::new(id, Shape::Cuboid { half }));
}

fn floor(world: &mut World) {
    static_box(world, DVec3::new(0., -0.5, 0.), DVec3::new(60., 0.5, 60.));
}

fn block(world: &mut World, mass: f64, half: DVec3, at: DVec3) -> BodyId {
    let id = world.add(Body::new(mass, Body::box_inertia(mass, half * 2.), at));
    world.add_collider(Collider::new(id, Shape::Cuboid { half }));
    id
}

fn crate_at(world: &mut World, at: DVec3) -> BodyId {
    block(world, 20., DVec3::splat(0.3), at)
}

fn dummy_at(world: &mut World, x: f64) -> BodyId {
    block(
        world,
        75.,
        DVec3::new(0.25, 0.9, 0.15),
        DVec3::new(x, 0.9, 0.),
    )
}

fn run(
    world: &mut World,
    tk: &mut Telekinesis,
    caster: DVec3,
    aim: DVec3,
    seconds: f64,
) -> Vec<Release> {
    let mut releases = Vec::new();
    for _ in 0..ticks(world, seconds) {
        tk.steer(world, caster, aim);
        world.step(&Uniform(DOWN));
        releases.extend(tk.after_step(world, caster, None));
    }
    releases
}

fn grab(world: &mut World, tk: &mut Telekinesis, caster: DVec3, body: BodyId) {
    let applied = tk
        .apply(
            world,
            caster,
            body,
            Size::Small,
            Target::Object,
            false,
            &mut Vec::new(),
        )
        .unwrap();
    assert_eq!(applied, Applied::Gripped);
}

/// A Strength save from the seeded dice with a forced d20.
fn save(roll: u32) -> bool {
    let mut dice = Dice::new(452);
    dice.force_save(1, roll).unwrap();
    dice.save(1, "Strength", 0, DC).success
}

#[test]
fn hand_path_per_application_is_at_most_thirty_feet() {
    let mut world = World::new(DT);
    floor(&mut world);
    let caster = DVec3::ZERO;
    let body = crate_at(&mut world, DVec3::new(2., 0.3, 0.));
    let mut tk = Telekinesis::cast(&mut world, caster, DOWN);
    grab(&mut world, &mut tk, caster, body);
    let start = tk.hand_position(&world);
    // Ask for far more than the budget, high enough to stay airborne.
    let aim = DVec3::new(14., 4., 10.);
    run(&mut world, &mut tk, caster, aim, 6.);
    let grip = tk.grip.unwrap();
    let end = tk.hand_position(&world);
    assert!((grip.path - MOVE_BUDGET).abs() < 1e-9, "{}", grip.path);
    assert!(end.distance(start) <= MOVE_BUDGET + 1e-9);
    assert!(!tk.steering());
    // Spent: the hand froze and the crate hangs there.
    assert!(world[body].pos.distance(end) < 0.05, "{}", world[body].pos);
    assert!(world[body].pos.y > 2.);
    // A re-application renews the budget.
    grab(&mut world, &mut tk, caster, body);
    assert_eq!(tk.grip.unwrap().path, 0.);
    assert!(tk.steering());
    run(&mut world, &mut tk, caster, aim, 2.);
    assert!(tk.hand_position(&world).distance(end) > 1.);
    assert!(tk.grip.unwrap().path <= MOVE_BUDGET + 1e-9);
}

#[test]
fn the_hand_never_leaves_range_and_moves_at_most_six_meters_a_second() {
    let mut world = World::new(DT);
    let caster = DVec3::ZERO;
    let body = crate_at(&mut world, DVec3::new(15., 1., 0.));
    let mut tk = Telekinesis::cast(&mut world, caster, DOWN);
    grab(&mut world, &mut tk, caster, body);
    let mut last = tk.hand_position(&world);
    for _ in 0..240 {
        tk.steer(&mut world, caster, DVec3::new(100., 1., 0.));
        world.step(&NoField);
        let now = tk.hand_position(&world);
        assert!(now.distance(last) <= HAND_SPEED * DT + 1e-12);
        assert!(now.length() <= RANGE + 1e-9, "{now}");
        last = now;
    }
    assert!((last.length() - RANGE).abs() < 1e-9);
}

#[test]
fn the_grip_releases_beyond_sixty_feet() {
    let mut world = World::new(DT);
    floor(&mut world);
    let mut caster = DVec3::ZERO;
    let body = crate_at(&mut world, DVec3::new(10., 0.3, 0.));
    let mut tk = Telekinesis::cast(&mut world, caster, DOWN);
    grab(&mut world, &mut tk, caster, body);
    let aim = DVec3::new(15., 2., 0.);
    assert!(run(&mut world, &mut tk, caster, aim, 2.).is_empty());
    // The crate hangs at the hand while the caster walks away.
    let mut released = None;
    for _ in 0..ticks(&world, 6.) {
        caster.x -= 3. * DT;
        tk.steer(&mut world, caster, aim);
        world.step(&Uniform(DOWN));
        let before = world[body].pos.distance(caster);
        if let Some(r) = tk.after_step(&mut world, caster, None).pop() {
            released = Some((r, before));
            break;
        }
        assert!(before <= RANGE);
    }
    let (release, distance) = released.expect("released at the range limit");
    assert_eq!(release.reason, Reason::OutOfRange);
    assert_eq!(release.body, Some(body));
    assert!(distance > RANGE && distance < RANGE + 0.05, "{distance}");
    assert!(tk.grip.is_none());
    run(&mut world, &mut tk, caster, aim, 2.);
    assert!(world[body].pos.y < 0.4, "{}", world[body].pos);
}

#[test]
fn a_target_beyond_range_or_larger_than_huge_is_refused() {
    let mut world = World::new(DT);
    let caster = DVec3::ZERO;
    let far = crate_at(&mut world, DVec3::new(RANGE + 0.1, 0.3, 0.));
    let near = crate_at(&mut world, DVec3::new(3., 0.3, 0.));
    let mut tk = Telekinesis::cast(&mut world, caster, DOWN);
    let mut releases = Vec::new();
    let refusal = tk.apply(
        &mut world,
        caster,
        far,
        Size::Small,
        Target::Object,
        false,
        &mut releases,
    );
    assert_eq!(refusal, Err(Refusal::OutOfRange));
    let refusal = tk.apply(
        &mut world,
        caster,
        near,
        Size::Gargantuan,
        Target::Object,
        false,
        &mut releases,
    );
    assert_eq!(refusal, Err(Refusal::TooLarge));
    assert!(tk.grip.is_none() && releases.is_empty());
    assert_eq!(world.joints().count(), 0);
}

#[test]
fn a_failed_save_suspends_a_creature_for_exactly_six_seconds_then_it_falls() {
    let mut world = World::new(DT);
    floor(&mut world);
    let caster = DVec3::ZERO;
    let dummy = dummy_at(&mut world, 4.);
    let mut tk = Telekinesis::cast(&mut world, caster, DOWN);
    run(&mut world, &mut tk, caster, DVec3::ZERO, 1.);
    let rest = world[dummy].pos.y;
    let saved = save(2);
    assert!(!saved);
    let applied = tk
        .apply(
            &mut world,
            caster,
            dummy,
            Size::Medium,
            Target::Creature { strength: 0 },
            saved,
            &mut Vec::new(),
        )
        .unwrap();
    assert_eq!(applied, Applied::Gripped);
    let gripped = world.tick;
    let lift = 20. * FEET + 0.02;
    let aim = world[dummy].pos + DVec3::Y * lift;
    let mut peak = rest;
    let mut released = None;
    while released.is_none() {
        assert!(world.tick - gripped <= ticks(&world, ROUND));
        tk.steer(&mut world, caster, aim);
        world.step(&Uniform(DOWN));
        peak = peak.max(world[dummy].pos.y);
        if world.tick - gripped > ticks(&world, 3.) {
            // Suspended: gravity is off and the hand is still.
            assert!((world[dummy].pos.y - aim.y).abs() < 1e-3);
            assert!(tk.grip.unwrap().restrained());
        }
        released = tk.after_step(&mut world, caster, None).pop();
    }
    let release = released.unwrap();
    assert_eq!(release.reason, Reason::HoldExpired);
    assert!(release.creature);
    assert_eq!(world.tick - gripped, 720);
    run(&mut world, &mut tk, caster, aim, 3.);
    let landed = world[dummy].pos.y;
    assert!((landed - rest).abs() < 0.01, "{landed} {rest}");
    let fall = peak - landed;
    assert!(fall >= 20. * FEET && fall < 21. * FEET, "{fall}");
    assert_eq!(fall_dice(fall), 2);
}

#[test]
fn a_successful_save_means_no_movement() {
    let mut world = World::new(DT);
    floor(&mut world);
    let caster = DVec3::ZERO;
    let dummy = dummy_at(&mut world, 4.);
    let mut tk = Telekinesis::cast(&mut world, caster, DOWN);
    run(&mut world, &mut tk, caster, DVec3::ZERO, 1.);
    let before = world[dummy].pos;
    let saved = save(18);
    assert!(saved);
    let applied = tk
        .apply(
            &mut world,
            caster,
            dummy,
            Size::Medium,
            Target::Creature { strength: 0 },
            saved,
            &mut Vec::new(),
        )
        .unwrap();
    assert_eq!(applied, Applied::Resisted);
    assert!(tk.grip.is_none());
    run(&mut world, &mut tk, caster, before + DVec3::Y * 6., 3.);
    assert!(world[dummy].pos.distance(before) < 1e-3);
    assert_eq!(world.joints().count(), 0);
}

#[test]
fn released_velocity_is_preserved_and_the_ledger_balances() {
    let mut world = World::new(DT);
    let caster = DVec3::ZERO;
    let body = crate_at(&mut world, DVec3::new(3., 1., 0.));
    let mut tk = Telekinesis::cast(&mut world, caster, DVec3::ZERO);
    grab(&mut world, &mut tk, caster, body);
    let origin = DVec3::new(1., -2., 0.5);
    let mut ledger = Ledger::new(origin, world.momentum(origin));
    let aim = DVec3::new(3., 1., 9.);
    for _ in 0..ticks(&world, 1.) {
        tk.steer(&mut world, caster, aim);
        world.step(&NoField);
        tk.after_step(&mut world, caster, Some(&mut ledger));
    }
    let before = world[body].vel;
    assert!((before.z - HAND_SPEED).abs() < 0.05, "{before}");
    let release = tk.release(&mut world, Reason::Let).unwrap();
    assert_eq!(release.velocity, before);
    assert_eq!(release.momentum, before * 20.);
    for _ in 0..ticks(&world, 1.) {
        tk.steer(&mut world, caster, aim);
        world.step(&NoField);
        tk.after_step(&mut world, caster, Some(&mut ledger));
    }
    assert_eq!(world[body].vel, before);
    let thrown = ledger.external[LEDGER_TERM].linear;
    assert!(thrown.distance(release.momentum) < 1e-9, "{thrown}");
    let error = ledger.error(world.momentum(origin));
    assert!(error.linear < 1e-9 && error.angular < 1e-9, "{error:?}");
}

#[test]
fn a_gripped_body_still_stops_at_a_wall() {
    let mut world = World::new(DT);
    floor(&mut world);
    static_box(
        &mut world,
        DVec3::new(3.25, 2., 0.),
        DVec3::new(0.25, 2., 4.),
    );
    let caster = DVec3::ZERO;
    let body = crate_at(&mut world, DVec3::new(0.5, 1., 0.));
    let mut tk = Telekinesis::cast(&mut world, caster, DOWN);
    grab(&mut world, &mut tk, caster, body);
    // The hand passes through the wall; the crate does not.
    run(&mut world, &mut tk, caster, DVec3::new(7., 1., 0.), 4.);
    assert!(tk.hand_position(&world).x > 6.);
    let face = world[body].pos.x + 0.3;
    assert!(face <= 3. + 0.01, "{face}");
    assert!(face > 2.9, "{face}");
    assert!(tk.grip.is_some());
}

#[test]
fn a_gripped_crate_knocks_over_a_standing_plank() {
    let mut world = World::new(DT);
    floor(&mut world);
    let caster = DVec3::ZERO;
    let plank = block(
        &mut world,
        8.,
        DVec3::new(0.05, 0.6, 0.3),
        DVec3::new(3., 0.6, 0.),
    );
    let body = crate_at(&mut world, DVec3::new(1., 1., 0.));
    let mut tk = Telekinesis::cast(&mut world, caster, DOWN);
    grab(&mut world, &mut tk, caster, body);
    run(&mut world, &mut tk, caster, DVec3::new(6., 1., 0.), 3.);
    let up = world[plank].orientation * DVec3::Y;
    assert!(up.y < 0.5, "{up}");
}

#[test]
fn a_huge_limit_holds_a_thousand_kilogram_block_steadily() {
    let mut world = World::new(DT);
    floor(&mut world);
    let caster = DVec3::ZERO;
    let stone = block(
        &mut world,
        1_000.,
        DVec3::splat(0.5),
        DVec3::new(4., 0.5, 0.),
    );
    let mut tk = Telekinesis::cast(&mut world, caster, DOWN);
    tk.apply(
        &mut world,
        caster,
        stone,
        Size::Huge,
        Target::Object,
        false,
        &mut Vec::new(),
    )
    .unwrap();
    let aim = DVec3::new(4., 3., 0.);
    run(&mut world, &mut tk, caster, aim, 4.);
    let sag = 9.81 / (std::f64::consts::TAU * LINEAR_HZ).powi(2);
    let pos = world[stone].pos;
    assert!((pos.y - (aim.y - sag)).abs() < 0.01, "{pos}");
    assert!(world[stone].vel.length() < 1e-3);
    let joint = world.joint(tk.grip.unwrap().linear).unwrap();
    assert!(!joint.saturated);
}

#[test]
fn switching_targets_releases_the_first() {
    let mut world = World::new(DT);
    floor(&mut world);
    let caster = DVec3::ZERO;
    let first = crate_at(&mut world, DVec3::new(2., 0.3, 0.));
    let second = crate_at(&mut world, DVec3::new(-2., 0.3, 0.));
    let mut tk = Telekinesis::cast(&mut world, caster, DOWN);
    grab(&mut world, &mut tk, caster, first);
    run(&mut world, &mut tk, caster, DVec3::new(2., 2., 0.), 1.);
    let mut releases = Vec::new();
    tk.apply(
        &mut world,
        caster,
        second,
        Size::Small,
        Target::Object,
        false,
        &mut releases,
    )
    .unwrap();
    assert_eq!(releases.len(), 1);
    assert_eq!(releases[0].body, Some(first));
    assert_eq!(releases[0].reason, Reason::Switched);
    assert_eq!(tk.grip.unwrap().body, second);
    assert_eq!(world.joints().count(), 2);
}

#[test]
fn a_checkpoint_mid_grip_replays_identically() {
    let mut world = World::new(DT);
    floor(&mut world);
    let caster = DVec3::ZERO;
    let body = crate_at(&mut world, DVec3::new(2., 0.3, 0.));
    let dummy = dummy_at(&mut world, -3.);
    let mut tk = Telekinesis::cast(&mut world, caster, DOWN);
    grab(&mut world, &mut tk, caster, body);
    let aim = |t: u64| DVec3::new(2. + (t as f64 * 0.01).sin() * 3., 2.5, -1.);
    for _ in 0..90 {
        let at = aim(world.tick);
        tk.steer(&mut world, caster, at);
        world.step(&Uniform(DOWN));
        tk.after_step(&mut world, caster, None);
    }
    let saved = serde_json::to_string(&(&world, &tk)).unwrap();
    let (mut world2, mut tk2): (World, Telekinesis) = serde_json::from_str(&saved).unwrap();
    assert_eq!(world2, world);
    assert_eq!(tk2, tk);
    let (mut a, mut b) = (Trace::default(), Trace::default());
    for (w, t, trace) in [
        (&mut world, &mut tk, &mut a),
        (&mut world2, &mut tk2, &mut b),
    ] {
        for i in 0..600 {
            let at = aim(w.tick);
            t.steer(w, caster, at);
            w.step(&Uniform(DOWN));
            t.after_step(w, caster, None);
            if i == 200 {
                t.release(w, Reason::Let);
                t.apply(
                    w,
                    caster,
                    dummy,
                    Size::Medium,
                    Target::Creature { strength: 0 },
                    false,
                    &mut Vec::new(),
                )
                .unwrap();
            }
            trace.record(w);
        }
    }
    a.compare(&b, Tolerance::EXACT).unwrap();
    assert_eq!(tk, tk2);
}

#[test]
fn the_creature_spring_settles_on_the_hand() {
    let mut position = DVec3::ZERO;
    let mut velocity = DVec3::ZERO;
    let hand = DVec3::new(0., 6.2, 0.);
    let dt = 1. / 30.;
    for _ in 0..60 {
        velocity = follow(position, velocity, hand, DVec3::ZERO, dt);
        assert!(velocity.length() <= CREATURE_SPEED + 1e-9);
        position += velocity * dt;
    }
    assert!(position.distance(hand) < 1e-3, "{position}");
    assert!(velocity.length() < 1e-2);
}

mod chamber {
    use crate::play::{Ability, Game};
    use crate::playground::{creature, hall};
    use glam::Vec3;

    /// The playground hall with dummies, the wizard at `caster` facing +z.
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

    fn settle(game: &mut Game, seconds: f32, movement: [f32; 2]) {
        for _ in 0..(seconds * 30.).round() as u32 {
            game.tick(1. / 30., movement).unwrap();
        }
    }

    #[test]
    fn telekinesis_is_the_first_spell_on_the_second_action_bar() {
        let spell = Ability::Spell(0);
        assert_eq!(spell.label(), "Telekinesis");
        assert_eq!(spell.icon(), "telekinesis-icon");
        assert_eq!(
            crate::spells::spell_in_slot(0).map(|s| s.key),
            Some("telekinesis")
        );
    }

    #[test]
    fn a_held_dummy_hangs_one_round_then_falls_and_takes_falling_damage() {
        let caster = Vec3::new(-4., 0., 0.);
        let at = caster + Vec3::Z * 3.;
        let mut game = hall_game(caster, &[(2, at)]);
        game.spells.dice.force_save(2, 1).unwrap();
        game.activate(Ability::Spell(0)).unwrap();
        assert!(game.spells.holds_creature(2));
        // Strafing right raises the hand 6 m.
        settle(&mut game, 1., [1., 0.]);
        settle(&mut game, 2., [0.; 2]);
        let high = game.actor_position(2).unwrap();
        assert!(high.y > 5.9, "{high}");
        // Restrained: it does not walk back to its place.
        assert!((high.x - at.x).abs() < 0.05 && (high.z - at.z).abs() < 0.05);
        settle(&mut game, 2.5, [0.; 2]);
        assert!(game.actor_position(2).unwrap().y > 5.9);
        settle(&mut game, 2., [0.; 2]);
        assert!(!game.spells.holds_creature(2));
        assert!(game.actor_position(2).unwrap().y.abs() < 1e-3);
        let fall = game
            .spells
            .log
            .iter()
            .find(|r| r.spell == "Falling")
            .expect("the dummy fell");
        assert!(fall.text.contains("1d6"), "{}", fall.text);
    }

    #[test]
    fn a_dummy_that_saves_stays_and_the_hand_holds_nothing() {
        let caster = Vec3::new(-4., 0., 0.);
        let at = caster + Vec3::Z * 3.;
        let mut game = hall_game(caster, &[(2, at)]);
        game.spells.dice.force_save(2, 20).unwrap();
        game.activate(Ability::Spell(0)).unwrap();
        assert!(!game.spells.holds_creature(2));
        settle(&mut game, 1., [1., 0.]);
        assert!(game.actor_position(2).unwrap().distance(at) < 1e-4);
        // Movement walks the caster when the hand holds nothing.
        assert!(game.player.distance(caster) > 3.);
    }

    #[test]
    fn a_thrown_crate_shoves_a_dummy_and_jumping_lets_go() {
        let caster = Vec3::new(-4., 0., 0.);
        let dummy = caster + Vec3::Z * 6.;
        let mut game = hall_game(caster, &[(2, dummy)]);
        let spec = crate::spells::PropSpec::reference(crate::spells::PropKind::Crate);
        let index = game
            .spawn_prop("crate", spec, caster + Vec3::new(0., 0.3, 1.5), 0.)
            .unwrap();
        settle(&mut game, 0.5, [0.; 2]);
        game.activate(Ability::Spell(0)).unwrap();
        // Raise it to chest height, then push it out toward the dummy.
        settle(&mut game, 0.2, [1., 0.]);
        settle(&mut game, 0.4, [0., 1.]);
        game.jump().unwrap();
        game.tick(1. / 30., [0.; 2]).unwrap();
        let tk = &game.spells.telekinesis[&game.player_actor()];
        assert!(tk.grip.is_none());
        assert!(game.spells.world[game.spells.props[index].body].vel.z > 4.);
        settle(&mut game, 1.5, [0.; 2]);
        // A 20 kg crate at 6 m/s gives a 75 kg dummy about 1.4 m/s, which
        // friction stops in about 6 cm.
        assert!(game.actor_position(2).unwrap().distance(dummy) > 0.03);
        let error = game.spells.ledger_error();
        assert!(error.linear < crate::spells::LEDGER_TOLERANCE, "{error:?}");
    }

    #[test]
    fn a_checkpoint_mid_grip_restores_and_continues_identically() {
        let caster = Vec3::new(-4., 0., 0.);
        let mut game = hall_game(caster, &[(2, caster + Vec3::Z * 3.)]);
        let spec = crate::spells::PropSpec::reference(crate::spells::PropKind::Crate);
        game.spawn_prop("crate", spec, caster + Vec3::new(0.4, 0.3, 2.), 0.)
            .unwrap();
        game.spells.dice.force_save(2, 1).unwrap();
        settle(&mut game, 0.3, [0.; 2]);
        game.activate(Ability::Spell(0)).unwrap();
        settle(&mut game, 0.3, [1., 0.]);
        let saved = game.checkpoint().unwrap();
        let mut restored = Game::restore(&saved).unwrap();
        for i in 0..90 {
            let axes = if i < 20 { [0., 1.] } else { [0.; 2] };
            game.tick(1. / 30., axes).unwrap();
            restored.tick(1. / 30., axes).unwrap();
        }
        assert_eq!(game.checkpoint().unwrap(), restored.checkpoint().unwrap());
    }

    #[test]
    fn the_ritual_chamber_lets_the_wizard_lift_a_cultist() {
        let scene = verse_engine::director::Scene::from_json(include_bytes!(
            "../../../../assets/verse/original/ritual.json"
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
        game.player = at - Vec3::Z * 2.5;
        game.face(std::f32::consts::PI).unwrap();
        game.tick(1. / 30., [0.; 2]).unwrap();
        game.spells.dice.force_save(cultist, 1).unwrap();
        game.activate(Ability::Spell(0)).unwrap();
        assert!(game.spells.holds_creature(cultist));
        settle(&mut game, 0.3, [1., 0.]);
        settle(&mut game, 1., [0.; 2]);
        assert!(game.actor_position(cultist).unwrap().y > at.y + 1.);
    }
}
