//! Autopilot - Terminal-style AI coding interface
//!
//! This crate provides a GPU-accelerated terminal UI for Claude Code and Codex.

#[path = "app_entry.rs"]
mod app_entry;
pub mod app;
pub mod autopilot_loop;
pub mod commands;
pub mod keybindings;
pub mod panels;

pub use app_entry::AutopilotApp;
