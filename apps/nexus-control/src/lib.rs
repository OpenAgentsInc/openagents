use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, RwLock};

use axum::extract::State;
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
const DEFAULT_LISTEN_ADDR: &str = "127.0.0.1:42020";
const DEFAULT_SESSION_TTL_SECONDS: u64 = 86_400;
const DEFAULT_SYNC_TOKEN_TTL_SECONDS: u64 = 900;
const DEFAULT_SYNC_TOKEN_REFRESH_AFTER_SECONDS: u64 = 300;
const DEFAULT_SYNC_STREAM_GRANTS: [&str; 2] = [
    "stream.activity_projection.v1",
    "stream.earn_job_lifecycle_projection.v1",
];
const DEFAULT_SYNC_SCOPES: [&str; 3] = ["sync.subscribe", "sync.checkpoint.write", "sync.append"];

#[derive(Debug, Clone)]
pub struct ServiceConfig {
    pub listen_addr: SocketAddr,
    pub session_ttl_seconds: u64,
    pub sync_token_ttl_seconds: u64,
    pub sync_token_refresh_after_seconds: u64,
    pub sync_stream_grants: Vec<String>,
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

        Ok(Self {
            listen_addr,
            session_ttl_seconds,
            sync_token_ttl_seconds,
            sync_token_refresh_after_seconds,
            sync_stream_grants,
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
        DesktopSessionCreateRequest, DesktopSessionResponse, ServiceConfig, SessionMeResponse,
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
        })
    }

    async fn response_json<T: serde::de::DeserializeOwned>(response: Response) -> Result<T> {
        let bytes = to_bytes(response.into_body(), usize::MAX).await?;
        Ok(serde_json::from_slice(bytes.as_ref())?)
    }

    #[tokio::test]
    async fn desktop_session_flow_mints_bearer_and_sync_token() -> Result<()> {
        let app = build_router(test_config()?);
        let create_request = DesktopSessionCreateRequest {
            desktop_client_id: "desktop-alpha".to_string(),
            device_name: Some("Chris MacBook".to_string()),
            bound_nostr_pubkey: Some("npub1alpha".to_string()),
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
        let session: DesktopSessionResponse = response_json(response).await?;
        assert_eq!(session.token_type, "Bearer");
        assert_eq!(session.bound_nostr_pubkey.as_deref(), Some("npub1alpha"));

        let me_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/api/session/me")
                    .header("authorization", format!("Bearer {}", session.access_token))
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(me_response.status(), StatusCode::OK);
        let me: SessionMeResponse = response_json(me_response).await?;
        assert_eq!(me.session_id, session.session_id);

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
}
