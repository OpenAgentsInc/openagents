//! Editor-agnostic vim emulation library
//!
//! This crate provides a complete vim emulation layer that can be integrated
//! with any editor. It's designed to be independent of any particular UI framework
//! or editor implementation.
//!
//! # Usage
//!
//! 1. Implement the `VimEditor` trait for your editor
//! 2. Create a `VimHandler` instance
//! 3. Forward key events to `VimHandler::handle_key`
//!
//! # Example
//!
//! ```ignore
//! use vim::{VimHandler, VimEditor, Key, Modifiers, KeyResult};
//!
//! struct MyEditor {
//!     // ... your editor state
//! }
//!
//! impl VimEditor for MyEditor {
//!     type Pos = vim::SimplePosition;
//!     // ... implement trait methods
//! }
//!
//! let mut editor = MyEditor::new();
//! let mut vim = VimHandler::new();
//!
//! // Handle key press
//! match vim.handle_key(Key::Char('j'), Modifiers::none(), &mut editor) {
//!     KeyResult::Handled => { /* key was consumed */ }
//!     KeyResult::Ignored => { /* let editor handle it */ }
//!     KeyResult::ModeChanged(mode) => { /* update UI for new mode */ }
//!     KeyResult::TextChanged => { /* content was modified */ }
//!     KeyResult::EnterCommand => { /* show : prompt */ }
//!     KeyResult::EnterSearch { forward } => { /* show search prompt */ }
//! }
//! ```

mod editor;
mod handler;
mod mode;
mod motion;
mod object;
mod operator;
mod state;

// Re-export main types
pub use editor::{
    CharClass, Key, KeyResult, Modifiers, Position, SimplePosition, TextRange, VimEditor,
};
pub use handler::VimHandler;
pub use mode::Mode;
pub use motion::{Motion, MotionKind};
pub use object::Object;
pub use operator::Operator;
pub use state::{RecordedAction, VimState};
