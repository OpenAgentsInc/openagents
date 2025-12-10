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

pub mod text_input;

pub use text_input::{TextInput, SubmitEvent, bind_text_input_keys};
