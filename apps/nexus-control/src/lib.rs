mod economy;
mod kernel;

use std::collections::{BTreeMap, HashMap};
use std::convert::Infallible;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;
use std::sync::{Arc, RwLock};

use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use openagents_kernel_core::authority::{
    CreateContractRequest, CreateContractResponse, CreateWorkUnitRequest, CreateWorkUnitResponse,
    FinalizeVerdictRequest, FinalizeVerdictResponse, SubmitOutputRequest, SubmitOutputResponse,
};
use openagents_kernel_core::receipts::Receipt;
use openagents_kernel_core::snapshots::EconomySnapshot;
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;
use tokio_stream::StreamExt;
use tokio_stream::wrappers::BroadcastStream;

use crate::economy::{
    AuthorityReceiptContext, PublicRuntimeSnapshot, PublicStatsSnapshot, ReceiptLedger,
};
use crate::kernel::{
    KernelMutationContext, KernelState, ReceiptProjectionEvent, SnapshotProjectionEvent,
};

const ENV_LISTEN_ADDR: &str = "NEXUS_CONTROL_LISTEN_ADDR";
const ENV_SESSION_TTL_SECONDS: &str = "NEXUS_CONTROL_SESSION_TTL_SECONDS";
const ENV_SYNC_TOKEN_TTL_SECONDS: &str = "NEXUS_CONTROL_SYNC_TOKEN_TTL_SECONDS";
const ENV_SYNC_TOKEN_REFRESH_AFTER_SECONDS: &str = "NEXUS_CONTROL_SYNC_TOKEN_REFRESH_AFTER_SECONDS";
const ENV_SYNC_STREAM_GRANTS: &str = "NEXUS_CONTROL_SYNC_STREAM_GRANTS";
const ENV_HOSTED_NEXUS_RELAY_URL: &str = "NEXUS_CONTROL_HOSTED_NEXUS_RELAY_URL";
const ENV_RECEIPT_LOG_PATH: &str = "NEXUS_CONTROL_RECEIPT_LOG_PATH";
const ENV_STARTER_DEMAND_BUDGET_CAP_SATS: &str = "NEXUS_CONTROL_STARTER_DEMAND_BUDGET_CAP_SATS";
const ENV_STARTER_DEMAND_DISPATCH_INTERVAL_SECONDS: &str =
    "NEXUS_CONTROL_STARTER_DEMAND_DISPATCH_INTERVAL_SECONDS";
const ENV_STARTER_DEMAND_REQUEST_TTL_SECONDS: &str =
    "NEXUS_CONTROL_STARTER_DEMAND_REQUEST_TTL_SECONDS";
const ENV_STARTER_DEMAND_MAX_ACTIVE_OFFERS_PER_SESSION: &str =
    "NEXUS_CONTROL_STARTER_DEMAND_MAX_ACTIVE_OFFERS_PER_SESSION";
const ENV_STARTER_DEMAND_START_CONFIRM_SECONDS: &str =
    "NEXUS_CONTROL_STARTER_DEMAND_START_CONFIRM_SECONDS";
const ENV_STARTER_DEMAND_HEARTBEAT_TIMEOUT_SECONDS: &str =
    "NEXUS_CONTROL_STARTER_DEMAND_HEARTBEAT_TIMEOUT_SECONDS";

const DEFAULT_LISTEN_ADDR: &str = "127.0.0.1:42020";
const DEFAULT_SESSION_TTL_SECONDS: u64 = 86_400;
const DEFAULT_SYNC_TOKEN_TTL_SECONDS: u64 = 900;
const DEFAULT_SYNC_TOKEN_REFRESH_AFTER_SECONDS: u64 = 300;
const DEFAULT_HOSTED_NEXUS_RELAY_URL: &str = "wss://relay.openagents.dev";
const DEFAULT_STARTER_DEMAND_BUDGET_CAP_SATS: u64 = 5_000;
const DEFAULT_STARTER_DEMAND_DISPATCH_INTERVAL_SECONDS: u64 = 12;
const DEFAULT_STARTER_DEMAND_REQUEST_TTL_SECONDS: u64 = 75;
const DEFAULT_STARTER_DEMAND_MAX_ACTIVE_OFFERS_PER_SESSION: usize = 1;
const DEFAULT_STARTER_DEMAND_START_CONFIRM_SECONDS: u64 = 15;
const DEFAULT_STARTER_DEMAND_HEARTBEAT_TIMEOUT_SECONDS: u64 = 30;
const DEFAULT_SYNC_STREAM_GRANTS: [&str; 2] = [
    "stream.activity_projection.v1",
    "stream.earn_job_lifecycle_projection.v1",
];
const DEFAULT_SYNC_SCOPES: [&str; 3] = ["sync.subscribe", "sync.checkpoint.write", "sync.append"];
const STARTER_DEMAND_REQUESTER: &str = "openagents-hosted-nexus";
const STARTER_DEMAND_REQUEST_KIND: u16 = 5050;
const AUTOPILOT_DESKTOP_CLIENT_ID_PREFIX: &str = "autopilot-desktop";

#[derive(Debug, Clone)]
pub struct ServiceConfig {
    pub listen_addr: SocketAddr,
    pub session_ttl_seconds: u64,
    pub sync_token_ttl_seconds: u64,
    pub sync_token_refresh_after_seconds: u64,
    pub sync_stream_grants: Vec<String>,
    pub hosted_nexus_relay_url: String,
    pub receipt_log_path: Option<PathBuf>,
    pub starter_demand_budget_cap_sats: u64,
    pub starter_demand_dispatch_interval_seconds: u64,
    pub starter_demand_request_ttl_seconds: u64,
    pub starter_demand_max_active_offers_per_session: usize,
    pub starter_demand_start_confirm_seconds: u64,
    pub starter_demand_heartbeat_timeout_seconds: u64,
}

impl ServiceConfig {
    pub fn from_env() -> Result<Self, String> {
        let listen_addr = std::env::var(ENV_LISTEN_ADDR)
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| DEFAULT_LISTEN_ADDR.to_string())
            .parse::<SocketAddr>()
            .map_err(|error| format!("invalid {ENV_LISTEN_ADDR}: {error}"))?;
        let session_ttl_seconds =
            parse_u64_env(ENV_SESSION_TTL_SECONDS, DEFAULT_SESSION_TTL_SECONDS)?;
        let sync_token_ttl_seconds =
            parse_u64_env(ENV_SYNC_TOKEN_TTL_SECONDS, DEFAULT_SYNC_TOKEN_TTL_SECONDS)?;
        let sync_token_refresh_after_seconds = parse_u64_env(
            ENV_SYNC_TOKEN_REFRESH_AFTER_SECONDS,
            DEFAULT_SYNC_TOKEN_REFRESH_AFTER_SECONDS,
        )?;
        if sync_token_refresh_after_seconds == 0 {
            return Err(format!(
                "{ENV_SYNC_TOKEN_REFRESH_AFTER_SECONDS} must be greater than zero"
            ));
        }
        if sync_token_refresh_after_seconds >= sync_token_ttl_seconds {
            return Err(format!(
                "{ENV_SYNC_TOKEN_REFRESH_AFTER_SECONDS} must be less than {ENV_SYNC_TOKEN_TTL_SECONDS}"
            ));
        }

