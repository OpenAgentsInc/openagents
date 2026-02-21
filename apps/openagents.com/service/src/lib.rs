use std::collections::HashMap;
use std::path::Path as FsPath;
use std::sync::Arc;
use std::time::SystemTime;

use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::header::{AUTHORIZATION, CACHE_CONTROL, CONTENT_TYPE, COOKIE, SET_COOKIE};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Redirect};
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::{SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use tower::ServiceBuilder;
use tower_http::request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer};
use tower_http::trace::TraceLayer;

pub mod auth;
pub mod config;
pub mod observability;
pub mod route_split;
pub mod sync_token;

use crate::auth::{AuthError, AuthService, PolicyCheckRequest, SessionBundle};
use crate::config::Config;
use crate::observability::{AuditEvent, Observability};
use crate::route_split::{RouteSplitDecision, RouteSplitService, RouteTarget};
use crate::sync_token::{SyncTokenError, SyncTokenIssueRequest, SyncTokenIssuer};

const SERVICE_NAME: &str = "openagents-control-service";
const CHALLENGE_COOKIE_NAME: &str = "oa_magic_challenge";
const CACHE_IMMUTABLE_ONE_YEAR: &str = "public, max-age=31536000, immutable";
const CACHE_SHORT_LIVED: &str = "public, max-age=60";
const CACHE_MANIFEST: &str = "no-cache, no-store, must-revalidate";

#[derive(Clone)]
struct AppState {
    config: Arc<Config>,
    auth: AuthService,
    observability: Observability,
    route_split: RouteSplitService,
    sync_token_issuer: SyncTokenIssuer,
    started_at: SystemTime,
}

#[derive(Debug, Serialize)]
struct RootResponse {
    service: &'static str,
    version: &'static str,
    docs: &'static str,
}

#[derive(Debug, Serialize)]
struct HealthResponse {
    status: &'static str,
    service: &'static str,
    version: &'static str,
    uptime_seconds: u64,
    auth_provider: &'static str,
}

#[derive(Debug, Serialize)]
struct ReadinessResponse {
    status: &'static str,
    static_dir: String,
}

#[derive(Debug, Serialize)]
struct ApiErrorDetail {
    code: &'static str,
    message: String,
}

#[derive(Debug, Serialize)]
struct ApiErrorResponse {
    message: String,
    error: ApiErrorDetail,
    #[serde(skip_serializing_if = "Option::is_none")]
    errors: Option<HashMap<String, Vec<String>>>,
}

#[derive(Debug, Deserialize)]
struct SendEmailCodeRequest {
    email: String,
}

