//! Tests for the screens and the monitor chords, over the layout crate and
//! no display.

use super::*;

const QHD: Screen = Screen {
    width: 2560,
    height: 1440,
};

const FHD: Screen = Screen {
    width: 1920,
    height: 1080,
};

/// Two screens side by side, the way a second monitor arrives: `DP-2` on
/// the left at 1.25 and `HDMI-A-3` on the right at 1.
fn two() -> Screens {
    let mut screens = Screens::default();
    screens.add("DP-2", QHD, 1.25);
    screens.add("HDMI-A-3", FHD, 1.0);
    screens
}

#[test]
fn one_screen_shows_the_first_desk_at_the_origin() {
    let mut screens = Screens::default();
    let index = screens.add(
        "nested-1",
        Screen {
            width: 1280,
            height: 800,
        },
        1.0,
    );
    assert_eq!(index, 0);
    let head = &screens.heads()[0];
    assert_eq!(head.at, (0, 0));
    assert_eq!(head.desk, 0);
    assert_eq!(
        head.logical(),
        Screen {
            width: 1280,
            height: 800
        }
    );
    assert_eq!(
        screens.focused().map(|head| head.name.as_str()),
        Some("nested-1")
    );
}

#[test]
fn a_second_screen_sits_to_the_right_of_the_first_in_logical_pixels() {
    let screens = two();
    let heads = screens.heads();
    assert_eq!(
        heads[0].logical(),
        Screen {
            width: 2048,
            height: 1152
        }
    );
    assert_eq!(heads[1].at, (2048, 0));
    assert_eq!(
        heads[1].desk, 1,
        "the second screen shows a desk of its own"
    );
    assert_eq!(screens.bounds().width, 2048 + 1920);
}

#[test]
fn a_connector_that_reports_twice_keeps_its_place_and_desk() {
    let mut screens = two();
    let again = screens.add("DP-2", FHD, 1.0);
    assert_eq!(again, 0);
    assert_eq!(screens.heads().len(), 2);
    assert_eq!(screens.heads()[0].mode, FHD);
    assert_eq!(screens.heads()[0].desk, 0);
    assert_eq!(
        screens.heads()[1].at,
        (1536, 0),
        "the new mode moves the right screen"
    );
}

#[test]
fn an_unplugged_screen_closes_the_gap_and_hands_the_focus_back() {
    let mut screens = two();
    assert!(screens.focus(1));
    let removed = screens.remove("HDMI-A-3");
    assert_eq!(removed.map(|head| head.name), Some("HDMI-A-3".to_string()));
    assert_eq!(screens.focused_index(), 0);
    assert_eq!(screens.heads().len(), 1);
    assert!(screens.remove("HDMI-A-3").is_none());

    let mut screens = two();
    screens.remove("DP-2");
    assert_eq!(screens.heads()[0].name, "HDMI-A-3");
    assert_eq!(
        screens.heads()[0].at,
        (0, 0),
        "the remaining screen moves to the origin"
    );
}

#[test]
fn a_screen_plugged_back_in_takes_a_desk_nobody_shows() {
    let mut screens = two();
    screens.remove("HDMI-A-3");
    screens.show(1);
    let index = screens.add("HDMI-A-3", FHD, 1.0);
    assert_eq!(screens.heads()[index].desk, 0);
}

#[test]
fn the_presentation_scale_is_taken_as_asked_on_this_host() {
    assert_eq!(snap_scale(QHD, 1.25), Ok(1.25));
    assert_eq!(snap_scale(FHD, 1.25), Ok(1.25));
    assert_eq!(snap_scale(QHD, 1.0), Ok(1.0));
    assert_eq!(snap_scale(QHD, 2.0), Ok(2.0));
}

#[test]
fn a_scale_that_leaves_a_fractional_pixel_moves_to_one_that_does_not() {
    let scale = snap_scale(QHD, 1.3).expect("a scale in range");
    let size = logical(QHD, scale);
    assert!((f64::from(QHD.width) / scale - f64::from(size.width)).abs() < 1e-9);
    assert!((f64::from(QHD.height) / scale - f64::from(size.height)).abs() < 1e-9);
    assert!((scale - 1.3).abs() <= 0.1, "{scale} moved too far");
}

