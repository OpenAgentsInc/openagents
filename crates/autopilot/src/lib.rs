//! Autopilot - Terminal-style AI coding interface
//!
//! This crate provides a GPU-accelerated terminal UI for Codex.

pub mod app;
#[path = "app_entry.rs"]
mod app_entry;
pub mod autopilot_loop;
pub mod commands;
pub mod keybindings;
pub mod panels;

pub use app_entry::AutopilotApp;
