//! Action system for WGPUI
//!
//! Provides a type-safe action dispatch system inspired by Zed's GPUI.
//!
//! # Overview
//!
//! Actions are named commands that can be triggered by keybindings or programmatically.
//! They enable keyboard-first workflows with user-customizable keybindings.
//!
//! # Example
//!
//! ```ignore
//! use wgpui::action::{Action, KeyBinding, Keymap};
//!
//! // Define an action
//! #[derive(Debug, Clone, Default)]
//! struct Save;
//! impl Action for Save {
//!     fn name() -> &'static str { "editor::Save" }
//!     fn boxed_clone(&self) -> Box<dyn AnyAction> { Box::new(self.clone()) }
//! }
//!
//! // Create a keybinding
//! let binding = KeyBinding::new("cmd-s", Save).unwrap();
//!
//! // Add to keymap
//! let mut keymap = Keymap::new();
//! keymap.add(binding);
//! ```

mod binding;
mod core;
mod dispatch;
mod keystroke;
#[macro_use]
mod macros;
mod registry;
pub mod standard;

pub use binding::KeyBinding;
pub use core::{Action, ActionId, AnyAction, NoAction};
pub use dispatch::{ActionHandler, ActionListeners, DispatchPhase, DispatchResult, PendingAction};
pub use keystroke::{Keystroke, KeystrokeMatch, KeystrokeParseError};
pub use registry::ActionRegistry;
