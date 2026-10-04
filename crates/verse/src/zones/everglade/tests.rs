use super::*;
use crate::{
    runtime::WorldRuntime,
    zones::{Intent, ZoneId, atmosphere},
};

/// Standing on the plaza's Everglade approach, facing through the arch.
fn at_everglade_portal() -> WorldRuntime {
    let mut runtime = WorldRuntime::new();
    let portal = ZoneId::Plaza
        .portals()
        .into_iter()
        .find(|(destination, _)| *destination == ZoneId::Everglade)
        .expect("a plaza arch to Everglade")
        .1;
    runtime.set_spawn(portal - Vec3::Z * 3.0, 0.0).unwrap();
    runtime
}

fn entered() -> WorldRuntime {
    let mut runtime = at_everglade_portal();
    runtime.zone_intent(Intent::Enter).unwrap();
    runtime
}

#[test]
fn the_everglade_portal_enters_immediately_and_returns_to_the_plaza_pose() {
    let mut runtime = at_everglade_portal();
    let pose = runtime.player;
    let plaza_faces = runtime.world.mesh.faces.len();
    let snapshot = runtime.zone_snapshot(1.0);
    assert!(snapshot.portal.near);
    let enter = snapshot.controls.iter().find(|c| c.action == Intent::Enter);
    assert_eq!(enter.map(|c| c.label.as_str()), Some("Enter Everglade"));
    runtime.zone_intent(Intent::Enter).unwrap();
    assert_eq!(runtime.zone, ZoneId::Everglade);
    assert_eq!(runtime.zone_revision, 1);
    assert!(!runtime.zone_loading(), "no download for a generated zone");
    assert_eq!(runtime.player.pos, Everglade::spawn());
    assert_eq!(runtime.zone_label(), "Everglade");
    let inside = runtime.zone_snapshot(1.0);
    assert_eq!(inside.id, ZoneId::Everglade);
    assert!(inside.caption.starts_with("Everglade"));
    assert!(
        inside
            .controls
            .iter()
            .any(|c| c.action == Intent::Return && c.label == "Plaza")
    );
    assert!(!runtime.zone_dynamic_mesh().lines.is_empty());
    // Walking works on the generated ground.
    let start = runtime.player.pos;
    for _ in 0..20 {
        runtime.tick(
            &InputState {
                forward: true,
                ..Default::default()
            },
            0.05,
        );
    }
    assert!(runtime.player.pos.distance(start) > 3.0);
    runtime.zone_intent(Intent::Return).unwrap();
    assert_eq!(runtime.zone, ZoneId::Plaza);
    assert_eq!(runtime.zone_revision, 2);
    assert_eq!(runtime.player.pos, pose.pos);
    assert_eq!(runtime.player.yaw, pose.yaw);
    assert_eq!(runtime.world.mesh.faces.len(), plaza_faces);
    assert!(runtime.zone_state.everglade.is_none());
}

#[test]
fn identity_and_atmosphere_are_the_zones_own() {
    assert_eq!(ZoneId::Everglade.world_id(), "everglade-v1");
    assert_eq!(ZoneId::Everglade.label(), "Everglade");
    assert_eq!(ZoneId::Everglade.sign(), "EVERGLADE");
    assert_eq!(
        serde_json::to_value(ZoneId::Everglade).unwrap(),
        "everglade"
    );
    let air = atmosphere(ZoneId::Everglade).validate().unwrap();
    assert_ne!(air, atmosphere(ZoneId::Plaza));
    // Green-gold: green leads, and red stays above blue.
    assert!(air.color[1] > air.color[0] && air.color[0] > air.color[2]);
    // The fog closes the view before the edge of the square.
    assert!(air.fog_end < HALF_EXTENT * 2.0 * std::f32::consts::SQRT_2);
    let portal = ZoneId::Everglade.portal();
    assert_eq!(ZoneId::Everglade.portals().len(), 1);
    assert_eq!(ZoneId::Everglade.portals()[0].0, ZoneId::Plaza);
    assert!(portal.x.hypot(portal.z) < CLEARING_RADIUS);
    assert_eq!(portal.y, height(portal.x, portal.z));
}

