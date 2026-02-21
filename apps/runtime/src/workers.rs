use std::{collections::HashMap, sync::Arc};

use chrono::{DateTime, Utc};
use thiserror::Error;
use tokio::sync::RwLock;

use crate::{
    projectors::{ProjectionPipeline, ProjectorError},
    types::{
        ProjectionCheckpoint, RegisterWorkerRequest, RuntimeWorker, WorkerHeartbeatRequest,
        WorkerLifecycleEvent, WorkerLiveness, WorkerOwner, WorkerStatus,
        WorkerStatusTransitionRequest,
    },
};

#[derive(Debug, Error)]
pub enum WorkerError {
    #[error("worker owner is invalid")]
    InvalidOwner,
    #[error("worker not found: {0}")]
    NotFound(String),
    #[error("worker owner does not match: {0}")]
    Forbidden(String),
    #[error("worker status transition is not allowed: {from:?} -> {to:?}")]
    InvalidTransition {
        from: WorkerStatus,
        to: WorkerStatus,
    },
    #[error("worker sequence overflow: {0}")]
    SequenceOverflow(String),
    #[error("projector failure: {0}")]
    Projector(#[from] ProjectorError),
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct WorkerSnapshot {
    pub worker: RuntimeWorker,
    pub liveness: WorkerLiveness,
}

#[derive(Clone)]
pub struct InMemoryWorkerRegistry {
    workers: Arc<RwLock<HashMap<String, WorkerRecord>>>,
    projectors: Arc<dyn ProjectionPipeline>,
    heartbeat_stale_after_ms: i64,
}

#[derive(Clone, Debug)]
struct WorkerRecord {
    worker: RuntimeWorker,
    events: Vec<WorkerLifecycleEvent>,
}

impl InMemoryWorkerRegistry {
    pub fn new(projectors: Arc<dyn ProjectionPipeline>, heartbeat_stale_after_ms: i64) -> Self {
        Self {
            workers: Arc::new(RwLock::new(HashMap::new())),
            projectors,
            heartbeat_stale_after_ms: heartbeat_stale_after_ms.max(1),
        }
    }

    pub async fn register_worker(
        &self,
        request: RegisterWorkerRequest,
    ) -> Result<WorkerSnapshot, WorkerError> {
        if !request.owner.is_valid() {
            return Err(WorkerError::InvalidOwner);
        }

        let worker_id = request
            .worker_id
            .as_deref()
            .and_then(normalized_non_empty)
            .map(std::borrow::ToOwned::to_owned)
            .unwrap_or_else(generated_worker_id);
        let now = Utc::now();
        let mut workers = self.workers.write().await;

        let (snapshot, event) = match workers.get_mut(&worker_id) {
            Some(record) => {
                if !owners_match(&record.worker.owner, &request.owner) {
                    return Err(WorkerError::Forbidden(worker_id));
                }

                record.worker.workspace_ref = request.workspace_ref.clone();
                record.worker.codex_home_ref = request.codex_home_ref.clone();
                record.worker.adapter = request
                    .adapter
                    .as_deref()
                    .and_then(normalized_non_empty)
                    .unwrap_or("in_memory")
                    .to_string();
                merge_metadata(&mut record.worker.metadata, &request.metadata);

                if matches!(
                    record.worker.status,
                    WorkerStatus::Stopped | WorkerStatus::Failed
                ) {
                    record.worker.status = WorkerStatus::Running;
                    record.worker.stopped_at = None;
                }

                let event = append_lifecycle_event(
                    record,
                    "worker.started".to_string(),
                    serde_json::json!({"status": "running", "reason": "register"}),
                    now,
                )?;
                (
                    build_snapshot(&record.worker, now, self.heartbeat_stale_after_ms),
                    event,
                )
            }
            None => {
                let mut record = WorkerRecord {
                    worker: RuntimeWorker {
                        worker_id: worker_id.clone(),
                        owner: request.owner.clone(),
                        workspace_ref: request.workspace_ref,
                        codex_home_ref: request.codex_home_ref,
                        adapter: request
                            .adapter
                            .as_deref()
                            .and_then(normalized_non_empty)
                            .unwrap_or("in_memory")
                            .to_string(),
                        status: WorkerStatus::Running,
                        latest_seq: 0,
                        metadata: request.metadata,
                        started_at: now,
                        stopped_at: None,
                        last_heartbeat_at: Some(now),
                        updated_at: now,
                    },
                    events: Vec::new(),
                };

                let event = append_lifecycle_event(
                    &mut record,
                    "worker.started".to_string(),
                    serde_json::json!({"status": "running", "reason": "register"}),
                    now,
                )?;
                let snapshot = build_snapshot(&record.worker, now, self.heartbeat_stale_after_ms);
                workers.insert(worker_id, record);
                (snapshot, event)
            }
        };
        drop(workers);

        self.projectors
            .apply_worker_event(&snapshot.worker.worker_id, &event)
            .await?;
        Ok(snapshot)
    }

    pub async fn heartbeat(
        &self,
        worker_id: &str,
        request: WorkerHeartbeatRequest,
    ) -> Result<WorkerSnapshot, WorkerError> {
        if !request.owner.is_valid() {
            return Err(WorkerError::InvalidOwner);
        }

        let now = Utc::now();
        let mut workers = self.workers.write().await;
        let record = workers
            .get_mut(worker_id)
            .ok_or_else(|| WorkerError::NotFound(worker_id.to_string()))?;
        if !owners_match(&record.worker.owner, &request.owner) {
            return Err(WorkerError::Forbidden(worker_id.to_string()));
        }

        record.worker.last_heartbeat_at = Some(now);
        merge_metadata(&mut record.worker.metadata, &request.metadata_patch);
        let event = append_lifecycle_event(
            record,
            "worker.heartbeat".to_string(),
            serde_json::json!({"status": format!("{:?}", record.worker.status).to_lowercase()}),
            now,
        )?;
        let snapshot = build_snapshot(&record.worker, now, self.heartbeat_stale_after_ms);
        drop(workers);

        self.projectors
            .apply_worker_event(&snapshot.worker.worker_id, &event)
            .await?;
        Ok(snapshot)
    }

    pub async fn transition_status(
        &self,
        worker_id: &str,
        request: WorkerStatusTransitionRequest,
    ) -> Result<WorkerSnapshot, WorkerError> {
        if !request.owner.is_valid() {
            return Err(WorkerError::InvalidOwner);
        }

        let now = Utc::now();
        let mut workers = self.workers.write().await;
        let record = workers
            .get_mut(worker_id)
            .ok_or_else(|| WorkerError::NotFound(worker_id.to_string()))?;
        if !owners_match(&record.worker.owner, &request.owner) {
            return Err(WorkerError::Forbidden(worker_id.to_string()));
        }

        let from = record.worker.status.clone();
        let to = request.status;
        if !is_transition_allowed(&from, &to) {
            return Err(WorkerError::InvalidTransition { from, to });
        }

        record.worker.status = to.clone();
        record.worker.updated_at = now;
        if matches!(to, WorkerStatus::Stopped | WorkerStatus::Failed) {
            record.worker.stopped_at = Some(now);
            record.worker.last_heartbeat_at = Some(now);
        } else {
            record.worker.stopped_at = None;
        }

        let event = append_lifecycle_event(
            record,
            to.as_event_label().to_string(),
            serde_json::json!({
                "status": format!("{to:?}").to_lowercase(),
                "reason": request.reason.unwrap_or_else(|| "status_transition".to_string())
            }),
            now,
        )?;
        let snapshot = build_snapshot(&record.worker, now, self.heartbeat_stale_after_ms);
        drop(workers);

        self.projectors
            .apply_worker_event(&snapshot.worker.worker_id, &event)
            .await?;
        Ok(snapshot)
    }

    pub async fn get_worker(
        &self,
        worker_id: &str,
        owner: &WorkerOwner,
    ) -> Result<WorkerSnapshot, WorkerError> {
        if !owner.is_valid() {
            return Err(WorkerError::InvalidOwner);
        }

        let now = Utc::now();
        let workers = self.workers.read().await;
        let record = workers
            .get(worker_id)
            .ok_or_else(|| WorkerError::NotFound(worker_id.to_string()))?;
        if !owners_match(&record.worker.owner, owner) {
            return Err(WorkerError::Forbidden(worker_id.to_string()));
        }
        Ok(build_snapshot(
            &record.worker,
            now,
            self.heartbeat_stale_after_ms,
        ))
    }

    pub async fn checkpoint_for_worker(
        &self,
        worker_id: &str,
    ) -> Result<Option<ProjectionCheckpoint>, WorkerError> {
        Ok(self.projectors.checkpoint_for_worker(worker_id).await?)
    }

    #[must_use]
    pub fn is_ready(&self) -> bool {
        self.projectors.is_ready()
    }

    #[cfg(test)]
    pub async fn get_worker_at(
        &self,
        worker_id: &str,
        owner: &WorkerOwner,
        now: DateTime<Utc>,
    ) -> Result<WorkerSnapshot, WorkerError> {
        if !owner.is_valid() {
            return Err(WorkerError::InvalidOwner);
        }

        let workers = self.workers.read().await;
        let record = workers
            .get(worker_id)
            .ok_or_else(|| WorkerError::NotFound(worker_id.to_string()))?;
        if !owners_match(&record.worker.owner, owner) {
            return Err(WorkerError::Forbidden(worker_id.to_string()));
        }
        Ok(build_snapshot(
            &record.worker,
            now,
            self.heartbeat_stale_after_ms,
        ))
    }
}

fn append_lifecycle_event(
    record: &mut WorkerRecord,
    event_type: String,
    payload: serde_json::Value,
    now: DateTime<Utc>,
) -> Result<WorkerLifecycleEvent, WorkerError> {
    let next_seq = record
        .worker
        .latest_seq
        .checked_add(1)
        .ok_or_else(|| WorkerError::SequenceOverflow(record.worker.worker_id.clone()))?;
    record.worker.latest_seq = next_seq;
    record.worker.updated_at = now;
    let event = WorkerLifecycleEvent {
        seq: next_seq,
        event_type,
        payload,
        occurred_at: now,
    };
    record.events.push(event.clone());
    Ok(event)
}

fn merge_metadata(target: &mut serde_json::Value, patch: &serde_json::Value) {
    if let (Some(target_map), Some(patch_map)) = (target.as_object_mut(), patch.as_object()) {
        for (key, value) in patch_map {
            target_map.insert(key.clone(), value.clone());
        }
    }
}

fn owners_match(left: &WorkerOwner, right: &WorkerOwner) -> bool {
    match (left.user_id, right.user_id) {
        (Some(left_id), Some(right_id)) => left_id == right_id,
        (None, None) => {
            left.guest_scope.as_deref().map(str::trim)
                == right.guest_scope.as_deref().map(str::trim)
        }
        _ => false,
    }
}

fn is_transition_allowed(from: &WorkerStatus, to: &WorkerStatus) -> bool {
    if from == to {
        return true;
    }
    match from {
        WorkerStatus::Starting => {
            matches!(
                to,
                WorkerStatus::Running | WorkerStatus::Stopping | WorkerStatus::Failed
            )
        }
        WorkerStatus::Running => {
            matches!(
                to,
                WorkerStatus::Stopping | WorkerStatus::Stopped | WorkerStatus::Failed
            )
        }
        WorkerStatus::Stopping => matches!(to, WorkerStatus::Stopped | WorkerStatus::Failed),
        WorkerStatus::Stopped => matches!(to, WorkerStatus::Starting | WorkerStatus::Running),
        WorkerStatus::Failed => matches!(to, WorkerStatus::Starting | WorkerStatus::Running),
    }
}

fn build_snapshot(
    worker: &RuntimeWorker,
    now: DateTime<Utc>,
    stale_after_ms: i64,
) -> WorkerSnapshot {
    let heartbeat_age_ms = worker
        .last_heartbeat_at
        .map(|last| (now - last).num_milliseconds().max(0));
    let heartbeat_state = if matches!(worker.status, WorkerStatus::Stopped | WorkerStatus::Failed) {
        format!("{:?}", worker.status).to_lowercase()
    } else if let Some(age_ms) = heartbeat_age_ms {
        if age_ms > stale_after_ms {
            "stale".to_string()
        } else {
            "fresh".to_string()
        }
    } else {
        "missing".to_string()
    };

    WorkerSnapshot {
        worker: worker.clone(),
        liveness: WorkerLiveness {
            heartbeat_age_ms,
            heartbeat_stale_after_ms: stale_after_ms,
            heartbeat_state,
        },
    }
}

fn generated_worker_id() -> String {
    format!("codexw_{}", uuid::Uuid::now_v7())
}

fn normalized_non_empty(value: &str) -> Option<&str> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

#[cfg(test)]
mod tests {
    use anyhow::{Result, anyhow};
    use chrono::Duration;
    use serde_json::json;

    use super::InMemoryWorkerRegistry;
    use crate::{
        projectors::InMemoryProjectionPipeline,
        types::{
            RegisterWorkerRequest, WorkerHeartbeatRequest, WorkerOwner, WorkerStatus,
            WorkerStatusTransitionRequest,
        },
    };

    fn user_owner(user_id: u64) -> WorkerOwner {
        WorkerOwner {
            user_id: Some(user_id),
            guest_scope: None,
        }
    }

    #[tokio::test]
    async fn registry_enforces_worker_ownership() -> Result<()> {
        let registry = InMemoryWorkerRegistry::new(InMemoryProjectionPipeline::shared(), 1_000);
        let created = registry
            .register_worker(RegisterWorkerRequest {
                worker_id: Some("desktop:worker-1".to_string()),
                owner: user_owner(7),
                workspace_ref: None,
                codex_home_ref: None,
                adapter: None,
                metadata: json!({"env": "dev"}),
            })
            .await?;
        assert_eq!(created.worker.worker_id, "desktop:worker-1");

        let forbidden = registry
            .get_worker(
                "desktop:worker-1",
                &WorkerOwner {
                    user_id: Some(8),
                    guest_scope: None,
                },
            )
            .await;
        if forbidden.is_ok() {
            return Err(anyhow!("expected forbidden access for non-owner"));
        }
        Ok(())
    }

    #[tokio::test]
    async fn registry_validates_status_transitions() -> Result<()> {
        let registry = InMemoryWorkerRegistry::new(InMemoryProjectionPipeline::shared(), 1_000);
        registry
            .register_worker(RegisterWorkerRequest {
                worker_id: Some("desktop:worker-2".to_string()),
                owner: user_owner(7),
                workspace_ref: None,
                codex_home_ref: None,
                adapter: None,
                metadata: json!({}),
            })
            .await?;

        let updated = registry
            .transition_status(
                "desktop:worker-2",
                WorkerStatusTransitionRequest {
                    owner: user_owner(7),
                    status: WorkerStatus::Stopping,
                    reason: Some("drain".to_string()),
                },
            )
            .await?;
        assert_eq!(updated.worker.status, WorkerStatus::Stopping);

        let invalid = registry
            .transition_status(
                "desktop:worker-2",
                WorkerStatusTransitionRequest {
                    owner: user_owner(7),
                    status: WorkerStatus::Running,
                    reason: None,
                },
            )
            .await;
        if invalid.is_ok() {
            return Err(anyhow!(
                "expected invalid transition from stopping to running"
            ));
        }
        Ok(())
    }

    #[tokio::test]
    async fn heartbeat_reports_stale_state_after_timeout() -> Result<()> {
        let registry = InMemoryWorkerRegistry::new(InMemoryProjectionPipeline::shared(), 50);
        let owner = user_owner(7);
        let snapshot = registry
            .register_worker(RegisterWorkerRequest {
                worker_id: Some("desktop:worker-3".to_string()),
                owner: owner.clone(),
                workspace_ref: None,
                codex_home_ref: None,
                adapter: None,
                metadata: json!({}),
            })
            .await?;
        registry
            .heartbeat(
                &snapshot.worker.worker_id,
                WorkerHeartbeatRequest {
                    owner: owner.clone(),
                    metadata_patch: json!({"last_seen": "now"}),
                },
            )
            .await?;

        let stale_now = chrono::Utc::now() + Duration::milliseconds(120);
        let stale_snapshot = registry
            .get_worker_at(&snapshot.worker.worker_id, &owner, stale_now)
            .await?;
        assert_eq!(stale_snapshot.liveness.heartbeat_state, "stale");
        Ok(())
    }
}
