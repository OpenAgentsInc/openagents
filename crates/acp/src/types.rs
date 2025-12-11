//! Core type definitions and traits for ACP.
//!
//! This module contains the fundamental types and traits used throughout
//! the ACP crate, adapted from Zed's acp_thread crate.

use agent_client_protocol as acp;
use anyhow::Result;
use gpui::{App, Entity, SharedString, Task};
use std::path::{Path, PathBuf};
use std::rc::Rc;

/// Unique identifier for user messages.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct UserMessageId(pub String);

impl UserMessageId {
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }
}

impl Default for UserMessageId {
    fn default() -> Self {
        Self::new()
    }
}

/// Minimal project stub for ACP operations.
///
/// This is a simplified version of Zed's Project struct, containing
/// only the fields needed for ACP operations.
#[derive(Clone, Debug)]
pub struct Project {
    /// Root directory path.
    pub root_path: PathBuf,
    /// Whether this is a local project (vs remote/SSH).
    pub is_local: bool,
}

impl Project {
    /// Create a new local project.
    pub fn local(root_path: impl Into<PathBuf>) -> Self {
        Self {
            root_path: root_path.into(),
            is_local: true,
        }
    }

    /// Create a new remote project.
    pub fn remote(root_path: impl Into<PathBuf>) -> Self {
        Self {
            root_path: root_path.into(),
            is_local: false,
        }
    }
}

/// Agent settings for configuration.
#[derive(Clone, Debug, Default)]
pub struct AgentSettings {
    /// Default model to use.
    pub default_model: Option<String>,
    /// Default mode to use.
    pub default_mode: Option<String>,
}

/// Trait for agent connections.
///
/// This trait defines the interface that all agent connections must implement,
/// allowing for different agent backends (Claude Code, Gemini, etc.).
pub trait AgentConnection: 'static {
    /// Get the telemetry ID for this agent.
    fn telemetry_id(&self) -> &'static str;

    /// Get available authentication methods.
    fn auth_methods(&self) -> &[acp::AuthMethod];

    /// Authenticate using the specified method.
    fn authenticate(&self, method_id: acp::AuthMethodId, cx: &mut App) -> Task<Result<()>>;

    /// Create a new conversation thread.
    fn new_thread(
        self: Rc<Self>,
        project: Project,
        cwd: &Path,
        cx: &mut App,
    ) -> Task<Result<Entity<crate::AcpThread>>>;

    /// Send a prompt to the agent.
    fn prompt(
        &self,
        id: Option<UserMessageId>,
        params: acp::PromptRequest,
        cx: &mut App,
    ) -> Task<Result<acp::PromptResponse>>;

    /// Cancel the current generation.
    fn cancel(&self, session_id: &acp::SessionId, cx: &mut App);

    /// Get session modes if supported.
    fn session_modes(
        &self,
        session_id: &acp::SessionId,
        cx: &App,
    ) -> Option<Rc<dyn AgentSessionModes>>;

    /// Get model selector if supported.
    fn model_selector(&self, session_id: &acp::SessionId) -> Option<Rc<dyn AgentModelSelector>>;

    /// Convert to Any for downcasting.
    fn into_any(self: Rc<Self>) -> Rc<dyn std::any::Any>;
}

/// Trait for managing session modes.
pub trait AgentSessionModes: 'static {
    /// Get the current mode ID.
    fn current_mode(&self) -> acp::SessionModeId;

    /// Get all available modes.
    fn all_modes(&self) -> Vec<acp::SessionMode>;

    /// Set the current mode.
    fn set_mode(&self, mode_id: acp::SessionModeId, cx: &mut App) -> Task<Result<()>>;
}

/// Information about an agent model.
#[derive(Clone, Debug)]
pub struct AgentModelInfo {
    pub model_id: acp::ModelId,
    pub name: SharedString,
    pub description: Option<SharedString>,
}

