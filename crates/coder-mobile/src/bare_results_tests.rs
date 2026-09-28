//! The Grid's RESULTS board through real bare-world scenes: it stands
//! beside the live board, loads the published results only while the player
//! is inside, needs no Gym connection, opens on a tap or the accessibility
//! request, and closes on walking out. The live board's connection and
//! polling are unchanged. It reads a local copy of the committed
//! publication; no network is used.
use super::{PointerPhase, Request, Scene, WorldTarget};
use crate::verse_ffi::{BareGym, VerseHandle, bare_config_with_gym};
use std::path::PathBuf;
use std::time::{Duration, Instant};

fn published() -> String {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../bench/terminal-bench/published")
        .canonicalize()
        .unwrap()
        .to_string_lossy()
        .into_owned()
}

fn scene(results_panel: bool) -> Scene {
    let mut scene = Scene::new(bare_config_with_gym(
        800,
        1200,
        2.0,
        false,
        None,
        BareGym {
            results_panel,
            results_base: Some(published()),
            ..BareGym::default()
        },
    ))
    .unwrap();
    scene.results_panel = results_panel;
    scene
}

/// Stands before the RESULTS board inside the Gym, facing the boards.
fn stand_before_results(scene: &mut Scene) {
    let site = scene.world.gym_site().unwrap();
    let mut near = verse::world::GYM_RESULTS_BOARD;
    near.x -= 3.5;
    near.y = 0.0;
    scene
        .world
        .set_spawn(site.point(near), site.yaw_of(std::f32::consts::FRAC_PI_2))
        .unwrap();
}

fn settle(scene: &mut Scene, mut t: f64, done: impl Fn(&Scene) -> bool) -> f64 {
    let start = Instant::now();
    while !done(scene) {
        assert!(start.elapsed() < Duration::from_secs(10));
        std::thread::sleep(Duration::from_millis(5));
        t += 1.0 / 60.0;
        scene.update(t).unwrap();
    }
    t
}

#[test]
fn the_results_board_loads_inside_opens_on_a_tap_and_closes_on_leaving() {
    let mut scene = scene(true);
    scene.activate(true).unwrap();
    scene.update(1.0).unwrap();
    // Outside the Gym nothing loads, and the board doesn't open.
    assert!(!scene.results.view().active);
    assert!(scene.action(Request::InteractResults).is_err());
    stand_before_results(&mut scene);
    scene.update(1.1).unwrap();
    let results = scene.world.results(scene.aspect());
    assert!(
        results.inside && results.near && results.visible,
        "{results:?}"
    );
    // Entering starts the load, with no Gym connection.
    assert!(scene.results.view().active);
    assert!(!scene.gym_board.view().configured);
    let t = settle(&mut scene, 1.1, |s| !s.results.view().loading);
    assert!(scene.results.view().page.is_some());
    assert!(scene.results_view().is_none(), "it opens only on a tap");
    // A tap on the RESULTS board opens its panel, not the live board's.
    let size = scene.lifecycle.viewport().logical_size();
    let [x, y] = [results.screen_x * size[0], results.screen_y * size[1]];
    assert_eq!(scene.world_target(x, y), Some(WorldTarget::Results));
    scene.pointer_at(2, PointerPhase::Down, x, y, t).unwrap();
    scene
        .pointer_at(2, PointerPhase::Up, x, y, t + 0.1)
        .unwrap();
    assert!(scene.results_open && !scene.gym_open);
    let view = scene.results_view().unwrap();
    assert!(view.status.contains("Publication"), "{}", view.status);
    // Choices travel through the same requests the host sends.
    scene
        .action(Request::Results {
            command: verse::gym_results::Action::Board {
                id: "tb4-fable-delegate-repro-9776".into(),
            },
        })
        .unwrap();
    let json = serde_json::to_value(scene.results_view().unwrap()).unwrap();
    assert_eq!(json["page"]["screen"], "board");
    // While it is open, touches don't turn the player.
    let yaw = scene.world.player.yaw;
    scene.pointer(3, PointerPhase::Down, 100.0, 100.0).unwrap();
    scene.pointer(3, PointerPhase::Move, 300.0, 100.0).unwrap();
    scene.pointer(3, PointerPhase::Up, 300.0, 100.0).unwrap();
    assert_eq!(scene.world.player.yaw, yaw);
    scene.action(Request::CloseResults).unwrap();
    assert!(!scene.results_open);
    // The accessibility request opens it with the same checks.
    scene.action(Request::InteractResults).unwrap();
    assert!(scene.results_open);
    // Walking out closes it and cancels the loader.
    scene.world.set_spawn(verse::world::SPAWN, 0.0).unwrap();
    scene.update(t + 1.0).unwrap();
    assert!(!scene.results_open && !scene.results.view().active);
    assert!(scene.results_view().is_none());
    assert!(
        scene
            .action(Request::Results {
                command: verse::gym_results::Action::Back,
            })
            .is_err()
    );
}

