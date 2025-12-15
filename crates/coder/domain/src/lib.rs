//! # coder_domain - Domain Model for Coder
//!
//! This crate defines the core domain entities, events, and projections
//! for the Coder agent platform. It follows an event-sourced architecture
//! where all state changes are captured as append-only domain events.
//!
//! ## Architecture
//!
//! - **Entities**: Core domain objects (Message, Run, Project, etc.)
//! - **Events**: Append-only event stream capturing all state changes
//! - **Projections**: Derived views optimized for UI consumption
//!
//! ## Example
//!
//! ```rust,ignore
//! use coder_domain::{Message, Role, DomainEvent};
//!
//! // Create a new message
//! let msg = Message::new(Role::User, "Hello, agent!");
//!
//! // Events capture all changes
//! let event = DomainEvent::MessageAdded {
//!     thread_id: ThreadId::new(),
//!     message: msg,
//! };
//! ```

pub mod event;
pub mod ids;
pub mod message;
pub mod projections;
pub mod run;
pub mod tool;

// Re-exports
pub use event::DomainEvent;
pub use ids::*;
pub use message::{Message, Role};
pub use projections::chat_view::{
    ChatEntry, ChatSnapshot, ChatView, MessageView, StreamingMessage, ThreadSummary,
};
pub use run::{Run, RunStatus, StepRun, StepStatus};
pub use tool::{ToolResult, ToolUse};
