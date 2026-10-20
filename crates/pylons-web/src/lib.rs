//! `/pylons`' Grid layer.
//!
//! - [`camera`] projects the still Grid's floor onto the screen and picks
//!   the spots the pylons stand on.
//! - The layer (wasm32 only) opens the page's event stream
//!   (`/pylons/live`), keeps the HTML list and the announcements current,
//!   stands a glowing pillar per online pylon on the Grid (rising as its
//!   beacon arrives, fading when it goes offline or its beacon expires,
//!   pulsing when a receipt for one of its jobs arrives), and opens the
//!   pylon's card on a click or tap through the page's `pylonsCard`.
//!
//! The site (`openagents-web`, `src/pages/pylons.rs`) serves the build
//! (`scripts/build-pylons-web.sh`) and loads it after the Grid's first
//! frame.

pub mod camera;

#[cfg(target_arch = "wasm32")]
mod web;
