//! The order the windows a screen shows draw in.
//!
//! The space draws its elements back to front in the order it holds them,
//! and mapping or raising an element moves it to the front. Three paths
//! move one there: `arrange` maps every window a screen shows, the focus
//! raises the focused window, and the desk protocol's `raise` verb raises
//! the window it names. A tile raised over a float hides the float, which
//! is how the camera circle went under the pane the pointer moved onto.
//! So after
//! each of those paths the space is put back in one order: the tiles, then
//! the floats in the order the layout raises them, then the pinned floats
//! over everything, so a pinned float stays visible whatever has the focus.

use coder_wm::WinId;

/// Where a window sits in the stack, back to front.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Layer {
    /// A tile. Tiles overlap nothing, so their order among themselves does
    /// not show.
    Tiled,
    /// A float, drawn over the tiles in the order the layout raises the
    /// floats of its desk.
    Floating,
    /// A pinned float, drawn over every other window.
    Pinned,
}

impl Layer {
    /// The layer one window draws on. A pinned window floats, so the pin
    /// decides first.
    pub fn of(floating: bool, pinned: bool) -> Layer {
        match (floating, pinned) {
            (_, true) => Layer::Pinned,
            (true, false) => Layer::Floating,
            (false, false) => Layer::Tiled,
        }
    }

    /// What the log calls the layer, or nothing for a tile.
    pub fn name(self) -> Option<&'static str> {
        match self {
            Layer::Tiled => None,
            Layer::Floating => Some("float"),
            Layer::Pinned => Some("pinned"),
        }
    }
}

/// The windows back to front: by layer, and within a layer in the order
/// they were given, which is the order the layout holds them in.
pub fn order(windows: &[(Layer, WinId)]) -> Vec<WinId> {
    let mut sorted: Vec<(Layer, WinId)> = windows.to_vec();
    sorted.sort_by_key(|(layer, _)| *layer);
    sorted.into_iter().map(|(_, id)| id).collect()
}

/// Whether raising one window changes the order: it floats, and another
/// float of its layer draws over it. A raise of the float already at the
/// front of its layer, or of a tile, moves nothing, so the desk answers it
/// without a restack and without a log line; the strip under the camera
/// asks for one on every pass.
pub fn raise_moves(windows: &[(Layer, WinId)], id: WinId) -> bool {
    let Some((layer, _)) = windows.iter().find(|(_, held)| *held == id) else {
        return false;
    };
    if *layer == Layer::Tiled {
        return false;
    }
    windows
        .iter()
        .rfind(|(held, _)| held == layer)
        .is_some_and(|(_, front)| *front != id)
}

#[cfg(test)]
mod tests {
    use super::*;

    const A: WinId = WinId(1);
    const B: WinId = WinId(2);
    const C: WinId = WinId(3);
    const D: WinId = WinId(4);

    #[test]
    fn a_pinned_float_draws_over_a_float_which_draws_over_the_tiles() {
        let windows = [
            (Layer::Floating, A),
            (Layer::Pinned, B),
            (Layer::Tiled, C),
            (Layer::Tiled, D),
        ];
        assert_eq!(order(&windows), vec![C, D, A, B]);
    }

    #[test]
    fn a_tile_that_took_the_focus_stays_under_the_pinned_camera() {
        // The focused tile is raised to the front of the space before the
        // stack is put back; the order puts the pinned float over it again.
        let windows = [(Layer::Pinned, A), (Layer::Tiled, B)];
        assert_eq!(order(&windows), vec![B, A]);
        let windows = [(Layer::Tiled, B), (Layer::Pinned, A)];
        assert_eq!(order(&windows), vec![B, A]);
    }

    #[test]
    fn floats_keep_the_order_the_layout_raises_them_in() {
        let windows = [
            (Layer::Floating, A),
            (Layer::Floating, B),
            (Layer::Tiled, C),
            (Layer::Floating, D),
        ];
        assert_eq!(order(&windows), vec![C, A, B, D]);
        let windows = [(Layer::Pinned, D), (Layer::Pinned, A), (Layer::Floating, B)];
        assert_eq!(order(&windows), vec![B, D, A]);
    }

    #[test]
    fn a_raise_of_the_float_already_in_front_moves_nothing() {
        // The camera circle and the strip under it, both pinned, with the
        // strip in front: the strip's `raise` on every pass is a no-op.
        let windows = [(Layer::Tiled, C), (Layer::Pinned, A), (Layer::Pinned, B)];
        assert!(!raise_moves(&windows, B));
        assert!(
            raise_moves(&windows, A),
            "the circle under the strip can be raised"
        );
        assert!(!raise_moves(&windows, C), "a tile overlaps nothing");
        assert!(
            !raise_moves(&windows, D),
            "a window the screens do not show"
        );
        // A float under a pinned float is the front of its own layer.
        let windows = [(Layer::Floating, C), (Layer::Pinned, A)];
        assert!(!raise_moves(&windows, C));
    }

    #[test]
    fn the_pin_decides_the_layer_before_the_float_does() {
        assert_eq!(Layer::of(false, false), Layer::Tiled);
        assert_eq!(Layer::of(true, false), Layer::Floating);
        assert_eq!(Layer::of(true, true), Layer::Pinned);
        assert_eq!(Layer::of(false, true), Layer::Pinned);
        assert_eq!(Layer::Tiled.name(), None);
        assert_eq!(Layer::Floating.name(), Some("float"));
        assert_eq!(Layer::Pinned.name(), Some("pinned"));
    }
}
