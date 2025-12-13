//! Core traits for agent execution.

use super::AgentManifest;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use thiserror::Error;

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
    PaymentRequired {
        amount_millisats: u64,
        bolt11: String,
    },
    #[error("invalid input: {0}")]
    InvalidInput(String),
    #[error("resource exhausted: {0}")]
    ResourceExhausted(String),
    #[error("cancelled")]
    Cancelled,
    #[error("internal error: {0}")]
    Internal(String),
}

pub type AgentResult<T> = Result<T, AgentError>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobRequest {
    pub id: String,
    pub kind: u16,
    pub inputs: Vec<JobInput>,
    #[serde(default)]
    pub params: Vec<JobParam>,
    pub customer: String,
    #[serde(default)]
    pub bid: Option<u64>,
    #[serde(default)]
    pub output_format: Option<String>,
    #[serde(default)]
    pub deadline: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobInput {
    pub data: String,
    pub input_type: String,
    #[serde(default)]
    pub relay: Option<String>,
    #[serde(default)]
    pub marker: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobParam {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobResult {
    pub job_id: String,
    pub content: String,
    pub success: bool,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub duration_ms: Option<u64>,
    #[serde(default)]
    pub cost_millisats: Option<u64>,
    #[serde(default)]
    pub metadata: Value,
}

impl JobResult {
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

    pub fn with_duration(mut self, duration_ms: u64) -> Self {
        self.duration_ms = Some(duration_ms);
        self
    }

    pub fn with_cost(mut self, cost_millisats: u64) -> Self {
        self.cost_millisats = Some(cost_millisats);
        self
    }
}

#[async_trait]
pub trait AgentExecutor: Send + Sync {
    async fn execute(&self, request: JobRequest) -> AgentResult<JobResult>;
    fn can_handle(&self, kind: u16) -> bool;
    fn manifest(&self) -> &AgentManifest;
}

#[async_trait]
pub trait AgentFactory: Send + Sync {
    async fn create(&self, manifest: AgentManifest) -> AgentResult<Arc<dyn AgentExecutor>>;
}

#[async_trait]
pub trait AgentRegistry: Send + Sync {
    async fn find_by_capability(&self, kind: u16) -> AgentResult<Vec<AgentManifest>>;
    async fn get(&self, name: &str) -> AgentResult<Option<AgentManifest>>;
    async fn register(&self, manifest: AgentManifest) -> AgentResult<()>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMessage {
    pub role: MessageRole,
    pub content: String,
    pub timestamp: u64,
    #[serde(default)]
    pub metadata: Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageRole {
    User,
    Agent,
    System,
    Tool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SessionState {
    pub message_count: u32,
    pub tokens_used: u64,
    pub created_at: u64,
    pub last_activity: u64,
    pub active: bool,
}

#[async_trait]
pub trait AgentSession: Send + Sync {
    fn id(&self) -> &str;
    async fn send(&self, message: &str) -> AgentResult<String>;
    fn history(&self) -> Vec<SessionMessage>;
    fn state(&self) -> &SessionState;
}

#[async_trait]
pub trait AgentStore: Send + Sync {
    async fn save_manifest(&self, manifest: &AgentManifest) -> AgentResult<()>;
    async fn load_manifest(&self, name: &str) -> AgentResult<Option<AgentManifest>>;
}

#[async_trait]
pub trait PermissionHandler: Send + Sync {
    async fn can_use_tool(
        &self,
        tool_name: &str,
        input: &Value,
        job_id: &str,
    ) -> AgentResult<PermissionResult>;
}

#[derive(Debug, Clone)]
pub enum PermissionResult {
    Allow,
    AllowModified { input: Value },
    Deny { reason: String },
    AskUser { prompt: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_job_result() {
        let success = JobResult::success("job123", "Hello!").with_duration(1000);
        assert!(success.success);
        assert_eq!(success.duration_ms, Some(1000));

        let failure = JobResult::failure("job456", "Error");
        assert!(!failure.success);
    }
}
