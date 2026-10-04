use super::layout::{Collision, DESKS, PATHS, TASK_COLUMNS, TASK_WALL};
use super::*;
use crate::{
    pbr::textured::TexturedScene,
    runtime::WorldRuntime,
    zones::{
        Intent, ZoneId, atmosphere,
        everglade_pack::{self, PLACED_TRIANGLE_BUDGET},
    },
};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

/// The committed, pinned pack.
fn pack_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(everglade_pack::PACK_DIRECTORY)
        .join(format!(
            "{}.{}",
            everglade_pack::PACK_SHA256,
            everglade_pack::PACK_EXTENSION
        ))
}

/// The pinned pack, decoded once for every test.
pub(super) fn pack() -> &'static ZonePack {
    static PACK: OnceLock<ZonePack> = OnceLock::new();
    PACK.get_or_init(|| ZonePack::load_local(&pack_path()).expect("the committed pack loads"))
}

/// The zone's static world, built once for every test.
fn world() -> &'static World {
    static WORLD: OnceLock<World> = OnceLock::new();
    WORLD.get_or_init(|| Everglade::world(pack()).expect("the layout builds from the pack"))
}

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

pub(super) fn entered() -> WorldRuntime {
    let mut runtime = at_everglade_portal();
    runtime.install_everglade(pack());
    assert_eq!(runtime.zone, ZoneId::Everglade);
    runtime
}

