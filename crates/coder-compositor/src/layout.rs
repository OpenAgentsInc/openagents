//! Pixels from the layout crate's normalized rectangles.
//!
//! `coder_wm` returns each tile as a rectangle in 0..1 of the desk and
//! knows nothing about a screen. This module turns one of those into the
//! pixels a window is configured to, with the gaps and the border
//! `os/modules/coderos/desktop.nix` sets: 6 pixels around the screen, 3
//! between tiles, and a one-pixel border drawn outside the window.
//!
//! The layout tiles inside a usable area rather than the whole screen. A
//! layer surface that holds an exclusive zone, such as a panel anchored to
//! the top edge, takes its pixels out of that area, so the tiles move down
//! instead of drawing under it.

use coder_wm::Rect;

/// The pixels between a tile and its neighbour, Hyprland's `gaps_in`.
pub const GAP_INNER: i32 = 3;

/// The pixels between the layout and the screen edge, Hyprland's
/// `gaps_out`.
pub const GAP_OUTER: i32 = 6;

/// The border's thickness in pixels, Hyprland's `border_size`.
pub const BORDER: i32 = 1;

/// The amber a focused window's border draws in, as
/// `crates/coder-ui-core/palette.rs` holds it and `desktop.nix` copies it.
pub const BORDER_ACTIVE: [f32; 4] = [1.0, 0.690, 0.0, 1.0];

/// The dim amber every other window's border draws in.
pub const BORDER_IDLE: [f32; 4] = [0.275, 0.192, 0.0, 1.0];

/// How tall the bar a notice draws across the top of the screen is, in
/// pixels.
pub const NOTICE_BAR: i32 = 4;

/// The near-black the desk draws behind the tiles.
pub const BACKGROUND: [f32; 4] = [0.031, 0.024, 0.0, 1.0];

/// A screen's size in pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Screen {
    /// The screen's width in pixels.
    pub width: i32,
    /// The screen's height in pixels.
    pub height: i32,
}

/// Where one window sits on a screen, in pixels from the screen's top left
/// corner.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Placed {
    /// The left edge.
    pub x: i32,
    /// The top edge.
    pub y: i32,
    /// The width.
    pub width: i32,
    /// The height.
    pub height: i32,
}

/// How much of the screen a tile takes: the whole of it for a window that
/// fills the screen, and the area inside the gaps for everything else.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fill {
    /// The layout's area, inside the gaps.
    Tiled,
    /// The whole screen, with no gap and no border. Hyprland's
    /// `fullscreen 0`.
    Screen,
}

/// The whole screen as a rectangle, which is the area the layout tiles in
/// when no layer surface holds an exclusive zone.
pub fn whole(screen: Screen) -> Placed {
    Placed {
        x: 0,
        y: 0,
        width: screen.width.max(1),
        height: screen.height.max(1),
    }
}

/// The pixels one normalized rectangle takes inside a usable area.
///
/// A tile keeps the outer gap at every edge it shares with the area and the
/// inner gap at every edge it shares with another tile, which is what
/// `gaps_in` and `gaps_out` do on the Hyprland session. The area is the
/// screen less every exclusive zone a layer surface holds, so a panel or a
/// notification bar takes its pixels out of the layout rather than off the
/// window under it.
pub fn place(rect: Rect, screen: Screen, usable: Placed, fill: Fill) -> Placed {
    if fill == Fill::Screen {
        return whole(screen);
    }
    let area_x = usable.x + GAP_OUTER;
    let area_y = usable.y + GAP_OUTER;
    let area_w = (usable.width - 2 * GAP_OUTER).max(1);
    let area_h = (usable.height - 2 * GAP_OUTER).max(1);
    let left = area_x + (rect.x * area_w as f32).round() as i32;
    let top = area_y + (rect.y * area_h as f32).round() as i32;
    let right = area_x + ((rect.x + rect.w) * area_w as f32).round() as i32;
    let bottom = area_y + ((rect.y + rect.h) * area_h as f32).round() as i32;
    let inset_left = if touches(rect.x, 0.0) { 0 } else { GAP_INNER };
    let inset_top = if touches(rect.y, 0.0) { 0 } else { GAP_INNER };
    let inset_right = if touches(rect.x + rect.w, 1.0) {
        0
    } else {
        GAP_INNER
    };
    let inset_bottom = if touches(rect.y + rect.h, 1.0) {
        0
    } else {
        GAP_INNER
    };
    Placed {
        x: left + inset_left,
        y: top + inset_top,
        width: (right - inset_right - left - inset_left).max(1),
        height: (bottom - inset_bottom - top - inset_top).max(1),
    }
}

