use std::{
    collections::HashMap,
    fs::{File, OpenOptions, create_dir_all, rename},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use async_trait::async_trait;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use tokio::sync::{Mutex, RwLock};
use uuid::Uuid;

use crate::types::{
    ProjectionCheckpoint, ProjectionDriftReport, RunEvent, RunProjectionSummary,
    WorkerLifecycleEvent, WorkerProjectionSummary,
};

#[derive(Debug, Error)]
pub enum ProjectorError {
    #[error("projector topic is missing for run {0}")]
    MissingTopic(Uuid),
    #[error("projector persistence error: {0}")]
    Persistence(String),
    #[error("projector serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
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
    async fn drift_for_topic(
        &self,
        topic: &str,
    ) -> Result<Option<ProjectionDriftReport>, ProjectorError>;
    async fn run_summary(
        &self,
        run_id: Uuid,
    ) -> Result<Option<RunProjectionSummary>, ProjectorError>;
    async fn recover_run_projection(
        &self,
        run_id: Uuid,
        events: &[RunEvent],
    ) -> Result<(), ProjectorError>;
    fn is_ready(&self) -> bool;
}

pub struct InMemoryProjectionPipeline {
    checkpoints: RwLock<HashMap<String, ProjectionCheckpoint>>,
    run_summaries: RwLock<HashMap<String, RunProjectionSummary>>,
    worker_summaries: RwLock<HashMap<String, WorkerProjectionSummary>>,
    drift_reports: RwLock<HashMap<String, ProjectionDriftReport>>,
    persistence_path: Option<PathBuf>,
    persistence_guard: Mutex<()>,
    bootstrapped: AtomicBool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct PersistedProjectionState {
    checkpoints: HashMap<String, ProjectionCheckpoint>,
    run_summaries: HashMap<String, RunProjectionSummary>,
    worker_summaries: HashMap<String, WorkerProjectionSummary>,
    drift_reports: HashMap<String, ProjectionDriftReport>,
}

impl InMemoryProjectionPipeline {
    #[must_use]
    pub fn new() -> Self {
        Self {
            checkpoints: RwLock::new(HashMap::new()),
            run_summaries: RwLock::new(HashMap::new()),
            worker_summaries: RwLock::new(HashMap::new()),
            drift_reports: RwLock::new(HashMap::new()),
            persistence_path: None,
            persistence_guard: Mutex::new(()),
            bootstrapped: AtomicBool::new(true),
        }
    }

    pub fn open(path: impl AsRef<Path>) -> Result<Self, ProjectorError> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path.parent() {
            create_dir_all(parent)
                .map_err(|error| ProjectorError::Persistence(error.to_string()))?;
        }

        let loaded = load_persisted_state(&path)?;
        Ok(Self {
            checkpoints: RwLock::new(loaded.checkpoints),
            run_summaries: RwLock::new(loaded.run_summaries),
            worker_summaries: RwLock::new(loaded.worker_summaries),
            drift_reports: RwLock::new(loaded.drift_reports),
            persistence_path: Some(path),
            persistence_guard: Mutex::new(()),
            bootstrapped: AtomicBool::new(true),
        })
    }

    pub fn open_default() -> Result<Self, ProjectorError> {
        let configured = std::env::var("RUNTIME_CHECKPOINT_PATH")
            .unwrap_or_else(|_| ".runtime-data/projection-state.json".to_string());
        Self::open(configured)
    }

    #[must_use]
    pub fn shared() -> Arc<Self> {
        Arc::new(Self::new())
    }

    #[must_use]
    pub fn shared_from_env() -> Arc<Self> {
        match Self::open_default() {
            Ok(projector) => Arc::new(projector),
            Err(_error) => Arc::new(Self::new()),
        }
    }

    async fn persist(&self) -> Result<(), ProjectorError> {
        let Some(path) = &self.persistence_path else {
            return Ok(());
        };

        let checkpoints = self.checkpoints.read().await.clone();
        let run_summaries = self.run_summaries.read().await.clone();
        let worker_summaries = self.worker_summaries.read().await.clone();
        let drift_reports = self.drift_reports.read().await.clone();
        let serialized = serde_json::to_vec_pretty(&PersistedProjectionState {
            checkpoints,
            run_summaries,
            worker_summaries,
            drift_reports,
        })?;

        let _guard = self.persistence_guard.lock().await;
        let tmp_path = path.with_extension("json.tmp");
        let mut file = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&tmp_path)
            .map_err(|error| ProjectorError::Persistence(error.to_string()))?;
        file.write_all(&serialized)
            .map_err(|error| ProjectorError::Persistence(error.to_string()))?;
        file.sync_data()
            .map_err(|error| ProjectorError::Persistence(error.to_string()))?;
        rename(&tmp_path, path).map_err(|error| ProjectorError::Persistence(error.to_string()))?;
        Ok(())
    }
}

