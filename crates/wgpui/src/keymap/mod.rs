//! Keymap system for WGPUI
//!
//! Provides context-aware keybinding resolution with precedence rules.
//!
//! # Overview
//!
//! The keymap system maps keystrokes to actions based on the current UI context.
//! When a key is pressed, the keymap finds the best matching binding considering:
//! 1. Context specificity (deeper context wins)
//! 2. Binding order (later bindings override earlier)
//!
//! # Example
//!
//! ```ignore
//! use wgpui::keymap::{Keymap, KeyContext};
//! use wgpui::action::{KeyBinding, standard::Cancel};
//!
//! // Create a keymap
//! let mut keymap = Keymap::new();
//! keymap.add(KeyBinding::new("escape", Cancel).unwrap());
//!
//! // Create a context stack
//! let mut context = KeyContext::new();
//! context.push("Window");
//! context.push("Modal");
//!
//! // Match keystrokes
//! if let Some(action) = keymap.match_keystroke(&key, &modifiers, &context) {
//!     // Dispatch action...
//! }
//! ```

mod context;
mod core;
mod defaults;

pub use context::KeyContext;
pub use core::Keymap;
pub use defaults::default_keymap;
