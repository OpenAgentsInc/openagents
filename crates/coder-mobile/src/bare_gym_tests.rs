//! The Grid's Gym through real bare-world scenes: the Gym stands in view of
//! the spawn, the labeled synthetic preview starts outside its doorway, the
//! centered stick walks in, the board loads only inside, a tap on the board
//! opens it, and walking out closes it. The native JSON path the OpenAgents
//! app uses carries the same board. No relay or Gym host is contacted.
use super::{Config, PointerPhase, Request, Scene, WorldTarget};
use crate::verse_ffi::{BareGym, VerseHandle, bare_config_with_gym};

fn preview() -> Box<Scene> {
    Scene::new(bare_config_with_gym(
        800,
        1200,
        2.0,
        false,
        None,
        BareGym {
            code: None,
            preview: true,
            panel: true,
            ..BareGym::default()
        },
    ))
    .unwrap()
}

/// The movement stick's middle in the 400 × 600 point viewport: 80 points
/// in from the left and above the bottom.
const STICK: [f32; 2] = [80.0, 520.0];

#[test]
fn the_grids_gym_stands_in_view_of_the_spawn() {
    let mut scene = Scene::new(crate::verse_ffi::bare_config(800, 1200, 2.0, false, None)).unwrap();
    scene.activate(true).unwrap();
    scene.update(1.0).unwrap();
    let gym = scene.gym();
    assert!(gym.visible && !gym.inside && !gym.near, "{gym:?}");
    // The board sits near the middle of the view, ahead past the toys.
    assert!((gym.screen_x - 0.5).abs() < 0.1, "{}", gym.screen_x);
    // Without a connection nothing loads, and the controls are refused.
    assert!(!scene.gym_board.view().configured);
    assert!(scene.action(Request::InteractGym).is_err());
    assert!(scene.action(Request::GymLaunch).is_err());
    let packet = serde_json::to_value(scene.packet()).unwrap();
    assert_eq!(packet["gym_active"], false);
    assert_eq!(packet["gym"]["inside"], false);
}

#[test]
fn the_bare_world_takes_a_gym_connection_and_still_refuses_other_panels() {
    // An invalid code is kept as the board's error, as in Coder, rather
    // than refusing the world.
    let scene = Scene::new(bare_config_with_gym(
        800,
        1200,
        2.0,
        false,
        None,
        BareGym {
            code: Some("gym-connect:not-a-grant".into()),
            preview: false,
            panel: true,
            ..BareGym::default()
        },
    ))
    .unwrap();
    assert!(scene.gym_configuration_error.is_some());
    for config in [
        Config {
            computer_hud: true,
            ..crate::verse_ffi::bare_config(800, 1200, 2.0, false, None)
        },
        Config {
            door_preferences: Some(String::new()),
            ..crate::verse_ffi::bare_config(800, 1200, 2.0, false, None)
        },
    ] {
        assert!(Scene::new(config).is_err());
    }
    // The preview is offline: it joins no relay.
    let preview = preview();
    assert!(preview.relay.is_none() && preview.synthetic);
}

#[test]
fn the_preview_walks_in_with_the_stick_and_opens_the_board_with_a_tap() {
    let mut scene = preview();
    scene.activate(true).unwrap();
    scene.update(1.0).unwrap();
    assert!(scene.world.is_bare());
    assert!(!scene.gym().inside);
    assert!(!scene.gym_board.view().active);
    // Hold the stick forward: the player walks through the doorway.
    let [x, y] = STICK;
    scene.pointer(1, PointerPhase::Down, x, y).unwrap();
    scene.pointer(1, PointerPhase::Move, x, y - 100.0).unwrap();
    for frame in 1..=160 {
        scene.update(1.0 + f64::from(frame) / 30.0).unwrap();
    }
    scene.pointer(1, PointerPhase::Up, x, y - 100.0).unwrap();
    scene.update(7.0).unwrap();
    let gym = scene.gym();
    assert!(gym.inside && gym.near && gym.visible, "{gym:?}");
    assert!(scene.gym_board.view().active);
    assert!(!scene.gym_board.view().runs.is_empty());
    assert!(scene.gym_view().is_none(), "the board opens only on a tap");
    // A tap on the board opens it.
    let size = scene.lifecycle.viewport().logical_size();
    let [x, y] = [gym.screen_x * size[0], gym.screen_y * size[1]];
    assert_eq!(scene.world_target(x, y), Some(WorldTarget::Gym));
    scene.pointer_at(2, PointerPhase::Down, x, y, 10.0).unwrap();
    scene.pointer_at(2, PointerPhase::Up, x, y, 10.1).unwrap();
    assert!(scene.gym_open);
    assert!(scene.gym_view().is_some_and(|view| !view.runs.is_empty()));
    // While it is open, touches do not move or turn the player.
    let yaw = scene.world.player.yaw;
    scene.pointer(3, PointerPhase::Down, 100.0, 100.0).unwrap();
    scene.pointer(3, PointerPhase::Move, 300.0, 100.0).unwrap();
    scene.pointer(3, PointerPhase::Up, 300.0, 100.0).unwrap();
    assert_eq!(scene.world.player.yaw, yaw);
    scene.action(Request::CloseGym).unwrap();
    assert!(!scene.gym_open);
    // Leaving the building pauses the board and closes it.
    scene.action(Request::InteractGym).unwrap();
    scene.world.set_spawn(verse::world::SPAWN, 0.0).unwrap();
    scene.update(8.0).unwrap();
    assert!(!scene.gym_open && !scene.gym_board.view().active);
    assert!(scene.gym_view().is_none());
}