        let sync_stream_grants = std::env::var(ENV_SYNC_STREAM_GRANTS)
            .ok()
            .map(|value| {
                value
                    .split(',')
                    .map(str::trim)
                    .filter(|item| !item.is_empty())
                    .map(ToOwned::to_owned)
                    .collect::<Vec<_>>()
            })
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| {
                DEFAULT_SYNC_STREAM_GRANTS
                    .iter()
                    .map(|value| value.to_string())
                    .collect()
            });

        let hosted_nexus_relay_url = std::env::var(ENV_HOSTED_NEXUS_RELAY_URL)
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| DEFAULT_HOSTED_NEXUS_RELAY_URL.to_string());
        let receipt_log_path = std::env::var(ENV_RECEIPT_LOG_PATH)
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .map(PathBuf::from);
        let starter_demand_budget_cap_sats = parse_u64_env(
            ENV_STARTER_DEMAND_BUDGET_CAP_SATS,
            DEFAULT_STARTER_DEMAND_BUDGET_CAP_SATS,
        )?
        .max(1);
        let starter_demand_dispatch_interval_seconds = parse_u64_env(
            ENV_STARTER_DEMAND_DISPATCH_INTERVAL_SECONDS,
            DEFAULT_STARTER_DEMAND_DISPATCH_INTERVAL_SECONDS,
        )?;
        let starter_demand_request_ttl_seconds = parse_u64_env(
            ENV_STARTER_DEMAND_REQUEST_TTL_SECONDS,
            DEFAULT_STARTER_DEMAND_REQUEST_TTL_SECONDS,
        )?;
        let starter_demand_max_active_offers_per_session = parse_usize_env(
            ENV_STARTER_DEMAND_MAX_ACTIVE_OFFERS_PER_SESSION,
            DEFAULT_STARTER_DEMAND_MAX_ACTIVE_OFFERS_PER_SESSION,
        )?;
        let starter_demand_start_confirm_seconds = parse_u64_env(
            ENV_STARTER_DEMAND_START_CONFIRM_SECONDS,
            DEFAULT_STARTER_DEMAND_START_CONFIRM_SECONDS,
        )?;
        let starter_demand_heartbeat_timeout_seconds = parse_u64_env(
            ENV_STARTER_DEMAND_HEARTBEAT_TIMEOUT_SECONDS,
            DEFAULT_STARTER_DEMAND_HEARTBEAT_TIMEOUT_SECONDS,
        )?;
        if starter_demand_dispatch_interval_seconds == 0 {
            return Err(format!(
                "{ENV_STARTER_DEMAND_DISPATCH_INTERVAL_SECONDS} must be greater than zero"
            ));
        }
        if starter_demand_request_ttl_seconds == 0 {
            return Err(format!(
                "{ENV_STARTER_DEMAND_REQUEST_TTL_SECONDS} must be greater than zero"
            ));
        }
        if starter_demand_max_active_offers_per_session == 0 {
            return Err(format!(
                "{ENV_STARTER_DEMAND_MAX_ACTIVE_OFFERS_PER_SESSION} must be greater than zero"
            ));
        }
        if starter_demand_start_confirm_seconds == 0 {
            return Err(format!(
                "{ENV_STARTER_DEMAND_START_CONFIRM_SECONDS} must be greater than zero"
            ));
        }
        if starter_demand_start_confirm_seconds >= starter_demand_request_ttl_seconds {
            return Err(format!(
                "{ENV_STARTER_DEMAND_START_CONFIRM_SECONDS} must be less than {ENV_STARTER_DEMAND_REQUEST_TTL_SECONDS}"
            ));
        }
        if starter_demand_heartbeat_timeout_seconds == 0 {
            return Err(format!(
                "{ENV_STARTER_DEMAND_HEARTBEAT_TIMEOUT_SECONDS} must be greater than zero"
            ));
        }
        if starter_demand_heartbeat_timeout_seconds >= starter_demand_request_ttl_seconds {
            return Err(format!(
                "{ENV_STARTER_DEMAND_HEARTBEAT_TIMEOUT_SECONDS} must be less than {ENV_STARTER_DEMAND_REQUEST_TTL_SECONDS}"
            ));
        }

        Ok(Self {
            listen_addr,
            session_ttl_seconds,
            sync_token_ttl_seconds,
            sync_token_refresh_after_seconds,
            sync_stream_grants,
            hosted_nexus_relay_url,
            receipt_log_path,
            starter_demand_budget_cap_sats,
            starter_demand_dispatch_interval_seconds,
            starter_demand_request_ttl_seconds,
            starter_demand_max_active_offers_per_session,
            starter_demand_start_confirm_seconds,
            starter_demand_heartbeat_timeout_seconds,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DesktopSessionCreateRequest {
    pub desktop_client_id: String,
    pub device_name: Option<String>,
    pub bound_nostr_pubkey: Option<String>,
    pub client_version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DesktopSessionResponse {
    pub session_id: String,
    pub account_id: String,
    pub access_token: String,
    pub token_type: String,
    pub desktop_client_id: String,
    pub device_name: Option<String>,
    pub bound_nostr_pubkey: Option<String>,
    pub client_version: Option<String>,
    pub issued_at_unix_ms: u64,
    pub expires_at_unix_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SyncTokenClaimsResponse {
    pub session_id: String,
    pub account_id: String,
    pub scope: Vec<String>,
    pub stream_grants: Vec<String>,
    pub issued_at_unix_ms: u64,
    pub not_before_unix_ms: u64,
    pub expires_at_unix_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SyncTokenResponse {
    pub token: String,
    pub transport: String,
    pub protocol_version: String,
    pub refresh_after_in: u64,
    pub rotation_id: String,
    pub claims: SyncTokenClaimsResponse,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionMeResponse {
    pub session_id: String,
    pub account_id: String,
    pub desktop_client_id: String,
    pub device_name: Option<String>,
    pub bound_nostr_pubkey: Option<String>,
    pub client_version: Option<String>,
    pub issued_at_unix_ms: u64,
    pub expires_at_unix_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StarterDemandPollRequest {
    pub provider_nostr_pubkey: Option<String>,
    pub primary_relay_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StarterDemandOffer {
    pub request_id: String,
    pub requester: String,
    pub request_kind: u16,
    pub capability: String,
    pub execution_input: Option<String>,
    pub price_sats: u64,
    pub ttl_seconds: u64,
    pub created_at_unix_ms: u64,
    pub expires_at_unix_ms: u64,
    pub status: String,
    pub start_confirm_by_unix_ms: Option<u64>,
    pub execution_started_at_unix_ms: Option<u64>,
    pub execution_expires_at_unix_ms: Option<u64>,
    pub last_heartbeat_at_unix_ms: Option<u64>,
    pub next_heartbeat_due_at_unix_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StarterDemandPollResponse {
    pub authority: String,
    pub hosted_nexus_relay_url: String,
    pub eligible: bool,
    pub reason: Option<String>,
    pub budget_cap_sats: u64,
    pub budget_allocated_sats: u64,
    pub dispatch_interval_seconds: u64,
    pub request_ttl_seconds: u64,
    pub max_active_offers_per_session: usize,
    pub start_confirm_seconds: u64,
    pub heartbeat_timeout_seconds: u64,
    pub heartbeat_interval_seconds: u64,
    pub offers: Vec<StarterDemandOffer>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StarterDemandAckRequest {
    pub provider_nostr_pubkey: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StarterDemandAckResponse {
    pub request_id: String,
    pub status: String,
    pub started_at_unix_ms: u64,
    pub execution_expires_at_unix_ms: u64,
    pub last_heartbeat_at_unix_ms: u64,
    pub next_heartbeat_due_at_unix_ms: u64,
    pub heartbeat_timeout_seconds: u64,
    pub heartbeat_interval_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StarterDemandHeartbeatRequest {
    pub provider_nostr_pubkey: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StarterDemandHeartbeatResponse {
    pub request_id: String,
    pub status: String,
    pub last_heartbeat_at_unix_ms: u64,
    pub next_heartbeat_due_at_unix_ms: u64,
    pub execution_expires_at_unix_ms: u64,
    pub heartbeat_timeout_seconds: u64,
    pub heartbeat_interval_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StarterDemandFailRequest {
    pub failure_reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StarterDemandFailResponse {
    pub request_id: String,
    pub status: String,
    pub released_at_unix_ms: u64,
    pub failure_reason: String,
    pub budget_cap_sats: u64,
    pub budget_allocated_sats: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StarterDemandCompleteRequest {
    pub payment_pointer: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StarterDemandCompleteResponse {
    pub request_id: String,
    pub status: String,
    pub payment_pointer: String,
    pub completed_at_unix_ms: u64,
    pub budget_cap_sats: u64,
    pub budget_allocated_sats: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HealthResponse {
    pub ok: bool,
    pub service: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct ErrorResponse {
    error: String,
    reason: String,
}

#[derive(Debug, Clone)]
struct AppState {
    config: ServiceConfig,
    store: Arc<RwLock<ControlStore>>,
    kernel_receipt_tx: broadcast::Sender<ReceiptProjectionEvent>,
    kernel_snapshot_tx: broadcast::Sender<SnapshotProjectionEvent>,
}

#[derive(Debug)]
struct ControlStore {
    sessions_by_access_token: HashMap<String, DesktopSessionRecord>,
    sync_tokens: HashMap<String, SyncTokenRecord>,
    starter_demand: StarterDemandState,
    economy: ReceiptLedger,
    kernel: KernelState,
}

impl ControlStore {
    fn new(config: &ServiceConfig) -> Self {
        Self {
            sessions_by_access_token: HashMap::new(),
            sync_tokens: HashMap::new(),
            starter_demand: StarterDemandState::default(),
            economy: ReceiptLedger::new(config.receipt_log_path.clone()),
            kernel: KernelState::new(),
        }
    }
}

#[derive(Debug, Default)]
struct StarterDemandState {
    budget_allocated_sats: u64,
    next_offer_seq: u64,
    next_template_index: usize,
    last_dispatch_by_session: HashMap<String, u64>,
    offers_by_session: HashMap<String, Vec<StarterDemandOfferRecord>>,
}

#[derive(Debug, Clone)]
struct DesktopSessionRecord {
    session_id: String,
    account_id: String,
    desktop_client_id: String,
    device_name: Option<String>,
    bound_nostr_pubkey: Option<String>,
    client_version: Option<String>,
    issued_at_unix_ms: u64,
    expires_at_unix_ms: u64,
}

#[derive(Debug, Clone)]
struct SyncTokenRecord {
    token: String,
    rotation_id: String,
    session_id: String,
    account_id: String,
    scopes: Vec<String>,
    stream_grants: Vec<String>,
    issued_at_unix_ms: u64,
    not_before_unix_ms: u64,
    expires_at_unix_ms: u64,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum StarterOfferStatus {
    Offered,
    Running,
    Completed,
    Released,
    Expired,
}

impl StarterOfferStatus {
    const fn label(self) -> &'static str {
        match self {
            Self::Offered => "awaiting_ack",
            Self::Running => "running",
            Self::Completed => "completed",
            Self::Released => "released",
            Self::Expired => "expired",
        }
    }
}

#[derive(Debug, Clone)]
struct StarterDemandOfferRecord {
    request_id: String,
    requester: String,
    request_kind: u16,
    capability: String,
    execution_input: Option<String>,
    price_sats: u64,
    ttl_seconds: u64,
    created_at_unix_ms: u64,
    expires_at_unix_ms: u64,
    start_confirm_by_unix_ms: Option<u64>,
    execution_started_at_unix_ms: Option<u64>,
    execution_expires_at_unix_ms: Option<u64>,
    last_heartbeat_at_unix_ms: Option<u64>,
    next_heartbeat_due_at_unix_ms: Option<u64>,
    status: StarterOfferStatus,
    payment_pointer: Option<String>,
    completed_at_unix_ms: Option<u64>,
    released_at_unix_ms: Option<u64>,
    failure_reason: Option<String>,
}

#[derive(Debug, Clone)]
struct StarterDemandOfferEvent {
    session_id: String,
    offer: StarterDemandOfferRecord,
}

#[derive(Debug, Clone, Copy)]
struct StarterDemandTemplate {
    capability: &'static str,
    summary: &'static str,
    payout_sats: u64,
}

const STARTER_DEMAND_TEMPLATES: [StarterDemandTemplate; 4] = [
    StarterDemandTemplate {
        capability: "starter.quest.text_generation",
        summary: "Summarize a short project update into three bullets.",
        payout_sats: 120,
    },
    StarterDemandTemplate {
        capability: "starter.quest.text_generation",
        summary: "Extract action items from a meeting note.",
        payout_sats: 150,
    },
    StarterDemandTemplate {
        capability: "starter.quest.text_generation",
        summary: "Translate a paragraph to plain English.",
        payout_sats: 90,
    },
    StarterDemandTemplate {
        capability: "starter.quest.text_generation",
        summary: "Classify a support ticket and suggest a response.",
        payout_sats: 110,
    },
];

#[derive(Debug, Clone)]
struct ApiError {
    status: StatusCode,
    error: &'static str,
    reason: String,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(ErrorResponse {
                error: self.error.to_string(),
                reason: self.reason,
            }),
        )
            .into_response()
    }
}

pub fn build_router(config: ServiceConfig) -> Router {
    let (kernel_receipt_tx, _) = broadcast::channel(256);
    let (kernel_snapshot_tx, _) = broadcast::channel(256);
    let state = AppState {
        store: Arc::new(RwLock::new(ControlStore::new(&config))),
        config,
        kernel_receipt_tx,
        kernel_snapshot_tx,
    };
    Router::new()
        .route("/healthz", get(healthz))
        .route("/stats", get(public_stats))
        .route("/api/session/desktop", post(create_desktop_session))
        .route("/api/session/me", get(session_me))
        .route("/api/sync/token", post(create_sync_token))
        .route("/api/starter-demand/poll", post(poll_starter_demand))
        .route(
            "/api/starter-demand/offers/{request_id}/ack",
            post(ack_starter_demand_offer),
        )
        .route(
            "/api/starter-demand/offers/{request_id}/heartbeat",
            post(heartbeat_starter_demand_offer),
        )
        .route(
            "/api/starter-demand/offers/{request_id}/fail",
            post(fail_starter_demand_offer),
        )
        .route(
            "/api/starter-demand/offers/{request_id}/complete",
            post(complete_starter_demand_offer),
        )
        .route("/v1/kernel/work_units", post(create_kernel_work_unit))
        .route("/v1/kernel/contracts", post(create_kernel_contract))
        .route("/v1/kernel/contracts/{contract_id}/submit", post(submit_kernel_output))
        .route(
            "/v1/kernel/contracts/{contract_id}/verdict/finalize",
            post(finalize_kernel_verdict),
        )
        .route("/v1/kernel/snapshots/{minute_start_ms}", get(get_kernel_snapshot))
        .route("/v1/kernel/receipts/{receipt_id}", get(get_kernel_receipt))
        .route("/v1/kernel/stream/receipts", get(stream_kernel_receipts))
        .route("/v1/kernel/stream/snapshots", get(stream_kernel_snapshots))
        .with_state(state)
}

pub async fn run_server(config: ServiceConfig) -> Result<(), anyhow::Error> {
    let listener = tokio::net::TcpListener::bind(config.listen_addr).await?;
    let local_addr = listener.local_addr()?;
    tracing::info!("nexus-control listening on {}", local_addr);
    axum::serve(listener, build_router(config)).await?;
    Ok(())
}

async fn healthz() -> Json<HealthResponse> {
    Json(HealthResponse {
        ok: true,
        service: "nexus-control".to_string(),
    })
}

async fn public_stats(
    State(state): State<AppState>,
) -> Result<Json<PublicStatsSnapshot>, ApiError> {
    let now = now_unix_ms();
    let mut store = state.store.write().map_err(|_| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        error: "internal_error",
        reason: "session_store_poisoned".to_string(),
    })?;
    let expired_events =
        prune_expired_starter_offers(&mut store.starter_demand, &state.config, now);
    record_expired_offer_receipts(&mut store, expired_events, now);
    let stats = store
        .economy
        .snapshot(&runtime_snapshot(&state.config, &store), now);
    Ok(Json(stats))
}

async fn create_desktop_session(
    State(state): State<AppState>,
    Json(request): Json<DesktopSessionCreateRequest>,
) -> Result<Json<DesktopSessionResponse>, ApiError> {
    let desktop_client_id = normalize_required_field(
        request.desktop_client_id.as_str(),
        "desktop_client_id_missing",
    )?;
    let now = now_unix_ms();
    let access_token = random_token();
    let session_id = format!("sess_{}", random_token());
    let account_id = format!(
        "desktop:{}",
        sanitize_identifier(desktop_client_id.as_str())
    );
    let issued_at_unix_ms = now;
    let expires_at_unix_ms =
        now.saturating_add(state.config.session_ttl_seconds.saturating_mul(1_000));
    let record = DesktopSessionRecord {
        session_id: session_id.clone(),
        account_id: account_id.clone(),
        desktop_client_id: desktop_client_id.clone(),
        device_name: normalize_optional_field(request.device_name.as_deref()),
        bound_nostr_pubkey: normalize_optional_field(request.bound_nostr_pubkey.as_deref()),
        client_version: normalize_optional_field(request.client_version.as_deref()),
        issued_at_unix_ms,
        expires_at_unix_ms,
    };

    let mut store = state.store.write().map_err(|_| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        error: "internal_error",
        reason: "session_store_poisoned".to_string(),
    })?;
    store
        .sessions_by_access_token
        .insert(access_token.clone(), record.clone());
    store.economy.record(
        "desktop_session.created",
        now,
        desktop_session_receipt_context(&record),
    );
    drop(store);

    Ok(Json(DesktopSessionResponse {
        session_id,
        account_id,
        access_token,
        token_type: "Bearer".to_string(),
        desktop_client_id,
        device_name: record.device_name,
        bound_nostr_pubkey: record.bound_nostr_pubkey,
        client_version: record.client_version,
        issued_at_unix_ms,
        expires_at_unix_ms,
    }))
}

async fn session_me(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<SessionMeResponse>, ApiError> {
    let session = authenticate_session(&state, &headers)?;
    Ok(Json(SessionMeResponse {
        session_id: session.session_id,
        account_id: session.account_id,
        desktop_client_id: session.desktop_client_id,
        device_name: session.device_name,
        bound_nostr_pubkey: session.bound_nostr_pubkey,
        client_version: session.client_version,
        issued_at_unix_ms: session.issued_at_unix_ms,
        expires_at_unix_ms: session.expires_at_unix_ms,
    }))
}

async fn create_sync_token(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<SyncTokenResponse>, ApiError> {
    let session = authenticate_session(&state, &headers)?;
    let now = now_unix_ms();
    let token = format!("sync_{}", random_token());
    let rotation_id = format!("rot_{}", random_token());
    let scopes = DEFAULT_SYNC_SCOPES
        .iter()
        .map(|value| value.to_string())
        .collect::<Vec<_>>();
    let stream_grants = state.config.sync_stream_grants.clone();
    let expires_at_unix_ms =
        now.saturating_add(state.config.sync_token_ttl_seconds.saturating_mul(1_000));
    let session_id = session.session_id.clone();
    let account_id = session.account_id.clone();
    let record = SyncTokenRecord {
        token: token.clone(),
        rotation_id: rotation_id.clone(),
        session_id,
        account_id,
        scopes,
        stream_grants,
        issued_at_unix_ms: now,
        not_before_unix_ms: now,
        expires_at_unix_ms,
    };
    let mut store = state.store.write().map_err(|_| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        error: "internal_error",
        reason: "session_store_poisoned".to_string(),
    })?;
    store.sync_tokens.insert(token.clone(), record.clone());
    store.economy.record(
        "sync_token.issued",
        now,
        sync_token_receipt_context(&session, &record),
    );
    drop(store);

    Ok(Json(SyncTokenResponse {
        token: record.token,
        transport: "spacetime_ws".to_string(),
        protocol_version: "spacetime.sync.v1".to_string(),
        refresh_after_in: state.config.sync_token_refresh_after_seconds,
        rotation_id: record.rotation_id,
        claims: SyncTokenClaimsResponse {
            session_id: record.session_id,
            account_id: record.account_id,
            scope: record.scopes,
            stream_grants: record.stream_grants,
            issued_at_unix_ms: record.issued_at_unix_ms,
            not_before_unix_ms: record.not_before_unix_ms,
            expires_at_unix_ms: record.expires_at_unix_ms,
        },
    }))
}

async fn poll_starter_demand(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<StarterDemandPollRequest>,
) -> Result<Json<StarterDemandPollResponse>, ApiError> {
    let session = authenticate_session(&state, &headers)?;
    let now = now_unix_ms();
    let relay_url = normalize_optional_field(request.primary_relay_url.as_deref());
    let provider_nostr_pubkey = normalize_optional_field(request.provider_nostr_pubkey.as_deref());
    let mut response = StarterDemandPollResponse {
        authority: "openagents-hosted-nexus".to_string(),
        hosted_nexus_relay_url: state.config.hosted_nexus_relay_url.clone(),
        eligible: false,
        reason: None,
        budget_cap_sats: state.config.starter_demand_budget_cap_sats,
        budget_allocated_sats: 0,
        dispatch_interval_seconds: state.config.starter_demand_dispatch_interval_seconds,
        request_ttl_seconds: state.config.starter_demand_request_ttl_seconds,
        max_active_offers_per_session: state.config.starter_demand_max_active_offers_per_session,
        start_confirm_seconds: state.config.starter_demand_start_confirm_seconds,
        heartbeat_timeout_seconds: state.config.starter_demand_heartbeat_timeout_seconds,
        heartbeat_interval_seconds: starter_demand_heartbeat_interval_seconds(&state.config),
        offers: Vec::new(),
    };

    if relay_url.as_deref() != Some(state.config.hosted_nexus_relay_url.as_str()) {
        let mut store = state.store.write().map_err(|_| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            error: "internal_error",
            reason: "session_store_poisoned".to_string(),
        })?;
        response.budget_allocated_sats = store.starter_demand.budget_allocated_sats;
        let reason = "starter_demand_requires_openagents_hosted_nexus".to_string();
        response.reason = Some(reason.clone());
        store.economy.record(
            "starter_demand.ineligible",
            now,
            starter_offer_receipt_context(
                Some(&session),
                None,
                Some(reason.as_str()),
                relay_url.as_deref(),
                None,
            ),
        );
        return Ok(Json(response));
    }
    if let Some(reason) =
        starter_demand_provider_proof_reason(&session, provider_nostr_pubkey.as_deref())
    {
        let mut store = state.store.write().map_err(|_| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            error: "internal_error",
            reason: "session_store_poisoned".to_string(),
        })?;
        response.budget_allocated_sats = store.starter_demand.budget_allocated_sats;
        response.reason = Some(reason.clone());
        store.economy.record(
            "starter_demand.ineligible",
            now,
            starter_offer_receipt_context(
                Some(&session),
                None,
                Some(reason.as_str()),
                relay_url.as_deref(),
                None,
            ),
        );
        return Ok(Json(response));
    }

    let mut store = state.store.write().map_err(|_| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        error: "internal_error",
        reason: "session_store_poisoned".to_string(),
    })?;
    let expired_events =
        prune_expired_starter_offers(&mut store.starter_demand, &state.config, now);
    record_expired_offer_receipts(&mut store, expired_events, now);
    let dispatched_offer =
        maybe_dispatch_starter_offer(&mut store.starter_demand, &state.config, &session, now);
    if let Some(offer) = dispatched_offer {
        store.economy.record(
            "starter_offer.dispatched",
            now,
            starter_offer_receipt_context(
                Some(&session),
                Some(&offer),
                None,
                relay_url.as_deref(),
                None,
            ),
        );
    }
    response.eligible = true;
    response.budget_allocated_sats = store.starter_demand.budget_allocated_sats;
    response.offers = store
        .starter_demand
        .offers_by_session
        .get(session.session_id.as_str())
        .into_iter()
        .flat_map(|offers| offers.iter())
        .filter(|offer| {
            matches!(
                offer.status,
                StarterOfferStatus::Offered | StarterOfferStatus::Running
            )
        })
        .map(starter_offer_response)
        .collect();
    Ok(Json(response))
}

async fn ack_starter_demand_offer(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(request_id): Path<String>,
    Json(request): Json<StarterDemandAckRequest>,
) -> Result<Json<StarterDemandAckResponse>, ApiError> {
    let session = authenticate_session(&state, &headers)?;
    let request_id = normalize_required_field(request_id.as_str(), "request_id_missing")?;
    if let Some(reason) = starter_demand_provider_proof_reason(
        &session,
        normalize_optional_field(request.provider_nostr_pubkey.as_deref()).as_deref(),
    ) {
        return Err(ApiError {
            status: StatusCode::FORBIDDEN,
            error: "forbidden",
            reason,
        });
    }
    let now = now_unix_ms();

    let mut store = state.store.write().map_err(|_| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        error: "internal_error",
        reason: "session_store_poisoned".to_string(),
    })?;
    let expired_events =
        prune_expired_starter_offers(&mut store.starter_demand, &state.config, now);
    record_expired_offer_receipts(&mut store, expired_events, now);
    let mut started_receipt_offer = None;
    let mut expired_offer = None;
    let mut released_budget_sats = 0u64;
    let response = {
        let offers = store
            .starter_demand
            .offers_by_session
            .get_mut(session.session_id.as_str())
            .ok_or_else(|| ApiError {
                status: StatusCode::NOT_FOUND,
                error: "not_found",
                reason: "starter_offer_not_found".to_string(),
            })?;
        let offer = offers
            .iter_mut()
            .find(|offer| offer.request_id == request_id)
            .ok_or_else(|| ApiError {
                status: StatusCode::NOT_FOUND,
                error: "not_found",
                reason: "starter_offer_not_found".to_string(),
            })?;

        if offer.status == StarterOfferStatus::Offered {
            let start_confirm_by_unix_ms = offer
                .start_confirm_by_unix_ms
                .unwrap_or(offer.created_at_unix_ms);
            if now > start_confirm_by_unix_ms {
                released_budget_sats =
                    expire_offer(offer, now, "starter_offer_start_confirm_missed");
                expired_offer = Some(offer.clone());
                None
            } else {
                let heartbeat_timeout_ms = state
                    .config
                    .starter_demand_heartbeat_timeout_seconds
                    .saturating_mul(1_000);
                let execution_expires_at_unix_ms = now.saturating_add(
                    state
                        .config
                        .starter_demand_request_ttl_seconds
                        .saturating_mul(1_000),
                );
                offer.status = StarterOfferStatus::Running;
                offer.execution_started_at_unix_ms = Some(now);
                offer.execution_expires_at_unix_ms = Some(execution_expires_at_unix_ms);
                offer.last_heartbeat_at_unix_ms = Some(now);
                offer.next_heartbeat_due_at_unix_ms =
                    Some(now.saturating_add(heartbeat_timeout_ms));
                offer.expires_at_unix_ms = execution_expires_at_unix_ms;
                started_receipt_offer = Some(offer.clone());
                Some(StarterDemandAckResponse {
                    request_id: offer.request_id.clone(),
                    status: offer.status.label().to_string(),
                    started_at_unix_ms: offer.execution_started_at_unix_ms.unwrap_or(now),
                    execution_expires_at_unix_ms: offer.execution_expires_at_unix_ms.unwrap_or(now),
                    last_heartbeat_at_unix_ms: offer.last_heartbeat_at_unix_ms.unwrap_or(now),
                    next_heartbeat_due_at_unix_ms: offer
                        .next_heartbeat_due_at_unix_ms
                        .unwrap_or(now),
                    heartbeat_timeout_seconds: state
                        .config
                        .starter_demand_heartbeat_timeout_seconds,
                    heartbeat_interval_seconds: starter_demand_heartbeat_interval_seconds(
                        &state.config,
                    ),
                })
            }
        } else if offer.status != StarterOfferStatus::Running {
            return Err(ApiError {
                status: StatusCode::CONFLICT,
                error: "conflict",
                reason: format!("starter_offer_not_ackable status={}", offer.status.label()),
            });
        } else {
            Some(StarterDemandAckResponse {
                request_id: offer.request_id.clone(),
                status: offer.status.label().to_string(),
                started_at_unix_ms: offer.execution_started_at_unix_ms.unwrap_or(now),
                execution_expires_at_unix_ms: offer.execution_expires_at_unix_ms.unwrap_or(now),
                last_heartbeat_at_unix_ms: offer.last_heartbeat_at_unix_ms.unwrap_or(now),
                next_heartbeat_due_at_unix_ms: offer.next_heartbeat_due_at_unix_ms.unwrap_or(now),
                heartbeat_timeout_seconds: state.config.starter_demand_heartbeat_timeout_seconds,
                heartbeat_interval_seconds: starter_demand_heartbeat_interval_seconds(
                    &state.config,
                ),
            })
        }
    };

    if let Some(expired_offer) = expired_offer {
        store.starter_demand.budget_allocated_sats = store
            .starter_demand
            .budget_allocated_sats
            .saturating_sub(released_budget_sats);
        store.economy.record(
            "starter_offer.expired",
            now,
            starter_offer_receipt_context(
                Some(&session),
                Some(&expired_offer),
                expired_offer.failure_reason.as_deref(),
                None,
                None,
            ),
        );
        return Err(ApiError {
            status: StatusCode::CONFLICT,
            error: "conflict",
            reason: "starter_offer_start_confirm_missed".to_string(),
        });
    }

    if let Some(started_offer) = started_receipt_offer {
        store.economy.record(
            "starter_offer.started",
            now,
            starter_offer_receipt_context(Some(&session), Some(&started_offer), None, None, None),
        );
    }

    Ok(Json(response.ok_or_else(|| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        error: "internal_error",
        reason: "starter_offer_ack_response_missing".to_string(),
    })?))
}

async fn heartbeat_starter_demand_offer(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(request_id): Path<String>,
    Json(request): Json<StarterDemandHeartbeatRequest>,
) -> Result<Json<StarterDemandHeartbeatResponse>, ApiError> {
    let session = authenticate_session(&state, &headers)?;
    let request_id = normalize_required_field(request_id.as_str(), "request_id_missing")?;
    if let Some(reason) = starter_demand_provider_proof_reason(
        &session,
        normalize_optional_field(request.provider_nostr_pubkey.as_deref()).as_deref(),
    ) {
        return Err(ApiError {
            status: StatusCode::FORBIDDEN,
            error: "forbidden",
            reason,
        });
    }
    let now = now_unix_ms();

    let mut store = state.store.write().map_err(|_| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        error: "internal_error",
        reason: "session_store_poisoned".to_string(),
    })?;
    let expired_events =
        prune_expired_starter_offers(&mut store.starter_demand, &state.config, now);
    record_expired_offer_receipts(&mut store, expired_events, now);
    let mut expired_offer = None;
    let mut released_budget_sats = 0u64;
    let mut heartbeat_offer = None;
    let mut heartbeat_error_reason = None;
    let response = {
        let offers = store
            .starter_demand
            .offers_by_session
            .get_mut(session.session_id.as_str())
            .ok_or_else(|| ApiError {
                status: StatusCode::NOT_FOUND,
                error: "not_found",
                reason: "starter_offer_not_found".to_string(),
            })?;
        let offer = offers
            .iter_mut()
            .find(|offer| offer.request_id == request_id)
            .ok_or_else(|| ApiError {
                status: StatusCode::NOT_FOUND,
                error: "not_found",
                reason: "starter_offer_not_found".to_string(),
            })?;

        if offer.status != StarterOfferStatus::Running {
            return Err(ApiError {
                status: StatusCode::CONFLICT,
                error: "conflict",
                reason: format!("starter_offer_not_running status={}", offer.status.label()),
            });
        }

        let execution_expires_at_unix_ms = offer.execution_expires_at_unix_ms.unwrap_or(now);
        let next_heartbeat_due_at_unix_ms = offer.next_heartbeat_due_at_unix_ms.unwrap_or(now);
        if now > execution_expires_at_unix_ms {
            released_budget_sats = expire_offer(offer, now, "starter_offer_execution_expired");
            expired_offer = Some(offer.clone());
            heartbeat_error_reason = Some("starter_offer_execution_expired".to_string());
            None
        } else if now > next_heartbeat_due_at_unix_ms {
            released_budget_sats = expire_offer(offer, now, "starter_offer_heartbeat_missed");
            expired_offer = Some(offer.clone());
            heartbeat_error_reason = Some("starter_offer_heartbeat_missed".to_string());
            None
        } else {
            let heartbeat_timeout_ms = state
                .config
                .starter_demand_heartbeat_timeout_seconds
                .saturating_mul(1_000);
            offer.last_heartbeat_at_unix_ms = Some(now);
            offer.next_heartbeat_due_at_unix_ms = Some(now.saturating_add(heartbeat_timeout_ms));
            heartbeat_offer = Some(offer.clone());
            Some(StarterDemandHeartbeatResponse {
                request_id: offer.request_id.clone(),
                status: offer.status.label().to_string(),
                last_heartbeat_at_unix_ms: offer.last_heartbeat_at_unix_ms.unwrap_or(now),
                next_heartbeat_due_at_unix_ms: offer.next_heartbeat_due_at_unix_ms.unwrap_or(now),
                execution_expires_at_unix_ms,
                heartbeat_timeout_seconds: state.config.starter_demand_heartbeat_timeout_seconds,
                heartbeat_interval_seconds: starter_demand_heartbeat_interval_seconds(
                    &state.config,
                ),
            })
        }
    };

    if let Some(expired_offer) = expired_offer {
        store.starter_demand.budget_allocated_sats = store
            .starter_demand
            .budget_allocated_sats
            .saturating_sub(released_budget_sats);
        store.economy.record(
            "starter_offer.expired",
            now,
            starter_offer_receipt_context(
                Some(&session),
                Some(&expired_offer),
                expired_offer.failure_reason.as_deref(),
                None,
                None,
            ),
        );
        return Err(ApiError {
            status: StatusCode::CONFLICT,
            error: "conflict",
            reason: heartbeat_error_reason
                .unwrap_or_else(|| "starter_offer_heartbeat_conflict".to_string()),
        });
    }

    if let Some(heartbeat_offer) = heartbeat_offer {
        store.economy.record(
            "starter_offer.heartbeat",
            now,
            starter_offer_receipt_context(Some(&session), Some(&heartbeat_offer), None, None, None),
        );
    }

    Ok(Json(response.ok_or_else(|| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        error: "internal_error",
        reason: "starter_offer_heartbeat_response_missing".to_string(),
    })?))
}

async fn fail_starter_demand_offer(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(request_id): Path<String>,
    Json(request): Json<StarterDemandFailRequest>,
) -> Result<Json<StarterDemandFailResponse>, ApiError> {
    let session = authenticate_session(&state, &headers)?;
    let request_id = normalize_required_field(request_id.as_str(), "request_id_missing")?;
    let failure_reason =
        normalize_required_field(request.failure_reason.as_str(), "failure_reason_missing")?;
    let now = now_unix_ms();

    let mut store = state.store.write().map_err(|_| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        error: "internal_error",
        reason: "session_store_poisoned".to_string(),
    })?;
    let expired_events =
        prune_expired_starter_offers(&mut store.starter_demand, &state.config, now);
    record_expired_offer_receipts(&mut store, expired_events, now);
    let (
        response_request_id,
        response_status,
        response_released_at_unix_ms,
        released_budget_sats,
        released_offer,
    ) = {
        let offers = store
            .starter_demand
            .offers_by_session
            .get_mut(session.session_id.as_str())
            .ok_or_else(|| ApiError {
                status: StatusCode::NOT_FOUND,
                error: "not_found",
                reason: "starter_offer_not_found".to_string(),
            })?;
        let offer = offers
            .iter_mut()
            .find(|offer| offer.request_id == request_id)
            .ok_or_else(|| ApiError {
                status: StatusCode::NOT_FOUND,
                error: "not_found",
                reason: "starter_offer_not_found".to_string(),
            })?;

        let mut released_offer = None;
        let released_budget_sats = match offer.status {
            StarterOfferStatus::Offered | StarterOfferStatus::Running => {
                let released_budget_sats = release_offer(offer, now, failure_reason.as_str());
                released_offer = Some(offer.clone());
                released_budget_sats
            }
            StarterOfferStatus::Released => {
                if offer.failure_reason.as_deref() != Some(failure_reason.as_str()) {
                    return Err(ApiError {
                        status: StatusCode::CONFLICT,
                        error: "conflict",
                        reason: "starter_offer_already_released_with_different_reason".to_string(),
                    });
                }
                0
            }
            StarterOfferStatus::Completed | StarterOfferStatus::Expired => {
                return Err(ApiError {
                    status: StatusCode::CONFLICT,
                    error: "conflict",
                    reason: format!(
                        "starter_offer_not_releasable status={}",
                        offer.status.label()
                    ),
                });
            }
        };

        (
            offer.request_id.clone(),
            offer.status.label().to_string(),
            offer.released_at_unix_ms.unwrap_or(now),
            released_budget_sats,
            released_offer,
        )
    };
    store.starter_demand.budget_allocated_sats = store
        .starter_demand
        .budget_allocated_sats
        .saturating_sub(released_budget_sats);
    if let Some(released_offer) = released_offer {
        store.economy.record(
            "starter_offer.released",
            now,
            starter_offer_receipt_context(
                Some(&session),
                Some(&released_offer),
                released_offer.failure_reason.as_deref(),
                None,
                None,
            ),
        );
    }

    Ok(Json(StarterDemandFailResponse {
        request_id: response_request_id,
        status: response_status,
        released_at_unix_ms: response_released_at_unix_ms,
        failure_reason,
        budget_cap_sats: state.config.starter_demand_budget_cap_sats,
        budget_allocated_sats: store.starter_demand.budget_allocated_sats,
    }))
}

async fn complete_starter_demand_offer(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(request_id): Path<String>,
    Json(request): Json<StarterDemandCompleteRequest>,
) -> Result<Json<StarterDemandCompleteResponse>, ApiError> {
    let session = authenticate_session(&state, &headers)?;
    let request_id = normalize_required_field(request_id.as_str(), "request_id_missing")?;
    let payment_pointer =
        normalize_required_field(request.payment_pointer.as_str(), "payment_pointer_missing")?;
    let now = now_unix_ms();

    let mut store = state.store.write().map_err(|_| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        error: "internal_error",
        reason: "session_store_poisoned".to_string(),
    })?;
    let expired_events =
        prune_expired_starter_offers(&mut store.starter_demand, &state.config, now);
    record_expired_offer_receipts(&mut store, expired_events, now);
    let (
        response_request_id,
        response_status,
        response_completed_at_unix_ms,
        released_budget_sats,
        completed_offer,
    ) = {
        let offers = store
            .starter_demand
            .offers_by_session
            .get_mut(session.session_id.as_str())
            .ok_or_else(|| ApiError {
                status: StatusCode::NOT_FOUND,
                error: "not_found",
                reason: "starter_offer_not_found".to_string(),
            })?;
        let offer = offers
            .iter_mut()
            .find(|offer| offer.request_id == request_id)
            .ok_or_else(|| ApiError {
                status: StatusCode::NOT_FOUND,
                error: "not_found",
                reason: "starter_offer_not_found".to_string(),
            })?;

        let mut completed_offer = None;
        let released_budget_sats = match offer.status {
            StarterOfferStatus::Running => {
                offer.status = StarterOfferStatus::Completed;
                offer.payment_pointer = Some(payment_pointer.clone());
                offer.completed_at_unix_ms = Some(now);
                offer.last_heartbeat_at_unix_ms = Some(now);
                offer.next_heartbeat_due_at_unix_ms = None;
                completed_offer = Some(offer.clone());
                offer.price_sats
            }
            StarterOfferStatus::Completed => {
                if offer.payment_pointer.as_deref() != Some(payment_pointer.as_str()) {
                    return Err(ApiError {
                        status: StatusCode::CONFLICT,
                        error: "conflict",
                        reason: "starter_offer_already_completed_with_different_payment_pointer"
                            .to_string(),
                    });
                }
                0
            }
            StarterOfferStatus::Offered => {
                return Err(ApiError {
                    status: StatusCode::CONFLICT,
                    error: "conflict",
                    reason: "starter_offer_start_not_confirmed".to_string(),
                });
            }
            StarterOfferStatus::Released | StarterOfferStatus::Expired => {
                return Err(ApiError {
                    status: StatusCode::CONFLICT,
                    error: "conflict",
                    reason: format!(
                        "starter_offer_not_completable status={}",
                        offer.status.label()
                    ),
                });
            }
        };

        (
            offer.request_id.clone(),
            offer.status.label().to_string(),
            offer.completed_at_unix_ms.unwrap_or(now),
            released_budget_sats,
            completed_offer,
        )
    };
    store.starter_demand.budget_allocated_sats = store
        .starter_demand
        .budget_allocated_sats
        .saturating_sub(released_budget_sats);
    if let Some(completed_offer) = completed_offer {
        store.economy.record(
            "starter_offer.completed",
            now,
            starter_offer_receipt_context(
                Some(&session),
                Some(&completed_offer),
                None,
                None,
                Some(payment_pointer.as_str()),
            ),
        );
    }

    Ok(Json(StarterDemandCompleteResponse {
        request_id: response_request_id,
        status: response_status,
        payment_pointer,
        completed_at_unix_ms: response_completed_at_unix_ms,
        budget_cap_sats: state.config.starter_demand_budget_cap_sats,
        budget_allocated_sats: store.starter_demand.budget_allocated_sats,
    }))
}

async fn create_kernel_work_unit(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<CreateWorkUnitRequest>,
) -> Result<Json<CreateWorkUnitResponse>, ApiError> {
    let session = authenticate_session(&state, &headers)?;
    let now = now_unix_ms();
    let result = {
        let mut store = state.store.write().map_err(|_| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            error: "internal_error",
            reason: "session_store_poisoned".to_string(),
        })?;
        store
            .kernel
            .create_work_unit(&kernel_mutation_context(&session, now), request)
            .map_err(kernel_api_error)?
    };
    record_kernel_mutation_observability(
        &state,
        &session,
        now,
        "kernel.work_unit.created",
        result.receipt_event.clone(),
        result.snapshot_event.clone(),
    );
    Ok(Json(result.response))
}

async fn create_kernel_contract(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<CreateContractRequest>,
) -> Result<Json<CreateContractResponse>, ApiError> {
    let session = authenticate_session(&state, &headers)?;
    let now = now_unix_ms();
    let result = {
        let mut store = state.store.write().map_err(|_| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            error: "internal_error",
            reason: "session_store_poisoned".to_string(),
        })?;
        store
            .kernel
            .create_contract(&kernel_mutation_context(&session, now), request)
            .map_err(kernel_api_error)?
    };
    record_kernel_mutation_observability(
        &state,
        &session,
        now,
        "kernel.contract.created",
        result.receipt_event.clone(),
        result.snapshot_event.clone(),
    );
    Ok(Json(result.response))
}

async fn submit_kernel_output(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(contract_id): Path<String>,
    Json(mut request): Json<SubmitOutputRequest>,
) -> Result<Json<SubmitOutputResponse>, ApiError> {
    let session = authenticate_session(&state, &headers)?;
    let contract_id = normalize_required_field(contract_id.as_str(), "contract_id_missing")?;
    if !request.submission.contract_id.trim().is_empty() && request.submission.contract_id != contract_id {
        return Err(ApiError {
            status: StatusCode::CONFLICT,
            error: "conflict",
            reason: "kernel_contract_id_mismatch".to_string(),
        });
    }
    request.submission.contract_id = contract_id;
    let now = now_unix_ms();
    let result = {
        let mut store = state.store.write().map_err(|_| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            error: "internal_error",
            reason: "session_store_poisoned".to_string(),
        })?;
        store
            .kernel
            .submit_output(&kernel_mutation_context(&session, now), request)
            .map_err(kernel_api_error)?
    };
    record_kernel_mutation_observability(
        &state,
        &session,
        now,
        "kernel.submission.received",
        result.receipt_event.clone(),
        result.snapshot_event.clone(),
    );
    Ok(Json(result.response))
}

async fn finalize_kernel_verdict(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(contract_id): Path<String>,
    Json(mut request): Json<FinalizeVerdictRequest>,
) -> Result<Json<FinalizeVerdictResponse>, ApiError> {
    let session = authenticate_session(&state, &headers)?;
    let contract_id = normalize_required_field(contract_id.as_str(), "contract_id_missing")?;
    if !request.verdict.contract_id.trim().is_empty() && request.verdict.contract_id != contract_id {
        return Err(ApiError {
            status: StatusCode::CONFLICT,
            error: "conflict",
            reason: "kernel_contract_id_mismatch".to_string(),
        });
    }
    request.verdict.contract_id = contract_id;
    let now = now_unix_ms();
    let result = {
        let mut store = state.store.write().map_err(|_| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            error: "internal_error",
            reason: "session_store_poisoned".to_string(),
        })?;
        store
            .kernel
            .finalize_verdict(&kernel_mutation_context(&session, now), request)
            .map_err(kernel_api_error)?
    };
    record_kernel_mutation_observability(
        &state,
        &session,
        now,
        "kernel.verdict.finalized",
        result.receipt_event.clone(),
        result.snapshot_event.clone(),
    );
    Ok(Json(result.response))
}

async fn get_kernel_snapshot(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(minute_start_ms): Path<i64>,
) -> Result<Json<EconomySnapshot>, ApiError> {
    let _session = authenticate_session(&state, &headers)?;
    let snapshot = {
        let mut store = state.store.write().map_err(|_| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            error: "internal_error",
            reason: "session_store_poisoned".to_string(),
        })?;
        store
            .kernel
            .get_snapshot(minute_start_ms)
            .map_err(kernel_api_error)?
    };
    Ok(Json(snapshot))
}

async fn get_kernel_receipt(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(receipt_id): Path<String>,
) -> Result<Json<Receipt>, ApiError> {
    let _session = authenticate_session(&state, &headers)?;
    let receipt_id = normalize_required_field(receipt_id.as_str(), "receipt_id_missing")?;
    let receipt = {
        let store = state.store.read().map_err(|_| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            error: "internal_error",
            reason: "session_store_poisoned".to_string(),
        })?;
        store.kernel.get_receipt(receipt_id.as_str())
    }
    .ok_or_else(|| ApiError {
        status: StatusCode::NOT_FOUND,
        error: "not_found",
        reason: "kernel_receipt_not_found".to_string(),
    })?;
    Ok(Json(receipt))
}

async fn stream_kernel_receipts(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>>, ApiError> {
    let _session = authenticate_session(&state, &headers)?;
    let stream = BroadcastStream::new(state.kernel_receipt_tx.subscribe()).filter_map(|message| {
        match message {
            Ok(event) => {
                let data = serde_json::to_string(&event).ok()?;
                Some(Ok(Event::default().event("receipt").data(data)))
            }
            Err(_) => None,
        }
    });
    Ok(Sse::new(stream).keep_alive(KeepAlive::new().interval(Duration::from_secs(15))))
}

async fn stream_kernel_snapshots(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>>, ApiError> {
    let _session = authenticate_session(&state, &headers)?;
    let stream = BroadcastStream::new(state.kernel_snapshot_tx.subscribe()).filter_map(|message| {
        match message {
            Ok(event) => {
                let data = serde_json::to_string(&event).ok()?;
                Some(Ok(Event::default().event("snapshot").data(data)))
            }
            Err(_) => None,
        }
    });
    Ok(Sse::new(stream).keep_alive(KeepAlive::new().interval(Duration::from_secs(15))))
}

fn kernel_mutation_context(
    session: &DesktopSessionRecord,
    now_unix_ms: u64,
) -> KernelMutationContext {
    KernelMutationContext {
        caller_id: session.account_id.clone(),
        session_id: session.session_id.clone(),
        now_unix_ms,
    }
}

fn kernel_api_error(reason: String) -> ApiError {
    let status = match reason.as_str() {
        "kernel_contract_not_found" | "kernel_work_unit_not_found" => StatusCode::NOT_FOUND,
        "kernel_idempotency_conflict" | "kernel_contract_id_mismatch" => StatusCode::CONFLICT,
        "work_unit_id_missing" | "contract_id_missing" | "receipt_id_missing" => {
            StatusCode::BAD_REQUEST
        }
        _ => StatusCode::BAD_REQUEST,
    };
    ApiError {
        status,
        error: "kernel_error",
        reason,
    }
}

fn record_kernel_mutation_observability(
    state: &AppState,
    session: &DesktopSessionRecord,
    now_unix_ms: u64,
    receipt_type: &str,
    receipt_event: Option<ReceiptProjectionEvent>,
    snapshot_event: Option<SnapshotProjectionEvent>,
) {
    if let Some(receipt_event) = receipt_event {
        if let Ok(mut store) = state.store.write() {
            let mut attributes = BTreeMap::new();
            if let Some(work_unit_id) = receipt_event.receipt.trace.work_unit_id.clone() {
                attributes.insert("work_unit_id".to_string(), work_unit_id);
            }
            if let Some(contract_id) = receipt_event.receipt.trace.contract_id.clone() {
                attributes.insert("contract_id".to_string(), contract_id);
            }
            attributes.insert(
                "canonical_receipt_id".to_string(),
                receipt_event.receipt.receipt_id.clone(),
            );
            attributes.insert(
                "canonical_receipt_type".to_string(),
                receipt_event.receipt.receipt_type.clone(),
            );
            store.economy.record(
                receipt_type,
                now_unix_ms,
                AuthorityReceiptContext {
                    session_id: Some(session.session_id.clone()),
                    account_id: Some(session.account_id.clone()),
                    request_id: Some(receipt_event.receipt.receipt_id.clone()),
                    status: Some(if receipt_event.seq > 0 {
                        "recorded".to_string()
                    } else {
                        "replayed".to_string()
                    }),
                    reason: None,
                    relay_url: None,
                    amount_sats: None,
                    payment_pointer: None,
                    attributes,
                },
            );
        }
        let _ = state.kernel_receipt_tx.send(receipt_event);
    }
    if let Some(snapshot_event) = snapshot_event {
        let _ = state.kernel_snapshot_tx.send(snapshot_event);
    }
}

fn authenticate_session(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<DesktopSessionRecord, ApiError> {
    let header = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .ok_or_else(|| ApiError {
            status: StatusCode::UNAUTHORIZED,
            error: "unauthorized",
            reason: "missing_bearer_token".to_string(),
        })?;
    let token = header
        .strip_prefix("Bearer ")
        .or_else(|| header.strip_prefix("bearer "))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ApiError {
            status: StatusCode::UNAUTHORIZED,
            error: "unauthorized",
            reason: "invalid_bearer_token".to_string(),
        })?;
    let store = state.store.read().map_err(|_| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        error: "internal_error",
        reason: "session_store_poisoned".to_string(),
    })?;
    let session = store
        .sessions_by_access_token
        .get(token)
        .cloned()
        .ok_or_else(|| ApiError {
            status: StatusCode::UNAUTHORIZED,
            error: "unauthorized",
            reason: "session_not_found".to_string(),
        })?;
    drop(store);
    if now_unix_ms() >= session.expires_at_unix_ms {
        return Err(ApiError {
            status: StatusCode::UNAUTHORIZED,
            error: "unauthorized",
            reason: "session_expired".to_string(),
        });
    }
    Ok(session)
}

fn prune_expired_starter_offers(
    state: &mut StarterDemandState,
    config: &ServiceConfig,
    now_unix_ms: u64,
) -> Vec<StarterDemandOfferEvent> {
    let mut released_budget_sats = 0u64;
    let mut expired_events = Vec::new();
    for (session_id, offers) in &mut state.offers_by_session {
        for offer in offers.iter_mut() {
            if offer.status == StarterOfferStatus::Offered
                && now_unix_ms
                    > offer
                        .start_confirm_by_unix_ms
                        .unwrap_or(offer.created_at_unix_ms)
            {
                released_budget_sats = released_budget_sats.saturating_add(expire_offer(
                    offer,
                    now_unix_ms,
                    "starter_offer_start_confirm_missed",
                ));
                expired_events.push(StarterDemandOfferEvent {
                    session_id: session_id.clone(),
                    offer: offer.clone(),
                });
            } else if offer.status == StarterOfferStatus::Running {
                if now_unix_ms
                    > offer
                        .execution_expires_at_unix_ms
                        .unwrap_or(offer.expires_at_unix_ms)
                {
                    released_budget_sats = released_budget_sats.saturating_add(expire_offer(
                        offer,
                        now_unix_ms,
                        "starter_offer_execution_expired",
                    ));
                    expired_events.push(StarterDemandOfferEvent {
                        session_id: session_id.clone(),
                        offer: offer.clone(),
                    });
                } else if now_unix_ms
                    > offer
                        .next_heartbeat_due_at_unix_ms
                        .unwrap_or(offer.expires_at_unix_ms)
                {
                    released_budget_sats = released_budget_sats.saturating_add(expire_offer(
                        offer,
                        now_unix_ms,
                        "starter_offer_heartbeat_missed",
                    ));
                    expired_events.push(StarterDemandOfferEvent {
                        session_id: session_id.clone(),
                        offer: offer.clone(),
                    });
                }
            }
        }
        offers.retain(|offer| {
            if offer.status == StarterOfferStatus::Completed {
                return true;
            }
            if matches!(
                offer.status,
                StarterOfferStatus::Offered | StarterOfferStatus::Running
            ) {
                return true;
            }
            let terminal_at_unix_ms = offer
                .released_at_unix_ms
                .or(offer.completed_at_unix_ms)
                .unwrap_or(offer.expires_at_unix_ms);
            now_unix_ms.saturating_sub(terminal_at_unix_ms)
                < config
                    .starter_demand_dispatch_interval_seconds
                    .saturating_mul(1_000)
        });
    }
    state.budget_allocated_sats = state
        .budget_allocated_sats
        .saturating_sub(released_budget_sats);
    expired_events
}

fn maybe_dispatch_starter_offer(
    state: &mut StarterDemandState,
    config: &ServiceConfig,
    session: &DesktopSessionRecord,
    now_unix_ms: u64,
) -> Option<StarterDemandOfferRecord> {
    let active_offers = state
        .offers_by_session
        .get(session.session_id.as_str())
        .map(|offers| {
            offers
                .iter()
                .filter(|offer| {
                    matches!(
                        offer.status,
                        StarterOfferStatus::Offered | StarterOfferStatus::Running
                    )
                })
                .count()
        })
        .unwrap_or(0);
    if active_offers >= config.starter_demand_max_active_offers_per_session {
        return None;
    }

    let last_dispatch_at = state
        .last_dispatch_by_session
        .get(session.session_id.as_str())
        .copied()
        .unwrap_or(0);
    let dispatch_interval_ms = config
        .starter_demand_dispatch_interval_seconds
        .saturating_mul(1_000);
    if last_dispatch_at != 0 && now_unix_ms < last_dispatch_at.saturating_add(dispatch_interval_ms)
    {
        return None;
    }

    let remaining_budget_sats = config
        .starter_demand_budget_cap_sats
        .saturating_sub(state.budget_allocated_sats);
    let template = next_template_for_remaining_budget(state, remaining_budget_sats)?;

    let request_id = format!("starter-hosted-{:06}", state.next_offer_seq);
    state.next_offer_seq = state.next_offer_seq.saturating_add(1);
    let created_at_unix_ms = now_unix_ms;
    let ttl_seconds = config.starter_demand_request_ttl_seconds;
    let start_confirm_by_unix_ms = created_at_unix_ms.saturating_add(
        config
            .starter_demand_start_confirm_seconds
            .saturating_mul(1_000),
    );
    let offer = StarterDemandOfferRecord {
        request_id: request_id.clone(),
        requester: STARTER_DEMAND_REQUESTER.to_string(),
        request_kind: STARTER_DEMAND_REQUEST_KIND,
        capability: template.capability.to_string(),
        execution_input: Some(template.summary.to_string()),
        price_sats: template.payout_sats,
        ttl_seconds,
        created_at_unix_ms,
        expires_at_unix_ms: start_confirm_by_unix_ms,
        start_confirm_by_unix_ms: Some(start_confirm_by_unix_ms),
        execution_started_at_unix_ms: None,
        execution_expires_at_unix_ms: None,
        last_heartbeat_at_unix_ms: None,
        next_heartbeat_due_at_unix_ms: None,
        status: StarterOfferStatus::Offered,
        payment_pointer: None,
        completed_at_unix_ms: None,
        released_at_unix_ms: None,
        failure_reason: None,
    };
    state
        .offers_by_session
        .entry(session.session_id.clone())
        .or_default()
        .push(offer.clone());
    state.budget_allocated_sats = state
        .budget_allocated_sats
        .saturating_add(template.payout_sats);
    state
        .last_dispatch_by_session
        .insert(session.session_id.clone(), now_unix_ms);
    Some(offer)
}

fn starter_demand_provider_proof_reason(
    session: &DesktopSessionRecord,
    provider_nostr_pubkey: Option<&str>,
) -> Option<String> {
    if !session
        .desktop_client_id
        .starts_with(AUTOPILOT_DESKTOP_CLIENT_ID_PREFIX)
    {
        return Some("starter_demand_requires_autopilot_desktop_session".to_string());
    }
    let Some(bound_nostr_pubkey) = session
        .bound_nostr_pubkey
        .as_deref()
        .filter(|value| !value.is_empty())
    else {
        return Some("starter_demand_requires_bound_nostr_identity".to_string());
    };
    let Some(provider_nostr_pubkey) = provider_nostr_pubkey.filter(|value| !value.is_empty())
    else {
        return Some("starter_demand_provider_nostr_pubkey_missing".to_string());
    };
    if bound_nostr_pubkey != provider_nostr_pubkey {
        return Some("starter_demand_provider_nostr_pubkey_mismatch".to_string());
    }
    None
}

fn starter_demand_heartbeat_interval_seconds(config: &ServiceConfig) -> u64 {
    config
        .starter_demand_heartbeat_timeout_seconds
        .saturating_div(2)
        .max(1)
}

fn expire_offer(
    offer: &mut StarterDemandOfferRecord,
    now_unix_ms: u64,
    failure_reason: &str,
) -> u64 {
    if matches!(
        offer.status,
        StarterOfferStatus::Completed | StarterOfferStatus::Released | StarterOfferStatus::Expired
    ) {
        return 0;
    }
    let released_budget_sats = if matches!(
        offer.status,
        StarterOfferStatus::Offered | StarterOfferStatus::Running
    ) {
        offer.price_sats
    } else {
        0
    };
    offer.status = StarterOfferStatus::Expired;
    offer.released_at_unix_ms = Some(now_unix_ms);
    offer.failure_reason = Some(failure_reason.to_string());
    offer.next_heartbeat_due_at_unix_ms = None;
    offer.expires_at_unix_ms = now_unix_ms;
    released_budget_sats
}

fn release_offer(
    offer: &mut StarterDemandOfferRecord,
    now_unix_ms: u64,
    failure_reason: &str,
) -> u64 {
    if matches!(
        offer.status,
        StarterOfferStatus::Completed | StarterOfferStatus::Released | StarterOfferStatus::Expired
    ) {
        return 0;
    }
    let released_budget_sats = if matches!(
        offer.status,
        StarterOfferStatus::Offered | StarterOfferStatus::Running
    ) {
        offer.price_sats
    } else {
        0
    };
    offer.status = StarterOfferStatus::Released;
    offer.released_at_unix_ms = Some(now_unix_ms);
    offer.failure_reason = Some(failure_reason.to_string());
    offer.next_heartbeat_due_at_unix_ms = None;
    offer.expires_at_unix_ms = now_unix_ms;
    released_budget_sats
}

fn next_template_for_remaining_budget(
    state: &mut StarterDemandState,
    remaining_budget_sats: u64,
) -> Option<StarterDemandTemplate> {
    for offset in 0..STARTER_DEMAND_TEMPLATES.len() {
        let index = (state.next_template_index + offset) % STARTER_DEMAND_TEMPLATES.len();
        let template = STARTER_DEMAND_TEMPLATES[index];
        if template.payout_sats <= remaining_budget_sats {
            state.next_template_index = (index + 1) % STARTER_DEMAND_TEMPLATES.len();
            return Some(template);
        }
    }
    None
}

fn starter_offer_response(record: &StarterDemandOfferRecord) -> StarterDemandOffer {
    StarterDemandOffer {
        request_id: record.request_id.clone(),
        requester: record.requester.clone(),
        request_kind: record.request_kind,
        capability: record.capability.clone(),
        execution_input: record.execution_input.clone(),
        price_sats: record.price_sats,
        ttl_seconds: record.ttl_seconds,
        created_at_unix_ms: record.created_at_unix_ms,
        expires_at_unix_ms: record.expires_at_unix_ms,
        status: record.status.label().to_string(),
        start_confirm_by_unix_ms: record.start_confirm_by_unix_ms,
        execution_started_at_unix_ms: record.execution_started_at_unix_ms,
        execution_expires_at_unix_ms: record.execution_expires_at_unix_ms,
        last_heartbeat_at_unix_ms: record.last_heartbeat_at_unix_ms,
        next_heartbeat_due_at_unix_ms: record.next_heartbeat_due_at_unix_ms,
    }
}

fn record_expired_offer_receipts(
    store: &mut ControlStore,
    expired_events: Vec<StarterDemandOfferEvent>,
    now_unix_ms: u64,
) {
    for event in expired_events {
        let session = find_session_by_id(store, event.session_id.as_str()).cloned();
        store.economy.record(
            "starter_offer.expired",
            now_unix_ms,
            starter_offer_receipt_context(
                session.as_ref(),
                Some(&event.offer),
                event.offer.failure_reason.as_deref(),
                None,
                None,
            ),
        );
    }
}

fn find_session_by_id<'a>(
    store: &'a ControlStore,
    session_id: &str,
) -> Option<&'a DesktopSessionRecord> {
    store
        .sessions_by_access_token
        .values()
        .find(|session| session.session_id == session_id)
}

