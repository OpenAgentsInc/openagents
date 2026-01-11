//! Agent Client Protocol adapter for OpenAgents
//!
//! This crate provides an ACP adapter layer that wraps the existing
//! `codex-agent-sdk` and `codex-agent-sdk` to enable standardized
//! communication between OpenAgents apps and AI coding agents.
//!
//! ## Architecture
//!
//! ```text
//! +-------------------+     +------------------+     +----------------------+
//! |   Desktop GUI     |     |   acp-adapter    |     |  Agent subprocess    |
//! |   (wry + actix)   |---->|   (this crate)   |---->|  (CC / Codex)        |
//! +-------------------+     +------------------+     +----------------------+
//! ```
//!
//! ## Usage
//!
//! ```rust,ignore
//! use acp_adapter::agents::codex::connect_codex;
//!
//! let connection = connect_codex(CodexAgentConfig::default(), &cwd).await?;
//! let session = connection.new_session(cwd.clone()).await?;
//! connection.prompt(&session.session_id, "Fix the bug").await?;
//! ```

pub mod agents;
pub mod client;
pub mod connection;
pub mod converters;
pub mod error;
pub mod permissions;
pub mod replay;
pub mod session;
pub mod streaming;
pub mod telemetry;
pub mod transport;

// Re-export main types
pub use agent_client_protocol_schema as acp;
pub use client::{
    AllowAllPermissions, DenyAllPermissions, OpenAgentsClient, PermissionHandler,
    UiPermissionHandler,
};
pub use connection::AcpAgentConnection;
pub use error::{AcpError, Result};
pub use permissions::{
    PermissionOptionKind, PermissionRequestManager, UiPermissionOption, UiPermissionRequest,
    UiPermissionResponse,
};
pub use replay::{ReplayConfig, ReplayStats, RlogReplay};
pub use session::AcpAgentSession;
pub use streaming::{RlogBuffer, RlogHeaderInfo, RlogStreamer, StreamConfig};
pub use telemetry::{ActionEvent, ApmTelemetry};

/// Command configuration for spawning an agent subprocess
#[derive(Debug, Clone)]
pub struct AgentCommand {
    /// Path to the executable
    pub path: std::path::PathBuf,
    /// Command line arguments
    pub args: Vec<String>,
    /// Environment variables
    pub env: Vec<(String, String)>,
}

impl AgentCommand {
    /// Create a new agent command
    pub fn new(path: impl Into<std::path::PathBuf>) -> Self {
        Self {
            path: path.into(),
            args: Vec::new(),
            env: Vec::new(),
        }
    }

    /// Add an argument
    pub fn arg(mut self, arg: impl Into<String>) -> Self {
        self.args.push(arg.into());
        self
    }

    /// Add multiple arguments
    pub fn args<I, S>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.args.extend(args.into_iter().map(Into::into));
        self
    }

    /// Add an environment variable
    pub fn env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env.push((key.into(), value.into()));
        self
    }
}
