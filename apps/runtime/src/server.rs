use std::sync::Arc;

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::{StatusCode, header},
    response::IntoResponse,
    routing::{get, post},
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    artifacts::{ArtifactError, RuntimeReceipt, build_receipt, build_replay_jsonl},
    authority::AuthorityError,
    config::Config,
    fanout::{ExternalFanoutHook, FanoutHub, FanoutMessage},
    orchestration::{OrchestrationError, RuntimeOrchestrator},
    types::{
        AppendRunEventRequest, ProjectionCheckpoint, ProjectionDriftReport, RegisterWorkerRequest,
        RunProjectionSummary, RuntimeRun, StartRunRequest, WorkerHeartbeatRequest, WorkerOwner,
        WorkerStatus, WorkerStatusTransitionRequest,
    },
    workers::{InMemoryWorkerRegistry, WorkerError, WorkerSnapshot},
};

#[derive(Clone)]
pub struct AppState {
    config: Config,
    orchestrator: Arc<RuntimeOrchestrator>,
    workers: Arc<InMemoryWorkerRegistry>,
    fanout: Arc<FanoutHub>,
    started_at: chrono::DateTime<Utc>,
}

impl AppState {
    #[must_use]
    pub fn new(
        config: Config,
        orchestrator: Arc<RuntimeOrchestrator>,
        workers: Arc<InMemoryWorkerRegistry>,
        fanout: Arc<FanoutHub>,
    ) -> Self {
        Self {
            config,
            orchestrator,
            workers,
            fanout,
            started_at: Utc::now(),
        }
    }
}

#[derive(Debug, Serialize)]
struct HealthResponse {
    status: &'static str,
    service: String,
    build_sha: String,
    uptime_seconds: i64,
    authority_write_mode: String,
    authority_writer_active: bool,
    fanout_driver: String,
}

#[derive(Debug, Serialize)]
struct ReadinessResponse {
    status: &'static str,
    authority_ready: bool,
    projector_ready: bool,
    workers_ready: bool,
    authority_writer_active: bool,
    fanout_driver: String,
}

#[derive(Debug, Deserialize)]
struct StartRunBody {
    worker_id: Option<String>,
    #[serde(default)]
    metadata: serde_json::Value,
}

#[derive(Debug, Deserialize)]
struct AppendRunEventBody {
    event_type: String,
    #[serde(default)]
    payload: serde_json::Value,
    idempotency_key: Option<String>,
    expected_previous_seq: Option<u64>,
}

#[derive(Debug, Serialize)]
struct RunResponse {
    run: RuntimeRun,
}

#[derive(Debug, Serialize)]
struct WorkerResponse {
    worker: WorkerSnapshot,
}

#[derive(Debug, Serialize)]
struct CheckpointResponse {
    checkpoint: ProjectionCheckpoint,
}

#[derive(Debug, Serialize)]
struct DriftResponse {
    drift: ProjectionDriftReport,
}

#[derive(Debug, Serialize)]
struct RunSummaryResponse {
    summary: RunProjectionSummary,
}

#[derive(Debug, Deserialize)]
struct FanoutPollQuery {
    after_seq: Option<u64>,
    limit: Option<usize>,
}

#[derive(Debug, Serialize)]
struct FanoutPollResponse {
    topic: String,
    driver: String,
    messages: Vec<FanoutMessage>,
}

#[derive(Debug, Serialize)]
struct FanoutHooksResponse {
    driver: String,
    hooks: Vec<ExternalFanoutHook>,
}

