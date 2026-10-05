//! Tests for the rules as the layout applies them.
//!
//! The tests need no display. Where the layout puts a window under a rule
//! is a function of the table and of the layout crate, so each test reads
//! the table with `coder_binds::matching` and drives `apply` against
//! `coder-wm`'s manager, the one the compositor holds.

use super::*;
use crate::extras::tests::host_rules;
use crate::layout::{Placed, Screen};
use crate::xwayland::fraction;

const SCREEN: Screen = Screen {
    width: 1280,
    height: 800,
};

/// A layout holding one tiled window, which is what the compositor's
/// `add_window` leaves before a rule applies.
fn layout_with_one() -> (Manager, WinId) {
    let mut manager = Manager::new();
    let id = manager.spawn();
    (manager, id)
}

#[test]
fn a_window_with_no_rule_maps_as_a_tile() {
    let (mut manager, first) = layout_with_one();
    let xterm = manager.spawn();
    let effects = coder_binds::matching("XTerm", "xterm");
    assert_eq!(effects, Effects::default());
    apply(&mut manager, xterm, effects, None);
    assert!(!manager.is_floating(xterm));
    assert!(!manager.is_pinned(xterm));
    let tiles = manager.tiles();
    assert_eq!(tiles.len(), 2);
    assert!(tiles.iter().all(|tile| !tile.floating));
    // Two tiles split the desk between them.
    let rect = manager.rect_of(xterm).expect("the window is on the desk");
    let beside = manager.rect_of(first).expect("the first window stays");
    assert!(rect.w * rect.h < 1.0 && beside.w * beside.h < 1.0);
    assert_ne!((rect.x, rect.y), (beside.x, beside.y));
}

#[test]
fn the_battle_net_launcher_floats_in_the_middle_of_the_screen() {
    let (mut manager, id) = layout_with_one();
    let asked = fraction(
        Placed {
            x: 0,
            y: 0,
            width: 640,
            height: 400,
        },
        SCREEN,
    );
    apply(
        &mut manager,
        id,
        coder_binds::matching_with("battle.net.exe", "Battle.net", &host_rules()),
        asked,
    );
    assert!(manager.is_floating(id));
    assert!(!manager.is_pinned(id));
    let rect = manager.rect_of(id).expect("the launcher is on the desk");
    assert!((rect.w - 0.5).abs() < 1e-6);
    assert!((rect.h - 0.5).abs() < 1e-6);
    assert!((rect.x - 0.25).abs() < 1e-6);
    assert!((rect.y - 0.25).abs() < 1e-6);
}

#[test]
fn a_game_client_tiles_and_a_floating_one_goes_back_in_the_tree() {
    for (class, title) in [("SC2_x64.exe", ""), ("wine", "StarCraft II")] {
        let effects = coder_binds::matching_with(class, title, &host_rules());
        assert!(effects.suppress_fullscreen, "{class} {title}");
        let (mut manager, id) = layout_with_one();
        // A client that floated itself before its title named the game
        // goes back into a tile when the rule reads the title.
        manager.set_floating(id, true);
        apply(&mut manager, id, effects, None);
        assert!(!manager.is_floating(id), "{class} {title}");
    }
}

#[test]
fn the_emulator_floats_where_it_asked() {
    let effects =
        coder_binds::matching(coder_binds::EMULATOR_CLASS, "Android Emulator - coder:5554");
    assert!(effects.keep_aspect);
    let (mut manager, id) = layout_with_one();
    let asked = fraction(
        Placed {
            x: 128,
            y: 80,
            width: 320,
            height: 640,
        },
        SCREEN,
    );
    apply(&mut manager, id, effects, asked);
    assert!(manager.is_floating(id));
    let rect = manager.rect_of(id).expect("the emulator is on the desk");
    assert!((rect.x - 0.1).abs() < 1e-6);
    assert!((rect.y - 0.1).abs() < 1e-6);
    assert!((rect.w - 0.25).abs() < 1e-6);
    assert!((rect.h - 0.8).abs() < 1e-6);
}

#[test]
fn the_camera_circle_and_the_hud_float_pinned_on_every_desk() {
    for (app_id, title) in [("mpv", "selfie"), ("recording-hud", "recording-hud")] {
        let effects = coder_binds::matching(app_id, title);
        assert_eq!(effects.border, Some(0), "{title}");
        assert_eq!(effects.shadow, Some(false), "{title}");
        let (mut manager, id) = layout_with_one();
        apply(&mut manager, id, effects, None);
        assert!(manager.is_floating(id), "{title}");
        assert!(manager.is_pinned(id), "{title}");
        // A pinned window is on whichever desk shows.
        manager.switch_workspace(3);
        assert_eq!(manager.desk_of(id), Some(4), "{title}");
        assert!(manager.tiles().iter().any(|tile| tile.id == id), "{title}");
    }
}

#[test]
fn a_window_that_has_not_sized_itself_floats_where_the_layout_had_it() {
    let (mut manager, id) = layout_with_one();
    apply(
        &mut manager,
        id,
        coder_binds::matching_with("battle.net.exe", "", &host_rules()),
        None,
    );
    let rect = manager.rect_of(id).expect("the launcher is on the desk");
    assert!((rect.x + rect.w / 2.0 - 0.5).abs() < 1e-6);
    assert!((rect.y + rect.h / 2.0 - 0.5).abs() < 1e-6);
}
