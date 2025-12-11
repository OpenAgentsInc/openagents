//! ACP (Agent Client Protocol) connection layer for Claude Code.
//!
//! This crate provides the core protocol handling for communicating with
//! Claude Code via the Agent Client Protocol over stdio.
//!
//! # Architecture
//!
//! The crate is organized into several modules:
//!
//! - `types` - Core type definitions and traits
//! - `connection` - ACP connection management over stdio
//! - `session` - Session state and message handling
//! - `claude_code` - Claude Code binary discovery and spawning
//! - `terminal` - Terminal wrapper for tool execution
//! - `error` - Error types

pub mod claude_code;
pub mod connection;
pub mod error;
pub mod session;
pub mod terminal;
pub mod types;

// Re-export key types
pub use agent_client_protocol as acp;
pub use claude_code::ClaudeCode;
pub use connection::{AcpConnection, AgentServerCommand};
pub use error::{AcpError, Result};
pub use session::{AcpThread, AcpThreadEvent, TerminalState};
pub use terminal::TerminalOutput;
pub use types::{
    AgentConnection, AgentModelInfo, AgentModelList, AgentModelSelector, AgentSessionModes,
    AgentSettings, AssistantMessage, AssistantMessageChunk, ContentBlock, Project, ThreadEntry,
    ThreadStatus, ToolCall, ToolCallContent, ToolCallStatus, UserMessage, UserMessageId,
};
