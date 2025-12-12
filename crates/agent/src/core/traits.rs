//! Core traits for agent execution.
//!
//! These traits define the interfaces that agent implementations must satisfy.
//! They enable pluggable execution backends (local, cloud, swarm) while
//! maintaining a consistent API.
//!
//! # Trait Hierarchy
//!
//! ```text
//! AgentExecutor (core execution)
//!     │
//!     ├── AgentSession (stateful conversation)
//!     │
//!     └── AgentFactory (create executors)
//!
//! AgentRegistry (discovery)
//!     │
//!     └── AgentStore (persistence)
//! ```

use super::{AgentEvent, AgentId, AgentManifest, AgentState, AgentStats, ExecutionEnvironment};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::broadcast;

/// Errors that can occur during agent execution.
#[derive(Debug, Error)]
pub enum AgentError {
    #[error("agent not found: {0}")]
    NotFound(String),

    #[error("agent not available: {0}")]
    NotAvailable(String),

    #[error("execution failed: {0}")]
    ExecutionFailed(String),

    #[error("permission denied: {0}")]
    PermissionDenied(String),

    #[error("timeout after {0}ms")]
    Timeout(u64),

    #[error("payment required: {amount_millisats} millisats")]
    PaymentRequired { amount_millisats: u64, bolt11: String },

    #[error("invalid input: {0}")]
    InvalidInput(String),

    #[error("resource exhausted: {0}")]
    ResourceExhausted(String),

    #[error("cancelled")]
    Cancelled,

    #[error("internal error: {0}")]
    Internal(String),
}

/// Result type for agent operations.
pub type AgentResult<T> = Result<T, AgentError>;

/// A job request (NIP-90 compatible).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobRequest {
    /// Unique job ID.
    pub id: String,

    /// Job kind (NIP-90: 5000-5999).
    pub kind: u16,

    /// Input data.
    pub inputs: Vec<JobInput>,

    /// Job parameters.
    #[serde(default)]
    pub params: Vec<JobParam>,

    /// Customer's public key (npub hex).
    pub customer: String,

    /// Maximum bid in millisats.
    #[serde(default)]
    pub bid: Option<u64>,

    /// Expected output format (MIME type).
    #[serde(default)]
    pub output_format: Option<String>,

    /// Deadline (Unix timestamp).
    #[serde(default)]
    pub deadline: Option<u64>,
}

/// Job input.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobInput {
    /// Input data.
    pub data: String,

    /// Input type ("text", "url", "event", "job").
    pub input_type: String,

    /// Relay hint (for event/job types).
    #[serde(default)]
    pub relay: Option<String>,

    /// Marker for how input should be used.
    #[serde(default)]
    pub marker: Option<String>,
}

/// Job parameter.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobParam {
    /// Parameter key.
    pub key: String,

    /// Parameter value.
    pub value: String,
}

/// A job result (NIP-90 compatible).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobResult {
    /// Job ID.
    pub job_id: String,

    /// Result content.
    pub content: String,

    /// Success indicator.
    pub success: bool,

    /// Error message (if !success).
    #[serde(default)]
    pub error: Option<String>,

    /// Duration in milliseconds.
    #[serde(default)]
    pub duration_ms: Option<u64>,

    /// Cost in millisats.
    #[serde(default)]
    pub cost_millisats: Option<u64>,

    /// Additional result metadata.
    #[serde(default)]
    pub metadata: Value,
}

impl JobResult {
    /// Create a successful result.
    pub fn success(job_id: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            job_id: job_id.into(),
            content: content.into(),
            success: true,
            error: None,
            duration_ms: None,
            cost_millisats: None,
            metadata: Value::Null,
        }
    }

    /// Create a failed result.
    pub fn failure(job_id: impl Into<String>, error: impl Into<String>) -> Self {
        Self {
            job_id: job_id.into(),
            content: String::new(),
            success: false,
            error: Some(error.into()),
            duration_ms: None,
            cost_millisats: None,
            metadata: Value::Null,
        }
    }

    /// Set the duration.
    pub fn with_duration(mut self, duration_ms: u64) -> Self {
        self.duration_ms = Some(duration_ms);
        self
    }

    /// Set the cost.
    pub fn with_cost(mut self, cost_millisats: u64) -> Self {
        self.cost_millisats = Some(cost_millisats);
        self
    }
}

/// Core trait for executing agent jobs.
///
/// Implementations of this trait handle the actual execution of work,
/// whether locally, in the cloud, or on the swarm network.
#[async_trait]
pub trait AgentExecutor: Send + Sync {
    /// Execute a job request.
    ///
    /// This is the primary method for running agent work.
    async fn execute(&self, request: JobRequest) -> AgentResult<JobResult>;

    /// Check if this executor can handle a job kind.
    fn can_handle(&self, kind: u16) -> bool;

    /// Get current agent state.
    fn state(&self) -> AgentState;

    /// Get agent statistics.
    fn stats(&self) -> AgentStats;

    /// Subscribe to agent events.
    fn events(&self) -> broadcast::Receiver<AgentEvent>;

    /// Cancel a running job.
    async fn cancel(&self, job_id: &str) -> AgentResult<()>;

    /// Get the agent manifest.
    fn manifest(&self) -> &AgentManifest;

    /// Get the agent ID.
    fn id(&self) -> Option<&AgentId> {
        self.manifest().id.as_ref()
    }
}

/// A stateful agent session for multi-turn conversations.
///
/// Sessions maintain state across multiple interactions,
/// enabling context-aware conversations.
#[async_trait]
pub trait AgentSession: Send + Sync {
    /// Session ID.
    fn id(&self) -> &str;

    /// Send a message and get a response.
    async fn send(&self, message: &str) -> AgentResult<String>;