fn runtime_snapshot(config: &ServiceConfig, store: &ControlStore) -> PublicRuntimeSnapshot {
    let (starter_offers_waiting_ack, starter_offers_running) = store
        .starter_demand
        .offers_by_session
        .values()
        .flat_map(|offers| offers.iter())
        .fold(
            (0usize, 0usize),
            |(waiting_ack, running), offer| match offer.status {
                StarterOfferStatus::Offered => (waiting_ack.saturating_add(1), running),
                StarterOfferStatus::Running => (waiting_ack, running.saturating_add(1)),
                StarterOfferStatus::Completed
                | StarterOfferStatus::Released
                | StarterOfferStatus::Expired => (waiting_ack, running),
            },
        );
    PublicRuntimeSnapshot {
        hosted_nexus_relay_url: config.hosted_nexus_relay_url.clone(),
        sessions_active: store.sessions_by_access_token.len(),
        sync_tokens_active: store.sync_tokens.len(),
        starter_demand_budget_cap_sats: config.starter_demand_budget_cap_sats,
        starter_demand_budget_allocated_sats: store.starter_demand.budget_allocated_sats,
        starter_offers_waiting_ack,
        starter_offers_running,
    }
}

fn desktop_session_receipt_context(record: &DesktopSessionRecord) -> AuthorityReceiptContext {
    let mut attributes = std::collections::BTreeMap::new();
    attributes.insert(
        "desktop_client_id".to_string(),
        record.desktop_client_id.clone(),
    );
    if let Some(device_name) = record.device_name.clone() {
        attributes.insert("device_name".to_string(), device_name);
    }
    if let Some(client_version) = record.client_version.clone() {
        attributes.insert("client_version".to_string(), client_version);
    }
    if let Some(bound_nostr_pubkey) = record.bound_nostr_pubkey.clone() {
        attributes.insert("bound_nostr_pubkey".to_string(), bound_nostr_pubkey);
    }
    AuthorityReceiptContext {
        session_id: Some(record.session_id.clone()),
        account_id: Some(record.account_id.clone()),
        status: Some("issued".to_string()),
        attributes,
        ..AuthorityReceiptContext::default()
    }
}

