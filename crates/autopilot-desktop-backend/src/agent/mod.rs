//! Unified Agent Architecture
//! 
//! This module provides a unified interface for multiple AI agents (Codex, Claude Code, Cursor)
//! using the Agent Client Protocol (ACP) as the base protocol.

pub mod unified;
pub mod trait_def;
pub mod acp_agent;
pub mod codex_agent;
pub mod gemini_agent;
pub mod adjutant;
pub mod manager;
pub mod commands;
pub mod resolver;
pub mod ui;

#[allow(unused_imports)]
pub use unified::{AgentId, UnifiedEvent, UnifiedConversationItem};
#[allow(unused_imports)]
pub use trait_def::Agent;
#[allow(unused_imports)]
pub use acp_agent::AcpAgent;
#[allow(unused_imports)]
pub use codex_agent::CodexAgent;
#[allow(unused_imports)]
pub use adjutant::AdjutantAgent;
#[allow(unused_imports)]
pub use manager::AgentManager;