#[test]
fn without_the_native_panel_the_board_never_opens() {
    let mut scene = preview();
    scene.gym_panel = false;
    scene.activate(true).unwrap();
    let site = scene.world.gym_site().unwrap();
    let mut near = verse::world::GYM_BOARD;
    near.x -= 3.0;
    near.y = 0.0;
    scene
        .world
        .set_spawn(site.point(near), site.yaw_of(std::f32::consts::FRAC_PI_2))
        .unwrap();
    scene.update(1.0).unwrap();
    let gym = scene.gym();
    assert!(gym.inside && gym.near);
    let size = scene.lifecycle.viewport().logical_size();
    let [x, y] = [gym.screen_x * size[0], gym.screen_y * size[1]];
    assert_eq!(scene.world_target(x, y), None);
    scene.pointer_at(2, PointerPhase::Down, x, y, 10.0).unwrap();
    scene.pointer_at(2, PointerPhase::Up, x, y, 10.1).unwrap();
    assert!(scene.action(Request::InteractGym).is_err());
    assert!(!scene.gym_open);
}

#[test]
fn the_native_json_path_carries_the_grids_board() {
    let mut handle = VerseHandle {
        scene: preview(),
        renderer: None,
        rendered_zone_revision: 0,
        rendered_chamber_revision: 0,
        layer: std::ptr::null_mut(),
    };
    handle.scene.activate(true).unwrap();
    // Stand before the board, facing it, as walking in would.
    let site = handle.scene.world.gym_site().unwrap();
    let mut near = verse::world::GYM_BOARD;
    near.x -= 3.0;
    near.y = 0.0;
    handle
        .scene
        .world
        .set_spawn(site.point(near), site.yaw_of(std::f32::consts::FRAC_PI_2))
        .unwrap();
    handle.scene.update(1.0).unwrap();
    let call = |handle: &mut VerseHandle, request: &str| -> serde_json::Value {
        serde_json::from_slice(&handle.call_bytes(request.as_bytes()).unwrap()).unwrap()
    };
    let opened = call(&mut handle, r#"{"action":"interact_gym"}"#);
    assert_eq!(opened["gym_open"], true, "{}", opened["error"]);
    assert_eq!(opened["gym_active"], true);
    let board = &opened["gym_board"];
    assert_eq!(board["configured"], true);
    assert!(!board["runs"].as_array().unwrap().is_empty());
    // A frame packet omits the catalog; the host asks for it by revision.
    let frame = call(&mut handle, r#"{"action":"snapshot"}"#);
    assert!(frame.get("gym_board").is_none());
    assert!(frame["gym_revision"].as_u64().is_some());
    let closed = call(&mut handle, r#"{"action":"close_gym"}"#);
    assert_eq!(closed["gym_open"], false);
}

#[test]
fn the_plain_grid_has_no_gym_boards_or_walls() {
    let mut scene = Scene::new(bare_config_with_gym(
        800,
        1200,
        2.0,
        false,
        None,
        BareGym {
            code: Some("gym-connect:not-a-grant".into()),
            panel: true,
            results_panel: true,
            evals_panel: true,
            without_gym: true,
            ..BareGym::default()
        },
    ))
    .unwrap();
    scene.remove_gym();
    scene.activate(true).unwrap();
    scene.update(1.0).unwrap();
    assert!(scene.world.gym_site().is_none());
    assert!(!scene.world.has_gym());
    assert!(scene.world.world.blockers.is_empty());
    let gym = scene.gym();
    assert!(!gym.visible && !gym.inside, "{gym:?}");
    assert!(scene.gym_configuration_error.is_none());
    assert!(scene.action(Request::InteractGym).is_err());
    assert!(scene.action(Request::InteractResults).is_err());
    assert!(scene.action(Request::InteractEvals).is_err());
}