fn sync_token_receipt_context(
    session: &DesktopSessionRecord,
    token: &SyncTokenRecord,
) -> AuthorityReceiptContext {
    let mut attributes = std::collections::BTreeMap::new();
    attributes.insert("rotation_id".to_string(), token.rotation_id.clone());
    attributes.insert("transport".to_string(), "spacetime_ws".to_string());
    attributes.insert(
        "protocol_version".to_string(),
        "spacetime.sync.v1".to_string(),
    );
    AuthorityReceiptContext {
        session_id: Some(session.session_id.clone()),
        account_id: Some(session.account_id.clone()),
        status: Some("issued".to_string()),
        attributes,
        ..AuthorityReceiptContext::default()
    }
}

fn starter_offer_receipt_context(
    session: Option<&DesktopSessionRecord>,
    offer: Option<&StarterDemandOfferRecord>,
    reason: Option<&str>,
    relay_url: Option<&str>,
    payment_pointer: Option<&str>,
) -> AuthorityReceiptContext {
    let mut attributes = std::collections::BTreeMap::new();
    if let Some(session) = session {
        attributes.insert(
            "desktop_client_id".to_string(),
            session.desktop_client_id.clone(),
        );
        if let Some(bound_nostr_pubkey) = session.bound_nostr_pubkey.clone() {
            attributes.insert("bound_nostr_pubkey".to_string(), bound_nostr_pubkey);
        }
    }
    if let Some(offer) = offer {
        attributes.insert("capability".to_string(), offer.capability.clone());
        attributes.insert("requester".to_string(), offer.requester.clone());
        attributes.insert("request_kind".to_string(), offer.request_kind.to_string());
    }
    AuthorityReceiptContext {
        session_id: session.map(|session| session.session_id.clone()),
        account_id: session.map(|session| session.account_id.clone()),
        request_id: offer.map(|offer| offer.request_id.clone()),
        status: offer.map(|offer| offer.status.label().to_string()),
        reason: reason.map(ToOwned::to_owned),
        relay_url: relay_url.map(ToOwned::to_owned),
        amount_sats: offer.map(|offer| offer.price_sats),
        payment_pointer: payment_pointer.map(ToOwned::to_owned),
        attributes,
    }
}

