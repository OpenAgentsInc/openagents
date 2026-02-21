use std::collections::HashMap;
use std::sync::Arc;
use std::time::SystemTime;

use axum::extract::State;
use axum::http::header::{AUTHORIZATION, COOKIE, SET_COOKIE};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::{SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use tower::ServiceBuilder;
use tower_http::request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer};
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;

pub mod auth;
pub mod config;

use crate::auth::{AuthError, AuthService, PolicyCheckRequest, SessionBundle};
use crate::config::Config;

const SERVICE_NAME: &str = "openagents-control-service";
const CHALLENGE_COOKIE_NAME: &str = "oa_magic_challenge";

#[derive(Clone)]
struct AppState {
    config: Arc<Config>,
    auth: AuthService,
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

pub fn build_router(config: Config) -> Router {
    let auth = AuthService::from_config(&config);
    let state = AppState {
        config: Arc::new(config),
        auth,
        started_at: SystemTime::now(),
    };

    let static_service = ServeDir::new(state.config.static_dir.clone());

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
        .route("/api/v1/auth/session", get(current_session))
        .route("/api/v1/control/status", get(control_status))
        .route("/api/v1/sync/token", post(sync_token_placeholder))
        .nest_service("/assets", static_service)
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

async fn send_email_code(
    State(state): State<AppState>,
    Json(payload): Json<SendEmailCodeRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiErrorResponse>)> {
    let challenge = state
        .auth
        .start_challenge(payload.email)
        .await
        .map_err(map_auth_error)?;

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

async fn refresh_session(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<RefreshSessionRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiErrorResponse>)> {
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
    let access_token =
        bearer_token(&headers).ok_or_else(|| unauthorized_error("Unauthenticated."))?;

    let revoked = state
        .auth
        .revoke_session_by_access_token(&access_token)
        .await
        .map_err(map_auth_error)?;

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

    let response = serde_json::json!({
        "data": {
            "service": SERVICE_NAME,
            "authProvider": state.auth.provider_name(),
            "activeOrgId": bundle.session.active_org_id,
            "memberships": bundle.memberships,
        }
    });

    Ok((StatusCode::OK, Json(response)))
}

async fn sync_token_placeholder() -> impl IntoResponse {
    not_implemented(
        "Sync-token minting API wiring lands in the OA-RUST-018 milestone.",
        "OA-RUST-018",
    )
}

fn not_implemented(
    message: &'static str,
    next_issue: &'static str,
) -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::NOT_IMPLEMENTED,
        Json(serde_json::json!({
            "error": "not_implemented",
            "message": message,
            "nextIssue": next_issue,
        })),
    )
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

    use anyhow::Result;
    use axum::body::Body;
    use axum::http::header::SET_COOKIE;
    use axum::http::{Request, StatusCode};
    use http_body_util::BodyExt;
    use serde_json::Value;
    use tempfile::tempdir;
    use tower::ServiceExt;

    use crate::build_router;
    use crate::config::Config;

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
        }
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
}
