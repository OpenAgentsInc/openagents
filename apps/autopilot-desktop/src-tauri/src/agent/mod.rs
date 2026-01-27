//! Unified Agent Architecture
//!
//! This module provides a unified interface for multiple AI agents (Codex, Claude Code, Cursor)
//! using the Agent Client Protocol (ACP) as the base protocol.

pub mod acp_agent;
pub mod adjutant;
pub mod commands;
pub mod manager;
pub mod resolver;
pub mod trait_def;
pub mod ui;
pub mod unified;

#[expect(unused_imports)]
pub use acp_agent::AcpAgent;
#[expect(unused_imports)]
pub use adjutant::AdjutantAgent;
#[expect(unused_imports)]
pub use manager::AgentManager;
#[expect(unused_imports)]
pub use trait_def::Agent;
#[expect(unused_imports)]
pub use unified::{AgentId, UnifiedConversationItem, UnifiedEvent};