#[test]
fn a_scale_out_of_range_is_refused() {
    assert!(snap_scale(QHD, 0.0).is_err());
    assert!(snap_scale(QHD, 9.0).is_err());
    assert!(snap_scale(QHD, f64::NAN).is_err());
}

#[test]
fn a_scale_change_moves_the_screen_beside_it() {
    let mut screens = two();
    assert_eq!(screens.set_scale("DP-2", 1.0), Ok(1.0));
    assert_eq!(screens.heads()[1].at, (2560, 0));
    assert_eq!(screens.set_scale("DP-2", 1.25), Ok(1.25));
    assert_eq!(screens.heads()[1].at, (2048, 0));
    assert!(screens.set_scale("DP-9", 1.25).is_err());
}

#[test]
fn a_point_finds_its_screen_and_a_point_off_every_screen_is_pulled_back() {
    let screens = two();
    assert_eq!(screens.head_at(10.0, 10.0), Some(0));
    assert_eq!(screens.head_at(2100.0, 10.0), Some(1));
    assert_eq!(
        screens.head_at(2100.0, 1100.0),
        None,
        "the right screen is shorter"
    );
    let (x, y) = screens.clamp(2100.0, 1100.0);
    assert_eq!(screens.head_at(x, y), Some(1));
    let (x, y) = screens.clamp(-50.0, -50.0);
    assert_eq!((x, y), (0.0, 0.0));
}

#[test]
fn tiles_on_the_right_screen_are_offset_into_the_shared_space() {
    let screens = two();
    let whole = coder_wm::Rect {
        x: 0.0,
        y: 0.0,
        w: 1.0,
        h: 1.0,
    };
    let left = screens.heads()[0].place(whole, Fill::Tiled);
    let right = screens.heads()[1].place(whole, Fill::Tiled);
    assert_eq!(left.x, layout::GAP_OUTER);
    assert_eq!(left.width, 2048 - 2 * layout::GAP_OUTER);
    assert_eq!(right.x, 2048 + layout::GAP_OUTER);
    let filled = screens.heads()[1].place(whole, Fill::Screen);
    assert_eq!((filled.x, filled.width, filled.height), (2048, 1920, 1080));
}

#[test]
fn the_neighbor_is_the_screen_that_lies_that_way() {
    let screens = two();
    assert_eq!(screens.neighbor(0, Dir::Right), Some(1));
    assert_eq!(screens.neighbor(1, Dir::Left), Some(0));
    assert_eq!(screens.neighbor(0, Dir::Left), None);
    assert_eq!(screens.neighbor(0, Dir::Up), None);
}

#[test]
fn ctrl_alt_tab_cycles_and_wraps() {
    let mut screens = two();
    assert_eq!(screens.cycle(1), Some(1));
    assert_eq!(screens.cycle(-1), Some(1));
    screens.focus(1);
    assert_eq!(screens.cycle(1), Some(0));
    let mut alone = Screens::default();
    alone.add("DP-2", QHD, 1.0);
    assert_eq!(alone.cycle(1), None, "one screen has nothing to cycle to");
}

#[test]
fn focusing_a_screen_moves_the_layout_to_its_desk() {
    let mut screens = two();
    let mut manager = Manager::new();
    manager.spawn();
    assert!(focus_screen(&mut screens, &mut manager, 1));
    assert_eq!(manager.workspace(), 1);
    assert_eq!(manager.focus(), None, "the right screen's desk is empty");
    assert!(focus_screen(&mut screens, &mut manager, 0));
    assert_eq!(manager.workspace(), 0);
    assert!(manager.focus().is_some());
    assert!(!focus_screen(&mut screens, &mut manager, 5));
}

#[test]
fn a_digit_for_a_desk_the_other_screen_shows_moves_the_focus_there() {
    let mut screens = two();
    let mut manager = Manager::new();
    show_desk(&mut screens, &mut manager, 1);
    assert_eq!(screens.focused_index(), 1);
    assert_eq!(manager.workspace(), 1);
    assert_eq!(
        screens.heads()[0].desk,
        0,
        "the left screen still shows desk 1"
    );
    show_desk(&mut screens, &mut manager, 4);
    assert_eq!(screens.heads()[1].desk, 4);
    assert_eq!(manager.workspace(), 4);
}

