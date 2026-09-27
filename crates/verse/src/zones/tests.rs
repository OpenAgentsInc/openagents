use super::*;
use crate::{controller::InputState, runtime::WorldRuntime};

fn assets() -> assets::LoadedAssets {
    assets::LoadedAssets::load_local(&std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../assets/verse/ruins/7c1535256a4687e70a0f624f4b97c651bfd0b36ba91347a09041698ef8d246a7.vzp")).unwrap()
}

#[test]
fn manifest_requires_the_exact_supported_profiles_and_content() {
    let valid = Manifest::ruins().unwrap();
    valid.validate().unwrap();
    for field in ["schema", "world", "ruleset", "physics", "asset_sha256"] {
        let mut v = serde_json::to_value(&valid).unwrap();
        v[field] = "unknown".into();
        assert!(
            serde_json::from_value::<Manifest>(v)
                .unwrap()
                .validate()
                .is_err()
        );
    }
    let mut invalid = valid.clone();
    invalid.asset_bytes += 1;
    assert!(invalid.validate().is_err());
    let mut v = serde_json::to_value(valid).unwrap();
    v["script"] = "run something".into();
    assert!(serde_json::from_value::<Manifest>(v).is_err());
}

#[test]
fn entering_and_returning_replace_only_the_zone_and_preserve_plaza_choices() {
    let mut runtime = WorldRuntime::new();
    runtime
        .set_spawn(glam::Vec3::new(-12.0, 0.0, 9.0), 0.7)
        .unwrap();
    runtime.hold_door_item(crate::doors::DemoItem::Bolt);
    let pose = runtime.player;
    let document = runtime.doors.document();
    let plaza_bytes = runtime.world.mesh.faces.len();
    assert!(runtime.zone_state.ruins.is_none());
    assert!(!runtime.zone_loading());
    runtime.install_ruins(assets());
    assert_eq!(runtime.zone, ZoneId::Ruins);
    assert_eq!(runtime.zone_revision, 1);
    assert_ne!(runtime.world.mesh.faces.len(), plaza_bytes);
    assert!(!runtime.computer(1.0).near);
    assert!(!runtime.gym(1.0).inside);
    assert!(!runtime.door(crate::doors::DoorId::Spark, 1.0).near);
    assert!(
        runtime.world.mesh.faces.len() * std::mem::size_of::<crate::mesh::Vertex>()
            < 96 * 1024 * 1024
    );
    assert!(
        runtime
            .world
            .mesh
            .faces
            .iter()
            .any(|v| v.color[1] > v.color[0])
    );
    runtime.zone_intent(Intent::Fireball).unwrap();
    runtime.tick(
        &InputState {
            forward: true,
            ..Default::default()
        },
        0.05,
    );
    runtime.zone_intent(Intent::Return).unwrap();
    assert_eq!(runtime.zone, ZoneId::Plaza);
    assert_eq!(runtime.zone_revision, 2);
    assert_eq!(runtime.player.pos, pose.pos);
    assert_eq!(runtime.player.yaw, pose.yaw);
    assert_eq!(runtime.doors.document(), document);
    assert_eq!(runtime.world.mesh.faces.len(), plaza_bytes);
    assert!(runtime.zone_state.ruins.is_none());
}

#[test]
fn loading_requires_explicit_nearby_entry_and_cancellation_keeps_the_plaza() {
    let mut runtime = WorldRuntime::new();
    assert!(runtime.zone_intent(Intent::Enter).is_err());
    assert_eq!(runtime.zone, ZoneId::Plaza);
    runtime
        .set_spawn(glam::Vec3::new(-12.0, 0.0, 9.0), 0.0)
        .unwrap();
    assert!(runtime.zone_intent(Intent::Enter).is_err());
    runtime.zone_cancel_loading();
    assert_eq!(runtime.zone_state.loading, LoadState::Idle);
    assert!(runtime.zone_state.ruins.is_none());
}

#[test]
fn fog_profiles_are_independent_and_bounded() {
    assert_ne!(atmosphere(ZoneId::Ruins), atmosphere(ZoneId::Plaza));
    atmosphere(ZoneId::Ruins).validate().unwrap();
    let mut bad = atmosphere(ZoneId::Ruins);
    bad.fog_start = bad.fog_end;
    assert!(bad.validate().is_err());
    bad = atmosphere(ZoneId::Ruins);
    bad.color[1] = f32::NAN;
    assert!(bad.validate().is_err());
}

fn at_l1_portal() -> WorldRuntime {
    let mut runtime = WorldRuntime::new();
    runtime
        .set_spawn(glam::Vec3::new(12.0, 0.0, 9.0), 0.0)
        .unwrap();
    runtime
}

#[test]
fn plaza_portals_stand_clear_of_structures() {
    let world = crate::world::build();
    for (destination, at) in ZoneId::Plaza.portals() {
        assert_ne!(destination, ZoneId::Plaza);
        // The opening and approach are walkable; only the two pillars block.
        for (x, z) in [(0.0, 0.0), (0.0, -3.0), (-1.0, -1.5), (1.0, -1.5)] {
            assert!(
                !world
                    .blockers
                    .iter()
                    .any(|b| b.contains(at.x + x, at.z + z, 0.4)),
                "{destination:?} portal approach is blocked"
            );
        }
        let pillars = world
            .blockers
            .iter()
            .filter(|b| b.contains(at.x - 1.9, at.z, 0.0) || b.contains(at.x + 1.9, at.z, 0.0))
            .count();
        assert_eq!(pillars, 2, "{destination:?} pillars");
    }
}

#[test]
fn the_l1_portal_enters_immediately_and_returns_to_the_plaza_pose() {
    let mut runtime = at_l1_portal();
    let pose = runtime.player;
    let snapshot = runtime.zone_snapshot(1.0);
    assert!(snapshot.portal.near);
    let enter = snapshot.controls.iter().find(|c| c.action == Intent::Enter);
    assert_eq!(enter.map(|c| c.label.as_str()), Some("Enter L1"));
    runtime.zone_intent(Intent::Enter).unwrap();
    assert_eq!(runtime.zone, ZoneId::Lagrange1);
    assert_eq!(runtime.zone_revision, 1);
    assert!(!runtime.zone_loading(), "no download for a generated zone");
    assert!(runtime.zone_state.ruins.is_none());
    let snapshot = runtime.zone_snapshot(1.0);
    let station = snapshot.station.expect("station physics");
    assert!((1.4e6..1.6e6).contains(&station.orbit.earth_distance_km));
    assert!(snapshot.caption.starts_with("Earth 1."));
    assert!(snapshot.controls.iter().any(|c| c.action == Intent::Grab));
    assert!(atmosphere(ZoneId::Lagrange1).validate().is_ok());
    runtime.zone_intent(Intent::Return).unwrap();
    assert_eq!(runtime.zone, ZoneId::Plaza);
    assert_eq!(runtime.player.pos, pose.pos);
    assert!(runtime.zone_state.lagrange.is_none());
}

#[test]
fn l1_flight_is_inertial_and_carries_parts_to_the_jig() {
    let mut runtime = at_l1_portal();
    runtime.zone_intent(Intent::Enter).unwrap();
    let start = runtime.player.pos;
    let forward = InputState {
        forward: true,
        ..Default::default()
    };
    // 40 N on 250 kg: about 0.16 m/s^2, so ten seconds reaches 1.6 m/s.
    for _ in 0..600 {
        runtime.tick(&forward, 1.0 / 60.0);
    }
    let moved = runtime.player.pos - start;
    // Yaw pi faces -Z, toward the station.
    assert!(
        moved.z < -6.0 && moved.z > -9.0,
        "thrust moves the astronaut: {moved}"
    );
    let speed = runtime.zone_snapshot(1.0).station.unwrap().speed_m_s;
    assert!(speed > 0.5 && speed <= 2.0 + 1e-6);
    // Releasing the stick holds position rather than stopping instantly.
    runtime.tick(&InputState::default(), 1.0 / 60.0);
    assert!(runtime.zone_snapshot(1.0).station.unwrap().speed_m_s > 0.3);
    // Put the hands at the depot and fetch the main engine.
    {
        let lagrange = runtime.zone_state.lagrange.as_mut().unwrap();
        let station = &mut lagrange.station;
        station.face(-std::f64::consts::FRAC_PI_2);
        station.astronaut_mut().vel = glam::DVec3::ZERO;
        station.astronaut_mut().pos =
            verse_lagrange::PartKind::MainEngine.stowage() + glam::DVec3::new(1.5, -0.2, 0.0);
    }
    runtime.player.yaw = -std::f32::consts::FRAC_PI_2;
    runtime.tick(&InputState::default(), 1.0 / 60.0);
    runtime.zone_intent(Intent::Grab).unwrap();
    let held = runtime.zone_snapshot(1.0);
    assert_eq!(
        held.station.as_ref().unwrap().carrying,
        Some(verse_lagrange::PartKind::MainEngine)
    );
    assert!(held.controls.iter().any(|c| c.action == Intent::Release));
    // Move the held engine onto its latch and release slowly.
    {
        let lagrange = runtime.zone_state.lagrange.as_mut().unwrap();
        let station = &mut lagrange.station;
        let part = station.body(&station.parts[0]).pos;
        station.translate(verse_lagrange::PartKind::MainEngine.slot() - part);
    }
    runtime.tick(&InputState::default(), 1.0 / 60.0);
    let ready = runtime.zone_snapshot(1.0);
    assert!(ready.station.as_ref().unwrap().latch_ready);
    assert!(ready.controls.iter().any(|c| c.label == "Latch"));
    runtime.zone_intent(Intent::Release).unwrap();
    assert_eq!(runtime.zone_snapshot(1.0).station.unwrap().installed, 1);
    // Map taps become autopilot targets rather than ground routes.
    runtime.navigate_to([0.0, 10.0]).unwrap();
    assert!(!runtime.navigation().is_active());
    assert!(runtime.navigate_to([400.0, 0.0]).is_err());
}

#[test]
fn spells_and_construction_are_scoped_to_their_zones() {
    let mut runtime = at_l1_portal();
    assert!(runtime.zone_intent(Intent::Grab).is_err());
    runtime.zone_intent(Intent::Enter).unwrap();
    assert!(runtime.zone_intent(Intent::Fireball).is_err());
    assert!(runtime.zone_intent(Intent::Enter).is_err());
}