/// The normalized rectangle that [`place`] turns back into `target`, for a
/// float asked to sit at exact pixels through the desk protocol's `shape`.
///
/// `place` rounds each edge on its own and moves an edge off the area's
/// edge in by the inner gap, so a rectangle reached through the layout
/// crate's drag by a pixel delta lands a pixel or a gap from what was
/// asked, and a move applied before a resize is clamped against the size
/// the window had. Each edge here takes the fraction whose rounding, with
/// the inset `place` adds to it, is the pixel asked for. An edge within
/// the inner gap of the area's edge has no exact fraction and sits on the
/// area's edge; a rectangle past the area is held to it.
pub fn normalize(target: Placed, usable: Placed) -> Rect {
    let area_x = usable.x + GAP_OUTER;
    let area_y = usable.y + GAP_OUTER;
    let area_w = (usable.width - 2 * GAP_OUTER).max(1);
    let area_h = (usable.height - 2 * GAP_OUTER).max(1);
    let left = start_edge(target.x - area_x, area_w);
    let top = start_edge(target.y - area_y, area_h);
    let right = end_edge(target.x + target.width - area_x, area_w);
    let bottom = end_edge(target.y + target.height - area_y, area_h);
    Rect {
        x: left,
        y: top,
        w: (right - left).max(0.0),
        h: (bottom - top).max(0.0),
    }
}

/// The fraction of a span a window's near edge sits at, for the pixel the
/// edge is asked to sit on counted from the area's start. An edge on the
/// area's edge, or within the rounding [`touches`] allows of it, carries
/// no inset; every other edge carries the inner gap, which the fraction
/// leaves room for.
fn start_edge(pixel: i32, span: i32) -> f32 {
    if pixel <= 0 {
        return 0.0;
    }
    let span = span as f32;
    let on_edge = pixel as f32 / span;
    if touches(on_edge, 0.0) {
        return on_edge;
    }
    let inset = (pixel - GAP_INNER) as f32 / span;
    if inset > 0.0 && !touches(inset, 0.0) {
        return inset.min(1.0);
    }
    inset.max(0.0)
}

/// The fraction of a span a window's far edge ends at, for the pixel past
/// the edge counted from the area's start: [`start_edge`] mirrored against
/// the area's end.
fn end_edge(pixel: i32, span: i32) -> f32 {
    if pixel >= span {
        return 1.0;
    }
    let span = span as f32;
    let on_edge = pixel as f32 / span;
    if touches(on_edge, 1.0) {
        return on_edge;
    }
    let inset = (pixel + GAP_INNER) as f32 / span;
    if inset < 1.0 && !touches(inset, 1.0) {
        return inset.max(0.0);
    }
    inset.min(1.0)
}

/// The four one-pixel bars that draw a window's border, outside the window
/// itself, clockwise from the top.
pub fn border_bars(placed: Placed) -> [Placed; 4] {
    let outer_w = placed.width + 2 * BORDER;
    [
        Placed {
            x: placed.x - BORDER,
            y: placed.y - BORDER,
            width: outer_w,
            height: BORDER,
        },
        Placed {
            x: placed.x + placed.width,
            y: placed.y,
            width: BORDER,
            height: placed.height,
        },
        Placed {
            x: placed.x - BORDER,
            y: placed.y + placed.height,
            width: outer_w,
            height: BORDER,
        },
        Placed {
            x: placed.x - BORDER,
            y: placed.y,
            width: BORDER,
            height: placed.height,
        },
    ]
}

