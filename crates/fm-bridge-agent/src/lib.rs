//! FM Bridge Agent Wrapper
//!
//! Agent-level abstraction for Apple's Foundation Models (FM) bridge
//! that mirrors the GPT-OSS agent interface with tool execution and
//! trajectory recording.

pub mod agent;
pub mod error;
pub mod session;

pub use agent::{FmBridgeAgent, FmBridgeAgentConfig};
pub use error::{FmBridgeAgentError, Result};
pub use session::{FmBridgeSession, Message, SessionState, ToolCall};
