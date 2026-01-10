//! Agent backend abstraction layer
//!
//! This module provides the abstraction for AI coding agents (Claude, Codex, etc.)
//! following Zed's AgentServer pattern but adapted for Coder's architecture.
//!
//! ## Overview
//!
//! The agent system allows Coder to work with multiple AI backends:
//! - **Claude Code**: Anthropic's coding agent (primary)
//! - **Codex**: OpenAI's coding agent (primary)
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────┐
//! │  Coder UI       │
//! └────────┬────────┘
//!          │ selects agent
//!          ▼
//! ┌─────────────────┐
//! │ AgentRegistry   │
//! └────────┬────────┘
//!          │ dispatches to
//!          ▼
//! ┌─────────────────────────────────────┐
//! │          AgentBackend trait         │
//! ├─────────────────┬───────────────────┤
//! │  ClaudeBackend  │   CodexBackend    │
//! │ (claude-agent-  │ (codex-agent-     │
//! │      sdk)       │       sdk)        │
//! └─────────────────┴───────────────────┘
//! ```

mod backend;
mod claude_backend;
mod codex_backend;
mod registry;

// Re-export main types
pub use backend::{
    AgentAvailability, AgentBackend, AgentConfig, AgentKind, AgentSession, BoxedAgentBackend,
    ModelInfo,
};
pub use claude_backend::ClaudeBackend;
pub use codex_backend::CodexBackend;
pub use registry::{AgentRegistry, AgentStatus};
