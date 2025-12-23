//! GPT-OSS Agent Wrapper
//!
//! Agent-level abstraction for GPT-OSS that integrates with the ACP (Agent Client Protocol)
//! adapter pattern. Provides tool handling and trajectory recording.
//!
//! ## Features
//!
//! - **Agent**: High-level agent with tool execution
//! - **Session**: Multi-turn conversation with history tracking
//! - **Tools**: Native Rust implementations of browser, python, and apply_patch
//! - **Trajectory Recording**: rlog-format flight recorder for reproducibility

pub mod agent;
pub mod error;
pub mod session;
pub mod tools;

pub use agent::{GptOssAgent, GptOssAgentConfig};
pub use error::{GptOssAgentError, Result};
pub use session::{GptOssSession, Message, SessionState, ToolCall};
