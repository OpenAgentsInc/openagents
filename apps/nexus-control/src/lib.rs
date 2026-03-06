use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, RwLock};

use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

const ENV_LISTEN_ADDR: &str = "NEXUS_CONTROL_LISTEN_ADDR";
const ENV_SESSION_TTL_SECONDS: &str = "NEXUS_CONTROL_SESSION_TTL_SECONDS";
const ENV_SYNC_TOKEN_TTL_SECONDS: &str = "NEXUS_CONTROL_SYNC_TOKEN_TTL_SECONDS";
const ENV_SYNC_TOKEN_REFRESH_AFTER_SECONDS: &str = "NEXUS_CONTROL_SYNC_TOKEN_REFRESH_AFTER_SECONDS";
const ENV_SYNC_STREAM_GRANTS: &str = "NEXUS_CONTROL_SYNC_STREAM_GRANTS";
const ENV_HOSTED_NEXUS_RELAY_URL: &str = "NEXUS_CONTROL_HOSTED_NEXUS_RELAY_URL";
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
}

#[derive(Debug, Default)]
struct ControlStore {
    sessions_by_access_token: HashMap<String, DesktopSessionRecord>,
    sync_tokens: HashMap<String, SyncTokenRecord>,
    starter_demand: StarterDemandState,
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
    let state = AppState {
        config,
        store: Arc::new(RwLock::new(ControlStore::default())),
    };
    Router::new()
        .route("/healthz", get(healthz))
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
    let record = SyncTokenRecord {
        token: token.clone(),
        rotation_id: rotation_id.clone(),
        session_id: session.session_id,
        account_id: session.account_id,
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
        let store = state.store.read().map_err(|_| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            error: "internal_error",
            reason: "session_store_poisoned".to_string(),
        })?;
        response.budget_allocated_sats = store.starter_demand.budget_allocated_sats;
        response.reason = Some("starter_demand_requires_openagents_hosted_nexus".to_string());
        return Ok(Json(response));
    }
    if let Some(reason) =
        starter_demand_provider_proof_reason(&session, provider_nostr_pubkey.as_deref())
    {
        let store = state.store.read().map_err(|_| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            error: "internal_error",
            reason: "session_store_poisoned".to_string(),
        })?;
        response.budget_allocated_sats = store.starter_demand.budget_allocated_sats;
        response.reason = Some(reason);
        return Ok(Json(response));
    }

    let now = now_unix_ms();
    let mut store = state.store.write().map_err(|_| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        error: "internal_error",
        reason: "session_store_poisoned".to_string(),
    })?;
    prune_expired_starter_offers(&mut store.starter_demand, &state.config, now);
    maybe_dispatch_starter_offer(&mut store.starter_demand, &state.config, &session, now);
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
    prune_expired_starter_offers(&mut store.starter_demand, &state.config, now);
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
            let released_budget_sats =
                expire_offer(offer, now, "starter_offer_start_confirm_missed");
            store.starter_demand.budget_allocated_sats = store
                .starter_demand
                .budget_allocated_sats
                .saturating_sub(released_budget_sats);
            return Err(ApiError {
                status: StatusCode::CONFLICT,
                error: "conflict",
                reason: "starter_offer_start_confirm_missed".to_string(),
            });
        }

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
        offer.next_heartbeat_due_at_unix_ms = Some(now.saturating_add(heartbeat_timeout_ms));
        offer.expires_at_unix_ms = execution_expires_at_unix_ms;
    }

    if offer.status != StarterOfferStatus::Running {
        return Err(ApiError {
            status: StatusCode::CONFLICT,
            error: "conflict",
            reason: format!("starter_offer_not_ackable status={}", offer.status.label()),
        });
    }

    Ok(Json(StarterDemandAckResponse {
        request_id: offer.request_id.clone(),
        status: offer.status.label().to_string(),
        started_at_unix_ms: offer.execution_started_at_unix_ms.unwrap_or(now),
        execution_expires_at_unix_ms: offer.execution_expires_at_unix_ms.unwrap_or(now),
        last_heartbeat_at_unix_ms: offer.last_heartbeat_at_unix_ms.unwrap_or(now),
        next_heartbeat_due_at_unix_ms: offer.next_heartbeat_due_at_unix_ms.unwrap_or(now),
        heartbeat_timeout_seconds: state.config.starter_demand_heartbeat_timeout_seconds,
        heartbeat_interval_seconds: starter_demand_heartbeat_interval_seconds(&state.config),
    }))
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
    prune_expired_starter_offers(&mut store.starter_demand, &state.config, now);
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
        let released_budget_sats = expire_offer(offer, now, "starter_offer_execution_expired");
        store.starter_demand.budget_allocated_sats = store
            .starter_demand
            .budget_allocated_sats
            .saturating_sub(released_budget_sats);
        return Err(ApiError {
            status: StatusCode::CONFLICT,
            error: "conflict",
            reason: "starter_offer_execution_expired".to_string(),
        });
    }
    if now > next_heartbeat_due_at_unix_ms {
        let released_budget_sats = expire_offer(offer, now, "starter_offer_heartbeat_missed");
        store.starter_demand.budget_allocated_sats = store
            .starter_demand
            .budget_allocated_sats
            .saturating_sub(released_budget_sats);
        return Err(ApiError {
            status: StatusCode::CONFLICT,
            error: "conflict",
            reason: "starter_offer_heartbeat_missed".to_string(),
        });
    }

    let heartbeat_timeout_ms = state
        .config
        .starter_demand_heartbeat_timeout_seconds
        .saturating_mul(1_000);
    offer.last_heartbeat_at_unix_ms = Some(now);
    offer.next_heartbeat_due_at_unix_ms = Some(now.saturating_add(heartbeat_timeout_ms));
    Ok(Json(StarterDemandHeartbeatResponse {
        request_id: offer.request_id.clone(),
        status: offer.status.label().to_string(),
        last_heartbeat_at_unix_ms: offer.last_heartbeat_at_unix_ms.unwrap_or(now),
        next_heartbeat_due_at_unix_ms: offer.next_heartbeat_due_at_unix_ms.unwrap_or(now),
        execution_expires_at_unix_ms,
        heartbeat_timeout_seconds: state.config.starter_demand_heartbeat_timeout_seconds,
        heartbeat_interval_seconds: starter_demand_heartbeat_interval_seconds(&state.config),
    }))
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
    prune_expired_starter_offers(&mut store.starter_demand, &state.config, now);
    let (response_request_id, response_status, response_released_at_unix_ms, released_budget_sats) = {
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

        let released_budget_sats = match offer.status {
            StarterOfferStatus::Offered | StarterOfferStatus::Running => {
                release_offer(offer, now, failure_reason.as_str())
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
        )
    };
    store.starter_demand.budget_allocated_sats = store
        .starter_demand
        .budget_allocated_sats
        .saturating_sub(released_budget_sats);

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
    prune_expired_starter_offers(&mut store.starter_demand, &state.config, now);
    let (response_request_id, response_status, response_completed_at_unix_ms, released_budget_sats) = {
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

        let released_budget_sats = match offer.status {
            StarterOfferStatus::Running => {
                offer.status = StarterOfferStatus::Completed;
                offer.payment_pointer = Some(payment_pointer.clone());
                offer.completed_at_unix_ms = Some(now);
                offer.last_heartbeat_at_unix_ms = Some(now);
                offer.next_heartbeat_due_at_unix_ms = None;
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
        )
    };
    store.starter_demand.budget_allocated_sats = store
        .starter_demand
        .budget_allocated_sats
        .saturating_sub(released_budget_sats);

    Ok(Json(StarterDemandCompleteResponse {
        request_id: response_request_id,
        status: response_status,
        payment_pointer,
        completed_at_unix_ms: response_completed_at_unix_ms,
        budget_cap_sats: state.config.starter_demand_budget_cap_sats,
        budget_allocated_sats: store.starter_demand.budget_allocated_sats,
    }))
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
) {
    let mut released_budget_sats = 0u64;
    for offers in state.offers_by_session.values_mut() {
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
}

fn maybe_dispatch_starter_offer(
    state: &mut StarterDemandState,
    config: &ServiceConfig,
    session: &DesktopSessionRecord,
    now_unix_ms: u64,
) {
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
        return;
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
        return;
    }

    let remaining_budget_sats = config
        .starter_demand_budget_cap_sats
        .saturating_sub(state.budget_allocated_sats);
    let Some(template) = next_template_for_remaining_budget(state, remaining_budget_sats) else {
        return;
    };

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
        .push(offer);
    state.budget_allocated_sats = state
        .budget_allocated_sats
        .saturating_add(template.payout_sats);
    state
        .last_dispatch_by_session
        .insert(session.session_id.clone(), now_unix_ms);
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
    use tower::ServiceExt;

    use super::{
        DesktopSessionCreateRequest, DesktopSessionResponse, ServiceConfig,
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
}
