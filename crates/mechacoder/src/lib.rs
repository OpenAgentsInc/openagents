//! MechaCoder - Claude Code harness with GPUI interface.
//!
//! This crate provides a focused UI for interacting with Claude Code,
//! featuring chat, diffs, terminal output, and permission handling.

pub mod actions;
pub mod app_menus;
pub mod screen;
pub mod ui;

// Re-export key types
pub use actions::*;
pub use screen::MechaCoderScreen;