#[test]
fn without_the_native_panel_the_results_board_never_opens_or_loads() {
    let mut scene = scene(false);
    scene.activate(true).unwrap();
    stand_before_results(&mut scene);
    scene.update(1.0).unwrap();
    let results = scene.world.results(scene.aspect());
    assert!(results.inside && results.near);
    assert!(!scene.results.view().active);
    let size = scene.lifecycle.viewport().logical_size();
    let [x, y] = [results.screen_x * size[0], results.screen_y * size[1]];
    assert_eq!(scene.world_target(x, y), None);
    assert!(scene.action(Request::InteractResults).is_err());
    let packet = serde_json::to_value(scene.packet()).unwrap();
    assert_eq!(packet["results_active"], false);
}

#[test]
fn the_native_json_path_carries_the_results_screens_under_the_packet_cap() {
    let mut handle = VerseHandle {
        scene: scene(true),
        renderer: None,
        rendered_zone_revision: 0,
    };
    handle.scene.activate(true).unwrap();
    stand_before_results(&mut handle.scene);
    handle.scene.update(1.0).unwrap();
    settle(&mut handle.scene, 1.0, |s| !s.results.view().loading);
    let call = |handle: &mut VerseHandle, request: &str| -> serde_json::Value {
        let bytes = handle.call_bytes(request.as_bytes()).unwrap();
        assert!(bytes.len() < 1024 * 1024);
        serde_json::from_slice(&bytes).unwrap()
    };
    let opened = call(&mut handle, r#"{"action":"interact_results"}"#);
    assert_eq!(opened["results_open"], true, "{}", opened["error"]);
    assert_eq!(opened["results_active"], true);
    assert_eq!(opened["results_view"]["page"]["screen"], "boards");
    // A frame packet carries only the revision.
    let frame = call(&mut handle, r#"{"action":"snapshot"}"#);
    assert!(frame.get("results_view").is_none());
    let revision = frame["results_revision"].as_u64().unwrap();
    let board = call(
        &mut handle,
        r#"{"action":"results","command":{"do":"board","id":"tb21-oos-microcoder-9683"}}"#,
    );
    assert!(board["results_revision"].as_u64().unwrap() > revision);
    assert_eq!(board["results_view"]["page"]["screen"], "board");
    assert_eq!(
        board["results_view"]["page"]["tasks"]
            .as_array()
            .unwrap()
            .len(),
        65
    );
    let refused = call(
        &mut handle,
        r#"{"action":"results","command":{"do":"attempt","id":"no-such-attempt"}}"#,
    );
    assert!(refused["error"].as_str().is_some());
    // The live board's requests still answer as before.
    let live = call(&mut handle, r#"{"action":"close_results"}"#);
    assert_eq!(live["results_open"], false);
    assert!(live["gym"]["inside"].as_bool().unwrap());
}
