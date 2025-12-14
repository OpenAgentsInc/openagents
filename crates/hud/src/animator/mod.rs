//! Animation system for HUD components.
//!
//! This module provides the core animation state machine and manager
//! for orchestrating complex animated UI sequences.

mod animator;
mod manager;
mod state;

pub use animator::HudAnimator;
pub use manager::{AnimatorManager, ManagerMode};
pub use state::AnimatorState;
