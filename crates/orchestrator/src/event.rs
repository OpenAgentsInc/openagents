//! Orchestrator events for observability

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// An event emitted by the orchestrator
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrchestratorEvent {
    /// Event ID
    pub id: String,
    /// Session ID
    pub session_id: String,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
    /// Event type
    pub event_type: EventType,
    /// Event data
    pub data: serde_json::Value,
}

/// Types of orchestrator events
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventType {
    // Session events
    SessionStarted,
    SessionPaused,
    SessionResumed,
    SessionCompleted,
    SessionFailed,

    // Task events
    TaskSelected,
    TaskStarted,
    TaskDecomposed,
    TaskCompleted,
    TaskFailed,
    TaskBlocked,
    TaskSkipped,

    // Subtask events
    SubtaskStarted,
    SubtaskCompleted,
    SubtaskFailed,

    // Tool events
    ToolCallStarted,
    ToolCallCompleted,
    ToolCallFailed,

    // LLM events
    LlmRequestStarted,
    LlmResponseReceived,
    LlmStreamChunk,

    // Verification events
    VerificationStarted,
    VerificationPassed,
    VerificationFailed,

    // Git events
    GitCommitCreated,
    GitBranchCreated,
    GitPushCompleted,

    // Error events
    ErrorOccurred,
    RetryAttempted,
    RecoveryStarted,
    RecoveryCompleted,

    // Metrics events
    MetricsSnapshot,
    TokenUsageUpdated,
}

impl OrchestratorEvent {
    /// Create a new event
    pub fn new(
        session_id: impl Into<String>,
        event_type: EventType,
        data: serde_json::Value,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            session_id: session_id.into(),
            timestamp: Utc::now(),
            event_type,
            data,
        }
    }

    /// Create a session started event
    pub fn session_started(session_id: &str) -> Self {
        Self::new(session_id, EventType::SessionStarted, serde_json::json!({}))
    }

    /// Create a session completed event
    pub fn session_completed(session_id: &str, tasks_completed: usize) -> Self {
        Self::new(
            session_id,
            EventType::SessionCompleted,
            serde_json::json!({ "tasks_completed": tasks_completed }),
        )
    }

    /// Create a task started event
    pub fn task_started(session_id: &str, task_id: &str, task_title: &str) -> Self {
        Self::new(
            session_id,
            EventType::TaskStarted,
            serde_json::json!({
                "task_id": task_id,
                "task_title": task_title
            }),
        )
    }

    /// Create a task completed event
    pub fn task_completed(session_id: &str, task_id: &str, commit_sha: Option<&str>) -> Self {
        Self::new(
            session_id,
            EventType::TaskCompleted,
            serde_json::json!({
                "task_id": task_id,
                "commit_sha": commit_sha
            }),
        )
    }

    /// Create a task failed event
    pub fn task_failed(session_id: &str, task_id: &str, error: &str) -> Self {
        Self::new(
            session_id,
            EventType::TaskFailed,
            serde_json::json!({
                "task_id": task_id,
                "error": error
            }),
        )
    }

    /// Create a tool call started event
    pub fn tool_call_started(session_id: &str, tool_name: &str, tool_id: &str) -> Self {
        Self::new(
            session_id,
            EventType::ToolCallStarted,
            serde_json::json!({
                "tool_name": tool_name,
                "tool_id": tool_id
            }),
        )
    }

    /// Create a tool call completed event
    pub fn tool_call_completed(
        session_id: &str,
        tool_name: &str,
        tool_id: &str,
        duration_ms: u64,
    ) -> Self {
        Self::new(
            session_id,
            EventType::ToolCallCompleted,
            serde_json::json!({
                "tool_name": tool_name,
                "tool_id": tool_id,
                "duration_ms": duration_ms
            }),
        )
    }

    /// Create an error event
    pub fn error_occurred(session_id: &str, error_type: &str, message: &str) -> Self {
        Self::new(
            session_id,
            EventType::ErrorOccurred,
            serde_json::json!({
                "error_type": error_type,
                "message": message
            }),
        )
    }

    /// Create a metrics snapshot event
    pub fn metrics_snapshot(session_id: &str, metrics: serde_json::Value) -> Self {
        Self::new(session_id, EventType::MetricsSnapshot, metrics)
    }
}

/// Event handler trait for processing orchestrator events
#[async_trait::async_trait]
pub trait EventHandler: Send + Sync {
    /// Handle an orchestrator event
    async fn handle(&self, event: OrchestratorEvent);
}

/// Simple event handler that logs events
pub struct LoggingEventHandler;

#[async_trait::async_trait]
impl EventHandler for LoggingEventHandler {
    async fn handle(&self, event: OrchestratorEvent) {
        tracing::info!(
            event_type = ?event.event_type,
            session_id = %event.session_id,
            "Orchestrator event: {:?}",
            event.data
        );
    }
}

/// Event handler that collects events in memory (useful for testing)
pub struct CollectingEventHandler {
    events: tokio::sync::Mutex<Vec<OrchestratorEvent>>,
}

impl CollectingEventHandler {
    pub fn new() -> Self {
        Self {
            events: tokio::sync::Mutex::new(Vec::new()),
        }
    }

    /// Get all collected events
    pub async fn events(&self) -> Vec<OrchestratorEvent> {
        self.events.lock().await.clone()
    }

    /// Get events of a specific type
    pub async fn events_of_type(&self, event_type: EventType) -> Vec<OrchestratorEvent> {
        self.events
            .lock()
            .await
            .iter()
            .filter(|e| e.event_type == event_type)
            .cloned()
            .collect()
    }

    /// Clear all events
    pub async fn clear(&self) {
        self.events.lock().await.clear();
    }
}

impl Default for CollectingEventHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl EventHandler for CollectingEventHandler {
    async fn handle(&self, event: OrchestratorEvent) {
        self.events.lock().await.push(event);
    }
}
