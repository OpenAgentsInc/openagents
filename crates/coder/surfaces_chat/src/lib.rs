//! # coder_surfaces_chat - Chat Surface for Coder
//!
//! The chat surface renders conversation threads with:
//! - Virtual scrolling for large message histories
//! - Streaming markdown rendering
//! - Tool use indicators
//! - Chat input
//!
//! This is the first vertical slice of the "own all six layers" implementation.

pub mod input;
pub mod message;
pub mod thread;
pub mod tool_use;

// Re-exports
pub use input::ChatInput;
pub use message::MessageBubble;
pub use thread::ChatThread;
pub use tool_use::ToolUseIndicator;
