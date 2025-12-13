//! Coder UI surface (Dioxus) for the coding agent platform.
//!
//! This crate hosts the Coder Studio components so the main Dioxus app can import
//! without cluttering its local views tree.

pub mod data;
pub mod types;
pub mod workflow;

mod database;
mod deploy;
mod editor;
mod infra;
mod projects;
mod screen;

pub use screen::CoderScreen;

// Shared theme constants for Coder Studio
pub const BG: &str = "#030303";
pub const PANEL: &str = "#0a0a0a";
pub const BORDER: &str = "#1c1c1c";
pub const TEXT: &str = "#e6e6e6";
pub const MUTED: &str = "#9a9a9a";
pub const ACCENT: &str = "#ffb400";
