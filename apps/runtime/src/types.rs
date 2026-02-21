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
