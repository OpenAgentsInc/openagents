//! Coder - Terminal-style Claude Code interface
//!
//! This crate provides a GPU-accelerated terminal UI for Claude Code.

pub mod app;
pub mod commands;
pub mod keybindings;
pub mod panels;

pub use app::CoderApp;
