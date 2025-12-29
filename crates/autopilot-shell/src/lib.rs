//! Unified Autopilot Shell
//!
//! Combines HUD effects with dock-based IDE layout for the autopilot interface.

mod actions;
mod dock;
mod hud;
mod keymap;
mod panels;
mod shell;

pub use actions::*;
pub use dock::{Dock, DockPosition, Panel};
pub use keymap::shell_keymap;
pub use shell::AutopilotShell;
