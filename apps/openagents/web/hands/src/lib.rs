//! Hand input for openagents.com/lander9.
//!
//! The page runs a hand landmarker in the browser and hands each frame's
//! landmarks here, packed as floats ([`track::RawFrame::decode`]). This
//! crate does the rest in Rust: [`filter`] smooths every coordinate with a
//! One Euro filter, [`track`] reads each hand's pose with `coder_hands`'
//! rules (pinch with its measured hysteresis, fist, open palm, point) and
//! turns the first hand into pointer input, a second hand's pinch into a
//! spread, and an open palm held for a second into letting go. [`synth`]
//! makes landmarks for a pose at a place, for the tests and for the page's
//! test hook when there is no camera.
//!
//! Nothing here reads a camera or a clock: callers pass the time.

pub mod filter;
pub mod synth;
pub mod track;

pub use coder_hands::{CONNECTIONS, INDEX_TIP, Landmark, THUMB_TIP};
pub use track::{Gesture, HandView, Hands, Input, Mapping, P, RawFrame, RawHand};

/// The fingertips, which the page draws brightest.
pub const TIPS: [usize; 5] = [4, 8, 12, 16, 20];
