use std::{collections::HashMap, sync::Arc};

use async_trait::async_trait;
use chrono::Utc;
use thiserror::Error;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::types::{RunEvent, RunStatus, RuntimeRun, StartRunRequest};

#[derive(Debug, Error)]
pub enum AuthorityError {
    #[error("run not found: {0}")]
    RunNotFound(Uuid),
    #[error("run event sequence overflow for run {0}")]
    SequenceOverflow(Uuid),
}

#[async_trait]
pub trait RuntimeAuthority: Send + Sync {
    async fn create_run(&self, request: StartRunRequest) -> Result<RuntimeRun, AuthorityError>;
    async fn append_event(
        &self,
        run_id: Uuid,
        event_type: String,
        payload: serde_json::Value,
    ) -> Result<RunEvent, AuthorityError>;
    async fn get_run(&self, run_id: Uuid) -> Result<Option<RuntimeRun>, AuthorityError>;
    async fn update_run_status(
        &self,
        run_id: Uuid,
        status: RunStatus,
    ) -> Result<RuntimeRun, AuthorityError>;
}

#[derive(Default)]
pub struct InMemoryRuntimeAuthority {
    runs: RwLock<HashMap<Uuid, RuntimeRun>>,
}

impl InMemoryRuntimeAuthority {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
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
    ) -> Result<RunEvent, AuthorityError> {
        let mut runs = self.runs.write().await;
        let run = runs
            .get_mut(&run_id)
            .ok_or(AuthorityError::RunNotFound(run_id))?;
        let next_seq = run.events.last().map_or(Ok(1_u64), |event| {
            event
                .seq
                .checked_add(1)
                .ok_or(AuthorityError::SequenceOverflow(run_id))
        })?;
        let now = Utc::now();
        let event = RunEvent {
            seq: next_seq,
            event_type,
            payload,
            recorded_at: now,
        };

        run.events.push(event.clone());
        run.updated_at = now;
        Ok(event)
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

    use super::{InMemoryRuntimeAuthority, RuntimeAuthority};
    use crate::types::StartRunRequest;

    #[tokio::test]
    async fn create_and_append_event_assigns_monotonic_sequence() -> Result<()> {
        let authority = InMemoryRuntimeAuthority::new();
        let run = authority
            .create_run(StartRunRequest {
                worker_id: Some("desktop:worker-1".to_string()),
                metadata: json!({"source": "test"}),
            })
            .await?;

        let first = authority
            .append_event(run.id, "run.started".to_string(), json!({"ok": true}))
            .await?;
        let second = authority
            .append_event(run.id, "run.step.completed".to_string(), json!({"step": 1}))
            .await?;

        assert_eq!(first.seq, 1);
        assert_eq!(second.seq, 2);
        Ok(())
    }
}
