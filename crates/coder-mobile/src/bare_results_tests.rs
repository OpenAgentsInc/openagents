//! The Grid's RESULTS board through real bare-world scenes: it stands
//! beside the live board, loads the published results only while the player
//! is inside, needs no Gym connection, opens on a tap or the accessibility
//! request, and closes on walking out. The live board's connection and
//! polling are unchanged. It reads the committed publication from a local
//! HTTP fixture; no outside network is used.
use super::{PointerPhase, Request, Scene, WorldTarget};
use crate::verse_ffi::{BareGym, VerseHandle, bare_config_with_gym};
use std::path::PathBuf;
use std::time::{Duration, Instant};

/// Serves the committed publication at `/<ref>/<path>` for any ref, as the
/// repository's raw files do, until the test ends.
fn published() -> String {
    use std::io::{BufRead, BufReader, Write};
    let dir =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../bench/terminal-bench/published");
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut line = String::new();
            if reader.read_line(&mut line).is_err() {
                continue;
            }
            loop {
                let mut header = String::new();
                if reader.read_line(&mut header).unwrap_or(0) == 0 || header.trim().is_empty() {
                    break;
                }
            }
            let path = line.split_whitespace().nth(1).unwrap_or("/");
            let body = path
                .trim_start_matches('/')
                .split_once('/')
                .map(|(_, rel)| rel)
                .filter(|rel| !rel.contains(".."))
                .and_then(|rel| std::fs::read(dir.join(rel)).ok());
            let response = match body {
                Some(body) => {
                    let mut r = format!(
                        "HTTP/1.1 200 OK\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
                        body.len()
                    )
                    .into_bytes();
                    r.extend(body);
                    r
                }
                None => b"HTTP/1.1 404 Not Found\r\ncontent-length: 0\r\nconnection: close\r\n\r\n"
                    .to_vec(),
            };
            let _ = stream.write_all(&response);
        }
    });
    format!("http://127.0.0.1:{port}/{{ref}}/")
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
            results_cache_directory: Some(
                tempfile::tempdir()
                    .unwrap()
                    .keep()
                    .to_string_lossy()
                    .into_owned(),
            ),
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
        rendered_chamber_revision: 0,
        layer: std::ptr::null_mut(),
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

#[test]
fn an_open_trace_plays_as_a_ghost_in_the_gym_and_closing_removes_it() {
    let mut scene = scene(true);
    scene.activate(true).unwrap();
    stand_before_results(&mut scene);
    scene.update(1.0).unwrap();
    let mut t = settle(&mut scene, 1.0, |s| !s.results.view().loading);
    scene.action(Request::InteractResults).unwrap();
    for command in [
        verse::gym_results::Action::Board {
            id: "tb4-fable-delegate-repro-9776".into(),
        },
        verse::gym_results::Action::Attempt {
            id: "coq-block-bound.p2".into(),
        },
        verse::gym_results::Action::Trace,
    ] {
        scene.action(Request::Results { command }).unwrap();
    }
    t = settle(&mut scene, t, |s| s.results.open_trace().is_some());
    scene.update(t + 0.1).unwrap();
    let site = scene.world.gym_site().unwrap();
    let ghost = scene
        .world
        .trace_ghost
        .expect("the ghost stands in the Gym");
    assert!(site.inside(ghost));
    assert!(
        scene
            .results
            .view()
            .replay
            .unwrap()
            .contains("the replay is at")
    );
    // Stepping to the end moves the ghost to the verifier's station.
    scene
        .action(Request::Results {
            command: verse::gym_results::Action::Seek { fraction: 1.0 },
        })
        .unwrap();
    for frame in 1..=120 {
        scene.update(t + 0.1 + f64::from(frame) / 60.0).unwrap();
    }
    let end = verse::gym_replay::ghost_at(site, verse::replay::Place::ProvingGround);
    assert!(scene.world.trace_ghost.unwrap().distance(end) < 0.1);
    scene.action(Request::CloseResults).unwrap();
    scene.update(t + 3.0).unwrap();
    assert!(scene.world.trace_ghost.is_none());
}
