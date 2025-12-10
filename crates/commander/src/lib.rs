//! Commander library - ATIF visualization components
//!
//! This module exports the GPUI components used for rendering ATIF trajectories,
//! making them available for use in the storybook and other crates.

pub mod actions;
pub mod app_menus;
pub mod components;

// Re-export TextInput from ui crate for backwards compatibility
pub use ui::{TextInput, SubmitEvent, bind_text_input_keys};