#[derive(Debug, Deserialize)]
struct OwnerQuery {
    owner_user_id: Option<u64>,
    owner_guest_scope: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DriftQuery {
    topic: String,
}

#[derive(Debug, Deserialize)]
struct RegisterWorkerBody {
    worker_id: Option<String>,
    owner_user_id: Option<u64>,
    owner_guest_scope: Option<String>,
    workspace_ref: Option<String>,
    codex_home_ref: Option<String>,
    adapter: Option<String>,
    #[serde(default)]
    metadata: serde_json::Value,
}

#[derive(Debug, Deserialize)]
struct WorkerHeartbeatBody {
    owner_user_id: Option<u64>,
    owner_guest_scope: Option<String>,
    #[serde(default)]
    metadata_patch: serde_json::Value,
}

#[derive(Debug, Deserialize)]
struct WorkerTransitionBody {
    owner_user_id: Option<u64>,
    owner_guest_scope: Option<String>,
    status: WorkerStatus,
    reason: Option<String>,
}

pub fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/healthz", get(health))
        .route("/readyz", get(readiness))
        .route("/internal/v1/runs", post(start_run))
        .route("/internal/v1/runs/:run_id", get(get_run))
        .route("/internal/v1/runs/:run_id/events", post(append_run_event))
        .route("/internal/v1/runs/:run_id/receipt", get(get_run_receipt))
        .route("/internal/v1/runs/:run_id/replay", get(get_run_replay))
        .route(
            "/internal/v1/khala/topics/:topic/messages",
            get(get_khala_topic_messages),
        )
        .route(
            "/internal/v1/khala/fanout/hooks",
            get(get_khala_fanout_hooks),
        )
        .route(
            "/internal/v1/projectors/checkpoints/:run_id",
            get(get_run_checkpoint),
        )
        .route("/internal/v1/projectors/drift", get(get_projector_drift))
        .route(
            "/internal/v1/projectors/run-summary/:run_id",
            get(get_projector_run_summary),
        )
        .route("/internal/v1/workers", post(register_worker))
        .route("/internal/v1/workers/:worker_id", get(get_worker))
        .route(
            "/internal/v1/workers/:worker_id/heartbeat",
            post(heartbeat_worker),
        )
        .route(
            "/internal/v1/workers/:worker_id/status",
            post(transition_worker),
        )
        .route(
            "/internal/v1/workers/:worker_id/checkpoint",
            get(get_worker_checkpoint),
        )
        .with_state(state)
}

async fn health(State(state): State<AppState>) -> Json<HealthResponse> {
    let uptime_seconds = (Utc::now() - state.started_at).num_seconds();
    Json(HealthResponse {
        status: "ok",
        service: state.config.service_name,
        build_sha: state.config.build_sha,
        uptime_seconds,
        authority_write_mode: state.config.authority_write_mode.as_str().to_string(),
        authority_writer_active: state.config.authority_write_mode.writes_enabled(),
        fanout_driver: state.fanout.driver_name().to_string(),
    })
}

async fn readiness(State(state): State<AppState>) -> impl IntoResponse {
    let runtime_readiness = state.orchestrator.readiness();
    let workers_ready = state.workers.is_ready();
    let ready = runtime_readiness.is_ready() && workers_ready;
    let status = if ready {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };
    (
        status,
        Json(ReadinessResponse {
            status: if ready { "ready" } else { "not_ready" },
            authority_ready: runtime_readiness.authority_ready,
            projector_ready: runtime_readiness.projector_ready,
            workers_ready,
            authority_writer_active: state.config.authority_write_mode.writes_enabled(),
            fanout_driver: state.fanout.driver_name().to_string(),
        }),
    )
}

async fn start_run(
    State(state): State<AppState>,
    Json(body): Json<StartRunBody>,
) -> Result<(StatusCode, Json<RunResponse>), ApiError> {
    ensure_runtime_write_authority(&state)?;
    let run = state
        .orchestrator
        .start_run(StartRunRequest {
            worker_id: body.worker_id,
            metadata: body.metadata,
        })
        .await
        .map_err(ApiError::from_orchestration)?;
    publish_latest_run_event(&state, &run).await?;
    Ok((StatusCode::CREATED, Json(RunResponse { run })))
}

async fn append_run_event(
    State(state): State<AppState>,
    Path(run_id): Path<Uuid>,
    Json(body): Json<AppendRunEventBody>,
) -> Result<Json<RunResponse>, ApiError> {
    ensure_runtime_write_authority(&state)?;
    let run = state
        .orchestrator
        .append_run_event(
            run_id,
            AppendRunEventRequest {
                event_type: body.event_type,
                payload: body.payload,
                idempotency_key: body.idempotency_key,
                expected_previous_seq: body.expected_previous_seq,
            },
        )
        .await
        .map_err(ApiError::from_orchestration)?;
    publish_latest_run_event(&state, &run).await?;
    Ok(Json(RunResponse { run }))
}

