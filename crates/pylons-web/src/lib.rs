//! `/pylons`' Grid layer.
//!
//! - [`camera`] projects the still Grid's floor onto the screen and picks
//!   the spots the pylons stand on.
//! - [`world_camera`] roots pylons to fixed floor points when Verse takes
//!   over, using the camera matrix from each rendered frame. The points are
//!   the Pylon Bay's (`pylons_view::bay`, #12423).
//! - The layer (wasm32 only) opens the page's event stream
//!   (`/pylons/live`), keeps the HTML list and the announcements current,
//!   and opens a pylon's card on a click or tap through the page's
//!   `pylonsCard`. Where Verse draws the Bay's machines (the Grid homepage,
//!   whose canvas carries `data-pylon-bay`), the layer hands Verse the Bay
//!   as `globalThis.pylonsBay` and only labels the machines: a name from
//!   afar, everything up close, under the pointer, or with the card open,
//!   and never one label over another. Elsewhere it stands a glowing pillar
//!   per online pylon (rising as its beacon arrives, fading when it goes
//!   offline or its beacon expires, pulsing when a receipt for one of its
//!   jobs arrives).
//!
//! - The layer belongs to the Grid plaza. When Verse leaves it for a zone
//!   (its `verse-zone` event and `data-verse-place` on the page's root,
//!   #12438), the labels, pillars, walking hint, and card hide, and come
//!   back on return; [`shown_in`] decides.
//!
//! The site (`openagents-web`, `src/pages/pylons.rs`) serves the build
//! (`scripts/build-pylons-web.sh`) and loads it after the Grid's first
//! frame.

pub mod camera;
pub use pylons_view::terminal;
pub mod world_camera;

#[cfg(target_arch = "wasm32")]
mod terminal_web;

#[cfg(target_arch = "wasm32")]
mod web;

/// Whether the layer shows while Verse is at `place`, the page root's
/// `data-verse-place`: on the Grid plaza (`grid`), or before Verse says
/// where it is (empty), and never over a zone (`zone`) or its load screen
/// (`loading`).
#[must_use]
pub fn shown_in(place: &str) -> bool {
    matches!(place, "" | "grid")
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_layer_shows_on_the_grid_only() {
        assert!(super::shown_in("grid"));
        assert!(super::shown_in(""));
        assert!(!super::shown_in("zone"));
        assert!(!super::shown_in("loading"));
    }
}
