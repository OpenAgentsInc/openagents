use std::{
    collections::{HashMap, VecDeque, hash_map::DefaultHasher},
    hash::{Hash, Hasher},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

use axum::{
    Json, Router,
    extract::{
        Path, Query, State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    http::{HeaderMap, StatusCode, header},
    response::IntoResponse,
    routing::{get, post},
};
use chrono::Utc;
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::{
    artifacts::{ArtifactError, RuntimeReceipt, build_receipt, build_replay_jsonl},
    authority::AuthorityError,
    bridge::{BridgeNostrPublisher, ProviderAdV1, build_provider_ad_event},
    config::Config,
    fanout::{ExternalFanoutHook, FanoutError, FanoutHub, FanoutMessage, FanoutTopicWindow},
    marketplace::{
        ProviderCatalogEntry, ProviderSelection, build_provider_catalog, is_provider_worker,
        select_provider_for_capability, select_provider_for_capability_excluding,
    },
    orchestration::{OrchestrationError, RuntimeOrchestrator},
    sync_auth::{AuthorizedKhalaTopic, SyncAuthError, SyncAuthorizer, SyncPrincipal},
    treasury::Treasury,
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
    compute_abuse: Arc<ComputeAbuseControls>,
    compute_telemetry: Arc<ComputeTelemetry>,
    treasury: Arc<Treasury>,
    fleet_seq: Arc<AtomicU64>,
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
            compute_abuse: Arc::new(ComputeAbuseControls::default()),
            compute_telemetry: Arc::new(ComputeTelemetry::default()),
            treasury: Arc::new(Treasury::default()),
            fleet_seq: Arc::new(AtomicU64::new(0)),
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
    fairness_limited_polls: AtomicU64,
    slow_consumer_evictions: AtomicU64,
    served_messages: AtomicU64,
}

#[derive(Default)]
struct ComputeAbuseControls {
    dispatch_window: Mutex<HashMap<String, VecDeque<chrono::DateTime<Utc>>>>,
}

const COMPUTE_DISPATCH_WINDOW_SECONDS: i64 = 60;
const COMPUTE_DISPATCH_MAX_PER_WINDOW: usize = 30;
const COMPUTE_TELEMETRY_LATENCY_SAMPLES: usize = 256;

impl ComputeAbuseControls {
    async fn enforce_dispatch_rate(&self, owner_key: &str) -> Result<(), ApiError> {
        let now = Utc::now();
        let window_start = now - chrono::Duration::seconds(COMPUTE_DISPATCH_WINDOW_SECONDS);

        let mut windows = self.dispatch_window.lock().await;
        let entries = windows.entry(owner_key.to_string()).or_default();
        while matches!(entries.front(), Some(front) if *front < window_start) {
            entries.pop_front();
        }
        if entries.len() >= COMPUTE_DISPATCH_MAX_PER_WINDOW {
            return Err(ApiError::RateLimited {
                retry_after_ms: 1_000,
                reason_code: "compute_dispatch_rate_limited".to_string(),
            });
        }
        entries.push_back(now);
        Ok(())
    }
}

#[derive(Default)]
struct ComputeTelemetry {
    owners: Mutex<HashMap<String, OwnerComputeTelemetry>>,
}

#[derive(Clone, Debug, Default)]
struct OwnerComputeTelemetry {
    dispatch_total: u64,
    dispatch_not_found: u64,
    dispatch_errors: u64,
    dispatch_fallbacks: u64,
    latencies_ms: VecDeque<u64>,
    updated_at: Option<chrono::DateTime<Utc>>,
}

#[derive(Clone, Debug, Serialize)]
struct OwnerComputeTelemetrySnapshot {
    dispatch_total: u64,
    dispatch_not_found: u64,
    dispatch_errors: u64,
    dispatch_fallbacks: u64,
    latency_ms_avg: Option<u64>,
    latency_ms_p50: Option<u64>,
    samples: usize,
    updated_at: Option<chrono::DateTime<Utc>>,
}

impl ComputeTelemetry {
    async fn record_dispatch_not_found(&self, owner_key: &str) {
        self.record(owner_key, |entry| {
            entry.dispatch_total = entry.dispatch_total.saturating_add(1);
            entry.dispatch_not_found = entry.dispatch_not_found.saturating_add(1);
        })
        .await;
    }

    async fn record_dispatch_error(&self, owner_key: &str) {
        self.record(owner_key, |entry| {
            entry.dispatch_total = entry.dispatch_total.saturating_add(1);
            entry.dispatch_errors = entry.dispatch_errors.saturating_add(1);
        })
        .await;
    }

    async fn record_dispatch_success(&self, owner_key: &str, latency_ms: u64, fallback: bool) {
        self.record(owner_key, |entry| {
            entry.dispatch_total = entry.dispatch_total.saturating_add(1);
            if fallback {
                entry.dispatch_fallbacks = entry.dispatch_fallbacks.saturating_add(1);
            }
            if entry.latencies_ms.len() >= COMPUTE_TELEMETRY_LATENCY_SAMPLES {
                entry.latencies_ms.pop_front();
            }
            entry.latencies_ms.push_back(latency_ms);
        })
        .await;
    }

    async fn snapshot(&self, owner_key: &str) -> OwnerComputeTelemetrySnapshot {
        let owners = self.owners.lock().await;
        let Some(entry) = owners.get(owner_key) else {
            return OwnerComputeTelemetrySnapshot {
                dispatch_total: 0,
                dispatch_not_found: 0,
                dispatch_errors: 0,
                dispatch_fallbacks: 0,
                latency_ms_avg: None,
                latency_ms_p50: None,
                samples: 0,
                updated_at: None,
            };
        };

        let samples = entry.latencies_ms.len();
        let latency_ms_avg = if samples == 0 {
            None
        } else {
            Some(entry.latencies_ms.iter().sum::<u64>() / samples as u64)
        };
        let latency_ms_p50 = if samples == 0 {
            None
        } else {
            let mut sorted = entry.latencies_ms.iter().copied().collect::<Vec<_>>();
            sorted.sort_unstable();
            Some(sorted[(samples - 1) / 2])
        };

        OwnerComputeTelemetrySnapshot {
            dispatch_total: entry.dispatch_total,
            dispatch_not_found: entry.dispatch_not_found,
            dispatch_errors: entry.dispatch_errors,
            dispatch_fallbacks: entry.dispatch_fallbacks,
            latency_ms_avg,
            latency_ms_p50,
            samples,
            updated_at: entry.updated_at,
        }
    }

    async fn record<F: FnOnce(&mut OwnerComputeTelemetry)>(&self, owner_key: &str, f: F) {
        let now = Utc::now();
        let mut owners = self.owners.lock().await;
        let entry = owners.entry(owner_key.to_string()).or_default();
        f(entry);
        entry.updated_at = Some(now);
    }
}

#[derive(Debug, Serialize)]
struct KhalaDeliveryMetricsSnapshot {
    total_polls: u64,
    throttled_polls: u64,
    limited_polls: u64,
    fairness_limited_polls: u64,
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

    fn record_fairness_limited(&self) {
        self.fairness_limited_polls.fetch_add(1, Ordering::Relaxed);
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
            fairness_limited_polls: self.fairness_limited_polls.load(Ordering::Relaxed),
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
    fairness_applied: bool,
    active_topic_count: usize,
    outbound_queue_limit: usize,
    consumer_lag: Option<u64>,
    slow_consumer_strikes: u32,
    slow_consumer_max_strikes: u32,
    recommended_reconnect_backoff_ms: u64,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum KhalaWsFrame {
    Hello {
        topic: String,
        after_seq: u64,
        limit: usize,
        recommended_reconnect_backoff_ms: u64,
    },
    Message {
        message: FanoutMessage,
    },
    StaleCursor {
        topic: String,
        requested_cursor: u64,
        oldest_available_cursor: u64,
        head_cursor: u64,
        reason_codes: Vec<String>,
        replay_lag: u64,
        replay_budget_events: u64,
        qos_tier: String,
    },
    Error {
        code: String,
        message: String,
    },
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

#[derive(Debug, Serialize)]
struct WorkersListResponse {
    workers: Vec<WorkerSnapshot>,
}

#[derive(Debug, Deserialize)]
struct ProviderCatalogQuery {
    owner_user_id: Option<u64>,
    owner_guest_scope: Option<String>,
    capability: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ComputeTelemetryQuery {
    owner_user_id: Option<u64>,
    owner_guest_scope: Option<String>,
    #[serde(default)]
    capability: Option<String>,
}

#[derive(Debug, Serialize)]
struct ProviderCatalogResponse {
    providers: Vec<ProviderCatalogEntry>,
}

#[derive(Debug, Serialize)]
struct JobTypesResponse {
    job_types: Vec<protocol::jobs::JobTypeInfo>,
}

#[derive(Debug, Deserialize)]
struct RouteProviderBody {
    owner_user_id: Option<u64>,
    owner_guest_scope: Option<String>,
    capability: String,
}

#[derive(Debug, Deserialize)]
struct DispatchSandboxRunBody {
    owner_user_id: Option<u64>,
    owner_guest_scope: Option<String>,
    request: protocol::SandboxRunRequest,
}

#[derive(Debug, Serialize)]
struct DispatchSandboxRunResponse {
    job_hash: String,
    selection: ProviderSelection,
    response: protocol::SandboxRunResponse,
    latency_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    fallback_from_provider_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SandboxVerificationBody {
    request: protocol::SandboxRunRequest,
    response: protocol::SandboxRunResponse,
}

#[derive(Debug, Serialize)]
struct SandboxVerificationResponse {
    passed: bool,
    exit_code: i32,
    violations: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct RepoIndexVerificationBody {
    request: protocol::RepoIndexRequest,
    response: protocol::RepoIndexResponse,
}

#[derive(Debug, Serialize)]
struct RepoIndexVerificationResponse {
    passed: bool,
    tree_sha256: String,
    violations: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct SettleSandboxRunBody {
    owner_user_id: Option<u64>,
    owner_guest_scope: Option<String>,
    run_id: Uuid,
    provider_id: String,
    provider_worker_id: String,
    amount_msats: u64,
    request: protocol::SandboxRunRequest,
    response: protocol::SandboxRunResponse,
}

#[derive(Debug, Serialize)]
struct SettleSandboxRunResponse {
    job_hash: String,
    reservation_id: String,
    amount_msats: u64,
    verification_passed: bool,
    exit_code: i32,
    #[serde(default)]
    violations: Vec<String>,
    settlement_status: String,
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
            "/internal/v1/khala/topics/:topic/ws",
            get(get_khala_topic_ws),
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
        .route(
            "/internal/v1/workers",
            get(list_workers).post(register_worker),
        )
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
        .route(
            "/internal/v1/marketplace/catalog/providers",
            get(get_provider_catalog),
        )
        .route(
            "/internal/v1/marketplace/catalog/job-types",
            get(get_job_types),
        )
        .route(
            "/internal/v1/marketplace/telemetry/compute",
            get(get_compute_telemetry),
        )
        .route(
            "/internal/v1/marketplace/route/provider",
            post(route_provider),
        )
        .route(
            "/internal/v1/marketplace/dispatch/sandbox-run",
            post(dispatch_sandbox_run),
        )
        .route(
            "/internal/v1/verifications/sandbox-run",
            post(verify_sandbox_run),
        )
        .route(
            "/internal/v1/verifications/repo-index",
            post(verify_repo_index),
        )
        .route(
            "/internal/v1/treasury/compute/summary",
            get(get_compute_treasury_summary),
        )
        .route(
            "/internal/v1/treasury/compute/settle/sandbox-run",
            post(settle_sandbox_run),
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
    if body.event_type.trim() == "payment" {
        return Err(ApiError::InvalidRequest(
            "payment events must be emitted via treasury settlement endpoints".to_string(),
        ));
    }
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
    enforce_khala_origin_policy(&state, &headers)?;
    let principal = authorize_khala_topic_access(&state, &headers, &topic).await?;
    let after_seq = query.after_seq.unwrap_or(0);
    let requested_limit = query
        .limit
        .unwrap_or(state.config.khala_poll_default_limit)
        .max(1);
    let principal_key = khala_principal_key(&principal);
    let active_topic_count = {
        let prefix = format!("{principal_key}|");
        let consumers = state.khala_delivery.consumers.lock().await;
        consumers
            .keys()
            .filter(|key| key.starts_with(prefix.as_str()))
            .count()
    };
    let mut limit = requested_limit
        .min(state.config.khala_poll_max_limit)
        .min(state.config.khala_outbound_queue_limit);
    let mut fairness_applied = false;
    if active_topic_count >= 2 && limit > state.config.khala_fair_topic_slice_limit {
        limit = state.config.khala_fair_topic_slice_limit;
        fairness_applied = true;
        state.khala_delivery.record_fairness_limited();
    }
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
        fairness_applied,
        active_topic_count,
        outbound_queue_limit: state.config.khala_outbound_queue_limit,
        consumer_lag,
        slow_consumer_strikes,
        slow_consumer_max_strikes: state.config.khala_slow_consumer_max_strikes,
        recommended_reconnect_backoff_ms: reconnect_backoff_ms,
    }))
}

async fn get_khala_topic_ws(
    headers: HeaderMap,
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    Path(topic): Path<String>,
    Query(query): Query<FanoutPollQuery>,
) -> Result<impl IntoResponse, ApiError> {
    if topic.trim().is_empty() {
        return Err(ApiError::InvalidRequest(
            "topic is required for khala websocket".to_string(),
        ));
    }
    enforce_khala_origin_policy(&state, &headers)?;
    let principal = authorize_khala_topic_access(&state, &headers, &topic).await?;
    let after_seq = query.after_seq.unwrap_or(0);
    let requested_limit = query
        .limit
        .unwrap_or(state.config.khala_poll_default_limit)
        .max(1);

    let state_for_socket = state.clone();
    let topic_for_socket = topic.clone();
    Ok(ws.on_upgrade(move |socket| {
        khala_ws_stream(
            state_for_socket,
            socket,
            principal,
            topic_for_socket,
            after_seq,
            requested_limit,
        )
    }))
}

async fn khala_ws_stream(
    state: AppState,
    mut socket: WebSocket,
    principal: SyncPrincipal,
    topic: String,
    mut after_seq: u64,
    requested_limit: usize,
) {
    let principal_key = khala_principal_key(&principal);
    let consumer_key = khala_consumer_key(&principal, topic.as_str());
    {
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
        consumers.entry(consumer_key.clone()).or_default();
    }

    let jitter_ms = deterministic_jitter_ms(
        consumer_key.as_str(),
        after_seq,
        state.config.khala_reconnect_jitter_ms,
    );
    let reconnect_backoff_ms = state
        .config
        .khala_reconnect_base_backoff_ms
        .saturating_add(jitter_ms);

    let hello = KhalaWsFrame::Hello {
        topic: topic.clone(),
        after_seq,
        limit: requested_limit,
        recommended_reconnect_backoff_ms: reconnect_backoff_ms,
    };
    if let Ok(payload) = serde_json::to_string(&hello) {
        let _ = socket.send(Message::Text(payload)).await;
    }

    let mut slow_consumer_strikes = 0u32;
    let mut last_head_cursor = None::<u64>;

    loop {
        let active_topic_count = {
            let prefix = format!("{principal_key}|");
            let consumers = state.khala_delivery.consumers.lock().await;
            consumers
                .keys()
                .filter(|key| key.starts_with(prefix.as_str()))
                .count()
        };

        let mut limit = requested_limit
            .min(state.config.khala_poll_max_limit)
            .min(state.config.khala_outbound_queue_limit);
        if active_topic_count >= 2 && limit > state.config.khala_fair_topic_slice_limit {
            limit = state.config.khala_fair_topic_slice_limit;
            state.khala_delivery.record_fairness_limited();
        }

        let window = state.fanout.topic_window(&topic).await.ok().flatten();
        let (_oldest_available_cursor, head_cursor, _queue_depth, _dropped_messages) =
            fanout_window_details(window.as_ref());
        if head_cursor != last_head_cursor {
            last_head_cursor = head_cursor;
        }
        let consumer_lag = head_cursor.map(|head| head.saturating_sub(after_seq));

        if consumer_lag.unwrap_or(0) > state.config.khala_slow_consumer_lag_threshold {
            slow_consumer_strikes = slow_consumer_strikes.saturating_add(1);
        } else {
            slow_consumer_strikes = 0;
        }
        if slow_consumer_strikes >= state.config.khala_slow_consumer_max_strikes {
            state.khala_delivery.record_slow_consumer_eviction();
            state
                .khala_delivery
                .record_disconnect_cause("slow_consumer_evicted")
                .await;
            let frame = KhalaWsFrame::Error {
                code: "slow_consumer_evicted".to_string(),
                message: format!(
                    "topic={} lag={} threshold={} strikes={} max_strikes={}",
                    topic,
                    consumer_lag.unwrap_or(0),
                    state.config.khala_slow_consumer_lag_threshold,
                    slow_consumer_strikes,
                    state.config.khala_slow_consumer_max_strikes
                ),
            };
            if let Ok(payload) = serde_json::to_string(&frame) {
                let _ = socket.send(Message::Text(payload)).await;
            }
            break;
        }

        match state.fanout.poll(&topic, after_seq, limit).await {
            Ok(messages) => {
                state.khala_delivery.record_total_poll(messages.len());
                let next_cursor = messages
                    .last()
                    .map_or(after_seq, |message| message.sequence);
                {
                    let mut consumers = state.khala_delivery.consumers.lock().await;
                    if let Some(consumer_state) = consumers.get_mut(&consumer_key) {
                        consumer_state.last_cursor = next_cursor;
                        consumer_state.last_poll_at = Some(Utc::now());
                    }
                }
                after_seq = next_cursor;

                for message in messages {
                    let frame = KhalaWsFrame::Message { message };
                    let Ok(payload) = serde_json::to_string(&frame) else {
                        continue;
                    };
                    if socket.send(Message::Text(payload)).await.is_err() {
                        state
                            .khala_delivery
                            .record_disconnect_cause("send_failed")
                            .await;
                        break;
                    }
                }
            }
            Err(FanoutError::StaleCursor {
                topic: stale_topic,
                requested_cursor,
                oldest_available_cursor,
                head_cursor,
                reason_codes,
                replay_lag,
                replay_budget_events,
                qos_tier,
            }) => {
                state
                    .khala_delivery
                    .record_disconnect_cause("stale_cursor")
                    .await;
                let frame = KhalaWsFrame::StaleCursor {
                    topic: stale_topic,
                    requested_cursor,
                    oldest_available_cursor,
                    head_cursor,
                    reason_codes,
                    replay_lag,
                    replay_budget_events,
                    qos_tier,
                };
                if let Ok(payload) = serde_json::to_string(&frame) {
                    let _ = socket.send(Message::Text(payload)).await;
                }
                break;
            }
            Err(error) => {
                state
                    .khala_delivery
                    .record_disconnect_cause("fanout_error")
                    .await;
                let frame = KhalaWsFrame::Error {
                    code: "fanout_error".to_string(),
                    message: error.to_string(),
                };
                if let Ok(payload) = serde_json::to_string(&frame) {
                    let _ = socket.send(Message::Text(payload)).await;
                }
                break;
            }
        }

        tokio::select! {
            biased;
            next = socket.next() => {
                match next {
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Ok(Message::Ping(payload))) => {
                        let _ = socket.send(Message::Pong(payload)).await;
                    }
                    Some(Ok(_)) => {}
                    Some(Err(_)) => break,
                }
            }
            _ = tokio::time::sleep(Duration::from_millis(25)) => {}
        }
    }

    let mut consumers = state.khala_delivery.consumers.lock().await;
    consumers.remove(&consumer_key);
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

fn enforce_khala_origin_policy(state: &AppState, headers: &HeaderMap) -> Result<(), ApiError> {
    if !state.config.khala_enforce_origin {
        return Ok(());
    }
    let Some(origin_header) = headers.get(header::ORIGIN) else {
        return Ok(());
    };
    let origin = origin_header
        .to_str()
        .unwrap_or_default()
        .trim()
        .trim_end_matches('/')
        .to_ascii_lowercase();
    if origin.is_empty() {
        return Ok(());
    }
    if state.config.khala_allowed_origins.contains(&origin) {
        return Ok(());
    }

    tracing::warn!(
        origin = %origin,
        allowed = ?state.config.khala_allowed_origins,
        "khala origin denied by policy"
    );
    Err(ApiError::KhalaOriginDenied(
        "origin_not_allowed".to_string(),
    ))
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
        AuthorizedKhalaTopic::FleetWorkers { .. }
        | AuthorizedKhalaTopic::RunEvents { .. }
        | AuthorizedKhalaTopic::CodexWorkerEvents => {}
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
    let mut metadata = body.metadata;
    if metadata_has_role(&metadata, "provider") {
        qualify_provider_metadata(&metadata).await?;
        annotate_provider_metadata(&mut metadata);
    }
    let snapshot = state
        .workers
        .register_worker(RegisterWorkerRequest {
            worker_id: body.worker_id,
            owner,
            workspace_ref: body.workspace_ref,
            codex_home_ref: body.codex_home_ref,
            adapter: body.adapter,
            metadata,
        })
        .await
        .map_err(ApiError::from_worker)?;
    publish_worker_snapshot(&state, &snapshot).await?;
    maybe_spawn_nostr_provider_ad_mirror(&state, &snapshot);
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

async fn list_workers(
    State(state): State<AppState>,
    Query(query): Query<OwnerQuery>,
) -> Result<Json<WorkersListResponse>, ApiError> {
    let owner = owner_from_parts(query.owner_user_id, query.owner_guest_scope)?;
    let workers = state
        .workers
        .list_workers(&owner)
        .await
        .map_err(ApiError::from_worker)?;
    Ok(Json(WorkersListResponse { workers }))
}

async fn get_provider_catalog(
    State(state): State<AppState>,
    Query(query): Query<ProviderCatalogQuery>,
) -> Result<Json<ProviderCatalogResponse>, ApiError> {
    let guest_scope = query.owner_guest_scope.clone().and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    });
    let owner_filter = match (query.owner_user_id, guest_scope) {
        (None, None) => None,
        (user_id, guest_scope) => Some(owner_from_parts(user_id, guest_scope)?),
    };

    let workers = match owner_filter.as_ref() {
        Some(owner) => state
            .workers
            .list_workers(owner)
            .await
            .map_err(ApiError::from_worker)?,
        None => state.workers.list_all_workers().await,
    };

    let mut providers = build_provider_catalog(&workers);
    if let Some(capability) = query
        .capability
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        providers.retain(|provider| provider.capabilities.iter().any(|cap| cap == capability));
    }

    Ok(Json(ProviderCatalogResponse { providers }))
}

async fn route_provider(
    State(state): State<AppState>,
    Json(body): Json<RouteProviderBody>,
) -> Result<Json<ProviderSelection>, ApiError> {
    let owner = owner_from_parts(body.owner_user_id, body.owner_guest_scope)?;
    let workers = state.workers.list_all_workers().await;
    let selection = select_provider_for_capability(&workers, Some(&owner), &body.capability)
        .ok_or(ApiError::NotFound)?;
    Ok(Json(selection))
}

async fn dispatch_sandbox_run(
    State(state): State<AppState>,
    Json(body): Json<DispatchSandboxRunBody>,
) -> Result<Json<DispatchSandboxRunResponse>, ApiError> {
    let owner = owner_from_parts(body.owner_user_id, body.owner_guest_scope)?;
    let owner_key = owner_rate_key(&owner);
    state
        .compute_abuse
        .enforce_dispatch_rate(owner_key.as_str())
        .await?;

    validate_sandbox_request_phase0(&body.request)?;
    let job_hash = protocol::hash::canonical_hash(&body.request)
        .map_err(|error| ApiError::InvalidRequest(format!("invalid sandbox request: {error}")))?;

    let workers = state.workers.list_all_workers().await;
    let selection = match select_provider_for_capability(
        &workers,
        Some(&owner),
        PHASE0_REQUIRED_PROVIDER_CAPABILITY,
    ) {
        Some(selection) => selection,
        None => {
            state
                .compute_telemetry
                .record_dispatch_not_found(owner_key.as_str())
                .await;
            return Err(ApiError::NotFound);
        }
    };

    match dispatch_sandbox_request_to_provider(&selection, &workers, &job_hash, &body.request).await
    {
        Ok((response, latency_ms)) => {
            let provider_failed = matches!(
                response.status,
                protocol::jobs::sandbox::SandboxStatus::Timeout
                    | protocol::jobs::sandbox::SandboxStatus::Cancelled
                    | protocol::jobs::sandbox::SandboxStatus::Error
            );
            if provider_failed {
                let reason = format!("dispatch_status:{:?}", response.status);
                if let Err(error) = apply_provider_failure_strike(
                    &state,
                    selection.provider.worker_id.as_str(),
                    job_hash.as_str(),
                    reason.as_str(),
                )
                .await
                {
                    tracing::warn!(
                        worker_id = %selection.provider.worker_id,
                        err = ?error,
                        "provider failure strike update failed"
                    );
                }
            }
            if provider_failed && selection.tier == crate::marketplace::ProviderSelectionTier::Owned
            {
                if let Some(alt) = select_provider_for_capability_excluding(
                    &workers,
                    Some(&owner),
                    PHASE0_REQUIRED_PROVIDER_CAPABILITY,
                    Some(selection.provider.worker_id.as_str()),
                ) {
                    if alt.provider.worker_id != selection.provider.worker_id {
                        let fallback_from_provider_id =
                            Some(selection.provider.provider_id.clone());
                        let (response, latency_ms) = dispatch_sandbox_request_to_provider(
                            &alt,
                            &workers,
                            &job_hash,
                            &body.request,
                        )
                        .await?;
                        state
                            .compute_telemetry
                            .record_dispatch_success(owner_key.as_str(), latency_ms, true)
                            .await;
                        return Ok(Json(DispatchSandboxRunResponse {
                            job_hash,
                            selection: alt,
                            response,
                            latency_ms,
                            fallback_from_provider_id,
                        }));
                    }
                }
                if let Some(reserve) = select_provider_for_capability_excluding(
                    &workers,
                    None,
                    PHASE0_REQUIRED_PROVIDER_CAPABILITY,
                    Some(selection.provider.worker_id.as_str()),
                ) {
                    if reserve.provider.worker_id != selection.provider.worker_id {
                        let fallback_from_provider_id =
                            Some(selection.provider.provider_id.clone());
                        let (response, latency_ms) = dispatch_sandbox_request_to_provider(
                            &reserve,
                            &workers,
                            &job_hash,
                            &body.request,
                        )
                        .await?;
                        state
                            .compute_telemetry
                            .record_dispatch_success(owner_key.as_str(), latency_ms, true)
                            .await;
                        return Ok(Json(DispatchSandboxRunResponse {
                            job_hash,
                            selection: reserve,
                            response,
                            latency_ms,
                            fallback_from_provider_id,
                        }));
                    }
                }
            }

            state
                .compute_telemetry
                .record_dispatch_success(owner_key.as_str(), latency_ms, false)
                .await;
            Ok(Json(DispatchSandboxRunResponse {
                job_hash,
                selection,
                response,
                latency_ms,
                fallback_from_provider_id: None,
            }))
        }
        Err(error) => {
            if let Err(err) = apply_provider_failure_strike(
                &state,
                selection.provider.worker_id.as_str(),
                job_hash.as_str(),
                "dispatch_error",
            )
            .await
            {
                tracing::warn!(
                    worker_id = %selection.provider.worker_id,
                    err = ?err,
                    "provider dispatch failure strike update failed"
                );
            }
            if selection.tier == crate::marketplace::ProviderSelectionTier::Owned {
                if let Some(alt) = select_provider_for_capability_excluding(
                    &workers,
                    Some(&owner),
                    PHASE0_REQUIRED_PROVIDER_CAPABILITY,
                    Some(selection.provider.worker_id.as_str()),
                ) {
                    if alt.provider.worker_id != selection.provider.worker_id {
                        let fallback_from_provider_id =
                            Some(selection.provider.provider_id.clone());
                        let (response, latency_ms) = dispatch_sandbox_request_to_provider(
                            &alt,
                            &workers,
                            &job_hash,
                            &body.request,
                        )
                        .await?;
                        state
                            .compute_telemetry
                            .record_dispatch_success(owner_key.as_str(), latency_ms, true)
                            .await;
                        return Ok(Json(DispatchSandboxRunResponse {
                            job_hash,
                            selection: alt,
                            response,
                            latency_ms,
                            fallback_from_provider_id,
                        }));
                    }
                }
                if let Some(reserve) = select_provider_for_capability_excluding(
                    &workers,
                    None,
                    PHASE0_REQUIRED_PROVIDER_CAPABILITY,
                    Some(selection.provider.worker_id.as_str()),
                ) {
                    if reserve.provider.worker_id != selection.provider.worker_id {
                        let fallback_from_provider_id =
                            Some(selection.provider.provider_id.clone());
                        let (response, latency_ms) = dispatch_sandbox_request_to_provider(
                            &reserve,
                            &workers,
                            &job_hash,
                            &body.request,
                        )
                        .await?;
                        state
                            .compute_telemetry
                            .record_dispatch_success(owner_key.as_str(), latency_ms, true)
                            .await;
                        return Ok(Json(DispatchSandboxRunResponse {
                            job_hash,
                            selection: reserve,
                            response,
                            latency_ms,
                            fallback_from_provider_id,
                        }));
                    }
                }
            }
            state
                .compute_telemetry
                .record_dispatch_error(owner_key.as_str())
                .await;
            Err(error)
        }
    }
}

async fn get_job_types() -> Json<JobTypesResponse> {
    let job_types = protocol::jobs::registered_job_types();
    Json(JobTypesResponse { job_types })
}

#[derive(Debug, Serialize)]
struct ComputeTelemetryResponse {
    schema: String,
    owner_key: String,
    capability: String,
    provider_total: usize,
    provider_eligible_owned: usize,
    provider_eligible_reserve: usize,
    provider_eligible_total: usize,
    dispatch: OwnerComputeTelemetrySnapshot,
}

async fn get_compute_telemetry(
    State(state): State<AppState>,
    Query(query): Query<ComputeTelemetryQuery>,
) -> Result<Json<ComputeTelemetryResponse>, ApiError> {
    let owner = owner_from_parts(query.owner_user_id, query.owner_guest_scope)?;
    let owner_key = owner_rate_key(&owner);
    let capability = query
        .capability
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(PHASE0_REQUIRED_PROVIDER_CAPABILITY)
        .to_string();

    let workers = state.workers.list_all_workers().await;
    let providers = build_provider_catalog(&workers);
    let provider_total = providers.len();

    let mut eligible_owned = 0usize;
    let mut eligible_reserve = 0usize;
    for provider in &providers {
        if !provider_is_eligible_for_capability(provider, capability.as_str()) {
            continue;
        }
        if owners_match(&provider.owner, &owner) {
            eligible_owned += 1;
            continue;
        }
        if provider.reserve_pool {
            eligible_reserve += 1;
        }
    }
    let provider_eligible_total = eligible_owned.saturating_add(eligible_reserve);

    let dispatch = state.compute_telemetry.snapshot(owner_key.as_str()).await;

    Ok(Json(ComputeTelemetryResponse {
        schema: "openagents.marketplace.compute_telemetry.v1".to_string(),
        owner_key,
        capability,
        provider_total,
        provider_eligible_owned: eligible_owned,
        provider_eligible_reserve: eligible_reserve,
        provider_eligible_total,
        dispatch,
    }))
}

async fn get_compute_treasury_summary(
    State(state): State<AppState>,
    Query(query): Query<OwnerQuery>,
) -> Result<Json<crate::treasury::ComputeTreasurySummary>, ApiError> {
    let owner = owner_from_parts(query.owner_user_id, query.owner_guest_scope)?;
    let owner_key = owner_rate_key(&owner);
    let summary = state
        .treasury
        .summarize_compute_owner(owner_key.as_str(), 50)
        .await;
    Ok(Json(summary))
}

async fn verify_sandbox_run(
    State(_state): State<AppState>,
    Json(body): Json<SandboxVerificationBody>,
) -> Result<Json<SandboxVerificationResponse>, ApiError> {
    let outcome = crate::verification::verify_sandbox_run(&body.request, &body.response);
    Ok(Json(SandboxVerificationResponse {
        passed: outcome.passed,
        exit_code: outcome.exit_code,
        violations: outcome.violations,
    }))
}

async fn verify_repo_index(
    State(_state): State<AppState>,
    Json(body): Json<RepoIndexVerificationBody>,
) -> Result<Json<RepoIndexVerificationResponse>, ApiError> {
    let outcome = crate::verification::verify_repo_index(&body.request, &body.response);

    Ok(Json(RepoIndexVerificationResponse {
        passed: outcome.passed,
        tree_sha256: outcome.tree_sha256,
        violations: outcome.violations,
    }))
}

async fn settle_sandbox_run(
    State(state): State<AppState>,
    Json(body): Json<SettleSandboxRunBody>,
) -> Result<Json<SettleSandboxRunResponse>, ApiError> {
    ensure_runtime_write_authority(&state)?;
    let owner = owner_from_parts(body.owner_user_id, body.owner_guest_scope)?;
    let owner_key = owner_rate_key(&owner);
    if body.provider_id.trim().is_empty() || body.provider_worker_id.trim().is_empty() {
        return Err(ApiError::InvalidRequest(
            "provider_id and provider_worker_id are required".to_string(),
        ));
    }
    if body.amount_msats == 0 {
        return Err(ApiError::InvalidRequest(
            "amount_msats must be greater than zero".to_string(),
        ));
    }

    validate_sandbox_request_phase0(&body.request)?;
    let job_hash = protocol::hash::canonical_hash(&body.request)
        .map_err(|error| ApiError::InvalidRequest(format!("invalid sandbox request: {error}")))?;

    let (reservation, _created) = state
        .treasury
        .reserve_compute_job(
            owner_key.as_str(),
            job_hash.as_str(),
            body.provider_id.trim(),
            body.provider_worker_id.trim(),
            body.amount_msats,
        )
        .await
        .map_err(ApiError::from_treasury)?;

    // Emit reservation receipt into the run for replay evidence (idempotent).
    state
        .orchestrator
        .append_run_event(
            body.run_id,
            AppendRunEventRequest {
                event_type: "receipt".to_string(),
                payload: serde_json::json!({
                    "receipt_type": "BudgetReserved",
                    "payload": {
                        "scope": "compute_job",
                        "amount_msats": reservation.amount_msats,
                        "reservation_id": reservation.reservation_id,
                        "job_hash": job_hash.clone(),
                        "provider_id": reservation.provider_id.clone(),
                    }
                }),
                idempotency_key: Some(format!("budget-reserved:{job_hash}")),
                expected_previous_seq: None,
            },
        )
        .await
        .map_err(ApiError::from_orchestration)?;

    let outcome = crate::verification::verify_sandbox_run(&body.request, &body.response);
    let violations = outcome.violations.clone();
    if violations.is_empty() {
        apply_provider_success_signal(&state, body.provider_worker_id.trim(), job_hash.as_str())
            .await?;
    } else {
        apply_provider_violation_strike(
            &state,
            body.provider_worker_id.trim(),
            job_hash.as_str(),
            &violations,
        )
        .await?;
    }

    // Record verification receipt evidence (idempotent).
    state
        .orchestrator
        .append_run_event(
            body.run_id,
            AppendRunEventRequest {
                event_type: "receipt".to_string(),
                payload: serde_json::json!({
                    "receipt_type": if outcome.passed { "VerificationPassed" } else { "VerificationFailed" },
                    "payload": { "job_hash": job_hash.clone(), "exit_code": outcome.exit_code, "violations": violations.clone() },
                }),
                idempotency_key: Some(format!("verify:{job_hash}")),
                expected_previous_seq: None,
            },
        )
        .await
        .map_err(ApiError::from_orchestration)?;

    let verification_command = body
        .request
        .commands
        .last()
        .map(|command| command.cmd.clone())
        .unwrap_or_else(|| "sandbox_run".to_string());
    let verification_duration_ms = body.response.runs.last().map(|run| run.duration_ms);

    // Emit verification event into the receipt bundle (idempotent).
    state
        .orchestrator
        .append_run_event(
            body.run_id,
            AppendRunEventRequest {
                event_type: "verification".to_string(),
                payload: serde_json::json!({
                    "command": verification_command,
                    "exit_code": outcome.exit_code,
                    "cwd": body.request.repo.mount_path,
                    "duration_ms": verification_duration_ms,
                }),
                idempotency_key: Some(format!("verification:{job_hash}")),
                expected_previous_seq: None,
            },
        )
        .await
        .map_err(ApiError::from_orchestration)?;

    let (settled, _changed) = state
        .treasury
        .settle_compute_job(job_hash.as_str(), outcome.passed, outcome.exit_code)
        .await
        .map_err(ApiError::from_treasury)?;

    let settlement_status = match settled.status {
        crate::treasury::SettlementStatus::Released => "released",
        crate::treasury::SettlementStatus::Withheld => "withheld",
        crate::treasury::SettlementStatus::Reserved => "reserved",
    };

    let payment_amount = if outcome.passed {
        settled.amount_msats
    } else {
        0
    };

    let settled_reservation_id = settled.reservation_id.clone();
    let settled_provider_id = settled.provider_id.clone();

    let payment_event = serde_json::json!({
        "rail": "lightning",
        "asset_id": "BTC_LN",
        "amount_msats": payment_amount,
        "payment_proof": if outcome.passed {
            serde_json::json!({
                "type": "internal_ledger",
                "reservation_id": settled_reservation_id.clone(),
                "provider_id": settled_provider_id.clone(),
            })
        } else {
            serde_json::json!({
                "type": "withheld",
                "reservation_id": settled_reservation_id.clone(),
                "exit_code": outcome.exit_code,
            })
        },
        "job_hash": job_hash.clone(),
        "status": settlement_status,
    });

    state
        .orchestrator
        .append_run_event(
            body.run_id,
            AppendRunEventRequest {
                event_type: "payment".to_string(),
                payload: payment_event,
                idempotency_key: Some(format!("payment:{job_hash}:{settlement_status}")),
                expected_previous_seq: None,
            },
        )
        .await
        .map_err(ApiError::from_orchestration)?;

    state
        .orchestrator
        .append_run_event(
            body.run_id,
            AppendRunEventRequest {
                event_type: "receipt".to_string(),
                payload: serde_json::json!({
                    "receipt_type": if outcome.passed { "PaymentReleased" } else { "PaymentWithheld" },
                    "payload": {
                        "job_hash": job_hash.clone(),
                        "amount_msats": settled.amount_msats,
                        "reservation_id": settled_reservation_id,
                        "provider_id": settled_provider_id,
                    }
                }),
                idempotency_key: Some(format!("receipt-payment:{job_hash}:{settlement_status}")),
                expected_previous_seq: None,
            },
        )
        .await
        .map_err(ApiError::from_orchestration)?;

    Ok(Json(SettleSandboxRunResponse {
        job_hash,
        reservation_id: settled.reservation_id,
        amount_msats: settled.amount_msats,
        verification_passed: outcome.passed,
        exit_code: outcome.exit_code,
        violations,
        settlement_status: settlement_status.to_string(),
    }))
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

#[derive(Debug)]
enum ApiError {
    NotFound,
    Forbidden(String),
    Conflict(String),
    KhalaUnauthorized(String),
    KhalaForbiddenTopic(String),
    KhalaOriginDenied(String),
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
        reason_codes: Vec<String>,
        replay_lag: u64,
        replay_budget_events: u64,
        qos_tier: String,
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

    fn from_treasury(error: crate::treasury::TreasuryError) -> Self {
        match error {
            crate::treasury::TreasuryError::NotReserved => Self::NotFound,
            crate::treasury::TreasuryError::InsufficientBudget => {
                Self::Forbidden("insufficient budget".to_string())
            }
            crate::treasury::TreasuryError::OwnerMismatch
            | crate::treasury::TreasuryError::AmountMismatch
            | crate::treasury::TreasuryError::AlreadySettled
            | crate::treasury::TreasuryError::SettlementConflict => {
                Self::Conflict(error.to_string())
            }
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
                reason_codes,
                replay_lag,
                replay_budget_events,
                qos_tier,
            } => Self::StaleCursor {
                topic,
                requested_cursor,
                oldest_available_cursor,
                head_cursor,
                reason_codes,
                replay_lag,
                replay_budget_events,
                qos_tier,
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

fn khala_principal_key(principal: &SyncPrincipal) -> String {
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
    format!("{user}|{org}|{device}")
}

fn khala_consumer_key(principal: &SyncPrincipal, topic: &str) -> String {
    format!("{}|{topic}", khala_principal_key(principal))
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

const PHASE0_REQUIRED_PROVIDER_CAPABILITY: &str = "oa.sandbox_run.v1";

fn metadata_has_role(metadata: &serde_json::Value, role: &str) -> bool {
    metadata_string_array(metadata, "roles")
        .iter()
        .any(|value| value == role)
}

fn metadata_string(metadata: &serde_json::Value, key: &str) -> Option<String> {
    metadata.get(key)?.as_str().map(|value| value.to_string())
}

fn metadata_string_array(metadata: &serde_json::Value, key: &str) -> Vec<String> {
    match metadata.get(key).and_then(|value| value.as_array()) {
        Some(values) => values
            .iter()
            .filter_map(|value| value.as_str())
            .map(|value| value.to_string())
            .collect(),
        None => Vec::new(),
    }
}

fn annotate_provider_metadata(metadata: &mut serde_json::Value) {
    if let Some(map) = metadata.as_object_mut() {
        map.insert("qualified".to_string(), serde_json::Value::Bool(true));
        map.insert(
            "qualified_at".to_string(),
            serde_json::Value::String(Utc::now().to_rfc3339()),
        );
        if !map.contains_key("failure_strikes") {
            map.insert(
                "failure_strikes".to_string(),
                serde_json::Value::Number(serde_json::Number::from(0_u64)),
            );
        }
        if !map.contains_key("quarantined") {
            map.insert("quarantined".to_string(), serde_json::Value::Bool(false));
        }
        if !map.contains_key("success_count") {
            map.insert(
                "success_count".to_string(),
                serde_json::Value::Number(serde_json::Number::from(0_u64)),
            );
        }
    }
}

async fn qualify_provider_metadata(metadata: &serde_json::Value) -> Result<(), ApiError> {
    let provider_base_url = metadata_string(metadata, "provider_base_url")
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            ApiError::InvalidRequest("provider_base_url is required for provider role".to_string())
        })?;

    let capabilities = metadata_string_array(metadata, "capabilities");
    if capabilities.is_empty() {
        return Err(ApiError::InvalidRequest(
            "capabilities[] is required for provider role".to_string(),
        ));
    }
    if !capabilities
        .iter()
        .any(|capability| capability == PHASE0_REQUIRED_PROVIDER_CAPABILITY)
    {
        return Err(ApiError::InvalidRequest(format!(
            "provider must advertise capability {PHASE0_REQUIRED_PROVIDER_CAPABILITY} for Phase 0"
        )));
    }

    probe_provider_health(provider_base_url.as_str()).await?;
    Ok(())
}

async fn probe_provider_health(base_url: &str) -> Result<(), ApiError> {
    let trimmed = base_url.trim_end_matches('/');
    let url = format!("{trimmed}/healthz");
    let resp = reqwest::Client::new()
        .get(url.as_str())
        .timeout(Duration::from_secs(2))
        .send()
        .await
        .map_err(|error| {
            ApiError::InvalidRequest(format!("provider health check failed ({url}): {error}"))
        })?;
    if !resp.status().is_success() {
        return Err(ApiError::InvalidRequest(format!(
            "provider health check returned {} ({url})",
            resp.status()
        )));
    }
    Ok(())
}

fn owner_rate_key(owner: &WorkerOwner) -> String {
    if let Some(user_id) = owner.user_id {
        return format!("user:{user_id}");
    }
    owner
        .guest_scope
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| format!("guest:{value}"))
        .unwrap_or_else(|| "guest:unknown".to_string())
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

fn provider_is_eligible_for_capability(provider: &ProviderCatalogEntry, capability: &str) -> bool {
    if provider
        .base_url
        .as_deref()
        .map(str::trim)
        .unwrap_or("")
        .is_empty()
    {
        return false;
    }
    if provider.quarantined {
        return false;
    }
    if provider.status != WorkerStatus::Running {
        return false;
    }
    if provider.heartbeat_state != "fresh" {
        return false;
    }
    provider.capabilities.iter().any(|cap| cap == capability)
}

fn validate_sandbox_request_phase0(request: &protocol::SandboxRunRequest) -> Result<(), ApiError> {
    if request.commands.is_empty() {
        return Err(ApiError::InvalidRequest(
            "sandbox request must include at least one command".to_string(),
        ));
    }
    if request.commands.len() > 20 {
        return Err(ApiError::InvalidRequest(
            "sandbox request exceeds 20 command cap".to_string(),
        ));
    }
    for command in &request.commands {
        if command.cmd.len() > 4096 {
            return Err(ApiError::InvalidRequest(
                "sandbox command exceeds 4096 byte cap".to_string(),
            ));
        }
    }
    if request.env.len() > 32 {
        return Err(ApiError::InvalidRequest(
            "sandbox env exceeds 32 entry cap".to_string(),
        ));
    }
    if request.sandbox.network_policy != protocol::jobs::sandbox::NetworkPolicy::None {
        return Err(ApiError::InvalidRequest(
            "sandbox network_policy must be none in Phase 0".to_string(),
        ));
    }
    if request.sandbox.resources.timeout_secs > 300 {
        return Err(ApiError::InvalidRequest(
            "sandbox timeout_secs exceeds 300 second cap".to_string(),
        ));
    }
    if request.sandbox.resources.memory_mb > 8192 {
        return Err(ApiError::InvalidRequest(
            "sandbox memory_mb exceeds 8192 cap".to_string(),
        ));
    }
    if request.sandbox.resources.cpus > 8.0 {
        return Err(ApiError::InvalidRequest(
            "sandbox cpus exceeds 8.0 cap".to_string(),
        ));
    }
    Ok(())
}

async fn dispatch_sandbox_request_to_provider(
    selection: &ProviderSelection,
    workers: &[WorkerSnapshot],
    job_hash: &str,
    request: &protocol::SandboxRunRequest,
) -> Result<(protocol::SandboxRunResponse, u64), ApiError> {
    let base_url = selection
        .provider
        .base_url
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ApiError::InvalidRequest("routed provider missing base_url".to_string()))?;
    let provider_snapshot = workers
        .iter()
        .find(|snapshot| snapshot.worker.worker_id == selection.provider.worker_id)
        .ok_or_else(|| ApiError::Internal("missing provider snapshot".to_string()))?;
    let max_timeout_secs = provider_snapshot
        .worker
        .metadata
        .pointer("/caps/max_timeout_secs")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(60)
        .max(1)
        .min(3_600) as u32;
    if request.sandbox.resources.timeout_secs > max_timeout_secs {
        return Err(ApiError::InvalidRequest(format!(
            "sandbox timeout_secs {} exceeds provider max_timeout_secs {}",
            request.sandbox.resources.timeout_secs, max_timeout_secs
        )));
    }

    let url = format!("{}/v1/sandbox_run", base_url.trim_end_matches('/'));
    let timeout_secs = request.sandbox.resources.timeout_secs.saturating_add(5);
    let started = std::time::Instant::now();
    let resp = reqwest::Client::new()
        .post(url.as_str())
        .header("x-idempotency-key", job_hash)
        .timeout(Duration::from_secs(timeout_secs as u64))
        .json(request)
        .send()
        .await
        .map_err(|error| ApiError::InvalidRequest(format!("provider dispatch failed: {error}")))?;
    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(ApiError::InvalidRequest(format!(
            "provider dispatch returned {}: {}",
            status, text
        )));
    }
    let parsed = resp
        .json::<protocol::SandboxRunResponse>()
        .await
        .map_err(|error| ApiError::Internal(format!("parse sandbox response: {error}")))?;
    let latency_ms = started.elapsed().as_millis() as u64;
    Ok((parsed, latency_ms))
}

const PROVIDER_VIOLATION_STRIKE_QUARANTINE_THRESHOLD: u64 = 3;
const PROVIDER_FAILURE_STRIKE_QUARANTINE_THRESHOLD: u64 = 5;

async fn apply_provider_violation_strike(
    state: &AppState,
    worker_id: &str,
    job_hash: &str,
    violations: &[String],
) -> Result<(), ApiError> {
    ensure_runtime_write_authority(state)?;

    let snapshot = state
        .workers
        .list_all_workers()
        .await
        .into_iter()
        .find(|snapshot| snapshot.worker.worker_id == worker_id)
        .ok_or_else(|| {
            ApiError::InvalidRequest(format!("unknown provider worker_id {worker_id}"))
        })?;

    let current_strikes = snapshot
        .worker
        .metadata
        .get("failure_strikes")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);

    if snapshot
        .worker
        .metadata
        .get("last_violation_job_hash")
        .and_then(serde_json::Value::as_str)
        == Some(job_hash)
    {
        return Ok(());
    }

    let next_strikes = current_strikes.saturating_add(1);
    let mut patch = serde_json::json!({
        "failure_strikes": next_strikes,
        "last_violation_at": Utc::now().to_rfc3339(),
        "last_violation_reason": violations.first().cloned().unwrap_or_else(|| "violation".to_string()),
        "last_violation_job_hash": job_hash,
    });
    let should_quarantine = next_strikes >= PROVIDER_VIOLATION_STRIKE_QUARANTINE_THRESHOLD;
    if should_quarantine {
        if let Some(map) = patch.as_object_mut() {
            map.insert("quarantined".to_string(), serde_json::Value::Bool(true));
            map.insert(
                "quarantine_reason".to_string(),
                serde_json::Value::String("verification_violations".to_string()),
            );
            map.insert(
                "quarantined_at".to_string(),
                serde_json::Value::String(Utc::now().to_rfc3339()),
            );
        }
    }

    let updated = state
        .workers
        .heartbeat(
            worker_id,
            WorkerHeartbeatRequest {
                owner: snapshot.worker.owner.clone(),
                metadata_patch: patch,
            },
        )
        .await
        .map_err(ApiError::from_worker)?;
    publish_worker_snapshot(state, &updated).await?;

    if should_quarantine && updated.worker.status != WorkerStatus::Failed {
        let transitioned = state
            .workers
            .transition_status(
                worker_id,
                WorkerStatusTransitionRequest {
                    owner: updated.worker.owner.clone(),
                    status: WorkerStatus::Failed,
                    reason: Some("quarantined".to_string()),
                },
            )
            .await
            .map_err(ApiError::from_worker)?;
        publish_worker_snapshot(state, &transitioned).await?;
    }

    Ok(())
}

async fn apply_provider_failure_strike(
    state: &AppState,
    worker_id: &str,
    job_hash: &str,
    reason: &str,
) -> Result<(), ApiError> {
    ensure_runtime_write_authority(state)?;

    let snapshot = state
        .workers
        .list_all_workers()
        .await
        .into_iter()
        .find(|snapshot| snapshot.worker.worker_id == worker_id)
        .ok_or_else(|| {
            ApiError::InvalidRequest(format!("unknown provider worker_id {worker_id}"))
        })?;

    if snapshot
        .worker
        .metadata
        .get("last_failure_job_hash")
        .and_then(serde_json::Value::as_str)
        == Some(job_hash)
    {
        return Ok(());
    }

    let current_strikes = snapshot
        .worker
        .metadata
        .get("failure_strikes")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let next_strikes = current_strikes.saturating_add(1);
    let mut patch = serde_json::json!({
        "failure_strikes": next_strikes,
        "last_failure_at": Utc::now().to_rfc3339(),
        "last_failure_reason": reason,
        "last_failure_job_hash": job_hash,
    });

    let should_quarantine = next_strikes >= PROVIDER_FAILURE_STRIKE_QUARANTINE_THRESHOLD;
    if should_quarantine {
        if let Some(map) = patch.as_object_mut() {
            map.insert("quarantined".to_string(), serde_json::Value::Bool(true));
            map.insert(
                "quarantine_reason".to_string(),
                serde_json::Value::String("provider_failures".to_string()),
            );
            map.insert(
                "quarantined_at".to_string(),
                serde_json::Value::String(Utc::now().to_rfc3339()),
            );
        }
    }

    let updated = state
        .workers
        .heartbeat(
            worker_id,
            WorkerHeartbeatRequest {
                owner: snapshot.worker.owner.clone(),
                metadata_patch: patch,
            },
        )
        .await
        .map_err(ApiError::from_worker)?;
    publish_worker_snapshot(state, &updated).await?;

    if should_quarantine && updated.worker.status != WorkerStatus::Failed {
        let transitioned = state
            .workers
            .transition_status(
                worker_id,
                WorkerStatusTransitionRequest {
                    owner: updated.worker.owner.clone(),
                    status: WorkerStatus::Failed,
                    reason: Some("quarantined".to_string()),
                },
            )
            .await
            .map_err(ApiError::from_worker)?;
        publish_worker_snapshot(state, &transitioned).await?;
    }

    Ok(())
}

async fn apply_provider_success_signal(
    state: &AppState,
    worker_id: &str,
    job_hash: &str,
) -> Result<(), ApiError> {
    ensure_runtime_write_authority(state)?;

    let snapshot = state
        .workers
        .list_all_workers()
        .await
        .into_iter()
        .find(|snapshot| snapshot.worker.worker_id == worker_id)
        .ok_or_else(|| {
            ApiError::InvalidRequest(format!("unknown provider worker_id {worker_id}"))
        })?;

    if snapshot
        .worker
        .metadata
        .get("last_success_job_hash")
        .and_then(serde_json::Value::as_str)
        == Some(job_hash)
    {
        return Ok(());
    }

    let current_success = snapshot
        .worker
        .metadata
        .get("success_count")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let next_success = current_success.saturating_add(1);
    let patch = serde_json::json!({
        "success_count": next_success,
        "last_success_at": Utc::now().to_rfc3339(),
        "last_success_job_hash": job_hash,
    });

    let updated = state
        .workers
        .heartbeat(
            worker_id,
            WorkerHeartbeatRequest {
                owner: snapshot.worker.owner.clone(),
                metadata_patch: patch,
            },
        )
        .await
        .map_err(ApiError::from_worker)?;
    publish_worker_snapshot(state, &updated).await?;
    Ok(())
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
    let meta = &snapshot.worker.metadata;
    let roles = meta
        .get("roles")
        .cloned()
        .unwrap_or_else(|| serde_json::json!([]));
    let capabilities = meta
        .get("capabilities")
        .cloned()
        .unwrap_or_else(|| serde_json::json!([]));
    let provider_id = meta.get("provider_id").and_then(serde_json::Value::as_str);
    let provider_base_url = meta
        .get("provider_base_url")
        .and_then(serde_json::Value::as_str);
    let min_price_msats = meta
        .get("min_price_msats")
        .and_then(serde_json::Value::as_u64);
    let reserve_pool = meta
        .get("reserve_pool")
        .and_then(serde_json::Value::as_bool);
    let qualified = meta.get("qualified").and_then(serde_json::Value::as_bool);
    let failure_strikes = meta
        .get("failure_strikes")
        .and_then(serde_json::Value::as_u64);
    let quarantined = meta.get("quarantined").and_then(serde_json::Value::as_bool);
    let quarantine_reason = meta
        .get("quarantine_reason")
        .and_then(serde_json::Value::as_str);

    let payload = serde_json::json!({
        "worker_id": snapshot.worker.worker_id,
        "status": snapshot.worker.status,
        "latest_seq": snapshot.worker.latest_seq,
        "heartbeat_state": snapshot.liveness.heartbeat_state,
        "heartbeat_age_ms": snapshot.liveness.heartbeat_age_ms,
        "roles": roles,
        "provider_id": provider_id,
        "provider_base_url": provider_base_url,
        "capabilities": capabilities,
        "min_price_msats": min_price_msats,
        "reserve_pool": reserve_pool,
        "qualified": qualified,
        "failure_strikes": failure_strikes,
        "quarantined": quarantined,
        "quarantine_reason": quarantine_reason,
        "owner_user_id": snapshot.worker.owner.user_id,
        "owner_guest_scope": snapshot.worker.owner.guest_scope,
    });

    let topic = format!("worker:{}:lifecycle", snapshot.worker.worker_id);
    state
        .fanout
        .publish(
            &topic,
            FanoutMessage {
                topic: topic.clone(),
                sequence: snapshot.worker.latest_seq,
                kind: snapshot.worker.status.as_event_label().to_string(),
                payload: payload.clone(),
                published_at: Utc::now(),
            },
        )
        .await
        .map_err(ApiError::from_fanout)?;

    if let Some(user_id) = snapshot.worker.owner.user_id {
        let fleet_topic = format!("fleet:user:{user_id}:workers");
        let fleet_seq = state
            .fleet_seq
            .fetch_add(1, Ordering::Relaxed)
            .saturating_add(1);
        state
            .fanout
            .publish(
                &fleet_topic,
                FanoutMessage {
                    topic: fleet_topic.clone(),
                    sequence: fleet_seq,
                    kind: snapshot.worker.status.as_event_label().to_string(),
                    payload,
                    published_at: Utc::now(),
                },
            )
            .await
            .map_err(ApiError::from_fanout)?;
    }

    Ok(())
}

fn maybe_spawn_nostr_provider_ad_mirror(state: &AppState, snapshot: &WorkerSnapshot) {
    if state.config.bridge_nostr_relays.is_empty() {
        return;
    }
    let Some(secret_key) = state.config.bridge_nostr_secret_key else {
        return;
    };
    if !is_provider_worker(&snapshot.worker) {
        return;
    }

    let meta = &snapshot.worker.metadata;
    let provider_id = meta
        .get("provider_id")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| snapshot.worker.worker_id.clone());
    let name = meta
        .get("name")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("OpenAgents Compute Provider")
        .to_string();
    let description = meta
        .get("description")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("OpenAgents Compute provider enrolled in Nexus registry")
        .to_string();
    let website = meta
        .get("website")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string);
    let capabilities = meta
        .get("capabilities")
        .and_then(serde_json::Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(serde_json::Value::as_str)
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let min_price_msats = meta
        .get("min_price_msats")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(1000);

    let relays = state.config.bridge_nostr_relays.clone();
    let payload = ProviderAdV1 {
        provider_id: provider_id.clone(),
        name,
        description,
        website,
        capabilities,
        min_price_msats,
    };

    tokio::spawn(async move {
        let event = match build_provider_ad_event(&secret_key, None, &payload) {
            Ok(event) => event,
            Err(error) => {
                tracing::warn!(
                    provider_id,
                    reason = %error,
                    "bridge nostr mirror failed to build provider ad"
                );
                return;
            }
        };
        let publisher = BridgeNostrPublisher::new(relays);
        if let Err(error) = publisher.connect().await {
            tracing::warn!(
                provider_id,
                reason = %error,
                "bridge nostr mirror failed to connect to relays"
            );
            return;
        }
        if let Err(error) = publisher.publish(&event).await {
            tracing::warn!(
                provider_id,
                reason = %error,
                "bridge nostr mirror failed to publish provider ad"
            );
        }
    });
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
            Self::KhalaOriginDenied(reason_code) => (
                StatusCode::FORBIDDEN,
                Json(serde_json::json!({
                    "error": "forbidden_origin",
                    "message": "origin is not allowed for khala access",
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
                reason_codes,
                replay_lag,
                replay_budget_events,
                qos_tier,
            } => (
                StatusCode::GONE,
                Json(serde_json::json!({
                    "error": "stale_cursor",
                    "message": "cursor cannot be resumed from retained stream window",
                    "details": {
                        "topic": topic,
                        "requested_cursor": requested_cursor,
                        "oldest_available_cursor": oldest_available_cursor,
                        "head_cursor": head_cursor,
                        "reason_codes": reason_codes,
                        "replay_lag": replay_lag,
                        "replay_budget_events": replay_budget_events,
                        "qos_tier": qos_tier,
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
        http::{HeaderValue, Method, Request},
    };
    use chrono::Utc;
    use futures::StreamExt;
    use http_body_util::BodyExt;
    use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};
    use serde_json::Value;
    use tokio::net::TcpListener;
    use tokio_tungstenite::connect_async;
    use tokio_tungstenite::tungstenite::Message as WsMessage;
    use tokio_tungstenite::tungstenite::client::IntoClientRequest;
    use tokio_tungstenite::tungstenite::{Error as WsError, http::StatusCode as WsStatusCode};
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
            khala_outbound_queue_limit: 200,
            khala_fair_topic_slice_limit: 50,
            khala_poll_min_interval_ms: 250,
            khala_slow_consumer_lag_threshold: 300,
            khala_slow_consumer_max_strikes: 3,
            khala_consumer_registry_capacity: 4096,
            khala_reconnect_base_backoff_ms: 400,
            khala_reconnect_jitter_ms: 250,
            khala_enforce_origin: true,
            khala_allowed_origins: HashSet::from([
                "https://openagents.com".to_string(),
                "https://www.openagents.com".to_string(),
            ]),
            khala_run_events_publish_rate_per_second: 240,
            khala_worker_lifecycle_publish_rate_per_second: 180,
            khala_codex_worker_events_publish_rate_per_second: 240,
            khala_fallback_publish_rate_per_second: 90,
            khala_run_events_replay_budget_events: 20_000,
            khala_worker_lifecycle_replay_budget_events: 10_000,
            khala_codex_worker_events_replay_budget_events: 3_000,
            khala_fallback_replay_budget_events: 500,
            khala_run_events_max_payload_bytes: 256 * 1024,
            khala_worker_lifecycle_max_payload_bytes: 64 * 1024,
            khala_codex_worker_events_max_payload_bytes: 128 * 1024,
            khala_fallback_max_payload_bytes: 64 * 1024,
            sync_token_signing_key: TEST_SYNC_SIGNING_KEY.to_string(),
            sync_token_issuer: "https://openagents.com".to_string(),
            sync_token_audience: "openagents-sync".to_string(),
            sync_token_require_jti: true,
            sync_token_max_age_seconds: 300,
            sync_revoked_jtis: revoked_for_config,
            bridge_nostr_relays: Vec::new(),
            bridge_nostr_secret_key: None,
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
                require_jti: true,
                max_token_age_seconds: 300,
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

    async fn spawn_http_server(
        app: axum::Router,
    ) -> Result<(std::net::SocketAddr, tokio::sync::oneshot::Sender<()>)> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let addr = listener.local_addr()?;
        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();
        tokio::spawn(async move {
            let server = axum::serve(listener, app).with_graceful_shutdown(async move {
                let _ = shutdown_rx.await;
            });
            let _ = server.await;
        });
        Ok((addr, shutdown_tx))
    }

    async fn spawn_provider_stub() -> Result<(String, tokio::sync::oneshot::Sender<()>)> {
        let app = axum::Router::new().route(
            "/healthz",
            axum::routing::get(|| async { (axum::http::StatusCode::OK, "ok") }),
        );
        let (addr, shutdown) = spawn_http_server(app).await?;
        Ok((format!("http://{addr}"), shutdown))
    }

    async fn spawn_compute_provider_stub() -> Result<(String, tokio::sync::oneshot::Sender<()>)> {
        async fn sandbox_run(
            axum::Json(request): axum::Json<protocol::SandboxRunRequest>,
        ) -> axum::Json<protocol::SandboxRunResponse> {
            let runs = request
                .commands
                .iter()
                .map(|cmd| protocol::jobs::sandbox::CommandResult {
                    cmd: cmd.cmd.clone(),
                    exit_code: 0,
                    duration_ms: 1,
                    stdout_sha256: "stdout".to_string(),
                    stderr_sha256: "stderr".to_string(),
                    stdout_preview: None,
                    stderr_preview: None,
                })
                .collect::<Vec<_>>();
            axum::Json(protocol::SandboxRunResponse {
                env_info: protocol::jobs::sandbox::EnvInfo {
                    image_digest: request.sandbox.image_digest.clone(),
                    hostname: None,
                    system_info: None,
                },
                runs,
                artifacts: Vec::new(),
                status: protocol::jobs::sandbox::SandboxStatus::Success,
                error: None,
                provenance: protocol::Provenance::new("stub"),
            })
        }

        let app = axum::Router::new()
            .route(
                "/healthz",
                axum::routing::get(|| async { (axum::http::StatusCode::OK, "ok") }),
            )
            .route("/v1/sandbox_run", axum::routing::post(sandbox_run));
        let (addr, shutdown) = spawn_http_server(app).await?;
        Ok((format!("http://{addr}"), shutdown))
    }

    async fn spawn_cancelled_compute_provider_stub()
    -> Result<(String, tokio::sync::oneshot::Sender<()>)> {
        async fn sandbox_run(
            axum::Json(request): axum::Json<protocol::SandboxRunRequest>,
        ) -> axum::Json<protocol::SandboxRunResponse> {
            let runs = request
                .commands
                .iter()
                .map(|cmd| protocol::jobs::sandbox::CommandResult {
                    cmd: cmd.cmd.clone(),
                    exit_code: 1,
                    duration_ms: 1,
                    stdout_sha256: "stdout".to_string(),
                    stderr_sha256: "stderr".to_string(),
                    stdout_preview: None,
                    stderr_preview: None,
                })
                .collect::<Vec<_>>();
            axum::Json(protocol::SandboxRunResponse {
                env_info: protocol::jobs::sandbox::EnvInfo {
                    image_digest: request.sandbox.image_digest.clone(),
                    hostname: None,
                    system_info: None,
                },
                runs,
                artifacts: Vec::new(),
                status: protocol::jobs::sandbox::SandboxStatus::Cancelled,
                error: None,
                provenance: protocol::Provenance::new("stub-cancelled"),
            })
        }

        let app = axum::Router::new()
            .route(
                "/healthz",
                axum::routing::get(|| async { (axum::http::StatusCode::OK, "ok") }),
            )
            .route("/v1/sandbox_run", axum::routing::post(sandbox_run));
        let (addr, shutdown) = spawn_http_server(app).await?;
        Ok((format!("http://{addr}"), shutdown))
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
        issue_sync_token_for_surface(scopes, user_id, org_id, jti, exp_offset_seconds, "ios")
    }

    fn issue_sync_token_for_surface(
        scopes: &[&str],
        user_id: Option<u64>,
        org_id: Option<&str>,
        jti: &str,
        exp_offset_seconds: i64,
        client_surface: &str,
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
            oa_client_surface: Some(client_surface.to_string()),
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
        crate::artifacts::validate_replay_jsonl(&replay_text)
            .map_err(|err| anyhow!("replay output missing required sections: {}", err))?;

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
        assert_eq!(
            messages_json
                .pointer("/fairness_applied")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(true),
            false
        );
        assert_eq!(
            messages_json
                .pointer("/outbound_queue_limit")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or_default(),
            200
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
    async fn khala_topic_messages_apply_fair_slice_when_principal_tracks_multiple_topics()
    -> Result<()> {
        let app = build_test_router_with_config(
            AuthorityWriteMode::RustActive,
            HashSet::new(),
            |config| {
                config.khala_poll_min_interval_ms = 1;
                config.khala_poll_default_limit = 50;
                config.khala_poll_max_limit = 50;
                config.khala_outbound_queue_limit = 50;
                config.khala_fair_topic_slice_limit = 2;
            },
        );
        let sync_token = issue_sync_token(
            &["runtime.run_events", "runtime.worker_lifecycle_events"],
            Some(1),
            Some("user:1"),
            "fair-slice",
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
                        "worker_id": "desktop:fair-run-worker",
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

        for index in 2..=12 {
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

        let worker_id = "desktop:fairness-worker";
        let register_worker = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/internal/v1/workers")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&serde_json::json!({
                        "worker_id": worker_id,
                        "owner_user_id": 1,
                        "metadata": {}
                    }))?))?,
            )
            .await?;
        assert_eq!(register_worker.status(), axum::http::StatusCode::CREATED);

        let heartbeat_worker = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri(format!("/internal/v1/workers/{worker_id}/heartbeat"))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&serde_json::json!({
                        "owner_user_id": 1,
                        "metadata_patch": {"pulse": true}
                    }))?))?,
            )
            .await?;
        assert_eq!(heartbeat_worker.status(), axum::http::StatusCode::OK);

        let warmup_run_poll = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!(
                        "/internal/v1/khala/topics/run:{run_id}:events/messages?after_seq=0&limit=50"
                    ))
                    .header("authorization", format!("Bearer {sync_token}"))
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(warmup_run_poll.status(), axum::http::StatusCode::OK);
        let warmup_run_json = response_json(warmup_run_poll).await?;
        assert_eq!(
            warmup_run_json
                .pointer("/fairness_applied")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(true),
            false
        );

        tokio::time::sleep(Duration::from_millis(5)).await;

        let warmup_worker_poll = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!(
                        "/internal/v1/khala/topics/worker:{worker_id}:lifecycle/messages?after_seq=0&limit=50"
                    ))
                    .header("authorization", format!("Bearer {sync_token}"))
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(warmup_worker_poll.status(), axum::http::StatusCode::OK);

        tokio::time::sleep(Duration::from_millis(5)).await;

        let fair_poll = app
            .oneshot(
                Request::builder()
                    .uri(format!(
                        "/internal/v1/khala/topics/run:{run_id}:events/messages?after_seq=1&limit=50"
                    ))
                    .header("authorization", format!("Bearer {sync_token}"))
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(fair_poll.status(), axum::http::StatusCode::OK);
        let fair_json = response_json(fair_poll).await?;
        assert_eq!(
            fair_json
                .pointer("/fairness_applied")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false),
            true
        );
        assert_eq!(
            fair_json
                .pointer("/active_topic_count")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or_default(),
            2
        );
        assert_eq!(
            fair_json
                .pointer("/limit_applied")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or_default(),
            2
        );
        let delivered_count = fair_json
            .pointer("/messages")
            .and_then(serde_json::Value::as_array)
            .map_or(0, std::vec::Vec::len);
        assert_eq!(delivered_count, 2);

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
        assert_eq!(
            stale_json
                .pointer("/details/qos_tier")
                .and_then(serde_json::Value::as_str)
                .unwrap_or(""),
            "warm"
        );
        assert_eq!(
            stale_json
                .pointer("/details/replay_budget_events")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or_default(),
            20_000
        );
        let reason_codes = stale_json
            .pointer("/details/reason_codes")
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| anyhow!("missing stale reason_codes"))?;
        let has_retention_reason = reason_codes.iter().any(|value| {
            value
                .as_str()
                .map(|reason| reason == "retention_floor_breach")
                .unwrap_or(false)
        });
        assert!(has_retention_reason, "expected retention_floor_breach");

        Ok(())
    }

    #[tokio::test]
    async fn khala_topic_messages_returns_stale_cursor_when_replay_budget_is_exceeded() -> Result<()>
    {
        let app = build_test_router_with_config(
            AuthorityWriteMode::RustActive,
            HashSet::new(),
            |config| {
                config.fanout_queue_capacity = 256;
                config.khala_run_events_replay_budget_events = 3;
            },
        );
        let sync_token = issue_sync_token(
            &["runtime.run_events"],
            Some(1),
            Some("user:1"),
            "stale-budget",
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
                        "worker_id": "desktop:stale-budget-worker",
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

        for index in 2..=12 {
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
                        "/internal/v1/khala/topics/run:{run_id}:events/messages?after_seq=1&limit=10"
                    ))
                    .header("authorization", format!("Bearer {sync_token}"))
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(stale_response.status(), axum::http::StatusCode::GONE);
        let stale_json = response_json(stale_response).await?;
        let reason_codes = stale_json
            .pointer("/details/reason_codes")
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| anyhow!("missing stale reason_codes"))?;
        let has_budget_reason = reason_codes.iter().any(|value| {
            value
                .as_str()
                .map(|reason| reason == "replay_budget_exceeded")
                .unwrap_or(false)
        });
        assert!(has_budget_reason, "expected replay_budget_exceeded");
        assert_eq!(
            stale_json
                .pointer("/details/replay_budget_events")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or_default(),
            3
        );
        assert_eq!(
            stale_json
                .pointer("/details/qos_tier")
                .and_then(serde_json::Value::as_str)
                .unwrap_or(""),
            "warm"
        );

        Ok(())
    }

    #[tokio::test]
    async fn khala_origin_policy_denies_untrusted_browser_origins() -> Result<()> {
        let app = test_router();
        let sync_token = issue_sync_token(
            &["runtime.run_events"],
            Some(1),
            Some("user:1"),
            "origin-deny",
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
                        "worker_id": "desktop:origin-policy-worker",
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

        let denied_response = app
            .oneshot(
                Request::builder()
                    .uri(format!(
                        "/internal/v1/khala/topics/run:{run_id}:events/messages?after_seq=0&limit=10"
                    ))
                    .header("authorization", format!("Bearer {sync_token}"))
                    .header("origin", "https://evil.example.com")
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(denied_response.status(), axum::http::StatusCode::FORBIDDEN);
        let denied_json = response_json(denied_response).await?;
        assert_eq!(
            denied_json
                .pointer("/error")
                .and_then(serde_json::Value::as_str)
                .unwrap_or(""),
            "forbidden_origin"
        );
        assert_eq!(
            denied_json
                .pointer("/reason_code")
                .and_then(serde_json::Value::as_str)
                .unwrap_or(""),
            "origin_not_allowed"
        );

        Ok(())
    }

    #[tokio::test]
    async fn khala_ws_origin_policy_denies_untrusted_browser_origins() -> Result<()> {
        let app = test_router();
        let (addr, shutdown) = spawn_http_server(app).await?;
        let sync_token = issue_sync_token(
            &["runtime.run_events"],
            Some(1),
            Some("user:1"),
            "ws-origin-deny",
            300,
        );

        let url = format!(
            "ws://{addr}/internal/v1/khala/topics/run:ws_test:events/ws?after_seq=0&limit=10"
        );
        let mut request = url.into_client_request()?;
        request.headers_mut().insert(
            "authorization",
            HeaderValue::from_str(&format!("Bearer {sync_token}"))?,
        );
        request
            .headers_mut()
            .insert("origin", HeaderValue::from_str("https://evil.example.com")?);

        let err = connect_async(request).await.expect_err("expected denial");
        match err {
            WsError::Http(response) => {
                assert_eq!(response.status(), WsStatusCode::FORBIDDEN);
            }
            other => return Err(anyhow!("unexpected ws error: {other:?}")),
        }

        let _ = shutdown.send(());
        Ok(())
    }

    #[tokio::test]
    async fn khala_ws_requires_valid_sync_token() -> Result<()> {
        let app = test_router();
        let (addr, shutdown) = spawn_http_server(app).await?;

        let url = format!(
            "ws://{addr}/internal/v1/khala/topics/run:ws_test:events/ws?after_seq=0&limit=10"
        );
        let request = url.into_client_request()?;
        let err = connect_async(request).await.expect_err("expected denial");
        match err {
            WsError::Http(response) => {
                assert_eq!(response.status(), WsStatusCode::UNAUTHORIZED);
                if let Some(body) = response.body() {
                    if let Ok(value) = serde_json::from_slice::<Value>(body) {
                        assert_eq!(
                            value
                                .pointer("/reason_code")
                                .and_then(Value::as_str)
                                .unwrap_or(""),
                            "missing_authorization"
                        );
                    }
                }
            }
            other => return Err(anyhow!("unexpected ws error: {other:?}")),
        }

        let _ = shutdown.send(());
        Ok(())
    }

    #[tokio::test]
    async fn khala_ws_upgrade_succeeds_for_authorized_origin() -> Result<()> {
        let app = test_router();
        let (addr, shutdown) = spawn_http_server(app).await?;
        let sync_token = issue_sync_token(
            &["runtime.run_events"],
            Some(1),
            Some("user:1"),
            "ws-origin-allow",
            300,
        );

        let url = format!(
            "ws://{addr}/internal/v1/khala/topics/run:ws_test:events/ws?after_seq=0&limit=10"
        );
        let mut request = url.into_client_request()?;
        request.headers_mut().insert(
            "authorization",
            HeaderValue::from_str(&format!("Bearer {sync_token}"))?,
        );
        request
            .headers_mut()
            .insert("origin", HeaderValue::from_str("https://openagents.com")?);

        let (mut stream, response) = connect_async(request).await?;
        assert_eq!(response.status(), WsStatusCode::SWITCHING_PROTOCOLS);

        let next = tokio::time::timeout(Duration::from_secs(1), stream.next()).await?;
        let Some(frame) = next else {
            return Err(anyhow!("expected hello frame"));
        };
        let frame = frame?;
        let text = match frame {
            WsMessage::Text(text) => text,
            other => return Err(anyhow!("unexpected ws frame: {other:?}")),
        };
        let value: Value = serde_json::from_str(&text)?;
        assert_eq!(value.get("type").and_then(Value::as_str), Some("hello"));

        let _ = shutdown.send(());
        Ok(())
    }

    #[tokio::test]
    async fn khala_origin_policy_allows_openagents_origin() -> Result<()> {
        let app = test_router();
        let sync_token = issue_sync_token(
            &["runtime.run_events"],
            Some(1),
            Some("user:1"),
            "origin-allow",
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
                        "worker_id": "desktop:origin-policy-allow-worker",
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

        let allowed_response = app
            .oneshot(
                Request::builder()
                    .uri(format!(
                        "/internal/v1/khala/topics/run:{run_id}:events/messages?after_seq=0&limit=10"
                    ))
                    .header("authorization", format!("Bearer {sync_token}"))
                    .header("origin", "https://openagents.com")
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(allowed_response.status(), axum::http::StatusCode::OK);
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
    async fn khala_onyx_surface_is_limited_to_run_event_topics() -> Result<()> {
        let app = test_router();

        let onyx_token = issue_sync_token_for_surface(
            &["runtime.run_events", "runtime.codex_worker_events"],
            Some(1),
            Some("user:1"),
            "onyx-surface-policy",
            300,
            "onyx",
        );

        let run_allowed = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/internal/v1/khala/topics/run:019c7f93:events/messages?after_seq=0&limit=10")
                    .header("authorization", format!("Bearer {onyx_token}"))
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(run_allowed.status(), axum::http::StatusCode::OK);

        let worker_denied = app
            .oneshot(
                Request::builder()
                    .uri("/internal/v1/khala/topics/worker:desktop:owner-worker:lifecycle/messages?after_seq=0&limit=10")
                    .header("authorization", format!("Bearer {onyx_token}"))
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(worker_denied.status(), axum::http::StatusCode::FORBIDDEN);
        let denied_json = response_json(worker_denied).await?;
        assert_eq!(
            denied_json
                .pointer("/reason_code")
                .and_then(serde_json::Value::as_str)
                .unwrap_or(""),
            "surface_policy_denied"
        );

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

    #[tokio::test]
    async fn worker_list_and_provider_catalog_surface_expected_entries() -> Result<()> {
        let app = test_router();
        let (provider_base_url, shutdown) = spawn_provider_stub().await?;

        let create_provider = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/internal/v1/workers")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&serde_json::json!({
                        "worker_id": "desktop:catalog-worker-1",
                        "owner_user_id": 11,
                        "metadata": {
                            "roles": ["client", "provider"],
                            "provider_id": "provider-local-1",
                            "provider_base_url": provider_base_url.clone(),
                            "capabilities": ["oa.sandbox_run.v1"],
                            "min_price_msats": 1000
                        }
                    }))?))?,
            )
            .await?;
        assert_eq!(create_provider.status(), axum::http::StatusCode::CREATED);

        let create_client_only = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/internal/v1/workers")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&serde_json::json!({
                        "worker_id": "desktop:catalog-worker-2",
                        "owner_user_id": 11,
                        "metadata": {
                            "roles": ["client"]
                        }
                    }))?))?,
            )
            .await?;
        assert_eq!(create_client_only.status(), axum::http::StatusCode::CREATED);

        let list_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/internal/v1/workers?owner_user_id=11")
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(list_response.status(), axum::http::StatusCode::OK);
        let list_json = response_json(list_response).await?;
        let workers = list_json
            .get("workers")
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| anyhow!("missing workers array"))?;
        assert_eq!(workers.len(), 2);

        let catalog_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/internal/v1/marketplace/catalog/providers?owner_user_id=11&capability=oa.sandbox_run.v1")
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(catalog_response.status(), axum::http::StatusCode::OK);
        let catalog_json = response_json(catalog_response).await?;
        let providers = catalog_json
            .get("providers")
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| anyhow!("missing providers array"))?;
        assert_eq!(providers.len(), 1);

        let provider_id = providers[0]
            .get("provider_id")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| anyhow!("missing provider_id"))?;
        assert_eq!(provider_id, "provider-local-1");

        let base_url = providers[0]
            .get("base_url")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| anyhow!("missing base_url"))?;
        assert_eq!(base_url, provider_base_url);

        let _ = shutdown.send(());
        Ok(())
    }

    #[tokio::test]
    async fn provider_catalog_surfaces_local_cluster_provider_metadata() -> Result<()> {
        let app = test_router();
        let (provider_base_url, shutdown) = spawn_provider_stub().await?;

        let create_provider = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/internal/v1/workers")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&serde_json::json!({
                        "worker_id": "desktop:cluster-provider-1",
                        "owner_user_id": 11,
                        "metadata": {
                            "roles": ["client", "provider"],
                            "provider_id": "provider-cluster-1",
                            "provider_base_url": provider_base_url.clone(),
                            "capabilities": ["oa.sandbox_run.v1"],
                            "min_price_msats": 1500,
                            "supply_class": "local_cluster",
                            "cluster_id": "cluster-1",
                            "cluster_members": ["node-a", "node-b"]
                        }
                    }))?))?,
            )
            .await?;
        assert_eq!(create_provider.status(), axum::http::StatusCode::CREATED);

        let catalog_response = app
            .oneshot(
                Request::builder()
                    .uri("/internal/v1/marketplace/catalog/providers?owner_user_id=11")
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(catalog_response.status(), axum::http::StatusCode::OK);
        let catalog_json = response_json(catalog_response).await?;
        let providers = catalog_json
            .get("providers")
            .and_then(Value::as_array)
            .ok_or_else(|| anyhow!("missing providers array"))?;
        assert_eq!(providers.len(), 1);
        assert_eq!(
            providers[0]
                .pointer("/supply_class")
                .and_then(Value::as_str),
            Some("local_cluster")
        );
        assert_eq!(
            providers[0].pointer("/cluster_id").and_then(Value::as_str),
            Some("cluster-1")
        );
        let members = providers[0]
            .pointer("/cluster_members")
            .and_then(Value::as_array)
            .ok_or_else(|| anyhow!("missing cluster_members"))?;
        assert_eq!(members.len(), 2);

        let _ = shutdown.send(());
        Ok(())
    }

    #[tokio::test]
    async fn dispatch_sandbox_run_calls_provider_and_enforces_phase0_hardening() -> Result<()> {
        let app = test_router();
        let (provider_base_url, shutdown) = spawn_compute_provider_stub().await?;

        let create_provider = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/internal/v1/workers")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&serde_json::json!({
                        "worker_id": "desktop:dispatch-provider-1",
                        "owner_user_id": 11,
                        "metadata": {
                            "roles": ["client", "provider"],
                            "provider_id": "provider-dispatch-1",
                            "provider_base_url": provider_base_url.clone(),
                            "capabilities": ["oa.sandbox_run.v1"],
                            "min_price_msats": 1000,
                            "caps": { "max_timeout_secs": 120 }
                        }
                    }))?))?,
            )
            .await?;
        assert_eq!(create_provider.status(), axum::http::StatusCode::CREATED);

        let request = protocol::SandboxRunRequest {
            sandbox: protocol::jobs::sandbox::SandboxConfig {
                image_digest: "sha256:test".to_string(),
                network_policy: protocol::jobs::sandbox::NetworkPolicy::None,
                resources: protocol::jobs::sandbox::ResourceLimits {
                    timeout_secs: 30,
                    ..Default::default()
                },
                ..Default::default()
            },
            commands: vec![protocol::jobs::sandbox::SandboxCommand::new("echo hi")],
            ..Default::default()
        };

        let ok_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/internal/v1/marketplace/dispatch/sandbox-run")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&serde_json::json!({
                        "owner_user_id": 11,
                        "request": request
                    }))?))?,
            )
            .await?;
        assert_eq!(ok_response.status(), axum::http::StatusCode::OK);
        let ok_json = response_json(ok_response).await?;
        assert_eq!(
            ok_json
                .pointer("/selection/provider/provider_id")
                .and_then(Value::as_str),
            Some("provider-dispatch-1")
        );
        assert_eq!(
            ok_json.pointer("/response/status").and_then(Value::as_str),
            Some("success")
        );

        let bad_response = app
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/internal/v1/marketplace/dispatch/sandbox-run")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&serde_json::json!({
                        "owner_user_id": 11,
                        "request": {
                            "sandbox": {
                                "image_digest": "sha256:test",
                                "network_policy": "full"
                            },
                            "commands": [{"cmd":"echo hi"}]
                        }
                    }))?))?,
            )
            .await?;
        assert_eq!(bad_response.status(), axum::http::StatusCode::BAD_REQUEST);

        let _ = shutdown.send(());
        Ok(())
    }

    #[tokio::test]
    async fn dispatch_failure_applies_strike_and_reroutes_owned_provider() -> Result<()> {
        let app = test_router();
        let (failing_base_url, failing_shutdown) = spawn_cancelled_compute_provider_stub().await?;
        let (ok_base_url, ok_shutdown) = spawn_compute_provider_stub().await?;
        let (reserve_base_url, reserve_shutdown) = spawn_compute_provider_stub().await?;

        let create_failing = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/internal/v1/workers")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&serde_json::json!({
                        "worker_id": "desktop:dispatch-fail-provider",
                        "owner_user_id": 11,
                        "metadata": {
                            "roles": ["client", "provider"],
                            "provider_id": "provider-fail",
                            "provider_base_url": failing_base_url.clone(),
                            "capabilities": ["oa.sandbox_run.v1"],
                            "min_price_msats": 900,
                            "caps": { "max_timeout_secs": 120 }
                        }
                    }))?))?,
            )
            .await?;
        assert_eq!(create_failing.status(), axum::http::StatusCode::CREATED);

        let create_ok_owned = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/internal/v1/workers")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&serde_json::json!({
                        "worker_id": "desktop:dispatch-ok-provider",
                        "owner_user_id": 11,
                        "metadata": {
                            "roles": ["client", "provider"],
                            "provider_id": "provider-ok-owned",
                            "provider_base_url": ok_base_url.clone(),
                            "capabilities": ["oa.sandbox_run.v1"],
                            "min_price_msats": 1200,
                            "caps": { "max_timeout_secs": 120 }
                        }
                    }))?))?,
            )
            .await?;
        assert_eq!(create_ok_owned.status(), axum::http::StatusCode::CREATED);

        let create_reserve = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/internal/v1/workers")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&serde_json::json!({
                        "worker_id": "desktop:dispatch-reserve-provider",
                        "owner_user_id": 999,
                        "metadata": {
                            "roles": ["client", "provider"],
                            "provider_id": "provider-reserve",
                            "provider_base_url": reserve_base_url.clone(),
                            "capabilities": ["oa.sandbox_run.v1"],
                            "min_price_msats": 1500,
                            "reserve_pool": true,
                            "caps": { "max_timeout_secs": 120 }
                        }
                    }))?))?,
            )
            .await?;
        assert_eq!(create_reserve.status(), axum::http::StatusCode::CREATED);

        let request = protocol::SandboxRunRequest {
            sandbox: protocol::jobs::sandbox::SandboxConfig {
                image_digest: "sha256:test".to_string(),
                network_policy: protocol::jobs::sandbox::NetworkPolicy::None,
                resources: protocol::jobs::sandbox::ResourceLimits {
                    timeout_secs: 30,
                    ..Default::default()
                },
                ..Default::default()
            },
            commands: vec![protocol::jobs::sandbox::SandboxCommand::new("echo hi")],
            ..Default::default()
        };

        let dispatch = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/internal/v1/marketplace/dispatch/sandbox-run")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&serde_json::json!({
                        "owner_user_id": 11,
                        "request": request.clone(),
                    }))?))?,
            )
            .await?;
        assert_eq!(dispatch.status(), axum::http::StatusCode::OK);
        let dispatch_json = response_json(dispatch).await?;
        assert_eq!(
            dispatch_json
                .pointer("/selection/provider/provider_id")
                .and_then(Value::as_str),
            Some("provider-ok-owned")
        );
        assert_eq!(
            dispatch_json
                .pointer("/fallback_from_provider_id")
                .and_then(Value::as_str),
            Some("provider-fail")
        );

        let failing_worker = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/internal/v1/workers/desktop:dispatch-fail-provider?owner_user_id=11")
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(failing_worker.status(), axum::http::StatusCode::OK);
        let failing_worker_json = response_json(failing_worker).await?;
        assert_eq!(
            failing_worker_json
                .pointer("/worker/worker/metadata/failure_strikes")
                .and_then(Value::as_u64),
            Some(1)
        );

        let route = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/internal/v1/marketplace/route/provider")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&serde_json::json!({
                        "owner_user_id": 11,
                        "capability": "oa.sandbox_run.v1",
                    }))?))?,
            )
            .await?;
        assert_eq!(route.status(), axum::http::StatusCode::OK);
        let route_json = response_json(route).await?;
        assert_eq!(
            route_json
                .pointer("/provider/provider_id")
                .and_then(Value::as_str),
            Some("provider-ok-owned")
        );

        let _ = failing_shutdown.send(());
        let _ = ok_shutdown.send(());
        let _ = reserve_shutdown.send(());
        Ok(())
    }

    #[tokio::test]
    async fn treasury_settlement_is_pay_after_verify_and_idempotent() -> Result<()> {
        let app = test_router();
        let (provider_base_url, shutdown) = spawn_compute_provider_stub().await?;

        let create_provider = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/internal/v1/workers")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&serde_json::json!({
                        "worker_id": "desktop:settle-provider-1",
                        "owner_user_id": 11,
                        "metadata": {
                            "roles": ["client", "provider"],
                            "provider_id": "provider-settle-1",
                            "provider_base_url": provider_base_url.clone(),
                            "capabilities": ["oa.sandbox_run.v1"],
                            "min_price_msats": 1000,
                            "caps": { "max_timeout_secs": 120 }
                        }
                    }))?))?,
            )
            .await?;
        assert_eq!(create_provider.status(), axum::http::StatusCode::CREATED);

        let create_run = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/internal/v1/runs")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&serde_json::json!({
                        "worker_id": "desktop:settlement-worker",
                        "metadata": {"policy_bundle_id": "test"}
                    }))?))?,
            )
            .await?;
        assert_eq!(create_run.status(), axum::http::StatusCode::CREATED);
        let run_json = response_json(create_run).await?;
        let run_id = run_json
            .pointer("/run/id")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow!("missing run id"))?
            .to_string();

        let request = protocol::SandboxRunRequest {
            sandbox: protocol::jobs::sandbox::SandboxConfig {
                image_digest: "sha256:test".to_string(),
                network_policy: protocol::jobs::sandbox::NetworkPolicy::None,
                resources: protocol::jobs::sandbox::ResourceLimits {
                    timeout_secs: 30,
                    ..Default::default()
                },
                ..Default::default()
            },
            commands: vec![protocol::jobs::sandbox::SandboxCommand::new("echo hi")],
            ..Default::default()
        };

        let dispatch = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/internal/v1/marketplace/dispatch/sandbox-run")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&serde_json::json!({
                        "owner_user_id": 11,
                        "request": request.clone(),
                    }))?))?,
            )
            .await?;
        assert_eq!(dispatch.status(), axum::http::StatusCode::OK);
        let dispatch_json = response_json(dispatch).await?;
        let response_value = dispatch_json
            .get("response")
            .cloned()
            .ok_or_else(|| anyhow!("missing dispatch response"))?;

        let settle = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/internal/v1/treasury/compute/settle/sandbox-run")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&serde_json::json!({
                        "owner_user_id": 11,
                        "run_id": run_id.clone(),
                        "provider_id": "provider-settle-1",
                        "provider_worker_id": "desktop:settle-provider-1",
                        "amount_msats": 1000,
                        "request": request.clone(),
                        "response": response_value.clone(),
                    }))?))?,
            )
            .await?;
        assert_eq!(settle.status(), axum::http::StatusCode::OK);
        let settle_json = response_json(settle).await?;
        assert_eq!(
            settle_json
                .pointer("/settlement_status")
                .and_then(Value::as_str),
            Some("released")
        );
        assert_eq!(
            settle_json
                .pointer("/verification_passed")
                .and_then(Value::as_bool),
            Some(true)
        );

        let worker_after_settle = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/internal/v1/workers/desktop:settle-provider-1?owner_user_id=11")
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(worker_after_settle.status(), axum::http::StatusCode::OK);
        let worker_after_settle_json = response_json(worker_after_settle).await?;
        assert_eq!(
            worker_after_settle_json
                .pointer("/worker/worker/metadata/success_count")
                .and_then(Value::as_u64),
            Some(1)
        );

        let receipt = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/internal/v1/runs/{run_id}/receipt"))
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(receipt.status(), axum::http::StatusCode::OK);
        let receipt_json = response_json(receipt).await?;
        assert_eq!(
            receipt_json
                .pointer("/metrics/payments_msats_total")
                .and_then(Value::as_u64),
            Some(1000)
        );

        // Idempotent settle should not add additional payment events.
        let settle_retry = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/internal/v1/treasury/compute/settle/sandbox-run")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&serde_json::json!({
                        "owner_user_id": 11,
                        "run_id": run_id.clone(),
                        "provider_id": "provider-settle-1",
                        "provider_worker_id": "desktop:settle-provider-1",
                        "amount_msats": 1000,
                        "request": request.clone(),
                        "response": response_value.clone(),
                    }))?))?,
            )
            .await?;
        assert_eq!(settle_retry.status(), axum::http::StatusCode::OK);
        let receipt_retry = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/internal/v1/runs/{run_id}/receipt"))
                    .body(Body::empty())?,
            )
            .await?;
        let receipt_retry_json = response_json(receipt_retry).await?;
        assert_eq!(
            receipt_retry_json
                .pointer("/metrics/payments_msats_total")
                .and_then(Value::as_u64),
            Some(1000)
        );

        let worker_after_retry = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/internal/v1/workers/desktop:settle-provider-1?owner_user_id=11")
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(worker_after_retry.status(), axum::http::StatusCode::OK);
        let worker_after_retry_json = response_json(worker_after_retry).await?;
        assert_eq!(
            worker_after_retry_json
                .pointer("/worker/worker/metadata/success_count")
                .and_then(Value::as_u64),
            Some(1)
        );

        // Append-run-events endpoint must reject direct payment writes.
        let reject_payment = app
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri(format!("/internal/v1/runs/{run_id}/events"))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&serde_json::json!({
                        "event_type": "payment",
                        "payload": {"rail":"lightning","asset_id":"BTC_LN","amount_msats":1,"payment_proof":{}}
                    }))?))?,
            )
            .await?;
        assert_eq!(reject_payment.status(), axum::http::StatusCode::BAD_REQUEST);

        let _ = shutdown.send(());
        Ok(())
    }

    #[tokio::test]
    async fn provider_is_auto_quarantined_after_repeated_verification_violations() -> Result<()> {
        let app = test_router();
        let (provider_base_url, shutdown) = spawn_provider_stub().await?;

        let create_provider = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/internal/v1/workers")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&serde_json::json!({
                        "worker_id": "desktop:quarantine-provider-1",
                        "owner_user_id": 11,
                        "metadata": {
                            "roles": ["client", "provider"],
                            "provider_id": "provider-quarantine-1",
                            "provider_base_url": provider_base_url.clone(),
                            "capabilities": ["oa.sandbox_run.v1"],
                            "min_price_msats": 1000
                        }
                    }))?))?,
            )
            .await?;
        assert_eq!(create_provider.status(), axum::http::StatusCode::CREATED);

        let create_run = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/internal/v1/runs")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&serde_json::json!({
                        "worker_id": "desktop:quarantine-runner",
                        "metadata": {"policy_bundle_id": "test"}
                    }))?))?,
            )
            .await?;
        assert_eq!(create_run.status(), axum::http::StatusCode::CREATED);
        let run_json = response_json(create_run).await?;
        let run_id = run_json
            .pointer("/run/id")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow!("missing run id"))?
            .to_string();

        for idx in 0..3 {
            let request = protocol::SandboxRunRequest {
                sandbox: protocol::jobs::sandbox::SandboxConfig {
                    image_digest: "sha256:test".to_string(),
                    network_policy: protocol::jobs::sandbox::NetworkPolicy::None,
                    ..Default::default()
                },
                commands: vec![protocol::jobs::sandbox::SandboxCommand::new(format!(
                    "echo hi {idx}"
                ))],
                ..Default::default()
            };
            let bad_response = protocol::SandboxRunResponse {
                env_info: protocol::jobs::sandbox::EnvInfo {
                    image_digest: "sha256:test".to_string(),
                    hostname: None,
                    system_info: None,
                },
                runs: vec![protocol::jobs::sandbox::CommandResult {
                    cmd: "echo bye".to_string(),
                    exit_code: 0,
                    duration_ms: 1,
                    stdout_sha256: "stdout".to_string(),
                    stderr_sha256: "stderr".to_string(),
                    stdout_preview: None,
                    stderr_preview: None,
                }],
                artifacts: Vec::new(),
                status: protocol::jobs::sandbox::SandboxStatus::Success,
                error: None,
                provenance: protocol::Provenance::new("test"),
            };

            let settle_response = app
                .clone()
                .oneshot(
                    Request::builder()
                        .method(Method::POST)
                        .uri("/internal/v1/treasury/compute/settle/sandbox-run")
                        .header("content-type", "application/json")
                        .body(Body::from(serde_json::to_vec(&serde_json::json!({
                            "owner_user_id": 11,
                            "run_id": run_id.clone(),
                            "provider_id": "provider-quarantine-1",
                            "provider_worker_id": "desktop:quarantine-provider-1",
                            "amount_msats": 1000,
                            "request": request,
                            "response": bad_response,
                        }))?))?,
                )
                .await?;
            assert_eq!(settle_response.status(), axum::http::StatusCode::OK);
        }

        let catalog_response = app
            .oneshot(
                Request::builder()
                    .uri("/internal/v1/marketplace/catalog/providers?owner_user_id=11")
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(catalog_response.status(), axum::http::StatusCode::OK);
        let catalog_json = response_json(catalog_response).await?;
        let providers = catalog_json
            .get("providers")
            .and_then(Value::as_array)
            .ok_or_else(|| anyhow!("missing providers array"))?;
        assert_eq!(providers.len(), 1);
        assert_eq!(
            providers[0].get("quarantined").and_then(Value::as_bool),
            Some(true)
        );

        let _ = shutdown.send(());
        Ok(())
    }

    #[tokio::test]
    async fn route_provider_prefers_owned_and_falls_back_to_reserve_pool() -> Result<()> {
        let app = test_router();
        let (provider_base_url, shutdown) = spawn_provider_stub().await?;

        let create_owned = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/internal/v1/workers")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&serde_json::json!({
                        "worker_id": "desktop:routing-owned",
                        "owner_user_id": 11,
                        "metadata": {
                            "roles": ["client", "provider"],
                            "provider_id": "provider-owned",
                            "provider_base_url": provider_base_url.clone(),
                            "capabilities": ["oa.sandbox_run.v1"],
                            "min_price_msats": 1000
                        }
                    }))?))?,
            )
            .await?;
        assert_eq!(create_owned.status(), axum::http::StatusCode::CREATED);

        let create_reserve = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/internal/v1/workers")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&serde_json::json!({
                        "worker_id": "desktop:routing-reserve",
                        "owner_user_id": 99,
                        "metadata": {
                            "roles": ["client", "provider"],
                            "provider_id": "provider-reserve",
                            "provider_base_url": provider_base_url.clone(),
                            "capabilities": ["oa.sandbox_run.v1"],
                            "min_price_msats": 2000,
                            "reserve_pool": true
                        }
                    }))?))?,
            )
            .await?;
        assert_eq!(create_reserve.status(), axum::http::StatusCode::CREATED);

        let route_owned = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/internal/v1/marketplace/route/provider")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&serde_json::json!({
                        "owner_user_id": 11,
                        "capability": "oa.sandbox_run.v1"
                    }))?))?,
            )
            .await?;
        assert_eq!(route_owned.status(), axum::http::StatusCode::OK);
        let route_owned_json = response_json(route_owned).await?;
        assert_eq!(
            route_owned_json
                .pointer("/provider/provider_id")
                .and_then(Value::as_str),
            Some("provider-owned")
        );
        assert_eq!(
            route_owned_json
                .pointer("/provider/supply_class")
                .and_then(Value::as_str),
            Some("single_node")
        );
        assert_eq!(
            route_owned_json.get("tier").and_then(Value::as_str),
            Some("owned")
        );

        let stop_owned = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/internal/v1/workers/desktop:routing-owned/status")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&serde_json::json!({
                        "owner_user_id": 11,
                        "status": "failed",
                        "reason": "offline"
                    }))?))?,
            )
            .await?;
        assert_eq!(stop_owned.status(), axum::http::StatusCode::OK);

        let route_reserve = app
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/internal/v1/marketplace/route/provider")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&serde_json::json!({
                        "owner_user_id": 11,
                        "capability": "oa.sandbox_run.v1"
                    }))?))?,
            )
            .await?;
        assert_eq!(route_reserve.status(), axum::http::StatusCode::OK);
        let route_reserve_json = response_json(route_reserve).await?;
        assert_eq!(
            route_reserve_json
                .pointer("/provider/provider_id")
                .and_then(Value::as_str),
            Some("provider-reserve")
        );
        assert_eq!(
            route_reserve_json
                .pointer("/provider/supply_class")
                .and_then(Value::as_str),
            Some("reserve_pool")
        );
        assert_eq!(
            route_reserve_json.get("tier").and_then(Value::as_str),
            Some("reserve_pool")
        );

        let _ = shutdown.send(());
        Ok(())
    }

    #[tokio::test]
    async fn job_type_registry_surfaces_verification_metadata() -> Result<()> {
        let app = test_router();

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/internal/v1/marketplace/catalog/job-types")
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(response.status(), axum::http::StatusCode::OK);

        let json = response_json(response).await?;
        let job_types = json
            .get("job_types")
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| anyhow!("missing job_types array"))?;

        let sandbox = job_types
            .iter()
            .find(|value| {
                value.get("job_type").and_then(serde_json::Value::as_str)
                    == Some(<protocol::SandboxRunRequest as protocol::JobRequest>::JOB_TYPE)
            })
            .ok_or_else(|| anyhow!("missing sandbox job type info"))?;

        let default_verification_mode = sandbox
            .pointer("/default_verification/mode")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| anyhow!("missing sandbox default verification mode"))?;
        assert_eq!(default_verification_mode, "objective");

        Ok(())
    }

    #[tokio::test]
    async fn sandbox_verification_endpoint_matches_objective_semantics() -> Result<()> {
        let app = test_router();

        let request = protocol::SandboxRunRequest {
            sandbox: protocol::jobs::sandbox::SandboxConfig {
                image_digest: "sha256:test".to_string(),
                ..Default::default()
            },
            commands: vec![protocol::jobs::sandbox::SandboxCommand::new("echo hi")],
            ..Default::default()
        };

        let response = protocol::SandboxRunResponse {
            env_info: protocol::jobs::sandbox::EnvInfo {
                image_digest: "sha256:test".to_string(),
                hostname: None,
                system_info: None,
            },
            runs: vec![protocol::jobs::sandbox::CommandResult {
                cmd: "echo hi".to_string(),
                exit_code: 0,
                duration_ms: 1,
                stdout_sha256: "stdout".to_string(),
                stderr_sha256: "stderr".to_string(),
                stdout_preview: None,
                stderr_preview: None,
            }],
            artifacts: Vec::new(),
            status: protocol::jobs::sandbox::SandboxStatus::Success,
            error: None,
            provenance: protocol::Provenance::new("test"),
        };

        let ok_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/internal/v1/verifications/sandbox-run")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&serde_json::json!({
                        "request": request.clone(),
                        "response": response.clone()
                    }))?))?,
            )
            .await?;
        assert_eq!(ok_response.status(), axum::http::StatusCode::OK);
        let ok_json = response_json(ok_response).await?;
        assert_eq!(ok_json.get("passed").and_then(Value::as_bool), Some(true));
        assert_eq!(ok_json.get("exit_code").and_then(Value::as_i64), Some(0));

        let mut failing_response = response;
        failing_response.runs[0].exit_code = 2;
        failing_response.status = protocol::jobs::sandbox::SandboxStatus::Failed;

        let fail_response = app
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/internal/v1/verifications/sandbox-run")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&serde_json::json!({
                        "request": request,
                        "response": failing_response
                    }))?))?,
            )
            .await?;
        assert_eq!(fail_response.status(), axum::http::StatusCode::OK);
        let fail_json = response_json(fail_response).await?;
        assert_eq!(
            fail_json.get("passed").and_then(Value::as_bool),
            Some(false)
        );
        assert_eq!(fail_json.get("exit_code").and_then(Value::as_i64), Some(2));
        Ok(())
    }

    #[tokio::test]
    async fn repo_index_verification_endpoint_matches_objective_semantics() -> Result<()> {
        let app = test_router();

        let digests = vec![
            protocol::jobs::repo_index::RepoFileDigest {
                path: "README.md".to_string(),
                sha256: "0".repeat(64),
                bytes: 1,
            },
            protocol::jobs::repo_index::RepoFileDigest {
                path: "src/lib.rs".to_string(),
                sha256: "1".repeat(64),
                bytes: 2,
            },
        ];
        let tree = protocol::jobs::repo_index::compute_tree_sha256(&digests)?;

        let request = protocol::RepoIndexRequest {
            expected_tree_sha256: tree.clone(),
            ..Default::default()
        };
        let response = protocol::RepoIndexResponse {
            tree_sha256: tree.clone(),
            digests: digests.clone(),
            artifacts: Vec::new(),
            provenance: protocol::Provenance::new("test"),
        };

        let ok_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/internal/v1/verifications/repo-index")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&serde_json::json!({
                        "request": request.clone(),
                        "response": response.clone()
                    }))?))?,
            )
            .await?;
        assert_eq!(ok_response.status(), axum::http::StatusCode::OK);
        let ok_json = response_json(ok_response).await?;
        assert_eq!(ok_json.get("passed").and_then(Value::as_bool), Some(true));
        assert_eq!(
            ok_json.get("tree_sha256").and_then(Value::as_str),
            Some(tree.as_str())
        );
        assert_eq!(
            ok_json
                .get("violations")
                .and_then(Value::as_array)
                .map(Vec::len),
            Some(0)
        );

        let mut failing_request = request;
        failing_request.expected_tree_sha256 = "f".repeat(64);

        let fail_response = app
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/internal/v1/verifications/repo-index")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&serde_json::json!({
                        "request": failing_request,
                        "response": response
                    }))?))?,
            )
            .await?;
        assert_eq!(fail_response.status(), axum::http::StatusCode::OK);
        let fail_json = response_json(fail_response).await?;
        assert_eq!(
            fail_json.get("passed").and_then(Value::as_bool),
            Some(false)
        );
        assert!(
            fail_json
                .get("violations")
                .and_then(Value::as_array)
                .map(Vec::len)
                .unwrap_or(0)
                > 0
        );

        Ok(())
    }

    #[tokio::test]
    async fn khala_fleet_topic_surfaces_worker_presence_stream() -> Result<()> {
        let app = test_router();
        let sync_token = issue_sync_token(
            &["runtime.worker_lifecycle_events"],
            Some(11),
            Some("user:11"),
            "fleet-presence",
            300,
        );

        let create_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/internal/v1/workers")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&serde_json::json!({
                        "worker_id": "desktop:fleet-worker-1",
                        "owner_user_id": 11,
                        "metadata": {"roles": ["client"]}
                    }))?))?,
            )
            .await?;
        assert_eq!(create_response.status(), axum::http::StatusCode::CREATED);

        let poll = app
            .oneshot(
                Request::builder()
                    .uri("/internal/v1/khala/topics/fleet:user:11:workers/messages?after_seq=0&limit=10")
                    .header("authorization", format!("Bearer {sync_token}"))
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(poll.status(), axum::http::StatusCode::OK);
        let poll_json = response_json(poll).await?;
        let messages = poll_json
            .get("messages")
            .and_then(Value::as_array)
            .ok_or_else(|| anyhow!("missing messages array"))?;
        assert!(!messages.is_empty());
        Ok(())
    }

    #[tokio::test]
    async fn khala_fleet_topic_enforces_user_binding() -> Result<()> {
        let app = test_router();
        let sync_token = issue_sync_token(
            &["runtime.worker_lifecycle_events"],
            Some(11),
            Some("user:11"),
            "fleet-binding",
            300,
        );

        let poll = app
            .oneshot(
                Request::builder()
                    .uri("/internal/v1/khala/topics/fleet:user:12:workers/messages?after_seq=0&limit=10")
                    .header("authorization", format!("Bearer {sync_token}"))
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(poll.status(), axum::http::StatusCode::FORBIDDEN);
        Ok(())
    }
}