    /// Send a message with structured input.
    async fn send_structured(&self, input: Value) -> AgentResult<Value>;

    /// Get conversation history.
    fn history(&self) -> Vec<SessionMessage>;

    /// Clear conversation history.
    fn clear(&mut self);

    /// Get session state.
    fn state(&self) -> &SessionState;

    /// Close the session.
    async fn close(&mut self) -> AgentResult<()>;
}

/// A message in a session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMessage {
    /// Message role.
    pub role: MessageRole,

    /// Message content.
    pub content: String,

    /// Timestamp.
    pub timestamp: u64,

    /// Additional metadata.
    #[serde(default)]
    pub metadata: Value,
}

/// Message role.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageRole {
    /// User message.
    User,
    /// Agent/assistant message.
    Agent,
    /// System message.
    System,
    /// Tool call.
    Tool,
}

/// Session state.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SessionState {
    /// Number of messages.
    pub message_count: u32,

    /// Total tokens used.
    pub tokens_used: u64,

    /// Session created timestamp.
    pub created_at: u64,

    /// Last activity timestamp.
    pub last_activity: u64,

    /// Whether session is active.
    pub active: bool,
}

/// Factory for creating agent executors.
///
/// Factories abstract over different execution backends,
/// allowing the same manifest to be executed in different environments.
#[async_trait]
pub trait AgentFactory: Send + Sync {
    /// Create an executor from a manifest.
    async fn create(&self, manifest: AgentManifest) -> AgentResult<Arc<dyn AgentExecutor>>;

    /// Create a session from a manifest.
    async fn create_session(
        &self,
        manifest: AgentManifest,
    ) -> AgentResult<Box<dyn AgentSession>>;

    /// Get supported execution environments.
    fn supported_environments(&self) -> Vec<ExecutionEnvironment>;

    /// Check if this factory can handle a manifest.
    fn can_create(&self, manifest: &AgentManifest) -> bool {
        self.supported_environments()
            .iter()
            .any(|env| manifest.requirements.environment.compatible_with(env))
    }
}

/// Registry for discovering agents.
///
/// Registries provide agent discovery and lookup functionality.
#[async_trait]
pub trait AgentRegistry: Send + Sync {
    /// Find agents by capability.
    async fn find_by_capability(&self, kind: u16) -> AgentResult<Vec<AgentManifest>>;

    /// Find agents by skill.
    async fn find_by_skill(&self, skill: &str) -> AgentResult<Vec<AgentManifest>>;

    /// Find agents by tag.
    async fn find_by_tag(&self, tag: &str) -> AgentResult<Vec<AgentManifest>>;

    /// Get agent by ID.
    async fn get(&self, id: &AgentId) -> AgentResult<Option<AgentManifest>>;

    /// List all agents.
    async fn list(&self, limit: usize, offset: usize) -> AgentResult<Vec<AgentManifest>>;

    /// Register an agent.
    async fn register(&self, manifest: AgentManifest) -> AgentResult<()>;

    /// Unregister an agent.
    async fn unregister(&self, id: &AgentId) -> AgentResult<()>;
}

/// Store for persisting agent data.
#[async_trait]
pub trait AgentStore: Send + Sync {
    /// Save a manifest.
    async fn save_manifest(&self, manifest: &AgentManifest) -> AgentResult<()>;

    /// Load a manifest by ID.
    async fn load_manifest(&self, id: &AgentId) -> AgentResult<Option<AgentManifest>>;

    /// Delete a manifest.
    async fn delete_manifest(&self, id: &AgentId) -> AgentResult<()>;

    /// Save session state.
    async fn save_session(&self, session_id: &str, state: &SessionState) -> AgentResult<()>;

    /// Load session state.
    async fn load_session(&self, session_id: &str) -> AgentResult<Option<SessionState>>;

    /// Save agent stats.
    async fn save_stats(&self, id: &AgentId, stats: &AgentStats) -> AgentResult<()>;

    /// Load agent stats.
    async fn load_stats(&self, id: &AgentId) -> AgentResult<Option<AgentStats>>;
}

/// Permission handler for tool invocations.
#[async_trait]
pub trait PermissionHandler: Send + Sync {
    /// Check if a tool can be used.
    async fn can_use_tool(
        &self,
        tool_name: &str,
        input: &Value,
        job_id: &str,
    ) -> AgentResult<PermissionResult>;
}

/// Result of a permission check.
#[derive(Debug, Clone)]
pub enum PermissionResult {
    /// Permission granted.
    Allow,

    /// Permission granted with modified input.
    AllowModified { input: Value },

    /// Permission denied.
    Deny { reason: String },

    /// Ask user for permission.
    AskUser { prompt: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_job_result() {
        let success = JobResult::success("job123", "Hello, world!")
            .with_duration(1000)
            .with_cost(5000);

        assert!(success.success);
        assert_eq!(success.duration_ms, Some(1000));
        assert_eq!(success.cost_millisats, Some(5000));

        let failure = JobResult::failure("job456", "Something went wrong");
        assert!(!failure.success);
        assert_eq!(failure.error, Some("Something went wrong".to_string()));
    }

    #[test]
    fn test_job_request() {
        let request = JobRequest {
            id: "job123".into(),
            kind: 5050,
            inputs: vec![JobInput {
                data: "Hello, agent!".into(),
                input_type: "text".into(),
                relay: None,
                marker: None,
            }],
            params: vec![JobParam {
                key: "model".into(),
                value: "claude-3-5-sonnet".into(),
            }],
            customer: "npub1...".into(),
            bid: Some(10000),
            output_format: Some("text/plain".into()),
            deadline: None,
        };

        assert_eq!(request.kind, 5050);
        assert_eq!(request.inputs.len(), 1);
    }
}