#[derive(Debug, Deserialize)]
struct VerifyEmailCodeRequest {
    code: String,
    #[serde(default)]
    challenge_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RefreshSessionRequest {
    #[serde(default)]
    refresh_token: Option<String>,
    #[serde(default)]
    rotate_refresh_token: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct SetActiveOrgRequest {
    org_id: String,
}

#[derive(Debug, Deserialize)]
struct PolicyAuthorizeRequest {
    #[serde(default)]
    org_id: Option<String>,
    #[serde(default)]
    required_scopes: Vec<String>,
    #[serde(default)]
    requested_topics: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct SyncTokenRequestPayload {
    #[serde(default)]
    scopes: Vec<String>,
    #[serde(default)]
    topics: Vec<String>,
    #[serde(default)]
    ttl_seconds: Option<u32>,
    #[serde(default)]
    device_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RouteSplitOverrideRequest {
    target: String,
}

#[derive(Debug, Deserialize)]
struct RouteSplitEvaluateRequest {
    path: String,
    #[serde(default)]
    cohort_key: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SendThreadMessageRequest {
    text: String,
}

pub fn build_router(config: Config) -> Router {
    build_router_with_observability(config, Observability::default())
}

pub fn build_router_with_observability(config: Config, observability: Observability) -> Router {
    let auth = AuthService::from_config(&config);
    let route_split = RouteSplitService::from_config(&config);
    let sync_token_issuer = SyncTokenIssuer::from_config(&config);
    let state = AppState {
        config: Arc::new(config),
        auth,
        observability,
        route_split,
        sync_token_issuer,
        started_at: SystemTime::now(),
    };

    Router::new()
        .route("/", get(root))
        .route("/healthz", get(health))
        .route("/readyz", get(readiness))
        .route("/api/auth/email", post(send_email_code))
        .route("/api/auth/verify", post(verify_email_code))
        .route("/api/auth/session", get(current_session))
        .route("/api/auth/refresh", post(refresh_session))
        .route("/api/auth/logout", post(logout_session))
        .route("/api/me", get(me))
        .route("/api/orgs/memberships", get(org_memberships))
        .route("/api/orgs/active", post(set_active_org))
        .route("/api/policy/authorize", post(policy_authorize))
        .route("/api/sync/token", post(sync_token))
        .route(
            "/api/runtime/threads/:thread_id/messages",
            post(send_thread_message),
        )
        .route("/api/v1/auth/session", get(current_session))
        .route("/api/v1/control/status", get(control_status))
        .route(
            "/api/v1/control/route-split/status",
            get(route_split_status),
        )
        .route(
            "/api/v1/control/route-split/override",
            post(route_split_override),
        )
        .route(
            "/api/v1/control/route-split/evaluate",
            post(route_split_evaluate),
        )
        .route("/api/v1/sync/token", post(sync_token))
        .route("/manifest.json", get(static_manifest))
        .route("/assets/*path", get(static_asset))
        .route("/*path", get(web_shell_entry))
        .with_state(state)
        .layer(
            ServiceBuilder::new()
                .layer(SetRequestIdLayer::x_request_id(MakeRequestUuid))
                .layer(PropagateRequestIdLayer::x_request_id())
                .layer(TraceLayer::new_for_http()),
        )
}

async fn root() -> Json<RootResponse> {
    Json(RootResponse {
        service: SERVICE_NAME,
        version: env!("CARGO_PKG_VERSION"),
        docs: "apps/openagents.com/service/README.md",
    })
}

async fn health(State(state): State<AppState>) -> Json<HealthResponse> {
    let uptime_seconds = match state.started_at.elapsed() {
        Ok(duration) => duration.as_secs(),
        Err(_) => 0,
    };

    Json(HealthResponse {
        status: "ok",
        service: SERVICE_NAME,
        version: env!("CARGO_PKG_VERSION"),
        uptime_seconds,
        auth_provider: state.auth.provider_name(),
    })
}

async fn readiness(State(state): State<AppState>) -> impl IntoResponse {
    let static_dir = state.config.static_dir.to_string_lossy().to_string();

    if state.config.static_dir.is_dir() {
        return (
            StatusCode::OK,
            Json(ReadinessResponse {
                status: "ready",
                static_dir,
            }),
        );
    }

    (
        StatusCode::SERVICE_UNAVAILABLE,
        Json(ReadinessResponse {
            status: "not_ready",
            static_dir,
        }),
    )
}

async fn static_manifest(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiErrorResponse>)> {
    let manifest_path = state.config.static_dir.join("manifest.json");
    let response = build_static_response(&manifest_path, CACHE_MANIFEST)
        .await
        .map_err(map_static_error)?;
    Ok(response)
}

async fn static_asset(
    State(state): State<AppState>,
    Path(path): Path<String>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiErrorResponse>)> {
    let relative_path = normalize_static_path(&path)
        .ok_or_else(|| static_not_found(format!("Asset '{}' was not found.", path)))?;

    let static_root = state.config.static_dir.as_path();
    let preferred = static_root.join("assets").join(&relative_path);
    let fallback = static_root.join(&relative_path);

    let asset_path = if preferred.is_file() {
        preferred
    } else if fallback.is_file() {
        fallback
    } else {
        return Err(static_not_found(format!(
            "Asset '{}' was not found.",
            relative_path
        )));
    };

    let cache_control = if is_hashed_asset_path(&relative_path) {
        CACHE_IMMUTABLE_ONE_YEAR
    } else {
        CACHE_SHORT_LIVED
    };

    let response = build_static_response(&asset_path, cache_control)
        .await
        .map_err(map_static_error)?;
    Ok(response)
}

async fn web_shell_entry(
    State(state): State<AppState>,
    headers: HeaderMap,
    uri: axum::http::Uri,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiErrorResponse>)> {
    let path = uri.path();
    if path.starts_with("/api/") {
        return Err(static_not_found(format!("Route '{}' was not found.", path)));
    }

    let request_id = request_id(&headers);
    let cohort_key = resolve_route_cohort_key(&headers);
    let mut decision = state.route_split.evaluate(path, &cohort_key).await;
    if is_pilot_chat_route(path) {
        decision = RouteSplitDecision {
            path: path.to_string(),
            target: RouteTarget::RustShell,
            reason: "pilot_route_rust_only".to_string(),
            cohort_bucket: decision.cohort_bucket,
            cohort_key: decision.cohort_key.clone(),
        };
    }

    emit_route_split_decision_audit(
        &state,
        &request_id,
        &decision,
        headers
            .get("user-agent")
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default(),
    );

    match decision.target {
        RouteTarget::RustShell => {
            let entry_path = state.config.static_dir.join("index.html");
            let response = build_static_response(&entry_path, CACHE_MANIFEST)
                .await
                .map_err(map_static_error)?;
            Ok(response)
        }
        RouteTarget::Legacy => {
            let redirect = state
                .route_split
                .legacy_redirect_url(path, uri.query())
                .ok_or_else(|| {
                    (
                        StatusCode::SERVICE_UNAVAILABLE,
                        Json(ApiErrorResponse {
                            message: "Legacy route target is not configured.".to_string(),
                            error: ApiErrorDetail {
                                code: "legacy_route_unavailable",
                                message: "Legacy route target is not configured.".to_string(),
                            },
                            errors: None,
                        }),
                    )
                })?;
            Ok(Redirect::temporary(&redirect).into_response())
        }
    }
}

async fn route_split_status(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiErrorResponse>)> {
    let access_token =
        bearer_token(&headers).ok_or_else(|| unauthorized_error("Unauthenticated."))?;
    state
        .auth
        .session_from_access_token(&access_token)
        .await
        .map_err(map_auth_error)?;

    let status = state.route_split.status().await;
    Ok((StatusCode::OK, Json(serde_json::json!({ "data": status }))))
}

async fn route_split_override(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<RouteSplitOverrideRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiErrorResponse>)> {
    let request_id = request_id(&headers);
    let access_token =
        bearer_token(&headers).ok_or_else(|| unauthorized_error("Unauthenticated."))?;
    let session = state
        .auth
        .session_from_access_token(&access_token)
        .await
        .map_err(map_auth_error)?;

    let normalized_target = payload.target.trim().to_lowercase();
    let override_target = match normalized_target.as_str() {
        "legacy" => Some(RouteTarget::Legacy),
        "rust" | "rust_shell" => Some(RouteTarget::RustShell),
        "clear" | "default" => None,
        _ => {
            return Err(validation_error(
                "target",
                "Target must be one of: legacy, rust, clear.",
            ));
        }
    };

    state.route_split.set_override_target(override_target).await;
    let status = state.route_split.status().await;

    state.observability.audit(
        AuditEvent::new("route.split.override.updated", request_id.clone())
            .with_user_id(session.user.id)
            .with_session_id(session.session.session_id)
            .with_org_id(session.session.active_org_id)
            .with_device_id(session.session.device_id)
            .with_attribute("target", normalized_target),
    );
    state
        .observability
        .increment_counter("route.split.override.updated", &request_id);

    Ok((StatusCode::OK, Json(serde_json::json!({ "data": status }))))
}

async fn route_split_evaluate(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<RouteSplitEvaluateRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiErrorResponse>)> {
    let access_token =
        bearer_token(&headers).ok_or_else(|| unauthorized_error("Unauthenticated."))?;
    state
        .auth
        .session_from_access_token(&access_token)
        .await
        .map_err(map_auth_error)?;

    if payload.path.trim().is_empty() {
        return Err(validation_error("path", "Path is required."));
    }

    let cohort_key = payload
        .cohort_key
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| resolve_route_cohort_key(&headers));

    let decision = state.route_split.evaluate(&payload.path, &cohort_key).await;
    Ok((
        StatusCode::OK,
        Json(serde_json::json!({ "data": decision })),
    ))
}

async fn build_static_response(
    file_path: &FsPath,
    cache_control: &'static str,
) -> Result<axum::response::Response, StaticResponseError> {
    let bytes = tokio::fs::read(file_path).await.map_err(|source| {
        if source.kind() == std::io::ErrorKind::NotFound {
            StaticResponseError::NotFound(format!(
                "Static file '{}' was not found.",
                file_path.display()
            ))
        } else {
            StaticResponseError::Io(source)
        }
    })?;

    let content_type = mime_guess::from_path(file_path).first_or_octet_stream();
    let mut response = axum::response::Response::new(Body::from(bytes));
    *response.status_mut() = StatusCode::OK;
    response.headers_mut().insert(
        CONTENT_TYPE,
        HeaderValue::from_str(content_type.as_ref())
            .map_err(|_| StaticResponseError::InvalidHeader(content_type.to_string()))?,
    );
    response
        .headers_mut()
        .insert(CACHE_CONTROL, HeaderValue::from_static(cache_control));

    Ok(response)
}

#[derive(Debug, thiserror::Error)]
enum StaticResponseError {
    #[error("{0}")]
    NotFound(String),
    #[error("static file read failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid header value '{0}'")]
    InvalidHeader(String),
}

fn map_static_error(error: StaticResponseError) -> (StatusCode, Json<ApiErrorResponse>) {
    match error {
        StaticResponseError::NotFound(message) => static_not_found(message),
        StaticResponseError::Io(_) | StaticResponseError::InvalidHeader(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiErrorResponse {
                message: "Failed to serve static asset.".to_string(),
                error: ApiErrorDetail {
                    code: "static_asset_error",
                    message: "Failed to serve static asset.".to_string(),
                },
                errors: None,
            }),
        ),
    }
}

fn static_not_found(message: String) -> (StatusCode, Json<ApiErrorResponse>) {
    (
        StatusCode::NOT_FOUND,
        Json(ApiErrorResponse {
            message: message.clone(),
            error: ApiErrorDetail {
                code: "not_found",
                message,
            },
            errors: None,
        }),
    )
}

fn normalize_static_path(path: &str) -> Option<String> {
    let trimmed = path.trim().trim_start_matches('/');
    if trimmed.is_empty() {
        return None;
    }

    let mut normalized_parts = Vec::new();
    for part in trimmed.split('/') {
        let segment = part.trim();
        if segment.is_empty() || segment == "." || segment == ".." {
            return None;
        }
        normalized_parts.push(segment);
    }

    Some(normalized_parts.join("/"))
}

fn is_hashed_asset_path(path: &str) -> bool {
    let Some(file_name) = FsPath::new(path)
        .file_name()
        .and_then(|value| value.to_str())
    else {
        return false;
    };

    let Some((stem, _ext)) = file_name.rsplit_once('.') else {
        return false;
    };

    let Some((_, hash_part)) = stem.rsplit_once('-') else {
        return false;
    };

    hash_part.len() >= 8 && hash_part.chars().all(|char| char.is_ascii_alphanumeric())
}

async fn send_email_code(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<SendEmailCodeRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiErrorResponse>)> {
    let request_id = request_id(&headers);
    let challenge = state
        .auth
        .start_challenge(payload.email)
        .await
        .map_err(map_auth_error)?;

    state.observability.audit(
        AuditEvent::new("auth.challenge.requested", request_id.clone())
            .with_attribute("provider", state.auth.provider_name())
            .with_attribute(
                "email_domain",
                email_domain(&challenge.email).unwrap_or_else(|| "unknown".to_string()),
            ),
    );
    state
        .observability
        .increment_counter("auth.challenge.requested", &request_id);

    let cookie = challenge_cookie(
        &challenge.challenge_id,
        state.config.auth_challenge_ttl_seconds,
    );
    let response = serde_json::json!({
        "ok": true,
        "status": "code-sent",
        "email": challenge.email,
        "challengeId": challenge.challenge_id,
    });

    Ok((
        [(SET_COOKIE, header_value(&cookie)?)],
        (StatusCode::OK, Json(response)),
    ))
}

async fn verify_email_code(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<VerifyEmailCodeRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiErrorResponse>)> {
    let request_id = request_id(&headers);
    let challenge_id = payload
        .challenge_id
        .and_then(non_empty)
        .or_else(|| extract_cookie_value(&headers, CHALLENGE_COOKIE_NAME));

    let challenge_id = match challenge_id {
        Some(value) => value,
        None => {
            return Err(validation_error(
                "code",
                "Your sign-in code expired. Request a new code.",
            ));
        }
    };

    let client_name = header_string(&headers, "x-client");
    let ip_address = header_string(&headers, "x-forwarded-for").unwrap_or_default();
    let user_agent = header_string(&headers, "user-agent").unwrap_or_default();

    let verified = state
        .auth
        .verify_challenge(
            &challenge_id,
            payload.code,
            client_name.as_deref(),
            &ip_address,
            &user_agent,
        )
        .await
        .map_err(map_auth_error)?;

    state.observability.audit(
        AuditEvent::new("auth.verify.completed", request_id.clone())
            .with_user_id(verified.user.id.clone())
            .with_session_id(verified.session.session_id.clone())
            .with_org_id(verified.session.active_org_id.clone())
            .with_device_id(verified.session.device_id.clone())
            .with_attribute("provider", state.auth.provider_name())
            .with_attribute("new_user", verified.new_user.to_string()),
    );
    state
        .observability
        .increment_counter("auth.verify.completed", &request_id);

    let clear_cookie = clear_cookie(CHALLENGE_COOKIE_NAME);

    let response = serde_json::json!({
        "ok": true,
        "userId": verified.user.id,
        "status": "authenticated",
        "user": {
            "id": verified.user.id,
            "email": verified.user.email,
            "name": verified.user.name,
            "workosId": verified.user.workos_user_id,
        },
        "redirect": "/",
        "tokenType": verified.token_type,
        "token": verified.access_token,
        "tokenName": verified.token_name,
        "refreshToken": verified.refresh_token,
        "sessionId": verified.session.session_id,
        "newUser": verified.new_user,
    });

    Ok((
        [(SET_COOKIE, header_value(&clear_cookie)?)],
        (StatusCode::OK, Json(response)),
    ))
}

async fn current_session(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiErrorResponse>)> {
    let access_token =
        bearer_token(&headers).ok_or_else(|| unauthorized_error("Unauthenticated."))?;

    let bundle = state
        .auth
        .session_from_access_token(&access_token)
        .await
        .map_err(map_auth_error)?;

    Ok((StatusCode::OK, Json(session_payload(bundle))))
}

async fn me(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiErrorResponse>)> {
    let access_token =
        bearer_token(&headers).ok_or_else(|| unauthorized_error("Unauthenticated."))?;

    let bundle = state
        .auth
        .session_from_access_token(&access_token)
        .await
        .map_err(map_auth_error)?;

    let response = serde_json::json!({
        "data": {
            "user": {
                "id": bundle.user.id,
                "email": bundle.user.email,
                "name": bundle.user.name,
                "workosId": bundle.user.workos_user_id,
            }
        }
    });

    Ok((StatusCode::OK, Json(response)))
}

async fn org_memberships(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiErrorResponse>)> {
    let access_token =
        bearer_token(&headers).ok_or_else(|| unauthorized_error("Unauthenticated."))?;

    let bundle = state
        .auth
        .session_from_access_token(&access_token)
        .await
        .map_err(map_auth_error)?;

    let response = serde_json::json!({
        "data": {
            "activeOrgId": bundle.session.active_org_id,
            "memberships": bundle.memberships,
        }
    });

    Ok((StatusCode::OK, Json(response)))
}

async fn set_active_org(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<SetActiveOrgRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiErrorResponse>)> {
    let request_id = request_id(&headers);
    let access_token =
        bearer_token(&headers).ok_or_else(|| unauthorized_error("Unauthenticated."))?;

    let normalized_org_id = payload.org_id.trim().to_string();
    if normalized_org_id.is_empty() {
        return Err(validation_error("org_id", "Organization id is required."));
    }

    let bundle = state
        .auth
        .set_active_org_by_access_token(&access_token, &normalized_org_id)
        .await
        .map_err(map_auth_error)?;

    state.observability.audit(
        AuditEvent::new("auth.active_org.updated", request_id.clone())
            .with_user_id(bundle.user.id.clone())
            .with_session_id(bundle.session.session_id.clone())
            .with_org_id(bundle.session.active_org_id.clone())
            .with_device_id(bundle.session.device_id.clone()),
    );
    state
        .observability
        .increment_counter("auth.active_org.updated", &request_id);

    let response = serde_json::json!({
        "ok": true,
        "activeOrgId": bundle.session.active_org_id,
        "memberships": bundle.memberships,
    });

    Ok((StatusCode::OK, Json(response)))
}

async fn policy_authorize(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<PolicyAuthorizeRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiErrorResponse>)> {
    let access_token =
        bearer_token(&headers).ok_or_else(|| unauthorized_error("Unauthenticated."))?;

    let decision = state
        .auth
        .evaluate_policy_by_access_token(
            &access_token,
            PolicyCheckRequest {
                org_id: payload.org_id,
                required_scopes: payload.required_scopes,
                requested_topics: payload.requested_topics,
            },
        )
        .await
        .map_err(map_auth_error)?;

    Ok((
        StatusCode::OK,
        Json(serde_json::json!({ "data": decision })),
    ))
}

async fn sync_token(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<SyncTokenRequestPayload>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiErrorResponse>)> {
    let request_id = request_id(&headers);
    let access_token =
        bearer_token(&headers).ok_or_else(|| unauthorized_error("Unauthenticated."))?;

    let session = state
        .auth
        .session_from_access_token(&access_token)
        .await
        .map_err(map_auth_error)?;

    let device_id = payload
        .device_id
        .and_then(non_empty)
        .unwrap_or_else(|| session.session.device_id.clone());

    if device_id != session.session.device_id {
        return Err(forbidden_error(
            "Requested device does not match active authenticated session device.",
        ));
    }

    let issued = state
        .sync_token_issuer
        .issue(SyncTokenIssueRequest {
            user_id: session.user.id.clone(),
            org_id: session.session.active_org_id.clone(),
            session_id: session.session.session_id.clone(),
            device_id,
            requested_scopes: payload.scopes,
            requested_topics: payload.topics,
            requested_ttl_seconds: payload.ttl_seconds,
        })
        .map_err(map_sync_error)?;

    let decision = state
        .auth
        .evaluate_policy_by_access_token(
            &access_token,
            PolicyCheckRequest {
                org_id: Some(issued.org_id.clone()),
                required_scopes: issued.scopes.clone(),
                requested_topics: issued
                    .granted_topics
                    .iter()
                    .map(|grant| grant.topic.clone())
                    .collect(),
            },
        )
        .await
        .map_err(map_auth_error)?;

    if !decision.allowed {
        return Err(forbidden_error(
            "Requested sync scopes/topics are not allowed for current org policy.",
        ));
    }

    state.observability.audit(
        AuditEvent::new("sync.token.issued", request_id.clone())
            .with_user_id(session.user.id.clone())
            .with_session_id(session.session.session_id.clone())
            .with_org_id(session.session.active_org_id.clone())
            .with_device_id(session.session.device_id.clone())
            .with_attribute("scope_count", issued.scopes.len().to_string())
            .with_attribute("topic_count", issued.granted_topics.len().to_string())
            .with_attribute("expires_in", issued.expires_in.to_string()),
    );
    state
        .observability
        .increment_counter("sync.token.issued", &request_id);

    Ok((StatusCode::OK, Json(serde_json::json!({ "data": issued }))))
}

async fn send_thread_message(
    State(state): State<AppState>,
    Path(thread_id): Path<String>,
    headers: HeaderMap,
    Json(payload): Json<SendThreadMessageRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiErrorResponse>)> {
    let request_id = request_id(&headers);
    let access_token =
        bearer_token(&headers).ok_or_else(|| unauthorized_error("Unauthenticated."))?;
    let session = state
        .auth
        .session_from_access_token(&access_token)
        .await
        .map_err(map_auth_error)?;

    let normalized_thread_id = thread_id.trim().to_string();
    if normalized_thread_id.is_empty() {
        return Err(validation_error("thread_id", "Thread id is required."));
    }

    let normalized_text = payload.text.trim().to_string();
    if normalized_text.is_empty() {
        return Err(validation_error("text", "Message text is required."));
    }

    if normalized_text.chars().count() > 20_000 {
        return Err(validation_error(
            "text",
            "Message text exceeds 20000 characters.",
        ));
    }

    let message_id = format!("msg_{}", uuid::Uuid::new_v4().simple());
    let accepted_at = Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);

    state.observability.audit(
        AuditEvent::new("runtime.thread.message.accepted", request_id.clone())
            .with_user_id(session.user.id.clone())
            .with_session_id(session.session.session_id.clone())
            .with_org_id(session.session.active_org_id.clone())
            .with_device_id(session.session.device_id.clone())
            .with_attribute("thread_id", normalized_thread_id.clone())
            .with_attribute("message_id", message_id.clone()),
    );
    state
        .observability
        .increment_counter("runtime.thread.message.accepted", &request_id);

    Ok((
        StatusCode::OK,
        Json(serde_json::json!({
            "data": {
                "accepted": true,
                "message": {
                    "id": message_id,
                    "thread_id": normalized_thread_id,
                    "text": normalized_text,
                    "accepted_at": accepted_at,
                }
            }
        })),
    ))
}

async fn refresh_session(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<RefreshSessionRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiErrorResponse>)> {
    let request_id = request_id(&headers);
    let token = payload
        .refresh_token
        .and_then(non_empty)
        .or_else(|| bearer_token(&headers))
        .ok_or_else(|| unauthorized_error("Invalid refresh token."))?;

    let rotate = payload.rotate_refresh_token.unwrap_or(true);

    let refreshed = state
        .auth
        .refresh_session(&token, rotate)
        .await
        .map_err(map_auth_error)?;

    state.observability.audit(
        AuditEvent::new("auth.refresh.completed", request_id.clone())
            .with_user_id(refreshed.session.user_id.clone())
            .with_session_id(refreshed.session.session_id.clone())
            .with_org_id(refreshed.session.active_org_id.clone())
            .with_device_id(refreshed.session.device_id.clone())
            .with_attribute("rotated_refresh_token", rotate.to_string()),
    );
    state
        .observability
        .increment_counter("auth.refresh.completed", &request_id);

    let response = serde_json::json!({
        "ok": true,
        "status": "active",
        "tokenType": refreshed.token_type,
        "token": refreshed.access_token,
        "refreshToken": refreshed.refresh_token,
        "refreshTokenId": refreshed.refresh_token_id,
        "replacedRefreshTokenId": refreshed.replaced_refresh_token_id,
        "sessionId": refreshed.session.session_id,
        "accessExpiresAt": timestamp(refreshed.session.access_expires_at),
        "refreshExpiresAt": timestamp(refreshed.session.refresh_expires_at),
    });

    Ok((StatusCode::OK, Json(response)))
}

async fn logout_session(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiErrorResponse>)> {
    let request_id = request_id(&headers);
    let access_token =
        bearer_token(&headers).ok_or_else(|| unauthorized_error("Unauthenticated."))?;

    let bundle = state
        .auth
        .session_from_access_token(&access_token)
        .await
        .map_err(map_auth_error)?;

    let revoked = state
        .auth
        .revoke_session_by_access_token(&access_token)
        .await
        .map_err(map_auth_error)?;

    state.observability.audit(
        AuditEvent::new("auth.logout.completed", request_id.clone())
            .with_user_id(bundle.user.id.clone())
            .with_session_id(revoked.session_id.clone())
            .with_org_id(bundle.session.active_org_id.clone())
            .with_device_id(bundle.session.device_id.clone()),
    );
    state
        .observability
        .increment_counter("auth.logout.completed", &request_id);

    let response = serde_json::json!({
        "ok": true,
        "status": "revoked",
        "sessionId": revoked.session_id,
        "revokedAt": timestamp(revoked.revoked_at),
    });

    Ok((StatusCode::OK, Json(response)))
}

async fn control_status(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiErrorResponse>)> {
    let access_token =
        bearer_token(&headers).ok_or_else(|| unauthorized_error("Unauthenticated."))?;

    let bundle = state
        .auth
        .session_from_access_token(&access_token)
        .await
        .map_err(map_auth_error)?;
    let route_split_status = state.route_split.status().await;

    let response = serde_json::json!({
        "data": {
            "service": SERVICE_NAME,
            "authProvider": state.auth.provider_name(),
            "activeOrgId": bundle.session.active_org_id,
            "memberships": bundle.memberships,
            "routeSplit": route_split_status,
        }
    });

    Ok((StatusCode::OK, Json(response)))
}

fn map_auth_error(error: AuthError) -> (StatusCode, Json<ApiErrorResponse>) {
    match error {
        AuthError::Validation { field, message } => validation_error(field, &message),
        AuthError::Unauthorized { message } => unauthorized_error(&message),
        AuthError::Forbidden { message } => forbidden_error(&message),
        AuthError::Provider { message } => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ApiErrorResponse {
                message: message.clone(),
                error: ApiErrorDetail {
                    code: "service_unavailable",
                    message,
                },
                errors: None,
            }),
        ),
    }
}

