use std::{
    collections::{HashMap, VecDeque, hash_map::DefaultHasher},
    hash::{Hash, Hasher},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode, header},
    response::IntoResponse,
    routing::{get, post},
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::{
    artifacts::{ArtifactError, RuntimeReceipt, build_receipt, build_replay_jsonl},
    authority::AuthorityError,
    config::Config,
    fanout::{ExternalFanoutHook, FanoutError, FanoutHub, FanoutMessage, FanoutTopicWindow},
    orchestration::{OrchestrationError, RuntimeOrchestrator},
    sync_auth::{AuthorizedKhalaTopic, SyncAuthError, SyncAuthorizer, SyncPrincipal},
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
    sync_auth: Arc<SyncAuthorizer>,
    khala_delivery: Arc<KhalaDeliveryControl>,
    started_at: chrono::DateTime<Utc>,
}

impl AppState {
    #[must_use]
    pub fn new(
        config: Config,
        orchestrator: Arc<RuntimeOrchestrator>,
        workers: Arc<InMemoryWorkerRegistry>,
        fanout: Arc<FanoutHub>,
        sync_auth: Arc<SyncAuthorizer>,
    ) -> Self {
        Self {
            config,
            orchestrator,
            workers,
            fanout,
            sync_auth,
            khala_delivery: Arc::new(KhalaDeliveryControl::default()),
            started_at: Utc::now(),
        }
    }
}

#[derive(Debug, Clone)]
struct KhalaConsumerState {
    last_poll_at: Option<chrono::DateTime<Utc>>,
    last_cursor: u64,
    slow_consumer_strikes: u32,
}

impl Default for KhalaConsumerState {
    fn default() -> Self {
        Self {
            last_poll_at: None,
            last_cursor: 0,
            slow_consumer_strikes: 0,
        }
    }
}

#[derive(Default)]
struct KhalaDeliveryControl {
    consumers: Mutex<HashMap<String, KhalaConsumerState>>,
    recent_disconnect_causes: Mutex<VecDeque<String>>,
    total_polls: AtomicU64,
    throttled_polls: AtomicU64,
    limited_polls: AtomicU64,
    slow_consumer_evictions: AtomicU64,
    served_messages: AtomicU64,
}

#[derive(Debug, Serialize)]
struct KhalaDeliveryMetricsSnapshot {
    total_polls: u64,
    throttled_polls: u64,
    limited_polls: u64,
    slow_consumer_evictions: u64,
    served_messages: u64,
    active_consumers: usize,
    recent_disconnect_causes: Vec<String>,
}

impl KhalaDeliveryControl {
    fn record_total_poll(&self, served_messages: usize) {
        self.total_polls.fetch_add(1, Ordering::Relaxed);
        self.served_messages
            .fetch_add(served_messages as u64, Ordering::Relaxed);
    }

    fn record_throttled_poll(&self) {
        self.throttled_polls.fetch_add(1, Ordering::Relaxed);
    }

    fn record_limit_capped(&self) {
        self.limited_polls.fetch_add(1, Ordering::Relaxed);
    }

    fn record_slow_consumer_eviction(&self) {
        self.slow_consumer_evictions.fetch_add(1, Ordering::Relaxed);
    }

    async fn record_disconnect_cause(&self, cause: &str) {
        let mut causes = self.recent_disconnect_causes.lock().await;
        causes.push_back(cause.to_string());
        while causes.len() > 32 {
            let _ = causes.pop_front();
        }
    }

