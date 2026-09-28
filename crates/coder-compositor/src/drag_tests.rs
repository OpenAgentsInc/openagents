//! What a Super drag does to the window it holds.

use super::*;

/// A window 200 by 200 pixels at 100, 100.
fn window() -> Placed {
    Placed {
        x: 100,
        y: 100,
        width: 200,
        height: 200,
    }
}

/// A screen of 800 by 600 pixels, with the drag's floor.
fn bounds() -> Bounds {
    Bounds {
        floor: FLOOR,
        width: 800,
        height: 600,
    }
}

#[test]
fn the_table_gives_the_left_button_a_move_and_the_right_one_a_resize() {
    assert_eq!(bound(coder_binds::Mods::SUPER, 272), Some(Kind::Move));
    assert_eq!(bound(coder_binds::Mods::SUPER, 273), Some(Kind::Resize));
}

#[test]
fn a_drag_with_no_modifier_belongs_to_the_client() {
    assert_eq!(bound(coder_binds::Mods::default(), 272), None);
    assert_eq!(bound(coder_binds::Mods::SUPER_SHIFT, 272), None);
    assert_eq!(bound(coder_binds::Mods::SUPER, 274), None, "the wheel");
}

#[test]
fn a_move_carries_the_window_by_what_the_pointer_moved() {
    let moved = moved(window(), 37.4, -12.6);
    assert_eq!(moved.x, 137);
    assert_eq!(moved.y, 87);
    assert_eq!((moved.width, moved.height), (200, 200));
}

#[test]
fn a_resize_from_the_right_edge_leaves_the_left_edge_where_it_was() {
    let sized = resized(window(), 60.0, 40.0, false, false, None, bounds());
    assert_eq!((sized.x, sized.y), (100, 100));
    assert_eq!((sized.width, sized.height), (260, 240));
}

#[test]
fn a_resize_from_the_left_edge_leaves_the_right_edge_where_it_was() {
    let sized = resized(window(), -60.0, -40.0, true, true, None, bounds());
    assert_eq!((sized.x, sized.y), (40, 60));
    assert_eq!((sized.width, sized.height), (260, 240));
    assert_eq!(sized.x + sized.width, 300, "the right edge holds");
    assert_eq!(sized.y + sized.height, 300, "the bottom edge holds");
}

#[test]
fn a_circle_stays_a_circle_as_it_grows() {
    let sized = resized(window(), 80.0, 0.0, false, false, Some(1.0), bounds());
    assert_eq!(sized.width, sized.height);
    assert_eq!(sized.width, 280);
}

#[test]
fn a_circle_stays_a_circle_as_it_shrinks() {
    let sized = resized(window(), -50.0, 0.0, false, false, Some(1.0), bounds());
    assert_eq!(sized.width, sized.height);
    assert_eq!(sized.width, 150);
}

#[test]
fn a_ratio_that_is_not_one_holds_too() {
    let wide = Placed {
        x: 0,
        y: 0,
        width: 400,
        height: 200,
    };
    let sized = resized(wide, 0.0, 50.0, false, false, Some(2.0), bounds());
    assert_eq!((sized.width, sized.height), (500, 250));
}

#[test]
fn a_resize_stops_at_the_floor_rather_than_turning_inside_out() {
    // The left edge is dragged a long way past the right one.
    let sized = resized(window(), 1000.0, 1000.0, true, true, None, bounds());
    assert_eq!((sized.width, sized.height), (FLOOR, FLOOR));
    assert_eq!(sized.x + sized.width, 300, "the right edge holds");
    assert_eq!(sized.y + sized.height, 300, "the bottom edge holds");
    assert!(sized.width > 0 && sized.height > 0);
}

#[test]
fn a_circle_stops_at_the_floor_and_is_still_a_circle() {
    let sized = resized(window(), -1000.0, 0.0, false, false, Some(1.0), bounds());
    assert_eq!((sized.width, sized.height), (FLOOR, FLOOR));
}

#[test]
fn a_resize_stops_at_the_screen() {
    let sized = resized(window(), 4000.0, 4000.0, false, false, None, bounds());
    assert_eq!((sized.width, sized.height), (800, 600));
}

#[test]
fn a_circle_stops_at_the_shorter_side_of_the_screen() {
    let sized = resized(window(), 4000.0, 0.0, false, false, Some(1.0), bounds());
    assert_eq!(
        (sized.width, sized.height),
        (600, 600),
        "the screen is 600 tall, and the ratio holds at the bound"
    );
}

#[test]
fn the_side_the_pointer_moved_further_along_drives_a_ratio() {
    let tall = resized(window(), 0.0, 90.0, false, false, Some(1.0), bounds());
    assert_eq!((tall.width, tall.height), (290, 290));
    let wide = resized(window(), 90.0, 0.0, false, false, Some(1.0), bounds());
    assert_eq!((wide.width, wide.height), (290, 290));
}