fn map_sync_error(error: SyncTokenError) -> (StatusCode, Json<ApiErrorResponse>) {
    match error {
        SyncTokenError::InvalidScope { message } => (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(ApiErrorResponse {
                message: message.clone(),
                error: ApiErrorDetail {
                    code: "invalid_scope",
                    message,
                },
                errors: None,
            }),
        ),
        SyncTokenError::InvalidRequest { message } => (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(ApiErrorResponse {
                message: message.clone(),
                error: ApiErrorDetail {
                    code: "invalid_request",
                    message,
                },
                errors: None,
            }),
        ),
        SyncTokenError::Unavailable { message } => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ApiErrorResponse {
                message: message.clone(),
                error: ApiErrorDetail {
                    code: "sync_token_unavailable",
                    message,
                },
                errors: None,
            }),
        ),
    }
}

fn validation_error(field: &'static str, message: &str) -> (StatusCode, Json<ApiErrorResponse>) {
    let mut errors = HashMap::new();
    errors.insert(field.to_string(), vec![message.to_string()]);

    (
        StatusCode::UNPROCESSABLE_ENTITY,
        Json(ApiErrorResponse {
            message: message.to_string(),
            error: ApiErrorDetail {
                code: "invalid_request",
                message: message.to_string(),
            },
            errors: Some(errors),
        }),
    )
}