async fn get_run(
    State(state): State<AppState>,
    Path(run_id): Path<Uuid>,
) -> Result<Json<RunResponse>, ApiError> {
    let run = state
        .orchestrator
        .get_run(run_id)
        .await
        .map_err(ApiError::from_orchestration)?
        .ok_or(ApiError::NotFound)?;
    Ok(Json(RunResponse { run }))
}

async fn get_run_receipt(
    State(state): State<AppState>,
    Path(run_id): Path<Uuid>,
) -> Result<Json<RuntimeReceipt>, ApiError> {
    let run = state
        .orchestrator
        .get_run(run_id)
        .await
        .map_err(ApiError::from_orchestration)?
        .ok_or(ApiError::NotFound)?;
    let receipt = build_receipt(&run).map_err(ApiError::from_artifacts)?;
    Ok(Json(receipt))
}

async fn get_run_replay(
    State(state): State<AppState>,
    Path(run_id): Path<Uuid>,
) -> Result<impl IntoResponse, ApiError> {
    let run = state
        .orchestrator
        .get_run(run_id)
        .await
        .map_err(ApiError::from_orchestration)?
        .ok_or(ApiError::NotFound)?;
    let replay = build_replay_jsonl(&run).map_err(ApiError::from_artifacts)?;
    Ok(([(header::CONTENT_TYPE, "application/x-ndjson")], replay))
}

async fn get_khala_topic_messages(
    State(state): State<AppState>,
    Path(topic): Path<String>,
    Query(query): Query<FanoutPollQuery>,
) -> Result<Json<FanoutPollResponse>, ApiError> {
    if topic.trim().is_empty() {
        return Err(ApiError::InvalidRequest(
            "topic is required for khala fanout polling".to_string(),
        ));
    }
    let after_seq = query.after_seq.unwrap_or(0);
    let limit = query.limit.unwrap_or(100);
    let messages = state
        .fanout
        .poll(&topic, after_seq, limit)
        .await
        .map_err(|error| ApiError::Internal(error.to_string()))?;
    Ok(Json(FanoutPollResponse {
        topic,
        driver: state.fanout.driver_name().to_string(),
        messages,
    }))
}

async fn get_khala_fanout_hooks(
    State(state): State<AppState>,
) -> Result<Json<FanoutHooksResponse>, ApiError> {
    Ok(Json(FanoutHooksResponse {
        driver: state.fanout.driver_name().to_string(),
        hooks: state.fanout.external_hooks(),
    }))
}

async fn get_run_checkpoint(
    State(state): State<AppState>,
    Path(run_id): Path<Uuid>,
) -> Result<Json<CheckpointResponse>, ApiError> {
    let checkpoint = state
        .orchestrator
        .checkpoint_for_run(run_id)
        .await
        .map_err(ApiError::from_orchestration)?
        .ok_or(ApiError::NotFound)?;
    Ok(Json(CheckpointResponse { checkpoint }))
}

async fn get_projector_drift(
    State(state): State<AppState>,
    Query(query): Query<DriftQuery>,
) -> Result<Json<DriftResponse>, ApiError> {
    if query.topic.trim().is_empty() {
        return Err(ApiError::InvalidRequest(
            "topic is required for drift lookup".to_string(),
        ));
    }

    let drift = state
        .orchestrator
        .projectors()
        .drift_for_topic(&query.topic)
        .await
        .map_err(|error| ApiError::Internal(error.to_string()))?
        .ok_or(ApiError::NotFound)?;
    Ok(Json(DriftResponse { drift }))
}

async fn get_projector_run_summary(
    State(state): State<AppState>,
    Path(run_id): Path<Uuid>,
) -> Result<Json<RunSummaryResponse>, ApiError> {
    let summary = state
        .orchestrator
        .projectors()
        .run_summary(run_id)
        .await
        .map_err(|error| ApiError::Internal(error.to_string()))?
        .ok_or(ApiError::NotFound)?;
    Ok(Json(RunSummaryResponse { summary }))
}

