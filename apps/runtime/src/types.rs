use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
}

impl Default for RunStatus {
    fn default() -> Self {
        Self::Pending
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RunEvent {
    pub seq: u64,
    pub event_type: String,
    pub payload: Value,
    pub recorded_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RuntimeRun {
    pub id: Uuid,
    pub worker_id: Option<String>,
    pub status: RunStatus,
    pub metadata: Value,
    pub events: Vec<RunEvent>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StartRunRequest {
    pub worker_id: Option<String>,
    pub metadata: Value,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AppendRunEventRequest {
    pub event_type: String,
    pub payload: Value,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProjectionCheckpoint {
    pub topic: String,
    pub last_seq: u64,
    pub last_event_type: String,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WorkerStatus {
    Starting,
    Running,
    Stopping,
    Stopped,
    Failed,
}

impl WorkerStatus {
    #[must_use]
    pub fn as_event_label(&self) -> &'static str {
        match self {
            Self::Starting => "worker.starting",
            Self::Running => "worker.running",
            Self::Stopping => "worker.stopping",
            Self::Stopped => "worker.stopped",
            Self::Failed => "worker.failed",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkerOwner {
    pub user_id: Option<u64>,
    pub guest_scope: Option<String>,
}

impl WorkerOwner {
    #[must_use]
    pub fn is_valid(&self) -> bool {
        match (self.user_id, self.guest_scope.as_deref()) {
            (Some(_), Some(scope)) => scope.trim().is_empty(),
            (Some(_), None) => true,
            (None, Some(scope)) => !scope.trim().is_empty(),
            (None, None) => false,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RuntimeWorker {
    pub worker_id: String,
    pub owner: WorkerOwner,
    pub workspace_ref: Option<String>,
    pub codex_home_ref: Option<String>,
    pub adapter: String,
    pub status: WorkerStatus,
    pub latest_seq: u64,
    pub metadata: Value,
    pub started_at: DateTime<Utc>,
    pub stopped_at: Option<DateTime<Utc>>,
    pub last_heartbeat_at: Option<DateTime<Utc>>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkerLifecycleEvent {
    pub seq: u64,
    pub event_type: String,
    pub payload: Value,
    pub occurred_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RegisterWorkerRequest {
    pub worker_id: Option<String>,
    pub owner: WorkerOwner,
    pub workspace_ref: Option<String>,
    pub codex_home_ref: Option<String>,
    pub adapter: Option<String>,
    pub metadata: Value,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkerHeartbeatRequest {
    pub owner: WorkerOwner,
    pub metadata_patch: Value,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkerStatusTransitionRequest {
    pub owner: WorkerOwner,
    pub status: WorkerStatus,
    pub reason: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkerLiveness {
    pub heartbeat_age_ms: Option<i64>,
    pub heartbeat_stale_after_ms: i64,
    pub heartbeat_state: String,
}