fn parse_u64_env(key: &str, default: u64) -> Result<u64, String> {
    std::env::var(key)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .map_or(Ok(default), |value| {
            value
                .parse::<u64>()
                .map_err(|error| format!("invalid {key}: {error}"))
        })
}

fn parse_usize_env(key: &str, default: usize) -> Result<usize, String> {
    std::env::var(key)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .map_or(Ok(default), |value| {
            value
                .parse::<usize>()
                .map_err(|error| format!("invalid {key}: {error}"))
        })
}

fn normalize_required_field(value: &str, reason: &str) -> Result<String, ApiError> {
    let normalized = value.trim();
    if normalized.is_empty() {
        return Err(ApiError {
            status: StatusCode::BAD_REQUEST,
            error: "invalid_request",
            reason: reason.to_string(),
        });
    }
    Ok(normalized.to_string())
}

fn normalize_optional_field(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|candidate| !candidate.is_empty())
        .map(ToOwned::to_owned)
}

fn sanitize_identifier(value: &str) -> String {
    let sanitized = value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '-' || character == '_' {
                character
            } else {
                '_'
            }
        })
        .collect::<String>();
    if sanitized.is_empty() {
        "desktop".to_string()
    } else {
        sanitized
    }
}

fn random_token() -> String {
    hex::encode(rand::random::<[u8; 24]>())
}