async fn register_worker(
    State(state): State<AppState>,
    Json(body): Json<RegisterWorkerBody>,
) -> Result<(StatusCode, Json<WorkerResponse>), ApiError> {
    ensure_runtime_write_authority(&state)?;
    let owner = owner_from_parts(body.owner_user_id, body.owner_guest_scope)?;
    let snapshot = state
        .workers
        .register_worker(RegisterWorkerRequest {
            worker_id: body.worker_id,
            owner,
            workspace_ref: body.workspace_ref,
            codex_home_ref: body.codex_home_ref,
            adapter: body.adapter,
            metadata: body.metadata,
        })
        .await
        .map_err(ApiError::from_worker)?;
    publish_worker_snapshot(&state, &snapshot).await?;
    Ok((
        StatusCode::CREATED,
        Json(WorkerResponse { worker: snapshot }),
    ))
}

async fn get_worker(
    State(state): State<AppState>,
    Path(worker_id): Path<String>,
    Query(query): Query<OwnerQuery>,
) -> Result<Json<WorkerResponse>, ApiError> {
    let owner = owner_from_parts(query.owner_user_id, query.owner_guest_scope)?;
    let snapshot = state
        .workers
        .get_worker(&worker_id, &owner)
        .await
        .map_err(ApiError::from_worker)?;
    Ok(Json(WorkerResponse { worker: snapshot }))
}

async fn heartbeat_worker(
    State(state): State<AppState>,
    Path(worker_id): Path<String>,
    Json(body): Json<WorkerHeartbeatBody>,
) -> Result<Json<WorkerResponse>, ApiError> {
    ensure_runtime_write_authority(&state)?;
    let owner = owner_from_parts(body.owner_user_id, body.owner_guest_scope)?;
    let snapshot = state
        .workers
        .heartbeat(
            &worker_id,
            WorkerHeartbeatRequest {
                owner,
                metadata_patch: body.metadata_patch,
            },
        )
        .await
        .map_err(ApiError::from_worker)?;
    publish_worker_snapshot(&state, &snapshot).await?;
    Ok(Json(WorkerResponse { worker: snapshot }))
}

async fn transition_worker(
    State(state): State<AppState>,
    Path(worker_id): Path<String>,
    Json(body): Json<WorkerTransitionBody>,
) -> Result<Json<WorkerResponse>, ApiError> {
    ensure_runtime_write_authority(&state)?;
    let owner = owner_from_parts(body.owner_user_id, body.owner_guest_scope)?;
    let snapshot = state
        .workers
        .transition_status(
            &worker_id,
            WorkerStatusTransitionRequest {
                owner,
                status: body.status,
                reason: body.reason,
            },
        )
        .await
        .map_err(ApiError::from_worker)?;
    publish_worker_snapshot(&state, &snapshot).await?;
    Ok(Json(WorkerResponse { worker: snapshot }))
}

async fn get_worker_checkpoint(
    State(state): State<AppState>,
    Path(worker_id): Path<String>,
) -> Result<Json<CheckpointResponse>, ApiError> {
    let checkpoint = state
        .workers
        .checkpoint_for_worker(&worker_id)
        .await
        .map_err(ApiError::from_worker)?
        .ok_or(ApiError::NotFound)?;
    Ok(Json(CheckpointResponse { checkpoint }))
}

enum ApiError {
    NotFound,
    Forbidden(String),
    Conflict(String),
    WritePathFrozen(String),
    InvalidRequest(String),
    Internal(String),
}

impl ApiError {
    fn from_orchestration(error: OrchestrationError) -> Self {
        match error {
            OrchestrationError::RunNotFound(_) => Self::NotFound,
            OrchestrationError::EmptyEventType => {
                Self::InvalidRequest("event_type cannot be empty".to_string())
            }
            OrchestrationError::RunStateMachine(state_error) => {
                Self::InvalidRequest(state_error.to_string())
            }
            OrchestrationError::Authority(AuthorityError::SequenceConflict {
                expected_previous_seq,
                actual_previous_seq,
                ..
            }) => Self::Conflict(format!(
                "expected_previous_seq {expected_previous_seq} does not match actual previous seq {actual_previous_seq}"
            )),
            other => Self::Internal(other.to_string()),
        }
    }

    fn from_worker(error: WorkerError) -> Self {
        match error {
            WorkerError::InvalidOwner => Self::InvalidRequest(
                "owner_user_id or owner_guest_scope must be provided (but not both)".to_string(),
            ),
            WorkerError::NotFound(_) => Self::NotFound,
            WorkerError::Forbidden(worker_id) => {
                Self::Forbidden(format!("owner mismatch for worker {worker_id}"))
            }
            WorkerError::InvalidTransition { from, to } => {
                Self::InvalidRequest(format!("invalid worker transition from {from:?} to {to:?}"))
            }
            other => Self::Internal(other.to_string()),
        }
    }

