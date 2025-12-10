//! OpenAgents UI Components
//!
//! Shared UI components used across the application.
//!
//! # Usage
//!
//! ```rust
//! use ui::{TextInput, bind_text_input_keys};
//!
//! // In your app initialization:
//! bind_text_input_keys(cx);
//!
//! // Create a text input:
//! let input = cx.new(|cx| TextInput::new("Placeholder...", cx));
//! ```
//!
//! ## Shadcn-style Components
//!
//! ```rust
//! use ui::{Button, ButtonVariant, Checkbox, Progress};
//!
//! // Button with variants
//! Button::new("Save").variant(ButtonVariant::Default)
//! Button::new("Delete").variant(ButtonVariant::Destructive)
//!
//! // Checkbox
//! Checkbox::new().checked(true)
//!
//! // Progress bar
//! Progress::new().value(0.75)
//! ```

pub mod text_input;
pub mod components;

pub use text_input::{TextInput, SubmitEvent, bind_text_input_keys};
pub use components::*;
