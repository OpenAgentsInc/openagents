//! Frame components for HUD UI.
//!
//! Frames provide decorative borders around content with animated
//! entry/exit effects.

mod circle;
mod corners;
mod header;
mod lines;
mod octagon;
mod underline;

pub use circle::FrameCircle;
pub use corners::FrameCorners;
pub use header::FrameHeader;
pub use lines::{FrameLines, FrameSides};
pub use octagon::FrameOctagon;
pub use underline::FrameUnderline;