/// The usable area one exclusive zone leaves, clamped to the screen.
///
/// `coder_wm` lays out in 0..1 of an area, so an area outside the screen or
/// one with no pixels in it would put every tile off the screen. A zone
/// that leaves nothing falls back to the whole screen, which keeps the
/// windows reachable when a layer surface asks for everything.
pub fn usable(screen: Screen, zone: Placed) -> Placed {
    let left = zone.x.clamp(0, screen.width.max(1));
    let top = zone.y.clamp(0, screen.height.max(1));
    let right = (zone.x + zone.width).clamp(left, screen.width.max(1));
    let bottom = (zone.y + zone.height).clamp(top, screen.height.max(1));
    let width = right - left;
    let height = bottom - top;
    if width <= 2 * GAP_OUTER || height <= 2 * GAP_OUTER {
        return whole(screen);
    }
    Placed {
        x: left,
        y: top,
        width,
        height,
    }
}

/// The fraction of a screen one resize step moves an edge by, which is what
/// the layout crate resizes in.
pub fn resize_fraction(step: i32, screen: Screen, horizontal: bool) -> f32 {
    let span = if horizontal {
        screen.width
    } else {
        screen.height
    };
    step as f32 / span.max(1) as f32
}

/// A normalized coordinate sits on an edge of the desk, within the rounding
/// the layout crate's ratios leave.
fn touches(value: f32, edge: f32) -> bool {
    (value - edge).abs() < 1e-3
}

/// Whether a point in logical pixels falls inside a window's tile, which
/// is where the window draws and takes the pointer. The right and bottom
/// edges belong to the next tile.
pub fn contains(placed: Placed, x: f64, y: f64) -> bool {
    x >= f64::from(placed.x)
        && x < f64::from(placed.x + placed.width)
        && y >= f64::from(placed.y)
        && y < f64::from(placed.y + placed.height)
}

#[cfg(test)]
mod tests {
    use super::*;
    use coder_wm::Manager;

    const SCREEN: Screen = Screen {
        width: 1280,
        height: 800,
    };

    fn placed_tiles(manager: &Manager) -> Vec<Placed> {
        manager
            .tiles()
            .into_iter()
            .map(|tile| place(tile.rect, SCREEN, whole(SCREEN), Fill::Tiled))
            .collect()
    }

    fn overlap(a: Placed, b: Placed) -> bool {
        a.x < b.x + b.width && b.x < a.x + a.width && a.y < b.y + b.height && b.y < a.y + a.height
    }

    #[test]
    fn one_tile_fills_the_screen_inside_the_outer_gap() {
        let mut manager = Manager::new();
        manager.spawn();
        let tiles = placed_tiles(&manager);
        assert_eq!(tiles.len(), 1);
        assert_eq!(
            tiles[0],
            Placed {
                x: GAP_OUTER,
                y: GAP_OUTER,
                width: SCREEN.width - 2 * GAP_OUTER,
                height: SCREEN.height - 2 * GAP_OUTER,
            }
        );
    }

    #[test]
    fn two_tiles_split_the_screen_with_the_inner_gap_between_them() {
        let mut manager = Manager::new();
        manager.spawn();
        manager.spawn();
        let tiles = placed_tiles(&manager);
        assert_eq!(tiles.len(), 2);
        let left = tiles[0];
        let right = tiles[1];
        assert_eq!(left.x, GAP_OUTER);
        assert_eq!(right.x + right.width, SCREEN.width - GAP_OUTER);
        assert_eq!(right.x - (left.x + left.width), 2 * GAP_INNER);
        assert_eq!(left.height, SCREEN.height - 2 * GAP_OUTER);
    }

    #[test]
    fn a_closed_tile_gives_its_pixels_back() {
        let mut manager = Manager::new();
        let first = manager.spawn();
        manager.spawn();
        manager.spawn();
        let three = placed_tiles(&manager);
        assert_eq!(three.len(), 3);
        for (index, tile) in three.iter().enumerate() {
            for other in three.iter().skip(index + 1) {
                assert!(!overlap(*tile, *other), "{tile:?} and {other:?} overlap");
            }
            assert!(tile.x >= GAP_OUTER, "{tile:?} left the screen");
            assert!(
                tile.x + tile.width <= SCREEN.width - GAP_OUTER,
                "{tile:?} left the screen"
            );
        }
        manager.close(first);
        let two = placed_tiles(&manager);
        assert_eq!(two.len(), 2);
        let widest = two.iter().map(|tile| tile.width).max().unwrap_or(0);
        let was = three.iter().map(|tile| tile.width).max().unwrap_or(0);
        assert!(widest > was, "the layout kept the closed tile's pixels");
    }