#[async_trait]
impl ProjectionPipeline for InMemoryProjectionPipeline {
    async fn apply_run_event(&self, run_id: Uuid, event: &RunEvent) -> Result<(), ProjectorError> {
        let topic = format!("run:{run_id}:events");
        let apply_result = apply_checkpoint(
            &topic,
            event.seq,
            &event.event_type,
            &self.checkpoints,
            &self.drift_reports,
        )
        .await?;
        if !apply_result.applied {
            return Ok(());
        }

        let run_id_key = run_id.to_string();
        let mut summaries = self.run_summaries.write().await;
        let summary = summaries
            .entry(run_id_key.clone())
            .or_insert_with(|| RunProjectionSummary {
                run_id: run_id_key.clone(),
                status: "created".to_string(),
                last_seq: 0,
                event_count: 0,
                projection_hash: String::new(),
                updated_at: Utc::now(),
            });
        summary.last_seq = event.seq;
        summary.event_count = summary.event_count.saturating_add(1);
        summary.status = project_run_status(summary.status.clone(), event);
        summary.updated_at = Utc::now();
        summary.projection_hash = hash_projection_state(&serde_json::json!({
            "run_id": summary.run_id,
            "status": summary.status,
            "last_seq": summary.last_seq,
            "event_count": summary.event_count,
        }))?;
        drop(summaries);

        self.persist().await
    }

