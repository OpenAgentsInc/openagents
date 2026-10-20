//! The Prussian duel inside the world runtime: entering from the plaza,
//! footwork through the shared input, a blow, the lit stage, restart, and
//! the return to the saved plaza pose.

use crate::{
    controller::InputState,
    runtime::WorldRuntime,
    zones::{Intent, ZoneId, atmosphere, duel},
};

#[test]
fn the_duel_opens_from_the_plaza_and_returns_to_its_pose() {
    let mut runtime = WorldRuntime::new();
    runtime
        .set_spawn(glam::Vec3::new(3.0, 0.0, 4.0), 0.5)
        .unwrap();
    let pose = runtime.player;
    assert!(runtime.duel().is_none(), "no duel in the plaza");
    runtime.enter_duel().unwrap();
    assert_eq!(runtime.zone, ZoneId::Duel);
    assert!(!runtime.zone_loading(), "no download for a generated zone");
    assert!(atmosphere(ZoneId::Duel).validate().is_ok());
    assert!(!runtime.companion_present());
    assert_eq!(runtime.player.pos, duel::Duel::spawn());
    let snapshot = runtime.zone_snapshot(1.6);
    assert!(
        snapshot.controls.is_empty(),
        "the duel draws its own screen"
    );
    assert!(snapshot.caption.contains("Prussian Duel"));
    assert!(runtime.enter_duel().is_err(), "only from the plaza");
    runtime.zone_intent(Intent::Return).unwrap();
    assert_eq!(runtime.zone, ZoneId::Plaza);
    assert!(runtime.duel().is_none());
    assert!(runtime.player.pos.distance(pose.pos) < 1e-4);
}

#[test]
fn the_knight_steps_in_strikes_and_the_meadow_is_lit() {
    let mut runtime = WorldRuntime::new();
    runtime.enter_duel().unwrap();
    let start = runtime.player.pos;
    let walk = InputState {
        forward: true,
        ..InputState::default()
    };
    for _ in 0..60 {
        runtime.tick(&walk, 1.0 / 60.0);
    }
    assert!(
        runtime.player.pos.z > start.z + 0.8,
        "W steps toward the Prussian: {}",
        runtime.player.pos
    );
    let duel = runtime.duel_mut().unwrap();
    duel.aim(0.0, -40.0);
    assert_eq!(
        duel.aim_dir(),
        duel::Dir::Overhead,
        "the mouse picks the blow"
    );
    duel.attack(None).unwrap();
    let mesh = runtime.dynamic_mesh();
    assert!(mesh.lit.len() > 3000, "both fighters draw lit");
    let stage = mesh.neon.expect("Everglade's stage");
    assert!(stage.key.is_some() && stage.daylight.is_some());
    assert!(!runtime.world.mesh.lit.is_empty(), "the meadow");
    assert!(runtime.duel_restart());
    assert_eq!(runtime.duel().map(duel::Duel::round), Some(1));
    assert_eq!(runtime.player.pos, duel::Duel::spawn());
}