#[test]
fn a_window_moved_to_the_next_screen_lands_whole_on_its_desk() {
    let mut screens = two();
    let mut manager = Manager::new();
    let first = manager.spawn();
    let moved = manager.spawn();
    assert!(move_window_to_screen(
        &mut screens,
        &mut manager,
        Dir::Right
    ));
    assert_eq!(screens.focused_index(), 1);
    assert_eq!(manager.workspace(), 1);
    assert_eq!(manager.focus(), Some(moved));
    assert_eq!(manager.desk_of(moved), Some(2));
    assert_eq!(manager.desk_of(first), Some(1));
    let tiles = manager.tiles();
    assert_eq!(
        tiles.len(),
        1,
        "the window is alone on the right screen's desk"
    );
    assert_eq!(tiles[0].rect.w, 1.0);
    assert!(!move_window_to_screen(
        &mut screens,
        &mut manager,
        Dir::Right
    ));
}

#[test]
fn an_empty_desk_moves_no_window_to_another_screen() {
    let mut screens = two();
    let mut manager = Manager::new();
    assert!(!move_window_to_screen(
        &mut screens,
        &mut manager,
        Dir::Right
    ));
    assert_eq!(screens.focused_index(), 0);
}

#[test]
fn a_moved_desk_takes_every_window_with_it() {
    let mut screens = two();
    let mut manager = Manager::new();
    let a = manager.spawn();
    let b = manager.spawn();
    assert!(move_desk_to_screen(&mut screens, &mut manager, Dir::Right));
    assert_eq!(screens.heads()[1].desk, 0);
    assert_eq!(screens.heads()[0].desk, 1);
    assert_eq!(screens.focused_index(), 1);
    assert_eq!(manager.workspace(), 0);
    assert_eq!(manager.desk_of(a), Some(1));
    assert_eq!(manager.desk_of(b), Some(1));
    assert!(!move_desk_to_screen(&mut screens, &mut manager, Dir::Up));
}

#[test]
fn a_window_sent_to_a_desk_on_the_other_screen_takes_the_focus_there() {
    let mut screens = two();
    let mut manager = Manager::new();
    let sent = manager.spawn();
    send_to_desk(&mut screens, &mut manager, 1);
    assert_eq!(screens.focused_index(), 1);
    assert_eq!(manager.focus(), Some(sent));
    assert_eq!(screens.heads()[0].desk, 0);
}

#[test]
fn the_home_of_a_hidden_desk_is_the_focused_screen() {
    let mut screens = two();
    screens.focus(1);
    assert_eq!(
        screens.home_of(0).map(|head| head.name.as_str()),
        Some("DP-2")
    );
    assert_eq!(
        screens.home_of(6).map(|head| head.name.as_str()),
        Some("HDMI-A-3")
    );
}

#[test]
fn an_exclusive_zone_is_recorded_once() {
    let mut screens = two();
    let zone = Placed {
        x: 0,
        y: 30,
        width: 2048,
        height: 1122,
    };
    assert!(screens.set_usable("DP-2", zone));
    assert!(!screens.set_usable("DP-2", zone));
    assert_eq!(screens.heads()[0].usable, zone);
    assert!(!screens.set_usable("DP-9", zone));
}

#[test]
fn a_float_normalized_on_the_second_screen_places_where_it_was_asked() {
    let screens = two();
    let head = &screens.heads()[1];
    assert_eq!(
        head.at.0, 2048,
        "the second screen sits after the scaled first"
    );
    let asked = Placed {
        x: head.at.0 + 300,
        y: 120,
        width: 400,
        height: 300,
    };
    assert_eq!(head.place(head.normalize(asked), Fill::Tiled), asked);
}

#[test]
fn focusing_a_window_on_a_hidden_desk_shows_that_desk() {
    let mut screens = Screens::default();
    screens.add("DP-2", QHD, 1.0);
    let mut manager = Manager::new();
    show_desk(&mut screens, &mut manager, 3);
    let game = manager.spawn();
    show_desk(&mut screens, &mut manager, 1);
    manager.spawn();
    assert_eq!(screens.heads()[0].desk, 1);
    assert!(focus_window(&mut screens, &mut manager, game));
    assert_eq!(
        screens.heads()[0].desk,
        3,
        "the screen shows the game's desk"
    );
    assert_eq!(manager.workspace(), 3);
    assert_eq!(manager.focus(), Some(game));
}