fn now_unix_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_millis() as u64)
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use axum::body::{Body, to_bytes};
    use axum::http::{Request, StatusCode};
    use axum::response::Response;
    use openagents_kernel_core::authority::{
        CreateContractRequest, CreateContractResponse, CreateWorkUnitRequest,
        CreateWorkUnitResponse, FinalizeVerdictRequest, FinalizeVerdictResponse,
        SubmitOutputRequest, SubmitOutputResponse,
    };
    use openagents_kernel_core::labor::{
        Contract, ContractStatus, SettlementLink, SettlementStatus, Submission, SubmissionStatus,
        Verdict, VerdictOutcome, WorkUnit, WorkUnitStatus,
    };
    use openagents_kernel_core::receipts::{
        Asset, Money, MoneyAmount, PolicyContext, Receipt, ReceiptHints, TraceContext,
        VerificationTier,
    };
    use openagents_kernel_core::time::floor_to_minute_utc;
    use serde_json::json;
    use tower::ServiceExt;

    use super::{
        DesktopSessionCreateRequest, DesktopSessionResponse, PublicStatsSnapshot, ServiceConfig,
        StarterDemandAckRequest, StarterDemandAckResponse, StarterDemandCompleteRequest,
        StarterDemandCompleteResponse, StarterDemandHeartbeatRequest,
        StarterDemandHeartbeatResponse, StarterDemandPollRequest, StarterDemandPollResponse,
        SyncTokenResponse, build_router,
    };

    fn test_config() -> Result<ServiceConfig> {
        Ok(ServiceConfig {
            listen_addr: "127.0.0.1:0".parse()?,
            session_ttl_seconds: 60,
            sync_token_ttl_seconds: 600,
            sync_token_refresh_after_seconds: 120,
            sync_stream_grants: vec![
                "stream.activity_projection.v1".to_string(),
                "stream.earn_job_lifecycle_projection.v1".to_string(),
            ],
            hosted_nexus_relay_url: "wss://relay.openagents.dev".to_string(),
            receipt_log_path: None,
            starter_demand_budget_cap_sats: 500,
            starter_demand_dispatch_interval_seconds: 1,
            starter_demand_request_ttl_seconds: 120,
            starter_demand_max_active_offers_per_session: 1,
            starter_demand_start_confirm_seconds: 5,
            starter_demand_heartbeat_timeout_seconds: 5,
        })
    }

    fn test_config_with_leases(
        start_confirm_seconds: u64,
        request_ttl_seconds: u64,
        heartbeat_timeout_seconds: u64,
    ) -> Result<ServiceConfig> {
        Ok(ServiceConfig {
            starter_demand_start_confirm_seconds: start_confirm_seconds,
            starter_demand_request_ttl_seconds: request_ttl_seconds,
            starter_demand_heartbeat_timeout_seconds: heartbeat_timeout_seconds,
            ..test_config()?
        })
    }

    async fn response_json<T: serde::de::DeserializeOwned>(response: Response) -> Result<T> {
        let bytes = to_bytes(response.into_body(), usize::MAX).await?;
        Ok(serde_json::from_slice(bytes.as_ref())?)
    }

    async fn create_session_token(app: &axum::Router) -> Result<DesktopSessionResponse> {
        create_session_token_for(app, "autopilot-desktop-alpha", Some("npub1alpha")).await
    }

    async fn create_session_token_for(
        app: &axum::Router,
        desktop_client_id: &str,
        bound_nostr_pubkey: Option<&str>,
    ) -> Result<DesktopSessionResponse> {
        let create_request = DesktopSessionCreateRequest {
            desktop_client_id: desktop_client_id.to_string(),
            device_name: Some("Chris MacBook".to_string()),
            bound_nostr_pubkey: bound_nostr_pubkey.map(str::to_string),
            client_version: Some("mvp".to_string()),
        };
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/session/desktop")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&create_request)?))?,
            )
            .await?;
        assert_eq!(response.status(), StatusCode::OK);
        response_json(response).await
    }

    fn authorization(session: &DesktopSessionResponse) -> String {
        format!("Bearer {}", session.access_token)
    }

    fn kernel_policy() -> PolicyContext {
        PolicyContext::default()
    }

    fn kernel_trace(work_unit_id: Option<&str>, contract_id: Option<&str>) -> TraceContext {
        TraceContext {
            work_unit_id: work_unit_id.map(str::to_string),
            contract_id: contract_id.map(str::to_string),
            ..TraceContext::default()
        }
    }

    fn work_unit_request(
        work_unit_id: &str,
        idempotency_key: &str,
        created_at_ms: i64,
    ) -> CreateWorkUnitRequest {
        CreateWorkUnitRequest {
            idempotency_key: idempotency_key.to_string(),
            trace: kernel_trace(Some(work_unit_id), None),
            policy: kernel_policy(),
            work_unit: WorkUnit {
                work_unit_id: work_unit_id.to_string(),
                external_request_id: Some(format!("request.{work_unit_id}")),
                requester_id: Some("buyer.alpha".to_string()),
                provider_id: Some("desktop".to_string()),
                capability: Some("starter.compute.job".to_string()),
                demand_source: Some("starter_demand".to_string()),
                created_at_ms,
                status: WorkUnitStatus::Created,
                quoted_price: Some(Money {
                    asset: Asset::Btc,
                    amount: MoneyAmount::AmountSats(21),
                }),
                metadata: json!({
                    "summary": "Run the provider earn loop job."
                }),
            },
            evidence: Vec::new(),
            hints: ReceiptHints::default(),
        }
    }

    fn contract_request(
        contract_id: &str,
        work_unit_id: &str,
        idempotency_key: &str,
        created_at_ms: i64,
    ) -> CreateContractRequest {
        CreateContractRequest {
            idempotency_key: idempotency_key.to_string(),
            trace: kernel_trace(Some(work_unit_id), Some(contract_id)),
            policy: kernel_policy(),
            contract: Contract {
                contract_id: contract_id.to_string(),
                work_unit_id: work_unit_id.to_string(),
                provider_id: Some("desktop".to_string()),
                created_at_ms,
                status: ContractStatus::Created,
                settlement_asset: Some(Asset::Btc),
                quoted_price: Some(Money {
                    asset: Asset::Btc,
                    amount: MoneyAmount::AmountSats(21),
                }),
                warranty_window_ms: Some(300_000),
                metadata: json!({
                    "provider": "desktop"
                }),
            },
            evidence: Vec::new(),
            hints: ReceiptHints::default(),
        }
    }

    fn submission_request(
        contract_id: &str,
        work_unit_id: &str,
        idempotency_key: &str,
        created_at_ms: i64,
    ) -> SubmitOutputRequest {
        SubmitOutputRequest {
            idempotency_key: idempotency_key.to_string(),
            trace: kernel_trace(Some(work_unit_id), Some(contract_id)),
            policy: kernel_policy(),
            submission: Submission {
                submission_id: format!("submission.{contract_id}"),
                contract_id: contract_id.to_string(),
                work_unit_id: work_unit_id.to_string(),
                created_at_ms,
                status: SubmissionStatus::Received,
                output_ref: Some("file://result.json".to_string()),
                provenance_digest: Some("sha256:submission.alpha".to_string()),
                metadata: json!({
                    "status": "completed"
                }),
            },
            evidence: Vec::new(),
            hints: ReceiptHints::default(),
        }
    }

    fn verdict_request(
        contract_id: &str,
        work_unit_id: &str,
        idempotency_key: &str,
        created_at_ms: i64,
    ) -> FinalizeVerdictRequest {
        let verdict_id = format!("verdict.{contract_id}");
        FinalizeVerdictRequest {
            idempotency_key: idempotency_key.to_string(),
            trace: kernel_trace(Some(work_unit_id), Some(contract_id)),
            policy: kernel_policy(),
            verdict: Verdict {
                verdict_id: verdict_id.clone(),
                contract_id: contract_id.to_string(),
                work_unit_id: work_unit_id.to_string(),
                created_at_ms,
                outcome: VerdictOutcome::Pass,
                verification_tier: Some(VerificationTier::TierOObjective),
                settlement_status: SettlementStatus::Settled,
                reason_code: Some("starter.compute.accepted".to_string()),
                metadata: json!({
                    "settlement": "approved"
                }),
            },
            settlement_link: Some(SettlementLink {
                settlement_id: format!("settlement.{contract_id}"),
                contract_id: contract_id.to_string(),
                work_unit_id: work_unit_id.to_string(),
                verdict_id,
                created_at_ms,
                payment_pointer: Some("wallet:receive:001".to_string()),
                settled_amount: Some(Money {
                    asset: Asset::Btc,
                    amount: MoneyAmount::AmountSats(21),
                }),
                status: SettlementStatus::Settled,
                metadata: json!({
                    "source": "starter_demand"
                }),
            }),
            claim_hook: None,
            evidence: Vec::new(),
            hints: ReceiptHints {
                verification_correlated: Some(false),
                ..ReceiptHints::default()
            },
        }
    }

    #[tokio::test]
    async fn desktop_session_flow_mints_bearer_and_sync_token() -> Result<()> {
        let app = build_router(test_config()?);
        let session = create_session_token(&app).await?;

        let token_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/sync/token")
                    .header("authorization", format!("Bearer {}", session.access_token))
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(token_response.status(), StatusCode::OK);
        let token: SyncTokenResponse = response_json(token_response).await?;
        assert_eq!(token.transport, "spacetime_ws");
        assert_eq!(token.protocol_version, "spacetime.sync.v1");
        assert!(
            token
                .claims
                .scope
                .iter()
                .any(|scope| scope == "sync.subscribe")
        );
        assert_eq!(
            token.claims.stream_grants,
            vec![
                "stream.activity_projection.v1".to_string(),
                "stream.earn_job_lifecycle_projection.v1".to_string()
            ]
        );

        Ok(())
    }

    #[tokio::test]
    async fn sync_token_requires_bearer_auth() -> Result<()> {
        let app = build_router(test_config()?);
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/sync/token")
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        let body: serde_json::Value = response_json(response).await?;
        assert_eq!(body["reason"], "missing_bearer_token");
        Ok(())
    }

    #[tokio::test]
    async fn public_stats_reflects_backend_session_and_sync_receipts() -> Result<()> {
        let app = build_router(test_config()?);

        let empty_stats = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/stats")
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(empty_stats.status(), StatusCode::OK);
        let empty: PublicStatsSnapshot = response_json(empty_stats).await?;
        assert_eq!(empty.receipt_count, 0);
        assert_eq!(empty.sessions_active, 0);
        assert_eq!(empty.sync_tokens_active, 0);

        let session = create_session_token(&app).await?;
        let token_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/sync/token")
                    .header("authorization", format!("Bearer {}", session.access_token))
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(token_response.status(), StatusCode::OK);

        let stats_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/stats")
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(stats_response.status(), StatusCode::OK);
        let stats: PublicStatsSnapshot = response_json(stats_response).await?;
        assert_eq!(stats.sessions_active, 1);
        assert_eq!(stats.sessions_issued_24h, 1);
        assert_eq!(stats.sync_tokens_active, 1);
        assert_eq!(stats.sync_tokens_issued_24h, 1);
        assert!(stats.receipt_count >= 2);
        assert!(
            stats
                .recent_receipts
                .iter()
                .any(|receipt| receipt.receipt_type == "desktop_session.created")
        );
        assert!(
            stats
                .recent_receipts
                .iter()
                .any(|receipt| receipt.receipt_type == "sync_token.issued")
        );
        Ok(())
    }

    #[tokio::test]
    async fn hosted_starter_demand_dispatches_and_completes_offer() -> Result<()> {
        let app = build_router(test_config()?);
        let session = create_session_token(&app).await?;

        let poll_request = StarterDemandPollRequest {
            provider_nostr_pubkey: Some("npub1alpha".to_string()),
            primary_relay_url: Some("wss://relay.openagents.dev".to_string()),
        };
        let first_poll = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/starter-demand/poll")
                    .header("authorization", format!("Bearer {}", session.access_token))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&poll_request)?))?,
            )
            .await?;
        assert_eq!(first_poll.status(), StatusCode::OK);
        let first: StarterDemandPollResponse = response_json(first_poll).await?;
        assert!(first.eligible);
        assert_eq!(first.offers.len(), 1);
        let request_id = first.offers[0].request_id.clone();

        let second_poll = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/starter-demand/poll")
                    .header("authorization", format!("Bearer {}", session.access_token))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&poll_request)?))?,
            )
            .await?;
        let second: StarterDemandPollResponse = response_json(second_poll).await?;
        assert_eq!(second.offers.len(), 1);
        assert_eq!(second.offers[0].request_id, request_id);

        let ack_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/starter-demand/offers/{}/ack", request_id))
                    .header("authorization", format!("Bearer {}", session.access_token))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&StarterDemandAckRequest {
                        provider_nostr_pubkey: Some("npub1alpha".to_string()),
                    })?))?,
            )
            .await?;
        assert_eq!(ack_response.status(), StatusCode::OK);
        let acked: StarterDemandAckResponse = response_json(ack_response).await?;
        assert_eq!(acked.request_id, request_id);
        assert_eq!(acked.status, "running");

        let heartbeat_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!(
                        "/api/starter-demand/offers/{}/heartbeat",
                        request_id
                    ))
                    .header("authorization", format!("Bearer {}", session.access_token))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(
                        &StarterDemandHeartbeatRequest {
                            provider_nostr_pubkey: Some("npub1alpha".to_string()),
                        },
                    )?))?,
            )
            .await?;
        assert_eq!(heartbeat_response.status(), StatusCode::OK);
        let heartbeat: StarterDemandHeartbeatResponse = response_json(heartbeat_response).await?;
        assert_eq!(heartbeat.request_id, request_id);
        assert_eq!(heartbeat.status, "running");

        let complete_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!(
                        "/api/starter-demand/offers/{}/complete",
                        request_id
                    ))
                    .header("authorization", format!("Bearer {}", session.access_token))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(
                        &StarterDemandCompleteRequest {
                            payment_pointer: "wallet:receive:001".to_string(),
                        },
                    )?))?,
            )
            .await?;
        assert_eq!(complete_response.status(), StatusCode::OK);
        let completed: StarterDemandCompleteResponse = response_json(complete_response).await?;
        assert_eq!(completed.request_id, request_id);
        assert_eq!(completed.status, "completed");
        assert_eq!(completed.payment_pointer, "wallet:receive:001");

        let stats_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/stats")
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(stats_response.status(), StatusCode::OK);
        let stats: PublicStatsSnapshot = response_json(stats_response).await?;
        assert_eq!(stats.starter_offers_dispatched_24h, 1);
        assert_eq!(stats.starter_offers_started_24h, 1);
        assert_eq!(stats.starter_offer_heartbeats_24h, 1);
        assert_eq!(stats.starter_offers_completed_24h, 1);
        assert_eq!(
            stats.starter_demand_paid_sats_24h,
            first.offers[0].price_sats
        );
        assert_eq!(stats.starter_offers_waiting_ack, 0);
        assert_eq!(stats.starter_offers_running, 0);

        Ok(())
    }

    #[tokio::test]
    async fn hosted_starter_demand_reissues_after_start_confirm_timeout() -> Result<()> {
        let app = build_router(test_config_with_leases(1, 10, 3)?);
        let session_alpha =
            create_session_token_for(&app, "autopilot-desktop-alpha", Some("npub1alpha")).await?;
        let session_beta =
            create_session_token_for(&app, "autopilot-desktop-beta", Some("npub1beta")).await?;

        let alpha_poll = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/starter-demand/poll")
                    .header(
                        "authorization",
                        format!("Bearer {}", session_alpha.access_token),
                    )
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&StarterDemandPollRequest {
                        provider_nostr_pubkey: Some("npub1alpha".to_string()),
                        primary_relay_url: Some("wss://relay.openagents.dev".to_string()),
                    })?))?,
            )
            .await?;
        assert_eq!(alpha_poll.status(), StatusCode::OK);
        let alpha_payload: StarterDemandPollResponse = response_json(alpha_poll).await?;
        let first_request_id = alpha_payload.offers[0].request_id.clone();

        tokio::time::sleep(std::time::Duration::from_millis(1_100)).await;

        let beta_poll = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/starter-demand/poll")
                    .header(
                        "authorization",
                        format!("Bearer {}", session_beta.access_token),
                    )
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&StarterDemandPollRequest {
                        provider_nostr_pubkey: Some("npub1beta".to_string()),
                        primary_relay_url: Some("wss://relay.openagents.dev".to_string()),
                    })?))?,
            )
            .await?;
        assert_eq!(beta_poll.status(), StatusCode::OK);
        let beta_payload: StarterDemandPollResponse = response_json(beta_poll).await?;
        assert_eq!(beta_payload.offers.len(), 1);
        assert_ne!(beta_payload.offers[0].request_id, first_request_id);

        Ok(())
    }

    #[tokio::test]
    async fn hosted_starter_demand_reissues_after_heartbeat_timeout() -> Result<()> {
        let app = build_router(test_config_with_leases(1, 10, 1)?);
        let session_alpha =
            create_session_token_for(&app, "autopilot-desktop-alpha", Some("npub1alpha")).await?;
        let session_beta =
            create_session_token_for(&app, "autopilot-desktop-beta", Some("npub1beta")).await?;

        let alpha_poll = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/starter-demand/poll")
                    .header(
                        "authorization",
                        format!("Bearer {}", session_alpha.access_token),
                    )
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&StarterDemandPollRequest {
                        provider_nostr_pubkey: Some("npub1alpha".to_string()),
                        primary_relay_url: Some("wss://relay.openagents.dev".to_string()),
                    })?))?,
            )
            .await?;
        let alpha_payload: StarterDemandPollResponse = response_json(alpha_poll).await?;
        let first_request_id = alpha_payload.offers[0].request_id.clone();

        let ack_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!(
                        "/api/starter-demand/offers/{}/ack",
                        first_request_id
                    ))
                    .header(
                        "authorization",
                        format!("Bearer {}", session_alpha.access_token),
                    )
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&StarterDemandAckRequest {
                        provider_nostr_pubkey: Some("npub1alpha".to_string()),
                    })?))?,
            )
            .await?;
        assert_eq!(ack_response.status(), StatusCode::OK);

        tokio::time::sleep(std::time::Duration::from_millis(1_100)).await;

        let beta_poll = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/starter-demand/poll")
                    .header(
                        "authorization",
                        format!("Bearer {}", session_beta.access_token),
                    )
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&StarterDemandPollRequest {
                        provider_nostr_pubkey: Some("npub1beta".to_string()),
                        primary_relay_url: Some("wss://relay.openagents.dev".to_string()),
                    })?))?,
            )
            .await?;
        assert_eq!(beta_poll.status(), StatusCode::OK);
        let beta_payload: StarterDemandPollResponse = response_json(beta_poll).await?;
        assert_eq!(beta_payload.offers.len(), 1);
        assert_ne!(beta_payload.offers[0].request_id, first_request_id);

        Ok(())
    }

    #[tokio::test]
    async fn hosted_starter_demand_requires_hosted_relay_path() -> Result<()> {
        let app = build_router(test_config()?);
        let session = create_session_token(&app).await?;
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/starter-demand/poll")
                    .header("authorization", format!("Bearer {}", session.access_token))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&StarterDemandPollRequest {
                        provider_nostr_pubkey: Some("npub1alpha".to_string()),
                        primary_relay_url: Some("wss://relay.example.com".to_string()),
                    })?))?,
            )
            .await?;
        assert_eq!(response.status(), StatusCode::OK);
        let payload: StarterDemandPollResponse = response_json(response).await?;
        assert!(!payload.eligible);
        assert_eq!(
            payload.reason.as_deref(),
            Some("starter_demand_requires_openagents_hosted_nexus")
        );
        assert!(payload.offers.is_empty());
        Ok(())
    }

    #[tokio::test]
    async fn hosted_starter_demand_requires_autopilot_desktop_session() -> Result<()> {
        let app = build_router(test_config()?);
        let session = create_session_token_for(&app, "desktop-alpha", Some("npub1alpha")).await?;
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/starter-demand/poll")
                    .header("authorization", format!("Bearer {}", session.access_token))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&StarterDemandPollRequest {
                        provider_nostr_pubkey: Some("npub1alpha".to_string()),
                        primary_relay_url: Some("wss://relay.openagents.dev".to_string()),
                    })?))?,
            )
            .await?;
        assert_eq!(response.status(), StatusCode::OK);
        let payload: StarterDemandPollResponse = response_json(response).await?;
        assert!(!payload.eligible);
        assert_eq!(
            payload.reason.as_deref(),
            Some("starter_demand_requires_autopilot_desktop_session")
        );
        Ok(())
    }

    #[tokio::test]
    async fn hosted_starter_demand_requires_bound_nostr_identity() -> Result<()> {
        let app = build_router(test_config()?);
        let session = create_session_token_for(&app, "autopilot-desktop-alpha", None).await?;
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/starter-demand/poll")
                    .header("authorization", format!("Bearer {}", session.access_token))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&StarterDemandPollRequest {
                        provider_nostr_pubkey: Some("npub1alpha".to_string()),
                        primary_relay_url: Some("wss://relay.openagents.dev".to_string()),
                    })?))?,
            )
            .await?;
        assert_eq!(response.status(), StatusCode::OK);
        let payload: StarterDemandPollResponse = response_json(response).await?;
        assert!(!payload.eligible);
        assert_eq!(
            payload.reason.as_deref(),
            Some("starter_demand_requires_bound_nostr_identity")
        );
        Ok(())
    }

    #[tokio::test]
    async fn hosted_starter_demand_requires_matching_provider_nostr_pubkey() -> Result<()> {
        let app = build_router(test_config()?);
        let session = create_session_token(&app).await?;

        let poll_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/starter-demand/poll")
                    .header("authorization", format!("Bearer {}", session.access_token))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&StarterDemandPollRequest {
                        provider_nostr_pubkey: Some("npub1alpha".to_string()),
                        primary_relay_url: Some("wss://relay.openagents.dev".to_string()),
                    })?))?,
            )
            .await?;
        assert_eq!(poll_response.status(), StatusCode::OK);
        let payload: StarterDemandPollResponse = response_json(poll_response).await?;
        let request_id = payload.offers[0].request_id.clone();

        let ack_response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/starter-demand/offers/{request_id}/ack"))
                    .header("authorization", format!("Bearer {}", session.access_token))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&StarterDemandAckRequest {
                        provider_nostr_pubkey: Some("npub1mismatch".to_string()),
                    })?))?,
            )
            .await?;
        assert_eq!(ack_response.status(), StatusCode::FORBIDDEN);
        let body: serde_json::Value = response_json(ack_response).await?;
        assert_eq!(
            body["reason"].as_str(),
            Some("starter_demand_provider_nostr_pubkey_mismatch")
        );
        Ok(())
    }

    #[tokio::test]
    async fn receipt_log_reloads_backend_receipts_when_configured() -> Result<()> {
        let receipt_log_path = std::env::temp_dir().join(format!(
            "nexus-control-receipts-{}.jsonl",
            super::random_token()
        ));
        let _ = std::fs::remove_file(receipt_log_path.as_path());

        let mut config = test_config()?;
        config.receipt_log_path = Some(receipt_log_path.clone());

        let app = build_router(config.clone());
        let _session = create_session_token(&app).await?;

        let log_contents = std::fs::read_to_string(receipt_log_path.as_path())?;
        assert!(log_contents.contains("\"receipt_type\":\"desktop_session.created\""));

        let reloaded_app = build_router(config);
        let stats_response = reloaded_app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/stats")
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(stats_response.status(), StatusCode::OK);
        let stats: PublicStatsSnapshot = response_json(stats_response).await?;
        assert!(stats.receipt_persistence_enabled);
        assert_eq!(stats.sessions_issued_24h, 1);
        assert_eq!(stats.receipt_count, 1);

        let _ = std::fs::remove_file(receipt_log_path.as_path());
        Ok(())
    }

    #[tokio::test]
    async fn kernel_authority_flow_persists_receipts_and_snapshot() -> Result<()> {
        let app = build_router(test_config()?);
        let session = create_session_token(&app).await?;
        let created_at_ms = 1_762_000_101_234i64;
        let minute_start_ms = floor_to_minute_utc(created_at_ms);

        let work_unit = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/kernel/work_units")
                    .header("authorization", authorization(&session))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&work_unit_request(
                        "work_unit.alpha",
                        "idemp.work.alpha",
                        created_at_ms,
                    ))?))?,
            )
            .await?;
        assert_eq!(work_unit.status(), StatusCode::OK);
        let work_unit_payload: CreateWorkUnitResponse = response_json(work_unit).await?;
        assert_eq!(work_unit_payload.receipt.receipt_type, "kernel.work_unit.create.v1");
        assert_eq!(work_unit_payload.work_unit.work_unit_id, "work_unit.alpha");

        let contract = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/kernel/contracts")
                    .header("authorization", authorization(&session))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&contract_request(
                        "contract.alpha",
                        "work_unit.alpha",
                        "idemp.contract.alpha",
                        created_at_ms + 1_000,
                    ))?))?,
            )
            .await?;
        assert_eq!(contract.status(), StatusCode::OK);
        let contract_payload: CreateContractResponse = response_json(contract).await?;
        assert_eq!(contract_payload.receipt.receipt_type, "kernel.contract.create.v1");
        assert_eq!(contract_payload.contract.contract_id, "contract.alpha");

        let submission = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/kernel/contracts/contract.alpha/submit")
                    .header("authorization", authorization(&session))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&submission_request(
                        "contract.alpha",
                        "work_unit.alpha",
                        "idemp.submit.alpha",
                        created_at_ms + 2_000,
                    ))?))?,
            )
            .await?;
        assert_eq!(submission.status(), StatusCode::OK);
        let submission_payload: SubmitOutputResponse = response_json(submission).await?;
        assert_eq!(submission_payload.receipt.receipt_type, "kernel.output.submit.v1");
        assert_eq!(
            submission_payload.submission.submission_id,
            "submission.contract.alpha"
        );

        let verdict = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/kernel/contracts/contract.alpha/verdict/finalize")
                    .header("authorization", authorization(&session))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&verdict_request(
                        "contract.alpha",
                        "work_unit.alpha",
                        "idemp.verdict.alpha",
                        created_at_ms + 3_000,
                    ))?))?,
            )
            .await?;
        assert_eq!(verdict.status(), StatusCode::OK);
        let verdict_payload: FinalizeVerdictResponse = response_json(verdict).await?;
        assert_eq!(
            verdict_payload.receipt.receipt_type,
            "kernel.verdict.finalize.v1"
        );
        assert_eq!(verdict_payload.verdict.verdict_id, "verdict.contract.alpha");
        assert_eq!(
            verdict_payload
                .settlement_link
                .as_ref()
                .map(|settlement| settlement.settlement_id.as_str()),
            Some("settlement.contract.alpha")
        );

        let receipt_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!(
                        "/v1/kernel/receipts/{}",
                        verdict_payload.receipt.receipt_id
                    ))
                    .header("authorization", authorization(&session))
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(receipt_response.status(), StatusCode::OK);
        let canonical_receipt: Receipt = response_json(receipt_response).await?;
        assert_eq!(
            canonical_receipt.canonical_hash,
            verdict_payload.receipt.canonical_hash
        );

        let snapshot_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!("/v1/kernel/snapshots/{minute_start_ms}"))
                    .header("authorization", authorization(&session))
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(snapshot_response.status(), StatusCode::OK);
        let snapshot: super::EconomySnapshot = response_json(snapshot_response).await?;
        assert_eq!(snapshot.as_of_ms, minute_start_ms);
        assert_eq!(snapshot.n, 1);
        assert_eq!(snapshot.nv, 1.0);
        assert_eq!(snapshot.sv, 1.0);
        assert_eq!(snapshot.sv_effective, 1.0);
        assert_eq!(snapshot.inputs.len(), 4);

        Ok(())
    }

    #[tokio::test]
    async fn kernel_authority_enforces_idempotency_per_caller() -> Result<()> {
        let app = build_router(test_config()?);
        let session = create_session_token(&app).await?;
        let request = work_unit_request("work_unit.idempotent", "idemp.shared", 1_762_000_200_123);

        let first_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/kernel/work_units")
                    .header("authorization", authorization(&session))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&request)?))?,
            )
            .await?;
        assert_eq!(first_response.status(), StatusCode::OK);
        let first: CreateWorkUnitResponse = response_json(first_response).await?;

        let replay_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/kernel/work_units")
                    .header("authorization", authorization(&session))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&request)?))?,
            )
            .await?;
        assert_eq!(replay_response.status(), StatusCode::OK);
        let replay: CreateWorkUnitResponse = response_json(replay_response).await?;
        assert_eq!(replay.receipt.receipt_id, first.receipt.receipt_id);
        assert_eq!(replay.receipt.canonical_hash, first.receipt.canonical_hash);

        let mut conflicting = request.clone();
        conflicting.work_unit.metadata = json!({
            "summary": "Changed payload with same idempotency key."
        });
        let conflict_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/kernel/work_units")
                    .header("authorization", authorization(&session))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&conflicting)?))?,
            )
            .await?;
        assert_eq!(conflict_response.status(), StatusCode::CONFLICT);
        let conflict: serde_json::Value = response_json(conflict_response).await?;
        assert_eq!(conflict["reason"], "kernel_idempotency_conflict");

        Ok(())
    }

    #[tokio::test]
    async fn kernel_stream_routes_require_auth_and_return_sse() -> Result<()> {
        let app = build_router(test_config()?);
        let session = create_session_token(&app).await?;

        let unauthorized_receipts = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/v1/kernel/stream/receipts")
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(unauthorized_receipts.status(), StatusCode::UNAUTHORIZED);

        let receipts_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/v1/kernel/stream/receipts")
                    .header("authorization", authorization(&session))
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(receipts_response.status(), StatusCode::OK);
        assert!(
            receipts_response
                .headers()
                .get("content-type")
                .and_then(|value| value.to_str().ok())
                .is_some_and(|value| value.starts_with("text/event-stream"))
        );

        let snapshots_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/v1/kernel/stream/snapshots")
                    .header("authorization", authorization(&session))
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(snapshots_response.status(), StatusCode::OK);
        assert!(
            snapshots_response
                .headers()
                .get("content-type")
                .and_then(|value| value.to_str().ok())
                .is_some_and(|value| value.starts_with("text/event-stream"))
        );

        Ok(())
    }
}
