//! Agent wrappers for different AI coding agents
//!
//! Provides configuration and connection helpers for specific agents
//! like Claude Code and Codex.

pub mod claude;
pub mod codex;

pub use claude::{connect_claude, ClaudeAgentConfig};
pub use codex::{connect_codex, CodexAgentConfig};