    async fn snapshot(&self) -> KhalaDeliveryMetricsSnapshot {
        let active_consumers = self.consumers.lock().await.len();
        let recent_disconnect_causes = self
            .recent_disconnect_causes
            .lock()
            .await
            .iter()
            .cloned()
            .collect::<Vec<_>>();
        KhalaDeliveryMetricsSnapshot {
            total_polls: self.total_polls.load(Ordering::Relaxed),
            throttled_polls: self.throttled_polls.load(Ordering::Relaxed),
            limited_polls: self.limited_polls.load(Ordering::Relaxed),
            slow_consumer_evictions: self.slow_consumer_evictions.load(Ordering::Relaxed),
            served_messages: self.served_messages.load(Ordering::Relaxed),
            active_consumers,
            recent_disconnect_causes,
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
    oldest_available_cursor: Option<u64>,
    head_cursor: Option<u64>,
    queue_depth: Option<usize>,
    dropped_messages: Option<u64>,
    next_cursor: u64,
    replay_complete: bool,
    limit_applied: usize,
    limit_capped: bool,
    consumer_lag: Option<u64>,
    slow_consumer_strikes: u32,
    slow_consumer_max_strikes: u32,
    recommended_reconnect_backoff_ms: u64,
}

#[derive(Debug, Serialize)]
struct FanoutHooksResponse {
    driver: String,
    hooks: Vec<ExternalFanoutHook>,
    delivery_metrics: KhalaDeliveryMetricsSnapshot,
    topic_windows: Vec<FanoutTopicWindow>,
}

#[derive(Debug, Serialize)]
struct FanoutMetricsResponse {
    driver: String,
    delivery_metrics: KhalaDeliveryMetricsSnapshot,
    topic_windows: Vec<FanoutTopicWindow>,
}

#[derive(Debug, Deserialize)]
struct FanoutMetricsQuery {
    topic_limit: Option<usize>,
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
            "/internal/v1/khala/fanout/metrics",
            get(get_khala_fanout_metrics),
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
    headers: HeaderMap,
    State(state): State<AppState>,
    Path(topic): Path<String>,
    Query(query): Query<FanoutPollQuery>,
) -> Result<Json<FanoutPollResponse>, ApiError> {
    if topic.trim().is_empty() {
        return Err(ApiError::InvalidRequest(
            "topic is required for khala fanout polling".to_string(),
        ));
    }
    let principal = authorize_khala_topic_access(&state, &headers, &topic).await?;
    let after_seq = query.after_seq.unwrap_or(0);
    let requested_limit = query
        .limit
        .unwrap_or(state.config.khala_poll_default_limit)
        .max(1);
    let limit = requested_limit.min(state.config.khala_poll_max_limit);
    let limit_capped = requested_limit > limit;
    if limit_capped {
        state.khala_delivery.record_limit_capped();
    }
    let window = state
        .fanout
        .topic_window(&topic)
        .await
        .map_err(ApiError::from_fanout)?;
    let (oldest_available_cursor, head_cursor, queue_depth, dropped_messages) =
        fanout_window_details(window.as_ref());
    let consumer_lag = head_cursor.map(|head| head.saturating_sub(after_seq));
    let consumer_key = khala_consumer_key(&principal, topic.as_str());
    let now = Utc::now();
    let jitter_ms = deterministic_jitter_ms(
        consumer_key.as_str(),
        after_seq,
        state.config.khala_reconnect_jitter_ms,
    );
    let reconnect_backoff_ms = state
        .config
        .khala_reconnect_base_backoff_ms
        .saturating_add(jitter_ms);

    let slow_consumer_strikes = {
        let mut consumers = state.khala_delivery.consumers.lock().await;
        if !consumers.contains_key(&consumer_key)
            && consumers.len() >= state.config.khala_consumer_registry_capacity
            && let Some(oldest_key) = consumers
                .iter()
                .min_by_key(|(_, value)| value.last_poll_at)
                .map(|(key, _)| key.clone())
        {
            let _ = consumers.remove(&oldest_key);
        }
        let consumer_state = consumers.entry(consumer_key.clone()).or_default();
        if let Some(last_poll_at) = consumer_state.last_poll_at {
            let elapsed_ms = now
                .signed_duration_since(last_poll_at)
                .num_milliseconds()
                .max(0) as u64;
            if elapsed_ms < state.config.khala_poll_min_interval_ms {
                consumer_state.last_poll_at = Some(now);
                drop(consumers);
                state.khala_delivery.record_throttled_poll();
                state
                    .khala_delivery
                    .record_disconnect_cause("rate_limited")
                    .await;
                let retry_after_ms = state
                    .config
                    .khala_poll_min_interval_ms
                    .saturating_sub(elapsed_ms)
                    .saturating_add(jitter_ms);
                return Err(ApiError::RateLimited {
                    retry_after_ms,
                    reason_code: "poll_interval_guard".to_string(),
                });
            }
        }

        let lag = consumer_lag.unwrap_or(0);
        if lag > state.config.khala_slow_consumer_lag_threshold {
            consumer_state.slow_consumer_strikes =
                consumer_state.slow_consumer_strikes.saturating_add(1);
        } else {
            consumer_state.slow_consumer_strikes = 0;
        }
        if consumer_state.slow_consumer_strikes >= state.config.khala_slow_consumer_max_strikes {
            let strikes = consumer_state.slow_consumer_strikes;
            let _ = consumers.remove(&consumer_key);
            drop(consumers);
            state.khala_delivery.record_slow_consumer_eviction();
            state
                .khala_delivery
                .record_disconnect_cause("slow_consumer_evicted")
                .await;
            return Err(ApiError::SlowConsumerEvicted {
                topic: topic.clone(),
                lag,
                lag_threshold: state.config.khala_slow_consumer_lag_threshold,
                strikes,
                max_strikes: state.config.khala_slow_consumer_max_strikes,
                suggested_after_seq: oldest_available_cursor,
            });
        }

        consumer_state.last_poll_at = Some(now);
        consumer_state.slow_consumer_strikes
    };

    let messages = state
        .fanout
        .poll(&topic, after_seq, limit)
        .await
        .map_err(ApiError::from_fanout)?;
    state.khala_delivery.record_total_poll(messages.len());
    let next_cursor = messages
        .last()
        .map_or(after_seq, |message| message.sequence);
    {
        let mut consumers = state.khala_delivery.consumers.lock().await;
        if let Some(consumer_state) = consumers.get_mut(&consumer_key) {
            consumer_state.last_cursor = next_cursor;
            consumer_state.last_poll_at = Some(now);
        }
    }
    let replay_complete = head_cursor.map_or(true, |head| next_cursor >= head);
    Ok(Json(FanoutPollResponse {
        topic,
        driver: state.fanout.driver_name().to_string(),
        messages,
        oldest_available_cursor,
        head_cursor,
        queue_depth,
        dropped_messages,
        next_cursor,
        replay_complete,
        limit_applied: limit,
        limit_capped,
        consumer_lag,
        slow_consumer_strikes,
        slow_consumer_max_strikes: state.config.khala_slow_consumer_max_strikes,
        recommended_reconnect_backoff_ms: reconnect_backoff_ms,
    }))
}

async fn get_khala_fanout_hooks(
    State(state): State<AppState>,
) -> Result<Json<FanoutHooksResponse>, ApiError> {
    let delivery_metrics = state.khala_delivery.snapshot().await;
    let topic_windows = state
        .fanout
        .topic_windows(20)
        .await
        .map_err(ApiError::from_fanout)?;
    Ok(Json(FanoutHooksResponse {
        driver: state.fanout.driver_name().to_string(),
        hooks: state.fanout.external_hooks(),
        delivery_metrics,
        topic_windows,
    }))
}

async fn get_khala_fanout_metrics(
    State(state): State<AppState>,
    Query(query): Query<FanoutMetricsQuery>,
) -> Result<Json<FanoutMetricsResponse>, ApiError> {
    let delivery_metrics = state.khala_delivery.snapshot().await;
    let topic_windows = state
        .fanout
        .topic_windows(query.topic_limit.unwrap_or(20))
        .await
        .map_err(ApiError::from_fanout)?;
    Ok(Json(FanoutMetricsResponse {
        driver: state.fanout.driver_name().to_string(),
        delivery_metrics,
        topic_windows,
    }))
}

async fn authorize_khala_topic_access(
    state: &AppState,
    headers: &HeaderMap,
    topic: &str,
) -> Result<SyncPrincipal, ApiError> {
    let authorization_header = headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok());
    let token = SyncAuthorizer::extract_bearer_token(authorization_header)
        .map_err(ApiError::from_sync_auth)?;
    let principal = state
        .sync_auth
        .authenticate(token)
        .map_err(ApiError::from_sync_auth)?;
    let authorized_topic = state
        .sync_auth
        .authorize_topic(&principal, topic)
        .map_err(ApiError::from_sync_auth)?;

    match authorized_topic {
        AuthorizedKhalaTopic::WorkerLifecycle { worker_id } => {
            let owner = WorkerOwner {
                user_id: principal.user_id,
                guest_scope: if principal.user_id.is_some() {
                    None
                } else {
                    principal.org_id.clone()
                },
            };
            match state.workers.get_worker(&worker_id, &owner).await {
                Ok(_) => {}
                Err(WorkerError::NotFound(_)) | Err(WorkerError::Forbidden(_)) => {
                    tracing::warn!(
                        topic,
                        worker_id,
                        user_id = principal.user_id,
                        org_id = ?principal.org_id,
                        device_id = ?principal.device_id,
                        "khala auth denied: worker owner mismatch"
                    );
                    return Err(ApiError::KhalaForbiddenTopic("owner_mismatch".to_string()));
                }
                Err(error) => {
                    tracing::warn!(
                        topic,
                        worker_id,
                        user_id = principal.user_id,
                        org_id = ?principal.org_id,
                        device_id = ?principal.device_id,
                        reason = %error,
                        "khala auth denied while validating worker ownership"
                    );
                    return Err(ApiError::KhalaForbiddenTopic(error.to_string()));
                }
            }
        }
        AuthorizedKhalaTopic::RunEvents { .. } | AuthorizedKhalaTopic::CodexWorkerEvents => {}
    }

    Ok(principal)
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
    KhalaUnauthorized(String),
    KhalaForbiddenTopic(String),
    PublishRateLimited {
        retry_after_ms: u64,
        reason_code: String,
        topic: String,
        topic_class: String,
        max_publish_per_second: u32,
    },
    PayloadTooLarge {
        reason_code: String,
        topic: String,
        topic_class: String,
        payload_bytes: usize,
        max_payload_bytes: usize,
    },
    RateLimited {
        retry_after_ms: u64,
        reason_code: String,
    },
    SlowConsumerEvicted {
        topic: String,
        lag: u64,
        lag_threshold: u64,
        strikes: u32,
        max_strikes: u32,
        suggested_after_seq: Option<u64>,
    },
    StaleCursor {
        topic: String,
        requested_cursor: u64,
        oldest_available_cursor: u64,
        head_cursor: u64,
    },
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

    fn from_fanout(error: FanoutError) -> Self {
        match error {
            FanoutError::InvalidTopic => {
                Self::InvalidRequest("topic is required for khala fanout operations".to_string())
            }
            FanoutError::StaleCursor {
                topic,
                requested_cursor,
                oldest_available_cursor,
                head_cursor,
            } => Self::StaleCursor {
                topic,
                requested_cursor,
                oldest_available_cursor,
                head_cursor,
            },
            FanoutError::PublishRateLimited {
                topic,
                topic_class,
                reason_code,
                max_publish_per_second,
                retry_after_ms,
            } => {
                tracing::warn!(
                    topic,
                    topic_class,
                    reason_code,
                    max_publish_per_second,
                    retry_after_ms,
                    "khala publish rate limit triggered"
                );
                Self::PublishRateLimited {
                    retry_after_ms,
                    reason_code,
                    topic,
                    topic_class,
                    max_publish_per_second,
                }
            }
            FanoutError::FramePayloadTooLarge {
                topic,
                topic_class,
                reason_code,
                payload_bytes,
                max_payload_bytes,
            } => {
                tracing::warn!(
                    topic,
                    topic_class,
                    reason_code,
                    payload_bytes,
                    max_payload_bytes,
                    "khala publish payload exceeds frame-size limit"
                );
                Self::PayloadTooLarge {
                    reason_code,
                    topic,
                    topic_class,
                    payload_bytes,
                    max_payload_bytes,
                }
            }
        }
    }

    fn from_sync_auth(error: SyncAuthError) -> Self {
        let code = error.code();
        if error.is_unauthorized() {
            tracing::warn!(reason_code = code, reason = %error, "khala auth denied");
            Self::KhalaUnauthorized(code.to_string())
        } else {
            tracing::warn!(reason_code = code, reason = %error, "khala topic denied");
            Self::KhalaForbiddenTopic(code.to_string())
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

fn fanout_window_details(
    window: Option<&FanoutTopicWindow>,
) -> (Option<u64>, Option<u64>, Option<usize>, Option<u64>) {
    match window {
        Some(window) => (
            Some(window.oldest_sequence.saturating_sub(1)),
            Some(window.head_sequence),
            Some(window.queue_depth),
            Some(window.dropped_messages),
        ),
        None => (None, None, None, None),
    }
}

fn khala_consumer_key(principal: &SyncPrincipal, topic: &str) -> String {
    let user = principal
        .user_id
        .map(|value| format!("user:{value}"))
        .unwrap_or_else(|| "user:none".to_string());
    let org = principal
        .org_id
        .clone()
        .unwrap_or_else(|| "org:none".to_string());
    let device = principal
        .device_id
        .clone()
        .unwrap_or_else(|| "device:none".to_string());
    format!("{topic}|{user}|{org}|{device}")
}

fn deterministic_jitter_ms(seed_key: &str, cursor: u64, max_jitter_ms: u64) -> u64 {
    if max_jitter_ms == 0 {
        return 0;
    }
    let mut hasher = DefaultHasher::new();
    seed_key.hash(&mut hasher);
    cursor.hash(&mut hasher);
    hasher.finish() % (max_jitter_ms.saturating_add(1))
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
        .map_err(ApiError::from_fanout)
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
        .map_err(ApiError::from_fanout)
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
            Self::KhalaUnauthorized(reason_code) => (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({
                    "error": "unauthorized",
                    "message": "khala sync authorization failed",
                    "reason_code": reason_code,
                })),
            )
                .into_response(),
            Self::KhalaForbiddenTopic(reason_code) => (
                StatusCode::FORBIDDEN,
                Json(serde_json::json!({
                    "error": "forbidden_topic",
                    "message": "topic subscription is not authorized",
                    "reason_code": reason_code,
                })),
            )
                .into_response(),
            Self::PublishRateLimited {
                retry_after_ms,
                reason_code,
                topic,
                topic_class,
                max_publish_per_second,
            } => (
                StatusCode::TOO_MANY_REQUESTS,
                Json(serde_json::json!({
                    "error": "rate_limited",
                    "message": "khala publish rate limit exceeded",
                    "reason_code": reason_code,
                    "retry_after_ms": retry_after_ms,
                    "topic": topic,
                    "topic_class": topic_class,
                    "max_publish_per_second": max_publish_per_second,
                })),
            )
                .into_response(),
            Self::PayloadTooLarge {
                reason_code,
                topic,
                topic_class,
                payload_bytes,
                max_payload_bytes,
            } => (
                StatusCode::PAYLOAD_TOO_LARGE,
                Json(serde_json::json!({
                    "error": "payload_too_large",
                    "message": "khala frame payload exceeds configured limit",
                    "reason_code": reason_code,
                    "topic": topic,
                    "topic_class": topic_class,
                    "payload_bytes": payload_bytes,
                    "max_payload_bytes": max_payload_bytes,
                })),
            )
                .into_response(),
            Self::RateLimited {
                retry_after_ms,
                reason_code,
            } => (
                StatusCode::TOO_MANY_REQUESTS,
                Json(serde_json::json!({
                    "error": "rate_limited",
                    "message": "poll interval guard triggered",
                    "reason_code": reason_code,
                    "retry_after_ms": retry_after_ms,
                })),
            )
                .into_response(),
            Self::SlowConsumerEvicted {
                topic,
                lag,
                lag_threshold,
                strikes,
                max_strikes,
                suggested_after_seq,
            } => (
                StatusCode::CONFLICT,
                Json(serde_json::json!({
                    "error": "slow_consumer_evicted",
                    "message": "consumer lag exceeded threshold repeatedly",
                    "reason_code": "slow_consumer_evicted",
                    "details": {
                        "topic": topic,
                        "lag": lag,
                        "lag_threshold": lag_threshold,
                        "strikes": strikes,
                        "max_strikes": max_strikes,
                        "suggested_after_seq": suggested_after_seq,
                        "recovery": "advance_cursor_or_rebootstrap"
                    }
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
            Self::StaleCursor {
                topic,
                requested_cursor,
                oldest_available_cursor,
                head_cursor,
            } => (
                StatusCode::GONE,
                Json(serde_json::json!({
                    "error": "stale_cursor",
                    "message": "cursor is older than retained stream window",
                    "details": {
                        "topic": topic,
                        "requested_cursor": requested_cursor,
                        "oldest_available_cursor": oldest_available_cursor,
                        "head_cursor": head_cursor,
                        "recovery": "reset_local_watermark_and_replay_bootstrap"
                    },
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
    use std::collections::HashSet;
    use std::sync::Arc;
    use std::time::Duration;

    use anyhow::{Result, anyhow};
    use axum::{
        body::Body,
        http::{Method, Request},
    };
    use chrono::Utc;
    use http_body_util::BodyExt;
    use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};
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
        sync_auth::{SyncAuthConfig, SyncAuthorizer, SyncTokenClaims},
        workers::InMemoryWorkerRegistry,
    };

    const TEST_SYNC_SIGNING_KEY: &str = "runtime-sync-test-key";

    fn loopback_bind_addr() -> std::net::SocketAddr {
        std::net::SocketAddr::from(([127, 0, 0, 1], 0))
    }

    fn build_test_router_with_config(
        mode: AuthorityWriteMode,
        revoked_jtis: HashSet<String>,
        mutate_config: impl FnOnce(&mut Config),
    ) -> axum::Router {
        let projectors = InMemoryProjectionPipeline::shared();
        let projector_pipeline: Arc<dyn crate::projectors::ProjectionPipeline> = projectors.clone();
        let revoked_for_config = revoked_jtis.clone();
        let mut config = Config {
            service_name: "runtime-test".to_string(),
            bind_addr: loopback_bind_addr(),
            build_sha: "test".to_string(),
            authority_write_mode: mode,
            fanout_driver: "memory".to_string(),
            fanout_queue_capacity: 64,
            khala_poll_default_limit: 100,
            khala_poll_max_limit: 200,
            khala_poll_min_interval_ms: 250,
            khala_slow_consumer_lag_threshold: 300,
            khala_slow_consumer_max_strikes: 3,
            khala_consumer_registry_capacity: 4096,
            khala_reconnect_base_backoff_ms: 400,
            khala_reconnect_jitter_ms: 250,
            khala_run_events_publish_rate_per_second: 240,
            khala_worker_lifecycle_publish_rate_per_second: 180,
            khala_codex_worker_events_publish_rate_per_second: 240,
            khala_fallback_publish_rate_per_second: 90,
            khala_run_events_max_payload_bytes: 256 * 1024,
            khala_worker_lifecycle_max_payload_bytes: 64 * 1024,
            khala_codex_worker_events_max_payload_bytes: 128 * 1024,
            khala_fallback_max_payload_bytes: 64 * 1024,
            sync_token_signing_key: TEST_SYNC_SIGNING_KEY.to_string(),
            sync_token_issuer: "https://openagents.com".to_string(),
            sync_token_audience: "openagents-sync".to_string(),
            sync_revoked_jtis: revoked_for_config,
        };
        mutate_config(&mut config);
        let fanout_limits = config.khala_fanout_limits();
        let fanout_capacity = config.fanout_queue_capacity;
        let state = AppState::new(
            config,
            Arc::new(RuntimeOrchestrator::new(
                Arc::new(InMemoryRuntimeAuthority::with_event_log(
                    DurableEventLog::new_memory(),
                )),
                projector_pipeline,
            )),
            Arc::new(InMemoryWorkerRegistry::new(projectors, 120_000)),
            Arc::new(FanoutHub::memory_with_limits(
                fanout_capacity,
                fanout_limits,
            )),
            Arc::new(SyncAuthorizer::from_config(SyncAuthConfig {
                signing_key: TEST_SYNC_SIGNING_KEY.to_string(),
                issuer: "https://openagents.com".to_string(),
                audience: "openagents-sync".to_string(),
                revoked_jtis,
            })),
        );
        build_router(state)
    }

    fn test_router_with_mode_and_revoked(
        mode: AuthorityWriteMode,
        revoked_jtis: HashSet<String>,
    ) -> axum::Router {
        build_test_router_with_config(mode, revoked_jtis, |_| {})
    }

    fn test_router_with_mode(mode: AuthorityWriteMode) -> axum::Router {
        test_router_with_mode_and_revoked(mode, HashSet::new())
    }

    fn test_router() -> axum::Router {
        test_router_with_mode(AuthorityWriteMode::RustActive)
    }

    async fn response_json(response: axum::response::Response) -> Result<Value> {
        let collected = response.into_body().collect().await?;
        let bytes = collected.to_bytes();
        Ok(serde_json::from_slice(&bytes)?)
    }

    fn issue_sync_token(
        scopes: &[&str],
        user_id: Option<u64>,
        org_id: Option<&str>,
        jti: &str,
        exp_offset_seconds: i64,
    ) -> String {
        let now = Utc::now().timestamp();
        let claims = SyncTokenClaims {
            iss: "https://openagents.com".to_string(),
            aud: "openagents-sync".to_string(),
            sub: match user_id {
                Some(value) => format!("user:{value}"),
                None => "guest:unknown".to_string(),
            },
            exp: (now + exp_offset_seconds) as usize,
            nbf: now as usize,
            iat: now as usize,
            jti: jti.to_string(),
            oa_user_id: user_id,
            oa_org_id: org_id.map(ToString::to_string),
            oa_device_id: Some("ios-device".to_string()),
            oa_sync_scopes: scopes.iter().map(|scope| (*scope).to_string()).collect(),
        };
        encode(
            &Header::new(Algorithm::HS256),
            &claims,
            &EncodingKey::from_secret(TEST_SYNC_SIGNING_KEY.as_bytes()),
        )
        .expect("sync token should encode")
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
        let sync_token = issue_sync_token(
            &["runtime.run_events"],
            Some(1),
            Some("user:1"),
            "fanout-ok",
            300,
        );
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
                    .header("authorization", format!("Bearer {sync_token}"))
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
        assert_eq!(
            messages_json
                .pointer("/oldest_available_cursor")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or_default(),
            0
        );
        assert_eq!(
            messages_json
                .pointer("/head_cursor")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or_default(),
            1
        );
        assert_eq!(
            messages_json
                .pointer("/next_cursor")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or_default(),
            1
        );
        assert_eq!(
            messages_json
                .pointer("/replay_complete")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false),
            true
        );
        assert_eq!(
            messages_json
                .pointer("/queue_depth")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or_default(),
            1
        );
        assert_eq!(
            messages_json
                .pointer("/limit_applied")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or_default(),
            10
        );
        assert_eq!(
            messages_json
                .pointer("/limit_capped")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false),
            false
        );

        let hooks_response = app
            .clone()
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
        assert_eq!(
            hooks_json
                .pointer("/delivery_metrics/total_polls")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or_default(),
            1
        );
        let topic_window_count = hooks_json
            .pointer("/topic_windows")
            .and_then(serde_json::Value::as_array)
            .map_or(0, std::vec::Vec::len);
        assert!(topic_window_count >= 1);

        let metrics_response = app
            .oneshot(
                Request::builder()
                    .uri("/internal/v1/khala/fanout/metrics?topic_limit=5")
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(metrics_response.status(), axum::http::StatusCode::OK);
        let metrics_json = response_json(metrics_response).await?;
        assert_eq!(
            metrics_json
                .pointer("/delivery_metrics/total_polls")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or_default(),
            1
        );

        Ok(())
    }

    #[tokio::test]
    async fn khala_topic_messages_rate_limits_fast_pollers() -> Result<()> {
        let app = test_router();
        let sync_token = issue_sync_token(
            &["runtime.run_events"],
            Some(1),
            Some("user:1"),
            "poll-rate-limit",
            300,
        );
        let create_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/internal/v1/runs")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&serde_json::json!({
                        "worker_id": "desktop:poll-rate-limit-worker",
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

        let first_poll = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!(
                        "/internal/v1/khala/topics/run:{run_id}:events/messages?after_seq=0&limit=10"
                    ))
                    .header("authorization", format!("Bearer {sync_token}"))
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(first_poll.status(), axum::http::StatusCode::OK);

        let second_poll = app
            .oneshot(
                Request::builder()
                    .uri(format!(
                        "/internal/v1/khala/topics/run:{run_id}:events/messages?after_seq=1&limit=10"
                    ))
                    .header("authorization", format!("Bearer {sync_token}"))
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(
            second_poll.status(),
            axum::http::StatusCode::TOO_MANY_REQUESTS
        );
        let error_json = response_json(second_poll).await?;
        assert_eq!(
            error_json
                .pointer("/error")
                .and_then(serde_json::Value::as_str)
                .unwrap_or(""),
            "rate_limited"
        );
        assert_eq!(
            error_json
                .pointer("/reason_code")
                .and_then(serde_json::Value::as_str)
                .unwrap_or(""),
            "poll_interval_guard"
        );
        assert!(
            error_json
                .pointer("/retry_after_ms")
                .and_then(serde_json::Value::as_u64)
                .is_some()
        );

        Ok(())
    }

    #[tokio::test]
    async fn khala_topic_messages_evict_slow_consumers_deterministically() -> Result<()> {
        let app = build_test_router_with_config(
            AuthorityWriteMode::RustActive,
            HashSet::new(),
            |config| {
                config.khala_poll_min_interval_ms = 1;
                config.khala_slow_consumer_lag_threshold = 2;
                config.khala_slow_consumer_max_strikes = 2;
                config.khala_poll_default_limit = 1;
                config.khala_poll_max_limit = 1;
            },
        );
        let sync_token = issue_sync_token(
            &["runtime.run_events"],
            Some(1),
            Some("user:1"),
            "slow-consumer-evict",
            300,
        );
        let create_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/internal/v1/runs")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&serde_json::json!({
                        "worker_id": "desktop:slow-consumer-worker",
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

        for index in 2..=7 {
            let response = app
                .clone()
                .oneshot(
                    Request::builder()
                        .method(Method::POST)
                        .uri(format!("/internal/v1/runs/{run_id}/events"))
                        .header("content-type", "application/json")
                        .body(Body::from(serde_json::to_vec(&serde_json::json!({
                            "event_type": "run.step",
                            "payload": {"step": index},
                            "expected_previous_seq": index - 1
                        }))?))?,
                )
                .await?;
            assert_eq!(response.status(), axum::http::StatusCode::OK);
        }

        let first_poll = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!(
                        "/internal/v1/khala/topics/run:{run_id}:events/messages?after_seq=0&limit=1"
                    ))
                    .header("authorization", format!("Bearer {sync_token}"))
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(first_poll.status(), axum::http::StatusCode::OK);
        let first_json = response_json(first_poll).await?;
        assert_eq!(
            first_json
                .pointer("/slow_consumer_strikes")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or_default(),
            1
        );

        tokio::time::sleep(Duration::from_millis(5)).await;

        let second_poll = app
            .oneshot(
                Request::builder()
                    .uri(format!(
                        "/internal/v1/khala/topics/run:{run_id}:events/messages?after_seq=0&limit=1"
                    ))
                    .header("authorization", format!("Bearer {sync_token}"))
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(second_poll.status(), axum::http::StatusCode::CONFLICT);
        let second_json = response_json(second_poll).await?;
        assert_eq!(
            second_json
                .pointer("/error")
                .and_then(serde_json::Value::as_str)
                .unwrap_or(""),
            "slow_consumer_evicted"
        );
        assert_eq!(
            second_json
                .pointer("/details/strikes")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or_default(),
            2
        );
        assert_eq!(
            second_json
                .pointer("/details/recovery")
                .and_then(serde_json::Value::as_str)
                .unwrap_or(""),
            "advance_cursor_or_rebootstrap"
        );

        Ok(())
    }

    #[tokio::test]
    async fn khala_publish_rate_limit_returns_deterministic_error() -> Result<()> {
        let app = build_test_router_with_config(
            AuthorityWriteMode::RustActive,
            HashSet::new(),
            |config| {
                config.khala_run_events_publish_rate_per_second = 1;
                config.khala_run_events_max_payload_bytes = 64 * 1024;
            },
        );
        let create_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/internal/v1/runs")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&serde_json::json!({
                        "worker_id": "desktop:publish-rate-limit-worker",
                        "metadata": {}
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
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri(format!("/internal/v1/runs/{run_id}/events"))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&serde_json::json!({
                        "event_type": "run.step",
                        "payload": {"step": 2},
                        "expected_previous_seq": 1
                    }))?))?,
            )
            .await?;
        assert_eq!(
            append_response.status(),
            axum::http::StatusCode::TOO_MANY_REQUESTS
        );
        let append_json = response_json(append_response).await?;
        assert_eq!(
            append_json
                .pointer("/error")
                .and_then(serde_json::Value::as_str)
                .unwrap_or(""),
            "rate_limited"
        );
        assert_eq!(
            append_json
                .pointer("/reason_code")
                .and_then(serde_json::Value::as_str)
                .unwrap_or(""),
            "khala_publish_rate_limited"
        );
        assert_eq!(
            append_json
                .pointer("/topic_class")
                .and_then(serde_json::Value::as_str)
                .unwrap_or(""),
            "run_events"
        );

        Ok(())
    }

    #[tokio::test]
    async fn khala_payload_limit_returns_payload_too_large_error() -> Result<()> {
        let app = build_test_router_with_config(
            AuthorityWriteMode::RustActive,
            HashSet::new(),
            |config| {
                config.khala_run_events_publish_rate_per_second = 50;
                config.khala_run_events_max_payload_bytes = 80;
            },
        );
        let create_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/internal/v1/runs")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&serde_json::json!({
                        "worker_id": "desktop:payload-limit-worker",
                        "metadata": {}
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
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri(format!("/internal/v1/runs/{run_id}/events"))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&serde_json::json!({
                        "event_type": "run.step",
                        "payload": {"blob": "this payload is intentionally much larger than eighty bytes to trigger the khala frame-size guard"},
                        "expected_previous_seq": 1
                    }))?))?,
            )
            .await?;
        assert_eq!(
            append_response.status(),
            axum::http::StatusCode::PAYLOAD_TOO_LARGE
        );
        let append_json = response_json(append_response).await?;
        assert_eq!(
            append_json
                .pointer("/error")
                .and_then(serde_json::Value::as_str)
                .unwrap_or(""),
            "payload_too_large"
        );
        assert_eq!(
            append_json
                .pointer("/reason_code")
                .and_then(serde_json::Value::as_str)
                .unwrap_or(""),
            "khala_frame_payload_too_large"
        );
        assert_eq!(
            append_json
                .pointer("/topic_class")
                .and_then(serde_json::Value::as_str)
                .unwrap_or(""),
            "run_events"
        );

        Ok(())
    }

