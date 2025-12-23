//! GPT-OSS Agent Wrapper
//!
//! Agent-level abstraction for GPT-OSS that integrates with the ACP (Agent Client Protocol)
//! adapter pattern. Provides tool handling and trajectory recording.

pub mod agent;
pub mod error;
pub mod tools;

pub use agent::{GptOssAgent, GptOssAgentConfig};
pub use error::{GptOssAgentError, Result};