    fn from_artifacts(error: ArtifactError) -> Self {
        Self::Internal(error.to_string())
    }
}

fn owner_from_parts(
    user_id: Option<u64>,
    guest_scope: Option<String>,
) -> Result<WorkerOwner, ApiError> {
    let owner = WorkerOwner {
        user_id,
        guest_scope,
    };
    if owner.is_valid() {
        Ok(owner)
    } else {
        Err(ApiError::InvalidRequest(
            "owner_user_id or owner_guest_scope must be provided (but not both)".to_string(),
        ))
    }
}

fn ensure_runtime_write_authority(state: &AppState) -> Result<(), ApiError> {
    if state.config.authority_write_mode.writes_enabled() {
        Ok(())
    } else {
        Err(ApiError::WritePathFrozen(format!(
            "runtime authority writes are disabled in mode {}",
            state.config.authority_write_mode.as_str()
        )))
    }
}

async fn publish_latest_run_event(state: &AppState, run: &RuntimeRun) -> Result<(), ApiError> {
    let Some(event) = run.events.last() else {
        return Ok(());
    };
    let topic = format!("run:{}:events", run.id);
    state
        .fanout
        .publish(
            &topic,
            FanoutMessage {
                topic: topic.clone(),
                sequence: event.seq,
                kind: event.event_type.clone(),
                payload: event.payload.clone(),
                published_at: Utc::now(),
            },
        )
        .await
        .map_err(|error| ApiError::Internal(error.to_string()))
}

