//! Session management and conversation processor for Coder.
//!
//! This crate provides:
//! - Session lifecycle management
//! - Conversation processor (main loop)
//! - System prompt builder
//! - Tool execution with permission integration

mod processor;
mod prompt;
mod session;

pub use processor::*;
pub use prompt::*;
pub use session::*;
