//! Coder - Terminal-style Claude Code interface
//!
//! This crate provides a GPU-accelerated terminal UI for Claude Code.

#[path = "app.rs"]
mod app_entry;
pub mod app;
pub mod autopilot_loop;
pub mod commands;
pub mod keybindings;
pub mod panels;

pub use app_entry::CoderApp;