    #[tokio::test]
    async fn khala_topic_messages_returns_stale_cursor_when_replay_floor_is_missed() -> Result<()> {
        let app = test_router();
        let sync_token = issue_sync_token(
            &["runtime.run_events"],
            Some(1),
            Some("user:1"),
            "stale-ok",
            300,
        );
        let create_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/internal/v1/runs")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&serde_json::json!({
                        "worker_id": "desktop:stale-cursor-worker",
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

        for index in 2..=80 {
            let response = app
                .clone()
                .oneshot(
                    Request::builder()
                        .method(Method::POST)
                        .uri(format!("/internal/v1/runs/{run_id}/events"))
                        .header("content-type", "application/json")
                        .body(Body::from(serde_json::to_vec(&serde_json::json!({
                            "event_type": "run.step",
                            "payload": {"step": index},
                            "expected_previous_seq": index - 1
                        }))?))?,
                )
                .await?;
            assert_eq!(response.status(), axum::http::StatusCode::OK);
        }

        let stale_response = app
            .oneshot(
                Request::builder()
                    .uri(format!(
                        "/internal/v1/khala/topics/run:{run_id}:events/messages?after_seq=0&limit=10"
                    ))
                    .header("authorization", format!("Bearer {sync_token}"))
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(stale_response.status(), axum::http::StatusCode::GONE);
        let stale_json = response_json(stale_response).await?;
        assert_eq!(
            stale_json
                .pointer("/error")
                .and_then(serde_json::Value::as_str)
                .unwrap_or(""),
            "stale_cursor"
        );
        assert_eq!(
            stale_json
                .pointer("/details/recovery")
                .and_then(serde_json::Value::as_str)
                .unwrap_or(""),
            "reset_local_watermark_and_replay_bootstrap"
        );

        Ok(())
    }

    #[tokio::test]
    async fn khala_topic_messages_requires_valid_sync_token() -> Result<()> {
        let app = test_router();
        let create_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/internal/v1/runs")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&serde_json::json!({
                        "worker_id": "desktop:auth-worker",
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

        let missing_auth = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!(
                        "/internal/v1/khala/topics/run:{run_id}:events/messages?after_seq=0&limit=10"
                    ))
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(missing_auth.status(), axum::http::StatusCode::UNAUTHORIZED);
        let missing_auth_json = response_json(missing_auth).await?;
        assert_eq!(
            missing_auth_json
                .pointer("/reason_code")
                .and_then(serde_json::Value::as_str)
                .unwrap_or(""),
            "missing_authorization"
        );

        let expired_token = issue_sync_token(
            &["runtime.run_events"],
            Some(1),
            Some("user:1"),
            "expired-token",
            -10,
        );
        let expired_auth = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!(
                        "/internal/v1/khala/topics/run:{run_id}:events/messages?after_seq=0&limit=10"
                    ))
                    .header("authorization", format!("Bearer {expired_token}"))
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(expired_auth.status(), axum::http::StatusCode::UNAUTHORIZED);
        let expired_json = response_json(expired_auth).await?;
        assert_eq!(
            expired_json
                .pointer("/reason_code")
                .and_then(serde_json::Value::as_str)
                .unwrap_or(""),
            "token_expired"
        );