fn unauthorized_error(message: &str) -> (StatusCode, Json<ApiErrorResponse>) {
    (
        StatusCode::UNAUTHORIZED,
        Json(ApiErrorResponse {
            message: message.to_string(),
            error: ApiErrorDetail {
                code: "unauthorized",
                message: message.to_string(),
            },
            errors: None,
        }),
    )
}

fn forbidden_error(message: &str) -> (StatusCode, Json<ApiErrorResponse>) {
    (
        StatusCode::FORBIDDEN,
        Json(ApiErrorResponse {
            message: message.to_string(),
            error: ApiErrorDetail {
                code: "forbidden",
                message: message.to_string(),
            },
            errors: None,
        }),
    )
}

fn session_payload(bundle: SessionBundle) -> serde_json::Value {
    serde_json::json!({
        "data": {
            "session": {
                "sessionId": bundle.session.session_id,
                "userId": bundle.session.user_id,
                "email": bundle.session.email,
                "deviceId": bundle.session.device_id,
                "status": bundle.session.status,
                "tokenName": bundle.session.token_name,
                "issuedAt": timestamp(bundle.session.issued_at),
                "accessExpiresAt": timestamp(bundle.session.access_expires_at),
                "refreshExpiresAt": timestamp(bundle.session.refresh_expires_at),
                "activeOrgId": bundle.session.active_org_id,
                "reauthRequired": bundle.session.reauth_required,
            },
            "user": {
                "id": bundle.user.id,
                "email": bundle.user.email,
                "name": bundle.user.name,
                "workosId": bundle.user.workos_user_id,
            },
            "memberships": bundle.memberships,
        }
    })
}

fn challenge_cookie(challenge_id: &str, max_age_seconds: u64) -> String {
    format!(
        "{CHALLENGE_COOKIE_NAME}={challenge_id}; Path=/; HttpOnly; SameSite=Lax; Max-Age={max_age_seconds}"
    )
}

fn clear_cookie(name: &str) -> String {
    format!("{name}=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0")
}

fn extract_cookie_value(headers: &HeaderMap, cookie_name: &str) -> Option<String> {
    let raw = headers.get(COOKIE)?.to_str().ok()?;
    for part in raw.split(';') {
        let mut pieces = part.trim().splitn(2, '=');
        let key = pieces.next()?.trim();
        let value = pieces.next()?.trim();

        if key == cookie_name {
            return non_empty(value.to_string());
        }
    }

    None
}

fn request_id(headers: &HeaderMap) -> String {
    header_string(headers, "x-request-id")
        .and_then(non_empty)
        .unwrap_or_else(|| format!("req_{}", uuid::Uuid::new_v4().simple()))
}

fn email_domain(email: &str) -> Option<String> {
    let domain = email.split('@').nth(1)?.trim();
    if domain.is_empty() {
        None
    } else {
        Some(domain.to_string())
    }
}

fn resolve_route_cohort_key(headers: &HeaderMap) -> String {
    header_string(headers, "x-oa-route-key")
        .or_else(|| header_string(headers, "x-device-id"))
        .or_else(|| header_string(headers, "x-forwarded-for"))
        .or_else(|| header_string(headers, "user-agent"))
        .and_then(non_empty)
        .unwrap_or_else(|| "anonymous".to_string())
}

fn is_pilot_chat_route(path: &str) -> bool {
    let normalized = path.trim();
    normalized == "/chat" || normalized.starts_with("/chat/")
}