async fn publish_worker_snapshot(
    state: &AppState,
    snapshot: &WorkerSnapshot,
) -> Result<(), ApiError> {
    let topic = format!("worker:{}:lifecycle", snapshot.worker.worker_id);
    state
        .fanout
        .publish(
            &topic,
            FanoutMessage {
                topic: topic.clone(),
                sequence: snapshot.worker.latest_seq,
                kind: snapshot.worker.status.as_event_label().to_string(),
                payload: serde_json::json!({
                    "worker_id": snapshot.worker.worker_id,
                    "status": snapshot.worker.status,
                    "latest_seq": snapshot.worker.latest_seq,
                    "heartbeat_state": snapshot.liveness.heartbeat_state,
                }),
                published_at: Utc::now(),
            },
        )
        .await
        .map_err(|error| ApiError::Internal(error.to_string()))
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        match self {
            Self::NotFound => (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({
                    "error": "not_found",
                })),
            )
                .into_response(),
            Self::Forbidden(message) => (
                StatusCode::FORBIDDEN,
                Json(serde_json::json!({
                    "error": "forbidden",
                    "message": message,
                })),
            )
                .into_response(),
            Self::Conflict(message) => (
                StatusCode::CONFLICT,
                Json(serde_json::json!({
                    "error": "conflict",
                    "message": message,
                })),
            )
                .into_response(),
            Self::WritePathFrozen(message) => (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({
                    "error": "write_path_frozen",
                    "message": message,
                })),
            )
                .into_response(),
            Self::InvalidRequest(message) => (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "error": "invalid_request",
                    "message": message,
                })),
            )
                .into_response(),
            Self::Internal(message) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": "internal",
                    "message": message,
                })),
            )
                .into_response(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use anyhow::{Result, anyhow};
    use axum::{
        body::Body,
        http::{Method, Request},
    };
    use http_body_util::BodyExt;
    use serde_json::Value;
    use tower::ServiceExt;

    use super::{AppState, build_router};
    use crate::{
        authority::InMemoryRuntimeAuthority,
        config::{AuthorityWriteMode, Config},
        event_log::DurableEventLog,
        fanout::FanoutHub,
        orchestration::RuntimeOrchestrator,
        projectors::InMemoryProjectionPipeline,
        workers::InMemoryWorkerRegistry,
    };

    fn loopback_bind_addr() -> std::net::SocketAddr {
        std::net::SocketAddr::from(([127, 0, 0, 1], 0))
    }

    fn test_router_with_mode(mode: AuthorityWriteMode) -> axum::Router {
        let projectors = InMemoryProjectionPipeline::shared();
        let projector_pipeline: Arc<dyn crate::projectors::ProjectionPipeline> = projectors.clone();
        let state = AppState::new(
            Config {
                service_name: "runtime-test".to_string(),
                bind_addr: loopback_bind_addr(),
                build_sha: "test".to_string(),
                authority_write_mode: mode,
                fanout_driver: "memory".to_string(),
                fanout_queue_capacity: 64,
            },
            Arc::new(RuntimeOrchestrator::new(
                Arc::new(InMemoryRuntimeAuthority::with_event_log(
                    DurableEventLog::new_memory(),
                )),
                projector_pipeline,
            )),
            Arc::new(InMemoryWorkerRegistry::new(projectors, 120_000)),
            Arc::new(FanoutHub::memory(64)),
        );
        build_router(state)
    }

    fn test_router() -> axum::Router {
        test_router_with_mode(AuthorityWriteMode::RustActive)
    }

    async fn response_json(response: axum::response::Response) -> Result<Value> {
        let collected = response.into_body().collect().await?;
        let bytes = collected.to_bytes();
        Ok(serde_json::from_slice(&bytes)?)
    }

    #[tokio::test]
    async fn health_and_readiness_endpoints_are_available() -> Result<()> {
        let app = test_router();

        let health = app
            .clone()
            .oneshot(Request::builder().uri("/healthz").body(Body::empty())?)
            .await?;
        let readiness = app
            .oneshot(Request::builder().uri("/readyz").body(Body::empty())?)
            .await?;

        assert_eq!(health.status(), axum::http::StatusCode::OK);
        assert_eq!(readiness.status(), axum::http::StatusCode::OK);
        Ok(())
    }

    #[tokio::test]
    async fn run_lifecycle_updates_projector_checkpoint() -> Result<()> {
        let app = test_router();

        let create_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/internal/v1/runs")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&serde_json::json!({
                        "worker_id": "desktop:worker-1",
                        "metadata": {"source": "test"}
                    }))?))?,
            )
            .await?;
        assert_eq!(create_response.status(), axum::http::StatusCode::CREATED);

        let create_json = response_json(create_response).await?;
        let run_id = create_json
            .pointer("/run/id")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| anyhow!("missing run id"))?
            .to_string();

        let append_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri(format!("/internal/v1/runs/{run_id}/events"))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&serde_json::json!({
                        "event_type": "run.step.completed",
                        "payload": {"step": 1}
                    }))?))?,
            )
            .await?;
        assert_eq!(append_response.status(), axum::http::StatusCode::OK);

        let checkpoint_response = app
            .oneshot(
                Request::builder()
                    .uri(format!("/internal/v1/projectors/checkpoints/{run_id}"))
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(checkpoint_response.status(), axum::http::StatusCode::OK);
        let checkpoint_json = response_json(checkpoint_response).await?;
        let last_seq = checkpoint_json
            .pointer("/checkpoint/last_seq")
            .and_then(serde_json::Value::as_u64)
            .ok_or_else(|| anyhow!("missing checkpoint last_seq"))?;
        assert_eq!(last_seq, 2);

        Ok(())
    }

    #[tokio::test]
    async fn run_state_machine_rejects_invalid_terminal_transition() -> Result<()> {
        let app = test_router();

        let create_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/internal/v1/runs")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&serde_json::json!({
                        "worker_id": "desktop:worker-state",
                        "metadata": {"source": "state-machine-test"}
                    }))?))?,
            )
            .await?;
        let create_json = response_json(create_response).await?;
        let run_id = create_json
            .pointer("/run/id")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| anyhow!("missing run id"))?
            .to_string();

        let finish_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri(format!("/internal/v1/runs/{run_id}/events"))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&serde_json::json!({
                        "event_type": "run.finished",
                        "payload": {"status": "succeeded", "reason_class": "completed"}
                    }))?))?,
            )
            .await?;
        assert_eq!(finish_response.status(), axum::http::StatusCode::OK);

        let invalid_transition = app
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri(format!("/internal/v1/runs/{run_id}/events"))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&serde_json::json!({
                        "event_type": "run.cancel_requested",
                        "payload": {"reason": "late_cancel"}
                    }))?))?,
            )
            .await?;
        assert_eq!(
            invalid_transition.status(),
            axum::http::StatusCode::BAD_REQUEST
        );

        Ok(())
    }

    #[tokio::test]
    async fn append_run_event_supports_idempotency_and_sequence_conflicts() -> Result<()> {
        let app = test_router();
        let create_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/internal/v1/runs")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&serde_json::json!({
                        "worker_id": "desktop:worker-idempotency",
                        "metadata": {}
                    }))?))?,
            )
            .await?;
        let create_json = response_json(create_response).await?;
        let run_id = create_json
            .pointer("/run/id")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| anyhow!("missing run id"))?
            .to_string();

        let first_append = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri(format!("/internal/v1/runs/{run_id}/events"))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&serde_json::json!({
                        "event_type": "run.step.completed",
                        "payload": {"step": 1},
                        "idempotency_key": "step-1",
                        "expected_previous_seq": 1
                    }))?))?,
            )
            .await?;
        assert_eq!(first_append.status(), axum::http::StatusCode::OK);

        let second_append = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri(format!("/internal/v1/runs/{run_id}/events"))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&serde_json::json!({
                        "event_type": "run.step.completed",
                        "payload": {"step": 1},
                        "idempotency_key": "step-1",
                        "expected_previous_seq": 1
                    }))?))?,
            )
            .await?;
        assert_eq!(second_append.status(), axum::http::StatusCode::OK);
        let second_json = response_json(second_append).await?;
        let event_count = second_json
            .pointer("/run/events")
            .and_then(serde_json::Value::as_array)
            .map_or(0, std::vec::Vec::len);
        assert_eq!(event_count, 2);

        let conflict_append = app
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri(format!("/internal/v1/runs/{run_id}/events"))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&serde_json::json!({
                        "event_type": "run.step.completed",
                        "payload": {"step": 2},
                        "idempotency_key": "step-2",
                        "expected_previous_seq": 1
                    }))?))?,
            )
            .await?;
        assert_eq!(conflict_append.status(), axum::http::StatusCode::CONFLICT);

        Ok(())
    }

    #[tokio::test]
    async fn run_artifact_endpoints_return_receipt_and_replay() -> Result<()> {
        let app = test_router();
        let create_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/internal/v1/runs")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&serde_json::json!({
                        "worker_id": "desktop:artifact-worker",
                        "metadata": {"policy_bundle_id": "policy.runtime.v1"}
                    }))?))?,
            )
            .await?;
        let create_json = response_json(create_response).await?;
        let run_id = create_json
            .pointer("/run/id")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| anyhow!("missing run id"))?
            .to_string();

        let receipt_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/internal/v1/runs/{run_id}/receipt"))
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(receipt_response.status(), axum::http::StatusCode::OK);
        let receipt_json = response_json(receipt_response).await?;
        assert_eq!(
            receipt_json
                .get("schema")
                .and_then(serde_json::Value::as_str)
                .unwrap_or(""),
            "openagents.receipt.v1"
        );

        let replay_response = app
            .oneshot(
                Request::builder()
                    .uri(format!("/internal/v1/runs/{run_id}/replay"))
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(replay_response.status(), axum::http::StatusCode::OK);
        let replay_text = String::from_utf8(
            replay_response
                .into_body()
                .collect()
                .await?
                .to_bytes()
                .to_vec(),
        )?;
        if !replay_text.contains("\"type\":\"ReplayHeader\"")
            || !replay_text.contains("\"type\":\"SessionEnd\"")
        {
            return Err(anyhow!("replay output missing required sections"));
        }

        Ok(())
    }

    #[tokio::test]
    async fn projector_summary_endpoint_returns_projected_run_state() -> Result<()> {
        let app = test_router();
        let create_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/internal/v1/runs")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&serde_json::json!({
                        "worker_id": "desktop:summary-worker",
                        "metadata": {}
                    }))?))?,
            )
            .await?;
        let create_json = response_json(create_response).await?;
        let run_id = create_json
            .pointer("/run/id")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| anyhow!("missing run id"))?
            .to_string();

        let _ = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri(format!("/internal/v1/runs/{run_id}/events"))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&serde_json::json!({
                        "event_type": "run.finished",
                        "payload": {"status": "succeeded"},
                        "idempotency_key": "summary-finish",
                        "expected_previous_seq": 1
                    }))?))?,
            )
            .await?;

        let summary_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/internal/v1/projectors/run-summary/{run_id}"))
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(summary_response.status(), axum::http::StatusCode::OK);
        let summary_json = response_json(summary_response).await?;
        assert_eq!(
            summary_json
                .pointer("/summary/status")
                .and_then(serde_json::Value::as_str)
                .unwrap_or(""),
            "succeeded"
        );

        let drift_not_found = app
            .oneshot(
                Request::builder()
                    .uri("/internal/v1/projectors/drift?topic=run:missing:events")
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(drift_not_found.status(), axum::http::StatusCode::NOT_FOUND);

        Ok(())
    }

    #[tokio::test]
    async fn khala_fanout_endpoints_surface_memory_driver_delivery() -> Result<()> {
        let app = test_router();
        let create_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/internal/v1/runs")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&serde_json::json!({
                        "worker_id": "desktop:fanout-worker",
                        "metadata": {}
                    }))?))?,
            )
            .await?;
        let create_json = response_json(create_response).await?;
        let run_id = create_json
            .pointer("/run/id")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| anyhow!("missing run id"))?
            .to_string();

        let messages_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!(
                        "/internal/v1/khala/topics/run:{run_id}:events/messages?after_seq=0&limit=10"
                    ))
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(messages_response.status(), axum::http::StatusCode::OK);
        let messages_json = response_json(messages_response).await?;
        let first_kind = messages_json
            .pointer("/messages/0/kind")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("");
        assert_eq!(first_kind, "run.started");

        let hooks_response = app
            .oneshot(
                Request::builder()
                    .uri("/internal/v1/khala/fanout/hooks")
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(hooks_response.status(), axum::http::StatusCode::OK);
        let hooks_json = response_json(hooks_response).await?;
        let hook_count = hooks_json
            .pointer("/hooks")
            .and_then(serde_json::Value::as_array)
            .map_or(0, std::vec::Vec::len);
        assert_eq!(hook_count, 3);

        Ok(())
    }

    #[tokio::test]
    async fn write_endpoints_are_frozen_when_authority_mode_is_read_only() -> Result<()> {
        let app = test_router_with_mode(AuthorityWriteMode::ReadOnly);
        let response = app
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/internal/v1/runs")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&serde_json::json!({
                        "worker_id": "desktop:blocked-worker",
                        "metadata": {}
                    }))?))?,
            )
            .await?;
        assert_eq!(
            response.status(),
            axum::http::StatusCode::SERVICE_UNAVAILABLE
        );

        let response_json = response_json(response).await?;
        assert_eq!(
            response_json
                .get("error")
                .and_then(serde_json::Value::as_str)
                .unwrap_or(""),
            "write_path_frozen"
        );
        Ok(())
    }

    #[tokio::test]
    async fn worker_lifecycle_enforces_owner_and_updates_checkpoint() -> Result<()> {
        let app = test_router();

        let create_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/internal/v1/workers")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&serde_json::json!({
                        "worker_id": "desktop:worker-9",
                        "owner_user_id": 11,
                        "metadata": {"workspace": "openagents"}
                    }))?))?,
            )
            .await?;
        assert_eq!(create_response.status(), axum::http::StatusCode::CREATED);

        let heartbeat_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/internal/v1/workers/desktop:worker-9/heartbeat")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&serde_json::json!({
                        "owner_user_id": 11,
                        "metadata_patch": {"ping": true}
                    }))?))?,
            )
            .await?;
        assert_eq!(heartbeat_response.status(), axum::http::StatusCode::OK);

        let forbidden_get = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/internal/v1/workers/desktop:worker-9?owner_user_id=12")
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(forbidden_get.status(), axum::http::StatusCode::FORBIDDEN);

        let checkpoint_response = app
            .oneshot(
                Request::builder()
                    .uri("/internal/v1/workers/desktop:worker-9/checkpoint")
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(checkpoint_response.status(), axum::http::StatusCode::OK);
        let checkpoint_json = response_json(checkpoint_response).await?;
        let event_type = checkpoint_json
            .pointer("/checkpoint/last_event_type")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| anyhow!("missing worker checkpoint event type"))?;
        assert_eq!(event_type, "worker.heartbeat");

        Ok(())
    }
}
