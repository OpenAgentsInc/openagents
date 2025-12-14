//! HUD - Arwes-style sci-fi HUD components for WGPUI.
//!
//! This crate provides GPU-accelerated sci-fi UI components inspired by
//! the Arwes framework, adapted for the WGPUI rendering system.
//!
//! # Components
//!
//! ## Animation
//! - [`animator::HudAnimator`] - Core animation state machine
//! - [`animator::AnimatorManager`] - Orchestrates child animations
//!
//! ## Frames
//! - [`frame::FrameCorners`] - Bracket-style corner frames
//! - [`frame::FrameLines`] - Edge line frames with gaps
//! - [`frame::FrameOctagon`] - 8-sided frame with clipped corners
//! - [`frame::FrameCircle`] - Circular frame border
//! - [`frame::FrameHeader`] - Header section with accents
//! - [`frame::FrameUnderline`] - Simple bottom line
//!
//! ## Backgrounds
//! - [`background::DotGridBackground`] - Animated dot grid
//! - [`background::GridLinesBackground`] - Grid line pattern
//! - [`background::MovingLinesBackground`] - Moving lines effect
//!
//! ## Text Animation
//! - [`text::TextSequence`] - Character-by-character reveal
//! - [`text::TextDecipher`] - Scramble/decipher effect
//!
//! ## Effects
//! - [`effects::Illuminator`] - Mouse-following glow
//!
//! ## Interactive
//! - [`button::HudButton`] - Animated button with frame
//!
//! # Theme
//!
//! All components use a white-on-black color scheme with varying opacities.
//! See [`theme`] for color constants.

pub mod animator;
pub mod background;
pub mod button;
pub mod easing;
pub mod effects;
pub mod frame;
pub mod text;
pub mod theme;

// Re-export commonly used types
pub use animator::{AnimatorManager, AnimatorState, HudAnimator, ManagerMode};
pub use background::{DotGridBackground, GridLinesBackground, LineDirection, MovingLinesBackground};
pub use button::HudButton;
pub use effects::Illuminator;
pub use frame::{FrameCircle, FrameCorners, FrameHeader, FrameLines, FrameOctagon, FrameSides, FrameUnderline};
pub use text::{TextDecipher, TextSequence};
pub use theme::hud as colors;