    #[test]
    fn a_window_that_fills_the_screen_keeps_no_gap() {
        let filled = place(
            coder_wm::Rect {
                x: 0.0,
                y: 0.0,
                w: 1.0,
                h: 1.0,
            },
            SCREEN,
            whole(SCREEN),
            Fill::Screen,
        );
        assert_eq!(
            filled,
            Placed {
                x: 0,
                y: 0,
                width: SCREEN.width,
                height: SCREEN.height,
            }
        );
    }

    #[test]
    fn a_point_past_a_tiles_edge_belongs_to_its_neighbour() {
        let mut manager = Manager::new();
        manager.spawn();
        manager.spawn();
        let tiles = placed_tiles(&manager);
        let (left, right) = (tiles[0], tiles[1]);
        let y = f64::from(left.y + left.height / 2);
        let inside_right = f64::from(right.x + 1);
        assert!(!contains(left, inside_right, y));
        assert!(contains(right, inside_right, y));
        assert!(contains(left, f64::from(left.x), y));
        assert!(!contains(left, f64::from(left.x + left.width), y));
    }

    #[test]
    fn the_border_draws_four_bars_outside_the_window() {
        let placed = Placed {
            x: 10,
            y: 20,
            width: 100,
            height: 50,
        };
        let bars = border_bars(placed);
        assert_eq!(bars[0].y, placed.y - BORDER);
        assert_eq!(bars[0].height, BORDER);
        assert_eq!(bars[1].x, placed.x + placed.width);
        assert_eq!(bars[2].y, placed.y + placed.height);
        assert_eq!(bars[3].x, placed.x - BORDER);
        for bar in bars {
            assert!(bar.width > 0 && bar.height > 0, "{bar:?} draws nothing");
        }
    }

    #[test]
    fn an_exclusive_zone_at_the_top_pushes_every_tile_below_it() {
        // A notification bar 30 pixels tall anchored to the top edge leaves
        // the rest of the screen, which is what `mako` asks for.
        let zone = Placed {
            x: 0,
            y: 30,
            width: SCREEN.width,
            height: SCREEN.height - 30,
        };
        let area = usable(SCREEN, zone);
        assert_eq!(area, zone);
        let mut manager = Manager::new();
        manager.spawn();
        manager.spawn();
        for tile in manager.tiles() {
            let placed = place(tile.rect, SCREEN, area, Fill::Tiled);
            assert!(placed.y >= 30 + GAP_OUTER, "{placed:?} drew under the bar");
            assert!(
                placed.y + placed.height <= SCREEN.height - GAP_OUTER,
                "{placed:?} left the screen"
            );
        }
    }

    #[test]
    fn a_window_that_fills_the_screen_ignores_an_exclusive_zone() {
        // Hyprland's `fullscreen 0` covers a panel, and so does this.
        let zone = Placed {
            x: 0,
            y: 30,
            width: SCREEN.width,
            height: SCREEN.height - 30,
        };
        let filled = place(
            coder_wm::Rect {
                x: 0.0,
                y: 0.0,
                w: 1.0,
                h: 1.0,
            },
            SCREEN,
            usable(SCREEN, zone),
            Fill::Screen,
        );
        assert_eq!(filled, whole(SCREEN));
    }

    #[test]
    fn a_zone_that_leaves_no_room_falls_back_to_the_whole_screen() {
        let nothing = Placed {
            x: 0,
            y: 0,
            width: SCREEN.width,
            height: 2,
        };
        assert_eq!(usable(SCREEN, nothing), whole(SCREEN));
        let outside = Placed {
            x: -400,
            y: -400,
            width: 100,
            height: 100,
        };
        assert_eq!(usable(SCREEN, outside), whole(SCREEN));
    }