#[test]
fn the_ground_is_flat_in_the_clearing_and_rises_gently_within_bounds() {
    let step = 0.5;
    let n = (2.0 * HALF_EXTENT / step) as i32;
    for i in 0..=n {
        for j in 0..=n {
            let (x, z) = (
                -HALF_EXTENT + i as f32 * step,
                -HALF_EXTENT + j as f32 * step,
            );
            let h = height(x, z);
            assert!(
                h.is_finite() && (0.0..=MAX_HEIGHT).contains(&h),
                "{x},{z}: {h}"
            );
            if x.hypot(z) <= CLEARING_RADIUS {
                assert_eq!(h, 0.0, "the clearing is flat at {x},{z}");
            }
            // Gentle: no step steeper than 0.6 m per meter.
            let dx = (height(x + step, z) - h).abs();
            let dz = (height(x, z + step) - h).abs();
            assert!(dx.max(dz) <= 0.6 * step, "{x},{z} is too steep");
        }
    }
    // The ground has risen by the tree ring in every direction.
    for k in 0..36 {
        let angle = k as f32 / 36.0 * std::f32::consts::TAU;
        let (x, z) = (angle.cos() * RING_RADIUS, angle.sin() * RING_RADIUS);
        assert!(height(x, z) >= RING_RISE - UNDULATION - 1e-4);
    }
    for (x, z) in [(f32::NAN, 0.0), (0.0, f32::INFINITY)] {
        assert_eq!(height(x, z), 0.0);
    }
}

#[test]
fn the_world_covers_the_square_with_ground_and_marks_every_station() {
    let world = Everglade::world();
    assert!(world.blockers.is_empty(), "the greybox blocks nothing");
    assert!(world.mesh.faces.len() % 3 == 0 && world.mesh.lines.len() % 2 == 0);
    let extent = world
        .mesh
        .faces
        .iter()
        .fold(0.0_f32, |m, v| m.max(v.pos[0].abs()).max(v.pos[2].abs()));
    assert!((extent - HALF_EXTENT).abs() < 1e-3);
    for v in world.mesh.faces.iter().chain(&world.mesh.lines) {
        assert!(v.pos.iter().chain(&v.color).all(|x| x.is_finite()));
        assert!(v.color.iter().all(|c| (0.0..=1.0).contains(c)));
    }
    // Ground vertices lie on the height function.
    let mut ground = Mesh::default();
    super::draw::ground(&mut ground);
    assert!(!ground.faces.is_empty());
    for v in &ground.faces {
        let [x, y, z] = v.pos;
        assert!((y - height(x, z)).abs() < 1e-4, "{x},{z}");
    }
}

#[test]
fn stations_have_fixed_reachable_points_in_the_clearing() {
    let mut ids = std::collections::BTreeSet::new();
    let portal = ZoneId::Everglade.portal();
    for station in &STATIONS {
        assert!(ids.insert(station.id), "duplicate {}", station.id);
        assert!(
            station
                .sign
                .bytes()
                .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b' '),
            "{}",
            station.sign
        );
        let [x, z] = station.at;
        assert!(
            x.hypot(z) < CLEARING_RADIUS - 1.0,
            "{} is not in the clearing",
            station.id
        );
        assert_eq!(station.position().y, 0.0);
        assert!(station.marker().distance(station.position()) > 1.0);
        assert!((station.marker().x - portal.x).hypot(station.marker().z - portal.z) > 4.0);
        assert_eq!(station_near(x, z).map(|s| s.id), Some(station.id));
        for other in &STATIONS {
            if other.id != station.id {
                let d = (other.at[0] - x).hypot(other.at[1] - z);
                assert!(
                    d >= STATION_RANGE,
                    "{} and {} overlap",
                    station.id,
                    other.id
                );
            }
        }
        let route = crate::nav::plan(
            [Everglade::spawn().x, Everglade::spawn().z],
            station.at,
            &Everglade::world().blockers,
            HALF_EXTENT,
        );
        assert!(route.is_ok(), "{}: {route:?}", station.id);
    }
    assert_eq!(ids.len(), 10);
    // Inside the hall: the desks, the gallery, and the hearth.
    for id in ["desks", "library", "oracle"] {
        let station = STATIONS.iter().find(|s| s.id == id).unwrap();
        let ([cx, cz], [hx, hz]) = HALL;
        assert!((station.at[0] - cx).abs() < hx && (station.at[1] - cz).abs() < hz);
    }
}