#[test]
fn the_everglade_portal_loads_the_pinned_pack_and_returns_to_the_plaza_pose() {
    let mut runtime = at_everglade_portal();
    let snapshot = runtime.zone_snapshot(1.0);
    assert!(snapshot.portal.near);
    let enter = snapshot.controls.iter().find(|c| c.action == Intent::Enter);
    assert_eq!(enter.map(|c| c.label.as_str()), Some("Enter Everglade"));
    // Without zone storage the pack cannot load.
    assert!(!enter.unwrap().enabled);
    assert!(runtime.zone_intent(Intent::Enter).is_err());
    assert!(runtime.is_plaza());

    // A cache that already holds the pinned pack serves the entry offline.
    let cache = tempfile::tempdir().unwrap();
    std::fs::copy(
        pack_path(),
        cache.path().join(format!(
            "{}.{}",
            everglade_pack::PACK_SHA256,
            everglade_pack::PACK_EXTENSION
        )),
    )
    .unwrap();
    runtime.configure_zone_cache(cache.path().to_owned());
    runtime.zone_state.error = None;
    let pose = runtime.player;
    let plaza_faces = runtime.world.mesh.faces.len();
    runtime.zone_intent(Intent::Enter).unwrap();
    assert!(runtime.zone_loading());
    assert!(runtime.is_plaza(), "the plaza stays until the pack arrives");
    let caption = runtime.zone_snapshot(1.0).caption;
    assert!(caption.starts_with("Loading Everglade"), "{caption}");
    let deadline = Instant::now() + Duration::from_secs(180);
    while !runtime.zone_tick() {
        assert!(
            runtime.zone_loading(),
            "load stopped: {:?}",
            runtime.zone_state.error
        );
        assert!(Instant::now() < deadline, "the cached pack did not load");
        std::thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(runtime.zone, ZoneId::Everglade);
    assert_eq!(runtime.zone_revision, 1);
    assert!(!runtime.zone_loading());
    assert_eq!(runtime.player.pos, Everglade::spawn());
    assert_eq!(runtime.zone_label(), "Everglade");
    assert!(runtime.world.mesh.textured.is_some());
    assert!(!runtime.world.blockers.is_empty());
    let inside = runtime.zone_snapshot(1.0);
    assert_eq!(inside.id, ZoneId::Everglade);
    assert!(inside.caption.starts_with("Everglade"));
    assert!(
        inside
            .controls
            .iter()
            .any(|c| c.action == Intent::Return && c.label == "Plaza")
    );
    // Frames draw on the zone's lit stage, which textured meshes need.
    let dynamic = runtime.zone_dynamic_mesh();
    assert!(!dynamic.lines.is_empty());
    let stage = dynamic.neon.expect("a lit stage");
    assert!(stage.key.is_some());
    assert_eq!(stage.field, atmosphere(ZoneId::Everglade).color);
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
    assert!(runtime.world.mesh.textured.is_none());
    assert!(runtime.zone_state.everglade.is_none());
}

#[test]
fn pack_bytes_a_browser_downloaded_install_only_when_pinned() {
    let bytes = std::fs::read(pack_path()).expect("the committed pack reads");
    let mut runtime = WorldRuntime::new();
    assert!(runtime.install_everglade_bytes(&bytes[..1024]).is_err());
    let mut tampered = bytes.clone();
    tampered[4096] ^= 1;
    assert!(runtime.install_everglade_bytes(&tampered).is_err());
    assert_eq!(runtime.zone, ZoneId::Plaza);
    runtime.install_everglade_bytes(&bytes).unwrap();
    assert_eq!(runtime.zone, ZoneId::Everglade);
}

#[test]
fn a_canceled_load_stays_in_the_plaza() {
    let mut runtime = at_everglade_portal();
    let cache = tempfile::tempdir().unwrap();
    std::fs::copy(
        pack_path(),
        cache.path().join(format!(
            "{}.{}",
            everglade_pack::PACK_SHA256,
            everglade_pack::PACK_EXTENSION
        )),
    )
    .unwrap();
    runtime.configure_zone_cache(cache.path().to_owned());
    runtime.zone_intent(Intent::Enter).unwrap();
    assert!(runtime.zone_loading());
    runtime.zone_intent(Intent::Cancel).unwrap();
    assert!(!runtime.zone_loading());
    // A late completion cannot enter the zone after cancellation.
    std::thread::sleep(Duration::from_millis(100));
    assert!(!runtime.zone_tick());
    assert!(runtime.is_plaza());
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
fn the_world_is_ground_textured_placements_and_boards() {
    let world = world();
    assert!(world.mesh.faces.len() % 3 == 0 && world.mesh.lines.len() % 2 == 0);
    for v in world.mesh.faces.iter().chain(&world.mesh.lines) {
        assert!(v.pos.iter().chain(&v.color).all(|x| x.is_finite()));
        assert!(v.color.iter().all(|c| (0.0..=1.0).contains(c)));
    }
    // The boards are the only vertex-color faces; the ground is textured
    // (`draw`'s tests check it lies on the height function).
    assert!(!world.mesh.faces.is_empty());
    let mut ground = TexturedScene::default();
    super::draw::ground(&mut ground);
    assert!(!ground.placements.is_empty());
    // Every placement and the ground are in the scene, which validates and
    // merges into cells, with base-color images within the pack's texture
    // budget.
    let scene = world.mesh.textured.as_ref().expect("a textured scene");
    assert_eq!(
        scene.placements.len(),
        layout::placements().len() + ground.placements.len()
    );
    scene.validate().unwrap();
    let merged = scene.merge().unwrap();
    assert!(!merged.batches.is_empty());
    let images: u64 = scene.images.iter().map(|i| i.rgba.len() as u64).sum();
    assert!(images <= everglade_pack::Limits::EVERGLADE.decoded_texture_bytes);
    // The greybox markers are gone: no station posts stand in the world.
    assert!(world.mesh.lines.is_empty());
}

#[test]
fn every_placement_names_an_admitted_model_and_stays_in_the_glade() {
    let placements = layout::placements();
    assert!(placements.len() > 100);
    for placement in &placements {
        let model = pack()
            .model(placement.model)
            .unwrap_or_else(|| panic!("{} is not in the pack", placement.model));
        let set = placement.model.split('/').next().unwrap();
        assert!(
            ["nature", "village", "props"].contains(&set),
            "{}",
            placement.model
        );
        assert!(placement.scale > 0.0 && placement.scale.is_finite());
        let transform = placement.transform();
        assert!(transform.is_finite(), "{}", placement.model);
        let (min, max) = model.bounds();
        for i in 0..8 {
            let corner = Vec3::new(
                if i & 1 == 0 { min[0] } else { max[0] },
                if i & 2 == 0 { min[1] } else { max[1] },
                if i & 4 == 0 { min[2] } else { max[2] },
            );
            let p = transform.transform_point3(corner);
            assert!(
                p.x.abs() < HALF_EXTENT - 1.0 && p.z.abs() < HALF_EXTENT - 1.0,
                "{} reaches {p}",
                placement.model
            );
        }
    }
    // The strongroom keeps the metal crate; the rigged chest is not admitted.
    assert!(placements.iter().any(|p| p.model == "props/Crate_Metal"));
    assert!(pack().model("props/Chest_Wood").is_none());
}

#[test]
fn the_layout_stays_within_the_placed_triangle_budget() {
    let triangles: u64 = layout::placements()
        .iter()
        .map(|p| pack().model(p.model).unwrap().triangles())
        .sum::<u64>()
        + super::draw::triangles();
    eprintln!("Everglade places {triangles} triangles of {PLACED_TRIANGLE_BUDGET} with the ground");
    assert!(triangles <= PLACED_TRIANGLE_BUDGET, "{triangles}");
    // The player is drawn, not placed; it has its own budget in the pack.
    let player = pack().character.as_ref().expect("the player's character");
    assert!(player.triangles() <= everglade_pack::Limits::EVERGLADE.character_triangles);
}

#[test]
fn prop_bounds_become_navigation_blockers() {
    let placements = layout::placements();
    let expected: Vec<_> = placements
        .iter()
        .flat_map(|p| p.footprints(pack().model(p.model).unwrap().bounds()))
        .chain(layout::board_blockers())
        .collect();
    assert_eq!(world().blockers, expected);
    for placement in &placements {
        let footprints = placement.footprints(pack().model(placement.model).unwrap().bounds());
        match placement.collision {
            Collision::None => assert!(footprints.is_empty()),
            Collision::Bounds | Collision::Core(_) => {
                assert_eq!(footprints.len(), 1, "{}", placement.model);
                let [x, z] = placement.at;
                let f = footprints[0];
                assert!(f.max[0] > f.min[0] && f.max[1] > f.min[1]);
                if matches!(placement.collision, Collision::Core(_)) {
                    assert!(f.contains(x, z, 0.0), "{} core", placement.model);
                }
            }
            // A doorway or arch leaves two jambs with the opening between.
            Collision::Opening(_) => {
                assert_eq!(footprints.len(), 2, "{}", placement.model);
                let [x, z] = placement.at;
                assert!(footprints.iter().all(|f| !f.contains(x, z, 0.0)));
            }
        }
    }
    // The hall's walls, the strongroom's fence, and the workbenches all
    // block.
    let blocks = |x: f32, z: f32| world().blockers.iter().any(|b| b.contains(x, z, 0.0));
    let ([cx, cz], [hx, hz]) = HALL;
    assert!(blocks(cx - hx, cz) && blocks(cx + hx, cz) && blocks(-5.0, cz + hz));
    assert!(blocks(14.0, 7.0));
    for desk in DESKS {
        assert!(blocks(desk.seat[0], desk.monitor.center.z));
    }
}

#[test]
fn no_blocker_covers_a_path() {
    let blockers = &world().blockers;
    for [a, b] in PATHS {
        assert!(
            crate::nav::segment_clear(a, b, blockers, HALF_EXTENT),
            "{a:?} to {b:?} is blocked"
        );
    }
    // The paths start at the return portal and reach both doorways.
    assert_eq!(PATHS[0][0][1], RETURN_PORTAL.z + 1.0);
    let south = HALL.0[1] - HALL.1[1];
    assert!(PATHS[2..].iter().all(|[a, b]| a[1] < south && b[1] > south));
}

#[test]
fn stations_have_fixed_reachable_points_and_furniture_ahead() {
    let mut ids = std::collections::BTreeSet::new();
    let portal = ZoneId::Everglade.portal();
    let blockers = &world().blockers;
    let spawn = [Everglade::spawn().x, Everglade::spawn().z];
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
        assert!((x - portal.x).hypot(z - portal.z) > 4.0);
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
        let route = crate::nav::plan(spawn, station.at, blockers, HALF_EXTENT);
        assert!(route.is_ok(), "{}: {route:?}", station.id);
        // Its furniture stands ahead, within reach of the standing point.
        // The desks station looks between the two middle workbenches, so a
        // furniture edge within 0.1 m of the line counts.
        if station.id != "approach" {
            let ahead = crate::controller::forward(station.facing);
            let furnished = (5..=35).any(|k| {
                let p = station.position() + ahead * (k as f32 * 0.1);
                blockers.iter().any(|b| b.contains(p.x, p.z, 0.1))
            });
            assert!(furnished, "nothing stands ahead of {}", station.id);
        }
    }
    assert_eq!(ids.len(), 10);
    // Inside the hall: the desks, the gallery, and the hearth.
    for id in ["desks", "library", "oracle"] {
        let station = STATIONS.iter().find(|s| s.id == id).unwrap();
        let ([cx, cz], [hx, hz]) = HALL;
        assert!((station.at[0] - cx).abs() < hx && (station.at[1] - cz).abs() < hz);
    }
    // The coordinates the studio workspace builds on do not move.
    let at = |id: &str| STATIONS.iter().find(|s| s.id == id).unwrap().at;
    assert_eq!(at("desks"), [0.0, 5.0]);
    assert_eq!(at("task_wall"), [-7.0, -9.0]);
    assert_eq!(at("merge"), [10.5, 5.0]);
}

#[test]
fn every_seat_reaches_its_desk_and_faces_its_monitor() {
    let blockers = &world().blockers;
    let spawn = [Everglade::spawn().x, Everglade::spawn().z];
    let desks = STATIONS.iter().find(|s| s.id == "desks").unwrap();
    for desk in DESKS {
        let route = crate::nav::plan(spawn, desk.seat, blockers, HALF_EXTENT);
        assert!(route.is_ok(), "{:?}: {route:?}", desk.seat);
        // The seat itself works beside the standing point, on open floor.
        let figure = studio::at_desk(&desk);
        let route = crate::nav::plan(spawn, figure, blockers, HALF_EXTENT);
        assert!(route.is_ok(), "{figure:?}: {route:?}");
        // The seat faces +z; its monitor is ahead, facing back at it.
        let monitor = desk.monitor;
        assert!(monitor.center.z > desk.seat[1]);
        assert!((monitor.center.x - desk.seat[0]).abs() < 0.01);
        let normal = crate::controller::forward(monitor.facing);
        assert!(normal.z < -0.99);
        // Every seat is in reach of the desks station.
        let d = (desk.seat[0] - desks.at[0]).hypot(desk.seat[1] - desks.at[1]);
        assert!(d <= STATION_RANGE + 1.0, "{:?}", desk.seat);
    }
}

#[test]
fn the_task_wall_faces_its_station_with_a_column_per_state() {
    let station = STATIONS.iter().find(|s| s.id == "task_wall").unwrap();
    let toward = (TASK_WALL.center - station.position())
        .with_y(0.0)
        .normalize();
    assert!(crate::controller::forward(station.facing).dot(toward) > 0.99);
    // The board's face points back at the station.
    assert!(crate::controller::forward(TASK_WALL.facing).dot(toward) < -0.99);
    assert_eq!(TASK_COLUMNS.len(), 5);
    for title in TASK_COLUMNS {
        assert!(title.bytes().all(|b| b.is_ascii_uppercase()));
        // Each title fits its column at the board's lettering height.
        let width = title.len() as f32 * 6.0 * 0.07 / 7.0;
        assert!(width < (TASK_WALL.size[0] - 0.12) / TASK_COLUMNS.len() as f32);
    }
    // Board-space -Z is the face: it maps to the facing direction.
    let front = TASK_WALL
        .transform()
        .transform_vector3(Vec3::NEG_Z)
        .normalize();
    assert!(front.dot(crate::controller::forward(TASK_WALL.facing)) > 0.99);
}

#[test]
fn the_camera_stays_inside_the_hall_with_the_player() {
    let focus = Vec3::new(0.0, 1.6, 5.0);
    // Behind and above: pulled in under the eaves and inside the doors.
    let eye = keep_eye_inside(focus, Vec3::new(0.0, 4.8, -3.4));
    assert!(eye.z >= 1.5 - 1e-4 && eye.y <= 2.2 + 1e-4, "{eye}");
    let direction = (eye - focus).normalize();
    assert!(direction.dot(Vec3::new(0.0, 3.2, -8.4).normalize()) > 0.999);
    // Already inside: unchanged.
    let near = Vec3::new(1.0, 2.0, 3.0);
    assert_eq!(keep_eye_inside(focus, near), near);
    // Outside the hall the camera is free.
    let outside = Vec3::new(0.0, 1.6, -10.0);
    let far = Vec3::new(0.0, 6.0, -19.0);
    assert_eq!(keep_eye_inside(outside, far), far);
    let mut runtime = entered();
    runtime.set_spawn(Vec3::new(0.0, 0.0, 5.0), 0.0).unwrap();
    let view = runtime.view(1.6);
    assert!(view.eye.z >= 1.5 - 1e-4 && view.eye.y <= 2.2 + 1e-4);
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
fn walls_stop_the_player_and_the_doorways_let_it_in() {
    let mut runtime = entered();
    let forward = InputState {
        forward: true,
        ..Default::default()
    };
    // Walking north at a window stops outside the south wall.
    runtime.set_spawn(Vec3::new(-5.0, 0.0, -1.0), 0.0).unwrap();
    for _ in 0..40 {
        runtime.tick(&forward, 0.05);
    }
    assert!(
        runtime.player.pos.z < HALL.0[1] - HALL.1[1],
        "{}",
        runtime.player.pos
    );
    // Walking north through a doorway enters the hall.
    runtime.set_spawn(Vec3::new(1.0, 0.0, -1.0), 0.0).unwrap();
    for _ in 0..20 {
        runtime.tick(&forward, 0.05);
    }
    assert!(
        runtime.player.pos.z > HALL.0[1] - HALL.1[1] + 1.0,
        "{}",
        runtime.player.pos
    );
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
    let mut runtime = entered();
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

#[test]
fn a_launch_into_everglade_loads_from_anywhere_in_the_plaza() {
    let mut runtime = WorldRuntime::new();
    // No zone storage: nothing to load from.
    assert!(runtime.enter_everglade().is_err());
    assert!(runtime.is_plaza() && !runtime.zone_loading());
    let cache = cached_pack();
    runtime.configure_zone_cache(cache.path().to_path_buf());
    // At the spawn, far from every portal, the launch still enters.
    runtime.enter_everglade().unwrap();
    assert!(runtime.zone_loading());
    assert!(runtime.enter_everglade().is_err(), "one load at a time");
    finish_loading(&mut runtime);
    assert_eq!(runtime.zone, ZoneId::Everglade);
    assert!(runtime.enter_everglade().is_err(), "only from the plaza");
}

#[test]
fn a_studio_notice_leads_the_everglade_caption() {
    let mut runtime = entered();
    let wall = STATIONS.iter().find(|s| s.id == "task_wall").unwrap();
    runtime.set_spawn(wall.position(), wall.facing).unwrap();
    runtime.set_studio_notice(Some("  No coding agent can sign in.  ".into()));
    let caption = runtime.zone_snapshot(1.0).caption;
    assert!(
        caption.starts_with("No coding agent can sign in. · "),
        "{caption}"
    );
    assert!(caption.contains("Task Wall"), "{caption}");
    runtime.set_studio_notice(Some("   ".into()));
    assert!(
        !runtime
            .zone_snapshot(1.0)
            .caption
            .contains("No coding agent")
    );
}

/// A zone cache that already holds the pinned pack, so an entry loads
/// offline.
fn cached_pack() -> tempfile::TempDir {
    let cache = tempfile::tempdir().unwrap();
    std::fs::copy(
        pack_path(),
        cache.path().join(format!(
            "{}.{}",
            everglade_pack::PACK_SHA256,
            everglade_pack::PACK_EXTENSION
        )),
    )
    .unwrap();
    cache
}

/// Walks forward for up to `seconds`, stopping once `done` holds.
fn walk_until(runtime: &mut WorldRuntime, seconds: f32, done: fn(&WorldRuntime) -> bool) -> bool {
    let forward = InputState {
        forward: true,
        ..InputState::default()
    };
    for _ in 0..(seconds * 60.0) as usize {
        runtime.tick(&forward, 1.0 / 60.0);
        if done(runtime) {
            return true;
        }
    }
    false
}

/// Polls the loader until the zone installs.
fn finish_loading(runtime: &mut WorldRuntime) {
    let deadline = Instant::now() + Duration::from_secs(180);
    while !runtime.zone_tick() {
        assert!(
            runtime.zone_loading(),
            "load stopped: {:?}",
            runtime.zone_state.error
        );
        assert!(Instant::now() < deadline, "the cached pack did not load");
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// Stands in front of `gate`, facing through it.
fn facing(runtime: &mut WorldRuntime, gate: crate::zones::Gate) {
    let (front, away) = gate.front();
    runtime
        .place_player(front, away + std::f32::consts::PI)
        .unwrap();
}

#[test]
fn walking_through_the_grids_everglade_arch_loads_the_pack_and_the_grid_arch_returns() {
    let mut runtime = WorldRuntime::bare();
    // Without zone storage the Grid has no arch to Everglade.
    assert!(runtime.everglade_gate().is_none());
    let cache = cached_pack();
    runtime.configure_zone_cache(cache.path().to_owned());
    let gate = runtime
        .everglade_gate()
        .expect("the Grid's arch to Everglade");
    // The Lagrange 1 portal stays hidden: only Everglade's arch draws.
    assert!(runtime.grid_gate().is_none());
    let drawn = runtime.grid_portal_mesh();
    let arch = gate.mesh(ZoneId::Plaza, "EVERGLADE", 0.0);
    assert_eq!(drawn.lines.len(), arch.lines.len());
    assert_eq!(drawn.faces.len(), arch.faces.len());
    // No button: in front of the arch nothing offers to enter.
    facing(&mut runtime, gate);
    let snapshot = runtime.zone_snapshot(1.0);
    assert!(!snapshot.portal.near && snapshot.controls.is_empty());
    assert!(runtime.zone_intent(Intent::Enter).is_err());

    // Walking into the opening starts the pack load, on the Grid, with the
    // panel's progress and Cancel.
    assert!(walk_until(&mut runtime, 3.0, WorldRuntime::zone_loading));
    assert!(runtime.is_plaza());
    let loading = runtime.zone_snapshot(1.0);
    assert!(
        loading.caption.starts_with("Loading Everglade"),
        "{}",
        loading.caption
    );
    assert!(loading.controls.iter().any(|c| c.action == Intent::Cancel));
    finish_loading(&mut runtime);
    assert_eq!(runtime.zone, ZoneId::Everglade);
    assert!(runtime.is_bare());
    assert_eq!(runtime.player.pos, Everglade::spawn());
    // The panel's return reads The Grid, and the zone's arch is lettered
    // for it.
    let inside = runtime.zone_snapshot(1.0);
    assert!(
        inside
            .controls
            .iter()
            .any(|c| c.action == Intent::Return && c.label == "The Grid")
    );
    let back = crate::zones::Gate::fixed(RETURN_PORTAL);
    assert_eq!(
        runtime.grid_portal_mesh().lines.len(),
        back.mesh(ZoneId::Everglade, "THE GRID", 0.0).lines.len()
    );

    // Walking through the return arch, once the crossing's one-second
    // cooldown has passed, comes back in front of the Grid's arch, facing
    // away, so walking on does not enter again.
    for _ in 0..70 {
        runtime.tick(&InputState::default(), 1.0 / 60.0);
    }
    facing(&mut runtime, back);
    assert!(walk_until(&mut runtime, 3.0, WorldRuntime::is_plaza));
    let (front, away) = gate.front();
    assert_eq!(runtime.player.pos, front);
    assert!((runtime.player.yaw - crate::controller::wrap(away)).abs() < 1e-5);
    assert!(runtime.world.mesh.textured.is_none());
    assert!(!walk_until(&mut runtime, 1.0, |r| r.zone_loading() || !r.is_plaza()));

    // A failed load offers Retry on the Grid, which loads again; inside,
    // the panel's The Grid button comes back to the same place.
    facing(&mut runtime, gate);
    assert!(walk_until(&mut runtime, 3.0, WorldRuntime::zone_loading));
    runtime.zone_intent(Intent::Cancel).unwrap();
    runtime.zone_load_failed("offline");
    let failed = runtime.zone_snapshot(1.0);
    let retry = failed.controls.iter().find(|c| c.action == Intent::Retry);
    assert!(retry.is_some_and(|c| c.enabled), "{:?}", failed.controls);
    // The canceled worker may still be finishing; Retry until it starts.
    let deadline = Instant::now() + Duration::from_secs(60);
    while runtime.zone_intent(Intent::Retry).is_err() {
        assert!(Instant::now() < deadline, "{:?}", runtime.zone_state.error);
        runtime.zone_load_failed("offline");
        std::thread::sleep(Duration::from_millis(20));
    }
    finish_loading(&mut runtime);
    assert_eq!(runtime.zone, ZoneId::Everglade);
    runtime.zone_intent(Intent::Return).unwrap();
    assert!(runtime.is_plaza());
    assert_eq!(runtime.player.pos, gate.front().0);
}

#[test]
fn the_player_is_the_outfitted_character_and_no_companion_follows() {
    let mut runtime = entered();
    let idle = crate::controller::InputState::default();
    for _ in 0..5 {
        runtime.tick(&idle, 0.05);
    }
    let dynamic = runtime.dynamic_mesh();
    let figure = dynamic.figure.as_ref().expect("the posed character");
    figure.validate().unwrap();
    // It stands on the ground where the player does, about as tall as one.
    let feet = runtime.player.pos;
    let (low, high) = figure
        .vertices
        .iter()
        .fold((f32::MAX, f32::MIN), |(lo, hi), v| {
            (lo.min(v.pos[1]), hi.max(v.pos[1]))
        });
    assert!(
        (low - feet.y).abs() < 0.15,
        "feet at {low}, ground {}",
        feet.y
    );
    assert!((1.5..2.3).contains(&(high - feet.y)), "head at {high}");
    assert!(figure.vertices.iter().all(|v| {
        let p = Vec3::from(v.pos);
        (p.x - feet.x).hypot(p.z - feet.z) < 1.2
    }));
    // No boxy avatar and no spade: no amber edges at the player at all.
    let spade = runtime.agent.mesh();
    assert!(
        dynamic
            .lines
            .iter()
            .all(|v| !spade.lines.iter().any(|s| s.pos == v.pos))
    );
    assert!(!runtime.companion_present());
    assert!(!runtime.companion(1.6).near);
    assert!(!runtime.pet_companion());
}

#[test]
fn movement_drives_the_characters_clips() {
    use super::player::Motion;
    use crate::controller::InputState;
    let mut runtime = entered();
    let motion = |runtime: &WorldRuntime| {
        runtime
            .zone_state
            .everglade
            .as_ref()
            .and_then(Everglade::player_motion)
    };
    runtime.tick(&InputState::default(), 0.05);
    assert_eq!(motion(&runtime), Some(Motion::Idle));
    let forward = InputState {
        forward: true,
        ..InputState::default()
    };
    let before = runtime.dynamic_mesh().figure.unwrap().vertices;
    for _ in 0..4 {
        runtime.tick(&forward, 0.05);
    }
    assert_eq!(motion(&runtime), Some(Motion::Run));
    // The pose follows the player rather than standing still.
    assert_ne!(runtime.dynamic_mesh().figure.unwrap().vertices, before);
    let back = InputState {
        backward: true,
        ..InputState::default()
    };
    for _ in 0..4 {
        runtime.tick(&back, 0.05);
    }
    assert_eq!(motion(&runtime), Some(Motion::Walk));
    runtime.tick(
        &InputState {
            jump: true,
            ..InputState::default()
        },
        0.05,
    );
    assert_eq!(motion(&runtime), Some(Motion::Jump));
}

#[test]
fn a_station_caption_names_the_control_the_device_has() {
    use crate::runtime::InteractHint;
    let podium = STATIONS.iter().find(|s| s.id == "podium").unwrap();
    let at = Vec3::new(podium.at[0], 0.0, podium.at[1]);
    assert!(Everglade::caption(at, InteractHint::Key).contains("F opens the decisions"));
    let tap = Everglade::caption(at, InteractHint::Tap);
    assert!(tap.contains("Tap Decisions to open the decisions"), "{tap}");
    assert!(!tap.contains("F opens"));
    let none = Everglade::caption(at, InteractHint::None);
    assert!(!none.contains("F opens") && !none.contains("Tap"), "{none}");
    // Away from every station, a device with no panels is not asked to
    // walk up to one.
    let away = Vec3::new(55.0, 0.0, 55.0);
    assert!(!Everglade::caption(away, InteractHint::None).contains("station"));
    // The OpenAgents app's Grid world opens no panel; Coder's phones tap.
    assert_eq!(WorldRuntime::bare().interact_hint, InteractHint::None);
    assert_eq!(WorldRuntime::new().interact_hint, InteractHint::Key);
}

#[test]
fn movement_hotbar_levitates_changes_altitude_and_lands() {
    use crate::controller::InputState;
    let mut runtime = entered();
    runtime.zone_intent(Intent::Lower).unwrap_err();
    runtime.zone_intent(Intent::Levitate).unwrap();
    for _ in 0..30 {
        runtime.tick(&InputState::default(), 0.05);
    }
    let hovering = runtime.player.pos.y;
    assert!((hovering - 1.5).abs() < 0.01);
    runtime.zone_intent(Intent::Rise).unwrap();
    for _ in 0..30 {
        runtime.tick(&InputState::default(), 0.05);
    }
    assert!((runtime.player.pos.y - hovering - 1.5).abs() < 0.01);
    runtime.zone_intent(Intent::Lower).unwrap();
    for _ in 0..30 {
        runtime.tick(&InputState::default(), 0.05);
    }
    assert!((runtime.player.pos.y - hovering).abs() < 0.01);
    runtime.zone_intent(Intent::Levitate).unwrap();
    let before = runtime.player.pos.y;
    runtime.tick(&InputState::default(), 0.05);
    assert!((before - runtime.player.pos.y - 0.1).abs() < 0.001);
    for _ in 0..30 {
        runtime.tick(&InputState::default(), 0.05);
    }
    assert!(!runtime.player.airborne());
    runtime.zone_intent(Intent::Jump).unwrap();
    runtime.tick(&InputState::default(), 0.05);
    assert!(runtime.player.airborne());
    runtime.zone_intent(Intent::Sprint).unwrap();
    runtime.tick(
        &InputState {
            forward: true,
            ..InputState::default()
        },
        0.05,
    );
    assert!(
        (runtime.player.speed - crate::controller::RUN_SPEED * crate::controller::SPRINT_MULT)
            .abs()
            < 0.01
    );
    runtime.zone_intent(Intent::Return).unwrap();
    assert!(runtime.zone_intent(Intent::Levitate).is_err());
}
