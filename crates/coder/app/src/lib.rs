//! # coder_app - Main Application Entry Point
//!
//! This crate provides the main entry point for Coder on both web and desktop.
//! It initializes the platform, sets up the application state, and runs the
//! main event loop.
//!
//! ## Architecture
//!
//! The app owns all six layers of the UI stack:
//! 1. Domain Model (coder_domain)
//! 2. UI Runtime (coder_ui_runtime)
//! 3. Layout Engine (wgpui/Taffy)
//! 4. Widgets (coder_widgets, coder_surfaces_*)
//! 5. Renderer (wgpui)
//! 6. Platform Glue (wgpui/platform)
//!
//! This architecture replaces Dioxus entirely with our own Rust-native stack.

pub mod app;
pub mod state;

// Re-exports
pub use app::App;
pub use state::AppState;