impl From<acp::ModelInfo> for AgentModelInfo {
    fn from(model: acp::ModelInfo) -> Self {
        Self {
            model_id: model.model_id,
            name: model.name.into(),
            description: model.description.map(Into::into),
        }
    }
}

/// Model list variants.
pub enum AgentModelList {
    /// Flat list of models.
    Flat(Vec<AgentModelInfo>),
    /// Grouped models (by provider, etc.).
    Grouped(Vec<(SharedString, Vec<AgentModelInfo>)>),
}

/// Trait for selecting agent models.
pub trait AgentModelSelector: 'static {
    /// List available models.
    fn list_models(&self, cx: &mut App) -> Task<Result<AgentModelList>>;

    /// Select a model.
    fn select_model(&self, model_id: acp::ModelId, cx: &mut App) -> Task<Result<()>>;

    /// Get the currently selected model.
    fn selected_model(&self, cx: &mut App) -> Task<Result<AgentModelInfo>>;
}

/// Content block in a message.
#[derive(Clone, Debug)]
pub enum ContentBlock {
    /// Text content.
    Text(String),
    /// Image content (base64 encoded).
    Image { data: String, media_type: String },
}

impl ContentBlock {
    /// Create a text content block.
    pub fn text(content: impl Into<String>) -> Self {
        Self::Text(content.into())
    }
}

/// User message in a thread.
#[derive(Clone, Debug)]
pub struct UserMessage {
    pub id: Option<UserMessageId>,
    pub content: ContentBlock,
    pub chunks: Vec<acp::ContentBlock>,
}

/// Chunk of an assistant message.
#[derive(Clone, Debug)]
pub enum AssistantMessageChunk {
    /// Regular message content.
    Message { content: String },
    /// Extended thinking content.
    Thought { content: String },
}

/// Assistant message in a thread.
#[derive(Clone, Debug)]
pub struct AssistantMessage {
    pub chunks: Vec<AssistantMessageChunk>,
}

/// Status of a tool call.
#[derive(Clone, Debug, Default)]
pub enum ToolCallStatus {
    #[default]
    Pending,
    WaitingForConfirmation {
        options: Vec<acp::PermissionOption>,
    },
    InProgress,
    Completed,
    Failed {
        error: String,
    },
    Rejected,
    Canceled,
}

impl From<acp::ToolCallStatus> for ToolCallStatus {
    fn from(status: acp::ToolCallStatus) -> Self {
        match status {
            acp::ToolCallStatus::Pending => Self::Pending,
            acp::ToolCallStatus::InProgress => Self::InProgress,
            acp::ToolCallStatus::Completed => Self::Completed,
            acp::ToolCallStatus::Failed => Self::Failed {
                error: "Unknown error".into(),
            },
            _ => Self::Pending, // Handle any future variants
        }
    }
}

/// Content within a tool call.
#[derive(Clone, Debug)]
pub enum ToolCallContent {
    /// Text content.
    Text(String),
    /// File diff.
    Diff {
        path: PathBuf,
        old_content: String,
        new_content: String,
    },
    /// Terminal output.
    Terminal {
        terminal_id: acp::TerminalId,
        output: String,
    },
}

/// A tool call in a thread.
#[derive(Clone, Debug)]
pub struct ToolCall {
    pub id: acp::ToolCallId,
    pub title: String,
    pub kind: acp::ToolKind,
    pub content: Vec<ToolCallContent>,
    pub status: ToolCallStatus,
    pub raw_input: Option<serde_json::Value>,
    pub raw_output: Option<serde_json::Value>,
}

/// Entry in a thread.
#[derive(Clone, Debug)]
pub enum ThreadEntry {
    UserMessage(UserMessage),
    AssistantMessage(AssistantMessage),
    ToolCall(ToolCall),
}

/// Status of a thread.
#[derive(Clone, Debug, Default)]
pub enum ThreadStatus {
    #[default]
    Idle,
    Streaming,
    WaitingForConfirmation,
    Error(String),
}
