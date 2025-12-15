//! Session state and management.

use chrono::{DateTime, Utc};
use coder_domain::{MessageId, SessionId, ThreadId};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use thiserror::Error;

/// Error types for session operations.
#[derive(Debug, Error)]
pub enum SessionError {
    #[error("Session not found: {0}")]
    NotFound(SessionId),

    #[error("Session is busy")]
    Busy,

    #[error("Session was aborted")]
    Aborted,

    #[error("Storage error: {0}")]
    Storage(#[from] coder_storage::StorageError),

    #[error("LLM error: {0}")]
    Llm(String),

    #[error("Tool error: {0}")]
    Tool(String),

    #[error("Permission error: {0}")]
    Permission(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

/// Session status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionStatus {
    /// Session is idle, waiting for user input.
    Idle,
    /// Session is processing a request.
    Busy,
    /// Session is waiting for permission approval.
    WaitingForPermission,
    /// Session is retrying after an error.
    Retrying { attempt: u32, next_retry_at: i64 },
    /// Session encountered an error.
    Error,
}

impl Default for SessionStatus {
    fn default() -> Self {
        Self::Idle
    }
}

/// Agent mode/configuration for the session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    /// Agent identifier (e.g., "default", "code", "architect").
    pub agent_id: String,
    /// Model to use (e.g., "claude-sonnet-4-20250514").
    pub model_id: String,
    /// Provider to use (e.g., "anthropic").
    pub provider_id: String,
    /// Maximum tokens for response.
    pub max_tokens: Option<u32>,
    /// Temperature for sampling.
    pub temperature: Option<f32>,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            agent_id: "default".to_string(),
            model_id: "claude-sonnet-4-5-20250929".to_string(),
            provider_id: "anthropic".to_string(),
            max_tokens: Some(8192),
            temperature: None,
        }
    }
}

/// A session represents a conversation context.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    /// Unique session identifier.
    pub id: SessionId,
    /// Thread for messages.
    pub thread_id: ThreadId,
    /// Working directory for file operations.
    pub working_directory: PathBuf,
    /// Session title (auto-generated or user-set).
    pub title: Option<String>,
    /// Current status.
    pub status: SessionStatus,
    /// Agent configuration.
    pub agent_config: AgentConfig,
    /// When the session was created.
    pub created_at: DateTime<Utc>,
    /// When the session was last updated.
    pub updated_at: DateTime<Utc>,
    /// Total cost accumulated.
    pub total_cost: f64,
    /// Total tokens used.
    pub total_tokens: u64,
    /// Current message being processed.
    pub current_message_id: Option<MessageId>,
}

impl Session {
    /// Create a new session.
    pub fn new(working_directory: impl Into<PathBuf>) -> Self {
        let now = Utc::now();
        Self {
            id: SessionId::new(),
            thread_id: ThreadId::new(),
            working_directory: working_directory.into(),
            title: None,
            status: SessionStatus::Idle,
            agent_config: AgentConfig::default(),
            created_at: now,
            updated_at: now,
            total_cost: 0.0,
            total_tokens: 0,
            current_message_id: None,
        }
    }

    /// Create with a specific agent config.
    pub fn with_agent(mut self, config: AgentConfig) -> Self {
        self.agent_config = config;
        self
    }

    /// Set the title.
    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Update the status.
    pub fn set_status(&mut self, status: SessionStatus) {
        self.status = status;
        self.updated_at = Utc::now();
    }

    /// Check if the session is busy.
    pub fn is_busy(&self) -> bool {
        matches!(
            self.status,
            SessionStatus::Busy
                | SessionStatus::WaitingForPermission
                | SessionStatus::Retrying { .. }
        )
    }

    /// Add cost and tokens from a completion.
    pub fn add_usage(&mut self, cost: f64, tokens: u64) {
        self.total_cost += cost;
        self.total_tokens += tokens;
        self.updated_at = Utc::now();
    }
}

/// Events emitted by sessions.
#[derive(Debug, Clone)]
pub enum SessionEvent {
    /// Session status changed.
    StatusChanged {
        session_id: SessionId,
        status: SessionStatus,
    },
    /// Message streaming started.
    MessageStarted {
        session_id: SessionId,
        message_id: MessageId,
    },
    /// Text delta received.
    TextDelta {
        session_id: SessionId,
        message_id: MessageId,
        delta: String,
    },
    /// Tool use started.
    ToolStarted {
        session_id: SessionId,
        message_id: MessageId,
        tool_name: String,
        tool_call_id: String,
    },
    /// Tool completed.
    ToolCompleted {
        session_id: SessionId,
        message_id: MessageId,
        tool_call_id: String,
        success: bool,
    },
    /// Message completed.
    MessageCompleted {
        session_id: SessionId,
        message_id: MessageId,
        finish_reason: String,
    },
    /// Error occurred.
    Error {
        session_id: SessionId,
        message_id: Option<MessageId>,
        error: String,
    },
}
