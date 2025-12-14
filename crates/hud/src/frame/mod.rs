//! Frame components for HUD UI.
//!
//! Frames provide decorative borders around content with animated
//! entry/exit effects.

mod corners;
mod lines;

pub use corners::FrameCorners;
pub use lines::{FrameLines, FrameSides};
