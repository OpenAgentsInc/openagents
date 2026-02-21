use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use async_trait::async_trait;
use chrono::Utc;
use thiserror::Error;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::types::{ProjectionCheckpoint, RunEvent, WorkerLifecycleEvent};

#[derive(Debug, Error)]
pub enum ProjectorError {
    #[error("projector topic is missing for run {0}")]
    MissingTopic(Uuid),
}

#[async_trait]
pub trait ProjectionPipeline: Send + Sync {
    async fn apply_run_event(&self, run_id: Uuid, event: &RunEvent) -> Result<(), ProjectorError>;
    async fn apply_worker_event(
        &self,
        worker_id: &str,
        event: &WorkerLifecycleEvent,
    ) -> Result<(), ProjectorError>;
    async fn checkpoint_for_run(
        &self,
        run_id: Uuid,
    ) -> Result<Option<ProjectionCheckpoint>, ProjectorError>;
    async fn checkpoint_for_worker(
        &self,
        worker_id: &str,
    ) -> Result<Option<ProjectionCheckpoint>, ProjectorError>;
    fn is_ready(&self) -> bool;
}

#[derive(Default)]
pub struct InMemoryProjectionPipeline {
    checkpoints: RwLock<HashMap<String, ProjectionCheckpoint>>,
    bootstrapped: AtomicBool,
}

impl InMemoryProjectionPipeline {
    #[must_use]
    pub fn new() -> Self {
        Self {
            checkpoints: RwLock::new(HashMap::new()),
            bootstrapped: AtomicBool::new(true),
        }
    }

    #[must_use]
    pub fn shared() -> Arc<Self> {
        Arc::new(Self::new())
    }
}

#[async_trait]
impl ProjectionPipeline for InMemoryProjectionPipeline {
    async fn apply_run_event(&self, run_id: Uuid, event: &RunEvent) -> Result<(), ProjectorError> {
        let topic = format!("run:{run_id}:events");
        let checkpoint = ProjectionCheckpoint {
            topic: topic.clone(),
            last_seq: event.seq,
            last_event_type: event.event_type.clone(),
            updated_at: Utc::now(),
        };
        let mut checkpoints = self.checkpoints.write().await;
        checkpoints.insert(topic, checkpoint);
        Ok(())
    }

    async fn apply_worker_event(
        &self,
        worker_id: &str,
        event: &WorkerLifecycleEvent,
    ) -> Result<(), ProjectorError> {
        let topic = format!("worker:{worker_id}:lifecycle");
        let checkpoint = ProjectionCheckpoint {
            topic: topic.clone(),
            last_seq: event.seq,
            last_event_type: event.event_type.clone(),
            updated_at: Utc::now(),
        };
        let mut checkpoints = self.checkpoints.write().await;
        checkpoints.insert(topic, checkpoint);
        Ok(())
    }

    async fn checkpoint_for_run(
        &self,
        run_id: Uuid,
    ) -> Result<Option<ProjectionCheckpoint>, ProjectorError> {
        let topic = format!("run:{run_id}:events");
        let checkpoints = self.checkpoints.read().await;
        Ok(checkpoints.get(&topic).cloned())
    }

    async fn checkpoint_for_worker(
        &self,
        worker_id: &str,
    ) -> Result<Option<ProjectionCheckpoint>, ProjectorError> {
        let topic = format!("worker:{worker_id}:lifecycle");
        let checkpoints = self.checkpoints.read().await;
        Ok(checkpoints.get(&topic).cloned())
    }

    fn is_ready(&self) -> bool {
        self.bootstrapped.load(Ordering::SeqCst)
    }
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use chrono::Utc;
    use serde_json::json;
    use uuid::Uuid;

    use super::{InMemoryProjectionPipeline, ProjectionPipeline};
    use crate::types::{RunEvent, WorkerLifecycleEvent};

    #[tokio::test]
    async fn checkpoint_tracks_latest_sequence() -> Result<()> {
        let run_id = Uuid::now_v7();
        let projector = InMemoryProjectionPipeline::new();
        projector
            .apply_run_event(
                run_id,
                &RunEvent {
                    seq: 1,
                    event_type: "run.started".to_string(),
                    payload: json!({"ok": true}),
                    recorded_at: Utc::now(),
                },
            )
            .await?;
        projector
            .apply_run_event(
                run_id,
                &RunEvent {
                    seq: 2,
                    event_type: "run.step.completed".to_string(),
                    payload: json!({"step": 1}),
                    recorded_at: Utc::now(),
                },
            )
            .await?;

        let checkpoint = projector
            .checkpoint_for_run(run_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("checkpoint missing"))?;
        assert_eq!(checkpoint.last_seq, 2);
        assert_eq!(checkpoint.last_event_type, "run.step.completed");
        Ok(())
    }

    #[tokio::test]
    async fn worker_checkpoint_tracks_latest_sequence() -> Result<()> {
        let projector = InMemoryProjectionPipeline::new();
        projector
            .apply_worker_event(
                "desktop:worker-1",
                &WorkerLifecycleEvent {
                    seq: 1,
                    event_type: "worker.started".to_string(),
                    payload: json!({"status": "running"}),
                    occurred_at: Utc::now(),
                },
            )
            .await?;
        projector
            .apply_worker_event(
                "desktop:worker-1",
                &WorkerLifecycleEvent {
                    seq: 2,
                    event_type: "worker.heartbeat".to_string(),
                    payload: json!({"status": "running"}),
                    occurred_at: Utc::now(),
                },
            )
            .await?;

        let checkpoint = projector
            .checkpoint_for_worker("desktop:worker-1")
            .await?
            .ok_or_else(|| anyhow::anyhow!("worker checkpoint missing"))?;
        assert_eq!(checkpoint.last_seq, 2);
        assert_eq!(checkpoint.last_event_type, "worker.heartbeat");
        Ok(())
    }
}