    async fn apply_worker_event(
        &self,
        worker_id: &str,
        event: &WorkerLifecycleEvent,
    ) -> Result<(), ProjectorError> {
        let topic = format!("worker:{worker_id}:lifecycle");
        let apply_result = apply_checkpoint(
            &topic,
            event.seq,
            &event.event_type,
            &self.checkpoints,
            &self.drift_reports,
        )
        .await?;
        if !apply_result.applied {
            return Ok(());
        }

        let mut summaries = self.worker_summaries.write().await;
        let summary =
            summaries
                .entry(worker_id.to_string())
                .or_insert_with(|| WorkerProjectionSummary {
                    worker_id: worker_id.to_string(),
                    status: "starting".to_string(),
                    last_seq: 0,
                    event_count: 0,
                    projection_hash: String::new(),
                    updated_at: Utc::now(),
                });
        summary.last_seq = event.seq;
        summary.event_count = summary.event_count.saturating_add(1);
        summary.status = project_worker_status(summary.status.clone(), event);
        summary.updated_at = Utc::now();
        summary.projection_hash = hash_projection_state(&serde_json::json!({
            "worker_id": summary.worker_id,
            "status": summary.status,
            "last_seq": summary.last_seq,
            "event_count": summary.event_count,
        }))?;
        drop(summaries);

        self.persist().await
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

    async fn drift_for_topic(
        &self,
        topic: &str,
    ) -> Result<Option<ProjectionDriftReport>, ProjectorError> {
        let reports = self.drift_reports.read().await;
        Ok(reports.get(topic).cloned())
    }

    async fn run_summary(
        &self,
        run_id: Uuid,
    ) -> Result<Option<RunProjectionSummary>, ProjectorError> {
        let summaries = self.run_summaries.read().await;
        Ok(summaries.get(&run_id.to_string()).cloned())
    }

    async fn recover_run_projection(
        &self,
        run_id: Uuid,
        events: &[RunEvent],
    ) -> Result<(), ProjectorError> {
        let mut sorted = events.to_vec();
        sorted.sort_by_key(|event| event.seq);
        for event in &sorted {
            self.apply_run_event(run_id, event).await?;
        }
        Ok(())
    }

    fn is_ready(&self) -> bool {
        self.bootstrapped.load(Ordering::SeqCst)
    }
}

struct ApplyCheckpointResult {
    applied: bool,
}

async fn apply_checkpoint(
    topic: &str,
    incoming_seq: u64,
    event_type: &str,
    checkpoints: &RwLock<HashMap<String, ProjectionCheckpoint>>,
    drift_reports: &RwLock<HashMap<String, ProjectionDriftReport>>,
) -> Result<ApplyCheckpointResult, ProjectorError> {
    let mut checkpoints_guard = checkpoints.write().await;
    let existing = checkpoints_guard.get(topic).cloned();
    if let Some(current) = existing.as_ref() {
        if incoming_seq <= current.last_seq {
            return Ok(ApplyCheckpointResult { applied: false });
        }

        let expected_next_seq = current.last_seq.saturating_add(1);
        if incoming_seq > expected_next_seq {
            let mut drift_guard = drift_reports.write().await;
            drift_guard.insert(
                topic.to_string(),
                ProjectionDriftReport {
                    topic: topic.to_string(),
                    expected_next_seq,
                    last_seen_seq: current.last_seq,
                    incoming_seq,
                    reason: "sequence_gap".to_string(),
                    detected_at: Utc::now(),
                },
            );
        }
    } else if incoming_seq > 1 {
        let mut drift_guard = drift_reports.write().await;
        drift_guard.insert(
            topic.to_string(),
            ProjectionDriftReport {
                topic: topic.to_string(),
                expected_next_seq: 1,
                last_seen_seq: 0,
                incoming_seq,
                reason: "sequence_gap".to_string(),
                detected_at: Utc::now(),
            },
        );
    }

    checkpoints_guard.insert(
        topic.to_string(),
        ProjectionCheckpoint {
            topic: topic.to_string(),
            last_seq: incoming_seq,
            last_event_type: event_type.to_string(),
            updated_at: Utc::now(),
        },
    );
    Ok(ApplyCheckpointResult { applied: true })
}

fn project_run_status(current: String, event: &RunEvent) -> String {
    match event.event_type.as_str() {
        "run.started" => "running".to_string(),
        "run.cancel_requested" => "canceling".to_string(),
        "run.finished" => event
            .payload
            .get("status")
            .and_then(serde_json::Value::as_str)
            .unwrap_or(&current)
            .to_string(),
        _ => current,
    }
}

fn project_worker_status(current: String, event: &WorkerLifecycleEvent) -> String {
    event
        .payload
        .get("status")
        .and_then(serde_json::Value::as_str)
        .map(std::borrow::ToOwned::to_owned)
        .or_else(|| {
            event
                .event_type
                .strip_prefix("worker.")
                .map(std::borrow::ToOwned::to_owned)
        })
        .unwrap_or(current)
}

fn hash_projection_state(value: &serde_json::Value) -> Result<String, ProjectorError> {
    let canonical = serde_json::to_string(value)?;
    let digest = Sha256::digest(canonical.as_bytes());
    Ok(format!("sha256:{}", hex::encode(digest)))
}

fn load_persisted_state(path: &Path) -> Result<PersistedProjectionState, ProjectorError> {
    if !path.exists() {
        return Ok(PersistedProjectionState {
            checkpoints: HashMap::new(),
            run_summaries: HashMap::new(),
            worker_summaries: HashMap::new(),
            drift_reports: HashMap::new(),
        });
    }

    let mut file =
        File::open(path).map_err(|error| ProjectorError::Persistence(error.to_string()))?;
    let mut buffer = String::new();
    file.read_to_string(&mut buffer)
        .map_err(|error| ProjectorError::Persistence(error.to_string()))?;
    if buffer.trim().is_empty() {
        return Ok(PersistedProjectionState {
            checkpoints: HashMap::new(),
            run_summaries: HashMap::new(),
            worker_summaries: HashMap::new(),
            drift_reports: HashMap::new(),
        });
    }
    serde_json::from_str(&buffer).map_err(ProjectorError::from)
}

#[cfg(test)]
mod tests {
    use anyhow::{Result, anyhow};
    use chrono::Utc;
    use serde_json::json;
    use tempfile::tempdir;
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
                    idempotency_key: None,
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
                    idempotency_key: None,
                    recorded_at: Utc::now(),
                },
            )
            .await?;

        let checkpoint = projector
            .checkpoint_for_run(run_id)
            .await?
            .ok_or_else(|| anyhow!("checkpoint missing"))?;
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
            .ok_or_else(|| anyhow!("worker checkpoint missing"))?;
        assert_eq!(checkpoint.last_seq, 2);
        assert_eq!(checkpoint.last_event_type, "worker.heartbeat");
        Ok(())
    }

    #[tokio::test]
    async fn projector_persists_and_recovers_checkpoint_state() -> Result<()> {
        let dir = tempdir()?;
        let checkpoint_path = dir.path().join("projection-state.json");
        let run_id = Uuid::now_v7();
        {
            let projector = InMemoryProjectionPipeline::open(&checkpoint_path)?;
            projector
                .apply_run_event(
                    run_id,
                    &RunEvent {
                        seq: 1,
                        event_type: "run.started".to_string(),
                        payload: json!({"source": "runtime"}),
                        idempotency_key: None,
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
                        idempotency_key: None,
                        recorded_at: Utc::now(),
                    },
                )
                .await?;
        }
        let recovered = InMemoryProjectionPipeline::open(&checkpoint_path)?;
        let checkpoint = recovered
            .checkpoint_for_run(run_id)
            .await?
            .ok_or_else(|| anyhow!("checkpoint missing after recovery"))?;
        assert_eq!(checkpoint.last_seq, 2);
        Ok(())
    }

    #[tokio::test]
    async fn recover_run_projection_is_idempotent() -> Result<()> {
        let run_id = Uuid::now_v7();
        let projector = InMemoryProjectionPipeline::new();
        let events = vec![
            RunEvent {
                seq: 1,
                event_type: "run.started".to_string(),
                payload: json!({}),
                idempotency_key: None,
                recorded_at: Utc::now(),
            },
            RunEvent {
                seq: 2,
                event_type: "run.step.completed".to_string(),
                payload: json!({"step": 1}),
                idempotency_key: None,
                recorded_at: Utc::now(),
            },
        ];

        projector.recover_run_projection(run_id, &events).await?;
        projector.recover_run_projection(run_id, &events).await?;
        let summary = projector
            .run_summary(run_id)
            .await?
            .ok_or_else(|| anyhow!("missing run summary"))?;
        assert_eq!(summary.event_count, 2);
        assert_eq!(summary.last_seq, 2);
        Ok(())
    }

    #[tokio::test]
    async fn sequence_gap_records_drift_report() -> Result<()> {
        let run_id = Uuid::now_v7();
        let projector = InMemoryProjectionPipeline::new();
        projector
            .apply_run_event(
                run_id,
                &RunEvent {
                    seq: 3,
                    event_type: "run.step.completed".to_string(),
                    payload: json!({"step": 3}),
                    idempotency_key: None,
                    recorded_at: Utc::now(),
                },
            )
            .await?;
        let topic = format!("run:{run_id}:events");
        let drift = projector
            .drift_for_topic(&topic)
            .await?
            .ok_or_else(|| anyhow!("expected drift report for sequence gap"))?;
        assert_eq!(drift.reason, "sequence_gap");
        assert_eq!(drift.incoming_seq, 3);
        Ok(())
    }
}
