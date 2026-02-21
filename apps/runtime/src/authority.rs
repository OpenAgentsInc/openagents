use std::{collections::HashMap, sync::Arc};

use async_trait::async_trait;
use chrono::Utc;
use thiserror::Error;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::{
    event_log::{DurableEventLog, EventLogAppendRequest, EventLogError},
    types::{RunEvent, RunStatus, RuntimeRun, StartRunRequest},
};

#[derive(Debug, Error)]
pub enum AuthorityError {
    #[error("run not found: {0}")]
    RunNotFound(Uuid),
    #[error(
        "run sequence conflict for run {run_id}: expected previous seq {expected_previous_seq}, actual previous seq {actual_previous_seq}"
    )]
    SequenceConflict {
        run_id: Uuid,
        expected_previous_seq: u64,
        actual_previous_seq: u64,
    },
    #[error("durable event log error: {0}")]
    EventLog(String),
}

#[derive(Clone, Debug)]
pub struct AppendEventOutcome {
    pub event: RunEvent,
    pub idempotent_replay: bool,
}

#[async_trait]
pub trait RuntimeAuthority: Send + Sync {
    async fn create_run(&self, request: StartRunRequest) -> Result<RuntimeRun, AuthorityError>;
    async fn append_event(
        &self,
        run_id: Uuid,
        event_type: String,
        payload: serde_json::Value,
        idempotency_key: Option<String>,
        expected_previous_seq: Option<u64>,
    ) -> Result<AppendEventOutcome, AuthorityError>;
    async fn get_run(&self, run_id: Uuid) -> Result<Option<RuntimeRun>, AuthorityError>;
    async fn update_run_status(
        &self,
        run_id: Uuid,
        status: RunStatus,
    ) -> Result<RuntimeRun, AuthorityError>;
}

pub struct InMemoryRuntimeAuthority {
    runs: RwLock<HashMap<Uuid, RuntimeRun>>,
    event_log: DurableEventLog,
}

impl InMemoryRuntimeAuthority {
    #[must_use]
    pub fn new() -> Self {
        let event_log = match DurableEventLog::open_default() {
            Ok(log) => log,
            Err(_error) => DurableEventLog::new_memory(),
        };
        Self::with_event_log(event_log)
    }

    #[must_use]
    pub fn with_event_log(event_log: DurableEventLog) -> Self {
        Self {
            runs: RwLock::new(HashMap::new()),
            event_log,
        }
    }

    #[must_use]
    pub fn shared() -> Arc<Self> {
        Arc::new(Self::new())
    }
}

#[async_trait]
impl RuntimeAuthority for InMemoryRuntimeAuthority {
    async fn create_run(&self, request: StartRunRequest) -> Result<RuntimeRun, AuthorityError> {
        let now = Utc::now();
        let run = RuntimeRun {
            id: Uuid::now_v7(),
            worker_id: request.worker_id,
            status: RunStatus::Created,
            metadata: request.metadata,
            events: Vec::new(),
            created_at: now,
            updated_at: now,
        };

        let mut runs = self.runs.write().await;
        runs.insert(run.id, run.clone());
        Ok(run)
    }

    async fn append_event(
        &self,
        run_id: Uuid,
        event_type: String,
        payload: serde_json::Value,
        idempotency_key: Option<String>,
        expected_previous_seq: Option<u64>,
    ) -> Result<AppendEventOutcome, AuthorityError> {
        if self.get_run(run_id).await?.is_none() {
            return Err(AuthorityError::RunNotFound(run_id));
        }
        let log_outcome = self
            .event_log
            .append(EventLogAppendRequest {
                run_id,
                event_type,
                payload,
                idempotency_key,
                expected_previous_seq,
            })
            .await
            .map_err(map_event_log_error)?;

        let events = self.event_log.events_for_run(run_id).await;
        let mut runs = self.runs.write().await;
        let run = runs
            .get_mut(&run_id)
            .ok_or(AuthorityError::RunNotFound(run_id))?;
        run.events = events;
        run.updated_at = Utc::now();
        Ok(AppendEventOutcome {
            event: log_outcome.event,
            idempotent_replay: log_outcome.idempotent_replay,
        })
    }

    async fn get_run(&self, run_id: Uuid) -> Result<Option<RuntimeRun>, AuthorityError> {
        let runs = self.runs.read().await;
        Ok(runs.get(&run_id).cloned())
    }

