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
    // The spawn heading faces the station, a little off the Sun line.
    let ahead = moved.dot(crate::controller::forward(super::Lagrange::spawn_yaw()));
    assert!(
        ahead > 6.0 && ahead < 9.0 && (moved.length() - ahead) < 0.5,
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
    // The forces overlay toggles and draws joint lines for the latch weld.
    let before = runtime
        .zone_state
        .lagrange
        .as_ref()
        .unwrap()
        .dynamic()
        .lines
        .len();
    runtime.zone_intent(Intent::Forces).unwrap();
    let lagrange = runtime.zone_state.lagrange.as_ref().unwrap();
    assert!(lagrange.overlay);
    assert!(lagrange.dynamic().lines.len() > before);
    assert!(
        runtime
            .zone_snapshot(1.0)
            .controls
            .iter()
            .any(|c| c.label == "Hide forces")
    );
    assert!(runtime.zone_snapshot(1.0).caption.contains(" g · spin "));
    assert!(runtime.zone_snapshot(1.0).caption.contains(" ms · "));
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

#[test]
fn only_lagrange_frames_carry_a_physical_sky_in_real_units() {
    let mut runtime = at_l1_portal();
    assert!(runtime.dynamic_mesh().sky.is_none());
    assert!(runtime.world.mesh.lit.is_empty());
    runtime.zone_intent(Intent::Enter).unwrap();
    let mesh = runtime.dynamic_mesh();
    let sky = mesh.sky.clone().expect("Lagrange frames carry a sky");
    assert!(!mesh.lit.is_empty() && !runtime.world.mesh.lit.is_empty());
    // The station pitches 30° about its truss: the Sun stands 30° above −Z.
    let expected = glam::Vec3::new(0.0, 0.5, -(3f32.sqrt() / 2.0));
    assert!(
        sky.sun_dir.angle_between(expected) < 0.01,
        "{}",
        sky.sun_dir
    );
    // About 130,000 lux at 0.99 AU, and the true angular sizes.
    assert!((125_000.0..135_000.0).contains(&sky.sun_illuminance));
    assert!((sky.sun_angular_radius.to_degrees() - 0.269).abs() < 0.005);
    assert!((sky.earth.angular_radius.to_degrees() - 0.243).abs() < 0.01);
    assert!(sky.earth.dir.angle_between(-sky.sun_dir) < 0.05);
    // The celestial basis stays a rotation.
    assert!((sky.celestial.determinant() - 1.0).abs() < 1e-4);
    runtime.zone_intent(Intent::Return).unwrap();
    assert!(runtime.dynamic_mesh().sky.is_none());
}

#[test]
fn the_plaza_renders_as_a_neon_stage_in_its_own_palette() {
    let mut runtime = at_l1_portal();
    let plaza = runtime.dynamic_mesh();
    let neon = plaza.neon.expect("plaza frames carry the neon stage");
    assert!(plaza.sky.is_none());
    // The stage keeps the amber world's field and fog.
    assert_eq!(neon.field, crate::palette::field());
    assert_eq!(neon.fog_start, crate::render::FOG_START);
    assert_eq!(neon.fog_end, crate::render::FOG_END);
    assert!(neon.line_gain >= 1.0);
    runtime.zone_intent(Intent::Enter).unwrap();
    let station = runtime.dynamic_mesh();
    assert!(station.neon.is_none() && station.sky.is_some());
}

fn gray(color: [f32; 3]) -> bool {
    color[0] == color[1] && color[1] == color[2]
}

/// Walks forward for up to `seconds`, stopping once the zone changes.
fn walk_until_zone_changes(runtime: &mut WorldRuntime, seconds: f32) -> bool {
    let revision = runtime.zone_revision;
    let forward = InputState {
        forward: true,
        ..InputState::default()
    };
    for _ in 0..(seconds * 60.0) as usize {
        runtime.tick(&forward, 1.0 / 60.0);
        if runtime.zone_revision != revision {
            return true;
        }
    }
    false
}

#[test]
#[ignore = "the Grid's ball and blocks are off for now (2026-10-01)"]
fn walking_through_the_grid_portal_enters_a_neutral_lagrange_1_and_flying_back_returns() {
    let mut runtime = WorldRuntime::bare();
    // The portal is hidden in the apps; this keeps its path working.
    runtime.open_grid_portal_for_tests();
    let gate = runtime.grid_gate().expect("the Grid's portal");
    let ball_at = runtime.ball().unwrap().body().pos;
    // The arch is drawn in the neutral palette with the player and ball, and
    // it offers no button: nothing near it admits a tap or an Enter.
    let dynamic = runtime.dynamic_mesh_with_interactions(true, true);
    let arch = gate.mesh(ZoneId::Plaza, ZoneId::Lagrange1.sign(), 0.0);
    assert!(dynamic.lines.len() >= arch.lines.len());
    assert!(dynamic.lines.iter().all(|v| gray(v.color)));
    let (front, away) = gate.front();
    runtime
        .place_player(front, away + std::f32::consts::PI)
        .unwrap();
    let snapshot = runtime.zone_snapshot(1.0);
    assert!(!snapshot.portal.near && snapshot.controls.is_empty());
    assert!(runtime.zone_intent(Intent::Enter).is_err());
    // Walking into the opening enters, with no button.
    assert!(walk_until_zone_changes(&mut runtime, 3.0));
    assert_eq!(runtime.zone, ZoneId::Lagrange1);
    assert!(runtime.is_bare() && !runtime.is_plaza());
    // Every guide, overlay, and the return arch is white or gray; the
    // station and sky keep their physical colors.
    assert!(runtime.world.mesh.lines.iter().all(|v| gray(v.color)));
    runtime.zone_intent(Intent::Forces).unwrap();
    let dynamic = runtime.dynamic_mesh_with_interactions(true, true);
    assert!(!dynamic.lines.is_empty() && !dynamic.lit.is_empty());
    assert!(
        dynamic
            .lines
            .iter()
            .chain(&dynamic.faces)
            .all(|v| gray(v.color))
    );
    assert!(dynamic.sky.is_some());
    let snapshot = runtime.zone_snapshot(1.0);
    assert!(snapshot.controls.iter().any(|c| c.action == Intent::Grab));
    let back = snapshot
        .controls
        .iter()
        .find(|c| c.action == Intent::Return)
        .unwrap();
    assert_eq!(back.label, "The Grid");
    // Fly the pack through the return arch: it comes back on its own.
    let portal = ZoneId::Lagrange1.portal();
    runtime
        .zone_state
        .lagrange
        .as_mut()
        .unwrap()
        .station
        .apply(verse_lagrange::Input::FlyTo {
            target: (portal + glam::Vec3::new(0.0, 0.9, 0.0)).as_dvec3(),
        })
        .unwrap();
    let mut returned = false;
    for _ in 0..(90 * 10) {
        runtime.tick(&InputState::default(), 0.1);
        if runtime.is_plaza() {
            returned = true;
            break;
        }
    }
    assert!(returned, "the pack reached the return arch");
    // On the Grid again, in front of the portal facing away from it, with the
    // ball where it was left and the Grid's ground in the neutral palette.
    let (front, away) = gate.front();
    assert_eq!(runtime.player.pos, front);
    assert!((runtime.player.yaw - crate::controller::wrap(away)).abs() < 1e-5);
    assert_eq!(runtime.grid_gate(), Some(gate));
    assert!(runtime.ball().unwrap().body().pos.distance(ball_at) < 1e-6);
    // The Grid's static world again: its ground and its Gym.
    assert_eq!(runtime.world.mesh.faces, crate::world::bare().mesh.faces);
    assert!(runtime.world.mesh.lines.iter().all(|v| gray(v.color)));
    // Walking on leads away; turning back and walking in enters again.
    assert!(!walk_until_zone_changes(&mut runtime, 1.0));
    assert!(runtime.is_plaza());
    let (front, away) = gate.front();
    runtime
        .place_player(front, away + std::f32::consts::PI)
        .unwrap();
    assert!(walk_until_zone_changes(&mut runtime, 3.0));
    // The HUD's button returns too.
    runtime.zone_intent(Intent::Return).unwrap();
    assert!(runtime.is_plaza());
    assert_eq!(runtime.player.pos, gate.front().0);
}

#[test]
fn the_grid_shows_no_portal_while_it_is_hidden() {
    const { assert!(!super::gate::GRID_PORTAL_OPEN) };
    // Without zone storage the Grid has no Everglade arch either, so it
    // draws no arch at all.
    let mut runtime = WorldRuntime::bare();
    assert!(runtime.grid_gate().is_none());
    assert!(runtime.grid_portal_mesh().lines.is_empty());
    assert!(runtime.grid_portal_mesh().faces.is_empty());
    // Walking where the arch would stand, facing through it, stays on the
    // Grid, and nothing offers to enter Lagrange 1.
    let layout = crate::blocks::Layout::grid();
    let (front, away) = super::Gate::grid(&layout).front();
    runtime
        .place_player(front, away + std::f32::consts::PI)
        .unwrap();
    assert!(!walk_until_zone_changes(&mut runtime, 3.0));
    assert!(runtime.is_plaza());
    let snapshot = runtime.zone_snapshot(1.0);
    assert!(!snapshot.portal.near && snapshot.controls.is_empty());
    assert!(runtime.zone_intent(Intent::Enter).is_err());
}

#[test]
fn coder_plaza_arches_still_need_their_button_and_stay_amber() {
    let mut runtime = WorldRuntime::new();
    assert!(runtime.grid_gate().is_none());
    let at = ZoneId::Plaza.portals()[1].1;
    assert_eq!(ZoneId::Plaza.portals()[1].0, ZoneId::Lagrange1);
    runtime.set_spawn(at - glam::Vec3::Z * 3.0, 0.0).unwrap();
    assert!(!walk_until_zone_changes(&mut runtime, 2.0));
    assert!(runtime.is_plaza());
    runtime.set_spawn(at - glam::Vec3::Z * 2.0, 0.0).unwrap();
    runtime.zone_intent(Intent::Enter).unwrap();
    assert_eq!(runtime.zone, ZoneId::Lagrange1);
    let lagrange = runtime.zone_state.lagrange.as_ref().unwrap();
    assert!(!lagrange.neutral);
    assert!(lagrange.dynamic().lines.iter().any(|v| !gray(v.color)));
    assert!(runtime.world.mesh.lines.iter().any(|v| !gray(v.color)));
    let snapshot = runtime.zone_snapshot(1.0);
    assert!(
        snapshot
            .controls
            .iter()
            .any(|c| c.action == Intent::Return && c.label == "Plaza")
    );
}

#[test]
fn the_bare_world_is_named_the_grid_and_coders_plaza_keeps_its_name() {
    let grid = WorldRuntime::bare();
    assert_eq!(grid.zone_label(), "The Grid");
    assert_eq!(grid.zone_snapshot(1.0).label, "The Grid");
    let plaza = WorldRuntime::new();
    assert_eq!(plaza.zone_label(), ZoneId::Plaza.label());
    assert_eq!(plaza.zone_snapshot(1.0).label, "Amber plaza");
}

#[test]
fn the_tether_button_unclips_and_clips_back_on() {
    let mut runtime = at_l1_portal();
    runtime.zone_intent(Intent::Enter).unwrap();
    let control = |runtime: &WorldRuntime| {
        runtime
            .zone_snapshot(1.0)
            .controls
            .into_iter()
            .find(|c| c.action == Intent::Tether)
            .expect("a tether control")
    };
    let unclip = control(&runtime);
    assert_eq!((unclip.label.as_str(), unclip.enabled), ("Unclip", true));
    runtime.zone_intent(Intent::Tether).unwrap();
    let clip = control(&runtime);
    // The clip is still at hand, so it clips straight back on.
    assert_eq!((clip.label.as_str(), clip.enabled), ("Clip", true));
    // Out of reach of the clip, the control is shown but disabled.
    runtime
        .zone_state
        .lagrange
        .as_mut()
        .unwrap()
        .station
        .translate(glam::DVec3::new(10.0, 0.0, 0.0));
    let far = control(&runtime);
    assert_eq!((far.label.as_str(), far.enabled), ("Clip", false));
    assert!(runtime.zone_intent(Intent::Tether).is_err());
    runtime
        .zone_state
        .lagrange
        .as_mut()
        .unwrap()
        .station
        .translate(glam::DVec3::new(-10.0, 0.0, 0.0));
    runtime.zone_intent(Intent::Tether).unwrap();
    assert_eq!(control(&runtime).label, "Unclip");
    assert!(runtime.zone_snapshot(1.0).station.unwrap().tethered);
}

/// The solids the lines wrap around follow the drawn structure: every
/// vertex of the station and its wings lies within a centimeter of one.
#[test]
fn the_lines_wrap_the_structure_as_it_is_drawn() {
    let solids = verse_lagrange::station::structure_solids();
    let mut drawn = Vec::new();
    super::lagrange::structure_vertices(&mut drawn);
    assert!(drawn.len() > 1_000);
    for vertex in drawn {
        let p = glam::DVec3::from(vertex.map(f64::from));
        let nearest = solids
            .iter()
            .map(|solid| solid.distance(p).0)
            .fold(f64::INFINITY, f64::min);
        assert!(nearest < 0.01, "{p} is {nearest} m from every solid");
    }
}
