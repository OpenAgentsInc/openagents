//! Unified Autopilot Shell
//!
//! Combines HUD effects with dock-based IDE layout for the autopilot interface.

mod actions;
pub mod claude_sessions;
mod components;
mod dock;
mod hud;
mod keymap;
mod panels;
pub mod rate_limits;
mod shell;

pub use actions::*;
pub use components::FullAutoToggle;
pub use dock::{Dock, DockPosition, Panel};
pub use keymap::shell_keymap;
pub use rate_limits::{RateLimitFetcher, RateLimitSnapshot};
pub use shell::AutopilotShell;
