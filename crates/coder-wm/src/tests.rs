//! The tiling model's tests: the chords, the desks, and the desk
//! protocol's reads.

use super::*;

#[test]
fn spawn_splits_the_focused_leaf() {
    let mut wm = Manager::new();
    let a = wm.spawn();
    let b = wm.spawn();
    let tiles = wm.tiles();
    assert_eq!(tiles.len(), 2);
    assert_eq!(tiles[0].id, a);
    assert_eq!(tiles[1].id, b);
    assert!((tiles[0].rect.w - 0.5).abs() < 1e-4);
    assert!(tiles[1].focused);
}

#[test]
fn all_tiles_name_every_desk() {
    let mut wm = Manager::new();
    let a = wm.spawn();
    wm.switch_workspace(1);
    let b = wm.spawn();
    assert!(wm.focus_id(a));
    assert_eq!(wm.workspace(), 0);
    let all = wm.all_tiles();
    assert!(all.iter().any(|(desk, t)| *desk == 1 && t.id == a));
    assert!(all.iter().any(|(desk, t)| *desk == 2 && t.id == b));
}

#[test]
fn a_tall_pane_splits_top_and_bottom() {
    let mut wm = Manager::new();
    let a = wm.spawn();
    let b = wm.spawn();
    let c = wm.spawn();
    let tiles = wm.tiles();
    assert_eq!(tiles.len(), 3);
    let a_tile = tiles.iter().find(|t| t.id == a).unwrap();
    let b_tile = tiles.iter().find(|t| t.id == b).unwrap();
    let c_tile = tiles.iter().find(|t| t.id == c).unwrap();
    assert!((a_tile.rect.w - 0.5).abs() < 1e-4);
    assert!((a_tile.rect.h - 1.0).abs() < 1e-4);
    assert!((b_tile.rect.w - 0.5).abs() < 1e-4);
    assert!((b_tile.rect.h - 0.5).abs() < 1e-4);
    assert!((c_tile.rect.h - 0.5).abs() < 1e-4);
    assert!(c_tile.rect.y > b_tile.rect.y);
    assert!((b_tile.rect.x - c_tile.rect.x).abs() < 1e-4);
}

#[test]
fn movewindow_down_stacks_a_side_pair() {
    let mut wm = Manager::new();
    let a = wm.spawn();
    let b = wm.spawn();
    wm.movewindow(Dir::Down);
    let tiles = wm.tiles();
    let a_tile = tiles.iter().find(|t| t.id == a).unwrap();
    let b_tile = tiles.iter().find(|t| t.id == b).unwrap();
    assert!((a_tile.rect.w - 1.0).abs() < 1e-4);
    assert!((b_tile.rect.w - 1.0).abs() < 1e-4);
    assert!(b_tile.rect.y > a_tile.rect.y);
}

#[test]
fn togglesplit_flips_a_side_pair() {
    let mut wm = Manager::new();
    let a = wm.spawn();
    let b = wm.spawn();
    wm.togglesplit();
    let tiles = wm.tiles();
    let a_tile = tiles.iter().find(|t| t.id == a).unwrap();
    let b_tile = tiles.iter().find(|t| t.id == b).unwrap();
    assert!((a_tile.rect.w - 1.0).abs() < 1e-4);
    assert!(b_tile.rect.y > a_tile.rect.y);
}

#[test]
fn close_leaves_the_sibling() {
    let mut wm = Manager::new();
    let a = wm.spawn();
    let b = wm.spawn();
    wm.close(b);
    let tiles = wm.tiles();
    assert_eq!(tiles.len(), 1);
    assert_eq!(tiles[0].id, a);
    assert_eq!(wm.focus(), Some(a));
}

#[test]
fn movefocus_reaches_the_right_tile() {
    let mut wm = Manager::new();
    let a = wm.spawn();
    let _b = wm.spawn();
    wm.movefocus(Dir::Left);
    assert_eq!(wm.focus(), Some(a));
}

#[test]
fn movewindow_swaps_neighbors() {
    let mut wm = Manager::new();
    let a = wm.spawn();
    let b = wm.spawn();
    wm.movewindow(Dir::Left);
    let tiles = wm.tiles();
    assert_eq!(tiles[0].id, b);
    assert_eq!(tiles[1].id, a);
}

#[test]
fn workspace_switch_hides_the_old_tiles() {
    let mut wm = Manager::new();
    let _a = wm.spawn();
    wm.switch_workspace(1);
    assert!(wm.tiles().is_empty());
    let b = wm.spawn();
    assert_eq!(wm.tiles()[0].id, b);
    wm.switch_workspace(0);
    assert_eq!(wm.tiles().len(), 1);
}

