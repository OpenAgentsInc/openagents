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
pub use connection::AcpConnection;
pub use error::{AcpError, Result};
pub use session::{AcpSession, AcpThread};
pub use types::{AgentConnection, AgentSettings, Project};