fn emit_route_split_decision_audit(
    state: &AppState,
    request_id: &str,
    decision: &RouteSplitDecision,
    user_agent: &str,
) {
    let mut event = AuditEvent::new("route.split.decision", request_id.to_string())
        .with_attribute("path", decision.path.clone())
        .with_attribute(
            "target",
            match decision.target {
                RouteTarget::Legacy => "legacy".to_string(),
                RouteTarget::RustShell => "rust_shell".to_string(),
            },
        )
        .with_attribute("reason", decision.reason.clone())
        .with_attribute("cohort_key", decision.cohort_key.clone())
        .with_attribute("user_agent", user_agent.to_string());

    if let Some(bucket) = decision.cohort_bucket {
        event = event.with_attribute("cohort_bucket", bucket.to_string());
    }

    state.observability.audit(event);
    state
        .observability
        .increment_counter("route.split.decision", request_id);
}

fn header_string(headers: &HeaderMap, key: &str) -> Option<String> {
    headers
        .get(key)
        .and_then(|value| value.to_str().ok())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn bearer_token(headers: &HeaderMap) -> Option<String> {
    let authorization = headers.get(AUTHORIZATION)?.to_str().ok()?.trim();
    let token = authorization.strip_prefix("Bearer ")?.trim();
    non_empty(token.to_string())
}

fn non_empty(value: String) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn timestamp(value: chrono::DateTime<Utc>) -> String {
    value.to_rfc3339_opts(SecondsFormat::Secs, true)
}

fn header_value(raw: &str) -> Result<HeaderValue, (StatusCode, Json<ApiErrorResponse>)> {
    HeaderValue::from_str(raw).map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiErrorResponse {
                message: "Failed to build response headers.".to_string(),
                error: ApiErrorDetail {
                    code: "internal_error",
                    message: "Failed to build response headers.".to_string(),
                },
                errors: None,
            }),
        )
    })
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::Arc;

    use anyhow::Result;
    use axum::body::Body;
    use axum::http::header::{CACHE_CONTROL, CONTENT_TYPE, SET_COOKIE};
    use axum::http::{Request, StatusCode};
    use http_body_util::BodyExt;
    use serde_json::Value;
    use tempfile::tempdir;
    use tower::ServiceExt;

    use crate::build_router;
    use crate::build_router_with_observability;
    use crate::config::Config;
    use crate::observability::{Observability, RecordingAuditSink};
    use crate::{CACHE_IMMUTABLE_ONE_YEAR, CACHE_MANIFEST};

    fn test_config(static_dir: PathBuf) -> Config {
        let bind_addr = std::net::SocketAddr::from(([127, 0, 0, 1], 0));
        Config {
            bind_addr,
            log_filter: "debug".to_string(),
            static_dir,
            auth_provider_mode: "mock".to_string(),
            workos_client_id: None,
            workos_api_key: None,
            workos_api_base_url: "https://api.workos.com".to_string(),
            mock_magic_code: "123456".to_string(),
            auth_challenge_ttl_seconds: 600,
            auth_access_ttl_seconds: 3600,
            auth_refresh_ttl_seconds: 86400,
            sync_token_enabled: true,
            sync_token_signing_key: Some("sync-test-signing-key".to_string()),
            sync_token_issuer: "https://openagents.test".to_string(),
            sync_token_audience: "openagents-sync-test".to_string(),
            sync_token_key_id: "sync-auth-test-v1".to_string(),
            sync_token_claims_version: "oa_sync_claims_v1".to_string(),
            sync_token_ttl_seconds: 300,
            sync_token_min_ttl_seconds: 60,
            sync_token_max_ttl_seconds: 900,
            sync_token_allowed_scopes: vec![
                "runtime.codex_worker_events".to_string(),
                "runtime.codex_worker_summaries".to_string(),
                "runtime.run_summaries".to_string(),
            ],
            sync_token_default_scopes: vec!["runtime.codex_worker_events".to_string()],
            route_split_enabled: true,
            route_split_mode: "cohort".to_string(),
            route_split_rust_routes: vec![
                "/chat".to_string(),
                "/workspace".to_string(),
                "/login".to_string(),
                "/register".to_string(),
                "/authenticate".to_string(),
                "/onboarding".to_string(),
                "/account".to_string(),
                "/settings".to_string(),
                "/l402".to_string(),
                "/billing".to_string(),
                "/admin".to_string(),
            ],
            route_split_cohort_percentage: 100,
            route_split_salt: "route-split-test-salt".to_string(),
            route_split_force_legacy: false,
            route_split_legacy_base_url: Some("https://legacy.openagents.test".to_string()),
        }
    }

    fn workos_required_config(static_dir: PathBuf) -> Config {
        let mut config = test_config(static_dir);
        config.auth_provider_mode = "workos".to_string();
        config.workos_client_id = None;
        config.workos_api_key = None;
        config
    }

    async fn read_json(response: axum::response::Response) -> Result<Value> {
        let bytes = response.into_body().collect().await?.to_bytes();
        let value = serde_json::from_slice::<Value>(&bytes)?;
        Ok(value)
    }

    fn cookie_value(response: &axum::response::Response) -> Option<String> {
        let header = response.headers().get(SET_COOKIE)?;
        let raw = header.to_str().ok()?;
        raw.split(';').next().map(|value| value.to_string())
    }

    #[tokio::test]
    async fn healthz_route_returns_ok() -> Result<()> {
        let app = build_router(test_config(std::env::temp_dir()));
        let request = Request::builder().uri("/healthz").body(Body::empty())?;
        let response = app.oneshot(request).await?;

        assert_eq!(response.status(), StatusCode::OK);
        let body = read_json(response).await?;
        assert_eq!(body["status"], "ok");
        assert_eq!(body["service"], "openagents-control-service");
        assert_eq!(body["auth_provider"], "mock");
        Ok(())
    }

    #[tokio::test]
    async fn readiness_route_is_not_ready_when_static_dir_missing() -> Result<()> {
        let base = tempdir()?;
        let missing_dir = base.path().join("missing-assets");
        let app = build_router(test_config(missing_dir));

        let request = Request::builder().uri("/readyz").body(Body::empty())?;
        let response = app.oneshot(request).await?;

        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        let body = read_json(response).await?;
        assert_eq!(body["status"], "not_ready");
        Ok(())
    }

    #[tokio::test]
    async fn readiness_route_is_ready_when_static_dir_exists() -> Result<()> {
        let static_dir = tempdir()?;
        let app = build_router(test_config(static_dir.path().to_path_buf()));

        let request = Request::builder().uri("/readyz").body(Body::empty())?;
        let response = app.oneshot(request).await?;

        assert_eq!(response.status(), StatusCode::OK);
        let body = read_json(response).await?;
        assert_eq!(body["status"], "ready");
        Ok(())
    }

    #[tokio::test]
    async fn static_hashed_asset_uses_immutable_cache_header() -> Result<()> {
        let static_dir = tempdir()?;
        let assets_dir = static_dir.path().join("assets");
        std::fs::create_dir_all(&assets_dir)?;
        std::fs::write(
            assets_dir.join("app-0a1b2c3d4e5f.js"),
            "console.log('openagents');",
        )?;
        let app = build_router(test_config(static_dir.path().to_path_buf()));

        let request = Request::builder()
            .uri("/assets/app-0a1b2c3d4e5f.js")
            .body(Body::empty())?;
        let response = app.oneshot(request).await?;

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response
                .headers()
                .get(CACHE_CONTROL)
                .and_then(|value| value.to_str().ok()),
            Some(CACHE_IMMUTABLE_ONE_YEAR)
        );
        let content_type = response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default();
        assert!(content_type.starts_with("text/javascript"));

        Ok(())
    }

    #[tokio::test]
    async fn manifest_uses_no_store_cache_header() -> Result<()> {
        let static_dir = tempdir()?;
        std::fs::write(
            static_dir.path().join("manifest.json"),
            r#"{"app":"assets/app-0a1b2c3d4e5f.js"}"#,
        )?;
        let app = build_router(test_config(static_dir.path().to_path_buf()));

        let request = Request::builder()
            .uri("/manifest.json")
            .body(Body::empty())?;
        let response = app.oneshot(request).await?;

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response
                .headers()
                .get(CACHE_CONTROL)
                .and_then(|value| value.to_str().ok()),
            Some(CACHE_MANIFEST)
        );

        Ok(())
    }

    #[tokio::test]
    async fn static_asset_rejects_path_traversal_segments() -> Result<()> {
        let static_dir = tempdir()?;
        let app = build_router(test_config(static_dir.path().to_path_buf()));

        let request = Request::builder()
            .uri("/assets/../manifest.json")
            .body(Body::empty())?;
        let response = app.oneshot(request).await?;

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        let body = read_json(response).await?;
        assert_eq!(body["error"]["code"], "not_found");
        Ok(())
    }

    #[tokio::test]
    async fn auth_email_and_verify_flow_returns_session_tokens() -> Result<()> {
        let app = build_router(test_config(std::env::temp_dir()));

        let send_request = Request::builder()
            .method("POST")
            .uri("/api/auth/email")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"email":"test@example.com"}"#))?;

        let send_response = app.clone().oneshot(send_request).await?;
        assert_eq!(send_response.status(), StatusCode::OK);
        let cookie = cookie_value(&send_response).unwrap_or_default();

        let verify_request = Request::builder()
            .method("POST")
            .uri("/api/auth/verify")
            .header("content-type", "application/json")
            .header("x-client", "autopilot-ios")
            .header("cookie", cookie)
            .body(Body::from(r#"{"code":"123456"}"#))?;

        let verify_response = app.clone().oneshot(verify_request).await?;
        assert_eq!(verify_response.status(), StatusCode::OK);
        let verify_body = read_json(verify_response).await?;

        assert_eq!(verify_body["status"], "authenticated");
        assert_eq!(verify_body["tokenType"], "Bearer");
        assert_eq!(verify_body["tokenName"], "mobile:autopilot-ios");

        let token = verify_body["token"]
            .as_str()
            .unwrap_or_default()
            .to_string();
        assert!(!token.is_empty());

        let session_request = Request::builder()
            .uri("/api/auth/session")
            .header("authorization", format!("Bearer {token}"))
            .body(Body::empty())?;

        let session_response = app.oneshot(session_request).await?;
        assert_eq!(session_response.status(), StatusCode::OK);

        Ok(())
    }

    #[tokio::test]
    async fn auth_verify_requires_pending_challenge_cookie() -> Result<()> {
        let app = build_router(test_config(std::env::temp_dir()));

        let request = Request::builder()
            .method("POST")
            .uri("/api/auth/verify")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"code":"123456"}"#))?;

        let response = app.oneshot(request).await?;
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        let body = read_json(response).await?;
        assert_eq!(body["error"]["code"], "invalid_request");
        Ok(())
    }

    #[tokio::test]
    async fn auth_email_rejects_when_workos_is_not_configured() -> Result<()> {
        let app = build_router(workos_required_config(std::env::temp_dir()));

        let request = Request::builder()
            .method("POST")
            .uri("/api/auth/email")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"email":"workos-required@example.com"}"#))?;

        let response = app.oneshot(request).await?;
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);

        let body = read_json(response).await?;
        assert_eq!(body["error"]["code"], "service_unavailable");

        let message = body["error"]["message"].as_str().unwrap_or_default();
        assert!(message.contains("WorkOS identity provider is required"));

        Ok(())
    }

    #[tokio::test]
    async fn refresh_rotates_refresh_token_and_logout_revokes_session() -> Result<()> {
        let app = build_router(test_config(std::env::temp_dir()));

        let send_request = Request::builder()
            .method("POST")
            .uri("/api/auth/email")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"email":"rotation@example.com"}"#))?;
        let send_response = app.clone().oneshot(send_request).await?;
        let cookie = cookie_value(&send_response).unwrap_or_default();

        let verify_request = Request::builder()
            .method("POST")
            .uri("/api/auth/verify")
            .header("content-type", "application/json")
            .header("x-client", "autopilot-desktop")
            .header("cookie", cookie)
            .body(Body::from(r#"{"code":"123456"}"#))?;
        let verify_response = app.clone().oneshot(verify_request).await?;
        let verify_body = read_json(verify_response).await?;

        let token = verify_body["token"]
            .as_str()
            .unwrap_or_default()
            .to_string();
        let refresh_token = verify_body["refreshToken"]
            .as_str()
            .unwrap_or_default()
            .to_string();

        let refresh_request = Request::builder()
            .method("POST")
            .uri("/api/auth/refresh")
            .header("content-type", "application/json")
            .body(Body::from(format!(
                r#"{{"refresh_token":"{refresh_token}","rotate_refresh_token":true}}"#
            )))?;
        let refresh_response = app.clone().oneshot(refresh_request).await?;
        assert_eq!(refresh_response.status(), StatusCode::OK);
        let refresh_body = read_json(refresh_response).await?;
        let new_token = refresh_body["token"]
            .as_str()
            .unwrap_or_default()
            .to_string();

        let logout_request = Request::builder()
            .method("POST")
            .uri("/api/auth/logout")
            .header("authorization", format!("Bearer {new_token}"))
            .body(Body::empty())?;
        let logout_response = app.clone().oneshot(logout_request).await?;
        assert_eq!(logout_response.status(), StatusCode::OK);

        let old_session_request = Request::builder()
            .uri("/api/auth/session")
            .header("authorization", format!("Bearer {token}"))
            .body(Body::empty())?;
        let old_session_response = app.oneshot(old_session_request).await?;
        assert_eq!(old_session_response.status(), StatusCode::UNAUTHORIZED);

        Ok(())
    }

    #[tokio::test]
    async fn org_membership_and_policy_matrix_enforces_boundaries() -> Result<()> {
        let app = build_router(test_config(std::env::temp_dir()));

        let send_request = Request::builder()
            .method("POST")
            .uri("/api/auth/email")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"email":"policy@openagents.com"}"#))?;
        let send_response = app.clone().oneshot(send_request).await?;
        let cookie = cookie_value(&send_response).unwrap_or_default();

        let verify_request = Request::builder()
            .method("POST")
            .uri("/api/auth/verify")
            .header("content-type", "application/json")
            .header("x-client", "autopilot-ios")
            .header("cookie", cookie)
            .body(Body::from(r#"{"code":"123456"}"#))?;
        let verify_response = app.clone().oneshot(verify_request).await?;
        let verify_body = read_json(verify_response).await?;
        let token = verify_body["token"]
            .as_str()
            .unwrap_or_default()
            .to_string();

        let memberships_request = Request::builder()
            .uri("/api/orgs/memberships")
            .header("authorization", format!("Bearer {token}"))
            .body(Body::empty())?;
        let memberships_response = app.clone().oneshot(memberships_request).await?;
        assert_eq!(memberships_response.status(), StatusCode::OK);
        let memberships_body = read_json(memberships_response).await?;
        let memberships = memberships_body["data"]["memberships"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        let has_openagents_org = memberships.iter().any(|membership| {
            membership["org_id"]
                .as_str()
                .map(|org_id| org_id == "org:openagents")
                .unwrap_or(false)
        });
        assert!(has_openagents_org);

        let set_org_request = Request::builder()
            .method("POST")
            .uri("/api/orgs/active")
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {token}"))
            .body(Body::from(r#"{"org_id":"org:openagents"}"#))?;
        let set_org_response = app.clone().oneshot(set_org_request).await?;
        assert_eq!(set_org_response.status(), StatusCode::OK);

        let deny_scope_request = Request::builder()
            .method("POST")
            .uri("/api/policy/authorize")
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {token}"))
            .body(Body::from(
                r#"{"org_id":"org:openagents","required_scopes":["runtime.write"],"requested_topics":["org:openagents:workers"]}"#,
            ))?;
        let deny_scope_response = app.clone().oneshot(deny_scope_request).await?;
        assert_eq!(deny_scope_response.status(), StatusCode::OK);
        let deny_scope_body = read_json(deny_scope_response).await?;
        assert_eq!(deny_scope_body["data"]["allowed"], false);

        let allow_request = Request::builder()
            .method("POST")
            .uri("/api/policy/authorize")
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {token}"))
            .body(Body::from(
                r#"{"org_id":"org:openagents","required_scopes":["runtime.read"],"requested_topics":["org:openagents:workers"]}"#,
            ))?;
        let allow_response = app.clone().oneshot(allow_request).await?;
        assert_eq!(allow_response.status(), StatusCode::OK);
        let allow_body = read_json(allow_response).await?;
        assert_eq!(allow_body["data"]["allowed"], true);

        let deny_topic_request = Request::builder()
            .method("POST")
            .uri("/api/policy/authorize")
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {token}"))
            .body(Body::from(
                r#"{"org_id":"org:openagents","required_scopes":["runtime.read"],"requested_topics":["org:other:workers"]}"#,
            ))?;
        let deny_topic_response = app.oneshot(deny_topic_request).await?;
        assert_eq!(deny_topic_response.status(), StatusCode::OK);
        let deny_topic_body = read_json(deny_topic_response).await?;
        assert_eq!(deny_topic_body["data"]["allowed"], false);

        Ok(())
    }

    #[tokio::test]
    async fn sync_token_mint_enforces_scope_and_org_policy() -> Result<()> {
        let app = build_router(test_config(std::env::temp_dir()));

        let send_request = Request::builder()
            .method("POST")
            .uri("/api/auth/email")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"email":"sync@openagents.com"}"#))?;
        let send_response = app.clone().oneshot(send_request).await?;
        let cookie = cookie_value(&send_response).unwrap_or_default();

        let verify_request = Request::builder()
            .method("POST")
            .uri("/api/auth/verify")
            .header("content-type", "application/json")
            .header("x-client", "autopilot-ios")
            .header("cookie", cookie)
            .body(Body::from(r#"{"code":"123456"}"#))?;
        let verify_response = app.clone().oneshot(verify_request).await?;
        let verify_body = read_json(verify_response).await?;
        let token = verify_body["token"]
            .as_str()
            .unwrap_or_default()
            .to_string();

        let memberships_request = Request::builder()
            .uri("/api/orgs/memberships")
            .header("authorization", format!("Bearer {token}"))
            .body(Body::empty())?;
        let memberships_response = app.clone().oneshot(memberships_request).await?;
        let memberships_body = read_json(memberships_response).await?;
        let memberships = memberships_body["data"]["memberships"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        let personal_org_id = memberships
            .iter()
            .find(|membership| membership["default_org"].as_bool().unwrap_or(false))
            .and_then(|membership| membership["org_id"].as_str())
            .unwrap_or_default()
            .to_string();

        let set_openagents_org = Request::builder()
            .method("POST")
            .uri("/api/orgs/active")
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {token}"))
            .body(Body::from(r#"{"org_id":"org:openagents"}"#))?;
        let set_openagents_response = app.clone().oneshot(set_openagents_org).await?;
        assert_eq!(set_openagents_response.status(), StatusCode::OK);

        let denied_by_policy_request = Request::builder()
            .method("POST")
            .uri("/api/sync/token")
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {token}"))
            .body(Body::from(
                r#"{"scopes":["runtime.codex_worker_events"],"topics":["org:openagents:worker_events"]}"#,
            ))?;
        let denied_by_policy_response = app.clone().oneshot(denied_by_policy_request).await?;
        assert_eq!(denied_by_policy_response.status(), StatusCode::FORBIDDEN);

        let set_personal_org = Request::builder()
            .method("POST")
            .uri("/api/orgs/active")
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {token}"))
            .body(Body::from(format!(r#"{{"org_id":"{personal_org_id}"}}"#)))?;
        let set_personal_response = app.clone().oneshot(set_personal_org).await?;
        assert_eq!(set_personal_response.status(), StatusCode::OK);

        let invalid_scope_request = Request::builder()
            .method("POST")
            .uri("/api/sync/token")
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {token}"))
            .body(Body::from(r#"{"scopes":["runtime.unknown_scope"]}"#))?;
        let invalid_scope_response = app.clone().oneshot(invalid_scope_request).await?;
        assert_eq!(
            invalid_scope_response.status(),
            StatusCode::UNPROCESSABLE_ENTITY
        );
        let invalid_scope_body = read_json(invalid_scope_response).await?;
        assert_eq!(invalid_scope_body["error"]["code"], "invalid_scope");

        let mismatched_device_request = Request::builder()
            .method("POST")
            .uri("/api/sync/token")
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {token}"))
            .body(Body::from(
                r#"{"scopes":["runtime.codex_worker_events"],"device_id":"mobile:other-device"}"#,
            ))?;
        let mismatched_device_response = app.clone().oneshot(mismatched_device_request).await?;
        assert_eq!(mismatched_device_response.status(), StatusCode::FORBIDDEN);

        let unsupported_topic_request = Request::builder()
            .method("POST")
            .uri("/api/sync/token")
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {token}"))
            .body(Body::from(
                r#"{"scopes":["runtime.codex_worker_events"],"topics":["org:openagents:unknown"]}"#,
            ))?;
        let unsupported_topic_response = app.clone().oneshot(unsupported_topic_request).await?;
        assert_eq!(
            unsupported_topic_response.status(),
            StatusCode::UNPROCESSABLE_ENTITY
        );
        let unsupported_topic_body = read_json(unsupported_topic_response).await?;
        assert_eq!(unsupported_topic_body["error"]["code"], "invalid_request");

        let success_request = Request::builder()
            .method("POST")
            .uri("/api/sync/token")
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {token}"))
            .body(Body::from(
                r#"{"scopes":["runtime.codex_worker_events","runtime.run_summaries"]}"#,
            ))?;
        let success_response = app.oneshot(success_request).await?;
        assert_eq!(success_response.status(), StatusCode::OK);
        let success_body = read_json(success_response).await?;
        assert_eq!(success_body["data"]["token_type"], "Bearer");
        assert_eq!(success_body["data"]["issuer"], "https://openagents.test");
        assert_eq!(success_body["data"]["claims_version"], "oa_sync_claims_v1");

        Ok(())
    }

    #[tokio::test]
    async fn sync_token_requires_active_non_revoked_session() -> Result<()> {
        let app = build_router(test_config(std::env::temp_dir()));

        let send_request = Request::builder()
            .method("POST")
            .uri("/api/auth/email")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"email":"sync-revoke@openagents.com"}"#))?;
        let send_response = app.clone().oneshot(send_request).await?;
        let cookie = cookie_value(&send_response).unwrap_or_default();

        let verify_request = Request::builder()
            .method("POST")
            .uri("/api/auth/verify")
            .header("content-type", "application/json")
            .header("x-client", "autopilot-ios")
            .header("cookie", cookie)
            .body(Body::from(r#"{"code":"123456"}"#))?;
        let verify_response = app.clone().oneshot(verify_request).await?;
        let verify_body = read_json(verify_response).await?;
        let token = verify_body["token"]
            .as_str()
            .unwrap_or_default()
            .to_string();

        let logout_request = Request::builder()
            .method("POST")
            .uri("/api/auth/logout")
            .header("authorization", format!("Bearer {token}"))
            .body(Body::empty())?;
        let logout_response = app.clone().oneshot(logout_request).await?;
        assert_eq!(logout_response.status(), StatusCode::OK);

        let sync_request = Request::builder()
            .method("POST")
            .uri("/api/sync/token")
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {token}"))
            .body(Body::from(r#"{"scopes":["runtime.codex_worker_events"]}"#))?;
        let sync_response = app.oneshot(sync_request).await?;
        assert_eq!(sync_response.status(), StatusCode::UNAUTHORIZED);

        Ok(())
    }

    #[tokio::test]
    async fn thread_message_command_requires_authentication() -> Result<()> {
        let app = build_router(test_config(std::env::temp_dir()));

        let request = Request::builder()
            .method("POST")
            .uri("/api/runtime/threads/thread-1/messages")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"text":"hello"}"#))?;
        let response = app.oneshot(request).await?;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        Ok(())
    }

    #[tokio::test]
    async fn thread_message_command_accepts_authenticated_message() -> Result<()> {
        let app = build_router(test_config(std::env::temp_dir()));

        let send_request = Request::builder()
            .method("POST")
            .uri("/api/auth/email")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"email":"thread-command@openagents.com"}"#))?;
        let send_response = app.clone().oneshot(send_request).await?;
        let cookie = cookie_value(&send_response).unwrap_or_default();

        let verify_request = Request::builder()
            .method("POST")
            .uri("/api/auth/verify")
            .header("content-type", "application/json")
            .header("x-client", "autopilot-ios")
            .header("cookie", cookie)
            .body(Body::from(r#"{"code":"123456"}"#))?;
        let verify_response = app.clone().oneshot(verify_request).await?;
        let verify_body = read_json(verify_response).await?;
        let token = verify_body["token"]
            .as_str()
            .unwrap_or_default()
            .to_string();

        let command_request = Request::builder()
            .method("POST")
            .uri("/api/runtime/threads/thread-42/messages")
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {token}"))
            .body(Body::from("{\"text\":\"Who are you?\"}"))?;
        let command_response = app.oneshot(command_request).await?;
        assert_eq!(command_response.status(), StatusCode::OK);
        let command_body = read_json(command_response).await?;
        assert_eq!(command_body["data"]["accepted"], true);
        assert_eq!(command_body["data"]["message"]["thread_id"], "thread-42");
        assert_eq!(command_body["data"]["message"]["text"], "Who are you?");
        assert!(
            command_body["data"]["message"]["id"]
                .as_str()
                .unwrap_or_default()
                .starts_with("msg_")
        );

        Ok(())
    }

    #[tokio::test]
    async fn audit_events_include_request_correlation_and_identity_fields() -> Result<()> {
        let static_dir = tempdir()?;
        let sink = Arc::new(RecordingAuditSink::default());
        let app = build_router_with_observability(
            test_config(static_dir.path().to_path_buf()),
            Observability::new(sink.clone()),
        );

        let send_request = Request::builder()
            .method("POST")
            .uri("/api/auth/email")
            .header("x-request-id", "req-auth-email")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"email":"audit@openagents.com"}"#))?;
        let send_response = app.clone().oneshot(send_request).await?;
        let cookie = cookie_value(&send_response).unwrap_or_default();

        let verify_request = Request::builder()
            .method("POST")
            .uri("/api/auth/verify")
            .header("x-request-id", "req-auth-verify")
            .header("content-type", "application/json")
            .header("x-client", "autopilot-ios")
            .header("cookie", cookie)
            .body(Body::from(r#"{"code":"123456"}"#))?;
        let verify_response = app.clone().oneshot(verify_request).await?;
        let verify_body = read_json(verify_response).await?;
        let token = verify_body["token"]
            .as_str()
            .unwrap_or_default()
            .to_string();

        let sync_request = Request::builder()
            .method("POST")
            .uri("/api/sync/token")
            .header("x-request-id", "req-sync-token")
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {token}"))
            .body(Body::from(r#"{"scopes":["runtime.codex_worker_events"]}"#))?;
        let sync_response = app.oneshot(sync_request).await?;
        assert_eq!(sync_response.status(), StatusCode::OK);

        let events = sink.events();
        let verify_event = events
            .iter()
            .find(|event| event.event_name == "auth.verify.completed")
            .expect("missing auth.verify.completed event");
        assert_eq!(verify_event.request_id, "req-auth-verify");
        assert_eq!(verify_event.outcome, "success");
        assert!(verify_event.user_id.is_some());
        assert!(verify_event.session_id.is_some());
        assert!(verify_event.org_id.is_some());
        assert!(verify_event.device_id.is_some());

        let sync_event = events
            .iter()
            .find(|event| event.event_name == "sync.token.issued")
            .expect("missing sync.token.issued event");
        assert_eq!(sync_event.request_id, "req-sync-token");
        assert_eq!(sync_event.outcome, "success");
        assert!(sync_event.attributes.contains_key("scope_count"));
        assert!(sync_event.attributes.contains_key("topic_count"));
        assert!(sync_event.attributes.contains_key("expires_in"));

        Ok(())
    }

    #[tokio::test]
    async fn route_split_serves_rust_shell_and_audits_decision() -> Result<()> {
        let static_dir = tempdir()?;
        std::fs::write(
            static_dir.path().join("index.html"),
            "<!doctype html><html><body>rust shell</body></html>",
        )?;

        let sink = Arc::new(RecordingAuditSink::default());
        let app = build_router_with_observability(
            test_config(static_dir.path().to_path_buf()),
            Observability::new(sink.clone()),
        );

        let request = Request::builder()
            .uri("/chat/thread-1")
            .header("x-request-id", "req-route-split")
            .header("x-oa-route-key", "user:route")
            .header("user-agent", "autopilot-ios")
            .body(Body::empty())?;
        let response = app.oneshot(request).await?;
        assert_eq!(response.status(), StatusCode::OK);

        let body = response.into_body().collect().await?.to_bytes();
        let html = String::from_utf8_lossy(&body);
        assert!(html.contains("rust shell"));

        let events = sink.events();
        let decision_event = events
            .iter()
            .find(|event| event.event_name == "route.split.decision")
            .expect("missing route.split.decision audit event");
        assert_eq!(decision_event.request_id, "req-route-split");
        assert_eq!(
            decision_event
                .attributes
                .get("target")
                .map(String::as_str)
                .unwrap_or_default(),
            "rust_shell"
        );

        Ok(())
    }

    #[tokio::test]
    async fn route_split_serves_management_route_prefixes_in_rust_cohort() -> Result<()> {
        let static_dir = tempdir()?;
        std::fs::write(
            static_dir.path().join("index.html"),
            "<!doctype html><html><body>rust shell</body></html>",
        )?;
        let app = build_router(test_config(static_dir.path().to_path_buf()));

        for path in [
            "/account/session",
            "/settings/profile",
            "/l402/paywalls",
            "/billing/deployments",
            "/admin",
        ] {
            let request = Request::builder()
                .uri(path)
                .header("x-oa-route-key", "user:route")
                .body(Body::empty())?;
            let response = app.clone().oneshot(request).await?;
            assert_eq!(
                response.status(),
                StatusCode::OK,
                "unexpected status for {path}"
            );
            let body = response.into_body().collect().await?.to_bytes();
            let html = String::from_utf8_lossy(&body);
            assert!(
                html.contains("rust shell"),
                "management route was not served by rust shell: {path}"
            );
        }

        Ok(())
    }

    #[tokio::test]
    async fn route_split_serves_auth_entry_routes_in_rust_cohort() -> Result<()> {
        let static_dir = tempdir()?;
        std::fs::write(
            static_dir.path().join("index.html"),
            "<!doctype html><html><body>rust shell</body></html>",
        )?;
        let app = build_router(test_config(static_dir.path().to_path_buf()));

        for path in [
            "/login",
            "/register",
            "/authenticate",
            "/onboarding/checklist",
        ] {
            let request = Request::builder()
                .uri(path)
                .header("x-oa-route-key", "user:route")
                .body(Body::empty())?;
            let response = app.clone().oneshot(request).await?;
            assert_eq!(
                response.status(),
                StatusCode::OK,
                "unexpected status for {path}"
            );
            let body = response.into_body().collect().await?.to_bytes();
            let html = String::from_utf8_lossy(&body);
            assert!(
                html.contains("rust shell"),
                "auth route was not served by rust shell: {path}"
            );
        }

        Ok(())
    }

    #[tokio::test]
    async fn route_split_rust_mode_with_root_prefix_serves_unlisted_paths() -> Result<()> {
        let static_dir = tempdir()?;
        std::fs::write(
            static_dir.path().join("index.html"),
            "<!doctype html><html><body>rust shell</body></html>",
        )?;

        let mut config = test_config(static_dir.path().to_path_buf());
        config.route_split_mode = "rust".to_string();
        config.route_split_rust_routes = vec!["/".to_string()];
        let app = build_router(config);

        for path in ["/feed", "/new-surface/path", "/login"] {
            let request = Request::builder()
                .uri(path)
                .header("x-oa-route-key", "user:route")
                .body(Body::empty())?;
            let response = app.clone().oneshot(request).await?;
            assert_eq!(
                response.status(),
                StatusCode::OK,
                "unexpected status for {path}"
            );
            let body = response.into_body().collect().await?.to_bytes();
            let html = String::from_utf8_lossy(&body);
            assert!(
                html.contains("rust shell"),
                "path was not served by rust shell: {path}"
            );
        }

        Ok(())
    }

    #[tokio::test]
    async fn route_split_override_keeps_chat_pilot_on_rust_shell() -> Result<()> {
        let static_dir = tempdir()?;
        std::fs::write(
            static_dir.path().join("index.html"),
            "<!doctype html><html><body>rust shell</body></html>",
        )?;
        let app = build_router(test_config(static_dir.path().to_path_buf()));

        let send_request = Request::builder()
            .method("POST")
            .uri("/api/auth/email")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"email":"routes@openagents.com"}"#))?;
        let send_response = app.clone().oneshot(send_request).await?;
        let cookie = cookie_value(&send_response).unwrap_or_default();

        let verify_request = Request::builder()
            .method("POST")
            .uri("/api/auth/verify")
            .header("content-type", "application/json")
            .header("x-client", "autopilot-ios")
            .header("cookie", cookie)
            .body(Body::from(r#"{"code":"123456"}"#))?;
        let verify_response = app.clone().oneshot(verify_request).await?;
        let verify_body = read_json(verify_response).await?;
        let token = verify_body["token"]
            .as_str()
            .unwrap_or_default()
            .to_string();

        let override_request = Request::builder()
            .method("POST")
            .uri("/api/v1/control/route-split/override")
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {token}"))
            .body(Body::from(r#"{"target":"legacy"}"#))?;
        let override_response = app.clone().oneshot(override_request).await?;
        assert_eq!(override_response.status(), StatusCode::OK);

        let route_request = Request::builder()
            .uri("/chat/thread-1")
            .header("x-oa-route-key", "user:route")
            .body(Body::empty())?;
        let route_response = app.clone().oneshot(route_request).await?;
        assert_eq!(route_response.status(), StatusCode::OK);
        let route_body = route_response.into_body().collect().await?.to_bytes();
        let route_html = String::from_utf8_lossy(&route_body);
        assert!(route_html.contains("rust shell"));

        let workspace_request = Request::builder()
            .uri("/workspace/session-1")
            .header("x-oa-route-key", "user:route")
            .body(Body::empty())?;
        let workspace_response = app.oneshot(workspace_request).await?;
        assert_eq!(workspace_response.status(), StatusCode::TEMPORARY_REDIRECT);
        assert_eq!(
            workspace_response
                .headers()
                .get("location")
                .and_then(|value| value.to_str().ok()),
            Some("https://legacy.openagents.test/workspace/session-1")
        );

        Ok(())
    }
}