#[test]
fn movetoworkspace_takes_the_window() {
    let mut wm = Manager::new();
    let a = wm.spawn();
    wm.movetoworkspace(2);
    assert_eq!(wm.workspace(), 2);
    assert_eq!(wm.tiles()[0].id, a);
    wm.switch_workspace(0);
    assert!(wm.tiles().is_empty());
}

#[test]
fn togglefloating_then_tile_again() {
    let mut wm = Manager::new();
    let a = wm.spawn();
    let _b = wm.spawn();
    wm.togglefloating();
    let tiles = wm.tiles();
    assert_eq!(tiles.iter().filter(|t| t.floating).count(), 1);
    assert_eq!(tiles.iter().filter(|t| !t.floating).count(), 1);
    wm.togglefloating();
    let tiles = wm.tiles();
    assert_eq!(tiles.len(), 2);
    assert!(tiles.iter().any(|t| t.id == a && !t.floating));
    assert_eq!(tiles.iter().filter(|t| t.floating).count(), 0);
}

#[test]
fn spawn_beside_a_float_keeps_both() {
    let mut wm = Manager::new();
    let a = wm.spawn();
    wm.togglefloating();
    let b = wm.spawn();
    let tiles = wm.tiles();
    assert!(tiles.iter().any(|t| t.id == a && t.floating));
    assert!(tiles.iter().any(|t| t.id == b && !t.floating));
}

#[test]
fn movetoworkspace_joins_a_populated_desk() {
    let mut wm = Manager::new();
    let a = wm.spawn();
    wm.switch_workspace(1);
    let b = wm.spawn();
    wm.switch_workspace(0);
    wm.movetoworkspace(1);
    assert_eq!(wm.workspace(), 1);
    let tiles = wm.tiles();
    assert_eq!(tiles.len(), 2);
    assert!(tiles.iter().any(|t| t.id == a));
    assert!(tiles.iter().any(|t| t.id == b));
}

#[test]
fn drag_float_stays_on_the_desk() {
    let mut wm = Manager::new();
    let _a = wm.spawn();
    let b = wm.spawn();
    wm.togglefloating();
    let before = wm.tiles().into_iter().find(|t| t.id == b).unwrap().rect.x;
    wm.drag_float(-0.1, 0.0);
    let after = wm.tiles().into_iter().find(|t| t.id == b).unwrap().rect.x;
    assert!((after - (before - 0.1)).abs() < 1e-3);
}

#[test]
fn resize_grab_grows_a_float() {
    let mut wm = Manager::new();
    let _a = wm.spawn();
    let b = wm.spawn();
    wm.togglefloating();
    let before = wm.tiles().into_iter().find(|t| t.id == b).unwrap().rect;
    wm.resize_grab(-0.1, 0.0, true, false);
    let after = wm.tiles().into_iter().find(|t| t.id == b).unwrap().rect;
    assert!(after.w > before.w);
    assert!(after.x < before.x);
}

#[test]
fn maximize_covers_the_workspace() {
    let mut wm = Manager::new();
    let _a = wm.spawn();
    let b = wm.spawn();
    wm.maximize();
    let tiles = wm.tiles();
    assert_eq!(tiles.len(), 1);
    assert_eq!(tiles[0].id, b);
    assert!((tiles[0].rect.w - 1.0).abs() < 1e-4);
    wm.maximize();
    assert_eq!(wm.tiles().len(), 2);
}

#[test]
fn fullscreen_covers_the_workspace() {
    let mut wm = Manager::new();
    let _a = wm.spawn();
    let b = wm.spawn();
    wm.fullscreen();
    let tiles = wm.tiles();
    assert_eq!(tiles.len(), 1);
    assert_eq!(tiles[0].id, b);
    assert!((tiles[0].rect.w - 1.0).abs() < 1e-4);
    wm.fullscreen();
    assert_eq!(wm.tiles().len(), 2);
}

#[test]
fn focus_at_picks_the_tile() {
    let mut wm = Manager::new();
    let a = wm.spawn();
    let b = wm.spawn();
    assert_eq!(wm.focus_at(0.25, 0.5), Some(a));
    assert_eq!(wm.focus_at(0.75, 0.5), Some(b));
}

#[test]
fn resize_grows_the_focused_side() {
    let mut wm = Manager::new();
    let _a = wm.spawn();
    let _b = wm.spawn();
    wm.resize(Dir::Left, 0.1);
    let tiles = wm.tiles();
    assert!(tiles[1].rect.w < 0.5);
}

