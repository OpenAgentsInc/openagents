//! The seam where hand tracking joins the desk, with tracking absent.
//!
//! In the private Coder repository this module reads the camera daemon's
//! landmarks, turns them into pointer motion, presses, desk switches, and
//! Escape through `coder_hands::gestures`, and draws the hand over every
//! window through a `hands_overlay` module. That code needs the
//! `coder-hands` crate, which moves with the camera daemon in #9874. Until
//! then this file keeps the interface the rest of the compositor calls, and
//! tracking is never on:
//!
//! - [`Hands`] is what the compositor state holds, and [`Hands::is_on`] is
//!   what the desk protocol's `hands` status reads.
//! - [`Picture`] is one raster the renderer draws over the windows, and
//!   [`Hands::pictures`] is always empty.
//! - [`Coder::toggle_hands`] answers the Super+H row, and
//!   [`Coder::hands_pass`] is what each loop calls when the reader has
//!   frames.
//!
//! #9874 replaces this file with the reader, brings `hands_overlay.rs`
//! back, and adds `coder-hands` to `Cargo.toml`. Nothing outside this file
//! has to change, because the rest of the compositor already calls these
//! names.

use std::sync::Arc;

use crate::state::Coder;

/// The launcher option that grants hands: the `option` of the Super+H row
/// in `crates/coder-binds`, and the name the desktop module writes into the
/// grant when hands are on.
pub const OPTION: &str = "hands";

/// One raster the renderer draws at a place in the shared space:
/// premultiplied `ARGB8888` in memory order, `B`, `G`, `R`, `A`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Picture {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub pixels: Vec<u8>,
}

/// Hand tracking in a build that has none: always off, drawing nothing.
pub struct Hands {
    pictures: Arc<Vec<Picture>>,
}

impl Hands {
    /// Tracking off. The build with `coder-hands` reads the camera daemon's
    /// sockets from the environment here.
    pub fn from_environment() -> Hands {
        Hands {
            pictures: Arc::new(Vec::new()),
        }
    }

    /// Whether hands drive the desk. Never, in this build.
    pub fn is_on(&self) -> bool {
        false
    }

    /// What the overlay draws, which is nothing.
    pub fn pictures(&self) -> &Arc<Vec<Picture>> {
        &self.pictures
    }

    /// What the reader's thread calls when a frame arrives. This build
    /// starts no reader, so nothing calls it.
    pub fn set_wake(&mut self, _wake: Arc<dyn Fn() + Send + Sync>) {}

    /// Starts the reader. This build has none, so it logs that instead.
    pub fn turn_on(&mut self) {
        log::warn!(
            "hand tracking is not in this build of the compositor; it arrives with the \
             camera daemon"
        );
    }
}

impl Coder {
    /// The Super+H chord. With no reader in this build, it logs that and
    /// changes nothing.
    pub fn toggle_hands(&mut self) {
        self.hands.turn_on();
    }

    /// Reads what the hands thread sent since the last pass. This build has
    /// no thread, so there is nothing to read.
    pub fn hands_pass(&mut self) {}
}
