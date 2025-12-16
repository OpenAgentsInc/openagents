//! OpenAI Codex CLI Agent Core
//!
//! This crate provides near 100% parity with the OpenAI Codex CLI agent,
//! ported from <https://github.com/openai/codex>.
//!
//! # Overview
//!
//! The Codex agent is an autonomous coding assistant that:
//! 1. Receives user prompts and context
//! 2. Streams responses from OpenAI models (GPT-4, GPT-5, etc.)
//! 3. Executes tools (shell, file operations, patches)
//! 4. Continues until the task is complete
//!
//! # Example
//!
//! ```ignore
//! use codex::{Codex, Config, CodexSpawnOk};
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let config = Config::builder()
//!         .model("gpt-4o")
//!         .working_directory(".")
//!         .build()?;
//!
//!     let CodexSpawnOk { codex, conversation_id } = Codex::spawn(config).await?;
//!
//!     codex.submit(Submission::UserTurn {
//!         content: "Hello, help me with my code".into()
//!     }).await?;
//!
//!     while let Ok(event) = codex.recv().await {
//!         match event.msg {
//!             EventMsg::AgentMessageContentDelta { delta } => print!("{}", delta),
//!             EventMsg::TurnCompleted { .. } => break,
//!             _ => {}
//!         }
//!     }
//!
//!     Ok(())
//! }
//! ```
//!
//! # Architecture
//!
//! ```text
//! User Input
//!      │
//!      ▼
//! ┌─────────┐
//! │ Codex   │◄─────────────────────────────┐
//! │ Session │                              │
//! └────┬────┘                              │
//!      │                                   │
//!      ▼                                   │
//! ┌─────────┐    ┌─────────┐    ┌────────┐│
//! │ Stream  │───▶│ Process │───▶│Execute ││
//! │ OpenAI  │    │ Response│    │ Tools  ││
//! └─────────┘    └─────────┘    └───┬────┘│
//!                                   │     │
//!                                   ▼     │
//!                              Has more   │
//!                              tool calls?│
//!                                   │     │
//!                              Yes──┘     │
//!                              No───▶ Done│
//! ```
//!
//! # Modules
//!
//! - [`protocol`] - Wire protocol types (Event, Submission, etc.)
//! - [`mcp_types`] - Model Context Protocol types
//! - [`client`] - HTTP client infrastructure
//! - [`api`] - OpenAI API wrapper (Responses API + Chat Completions)
//! - [`utils`] - Shared utilities (paths, strings, git, async)
//! - [`apply_patch`] - Unified diff patching
//! - [`execpolicy`] - Command execution policies
//! - [`file_search`] - File search (nucleo-based)
//! - [`rmcp_client`] - MCP client
//! - [`core`] - Main agent engine
//! - [`stubs`] - Stubs for removed dependencies

// Phase 1: Foundation modules
pub mod api;
pub mod client;
pub mod mcp_types;
pub mod protocol;

// Phase 2: Utilities & Tools
pub mod apply_patch;
pub mod execpolicy;
pub mod file_search;
pub mod utils;

// Phase 3: MCP
pub mod rmcp_client;

// Phase 4: Core Agent
pub mod core;
pub mod stubs;

// Re-exports for convenience
// TODO: Add re-exports after core module is implemented
// pub use core::codex::Codex;
// pub use core::codex::CodexSpawnOk;
// pub use core::config::Config;
// pub use protocol::protocol::{Event, EventMsg, Submission};
