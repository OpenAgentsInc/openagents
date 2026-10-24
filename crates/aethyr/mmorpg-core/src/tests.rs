use super::*;
use std::collections::BTreeSet;

fn world() -> World {
    World::new(
        WorldConfig {
            spawn: [0.; 3],
            spawners: Some(vec![]),
            ..Default::default()
        },
        &FlatSurface(0.),
    )
    .unwrap()
}
fn snapshot(world: &World, id: u32) -> Snapshot {
    world.snapshot(id, &mut BTreeSet::new(), 0).unwrap()
}

#[test]
fn source_class_stats_equipment_and_melee_magic_formulas_match() {
    for (class, hp, max, damage, hit, cooldown, range) in [
        (Class::Paladin, 200., 200., 300., 21, 60, 10.),
        (Class::Sorceror, 100., 100., 20., 60, 90, 40.),
        (Class::Zombie, 20., 50., 5., 60, 180, 10.),
        (Class::Warrok, 1000., 1000., 40., 90, 156, 15.),
    ] {
        let stats = class.stats();
        assert_eq!((stats.health, stats.max_health), (hp, max));
        assert_eq!(stats.damage(class, &Inventory::new(class)), damage);
        assert_eq!(class.attack_hit(), hit);
        assert_eq!(class.attack_cooldown(), cooldown);
        assert_eq!(class.range(), range);
    }
    let mut inventory = Inventory::new(Class::Paladin);
    inventory.swap(0, 24, 2).unwrap();
    assert_eq!(
        Class::Paladin.stats().damage(Class::Paladin, &inventory),
        10.
    );
    assert_eq!(inventory.slots.iter().flatten().count(), 3);
    assert_eq!(Class::Paladin.attack_duration(), 92);
    assert_eq!(Class::Sorceror.dance_duration(), 496);
}

#[test]
fn prediction_uses_source_drag_acceleration_turning_and_action_locks() {
    let mut body = Prediction::new([0.; 3], 0.);
    body.grounded = true;
    let controls = Controls {
        forward: 1.,
        turn: 1.,
        ..Default::default()
    };
    let mut expected = 0.;
    for _ in 0..60 {
        expected += (-5. * expected * DT).clamp(-expected.abs(), expected.abs());
        expected += 100. / 6. * DT;
        predict(
            &mut body,
            controls,
            Class::Paladin,
            1. / 6.,
            &FlatSurface(0.),
        );
    }
    assert!((body.velocity - expected).abs() < 0.00001);
    assert!((body.yaw - std::f32::consts::FRAC_PI_2).abs() < 0.00001);
    for _ in 0..2 {
        predict(
            &mut body,
            Controls::default(),
            Class::Paladin,
            1. / 6.,
            &FlatSurface(0.),
        );
    }
    assert!(predict(
        &mut body,
        Controls {
            attack: true,
            ..Default::default()
        },
        Class::Paladin,
        1. / 6.,
        &FlatSurface(0.)
    ));
    let position = body.position;
    let momentum = body.velocity;
    for _ in 0..90 {
        predict(
            &mut body,
            controls,
            Class::Paladin,
            1. / 6.,
            &FlatSurface(0.),
        );
    }
    assert_eq!(body.position, position);
    assert_eq!(body.velocity, momentum);
    predict(
        &mut body,
        Controls::default(),
        Class::Paladin,
        1. / 6.,
        &FlatSurface(0.),
    );
    assert_ne!(body.position, position);
}

#[test]
fn source_population_is_seeded_bounded_and_contains_both_monsters() {
    let a = World::new(WorldConfig::default(), &FlatSurface(0.)).unwrap();
    let b = World::new(WorldConfig::default(), &FlatSurface(0.)).unwrap();
    assert!(a.population().npcs > 500 && a.population().npcs < 800);
    assert_eq!(a.population().npcs, b.population().npcs);
    // An anonymous join receives the same deterministic world with a private inventory.
    let mut a = a;
    let mut b = b;
    let pa = a.join("A", Class::Paladin, &FlatSurface(0.)).unwrap();
    let pb = b.join("A", Class::Paladin, &FlatSurface(0.)).unwrap();
    let sa = serde_json::to_value(snapshot(&a, pa)).unwrap();
    let sb = serde_json::to_value(snapshot(&b, pb)).unwrap();
    assert_eq!(sa, sb);
    assert!(a.config.world_scale == 1. / 6.);
}