    #[test]
    fn a_zone_is_clamped_to_the_screen_it_sits_on() {
        let wider = Placed {
            x: -20,
            y: 10,
            width: SCREEN.width + 200,
            height: SCREEN.height,
        };
        let area = usable(SCREEN, wider);
        assert_eq!(area.x, 0);
        assert_eq!(area.y, 10);
        assert_eq!(area.width, SCREEN.width);
        assert_eq!(area.height, SCREEN.height - 10);
    }

    #[test]
    fn a_resize_step_is_the_same_forty_pixels_the_session_resizes_by() {
        let across = resize_fraction(40, SCREEN, true);
        assert!((across - 40.0 / 1280.0).abs() < 1e-6, "{across}");
        let down = resize_fraction(40, SCREEN, false);
        assert!((down - 40.0 / 800.0).abs() < 1e-6, "{down}");
    }

    /// The pixels `place` answers for a float `normalize` put at `asked`.
    fn round_trip(asked: Placed, screen: Screen, area: Placed) -> Placed {
        place(normalize(asked, area), screen, area, Fill::Tiled)
    }

    #[test]
    fn the_camera_circle_reads_back_where_presentation_mode_put_it() {
        // The rectangles from the parity host: the corner `off` asked
        // for on a 2560x1440 screen at scale 1, and the corner `on` chose
        // on the 2048x1152 logical screen that 1.25 makes of it.
        let qhd = Screen {
            width: 2560,
            height: 1440,
        };
        let asked = Placed {
            x: 1857,
            y: 142,
            width: 561,
            height: 561,
        };
        assert_eq!(round_trip(asked, qhd, whole(qhd)), asked);
        let scaled = Screen {
            width: 2048,
            height: 1152,
        };
        let corner = Placed {
            x: 1348,
            y: 140,
            width: 560,
            height: 560,
        };
        assert_eq!(round_trip(corner, scaled, whole(scaled)), corner);
    }

    #[test]
    fn every_float_clear_of_the_area_edges_reads_back_as_asked() {
        let area = whole(SCREEN);
        let mut checked = 0;
        for x in (11..SCREEN.width).step_by(37) {
            for y in (11..SCREEN.height).step_by(29) {
                for width in [1, 20, 104, 561] {
                    for height in [1, 20, 104, 561] {
                        if x + width > SCREEN.width - GAP_OUTER - 5
                            || y + height > SCREEN.height - GAP_OUTER - 5
                        {
                            continue;
                        }
                        let asked = Placed {
                            x,
                            y,
                            width,
                            height,
                        };
                        assert_eq!(round_trip(asked, SCREEN, area), asked);
                        checked += 1;
                    }
                }
            }
        }
        assert!(checked > 1000, "{checked} rectangles were checked");
    }

    #[test]
    fn a_float_under_a_panel_reads_back_against_the_area_the_panel_leaves() {
        let zone = Placed {
            x: 0,
            y: 40,
            width: SCREEN.width,
            height: SCREEN.height - 40,
        };
        let area = usable(SCREEN, zone);
        let asked = Placed {
            x: 300,
            y: 200,
            width: 200,
            height: 150,
        };
        assert_eq!(round_trip(asked, SCREEN, area), asked);
    }

    #[test]
    fn a_float_on_the_area_edge_sits_on_it() {
        let area = whole(SCREEN);
        let asked = Placed {
            x: GAP_OUTER,
            y: GAP_OUTER,
            width: 100,
            height: 100,
        };
        assert_eq!(round_trip(asked, SCREEN, area), asked);
    }

    #[test]
    fn a_float_asked_past_the_area_is_held_to_it() {
        let area = whole(SCREEN);
        let asked = Placed {
            x: 1200,
            y: 700,
            width: 300,
            height: 300,
        };
        let placed = round_trip(asked, SCREEN, area);
        assert_eq!((placed.x, placed.y), (1200, 700));
        assert_eq!(placed.x + placed.width, SCREEN.width - GAP_OUTER);
        assert_eq!(placed.y + placed.height, SCREEN.height - GAP_OUTER);
    }
}