#[test]
fn the_map_lists_the_return_portal_and_the_studio_stations() {
    let mut hud = crate::minimap::MapHud::default();
    hud.expanded = true;
    let map = hud.snapshot_for_zone(
        [393.0, 852.0],
        [0.0, -25.0],
        true,
        "",
        None,
        ZoneId::Everglade,
    );
    let ids: Vec<&str> = map.landmarks.iter().map(|l| l.id).collect();
    assert_eq!(ids[0], "return");
    assert_eq!(ids.len(), STATIONS.len());
    for station in STATIONS.iter().filter(|s| s.id != "approach") {
        assert!(ids.contains(&station.id), "{}", station.id);
    }
    assert!(
        crate::minimap::LANDMARKS
            .iter()
            .any(|l| l.id == "everglade" && l.label == "Everglade portal")
    );
}

#[test]
fn walking_and_jumping_follow_the_slope_and_the_camera_stays_above_it() {
    let mut runtime = entered();
    // On the rise, facing outward toward the tree ring.
    let at = Vec3::new(0.0, 0.0, -40.0);
    runtime
        .set_spawn(at.with_y(height(at.x, at.z)), std::f32::consts::PI)
        .unwrap();
    let start = runtime.player.pos;
    let forward = InputState {
        forward: true,
        ..Default::default()
    };
    for _ in 0..30 {
        runtime.tick(&forward, 0.05);
    }
    let p = runtime.player.pos;
    assert!(p.z < start.z - 5.0);
    assert!(p.y > start.y + 0.5, "the ground rose: {start} to {p}");
    assert!((p.y - height(p.x, p.z)).abs() < 1e-4);
    assert!(!runtime.player.airborne());
    runtime.tick(
        &InputState {
            jump: true,
            ..Default::default()
        },
        0.05,
    );
    assert!(runtime.player.pos.y > height(runtime.player.pos.x, runtime.player.pos.z));
    for _ in 0..60 {
        runtime.tick(&InputState::default(), 0.05);
    }
    let p = runtime.player.pos;
    assert!((p.y - height(p.x, p.z)).abs() < 1e-4, "landed on the slope");
    // Turn to face downhill so the camera sits behind, over rising ground.
    runtime.player.yaw = 0.0;
    runtime.camera.distance = crate::camera::MIN_DISTANCE;
    runtime.camera.pitch = -0.6;
    let eye = runtime.view(1.0).eye;
    assert!(eye.y >= height(eye.x, eye.z) + 0.399);
}

#[test]
fn the_caption_names_the_station_in_reach() {
    let mut runtime = entered();
    let wall = STATIONS.iter().find(|s| s.id == "task_wall").unwrap();
    runtime.set_spawn(wall.position(), wall.facing).unwrap();
    let caption = runtime.zone_snapshot(1.0).caption;
    assert!(
        caption.contains("Task Wall · Yard notice board"),
        "{caption}"
    );
    // At most four HUD lines.
    assert!(caption.lines().count() <= 4);
}

#[test]
fn other_zones_intents_are_refused_in_everglade() {
    let mut runtime = at_everglade_portal();
    runtime.zone_intent(Intent::Enter).unwrap();
    for intent in [
        Intent::Enter,
        Intent::Grab,
        Intent::Tether,
        Intent::Increase,
        Intent::Step,
        Intent::Fireball,
    ] {
        assert!(runtime.zone_intent(intent).is_err(), "{intent:?}");
        assert_eq!(runtime.zone, ZoneId::Everglade);
    }
}
