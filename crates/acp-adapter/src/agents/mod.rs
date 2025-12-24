//! Agent wrappers for different AI coding agents
//!
//! Provides configuration and connection helpers for specific agents
//! like Claude Code, Codex, and GPT-OSS.

pub mod claude;
pub mod codex;
pub mod gpt_oss;

pub use claude::{connect_claude, ClaudeAgentConfig};
pub use codex::{connect_codex, CodexAgentConfig};
pub use gpt_oss::{connect_gpt_oss, GptOssAgentConfig};
