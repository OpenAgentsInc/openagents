//! Agent backend abstraction layer
//!
//! This module provides the abstraction for AI coding agents (Codex only today)
//! following Zed's AgentServer pattern but adapted for Coder's architecture.
//!
//! ## Overview
//!
//! The agent system allows Coder to work with Codex:
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
//! │   CodexBackend  │
//! │ (app-server)    │
//! └─────────────────┴───────────────────┘
//! ```

mod backend;
mod codex_backend;
mod registry;
mod state;

pub(crate) use backend::AgentKind;
pub(crate) use registry::AgentRegistry;
pub(crate) use state::{AgentBackendsEvent, AgentBackendsState, AgentBackendsStatus};
