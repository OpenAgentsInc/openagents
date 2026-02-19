//! Agent backend trait definitions
//!
//! Defines the abstraction layer for AI coding agents (Codex, etc.)
//! following Zed's AgentServer pattern.

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::mpsc;

use crate::app::events::ResponseEvent;

/// Information about a model available from an agent
#[derive(Debug, Clone)]
pub struct ModelInfo {
    /// Model identifier (e.g., "gpt-4o")
    pub id: String,
    /// Human-readable name (e.g., "GPT-4o")
    pub name: String,
    /// Optional description
    pub description: Option<String>,
    /// Whether this is the default model
    pub is_default: bool,
}

/// Configuration for connecting to an agent
#[allow(dead_code)]
#[derive(Debug, Clone, Default)]
pub struct AgentConfig {
    /// Working directory for the agent
    pub cwd: std::path::PathBuf,
    /// Model to use (if None, uses agent's default)
    pub model_id: Option<String>,
    /// Permission mode ("default", "plan", "bypassPermissions")
    pub permission_mode: Option<String>,
    /// Resume an existing session
    pub resume_session: Option<String>,
    /// Fork from a session
    pub fork_session: bool,
    /// MCP servers to pass to the agent
    pub mcp_servers: Vec<serde_json::Value>,
    /// Agent definitions
    pub agents: Vec<serde_json::Value>,
    /// Additional settings
    pub extra_settings: HashMap<String, String>,
    /// Maximum thinking tokens
    pub max_thinking_tokens: Option<u32>,
    /// Persist session
    pub persist_session: bool,
}

/// Result of agent availability check
#[derive(Debug, Clone)]
pub struct AgentAvailability {
    /// Whether the agent is available
    pub available: bool,
    /// Path to executable if available
    pub executable_path: Option<std::path::PathBuf>,
    /// Version string if available
    pub version: Option<String>,
    /// Error message if not available
    pub error: Option<String>,
}

/// Types of agents supported
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AgentKind {
    /// OpenAI Codex CLI
    Codex,
}

impl AgentKind {
    /// Get all agent kinds
    pub fn all() -> &'static [AgentKind] {
        &[AgentKind::Codex]
    }

    /// Get display name
    pub fn display_name(&self) -> &'static str {
        match self {
            AgentKind::Codex => "Codex",
        }
    }

    /// Get icon name (for UI rendering)
    pub fn icon(&self) -> &'static str {
        match self {
            AgentKind::Codex => "openai",
        }
    }

    /// Get executable name to search for
    pub fn executable_name(&self) -> &'static str {
        match self {
            AgentKind::Codex => "codex",
        }
    }
}

impl std::fmt::Display for AgentKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

/// Trait for agent backends
///
/// This is our equivalent of Zed's `AgentServer` trait, adapted for
/// the Coder app's architecture.
pub trait AgentBackend: Send + Sync {
    /// Get the agent kind
    fn kind(&self) -> AgentKind;

    /// Get display name
    fn display_name(&self) -> &'static str {
        self.kind().display_name()
    }

    /// Get icon name
    fn icon(&self) -> &'static str {
        self.kind().icon()
    }

    /// Check if agent is available
    fn check_availability(&self) -> AgentAvailability;

    /// Query available models from the agent
    ///
    /// This is called to dynamically discover models. For some agents
    /// this may require spawning the agent process briefly.
    fn available_models(&self) -> Pin<Box<dyn Future<Output = Vec<ModelInfo>> + Send + '_>>;

    /// Get default model ID
    fn default_model_id(&self) -> Option<&str>;

    /// Connect to the agent and create a session
    #[allow(dead_code)]
    fn connect(
        &self,
        config: AgentConfig,
        response_tx: mpsc::UnboundedSender<ResponseEvent>,
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<Box<dyn AgentSession>>> + Send + '_>>;
}

/// Trait for an active agent session
///
/// Represents a connected session with an agent that can send prompts
/// and receive responses.
#[allow(dead_code)]
#[async_trait]
pub trait AgentSession: Send {
    /// Send a prompt to the agent
    async fn prompt(&mut self, text: &str) -> anyhow::Result<()>;

    /// Interrupt the current request
    async fn interrupt(&mut self) -> anyhow::Result<()>;

    /// Abort the current request
    async fn abort(&mut self) -> anyhow::Result<()>;

    /// Get the session ID if available
    fn session_id(&self) -> Option<&str>;

    /// Get MCP server status
    async fn mcp_server_status(&mut self) -> anyhow::Result<serde_json::Value>;

    /// Rewind files to a checkpoint
    async fn rewind_files(&mut self, user_message_id: &str) -> anyhow::Result<()>;
}

/// Type alias for boxed agent backend
pub type BoxedAgentBackend = Arc<dyn AgentBackend>;
