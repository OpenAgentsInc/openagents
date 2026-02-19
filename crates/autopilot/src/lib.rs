//! Autopilot - Terminal-style AI coding interface
//!
//! This crate provides a GPU-accelerated terminal UI for Codex.

pub mod app;
#[path = "app_entry.rs"]
mod app_entry;
pub mod autopilot_loop;
pub mod commands;
#[cfg(feature = "single-instance")]
mod instance;
pub mod keybindings;
pub mod panels;

#[cfg(test)]
mod tests;

pub use app_entry::AutopilotApp;
#[cfg(feature = "single-instance")]
pub use instance::kill_other_instances;