    async fn update_run_status(
        &self,
        run_id: Uuid,
        status: RunStatus,
    ) -> Result<RuntimeRun, AuthorityError> {
        let mut runs = self.runs.write().await;
        let run = runs
            .get_mut(&run_id)
            .ok_or(AuthorityError::RunNotFound(run_id))?;
        run.status = status;
        run.updated_at = Utc::now();
        Ok(run.clone())
    }
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use serde_json::json;
    use tempfile::tempdir;

    use super::{AuthorityError, InMemoryRuntimeAuthority, RuntimeAuthority};
    use crate::event_log::DurableEventLog;
    use crate::types::StartRunRequest;

    #[tokio::test]
    async fn create_and_append_event_assigns_monotonic_sequence() -> Result<()> {
        let dir = tempdir()?;
        let authority = InMemoryRuntimeAuthority::with_event_log(DurableEventLog::open(
            dir.path().join("events.jsonl"),
        )?);
        let run = authority
            .create_run(StartRunRequest {
                worker_id: Some("desktop:worker-1".to_string()),
                metadata: json!({"source": "test"}),
            })
            .await?;

        let first = authority
            .append_event(
                run.id,
                "run.started".to_string(),
                json!({"ok": true}),
                Some("run-started".to_string()),
                Some(0),
            )
            .await?;
        let second = authority
            .append_event(
                run.id,
                "run.step.completed".to_string(),
                json!({"step": 1}),
                Some("step-1".to_string()),
                Some(1),
            )
            .await?;

        assert_eq!(first.event.seq, 1);
        assert_eq!(second.event.seq, 2);
        Ok(())
    }

    #[tokio::test]
    async fn append_event_returns_idempotent_replay_for_duplicate_key() -> Result<()> {
        let dir = tempdir()?;
        let authority = InMemoryRuntimeAuthority::with_event_log(DurableEventLog::open(
            dir.path().join("events.jsonl"),
        )?);
        let run = authority
            .create_run(StartRunRequest {
                worker_id: Some("desktop:worker-2".to_string()),
                metadata: json!({"source": "test"}),
            })
            .await?;

        let first = authority
            .append_event(
                run.id,
                "run.step.completed".to_string(),
                json!({"step": 1}),
                Some("dup-key".to_string()),
                Some(0),
            )
            .await?;
        let second = authority
            .append_event(
                run.id,
                "run.step.completed".to_string(),
                json!({"step": 1}),
                Some("dup-key".to_string()),
                Some(0),
            )
            .await?;

        assert_eq!(first.event.seq, 1);
        assert!(!first.idempotent_replay);
        assert_eq!(second.event.seq, 1);
        assert!(second.idempotent_replay);
        Ok(())
    }

    #[tokio::test]
    async fn append_event_rejects_sequence_conflicts() -> Result<()> {
        let dir = tempdir()?;
        let authority = InMemoryRuntimeAuthority::with_event_log(DurableEventLog::open(
            dir.path().join("events.jsonl"),
        )?);
        let run = authority
            .create_run(StartRunRequest {
                worker_id: Some("desktop:worker-3".to_string()),
                metadata: json!({"source": "test"}),
            })
            .await?;
        let _ = authority
            .append_event(
                run.id,
                "run.started".to_string(),
                json!({}),
                Some("first".to_string()),
                Some(0),
            )
            .await?;

        let conflict = authority
            .append_event(
                run.id,
                "run.step.completed".to_string(),
                json!({"step": 2}),
                Some("step-two".to_string()),
                Some(0),
            )
            .await;
        if !matches!(
            conflict,
            Err(AuthorityError::SequenceConflict {
                expected_previous_seq: 0,
                actual_previous_seq: 1,
                ..
            })
        ) {
            return Err(anyhow::anyhow!("expected sequence conflict"));
        }
        Ok(())
    }
}

fn map_event_log_error(error: EventLogError) -> AuthorityError {
    match error {
        EventLogError::SequenceConflict {
            run_id,
            expected_previous_seq,
            actual_previous_seq,
        } => AuthorityError::SequenceConflict {
            run_id,
            expected_previous_seq,
            actual_previous_seq,
        },
        other => AuthorityError::EventLog(other.to_string()),
    }
}
