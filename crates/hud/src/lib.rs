//! HUD - Arwes-style sci-fi HUD components for WGPUI.
//!
//! This crate provides GPU-accelerated sci-fi UI components inspired by
//! the Arwes framework, adapted for the WGPUI rendering system.
//!
//! # Components
//!
//! - [`animator::HudAnimator`] - Core animation state machine
//! - [`animator::AnimatorManager`] - Orchestrates child animations
//! - [`frame::FrameCorners`] - Bracket-style corner frames
//! - [`frame::FrameLines`] - Edge line frames with gaps
//! - [`background::DotGridBackground`] - Animated dot grid
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
pub mod frame;
pub mod theme;

// Re-export commonly used types
pub use animator::{AnimatorManager, AnimatorState, HudAnimator, ManagerMode};
pub use background::DotGridBackground;
pub use button::HudButton;
pub use frame::{FrameCorners, FrameLines, FrameSides};
pub use theme::hud as colors;
