use std::sync::Arc;

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    config::Config,
    orchestration::{OrchestrationError, RuntimeOrchestrator},
    types::{
        AppendRunEventRequest, ProjectionCheckpoint, RegisterWorkerRequest, RuntimeRun,
        StartRunRequest, WorkerHeartbeatRequest, WorkerOwner, WorkerStatus,
        WorkerStatusTransitionRequest,
    },
    workers::{InMemoryWorkerRegistry, WorkerError, WorkerSnapshot},
};

#[derive(Clone)]
pub struct AppState {
    config: Config,
    orchestrator: Arc<RuntimeOrchestrator>,
    workers: Arc<InMemoryWorkerRegistry>,
    started_at: chrono::DateTime<Utc>,
}

impl AppState {
    #[must_use]
    pub fn new(
        config: Config,
        orchestrator: Arc<RuntimeOrchestrator>,
        workers: Arc<InMemoryWorkerRegistry>,
    ) -> Self {
        Self {
            config,
            orchestrator,
            workers,
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
}

#[derive(Debug, Serialize)]
struct ReadinessResponse {
    status: &'static str,
    authority_ready: bool,
    projector_ready: bool,
    workers_ready: bool,
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

#[derive(Debug, Deserialize)]
struct OwnerQuery {
    owner_user_id: Option<u64>,
    owner_guest_scope: Option<String>,
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
        .route(
            "/internal/v1/projectors/checkpoints/:run_id",
            get(get_run_checkpoint),
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
        }),
    )
}

async fn start_run(
    State(state): State<AppState>,
    Json(body): Json<StartRunBody>,
) -> Result<(StatusCode, Json<RunResponse>), ApiError> {
    let run = state
        .orchestrator
        .start_run(StartRunRequest {
            worker_id: body.worker_id,
            metadata: body.metadata,
        })
        .await
        .map_err(ApiError::from_orchestration)?;
    Ok((StatusCode::CREATED, Json(RunResponse { run })))
}

async fn append_run_event(
    State(state): State<AppState>,
    Path(run_id): Path<Uuid>,
    Json(body): Json<AppendRunEventBody>,
) -> Result<Json<RunResponse>, ApiError> {
    let run = state
        .orchestrator
        .append_run_event(
            run_id,
            AppendRunEventRequest {
                event_type: body.event_type,
                payload: body.payload,
            },
        )
        .await
        .map_err(ApiError::from_orchestration)?;
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

async fn register_worker(
    State(state): State<AppState>,
    Json(body): Json<RegisterWorkerBody>,
) -> Result<(StatusCode, Json<WorkerResponse>), ApiError> {
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
    Ok(Json(WorkerResponse { worker: snapshot }))
}

async fn transition_worker(
    State(state): State<AppState>,
    Path(worker_id): Path<String>,
    Json(body): Json<WorkerTransitionBody>,
) -> Result<Json<WorkerResponse>, ApiError> {
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
        authority::InMemoryRuntimeAuthority, config::Config, orchestration::RuntimeOrchestrator,
        projectors::InMemoryProjectionPipeline, workers::InMemoryWorkerRegistry,
    };

    fn loopback_bind_addr() -> std::net::SocketAddr {
        std::net::SocketAddr::from(([127, 0, 0, 1], 0))
    }

    fn test_router() -> axum::Router {
        let projectors = InMemoryProjectionPipeline::shared();
        let projector_pipeline: Arc<dyn crate::projectors::ProjectionPipeline> = projectors.clone();
        let state = AppState::new(
            Config {
                service_name: "runtime-test".to_string(),
                bind_addr: loopback_bind_addr(),
                build_sha: "test".to_string(),
            },
            Arc::new(RuntimeOrchestrator::new(
                InMemoryRuntimeAuthority::shared(),
                projector_pipeline,
            )),
            Arc::new(InMemoryWorkerRegistry::new(projectors, 120_000)),
        );
        build_router(state)
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
