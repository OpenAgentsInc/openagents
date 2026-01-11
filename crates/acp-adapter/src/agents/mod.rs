//! Agent wrappers for different AI coding agents
//!
//! Provides configuration and connection helpers for specific agents
//! like Codex and GPT-OSS.

pub mod codex;
pub mod fm_bridge;
pub mod gpt_oss;

pub use codex::{CodexAgentConfig, connect_codex};
pub use fm_bridge::{FmBridgeAgentConfig, connect_fm_bridge};
pub use gpt_oss::{GptOssAgentConfig, connect_gpt_oss};