#[test]
fn title_strip_hits_the_top_of_the_tile() {
    let mut wm = Manager::new();
    let _a = wm.spawn();
    let tile = wm.tiles()[0];
    assert!(tile.title_contains(0.5, 0.01, 0.05));
    assert!(!tile.title_contains(0.5, 0.4, 0.05));
    assert!(!tile.title_contains(1.2, 0.01, 0.05));
}

#[test]
fn a_pinned_window_follows_the_desk_you_switch_to() {
    let mut wm = Manager::new();
    let a = wm.spawn();
    assert!(wm.set_pinned(a, true));
    assert!(wm.is_pinned(a));
    assert!(wm.is_floating(a), "a pinned window floats");
    wm.switch_workspace(2);
    assert_eq!(wm.desk_of(a), Some(3));
    assert_eq!(wm.focus(), Some(a));
    assert!(wm.set_pinned(a, false));
    wm.switch_workspace(0);
    assert_eq!(wm.desk_of(a), Some(3));
}

#[test]
fn a_pin_survives_a_focus_change_a_raise_a_desk_switch_and_a_move() {
    let mut wm = Manager::new();
    let camera = wm.spawn();
    let pane = wm.spawn();
    assert!(wm.set_pinned(camera, true));
    assert!(wm.place_float(
        camera,
        Rect {
            x: 0.7,
            y: 0.1,
            w: 0.2,
            h: 0.2
        }
    ));
    // The focus moves to the pane and back, as the pointer moves it.
    assert!(wm.focus_id(pane));
    assert!(wm.is_pinned(camera), "a focus change keeps the pin");
    assert!(wm.focus_id(camera));
    assert!(wm.raise(camera));
    assert!(wm.is_pinned(camera), "a raise keeps the pin");
    // Another tile opens beside the pane, the way Super+Return opens one.
    assert!(wm.focus_id(pane));
    wm.spawn();
    assert!(wm.is_pinned(camera) && wm.is_floating(camera));
    // The camera moves with the desk, and a float moved by hand stays
    // pinned; only going back into the tree drops the pin.
    wm.switch_workspace(3);
    assert_eq!(wm.desk_of(camera), Some(4));
    assert!(wm.is_pinned(camera));
    assert!(wm.place_float(
        camera,
        Rect {
            x: 0.1,
            y: 0.1,
            w: 0.2,
            h: 0.2
        }
    ));
    assert!(wm.is_pinned(camera));
    assert!(wm.set_floating(camera, false));
    assert!(!wm.is_pinned(camera), "a window tiled again is not pinned");
}

#[test]
fn place_moves_a_window_without_switching_the_desk() {
    let mut wm = Manager::new();
    let a = wm.spawn();
    let b = wm.spawn();
    assert!(wm.place(b, 4));
    assert_eq!(wm.workspace(), 0);
    assert_eq!(wm.desk_of(b), Some(5));
    assert_eq!(wm.tiles().len(), 1);
    assert_eq!(wm.tiles()[0].id, a);
}

#[test]
fn raise_puts_a_float_over_the_other_floats() {
    let mut wm = Manager::new();
    let a = wm.spawn();
    let b = wm.spawn();
    assert!(wm.set_floating(a, true));
    assert!(wm.set_floating(b, true));
    assert!(wm.place_float(
        a,
        Rect {
            x: 0.1,
            y: 0.1,
            w: 0.5,
            h: 0.5
        }
    ));
    assert!(wm.place_float(
        b,
        Rect {
            x: 0.1,
            y: 0.1,
            w: 0.5,
            h: 0.5
        }
    ));
    assert_eq!(wm.tile_at(0.2, 0.2).map(|tile| tile.id), Some(b));
    assert!(wm.raise(a));
    assert_eq!(wm.tile_at(0.2, 0.2).map(|tile| tile.id), Some(a));
    assert!(!wm.raise(WinId(99)));
}

#[test]
fn set_floating_puts_a_window_back_in_the_tree() {
    let mut wm = Manager::new();
    let a = wm.spawn();
    let b = wm.spawn();
    assert!(wm.set_floating(b, true));
    assert!(wm.is_floating(b));
    assert!(wm.set_floating(b, false));
    assert!(!wm.is_floating(b));
    assert_eq!(wm.tiles().len(), 2);
    assert!(wm.tiles().iter().any(|tile| tile.id == a));
    assert!(!wm.set_floating(WinId(99), true));
}

#[test]
fn fullscreen_and_the_desk_a_window_sits_on_read_back() {
    let mut wm = Manager::new();
    let a = wm.spawn();
    assert_eq!(wm.desk_of(a), Some(1));
    assert!(!wm.is_fullscreen(a));
    wm.fullscreen();
    assert!(wm.is_fullscreen(a));
    assert_eq!(wm.desk_of(WinId(99)), None);
}
