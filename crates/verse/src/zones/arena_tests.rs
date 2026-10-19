//! The Grid Arena inside the world runtime: entering from the plaza,
//! walking the Striker with the shared controller, its abilities and
//! restart, and the return to the saved plaza pose.

use crate::{
    controller::InputState,
    runtime::WorldRuntime,
    zones::{Intent, ZoneId, arena, atmosphere},
};

#[test]
fn the_arena_opens_from_the_plaza_and_returns_to_its_pose() {
    let mut runtime = WorldRuntime::new();
    runtime
        .set_spawn(glam::Vec3::new(3.0, 0.0, 4.0), 0.5)
        .unwrap();
    let pose = runtime.player;
    assert!(runtime.arena_press(0).is_none(), "no arena in the plaza");
    runtime.enter_arena().unwrap();
    assert_eq!(runtime.zone, ZoneId::Arena);
    assert_eq!(ZoneId::from_name("arena"), Some(ZoneId::Arena));
    assert!(!runtime.zone_loading(), "no download for a generated zone");
    assert!(atmosphere(ZoneId::Arena).validate().is_ok());
    assert!(!runtime.companion_present());
    assert_eq!(runtime.player.pos, arena::Arena::spawn());
    let snapshot = runtime.zone_snapshot(1.6);
    assert!(snapshot.controls.is_empty(), "the arena draws its own bar");
    assert!(snapshot.caption.contains("Grid Arena"));
    assert!(runtime.enter_arena().is_err(), "only from the plaza");
    runtime.zone_intent(Intent::Return).unwrap();
    assert_eq!(runtime.zone, ZoneId::Plaza);
    assert!(runtime.arena().is_none());
    assert!(runtime.player.pos.distance(pose.pos) < 1e-4);
}

#[test]
fn the_striker_walks_fights_and_restarts() {
    let mut runtime = WorldRuntime::new();
    runtime.enter_arena().unwrap();
    let start = runtime.player.pos;
    let walk = InputState {
        forward: true,
        ..InputState::default()
    };
    for _ in 0..30 {
        runtime.tick(&walk, 1.0 / 60.0);
    }
    assert!(
        runtime.player.pos.z > start.z + 2.0,
        "W walks toward the enemy"
    );
    assert_eq!(
        runtime.arena_press(0),
        Some(Ok(arena::Ability::Laser)),
        "the laser reaches across the ring"
    );
    assert_eq!(
        runtime.arena_press(0),
        Some(Err(arena::Refusal::Busy)),
        "the global cooldown holds the next"
    );
    let mesh = runtime.dynamic_mesh();
    assert!(mesh.lines.len() > 300, "both robots draw");
    assert!(runtime.arena_restart());
    assert_eq!(runtime.arena().map(arena::Arena::round), Some(1));
    assert_eq!(runtime.player.pos, arena::Arena::spawn());
}
