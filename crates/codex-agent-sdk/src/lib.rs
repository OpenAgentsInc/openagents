//! # Codex Agent SDK for Rust
//!
//! A Rust SDK for programmatically interacting with the Codex CLI (OpenAI's AI coding agent).
//! Create autonomous agents that can understand codebases, edit files, run commands,
//! and execute complex workflows.
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use codex_agent_sdk::{Codex, ThreadOptions, TurnOptions};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), codex_agent_sdk::Error> {
//!     let codex = Codex::new();
//!     let mut thread = codex.start_thread(ThreadOptions::default());
//!
//!     // Run a simple query
//!     let turn = thread.run("What files are in this directory?", TurnOptions::default()).await?;
//!     println!("Response: {}", turn.final_response);
//!
//!     Ok(())
//! }
//! ```
//!
//! ## Streaming Example
//!
//! ```rust,no_run
//! use codex_agent_sdk::{Codex, ThreadOptions, TurnOptions, ThreadEvent};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), codex_agent_sdk::Error> {
//!     let codex = Codex::new();
//!     let mut thread = codex.start_thread(ThreadOptions::default());
//!
//!     let mut streamed = thread.run_streamed("Analyze this codebase", TurnOptions::default()).await?;
//!
//!     while let Some(event) = streamed.next().await {
//!         match event? {
//!             ThreadEvent::ItemCompleted(item) => {
//!                 println!("Item completed: {:?}", item);
//!             }
//!             ThreadEvent::TurnCompleted(tc) => {
//!                 println!("Turn completed with {} input tokens", tc.usage.input_tokens);
//!             }
//!             _ => {}
//!         }
//!     }
//!
//!     Ok(())
//! }
//! ```
//!
//! ## Features
//!
//! - **Streaming responses**: Process events as they arrive via async streams
//! - **Thread management**: Continue conversations across multiple turns
//! - **Sandbox modes**: Control file system access (read-only, workspace-write, full-access)
//! - **Structured output**: Request JSON responses with schema validation
//!
//! ## Protocol
//!
//! This SDK communicates with the Codex CLI via JSONL over stdin/stdout.
//! The CLI is spawned with `codex exec --experimental-json`.

pub mod error;
pub mod events;
pub mod items;
pub mod options;
pub mod thread;
pub mod transport;

// Re-export main types at crate root
pub use error::{Error, Result};
pub use events::{
    ItemCompletedEvent, ItemStartedEvent, ItemUpdatedEvent, ThreadErrorEvent, ThreadEvent,
    ThreadStartedEvent, TurnCompletedEvent, TurnFailedEvent, TurnStartedEvent, Usage,
};
pub use items::{
    AgentMessageItem, CommandExecutionItem, CommandExecutionStatus, ErrorItem, FileChangeItem,
    FileUpdateChange, McpToolCallItem, McpToolCallItemError, McpToolCallItemResult,
    McpToolCallStatus, PatchApplyStatus, PatchChangeKind, ReasoningItem, ThreadItem,
    ThreadItemDetails, TodoItem, TodoListItem, WebSearchItem,
};
pub use options::{
    ApprovalMode, CodexOptions, ModelReasoningEffort, SandboxMode, ThreadOptions, TurnOptions,
};
pub use thread::{Input, StreamedTurn, Thread, Turn, UserInput};

/// Main Codex SDK client.
///
/// Use this struct to create and manage conversation threads with the Codex agent.
///
/// # Example
///
/// ```rust,no_run
/// use codex_agent_sdk::{Codex, ThreadOptions, TurnOptions};
///
/// # async fn example() -> Result<(), codex_agent_sdk::Error> {
/// let codex = Codex::new();
/// let mut thread = codex.start_thread(ThreadOptions::default());
/// let turn = thread.run("Hello!", TurnOptions::default()).await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone, Default)]
pub struct Codex {
    options: CodexOptions,
}

impl Codex {
    /// Create a new Codex client with default options.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a new Codex client with custom options.
    pub fn with_options(options: CodexOptions) -> Self {
        Self { options }
    }

    /// Start a new conversation thread.
    ///
    /// # Arguments
    /// * `options` - Thread configuration (model, sandbox mode, working directory, etc.)
    pub fn start_thread(&self, options: ThreadOptions) -> Thread {
        Thread::new(self.options.clone(), options, None)
    }

    /// Resume an existing thread by ID.
    ///
    /// # Arguments
    /// * `id` - The thread ID from a previous session
    /// * `options` - Thread configuration
    pub fn resume_thread(&self, id: impl Into<String>, options: ThreadOptions) -> Thread {
        Thread::new(self.options.clone(), options, Some(id.into()))
    }
}

/// Convenience function to create a thread with default options.
///
/// # Example
///
/// ```rust,no_run
/// use codex_agent_sdk::{thread, ThreadOptions, TurnOptions};
///
/// # async fn example() -> Result<(), codex_agent_sdk::Error> {
/// let mut t = thread(ThreadOptions::default());
/// let turn = t.run("What is 2 + 2?", TurnOptions::default()).await?;
/// # Ok(())
/// # }
/// ```
pub fn thread(options: ThreadOptions) -> Thread {
    Codex::new().start_thread(options)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_codex_new() {
        let codex = Codex::new();
        assert!(codex.options.codex_path_override.is_none());
    }

    #[test]
    fn test_thread_options_default() {
        let options = ThreadOptions::default();
        assert!(options.model.is_none());
        assert!(options.sandbox_mode.is_none());
    }
}
