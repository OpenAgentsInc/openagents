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