#[test]
fn input_ack_is_after_simulation_stale_inputs_are_ignored_and_controls_expire() {
    let mut world = world();
    let id = world.join("A", Class::Paladin, &FlatSurface(0.)).unwrap();
    world
        .input(
            id,
            7,
            Controls {
                forward: 1.,
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(snapshot(&world, id).ack_input, 0);
    world.step(&FlatSurface(0.));
    let first = snapshot(&world, id);
    assert_eq!(first.ack_input, 7);
    assert!(first.player.position[2] < 0.);
    world
        .input(
            id,
            6,
            Controls {
                forward: -1.,
                ..Default::default()
            },
        )
        .unwrap();
    for _ in 0..120 {
        world.step(&FlatSurface(0.));
    }
    let final_state = snapshot(&world, id);
    assert_eq!(final_state.ack_input, 7);
    assert!(final_state.player.prediction.unwrap().velocity.abs() < 0.001);
}

#[test]
fn short_attack_press_is_latched_until_a_tick_and_receipts_do_not_repeat_mutations() {
    let mut world = world();
    let id = world.join("A", Class::Paladin, &FlatSurface(0.)).unwrap();
    world
        .input(
            id,
            1,
            Controls {
                attack: true,
                ..Default::default()
            },
        )
        .unwrap();
    world.input(id, 2, Controls::default()).unwrap();
    world.step(&FlatSurface(0.));
    assert_eq!(snapshot(&world, id).player.state, Motion::Attack);
    let receipt = world.swap(id, "swap-1", 0, 24, 2);
    assert!(receipt.ok);
    let duplicate = world.swap(id, "swap-1", 0, 24, 2);
    assert!(duplicate.duplicate && duplicate.ok);
    assert_eq!(snapshot(&world, id).player.inventory_version, Some(1));
    assert!(!world.swap(id, "swap-1", 1, 2, 24).ok);
    assert!(!world.swap(id, "stale", 0, 2, 24).ok);
    assert_eq!(snapshot(&world, id).player.inventory_version, Some(1));
}

#[test]
fn nearby_chat_events_are_bounded_and_retries_have_one_visible_message() {
    let mut world = world();
    let a = world.join("A", Class::Paladin, &FlatSurface(0.)).unwrap();
    let b = world.join("B", Class::Sorceror, &FlatSurface(0.)).unwrap();
    assert!(world.chat(a, "chat-1", "Hello").ok);
    assert!(world.chat(a, "chat-1", "Hello").duplicate);
    assert!(!world.chat(a, "too-fast", "Again").ok);
    let snap = snapshot(&world, b);
    assert_eq!(
        snap.events
            .iter()
            .filter(|e| e.kind == "chat" && e.text.as_deref() == Some("Hello"))
            .count(),
        1
    );
    assert_eq!(snap.actors[0].name, "A");
    assert!(snap.actors[0].inventory.is_none());
    let mut known = BTreeSet::new();
    let before = world.snapshot(b, &mut known, 0).unwrap();
    assert_eq!(before.actors.len(), 1);
    world.leave(a);
    assert_eq!(
        world
            .snapshot(b, &mut known, before.last_event_id)
            .unwrap()
            .despawned,
        vec![a]
    );
}

#[test]
fn player_cap_and_oversized_or_invalid_requests_do_not_change_the_world() {
    let mut world = world();
    assert!(world.join("", Class::Paladin, &FlatSurface(0.)).is_err());
    assert!(world.join("A", Class::Zombie, &FlatSurface(0.)).is_err());
    for i in 0..32 {
        world
            .join(&format!("Player{i}"), Class::Paladin, &FlatSurface(0.))
            .unwrap();
    }
    assert_eq!(world.population().players, 32);
    assert!(
        world
            .join("Overflow", Class::Paladin, &FlatSurface(0.))
            .is_err()
    );
}

#[test]
fn source_progression_hook_keeps_cumulative_xp_without_inventing_kill_rewards() {
    let mut stats = Class::Paladin.stats();
    assert!(!stats.add_experience(99));
    assert!(stats.add_experience(1));
    assert_eq!(
        (
            stats.level,
            stats.strength,
            stats.wisdomness,
            stats.benchpress,
            stats.curl,
            stats.experience
        ),
        (2, 51, 6, 21, 102, 100)
    );
    assert!(stats.add_experience(1000));
    assert_eq!(stats.level, 3);
    assert_eq!(stats.experience, 1100);
}