        let revoked_router = test_router_with_mode_and_revoked(
            AuthorityWriteMode::RustActive,
            HashSet::from([String::from("revoked-jti")]),
        );
        let revoked_create = revoked_router
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/internal/v1/runs")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&serde_json::json!({
                        "worker_id": "desktop:revoked-worker",
                        "metadata": {}
                    }))?))?,
            )
            .await?;
        let revoked_create_json = response_json(revoked_create).await?;
        let revoked_run_id = revoked_create_json
            .pointer("/run/id")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| anyhow!("missing run id"))?
            .to_string();
        let revoked_token = issue_sync_token(
            &["runtime.run_events"],
            Some(1),
            Some("user:1"),
            "revoked-jti",
            300,
        );
        let revoked_response = revoked_router
            .oneshot(
                Request::builder()
                    .uri(format!(
                        "/internal/v1/khala/topics/run:{revoked_run_id}:events/messages?after_seq=0&limit=10"
                    ))
                    .header("authorization", format!("Bearer {revoked_token}"))
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(
            revoked_response.status(),
            axum::http::StatusCode::UNAUTHORIZED
        );
        let revoked_json = response_json(revoked_response).await?;
        assert_eq!(
            revoked_json
                .pointer("/reason_code")
                .and_then(serde_json::Value::as_str)
                .unwrap_or(""),
            "token_revoked"
        );

        Ok(())
    }

    #[tokio::test]
    async fn khala_topic_messages_enforce_scope_matrix() -> Result<()> {
        let app = test_router();
        let create_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/internal/v1/runs")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&serde_json::json!({
                        "worker_id": "desktop:scope-worker",
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

        let missing_scope_token = issue_sync_token(
            &["runtime.codex_worker_events"],
            Some(1),
            Some("user:1"),
            "scope-missing",
            300,
        );
        let denied = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!(
                        "/internal/v1/khala/topics/run:{run_id}:events/messages?after_seq=0&limit=10"
                    ))
                    .header("authorization", format!("Bearer {missing_scope_token}"))
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(denied.status(), axum::http::StatusCode::FORBIDDEN);
        let denied_json = response_json(denied).await?;
        assert_eq!(
            denied_json
                .pointer("/error")
                .and_then(serde_json::Value::as_str)
                .unwrap_or(""),
            "forbidden_topic"
        );
        assert_eq!(
            denied_json
                .pointer("/reason_code")
                .and_then(serde_json::Value::as_str)
                .unwrap_or(""),
            "missing_scope"
        );

        let allowed_token = issue_sync_token(
            &["runtime.run_events"],
            Some(1),
            Some("user:1"),
            "scope-allowed",
            300,
        );
        let allowed = app
            .oneshot(
                Request::builder()
                    .uri(format!(
                        "/internal/v1/khala/topics/run:{run_id}:events/messages?after_seq=0&limit=10"
                    ))
                    .header("authorization", format!("Bearer {allowed_token}"))
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(allowed.status(), axum::http::StatusCode::OK);

        Ok(())
    }

    #[tokio::test]
    async fn khala_worker_topic_enforces_owner_scope() -> Result<()> {
        let app = test_router();

        let create_worker = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/internal/v1/workers")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&serde_json::json!({
                        "worker_id": "desktop:owner-worker",
                        "owner_user_id": 11,
                        "metadata": {}
                    }))?))?,
            )
            .await?;
        assert_eq!(create_worker.status(), axum::http::StatusCode::CREATED);

        let denied_owner_token = issue_sync_token(
            &["runtime.codex_worker_events"],
            Some(22),
            Some("user:22"),
            "owner-mismatch",
            300,
        );
        let denied = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/internal/v1/khala/topics/worker:desktop:owner-worker:lifecycle/messages?after_seq=0&limit=10")
                    .header("authorization", format!("Bearer {denied_owner_token}"))
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(denied.status(), axum::http::StatusCode::FORBIDDEN);
        let denied_json = response_json(denied).await?;
        assert_eq!(
            denied_json
                .pointer("/reason_code")
                .and_then(serde_json::Value::as_str)
                .unwrap_or(""),
            "owner_mismatch"
        );

        let allowed_owner_token = issue_sync_token(
            &["runtime.codex_worker_events"],
            Some(11),
            Some("user:11"),
            "owner-allowed",
            300,
        );
        let allowed = app
            .oneshot(
                Request::builder()
                    .uri("/internal/v1/khala/topics/worker:desktop:owner-worker:lifecycle/messages?after_seq=0&limit=10")
                    .header("authorization", format!("Bearer {allowed_owner_token}"))
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(allowed.status(), axum::http::StatusCode::OK);

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
