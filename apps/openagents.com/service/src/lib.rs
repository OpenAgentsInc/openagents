use std::collections::{HashMap, VecDeque};
use std::path::Path as FsPath;
use std::sync::Arc;
use std::time::SystemTime;

use axum::body::Body;
use axum::body::to_bytes;
use axum::extract::{Form, Path, Query, Request, State};
use axum::http::header::{AUTHORIZATION, CACHE_CONTROL, CONTENT_TYPE, COOKIE, SET_COOKIE};
use axum::http::{HeaderMap, HeaderValue, Method, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use chrono::{SecondsFormat, Utc};
use hmac::{Hmac, Mac};
use openagents_client_core::compatibility::{
    ClientCompatibilityHandshake, CompatibilityFailure, CompatibilitySurface, CompatibilityWindow,
    negotiate_compatibility,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::sync::Mutex;
use tower::ServiceBuilder;
use tower_http::request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer};
use tower_http::trace::TraceLayer;

pub mod api_envelope;
pub mod auth;
pub mod codex_threads;
pub mod config;
pub mod domain_store;
pub mod khala_token;
pub mod observability;
pub mod openapi;
pub mod route_split;
pub mod sync_token;

use crate::api_envelope::{
    ApiErrorCode, ApiErrorResponse, error_response_with_status, forbidden_error, not_found_error,
    ok_data, unauthorized_error, validation_error,
};
use crate::auth::{
    AuthError, AuthService, PolicyCheckRequest, SessionBundle, SessionRevocationReason,
    SessionRevocationRequest, SessionRevocationTarget,
};
use crate::codex_threads::{
    AutopilotThreadProjection, CodexThreadStore, ThreadMessageProjection, ThreadProjection,
    ThreadStoreError,
};
use crate::config::Config;
use crate::domain_store::{
    AutopilotAggregate, CreateAutopilotInput, DomainStore, DomainStoreError, UpdateAutopilotInput,
    UpsertAutopilotPolicyInput, UpsertAutopilotProfileInput,
};
use crate::khala_token::{KhalaTokenError, KhalaTokenIssueRequest, KhalaTokenIssuer};
use crate::observability::{AuditEvent, Observability};
use crate::openapi::{
    ROUTE_AUTH_EMAIL, ROUTE_AUTH_LOGOUT, ROUTE_AUTH_REFRESH, ROUTE_AUTH_REGISTER,
    ROUTE_AUTH_SESSION, ROUTE_AUTH_SESSIONS, ROUTE_AUTH_SESSIONS_REVOKE, ROUTE_AUTH_VERIFY,
    ROUTE_AUTOPILOTS, ROUTE_AUTOPILOTS_BY_ID, ROUTE_AUTOPILOTS_STREAM, ROUTE_AUTOPILOTS_THREADS,
    ROUTE_KHALA_TOKEN, ROUTE_ME, ROUTE_OPENAPI_JSON, ROUTE_ORGS_ACTIVE, ROUTE_ORGS_MEMBERSHIPS,
    ROUTE_POLICY_AUTHORIZE, ROUTE_RUNTIME_CODEX_WORKER_REQUESTS, ROUTE_RUNTIME_THREAD_MESSAGES,
    ROUTE_RUNTIME_THREADS, ROUTE_SETTINGS_PROFILE, ROUTE_SYNC_TOKEN, ROUTE_TOKENS,
    ROUTE_TOKENS_BY_ID, ROUTE_TOKENS_CURRENT, ROUTE_V1_AUTH_SESSION, ROUTE_V1_AUTH_SESSIONS,
    ROUTE_V1_AUTH_SESSIONS_REVOKE, ROUTE_V1_CONTROL_ROUTE_SPLIT_EVALUATE,
    ROUTE_V1_CONTROL_ROUTE_SPLIT_OVERRIDE, ROUTE_V1_CONTROL_ROUTE_SPLIT_STATUS,
    ROUTE_V1_CONTROL_STATUS, ROUTE_V1_SYNC_TOKEN, openapi_document,
};
use crate::route_split::{RouteSplitDecision, RouteSplitService, RouteTarget};
use crate::sync_token::{SyncTokenError, SyncTokenIssueRequest, SyncTokenIssuer};

const SERVICE_NAME: &str = "openagents-control-service";
const CHALLENGE_COOKIE_NAME: &str = "oa_magic_challenge";
const AUTH_ACCESS_COOKIE_NAME: &str = "oa_access_token";
const AUTH_REFRESH_COOKIE_NAME: &str = "oa_refresh_token";
const LOCAL_TEST_AUTH_COOKIE_NAME: &str = "oa_local_test_auth";
const CACHE_IMMUTABLE_ONE_YEAR: &str = "public, max-age=31536000, immutable";
const CACHE_SHORT_LIVED: &str = "public, max-age=60";
const CACHE_MANIFEST: &str = "no-cache, no-store, must-revalidate";
const HEADER_OA_CLIENT_BUILD_ID: &str = "x-oa-client-build-id";
const HEADER_OA_COMPAT_CODE: &str = "x-oa-compatibility-code";
const HEADER_OA_COMPAT_MAX_BUILD: &str = "x-oa-compatibility-max-client-build-id";
const HEADER_OA_COMPAT_MAX_SCHEMA: &str = "x-oa-compatibility-max-schema-version";
const HEADER_OA_COMPAT_MIN_BUILD: &str = "x-oa-compatibility-min-client-build-id";
const HEADER_OA_COMPAT_MIN_SCHEMA: &str = "x-oa-compatibility-min-schema-version";
const HEADER_OA_COMPAT_PROTOCOL: &str = "x-oa-compatibility-protocol-version";
const HEADER_OA_COMPAT_UPGRADE_REQUIRED: &str = "x-oa-compatibility-upgrade-required";
const HEADER_OA_PROTOCOL_VERSION: &str = "x-oa-protocol-version";
const HEADER_OA_SCHEMA_VERSION: &str = "x-oa-schema-version";
const HEADER_X_FORWARDED_FOR: &str = "x-forwarded-for";
const HEADER_X_REAL_IP: &str = "x-real-ip";
const RUNTIME_INTERNAL_BODY_HASH_HEADER: &str = "x-oa-internal-body-sha256";
const RUNTIME_INTERNAL_KEY_ID_HEADER: &str = "x-oa-internal-key-id";
const RUNTIME_INTERNAL_NONCE_HEADER: &str = "x-oa-internal-nonce";
const RUNTIME_INTERNAL_SIGNATURE_HEADER: &str = "x-oa-internal-signature";
const RUNTIME_INTERNAL_TIMESTAMP_HEADER: &str = "x-oa-internal-timestamp";
const MAINTENANCE_BYPASS_QUERY_PARAM: &str = "maintenance_bypass";
const MAINTENANCE_CACHE_CONTROL: &str = "no-store, no-cache, must-revalidate";
const THROTTLE_AUTH_EMAIL_LIMIT: usize = 30;
const THROTTLE_AUTH_EMAIL_WINDOW_SECONDS: i64 = 60;
const THROTTLE_LOGIN_EMAIL_LIMIT: usize = 6;
const THROTTLE_LOGIN_EMAIL_WINDOW_SECONDS: i64 = 60;
const THROTTLE_LOGIN_VERIFY_LIMIT: usize = 10;
const THROTTLE_LOGIN_VERIFY_WINDOW_SECONDS: i64 = 60;
const THROTTLE_THREAD_MESSAGE_LIMIT: usize = 60;
const THROTTLE_THREAD_MESSAGE_WINDOW_SECONDS: i64 = 60;
const THROTTLE_CODEX_CONTROL_REQUEST_LIMIT: usize = 60;
const THROTTLE_CODEX_CONTROL_REQUEST_WINDOW_SECONDS: i64 = 60;
const RUNTIME_INTERNAL_NONCE_GRACE_SECONDS: i64 = 5;
const RUNTIME_INTERNAL_MAX_BODY_BYTES: usize = 1024 * 1024;
const CODEX_CONTROL_METHOD_ALLOWLIST: &[&str] = &[
    "thread/start",
    "thread/resume",
    "turn/start",
    "turn/interrupt",
    "thread/list",
    "thread/read",
];
type HmacSha256 = Hmac<Sha256>;

#[derive(Clone)]
struct AppState {
    config: Arc<Config>,
    auth: AuthService,
    observability: Observability,
    route_split: RouteSplitService,
    khala_token_issuer: KhalaTokenIssuer,
    sync_token_issuer: SyncTokenIssuer,
    codex_thread_store: CodexThreadStore,
    _domain_store: DomainStore,
    runtime_revocation_client: Option<RuntimeRevocationClient>,
    throttle_state: ThrottleState,
    codex_control_receipts: CodexControlReceiptState,
    runtime_internal_nonces: RuntimeInternalNonceState,
    started_at: SystemTime,
}

#[derive(Clone, Default)]
struct ThrottleState {
    buckets: Arc<Mutex<HashMap<String, VecDeque<i64>>>>,
}

#[derive(Clone, Default)]
struct RuntimeInternalNonceState {
    entries: Arc<Mutex<HashMap<String, i64>>>,
}

#[derive(Clone, Default)]
struct CodexControlReceiptState {
    entries: Arc<Mutex<HashMap<String, serde_json::Value>>>,
}

#[derive(Clone)]
struct RuntimeRevocationClient {
    endpoint_url: String,
    signature_secret: String,
    signature_ttl_seconds: u64,
    http: reqwest::Client,
}

#[derive(Debug, Serialize)]
struct RuntimeRevocationRequest {
    session_ids: Vec<String>,
    device_ids: Vec<String>,
    reason: String,
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
struct CompatibilityErrorDetail {
    code: String,
    message: String,
}

#[derive(Debug, Serialize)]
struct CompatibilityErrorResponse {
    message: String,
    error: CompatibilityErrorDetail,
    compatibility: CompatibilityFailure,
}

#[derive(Debug, Deserialize)]
struct SendEmailCodeRequest {
    email: String,
}

#[derive(Debug, Deserialize)]
struct LoginEmailForm {
    email: String,
}

#[derive(Debug, Deserialize)]
struct VerifyEmailCodeRequest {
    code: String,
    #[serde(default)]
    challenge_id: Option<String>,
    #[serde(default, alias = "deviceId")]
    device_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct LoginVerifyForm {
    code: String,
    #[serde(default, alias = "challengeId")]
    challenge_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AuthRegisterRequest {
    email: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default, alias = "tokenName")]
    token_name: Option<String>,
    #[serde(default, alias = "tokenAbilities")]
    token_abilities: Option<Vec<String>>,
    #[serde(default, alias = "createAutopilot")]
    create_autopilot: Option<bool>,
    #[serde(default, alias = "autopilotDisplayName")]
    autopilot_display_name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RefreshSessionRequest {
    #[serde(default)]
    refresh_token: Option<String>,
    #[serde(default)]
    rotate_refresh_token: Option<bool>,
    #[serde(default)]
    device_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ListSessionsRequest {
    #[serde(default)]
    device_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RevokeSessionsRequest {
    #[serde(default)]
    session_id: Option<String>,
    #[serde(default)]
    device_id: Option<String>,
    #[serde(default)]
    revoke_all_sessions: Option<bool>,
    #[serde(default)]
    include_current: Option<bool>,
    #[serde(default)]
    reason: Option<SessionRevocationReason>,
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
struct MeQuery {
    #[serde(default)]
    chat_limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct AutopilotListQuery {
    #[serde(default)]
    limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct AutopilotThreadListQuery {
    #[serde(default)]
    limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct AutopilotStreamQuery {
    #[serde(default, alias = "conversationId")]
    conversation_id: Option<String>,
    #[serde(default, alias = "threadId")]
    thread_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CreateAutopilotRequestPayload {
    #[serde(default)]
    handle: Option<String>,
    #[serde(default, alias = "displayName")]
    display_name: Option<String>,
    #[serde(default)]
    avatar: Option<String>,
    #[serde(default)]
    tagline: Option<String>,
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    visibility: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CreateAutopilotThreadRequestPayload {
    #[serde(default)]
    title: Option<String>,
}

#[derive(Debug, Deserialize)]
struct UpdateAutopilotRequestPayload {
    #[serde(default, alias = "displayName")]
    display_name: Option<String>,
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    visibility: Option<String>,
    #[serde(default)]
    avatar: Option<String>,
    #[serde(default)]
    tagline: Option<String>,
    #[serde(default)]
    profile: Option<UpdateAutopilotProfilePayload>,
    #[serde(default)]
    policy: Option<UpdateAutopilotPolicyPayload>,
}

#[derive(Debug, Deserialize, Default)]
struct UpdateAutopilotProfilePayload {
    #[serde(default, alias = "ownerDisplayName")]
    owner_display_name: Option<String>,
    #[serde(default, alias = "personaSummary")]
    persona_summary: Option<String>,
    #[serde(default, alias = "autopilotVoice")]
    autopilot_voice: Option<String>,
    #[serde(default)]
    principles: Option<serde_json::Value>,
    #[serde(default)]
    preferences: Option<serde_json::Value>,
    #[serde(default, alias = "onboardingAnswers")]
    onboarding_answers: Option<serde_json::Value>,
    #[serde(default, alias = "schemaVersion")]
    schema_version: Option<u16>,
}

#[derive(Debug, Deserialize, Default)]
struct UpdateAutopilotPolicyPayload {
    #[serde(default, alias = "modelProvider")]
    model_provider: Option<String>,
    #[serde(default)]
    model: Option<String>,
    #[serde(default, alias = "toolAllowlist")]
    tool_allowlist: Option<Vec<String>>,
    #[serde(default, alias = "toolDenylist")]
    tool_denylist: Option<Vec<String>>,
    #[serde(default, alias = "l402RequireApproval")]
    l402_require_approval: Option<bool>,
    #[serde(default, alias = "l402MaxSpendMsatsPerCall")]
    l402_max_spend_msats_per_call: Option<u64>,
    #[serde(default, alias = "l402MaxSpendMsatsPerDay")]
    l402_max_spend_msats_per_day: Option<u64>,
    #[serde(default, alias = "l402AllowedHosts")]
    l402_allowed_hosts: Option<Vec<String>>,
    #[serde(default, alias = "dataPolicy")]
    data_policy: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct CreateTokenRequestPayload {
    name: String,
    #[serde(default)]
    abilities: Option<Vec<String>>,
    #[serde(default, alias = "expiresAt")]
    expires_at: Option<String>,
}

#[derive(Debug, Deserialize)]
struct KhalaTokenRequestPayload {
    #[serde(default)]
    scope: Vec<String>,
    #[serde(default)]
    workspace_id: Option<String>,
    #[serde(default)]
    role: Option<String>,
}

#[derive(Debug, Deserialize)]
struct UpdateProfileRequestPayload {
    name: String,
}

#[derive(Debug, Deserialize)]
struct DeleteProfileRequestPayload {
    email: String,
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
    #[serde(default)]
    domain: Option<String>,
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

#[derive(Debug, Deserialize)]
struct RuntimeCodexWorkerControlRequestEnvelope {
    request: RuntimeCodexWorkerControlRequest,
}

#[derive(Debug, Deserialize)]
struct RuntimeCodexWorkerControlRequest {
    request_id: String,
    method: String,
    #[serde(default)]
    params: serde_json::Value,
    #[serde(default)]
    request_version: Option<String>,
    #[serde(default)]
    source: Option<String>,
    #[serde(default)]
    session_id: Option<String>,
    #[serde(default)]
    thread_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct LegacyChatsListQuery {
    #[serde(default)]
    limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct LegacyCreateChatRequest {
    #[serde(default)]
    title: Option<String>,
}

#[derive(Debug, Deserialize)]
struct LocalTestLoginQuery {
    email: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    expires: Option<i64>,
    #[serde(default)]
    signature: Option<String>,
}

pub fn build_router(config: Config) -> Router {
    build_router_with_observability(config, Observability::default())
}

pub fn build_router_with_observability(config: Config, observability: Observability) -> Router {
    let auth = AuthService::from_config(&config);
    let route_split = RouteSplitService::from_config(&config);
    let khala_token_issuer = KhalaTokenIssuer::from_config(&config);
    let sync_token_issuer = SyncTokenIssuer::from_config(&config);
    let codex_thread_store = CodexThreadStore::from_config(&config);
    let domain_store = DomainStore::from_config(&config);
    let runtime_revocation_client = RuntimeRevocationClient::from_config(&config);
    let state = AppState {
        config: Arc::new(config),
        auth,
        observability,
        route_split,
        khala_token_issuer,
        sync_token_issuer,
        codex_thread_store,
        _domain_store: domain_store,
        runtime_revocation_client,
        throttle_state: ThrottleState::default(),
        codex_control_receipts: CodexControlReceiptState::default(),
        runtime_internal_nonces: RuntimeInternalNonceState::default(),
        started_at: SystemTime::now(),
    };
    let compatibility_state = state.clone();
    let maintenance_state = state.clone();
    let auth_email_throttle_state = state.clone();
    let login_email_throttle_state = state.clone();
    let login_verify_throttle_state = state.clone();
    let local_test_login_throttle_state = state.clone();
    let thread_message_throttle_state = state.clone();
    let codex_control_request_throttle_state = state.clone();
    let authenticated_routes_state = state.clone();
    let workos_session_state = state.clone();
    let admin_state = state.clone();

    let web_auth_router = Router::new()
        .route("/login", get(login_page))
        .route(
            "/login/email",
            post(login_email).route_layer(middleware::from_fn_with_state(
                login_email_throttle_state,
                throttle_login_email_gate,
            )),
        )
        .route(
            "/login/verify",
            post(login_verify).route_layer(middleware::from_fn_with_state(
                login_verify_throttle_state,
                throttle_login_verify_gate,
            )),
        )
        .route("/logout", post(web_logout))
        .route(
            "/internal/test-login",
            get(local_test_login).route_layer(middleware::from_fn_with_state(
                local_test_login_throttle_state,
                throttle_auth_email_gate,
            )),
        );

    let public_api_router = Router::new()
        .route(
            ROUTE_AUTH_EMAIL,
            post(send_email_code).route_layer(middleware::from_fn_with_state(
                auth_email_throttle_state,
                throttle_auth_email_gate,
            )),
        )
        .route(
            ROUTE_AUTH_REGISTER,
            post(auth_register).route_layer(middleware::from_fn_with_state(
                state.clone(),
                throttle_auth_email_gate,
            )),
        )
        .route(ROUTE_AUTH_VERIFY, post(verify_email_code))
        .route(ROUTE_AUTH_REFRESH, post(refresh_session));

    let protected_api_router = Router::new()
        .route(ROUTE_AUTH_SESSION, get(current_session))
        .route(ROUTE_AUTH_SESSIONS, get(list_sessions))
        .route(ROUTE_AUTH_SESSIONS_REVOKE, post(revoke_sessions))
        .route(ROUTE_AUTH_LOGOUT, post(logout_session))
        .route(ROUTE_ME, get(me))
        .route(
            ROUTE_AUTOPILOTS,
            get(list_autopilots).post(create_autopilot),
        )
        .route(
            ROUTE_AUTOPILOTS_BY_ID,
            get(show_autopilot).patch(update_autopilot),
        )
        .route(
            ROUTE_AUTOPILOTS_THREADS,
            get(list_autopilot_threads).post(create_autopilot_thread),
        )
        .route(ROUTE_AUTOPILOTS_STREAM, post(autopilot_stream))
        .route(
            ROUTE_TOKENS,
            get(list_personal_access_tokens)
                .post(create_personal_access_token)
                .delete(delete_all_personal_access_tokens),
        )
        .route(
            ROUTE_SETTINGS_PROFILE,
            get(settings_profile_show)
                .patch(settings_profile_update)
                .delete(settings_profile_delete),
        )
        .route(
            ROUTE_TOKENS_CURRENT,
            delete(delete_current_personal_access_token),
        )
        .route(ROUTE_TOKENS_BY_ID, delete(delete_personal_access_token))
        .route(ROUTE_KHALA_TOKEN, post(khala_token))
        .route(ROUTE_ORGS_MEMBERSHIPS, get(org_memberships))
        .route(ROUTE_ORGS_ACTIVE, post(set_active_org))
        .route(ROUTE_POLICY_AUTHORIZE, post(policy_authorize))
        .route(ROUTE_SYNC_TOKEN, post(sync_token))
        .route("/api/chat/stream", post(legacy_chat_stream))
        .route(
            "/api/chats",
            get(legacy_chats_index).post(legacy_chats_store),
        )
        .route("/api/chats/:conversation_id", get(legacy_chats_show))
        .route(
            "/api/chats/:conversation_id/messages",
            get(legacy_chats_messages),
        )
        .route(
            "/api/chats/:conversation_id/stream",
            post(legacy_chats_stream),
        )
        .route("/api/chats/:conversation_id/runs", get(legacy_chats_runs))
        .route(
            "/api/chats/:conversation_id/runs/:run_id/events",
            get(legacy_chats_run_events),
        )
        .route(ROUTE_RUNTIME_THREADS, get(list_runtime_threads))
        .route(
            ROUTE_RUNTIME_THREAD_MESSAGES,
            get(list_runtime_thread_messages)
                .post(send_thread_message)
                .route_layer(middleware::from_fn_with_state(
                    thread_message_throttle_state,
                    throttle_thread_message_gate,
                )),
        )
        .route(
            ROUTE_RUNTIME_CODEX_WORKER_REQUESTS,
            post(runtime_codex_worker_request).route_layer(middleware::from_fn_with_state(
                codex_control_request_throttle_state,
                throttle_codex_control_request_gate,
            )),
        )
        .route(ROUTE_V1_AUTH_SESSION, get(current_session))
        .route(ROUTE_V1_AUTH_SESSIONS, get(list_sessions))
        .route(ROUTE_V1_AUTH_SESSIONS_REVOKE, post(revoke_sessions))
        .route(ROUTE_V1_CONTROL_STATUS, get(control_status))
        .route(ROUTE_V1_CONTROL_ROUTE_SPLIT_STATUS, get(route_split_status))
        .route(
            ROUTE_V1_CONTROL_ROUTE_SPLIT_OVERRIDE,
            post(route_split_override).route_layer(middleware::from_fn_with_state(
                admin_state,
                admin_email_gate,
            )),
        )
        .route(
            ROUTE_V1_CONTROL_ROUTE_SPLIT_EVALUATE,
            post(route_split_evaluate),
        )
        .route(ROUTE_V1_SYNC_TOKEN, post(sync_token))
        .route_layer(middleware::from_fn_with_state(
            workos_session_state,
            workos_session_gate,
        ))
        .route_layer(middleware::from_fn_with_state(
            authenticated_routes_state,
            auth_session_gate,
        ));

    Router::new()
        .route("/", get(web_shell_entry))
        .route("/healthz", get(health))
        .route("/readyz", get(readiness))
        .merge(web_auth_router)
        .merge(public_api_router)
        .merge(protected_api_router)
        .route(ROUTE_OPENAPI_JSON, get(openapi_spec))
        .route("/sw.js", get(static_service_worker))
        .route("/manifest.json", get(static_manifest))
        .route("/assets/*path", get(static_asset))
        .route("/*path", get(web_shell_entry))
        .with_state(state)
        .layer(middleware::from_fn_with_state(
            maintenance_state,
            maintenance_mode_gate,
        ))
        .layer(middleware::from_fn_with_state(
            compatibility_state,
            control_compatibility_gate,
        ))
        .layer(
            ServiceBuilder::new()
                .layer(SetRequestIdLayer::x_request_id(MakeRequestUuid))
                .layer(PropagateRequestIdLayer::x_request_id())
                .layer(TraceLayer::new_for_http()),
        )
}

impl RuntimeRevocationClient {
    fn from_config(config: &Config) -> Option<Self> {
        let base_url = config.runtime_sync_revoke_base_url.as_ref()?;
        let secret = config.runtime_signature_secret.as_ref()?;
        let revoke_path = config.runtime_sync_revoke_path.trim();
        if revoke_path.is_empty() {
            return None;
        }

        let normalized_path = if revoke_path.starts_with('/') {
            revoke_path.to_string()
        } else {
            format!("/{revoke_path}")
        };

        Some(Self {
            endpoint_url: format!("{}{}", base_url.trim_end_matches('/'), normalized_path),
            signature_secret: secret.clone(),
            signature_ttl_seconds: config.runtime_signature_ttl_seconds.max(30),
            http: reqwest::Client::new(),
        })
    }

    async fn revoke_sessions(
        &self,
        session_ids: Vec<String>,
        device_ids: Vec<String>,
        reason: SessionRevocationReason,
    ) -> Result<(), String> {
        if session_ids.is_empty() && device_ids.is_empty() {
            return Ok(());
        }

        let token = self
            .signature_token()
            .map_err(|error| format!("failed to sign runtime revocation token: {error}"))?;

        let response = self
            .http
            .post(&self.endpoint_url)
            .header("x-oa-runtime-signature", token)
            .json(&RuntimeRevocationRequest {
                session_ids,
                device_ids,
                reason: reason.as_str().to_string(),
            })
            .send()
            .await
            .map_err(|error| format!("runtime revocation request failed: {error}"))?;

        if response.status().is_success() {
            return Ok(());
        }

        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        Err(format!("runtime revocation rejected ({status}): {body}"))
    }

    fn signature_token(&self) -> Result<String, anyhow::Error> {
        let now = Utc::now().timestamp().max(0) as u64;
        let payload = serde_json::json!({
            "iat": now,
            "exp": now + self.signature_ttl_seconds,
            "nonce": format!("revk_{}", uuid::Uuid::new_v4().simple()),
        });
        let payload_bytes = serde_json::to_vec(&payload)?;
        let payload_segment = URL_SAFE_NO_PAD.encode(payload_bytes);

        let mut mac = HmacSha256::new_from_slice(self.signature_secret.as_bytes())?;
        mac.update(payload_segment.as_bytes());
        let signature = mac.finalize().into_bytes();
        let signature_segment = URL_SAFE_NO_PAD.encode(signature);

        Ok(format!("v1.{payload_segment}.{signature_segment}"))
    }
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

async fn auth_session_gate(
    State(state): State<AppState>,
    mut request: Request,
    next: Next,
) -> Response {
    match session_bundle_from_headers(&state, request.headers()).await {
        Ok(bundle) => {
            request.extensions_mut().insert(bundle);
            next.run(request).await
        }
        Err(response) => response.into_response(),
    }
}

async fn workos_session_gate(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Response {
    if state.auth.provider_name() != "workos" {
        return next.run(request).await;
    }

    let bundle = if let Some(existing) = request.extensions().get::<SessionBundle>() {
        existing.clone()
    } else {
        match session_bundle_from_headers(&state, request.headers()).await {
            Ok(bundle) => bundle,
            Err(response) => return response.into_response(),
        }
    };

    let workos_user_id = bundle.user.workos_user_id.trim();
    if workos_user_id.is_empty() {
        return unauthorized_error("WorkOS session required.").into_response();
    }

    if workos_user_id.starts_with("test_local_") && !state.config.auth_local_test_login_enabled {
        return unauthorized_error("WorkOS session required.").into_response();
    }

    next.run(request).await
}

async fn admin_email_gate(State(state): State<AppState>, request: Request, next: Next) -> Response {
    let bundle = if let Some(existing) = request.extensions().get::<SessionBundle>() {
        existing.clone()
    } else {
        match session_bundle_from_headers(&state, request.headers()).await {
            Ok(bundle) => bundle,
            Err(response) => return response.into_response(),
        }
    };

    if !is_admin_email(&bundle.user.email, &state.config.admin_emails) {
        return forbidden_error("Forbidden.").into_response();
    }

    next.run(request).await
}

async fn throttle_auth_email_gate(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Response {
    let key = format!("auth.email:{}", request_identity_key(request.headers()));
    match consume_throttle_token(
        &state.throttle_state,
        &key,
        THROTTLE_AUTH_EMAIL_LIMIT,
        THROTTLE_AUTH_EMAIL_WINDOW_SECONDS,
    )
    .await
    {
        Ok(()) => next.run(request).await,
        Err(retry_after_seconds) => error_response_with_status(
            StatusCode::TOO_MANY_REQUESTS,
            ApiErrorCode::RateLimited,
            format!("Too many requests. Retry in {retry_after_seconds}s."),
        )
        .into_response(),
    }
}

async fn throttle_login_email_gate(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Response {
    let key = format!("auth.web.email:{}", request_identity_key(request.headers()));
    match consume_throttle_token(
        &state.throttle_state,
        &key,
        THROTTLE_LOGIN_EMAIL_LIMIT,
        THROTTLE_LOGIN_EMAIL_WINDOW_SECONDS,
    )
    .await
    {
        Ok(()) => next.run(request).await,
        Err(retry_after_seconds) => error_response_with_status(
            StatusCode::TOO_MANY_REQUESTS,
            ApiErrorCode::RateLimited,
            format!("Too many requests. Retry in {retry_after_seconds}s."),
        )
        .into_response(),
    }
}

async fn throttle_login_verify_gate(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Response {
    let key = format!(
        "auth.web.verify:{}",
        request_identity_key(request.headers())
    );
    match consume_throttle_token(
        &state.throttle_state,
        &key,
        THROTTLE_LOGIN_VERIFY_LIMIT,
        THROTTLE_LOGIN_VERIFY_WINDOW_SECONDS,
    )
    .await
    {
        Ok(()) => next.run(request).await,
        Err(retry_after_seconds) => error_response_with_status(
            StatusCode::TOO_MANY_REQUESTS,
            ApiErrorCode::RateLimited,
            format!("Too many requests. Retry in {retry_after_seconds}s."),
        )
        .into_response(),
    }
}

async fn throttle_thread_message_gate(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Response {
    if request.method() != Method::POST {
        return next.run(request).await;
    }

    let key = format!(
        "runtime.thread.message:{}",
        request_identity_key(request.headers())
    );
    match consume_throttle_token(
        &state.throttle_state,
        &key,
        THROTTLE_THREAD_MESSAGE_LIMIT,
        THROTTLE_THREAD_MESSAGE_WINDOW_SECONDS,
    )
    .await
    {
        Ok(()) => next.run(request).await,
        Err(retry_after_seconds) => error_response_with_status(
            StatusCode::TOO_MANY_REQUESTS,
            ApiErrorCode::RateLimited,
            format!("Too many requests. Retry in {retry_after_seconds}s."),
        )
        .into_response(),
    }
}

async fn throttle_codex_control_request_gate(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Response {
    if request.method() != Method::POST {
        return next.run(request).await;
    }

    let key = format!(
        "runtime.codex.control.request:{}",
        request_identity_key(request.headers())
    );
    match consume_throttle_token(
        &state.throttle_state,
        &key,
        THROTTLE_CODEX_CONTROL_REQUEST_LIMIT,
        THROTTLE_CODEX_CONTROL_REQUEST_WINDOW_SECONDS,
    )
    .await
    {
        Ok(()) => next.run(request).await,
        Err(retry_after_seconds) => error_response_with_status(
            StatusCode::TOO_MANY_REQUESTS,
            ApiErrorCode::RateLimited,
            format!("Too many requests. Retry in {retry_after_seconds}s."),
        )
        .into_response(),
    }
}

#[allow(dead_code)]
async fn runtime_internal_request_gate(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Response {
    let (parts, body) = request.into_parts();
    let body_bytes = match to_bytes(body, RUNTIME_INTERNAL_MAX_BODY_BYTES).await {
        Ok(bytes) => bytes,
        Err(_) => {
            return error_response_with_status(
                StatusCode::UNAUTHORIZED,
                ApiErrorCode::Unauthorized,
                "Invalid runtime internal request body.".to_string(),
            )
            .into_response();
        }
    };

    if let Err(response) =
        verify_runtime_internal_headers(&state, &parts.headers, &body_bytes).await
    {
        return response.into_response();
    }

    let request = Request::from_parts(parts, Body::from(body_bytes));
    next.run(request).await
}

async fn verify_runtime_internal_headers(
    state: &AppState,
    headers: &HeaderMap,
    body: &[u8],
) -> Result<(), (StatusCode, Json<ApiErrorResponse>)> {
    let secret = state
        .config
        .runtime_internal_shared_secret
        .as_ref()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
            error_response_with_status(
                StatusCode::INTERNAL_SERVER_ERROR,
                ApiErrorCode::InternalError,
                "Runtime internal auth is misconfigured.".to_string(),
            )
        })?;

    let provided_key_id =
        header_string(headers, RUNTIME_INTERNAL_KEY_ID_HEADER).unwrap_or_default();
    if provided_key_id.is_empty() || provided_key_id != state.config.runtime_internal_key_id {
        return Err(unauthorized_error("Runtime internal key id is invalid."));
    }

    let timestamp = header_string(headers, RUNTIME_INTERNAL_TIMESTAMP_HEADER).unwrap_or_default();
    let nonce = header_string(headers, RUNTIME_INTERNAL_NONCE_HEADER).unwrap_or_default();
    let provided_body_hash =
        header_string(headers, RUNTIME_INTERNAL_BODY_HASH_HEADER).unwrap_or_default();
    let provided_signature =
        header_string(headers, RUNTIME_INTERNAL_SIGNATURE_HEADER).unwrap_or_default();

    if timestamp.is_empty()
        || nonce.is_empty()
        || provided_body_hash.is_empty()
        || provided_signature.is_empty()
    {
        return Err(unauthorized_error(
            "Runtime internal auth headers are missing.",
        ));
    }

    let timestamp_epoch = timestamp
        .parse::<i64>()
        .map_err(|_| unauthorized_error("Runtime internal timestamp is invalid."))?;

    let now_epoch = Utc::now().timestamp();
    let ttl_seconds = state.config.runtime_internal_signature_ttl_seconds as i64;
    if (now_epoch - timestamp_epoch).abs() > ttl_seconds {
        return Err(unauthorized_error("Runtime internal signature expired."));
    }

    let computed_body_hash = sha256_hex(body);
    if computed_body_hash != provided_body_hash {
        return Err(unauthorized_error("Runtime internal body hash mismatch."));
    }

    let payload = format!("{timestamp}\n{nonce}\n{computed_body_hash}");
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).map_err(|_| {
        error_response_with_status(
            StatusCode::INTERNAL_SERVER_ERROR,
            ApiErrorCode::InternalError,
            "Runtime internal auth is misconfigured.".to_string(),
        )
    })?;
    mac.update(payload.as_bytes());
    let expected_signature = sha256_bytes_hex(&mac.finalize().into_bytes());
    if expected_signature != provided_signature {
        return Err(unauthorized_error("Runtime internal signature is invalid."));
    }

    let replay_key = format!("{provided_key_id}:{nonce}");
    let expires_at = timestamp_epoch + ttl_seconds + RUNTIME_INTERNAL_NONCE_GRACE_SECONDS;
    let mut entries = state.runtime_internal_nonces.entries.lock().await;
    entries.retain(|_, expiry| *expiry > now_epoch);
    if entries.contains_key(&replay_key) {
        return Err(unauthorized_error(
            "Runtime internal nonce replay detected.",
        ));
    }
    entries.insert(replay_key, expires_at);

    Ok(())
}

async fn consume_throttle_token(
    throttle_state: &ThrottleState,
    bucket_key: &str,
    max_requests: usize,
    window_seconds: i64,
) -> Result<(), i64> {
    let now_epoch = Utc::now().timestamp();
    let window_start = now_epoch - window_seconds;

    let mut buckets = throttle_state.buckets.lock().await;
    let bucket = buckets.entry(bucket_key.to_string()).or_default();

    while let Some(oldest) = bucket.front() {
        if *oldest < window_start {
            let _ = bucket.pop_front();
        } else {
            break;
        }
    }

    if bucket.len() >= max_requests {
        let retry_after = bucket
            .front()
            .map(|oldest| ((*oldest + window_seconds) - now_epoch).max(1))
            .unwrap_or(1);
        return Err(retry_after);
    }

    bucket.push_back(now_epoch);
    Ok(())
}

async fn session_bundle_from_headers(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<SessionBundle, (StatusCode, Json<ApiErrorResponse>)> {
    let access_token =
        access_token_from_headers(headers).ok_or_else(|| unauthorized_error("Unauthenticated."))?;
    state
        .auth
        .session_or_pat_from_access_token(&access_token)
        .await
        .map_err(map_auth_error)
}

fn request_identity_key(headers: &HeaderMap) -> String {
    if let Some(access_token) = bearer_token(headers) {
        return format!("token:{access_token}");
    }

    if let Some(value) = header_string(headers, HEADER_X_FORWARDED_FOR) {
        let first_ip = value.split(',').next().unwrap_or_default().trim();
        if !first_ip.is_empty() {
            return format!("ip:{first_ip}");
        }
    }

    if let Some(value) = header_string(headers, HEADER_X_REAL_IP) {
        let ip = value.trim();
        if !ip.is_empty() {
            return format!("ip:{ip}");
        }
    }

    "ip:unknown".to_string()
}

fn is_admin_email(email: &str, admin_emails: &[String]) -> bool {
    let normalized_email = email.trim().to_lowercase();
    if normalized_email.is_empty() {
        return false;
    }

    admin_emails.iter().any(|configured| {
        let candidate = configured.trim().to_lowercase();
        !candidate.is_empty() && candidate == normalized_email
    })
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    sha256_bytes_hex(&digest)
}

fn sha256_bytes_hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write as _;
        let _ = write!(&mut output, "{byte:02x}");
    }
    output
}

async fn maintenance_mode_gate(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Response {
    if !state.config.maintenance_mode_enabled {
        return next.run(request).await;
    }

    let path = request.uri().path().to_string();
    if maintenance_path_is_allowed(&path, &state.config.maintenance_allowed_paths) {
        return next.run(request).await;
    }

    if let Some(bypass_token) = state.config.maintenance_bypass_token.as_ref() {
        if let Some(candidate) =
            query_param_value(request.uri().query(), MAINTENANCE_BYPASS_QUERY_PARAM)
        {
            if candidate == *bypass_token {
                let now = Utc::now().timestamp().max(0) as u64;
                let expires_at = now + state.config.maintenance_bypass_cookie_ttl_seconds;
                if let Some(cookie_payload) =
                    maintenance_bypass_cookie_payload(bypass_token, expires_at)
                {
                    let mut response =
                        Redirect::temporary(&maintenance_redirect_location(request.uri()))
                            .into_response();
                    if let Ok(value) = HeaderValue::from_str(&maintenance_bypass_cookie(
                        &state.config.maintenance_bypass_cookie_name,
                        &cookie_payload,
                        state.config.maintenance_bypass_cookie_ttl_seconds,
                    )) {
                        response.headers_mut().insert(SET_COOKIE, value);
                    }
                    return response;
                }
            }
        }

        if let Some(cookie) = extract_cookie_value(
            request.headers(),
            &state.config.maintenance_bypass_cookie_name,
        ) {
            if maintenance_cookie_is_valid(&cookie, bypass_token) {
                return next.run(request).await;
            }
        }
    }

    maintenance_response()
}

async fn control_compatibility_gate(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Response {
    let path = request.uri().path().to_string();
    let Some(surface) = compatibility_surface_for_path(&path) else {
        return next.run(request).await;
    };
    if !state.config.compat_control_enforced {
        return next.run(request).await;
    }

    let header_snapshot = request.headers().clone();
    let request_id = request_id(&header_snapshot);

    match validate_control_compatibility(surface, &state.config, &header_snapshot) {
        Ok(()) => next.run(request).await,
        Err(failure) => {
            let client_name = header_string(&header_snapshot, "x-client")
                .unwrap_or_else(|| "unknown".to_string());
            let client_build_id = header_string(&header_snapshot, HEADER_OA_CLIENT_BUILD_ID)
                .unwrap_or_else(|| "missing".to_string());

            state
                .observability
                .increment_counter("compatibility.rejected.control", &request_id);
            state.observability.increment_counter(
                &format!("compatibility.rejected.control.{}", failure.code),
                &request_id,
            );
            state.observability.audit(
                AuditEvent::new("compatibility.rejected", request_id.clone())
                    .with_outcome("rejected")
                    .with_attribute("surface", compatibility_surface_label(surface))
                    .with_attribute("path", path)
                    .with_attribute("client", client_name)
                    .with_attribute("client_build_id", client_build_id)
                    .with_attribute("code", failure.code.clone())
                    .with_attribute("protocol_version", failure.protocol_version.clone()),
            );

            compatibility_failure_response(failure)
        }
    }
}

fn compatibility_surface_for_path(path: &str) -> Option<CompatibilitySurface> {
    if path.starts_with("/api/v1/control/") {
        return Some(CompatibilitySurface::ControlApi);
    }

    if path == ROUTE_V1_SYNC_TOKEN {
        return Some(CompatibilitySurface::ControlApi);
    }

    None
}

fn compatibility_surface_label(surface: CompatibilitySurface) -> &'static str {
    match surface {
        CompatibilitySurface::ControlApi => "control_api",
        CompatibilitySurface::KhalaWebSocket => "khala_websocket",
    }
}

fn validate_control_compatibility(
    surface: CompatibilitySurface,
    config: &Config,
    headers: &HeaderMap,
) -> Result<(), CompatibilityFailure> {
    let schema_version = header_string(headers, HEADER_OA_SCHEMA_VERSION)
        .and_then(|value| value.parse::<u32>().ok())
        .unwrap_or(0);

    let handshake = ClientCompatibilityHandshake {
        client_build_id: header_string(headers, HEADER_OA_CLIENT_BUILD_ID).unwrap_or_default(),
        protocol_version: header_string(headers, HEADER_OA_PROTOCOL_VERSION).unwrap_or_default(),
        schema_version,
    };

    let window = CompatibilityWindow {
        protocol_version: config.compat_control_protocol_version.clone(),
        min_client_build_id: config.compat_control_min_client_build_id.clone(),
        max_client_build_id: config.compat_control_max_client_build_id.clone(),
        min_schema_version: config.compat_control_min_schema_version,
        max_schema_version: config.compat_control_max_schema_version,
    };

    negotiate_compatibility(surface, &handshake, &window)
}

fn compatibility_failure_response(failure: CompatibilityFailure) -> Response {
    let code = failure.code.clone();
    let protocol_version = failure.protocol_version.clone();
    let min_client_build_id = failure.min_client_build_id.clone();
    let max_client_build_id = failure.max_client_build_id.clone();
    let min_schema_version = failure.min_schema_version;
    let max_schema_version = failure.max_schema_version;
    let upgrade_required = failure.upgrade_required;
    let message = failure.message.clone();
    let mut response = (
        StatusCode::UPGRADE_REQUIRED,
        Json(CompatibilityErrorResponse {
            message: message.clone(),
            error: CompatibilityErrorDetail {
                code: failure.code.clone(),
                message,
            },
            compatibility: failure,
        }),
    )
        .into_response();

    response
        .headers_mut()
        .insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response.headers_mut().insert(
        HEADER_OA_COMPAT_UPGRADE_REQUIRED,
        HeaderValue::from_static(if upgrade_required { "true" } else { "false" }),
    );
    if let Ok(value) = HeaderValue::from_str(&code) {
        response.headers_mut().insert(HEADER_OA_COMPAT_CODE, value);
    }
    if let Ok(value) = HeaderValue::from_str(&protocol_version) {
        response
            .headers_mut()
            .insert(HEADER_OA_COMPAT_PROTOCOL, value);
    }
    if let Ok(value) = HeaderValue::from_str(&min_client_build_id) {
        response
            .headers_mut()
            .insert(HEADER_OA_COMPAT_MIN_BUILD, value);
    }
    if let Some(max_client_build_id) = max_client_build_id
        && let Ok(value) = HeaderValue::from_str(&max_client_build_id)
    {
        response
            .headers_mut()
            .insert(HEADER_OA_COMPAT_MAX_BUILD, value);
    }
    if let Ok(value) = HeaderValue::from_str(&min_schema_version.to_string()) {
        response
            .headers_mut()
            .insert(HEADER_OA_COMPAT_MIN_SCHEMA, value);
    }
    if let Ok(value) = HeaderValue::from_str(&max_schema_version.to_string()) {
        response
            .headers_mut()
            .insert(HEADER_OA_COMPAT_MAX_SCHEMA, value);
    }

    response
}

async fn openapi_spec() -> Result<impl IntoResponse, (StatusCode, Json<ApiErrorResponse>)> {
    let document = openapi_document();
    let encoded = serde_json::to_vec(&document).map_err(|_| {
        error_response_with_status(
            StatusCode::INTERNAL_SERVER_ERROR,
            ApiErrorCode::InternalError,
            "Failed to generate OpenAPI document.".to_string(),
        )
    })?;

    let mut response = Response::new(Body::from(encoded));
    *response.status_mut() = StatusCode::OK;
    response
        .headers_mut()
        .insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    response
        .headers_mut()
        .insert(CACHE_CONTROL, HeaderValue::from_static(CACHE_MANIFEST));

    Ok(response)
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

async fn static_service_worker(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiErrorResponse>)> {
    let service_worker_path = state.config.static_dir.join("sw.js");
    let response = build_static_response(&service_worker_path, CACHE_MANIFEST)
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
    if is_retired_web_route(path) {
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
            route_domain: "chat_pilot".to_string(),
            rollback_target: Some(RouteTarget::RustShell),
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
                    error_response_with_status(
                        StatusCode::SERVICE_UNAVAILABLE,
                        ApiErrorCode::LegacyRouteUnavailable,
                        "Legacy route target is not configured.".to_string(),
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
    Ok(ok_data(status))
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
    let normalized_domain = payload
        .domain
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_lowercase());

    match normalized_target.as_str() {
        "legacy" => {
            if let Some(domain) = normalized_domain.as_deref() {
                state
                    .route_split
                    .set_domain_override_target(domain, Some(RouteTarget::Legacy))
                    .await
                    .map_err(|message| validation_error("domain", &message))?;
            } else {
                state
                    .route_split
                    .set_override_target(Some(RouteTarget::Legacy))
                    .await;
            }
        }
        "rust" | "rust_shell" => {
            if let Some(domain) = normalized_domain.as_deref() {
                state
                    .route_split
                    .set_domain_override_target(domain, Some(RouteTarget::RustShell))
                    .await
                    .map_err(|message| validation_error("domain", &message))?;
            } else {
                state
                    .route_split
                    .set_override_target(Some(RouteTarget::RustShell))
                    .await;
            }
        }
        "clear" | "default" => {
            if let Some(domain) = normalized_domain.as_deref() {
                state
                    .route_split
                    .set_domain_override_target(domain, None)
                    .await
                    .map_err(|message| validation_error("domain", &message))?;
            } else {
                state.route_split.set_override_target(None).await;
            }
        }
        "rollback" => {
            if let Some(domain) = normalized_domain.as_deref() {
                let rollback_target = state
                    .route_split
                    .rollback_target_for_domain(Some(domain))
                    .ok_or_else(|| validation_error("domain", "Unknown route domain."))?;
                state
                    .route_split
                    .set_domain_override_target(domain, Some(rollback_target))
                    .await
                    .map_err(|message| validation_error("domain", &message))?;
            } else if let Some(global_target) = state.route_split.rollback_target_for_domain(None) {
                state
                    .route_split
                    .set_override_target(Some(global_target))
                    .await;
            } else {
                state
                    .route_split
                    .set_override_target(Some(RouteTarget::Legacy))
                    .await;
            }
        }
        _ => {
            return Err(validation_error(
                "target",
                "Target must be one of: legacy, rust, rollback, clear.",
            ));
        }
    }

    let status = state.route_split.status().await;
    let scope = normalized_domain
        .clone()
        .map(|domain| format!("domain:{domain}"))
        .unwrap_or_else(|| "global".to_string());

    state.observability.audit(
        AuditEvent::new("route.split.override.updated", request_id.clone())
            .with_user_id(session.user.id)
            .with_session_id(session.session.session_id)
            .with_org_id(session.session.active_org_id)
            .with_device_id(session.session.device_id)
            .with_attribute("target", normalized_target)
            .with_attribute("scope", scope),
    );
    state
        .observability
        .increment_counter("route.split.override.updated", &request_id);

    Ok(ok_data(status))
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
    Ok(ok_data(decision))
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
        StaticResponseError::Io(_) | StaticResponseError::InvalidHeader(_) => {
            error_response_with_status(
                StatusCode::INTERNAL_SERVER_ERROR,
                ApiErrorCode::StaticAssetError,
                "Failed to serve static asset.".to_string(),
            )
        }
    }
}

fn static_not_found(message: String) -> (StatusCode, Json<ApiErrorResponse>) {
    not_found_error(message)
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

async fn login_page(
    State(state): State<AppState>,
    headers: HeaderMap,
    uri: axum::http::Uri,
) -> Result<Response, (StatusCode, Json<ApiErrorResponse>)> {
    if session_bundle_from_headers(&state, &headers).await.is_ok() {
        return Ok(Redirect::temporary("/").into_response());
    }

    web_shell_entry(State(state), headers, uri)
        .await
        .map(IntoResponse::into_response)
}

async fn login_email(
    State(state): State<AppState>,
    headers: HeaderMap,
    Form(payload): Form<LoginEmailForm>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiErrorResponse>)> {
    if session_bundle_from_headers(&state, &headers).await.is_ok() {
        return Ok(Redirect::temporary("/").into_response());
    }

    let request_id = request_id(&headers);
    let challenge = state
        .auth
        .start_challenge(payload.email)
        .await
        .map_err(map_auth_error)?;

    state.observability.audit(
        AuditEvent::new("auth.web.challenge.requested", request_id.clone())
            .with_attribute("provider", state.auth.provider_name())
            .with_attribute(
                "email_domain",
                email_domain(&challenge.email).unwrap_or_else(|| "unknown".to_string()),
            ),
    );
    state
        .observability
        .increment_counter("auth.web.challenge.requested", &request_id);

    let cookie = challenge_cookie(
        &challenge.challenge_id,
        state.config.auth_challenge_ttl_seconds,
    );

    let mut response = Redirect::temporary("/login?status=code-sent").into_response();
    append_set_cookie_header(&mut response, &cookie)?;
    Ok(response)
}

async fn login_verify(
    State(state): State<AppState>,
    headers: HeaderMap,
    Form(payload): Form<LoginVerifyForm>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiErrorResponse>)> {
    if session_bundle_from_headers(&state, &headers).await.is_ok() {
        return Ok(Redirect::temporary("/").into_response());
    }

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

    let ip_address = header_string(&headers, HEADER_X_FORWARDED_FOR).unwrap_or_default();
    let user_agent = header_string(&headers, "user-agent").unwrap_or_default();
    let verified = state
        .auth
        .verify_challenge(
            &challenge_id,
            payload.code,
            Some("openagents-web"),
            header_string(&headers, "x-device-id").as_deref(),
            &ip_address,
            &user_agent,
        )
        .await
        .map_err(map_auth_error)?;

    state.observability.audit(
        AuditEvent::new("auth.web.verify.completed", request_id.clone())
            .with_user_id(verified.user.id.clone())
            .with_session_id(verified.session.session_id.clone())
            .with_org_id(verified.session.active_org_id.clone())
            .with_device_id(verified.session.device_id.clone())
            .with_attribute("provider", state.auth.provider_name())
            .with_attribute("new_user", verified.new_user.to_string()),
    );
    state
        .observability
        .increment_counter("auth.web.verify.completed", &request_id);

    let mut response = Redirect::temporary("/").into_response();
    append_set_cookie_header(
        &mut response,
        &auth_access_cookie(&verified.access_token, state.config.auth_access_ttl_seconds),
    )?;
    append_set_cookie_header(
        &mut response,
        &auth_refresh_cookie(
            &verified.refresh_token,
            state.config.auth_refresh_ttl_seconds,
        ),
    )?;
    append_set_cookie_header(&mut response, &clear_cookie(CHALLENGE_COOKIE_NAME))?;
    Ok(response)
}

async fn web_logout(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiErrorResponse>)> {
    let request_id = request_id(&headers);
    let access_token = access_token_from_headers(&headers);

    if let Some(token) = access_token {
        match state.auth.revoke_session_by_access_token(&token).await {
            Ok(revoked) => {
                propagate_runtime_revocation(
                    &state,
                    &request_id,
                    vec![revoked.session_id],
                    Vec::new(),
                    SessionRevocationReason::UserRequested,
                )
                .await?;
            }
            Err(AuthError::Unauthorized { .. }) => {}
            Err(error) => return Err(map_auth_error(error)),
        }
    }

    let mut response = Redirect::temporary("/").into_response();
    append_set_cookie_header(&mut response, &clear_cookie(AUTH_ACCESS_COOKIE_NAME))?;
    append_set_cookie_header(&mut response, &clear_cookie(AUTH_REFRESH_COOKIE_NAME))?;
    append_set_cookie_header(&mut response, &clear_cookie(LOCAL_TEST_AUTH_COOKIE_NAME))?;
    append_set_cookie_header(&mut response, &clear_cookie(CHALLENGE_COOKIE_NAME))?;
    Ok(response)
}

async fn local_test_login(
    State(state): State<AppState>,
    headers: HeaderMap,
    uri: axum::http::Uri,
    Query(payload): Query<LocalTestLoginQuery>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiErrorResponse>)> {
    if session_bundle_from_headers(&state, &headers).await.is_ok() {
        return Ok(Redirect::temporary("/").into_response());
    }

    if !state.config.auth_local_test_login_enabled {
        return Err(not_found_error("Not found."));
    }

    let signing_key = state
        .config
        .auth_local_test_login_signing_key
        .as_deref()
        .ok_or_else(|| not_found_error("Not found."))?;
    let signature = payload
        .signature
        .and_then(non_empty)
        .ok_or_else(|| forbidden_error("Invalid signature."))?;
    if signature.is_empty() {
        return Err(forbidden_error("Invalid signature."));
    }
    let expires = payload.expires.unwrap_or_default();
    if expires <= Utc::now().timestamp() {
        return Err(forbidden_error("Invalid signature."));
    }

    if !local_test_login_signature_is_valid(&uri, signing_key) {
        return Err(forbidden_error("Invalid signature."));
    }

    let email = non_empty(payload.email)
        .ok_or_else(|| validation_error("email", "Invalid email."))?
        .to_lowercase();
    if !local_test_login_email_allowed(&email, &state.config.auth_local_test_login_allowed_emails) {
        return Err(forbidden_error("Forbidden."));
    }

    let verified = state
        .auth
        .local_test_sign_in(
            email,
            payload.name.and_then(non_empty),
            Some("openagents-web"),
            header_string(&headers, "x-device-id").as_deref(),
        )
        .await
        .map_err(map_auth_error)?;

    let mut response = Redirect::temporary("/").into_response();
    append_set_cookie_header(
        &mut response,
        &auth_access_cookie(&verified.access_token, state.config.auth_access_ttl_seconds),
    )?;
    append_set_cookie_header(
        &mut response,
        &auth_refresh_cookie(
            &verified.refresh_token,
            state.config.auth_refresh_ttl_seconds,
        ),
    )?;
    append_set_cookie_header(
        &mut response,
        &local_test_auth_cookie(state.config.auth_refresh_ttl_seconds),
    )?;
    Ok(response)
}

async fn send_email_code(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<SendEmailCodeRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiErrorResponse>)> {
    let request_id = request_id(&headers);
    let challenge = match state.auth.start_challenge(payload.email).await {
        Ok(challenge) => challenge,
        Err(error) => {
            emit_auth_failure_event(&state, &request_id, "auth.challenge.failed", &error);
            return Err(map_auth_error(error));
        }
    };

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

async fn auth_register(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<AuthRegisterRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiErrorResponse>)> {
    let request_id = request_id(&headers);

    if !auth_api_signup_is_enabled(&state.config) {
        return Err(not_found_error("Not found."));
    }

    let email =
        non_empty(payload.email).ok_or_else(|| validation_error("email", "Email is required."))?;
    let email = email.to_lowercase();

    if let Some(name) = payload
        .name
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        if name.chars().count() > 120 {
            return Err(validation_error(
                "name",
                "Name may not be greater than 120 characters.",
            ));
        }
    }

    if let Some(domain) = email_domain(&email).map(|value| value.to_lowercase()) {
        if !state.config.auth_api_signup_allowed_domains.is_empty()
            && !state
                .config
                .auth_api_signup_allowed_domains
                .iter()
                .any(|allowed| allowed == &domain)
        {
            return Err(validation_error(
                "email",
                "Email domain is not allowed for API signup in this environment.",
            ));
        }
    }

    let token_name = normalize_register_token_name(
        payload.token_name,
        &state.config.auth_api_signup_default_token_name,
    )?;
    let token_abilities = normalize_register_token_abilities(payload.token_abilities)?;
    let create_autopilot = payload.create_autopilot.unwrap_or(false);
    let autopilot_display_name =
        normalize_optional_display_name(payload.autopilot_display_name, "autopilotDisplayName")?;
    let requested_name = normalize_optional_display_name(payload.name, "name")?;

    let registered = state
        .auth
        .register_api_user(email, requested_name)
        .await
        .map_err(map_auth_error)?;

    let issued_token = state
        .auth
        .issue_personal_access_token(
            &registered.user.id,
            token_name.clone(),
            token_abilities.clone(),
            None,
        )
        .await
        .map_err(map_auth_error)?;

    let autopilot_payload = if create_autopilot {
        let autopilot_display = autopilot_display_name
            .clone()
            .unwrap_or_else(|| "Autopilot".to_string());
        let autopilot = state
            ._domain_store
            .create_autopilot(CreateAutopilotInput {
                owner_user_id: registered.user.id.clone(),
                owner_display_name: registered.user.name.clone(),
                display_name: autopilot_display,
                handle_seed: None,
                avatar: None,
                status: None,
                visibility: None,
                tagline: None,
            })
            .await
            .map_err(map_domain_store_error)?;
        Some(serde_json::json!({
            "id": autopilot.autopilot.id,
            "handle": autopilot.autopilot.handle,
            "displayName": autopilot.autopilot.display_name,
            "status": autopilot.autopilot.status,
            "visibility": autopilot.autopilot.visibility,
        }))
    } else {
        None
    };

    state.observability.audit(
        AuditEvent::new("auth.register.completed", request_id.clone())
            .with_user_id(registered.user.id.clone())
            .with_attribute("created", registered.created.to_string())
            .with_attribute("token_name", token_name.clone())
            .with_attribute("autopilot_created", create_autopilot.to_string())
            .with_attribute(
                "email_domain",
                email_domain(&registered.user.email).unwrap_or_else(|| "unknown".to_string()),
            ),
    );
    state
        .observability
        .increment_counter("auth.register.completed", &request_id);

    let status = if registered.created {
        StatusCode::CREATED
    } else {
        StatusCode::OK
    };
    let response = serde_json::json!({
        "data": {
            "created": registered.created,
            "tokenType": "Bearer",
            "token": issued_token.plain_text_token,
            "tokenName": token_name,
            "tokenAbilities": token_abilities,
            "user": {
                "id": registered.user.id,
                "name": registered.user.name,
                "email": registered.user.email,
                "handle": user_handle_from_email(&registered.user.email),
            },
            "autopilot": autopilot_payload,
        }
    });

    Ok((status, Json(response)))
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
    let device_id = payload
        .device_id
        .or_else(|| header_string(&headers, "x-device-id"));
    let ip_address = header_string(&headers, "x-forwarded-for").unwrap_or_default();
    let user_agent = header_string(&headers, "user-agent").unwrap_or_default();

    let verified = match state
        .auth
        .verify_challenge(
            &challenge_id,
            payload.code,
            client_name.as_deref(),
            device_id.as_deref(),
            &ip_address,
            &user_agent,
        )
        .await
    {
        Ok(verified) => verified,
        Err(error) => {
            emit_auth_failure_event(&state, &request_id, "auth.verify.failed", &error);
            return Err(map_auth_error(error));
        }
    };

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

async fn list_sessions(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<ListSessionsRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiErrorResponse>)> {
    let request_id = request_id(&headers);
    let access_token =
        bearer_token(&headers).ok_or_else(|| unauthorized_error("Unauthenticated."))?;

    let bundle = state
        .auth
        .session_from_access_token(&access_token)
        .await
        .map_err(map_auth_error)?;

    let sessions = state
        .auth
        .list_user_sessions(&bundle.user.id, query.device_id.as_deref())
        .await
        .map_err(map_auth_error)?;

    state.observability.audit(
        AuditEvent::new("auth.sessions.listed", request_id.clone())
            .with_user_id(bundle.user.id)
            .with_session_id(bundle.session.session_id.clone())
            .with_org_id(bundle.session.active_org_id)
            .with_device_id(bundle.session.device_id)
            .with_attribute("session_count", sessions.len().to_string()),
    );
    state
        .observability
        .increment_counter("auth.sessions.listed", &request_id);

    Ok((
        StatusCode::OK,
        Json(serde_json::json!({
            "data": {
                "currentSessionId": bundle.session.session_id,
                "sessions": sessions,
            }
        })),
    ))
}

async fn revoke_sessions(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<RevokeSessionsRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiErrorResponse>)> {
    let request_id = request_id(&headers);
    let access_token =
        bearer_token(&headers).ok_or_else(|| unauthorized_error("Unauthenticated."))?;

    let bundle = state
        .auth
        .session_from_access_token(&access_token)
        .await
        .map_err(map_auth_error)?;

    let target = resolve_session_revocation_target(&payload)?;
    let reason = payload
        .reason
        .unwrap_or(SessionRevocationReason::UserRequested);
    let include_current = payload.include_current.unwrap_or(false);

    let result = state
        .auth
        .revoke_user_sessions(
            &bundle.user.id,
            &bundle.session.session_id,
            SessionRevocationRequest {
                target,
                include_current,
                reason,
            },
        )
        .await
        .map_err(map_auth_error)?;

    state.observability.audit(
        AuditEvent::new("auth.sessions.revoked", request_id.clone())
            .with_user_id(bundle.user.id)
            .with_session_id(bundle.session.session_id)
            .with_org_id(bundle.session.active_org_id)
            .with_device_id(bundle.session.device_id)
            .with_attribute("reason", revocation_reason_label(reason).to_string())
            .with_attribute(
                "revoked_session_count",
                result.revoked_session_ids.len().to_string(),
            )
            .with_attribute(
                "revoked_refresh_token_count",
                result.revoked_refresh_token_ids.len().to_string(),
            ),
    );
    state
        .observability
        .increment_counter("auth.sessions.revoked", &request_id);

    propagate_runtime_revocation(
        &state,
        &request_id,
        result.revoked_session_ids.clone(),
        Vec::new(),
        reason,
    )
    .await?;

    Ok((
        StatusCode::OK,
        Json(serde_json::json!({
            "ok": true,
            "revokedSessionIds": result.revoked_session_ids,
            "revokedRefreshTokenIds": result.revoked_refresh_token_ids,
            "reason": reason,
            "revokedAt": timestamp(result.revoked_at),
        })),
    ))
}

async fn me(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<MeQuery>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiErrorResponse>)> {
    let access_token = access_token_from_headers(&headers)
        .ok_or_else(|| unauthorized_error("Unauthenticated."))?;

    let bundle = state
        .auth
        .session_or_pat_from_access_token(&access_token)
        .await
        .map_err(map_auth_error)?;

    let chat_limit = query.chat_limit.unwrap_or(50).clamp(1, 200);
    let chat_threads = state
        .codex_thread_store
        .list_threads_for_user(&bundle.user.id, None)
        .await
        .map_err(map_thread_store_error)?
        .into_iter()
        .take(chat_limit)
        .map(|thread| {
            serde_json::json!({
                "id": thread.thread_id,
                "title": thread_title(&thread.thread_id, thread.message_count),
                "updatedAt": timestamp(thread.updated_at),
            })
        })
        .collect::<Vec<_>>();

    let response = serde_json::json!({
        "data": {
            "user": {
                "id": bundle.user.id,
                "email": bundle.user.email,
                "name": bundle.user.name,
                "handle": user_handle_from_email(&bundle.user.email),
                "avatar": "",
                "createdAt": serde_json::Value::Null,
                "updatedAt": serde_json::Value::Null,
                "workosId": bundle.user.workos_user_id,
            },
            "chatThreads": chat_threads,
        }
    });

    Ok((StatusCode::OK, Json(response)))
}

async fn list_autopilots(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<AutopilotListQuery>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiErrorResponse>)> {
    let request_id = request_id(&headers);
    let bundle = session_bundle_from_headers(&state, &headers).await?;

    let limit = query.limit.unwrap_or(100).clamp(1, 200);
    let autopilots = state
        ._domain_store
        .list_autopilots_for_owner(&bundle.user.id, limit)
        .await
        .map_err(map_domain_store_error)?;
    let payload = autopilots
        .iter()
        .map(autopilot_aggregate_payload)
        .collect::<Vec<_>>();

    state.observability.audit(
        AuditEvent::new("autopilot.list_viewed", request_id.clone())
            .with_user_id(bundle.user.id)
            .with_session_id(bundle.session.session_id)
            .with_org_id(bundle.session.active_org_id)
            .with_device_id(bundle.session.device_id)
            .with_attribute("limit", limit.to_string())
            .with_attribute("count", payload.len().to_string()),
    );
    state
        .observability
        .increment_counter("autopilot.list_viewed", &request_id);

    Ok((StatusCode::OK, Json(serde_json::json!({ "data": payload }))))
}

async fn create_autopilot(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateAutopilotRequestPayload>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiErrorResponse>)> {
    let request_id = request_id(&headers);
    let bundle = session_bundle_from_headers(&state, &headers).await?;

    let handle_seed = normalize_autopilot_handle_seed(payload.handle)?;
    let display_name = normalize_optional_display_name(payload.display_name, "displayName")?
        .unwrap_or_else(|| "Autopilot".to_string());
    let avatar = normalize_optional_bounded_string(payload.avatar, "avatar", 255)?;
    let tagline = normalize_optional_bounded_string(payload.tagline, "tagline", 255)?;
    let status = normalize_autopilot_enum(
        payload.status,
        "status",
        &["active", "disabled", "archived"],
    )?;
    let visibility = normalize_autopilot_enum(
        payload.visibility,
        "visibility",
        &["private", "discoverable", "public"],
    )?;

    let autopilot = state
        ._domain_store
        .create_autopilot(CreateAutopilotInput {
            owner_user_id: bundle.user.id.clone(),
            owner_display_name: bundle.user.name.clone(),
            display_name,
            handle_seed,
            avatar,
            status,
            visibility,
            tagline,
        })
        .await
        .map_err(map_domain_store_error)?;

    state.observability.audit(
        AuditEvent::new("autopilot.created", request_id.clone())
            .with_user_id(bundle.user.id)
            .with_session_id(bundle.session.session_id)
            .with_org_id(bundle.session.active_org_id)
            .with_device_id(bundle.session.device_id)
            .with_attribute("autopilot_id", autopilot.autopilot.id.clone())
            .with_attribute("handle", autopilot.autopilot.handle.clone())
            .with_attribute("status", autopilot.autopilot.status.clone())
            .with_attribute("visibility", autopilot.autopilot.visibility.clone()),
    );
    state
        .observability
        .increment_counter("autopilot.created", &request_id);

    Ok((
        StatusCode::CREATED,
        Json(serde_json::json!({
            "data": autopilot_aggregate_payload(&autopilot),
        })),
    ))
}

async fn show_autopilot(
    State(state): State<AppState>,
    Path(autopilot): Path<String>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiErrorResponse>)> {
    let request_id = request_id(&headers);
    let bundle = session_bundle_from_headers(&state, &headers).await?;
    let reference = normalize_autopilot_reference(autopilot)?;

    let autopilot = state
        ._domain_store
        .resolve_owned_autopilot(&bundle.user.id, &reference)
        .await
        .map_err(map_domain_store_error)?;

    state.observability.audit(
        AuditEvent::new("autopilot.viewed", request_id.clone())
            .with_user_id(bundle.user.id)
            .with_session_id(bundle.session.session_id)
            .with_org_id(bundle.session.active_org_id)
            .with_device_id(bundle.session.device_id)
            .with_attribute("autopilot_id", autopilot.autopilot.id.clone())
            .with_attribute("handle", autopilot.autopilot.handle.clone()),
    );
    state
        .observability
        .increment_counter("autopilot.viewed", &request_id);

    Ok((
        StatusCode::OK,
        Json(serde_json::json!({
            "data": autopilot_aggregate_payload(&autopilot),
        })),
    ))
}

async fn update_autopilot(
    State(state): State<AppState>,
    Path(autopilot): Path<String>,
    headers: HeaderMap,
    Json(payload): Json<UpdateAutopilotRequestPayload>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiErrorResponse>)> {
    let request_id = request_id(&headers);
    let bundle = session_bundle_from_headers(&state, &headers).await?;
    let reference = normalize_autopilot_reference(autopilot)?;
    let field_count = autopilot_update_field_count(
        payload.display_name.is_some(),
        payload.status.is_some(),
        payload.visibility.is_some(),
        payload.avatar.is_some(),
        payload.tagline.is_some(),
        payload.profile.is_some(),
        payload.policy.is_some(),
    );

    let display_name = normalize_optional_display_name(payload.display_name, "displayName")?;
    let status = normalize_autopilot_enum(
        payload.status,
        "status",
        &["active", "disabled", "archived"],
    )?;
    let visibility = normalize_autopilot_enum(
        payload.visibility,
        "visibility",
        &["private", "discoverable", "public"],
    )?;
    let avatar = normalize_optional_bounded_string(payload.avatar, "avatar", 255)?;
    let tagline = normalize_optional_bounded_string(payload.tagline, "tagline", 255)?;
    let profile = payload
        .profile
        .map(normalize_autopilot_profile_update)
        .transpose()?;
    let policy = payload
        .policy
        .map(normalize_autopilot_policy_update)
        .transpose()?;

    let updated = state
        ._domain_store
        .update_owned_autopilot(
            &bundle.user.id,
            &reference,
            UpdateAutopilotInput {
                display_name,
                avatar,
                status,
                visibility,
                tagline,
                profile,
                policy,
            },
        )
        .await
        .map_err(map_domain_store_error)?;

    state.observability.audit(
        AuditEvent::new("autopilot.updated", request_id.clone())
            .with_user_id(bundle.user.id)
            .with_session_id(bundle.session.session_id)
            .with_org_id(bundle.session.active_org_id)
            .with_device_id(bundle.session.device_id)
            .with_attribute("autopilot_id", updated.autopilot.id.clone())
            .with_attribute("field_count", field_count.to_string())
            .with_attribute("status", updated.autopilot.status.clone())
            .with_attribute("visibility", updated.autopilot.visibility.clone()),
    );
    state
        .observability
        .increment_counter("autopilot.updated", &request_id);

    Ok((
        StatusCode::OK,
        Json(serde_json::json!({
            "data": autopilot_aggregate_payload(&updated),
        })),
    ))
}

async fn create_autopilot_thread(
    State(state): State<AppState>,
    Path(autopilot): Path<String>,
    headers: HeaderMap,
    Json(payload): Json<CreateAutopilotThreadRequestPayload>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiErrorResponse>)> {
    let request_id = request_id(&headers);
    let bundle = session_bundle_from_headers(&state, &headers).await?;
    let reference = normalize_autopilot_reference(autopilot)?;
    let title = normalize_optional_bounded_string(payload.title, "title", 200)?
        .unwrap_or_else(|| "New conversation".to_string());

    let autopilot = state
        ._domain_store
        .resolve_owned_autopilot(&bundle.user.id, &reference)
        .await
        .map_err(map_domain_store_error)?;
    let thread_id = format!("thread_{}", uuid::Uuid::new_v4().simple());
    let thread = state
        .codex_thread_store
        .create_autopilot_thread_for_user(
            &bundle.user.id,
            &bundle.session.active_org_id,
            &thread_id,
            &autopilot.autopilot.id,
            &title,
        )
        .await
        .map_err(map_thread_store_error)?;

    state.observability.audit(
        AuditEvent::new("autopilot.thread_created", request_id.clone())
            .with_user_id(bundle.user.id)
            .with_session_id(bundle.session.session_id)
            .with_org_id(bundle.session.active_org_id)
            .with_device_id(bundle.session.device_id)
            .with_attribute("autopilot_id", autopilot.autopilot.id.clone())
            .with_attribute("thread_id", thread.id.clone())
            .with_attribute("title_length", thread.title.chars().count().to_string()),
    );
    state
        .observability
        .increment_counter("autopilot.thread_created", &request_id);

    Ok((
        StatusCode::CREATED,
        Json(serde_json::json!({
            "data": autopilot_thread_payload(&thread),
        })),
    ))
}

async fn list_autopilot_threads(
    State(state): State<AppState>,
    Path(autopilot): Path<String>,
    headers: HeaderMap,
    Query(query): Query<AutopilotThreadListQuery>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiErrorResponse>)> {
    let request_id = request_id(&headers);
    let bundle = session_bundle_from_headers(&state, &headers).await?;
    let reference = normalize_autopilot_reference(autopilot)?;

    let autopilot = state
        ._domain_store
        .resolve_owned_autopilot(&bundle.user.id, &reference)
        .await
        .map_err(map_domain_store_error)?;

    let limit = query.limit.unwrap_or(50).clamp(1, 200);
    let mut threads = state
        .codex_thread_store
        .list_autopilot_threads_for_user(
            &bundle.user.id,
            Some(&bundle.session.active_org_id),
            &autopilot.autopilot.id,
        )
        .await
        .map_err(map_thread_store_error)?;
    threads.truncate(limit);

    state.observability.audit(
        AuditEvent::new("autopilot.threads_viewed", request_id.clone())
            .with_user_id(bundle.user.id)
            .with_session_id(bundle.session.session_id)
            .with_org_id(bundle.session.active_org_id)
            .with_device_id(bundle.session.device_id)
            .with_attribute("autopilot_id", autopilot.autopilot.id.clone())
            .with_attribute("limit", limit.to_string())
            .with_attribute("count", threads.len().to_string()),
    );
    state
        .observability
        .increment_counter("autopilot.threads_viewed", &request_id);

    Ok((
        StatusCode::OK,
        Json(serde_json::json!({
            "data": threads
                .iter()
                .map(autopilot_thread_payload)
                .collect::<Vec<_>>(),
        })),
    ))
}

async fn autopilot_stream(
    State(state): State<AppState>,
    Path(autopilot): Path<String>,
    headers: HeaderMap,
    Query(query): Query<AutopilotStreamQuery>,
    Json(payload): Json<serde_json::Value>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiErrorResponse>)> {
    let request_id = request_id(&headers);
    let bundle = session_bundle_from_headers(&state, &headers).await?;
    let reference = normalize_autopilot_reference(autopilot)?;
    let autopilot = state
        ._domain_store
        .resolve_owned_autopilot(&bundle.user.id, &reference)
        .await
        .map_err(map_domain_store_error)?;

    let requested_thread_id = query
        .conversation_id
        .and_then(non_empty)
        .or_else(|| query.thread_id.and_then(non_empty))
        .or_else(|| autopilot_stream_thread_id_from_payload(&payload));

    let thread = match requested_thread_id {
        Some(thread_id) => {
            let normalized_thread_id = normalized_conversation_id(&thread_id)?;
            let _existing = state
                .codex_thread_store
                .get_thread_for_user(&bundle.user.id, &normalized_thread_id)
                .await
                .map_err(map_autopilot_stream_thread_error)?;
            state
                .codex_thread_store
                .create_autopilot_thread_for_user(
                    &bundle.user.id,
                    &bundle.session.active_org_id,
                    &normalized_thread_id,
                    &autopilot.autopilot.id,
                    "Autopilot conversation",
                )
                .await
                .map_err(map_autopilot_stream_thread_error)?
        }
        None => {
            let thread_id = format!("thread_{}", uuid::Uuid::new_v4().simple());
            state
                .codex_thread_store
                .create_autopilot_thread_for_user(
                    &bundle.user.id,
                    &bundle.session.active_org_id,
                    &thread_id,
                    &autopilot.autopilot.id,
                    "Autopilot conversation",
                )
                .await
                .map_err(map_thread_store_error)?
        }
    };

    let text = legacy_stream_user_text_from_payload(&payload).ok_or_else(|| {
        validation_error(
            "messages",
            "Autopilot stream payload must include user message text.",
        )
    })?;
    validate_codex_turn_text(&text)?;

    let worker_id = legacy_stream_worker_id_from_payload(&payload)
        .unwrap_or_else(|| "desktopw:shared".to_string());
    let thread_id = thread.id.clone();
    let control_request_id = format!("autopilot_stream_{}", uuid::Uuid::new_v4().simple());
    let control_request = RuntimeCodexWorkerControlRequest {
        request_id: control_request_id.clone(),
        method: "turn/start".to_string(),
        params: serde_json::json!({
            "thread_id": thread_id.clone(),
            "text": text,
            "autopilot_id": autopilot.autopilot.id,
        }),
        request_version: Some("v1".to_string()),
        source: Some("autopilot_stream_alias".to_string()),
        session_id: Some(bundle.session.session_id.clone()),
        thread_id: Some(thread_id.clone()),
    };
    let control_response =
        execute_codex_control_request(&state, &bundle, "turn/start", &control_request).await?;

    let worker_events_topic = org_worker_events_topic(&bundle.session.active_org_id);
    state.observability.audit(
        AuditEvent::new("autopilot.stream.bootstrap.accepted", request_id.clone())
            .with_user_id(bundle.user.id.clone())
            .with_session_id(bundle.session.session_id.clone())
            .with_org_id(bundle.session.active_org_id.clone())
            .with_device_id(bundle.session.device_id.clone())
            .with_attribute("autopilot_id", autopilot.autopilot.id.clone())
            .with_attribute("thread_id", thread_id.clone())
            .with_attribute("worker_id", worker_id.clone())
            .with_attribute("method", "turn/start".to_string())
            .with_attribute("transport", "khala_ws".to_string()),
    );
    state
        .observability
        .increment_counter("autopilot.stream.bootstrap.accepted", &request_id);

    Ok(ok_data(serde_json::json!({
        "accepted": true,
        "autopilotId": autopilot.autopilot.id,
        "autopilotConfigVersion": autopilot.autopilot.config_version,
        "threadId": thread_id.clone(),
        "conversationId": thread_id,
        "streamProtocol": "disabled",
        "delivery": {
            "transport": "khala_ws",
            "topic": worker_events_topic,
            "scope": "runtime.codex_worker_events",
            "syncTokenRoute": ROUTE_SYNC_TOKEN,
        },
        "control": {
            "method": "turn/start",
            "workerId": worker_id,
            "requestId": control_request_id,
        },
        "response": control_response,
    })))
}

async fn settings_profile_show(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiErrorResponse>)> {
    let request_id = request_id(&headers);
    let access_token = access_token_from_headers(&headers)
        .ok_or_else(|| unauthorized_error("Unauthenticated."))?;

    let bundle = state
        .auth
        .session_or_pat_from_access_token(&access_token)
        .await
        .map_err(map_auth_error)?;

    state.observability.audit(
        AuditEvent::new("profile.read", request_id.clone())
            .with_user_id(bundle.user.id.clone())
            .with_session_id(bundle.session.session_id.clone())
            .with_org_id(bundle.session.active_org_id.clone())
            .with_device_id(bundle.session.device_id.clone()),
    );
    state
        .observability
        .increment_counter("profile.read", &request_id);

    Ok((
        StatusCode::OK,
        Json(serde_json::json!({
            "data": {
                "id": bundle.user.id,
                "name": bundle.user.name,
                "email": bundle.user.email,
                "avatar": "",
                "createdAt": serde_json::Value::Null,
                "updatedAt": serde_json::Value::Null,
            }
        })),
    ))
}

async fn settings_profile_update(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<UpdateProfileRequestPayload>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiErrorResponse>)> {
    let request_id = request_id(&headers);
    let access_token = access_token_from_headers(&headers)
        .ok_or_else(|| unauthorized_error("Unauthenticated."))?;

    let bundle = state
        .auth
        .session_or_pat_from_access_token(&access_token)
        .await
        .map_err(map_auth_error)?;

    let normalized_name = payload.name.trim().to_string();
    if normalized_name.is_empty() {
        return Err(validation_error("name", "The name field is required."));
    }
    if normalized_name.chars().count() > 255 {
        return Err(validation_error(
            "name",
            "The name field may not be greater than 255 characters.",
        ));
    }

    let updated_user = state
        .auth
        .update_profile_name(&bundle.user.id, normalized_name)
        .await
        .map_err(map_auth_error)?;

    let updated_at = timestamp(Utc::now());
    state.observability.audit(
        AuditEvent::new("profile.updated", request_id.clone())
            .with_user_id(updated_user.id.clone())
            .with_session_id(bundle.session.session_id.clone())
            .with_org_id(bundle.session.active_org_id.clone())
            .with_device_id(bundle.session.device_id.clone())
            .with_attribute("field_updated", "name".to_string())
            .with_attribute("source", "api".to_string()),
    );
    state
        .observability
        .increment_counter("profile.updated", &request_id);

    Ok((
        StatusCode::OK,
        Json(serde_json::json!({
            "data": {
                "id": updated_user.id,
                "name": updated_user.name,
                "email": updated_user.email,
                "avatar": "",
                "updatedAt": updated_at,
            }
        })),
    ))
}

async fn settings_profile_delete(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<DeleteProfileRequestPayload>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiErrorResponse>)> {
    let request_id = request_id(&headers);
    let access_token = access_token_from_headers(&headers)
        .ok_or_else(|| unauthorized_error("Unauthenticated."))?;

    let bundle = state
        .auth
        .session_or_pat_from_access_token(&access_token)
        .await
        .map_err(map_auth_error)?;

    let normalized_email = payload.email.trim().to_lowercase();
    if normalized_email.is_empty() {
        return Err(validation_error("email", "The email field is required."));
    }
    if !normalized_email.contains('@') || normalized_email.chars().count() > 255 {
        return Err(validation_error(
            "email",
            "The email field must be a valid email address.",
        ));
    }

    let user_email = bundle.user.email.trim().to_lowercase();
    if normalized_email != user_email {
        return Err(validation_error(
            "email",
            "Email confirmation does not match the authenticated user.",
        ));
    }

    let deleted_user = state
        .auth
        .delete_profile(&bundle.user.id)
        .await
        .map_err(map_auth_error)?;

    state.observability.audit(
        AuditEvent::new("profile.deleted", request_id.clone())
            .with_user_id(deleted_user.id)
            .with_session_id(bundle.session.session_id)
            .with_org_id(bundle.session.active_org_id)
            .with_device_id(bundle.session.device_id)
            .with_attribute("source", "api".to_string()),
    );
    state
        .observability
        .increment_counter("profile.deleted", &request_id);

    Ok((
        StatusCode::OK,
        Json(serde_json::json!({
            "data": {
                "deleted": true,
            }
        })),
    ))
}

async fn list_personal_access_tokens(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiErrorResponse>)> {
    let access_token = access_token_from_headers(&headers)
        .ok_or_else(|| unauthorized_error("Unauthenticated."))?;

    let bundle = state
        .auth
        .session_or_pat_from_access_token(&access_token)
        .await
        .map_err(map_auth_error)?;

    let current_token_id = state
        .auth
        .current_personal_access_token_id(&bundle.user.id, &access_token)
        .await;

    let tokens = state
        .auth
        .list_personal_access_tokens(&bundle.user.id)
        .await
        .map_err(map_auth_error)?
        .into_iter()
        .filter(|token| token.revoked_at.is_none())
        .map(|token| {
            serde_json::json!({
                "id": token.token_id,
                "name": token.name,
                "abilities": token.scopes,
                "lastUsedAt": token.last_used_at.map(timestamp),
                "expiresAt": token.expires_at.map(timestamp),
                "createdAt": timestamp(token.created_at),
                "isCurrent": current_token_id
                    .as_ref()
                    .map(|current| current == &token.token_id)
                    .unwrap_or(false),
            })
        })
        .collect::<Vec<_>>();

    Ok((StatusCode::OK, Json(serde_json::json!({ "data": tokens }))))
}

async fn create_personal_access_token(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateTokenRequestPayload>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiErrorResponse>)> {
    let access_token = access_token_from_headers(&headers)
        .ok_or_else(|| unauthorized_error("Unauthenticated."))?;

    let bundle = state
        .auth
        .session_or_pat_from_access_token(&access_token)
        .await
        .map_err(map_auth_error)?;

    let name = payload.name.trim().to_string();
    if name.is_empty() {
        return Err(validation_error("name", "Token name is required."));
    }
    if name.chars().count() > 100 {
        return Err(validation_error(
            "name",
            "Token name may not be greater than 100 characters.",
        ));
    }

    let abilities = match payload.abilities {
        Some(values) if !values.is_empty() => {
            let mut normalized = Vec::new();
            for value in values {
                let trimmed = value.trim();
                if trimmed.chars().count() > 100 {
                    return Err(validation_error(
                        "abilities",
                        "Token abilities may not be greater than 100 characters.",
                    ));
                }

                if !trimmed.is_empty() {
                    normalized.push(trimmed.to_string());
                }
            }

            if normalized.is_empty() {
                vec!["*".to_string()]
            } else {
                normalized
            }
        }
        _ => vec!["*".to_string()],
    };

    let now = Utc::now();
    let expires_at = payload
        .expires_at
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(parse_rfc3339_utc)
        .transpose()
        .map_err(|_| validation_error("expires_at", "The expires_at is not a valid date."))?;
    let ttl_seconds =
        expires_at.map(|value| value.signed_duration_since(now).num_seconds().max(0) as u64);

    let issued = state
        .auth
        .issue_personal_access_token(
            &bundle.user.id,
            name.clone(),
            abilities.clone(),
            ttl_seconds,
        )
        .await
        .map_err(map_auth_error)?;

    let response = serde_json::json!({
        "data": {
            "token": issued.plain_text_token,
            "tokenableId": bundle.user.id,
            "name": name,
            "abilities": abilities,
            "expiresAt": issued.token.expires_at.map(timestamp),
        }
    });

    Ok((StatusCode::CREATED, Json(response)))
}

async fn delete_current_personal_access_token(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiErrorResponse>)> {
    let access_token = access_token_from_headers(&headers)
        .ok_or_else(|| unauthorized_error("Unauthenticated."))?;

    let bundle = state
        .auth
        .session_or_pat_from_access_token(&access_token)
        .await
        .map_err(map_auth_error)?;

    let token_id = state
        .auth
        .current_personal_access_token_id(&bundle.user.id, &access_token)
        .await;

    let deleted = match token_id {
        Some(token_id) => state
            .auth
            .revoke_personal_access_token(&bundle.user.id, &token_id)
            .await
            .map_err(map_auth_error)?,
        None => false,
    };

    Ok((
        StatusCode::OK,
        Json(serde_json::json!({ "data": { "deleted": deleted } })),
    ))
}

async fn delete_personal_access_token(
    State(state): State<AppState>,
    Path(token_id): Path<String>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiErrorResponse>)> {
    let access_token = access_token_from_headers(&headers)
        .ok_or_else(|| unauthorized_error("Unauthenticated."))?;

    let bundle = state
        .auth
        .session_or_pat_from_access_token(&access_token)
        .await
        .map_err(map_auth_error)?;

    let normalized_token_id = token_id.trim().to_string();
    if normalized_token_id.is_empty() {
        return Err(not_found_error("Not found."));
    }

    let deleted = state
        .auth
        .revoke_personal_access_token(&bundle.user.id, &normalized_token_id)
        .await
        .map_err(map_auth_error)?;
    if !deleted {
        return Err(not_found_error("Not found."));
    }

    Ok((
        StatusCode::OK,
        Json(serde_json::json!({ "data": { "deleted": true } })),
    ))
}

async fn delete_all_personal_access_tokens(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiErrorResponse>)> {
    let access_token = access_token_from_headers(&headers)
        .ok_or_else(|| unauthorized_error("Unauthenticated."))?;

    let bundle = state
        .auth
        .session_or_pat_from_access_token(&access_token)
        .await
        .map_err(map_auth_error)?;

    let deleted_count = state
        .auth
        .revoke_all_personal_access_tokens(&bundle.user.id)
        .await
        .map_err(map_auth_error)?;

    Ok((
        StatusCode::OK,
        Json(serde_json::json!({ "data": { "deletedCount": deleted_count } })),
    ))
}

async fn khala_token(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<KhalaTokenRequestPayload>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiErrorResponse>)> {
    let access_token = access_token_from_headers(&headers)
        .ok_or_else(|| unauthorized_error("Unauthenticated."))?;

    let session = state
        .auth
        .session_or_pat_from_access_token(&access_token)
        .await
        .map_err(map_auth_error)?;

    let mut scope = Vec::new();
    for entry in payload.scope {
        let value = entry.trim();
        if value.chars().count() > 120 {
            return Err(validation_error(
                "scope",
                "Scope entries may not be greater than 120 characters.",
            ));
        }

        if !value.is_empty() {
            scope.push(value.to_string());
        }
    }

    let workspace_id = payload
        .workspace_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string);
    if workspace_id
        .as_ref()
        .map(|value| value.chars().count() > 120)
        .unwrap_or(false)
    {
        return Err(validation_error(
            "workspace_id",
            "Workspace id may not be greater than 120 characters.",
        ));
    }

    let role = payload
        .role
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string);
    if role
        .as_deref()
        .map(|value| !matches!(value, "member" | "admin" | "owner"))
        .unwrap_or(false)
    {
        return Err(validation_error(
            "role",
            "Role must be one of member, admin, owner.",
        ));
    }

    let issued = state
        .khala_token_issuer
        .issue(KhalaTokenIssueRequest {
            user_id: session.user.id.clone(),
            scope,
            workspace_id,
            role,
        })
        .map_err(map_khala_error)?;

    Ok(ok_data(issued))
}

async fn org_memberships(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiErrorResponse>)> {
    let access_token = access_token_from_headers(&headers)
        .ok_or_else(|| unauthorized_error("Unauthenticated."))?;

    let bundle = state
        .auth
        .session_or_pat_from_access_token(&access_token)
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
    let access_token = access_token_from_headers(&headers)
        .ok_or_else(|| unauthorized_error("Unauthenticated."))?;

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
    let access_token = access_token_from_headers(&headers)
        .ok_or_else(|| unauthorized_error("Unauthenticated."))?;

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

    Ok(ok_data(decision))
}

async fn sync_token(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<SyncTokenRequestPayload>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiErrorResponse>)> {
    let request_id = request_id(&headers);
    let access_token = access_token_from_headers(&headers)
        .ok_or_else(|| unauthorized_error("Unauthenticated."))?;

    let session = state
        .auth
        .session_or_pat_from_access_token(&access_token)
        .await
        .map_err(map_auth_error)?;

    let is_pat_session = session.session.session_id.starts_with("pat:");

    let device_id = payload
        .device_id
        .and_then(non_empty)
        .unwrap_or_else(|| session.session.device_id.clone());

    if !is_pat_session && device_id != session.session.device_id {
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

    Ok(ok_data(issued))
}

async fn legacy_chats_index(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<LegacyChatsListQuery>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiErrorResponse>)> {
    let access_token =
        bearer_token(&headers).ok_or_else(|| unauthorized_error("Unauthenticated."))?;
    let session = state
        .auth
        .session_from_access_token(&access_token)
        .await
        .map_err(map_auth_error)?;

    let limit = query.limit.unwrap_or(50).clamp(1, 200);
    let mut threads = state
        .codex_thread_store
        .list_threads_for_user(&session.user.id, Some(&session.session.active_org_id))
        .await
        .map_err(map_thread_store_error)?;
    threads.truncate(limit);

    let data = threads
        .iter()
        .map(|thread| legacy_chat_summary(thread, None))
        .collect::<Vec<_>>();

    Ok(legacy_chat_data_response(
        serde_json::json!(data),
        StatusCode::OK,
    ))
}

async fn legacy_chats_store(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<LegacyCreateChatRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiErrorResponse>)> {
    let access_token =
        bearer_token(&headers).ok_or_else(|| unauthorized_error("Unauthenticated."))?;
    let session = state
        .auth
        .session_from_access_token(&access_token)
        .await
        .map_err(map_auth_error)?;

    let title = payload
        .title
        .as_deref()
        .and_then(|value| non_empty(value.to_string()))
        .unwrap_or_else(|| "New conversation".to_string());
    let conversation_id = format!("thread_{}", uuid::Uuid::new_v4().simple());
    let thread = state
        .codex_thread_store
        .create_thread_for_user(
            &session.user.id,
            &session.session.active_org_id,
            &conversation_id,
        )
        .await
        .map_err(map_thread_store_error)?;

    Ok(legacy_chat_data_response(
        legacy_chat_summary(&thread, Some(&title)),
        StatusCode::CREATED,
    ))
}

async fn legacy_chats_show(
    State(state): State<AppState>,
    Path(conversation_id): Path<String>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiErrorResponse>)> {
    let access_token =
        bearer_token(&headers).ok_or_else(|| unauthorized_error("Unauthenticated."))?;
    let session = state
        .auth
        .session_from_access_token(&access_token)
        .await
        .map_err(map_auth_error)?;
    let normalized_conversation_id = normalized_conversation_id(&conversation_id)?;

    let thread = state
        .codex_thread_store
        .get_thread_for_user(&session.user.id, &normalized_conversation_id)
        .await
        .map_err(map_thread_store_error)?;
    let messages = state
        .codex_thread_store
        .list_thread_messages_for_user(&session.user.id, &normalized_conversation_id)
        .await
        .map_err(map_thread_store_error)?;

    Ok(legacy_chat_data_response(
        serde_json::json!({
            "conversation": legacy_chat_summary(&thread, None),
            "messages": legacy_chat_messages(&messages),
            "runs": [],
        }),
        StatusCode::OK,
    ))
}

async fn legacy_chats_messages(
    State(state): State<AppState>,
    Path(conversation_id): Path<String>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiErrorResponse>)> {
    let access_token =
        bearer_token(&headers).ok_or_else(|| unauthorized_error("Unauthenticated."))?;
    let session = state
        .auth
        .session_from_access_token(&access_token)
        .await
        .map_err(map_auth_error)?;
    let normalized_conversation_id = normalized_conversation_id(&conversation_id)?;

    let _ = state
        .codex_thread_store
        .get_thread_for_user(&session.user.id, &normalized_conversation_id)
        .await
        .map_err(map_thread_store_error)?;
    let messages = state
        .codex_thread_store
        .list_thread_messages_for_user(&session.user.id, &normalized_conversation_id)
        .await
        .map_err(map_thread_store_error)?;

    Ok(legacy_chat_data_response(
        serde_json::json!(legacy_chat_messages(&messages)),
        StatusCode::OK,
    ))
}

async fn legacy_chat_stream(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<serde_json::Value>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiErrorResponse>)> {
    legacy_stream_bridge(state, None, headers, payload).await
}

async fn legacy_chats_stream(
    State(state): State<AppState>,
    Path(conversation_id): Path<String>,
    headers: HeaderMap,
    Json(payload): Json<serde_json::Value>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiErrorResponse>)> {
    legacy_stream_bridge(state, Some(conversation_id), headers, payload).await
}

async fn legacy_stream_bridge(
    state: AppState,
    conversation_id: Option<String>,
    headers: HeaderMap,
    payload: serde_json::Value,
) -> Result<Response, (StatusCode, Json<ApiErrorResponse>)> {
    let audit_request_id = request_id(&headers);
    let access_token =
        bearer_token(&headers).ok_or_else(|| unauthorized_error("Unauthenticated."))?;
    let session = state
        .auth
        .session_from_access_token(&access_token)
        .await
        .map_err(map_auth_error)?;

    let thread_id = match conversation_id {
        Some(conversation_id) => normalized_conversation_id(&conversation_id)?,
        None => legacy_stream_thread_id_from_payload(&payload)
            .unwrap_or_else(|| format!("thread_{}", uuid::Uuid::new_v4().simple())),
    };

    let text = legacy_stream_user_text_from_payload(&payload).ok_or_else(|| {
        validation_error(
            "messages",
            "Legacy stream payload must include user message text.",
        )
    })?;
    validate_codex_turn_text(&text)?;

    let worker_id = legacy_stream_worker_id_from_payload(&payload);
    let thread_id_for_bridge = thread_id.clone();
    let text_for_bridge = text.clone();
    let bridge_request_id = format!("legacy_stream_{}", uuid::Uuid::new_v4().simple());
    let control_request = RuntimeCodexWorkerControlRequest {
        request_id: bridge_request_id.clone(),
        method: "turn/start".to_string(),
        params: serde_json::json!({
            "thread_id": thread_id_for_bridge,
            "text": text_for_bridge,
        }),
        request_version: Some("v1".to_string()),
        source: Some("legacy_chat_stream_alias".to_string()),
        session_id: None,
        thread_id: None,
    };

    let worker_id_for_response = worker_id.unwrap_or_else(|| "desktopw:shared".to_string());
    let bridge_response =
        execute_codex_control_request(&state, &session, "turn/start", &control_request).await?;

    state.observability.audit(
        AuditEvent::new(
            "legacy.chat.stream.bridge.accepted",
            audit_request_id.clone(),
        )
        .with_user_id(session.user.id.clone())
        .with_session_id(session.session.session_id.clone())
        .with_org_id(session.session.active_org_id.clone())
        .with_device_id(session.session.device_id.clone())
        .with_attribute("thread_id", thread_id)
        .with_attribute("bridge_method", "turn/start")
        .with_attribute("bridge_request_id", bridge_request_id.clone())
        .with_attribute("worker_id", worker_id_for_response.clone()),
    );
    state
        .observability
        .increment_counter("legacy.chat.stream.bridge.accepted", &audit_request_id);

    Ok(legacy_chat_stream_data_response(
        serde_json::json!({
            "retired": true,
            "stream_protocol": "disabled",
            "canonical": "/api/runtime/codex/workers/:worker_id/requests",
            "bridge": {
                "method": "turn/start",
                "worker_id": worker_id_for_response,
                "request_id": bridge_request_id,
            },
            "response": bridge_response,
        }),
        StatusCode::OK,
    ))
}

async fn legacy_chats_runs(
    State(state): State<AppState>,
    Path(conversation_id): Path<String>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiErrorResponse>)> {
    let access_token =
        bearer_token(&headers).ok_or_else(|| unauthorized_error("Unauthenticated."))?;
    let session = state
        .auth
        .session_from_access_token(&access_token)
        .await
        .map_err(map_auth_error)?;
    let normalized_conversation_id = normalized_conversation_id(&conversation_id)?;

    let _ = state
        .codex_thread_store
        .get_thread_for_user(&session.user.id, &normalized_conversation_id)
        .await
        .map_err(map_thread_store_error)?;

    Ok(legacy_chat_data_response(
        serde_json::json!([]),
        StatusCode::OK,
    ))
}

async fn legacy_chats_run_events(
    State(state): State<AppState>,
    Path((conversation_id, run_id)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiErrorResponse>)> {
    let access_token =
        bearer_token(&headers).ok_or_else(|| unauthorized_error("Unauthenticated."))?;
    let session = state
        .auth
        .session_from_access_token(&access_token)
        .await
        .map_err(map_auth_error)?;
    let normalized_conversation_id = normalized_conversation_id(&conversation_id)?;
    let normalized_run_id = run_id.trim().to_string();
    if normalized_run_id.is_empty() {
        return Err(validation_error("run_id", "Run id is required."));
    }

    let _ = state
        .codex_thread_store
        .get_thread_for_user(&session.user.id, &normalized_conversation_id)
        .await
        .map_err(map_thread_store_error)?;

    Ok(legacy_chat_data_response(
        serde_json::json!({
            "run": {
                "id": normalized_run_id,
                "status": "retired",
                "modelProvider": "codex_app_server",
                "model": null,
            },
            "events": [],
        }),
        StatusCode::OK,
    ))
}

fn normalized_conversation_id(
    raw_conversation_id: &str,
) -> Result<String, (StatusCode, Json<ApiErrorResponse>)> {
    let normalized = raw_conversation_id.trim().to_string();
    if normalized.is_empty() {
        return Err(validation_error(
            "conversation_id",
            "Conversation id is required.",
        ));
    }
    Ok(normalized)
}

fn legacy_chat_data_response(payload: serde_json::Value, status: StatusCode) -> Response {
    let mut response = (status, Json(serde_json::json!({ "data": payload }))).into_response();
    response
        .headers_mut()
        .insert("x-oa-legacy-chat-retired", HeaderValue::from_static("true"));
    response.headers_mut().insert(
        "x-oa-legacy-chat-canonical",
        HeaderValue::from_static("/api/runtime/threads"),
    );
    response
}

fn legacy_chat_stream_data_response(payload: serde_json::Value, status: StatusCode) -> Response {
    let mut response = (status, Json(serde_json::json!({ "data": payload }))).into_response();
    response
        .headers_mut()
        .insert("x-oa-legacy-chat-retired", HeaderValue::from_static("true"));
    response.headers_mut().insert(
        "x-oa-legacy-chat-canonical",
        HeaderValue::from_static("/api/runtime/codex/workers/:worker_id/requests"),
    );
    response.headers_mut().insert(
        "x-oa-legacy-chat-stream-protocol",
        HeaderValue::from_static("disabled"),
    );
    response
}

fn legacy_stream_thread_id_from_payload(payload: &serde_json::Value) -> Option<String> {
    json_non_empty_string(payload.get("thread_id"))
        .or_else(|| json_non_empty_string(payload.get("threadId")))
        .or_else(|| json_non_empty_string(payload.get("conversation_id")))
        .or_else(|| json_non_empty_string(payload.get("conversationId")))
        .or_else(|| json_non_empty_string(payload.get("id")))
}

fn autopilot_stream_thread_id_from_payload(payload: &serde_json::Value) -> Option<String> {
    json_non_empty_string(payload.get("conversationId"))
        .or_else(|| json_non_empty_string(payload.get("conversation_id")))
        .or_else(|| json_non_empty_string(payload.get("threadId")))
        .or_else(|| json_non_empty_string(payload.get("thread_id")))
}

fn legacy_stream_worker_id_from_payload(payload: &serde_json::Value) -> Option<String> {
    json_non_empty_string(payload.get("worker_id"))
        .or_else(|| json_non_empty_string(payload.get("workerId")))
}

fn org_worker_events_topic(org_id: &str) -> String {
    if org_id.starts_with("org:") {
        format!("{org_id}:worker_events")
    } else {
        format!("org:{org_id}:worker_events")
    }
}

fn legacy_stream_user_text_from_payload(payload: &serde_json::Value) -> Option<String> {
    if let Some(text) = json_non_empty_string(payload.get("text"))
        .or_else(|| json_non_empty_string(payload.get("message")))
    {
        return Some(text);
    }

    let messages = payload.get("messages")?.as_array()?;
    for message in messages.iter().rev() {
        let role = json_non_empty_string(message.get("role"))
            .unwrap_or_else(|| "user".to_string())
            .to_ascii_lowercase();
        if role != "user" {
            continue;
        }
        if let Some(text) = legacy_stream_message_text(message) {
            return Some(text);
        }
    }

    None
}

fn legacy_stream_message_text(message: &serde_json::Value) -> Option<String> {
    if let Some(text) = json_non_empty_string(message.get("text"))
        .or_else(|| json_non_empty_string(message.get("message")))
    {
        return Some(text);
    }

    match message.get("content") {
        Some(serde_json::Value::String(content)) => non_empty(content.to_string()),
        Some(serde_json::Value::Object(content)) => json_non_empty_string(content.get("text")),
        Some(serde_json::Value::Array(parts)) => {
            let joined = parts
                .iter()
                .filter_map(|part| match part {
                    serde_json::Value::String(text) => non_empty(text.to_string()),
                    serde_json::Value::Object(object) => json_non_empty_string(object.get("text"))
                        .or_else(|| json_non_empty_string(object.get("value"))),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("\n");
            non_empty(joined)
        }
        _ => None,
    }
}

fn json_non_empty_string(value: Option<&serde_json::Value>) -> Option<String> {
    let value = value?;
    let raw = value.as_str()?;
    non_empty(raw.to_string())
}

fn legacy_chat_summary(
    thread: &ThreadProjection,
    title_override: Option<&str>,
) -> serde_json::Value {
    serde_json::json!({
        "id": thread.thread_id,
        "title": title_override
            .map(str::to_string)
            .unwrap_or_else(|| format!("Codex Thread {}", thread.thread_id)),
        "autopilotId": serde_json::Value::Null,
        "createdAt": thread.created_at.to_rfc3339_opts(SecondsFormat::Millis, true),
        "updatedAt": thread.updated_at.to_rfc3339_opts(SecondsFormat::Millis, true),
    })
}

fn legacy_chat_messages(messages: &[ThreadMessageProjection]) -> Vec<serde_json::Value> {
    messages
        .iter()
        .map(|message| {
            serde_json::json!({
                "id": message.message_id,
                "runId": serde_json::Value::Null,
                "autopilotId": serde_json::Value::Null,
                "role": message.role,
                "content": message.text,
                "meta": serde_json::Value::Null,
                "createdAt": message.created_at.to_rfc3339_opts(SecondsFormat::Millis, true),
                "updatedAt": message.created_at.to_rfc3339_opts(SecondsFormat::Millis, true),
            })
        })
        .collect()
}

async fn list_runtime_threads(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiErrorResponse>)> {
    let access_token =
        bearer_token(&headers).ok_or_else(|| unauthorized_error("Unauthenticated."))?;
    let session = state
        .auth
        .session_from_access_token(&access_token)
        .await
        .map_err(map_auth_error)?;

    let threads = state
        .codex_thread_store
        .list_threads_for_user(&session.user.id, Some(&session.session.active_org_id))
        .await
        .map_err(map_thread_store_error)?;

    Ok(ok_data(serde_json::json!({
        "threads": threads,
    })))
}

async fn list_runtime_thread_messages(
    State(state): State<AppState>,
    Path(thread_id): Path<String>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiErrorResponse>)> {
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

    let messages = state
        .codex_thread_store
        .list_thread_messages_for_user(&session.user.id, &normalized_thread_id)
        .await
        .map_err(map_thread_store_error)?;

    Ok(ok_data(serde_json::json!({
        "thread_id": normalized_thread_id,
        "messages": messages,
    })))
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

    let appended = state
        .codex_thread_store
        .append_user_message(
            &session.user.id,
            &session.session.active_org_id,
            &normalized_thread_id,
            normalized_text,
        )
        .await
        .map_err(map_thread_store_error)?;
    let accepted_at = appended
        .message
        .created_at
        .to_rfc3339_opts(SecondsFormat::Millis, true);

    state.observability.audit(
        AuditEvent::new("runtime.thread.message.accepted", request_id.clone())
            .with_user_id(session.user.id.clone())
            .with_session_id(session.session.session_id.clone())
            .with_org_id(session.session.active_org_id.clone())
            .with_device_id(session.session.device_id.clone())
            .with_attribute("thread_id", normalized_thread_id.clone())
            .with_attribute("message_id", appended.message.message_id.clone()),
    );
    state
        .observability
        .increment_counter("runtime.thread.message.accepted", &request_id);

    Ok(ok_data(serde_json::json!({
        "accepted": true,
        "thread": appended.thread,
        "message": {
            "id": appended.message.message_id,
            "thread_id": appended.message.thread_id,
            "role": appended.message.role,
            "text": appended.message.text,
            "accepted_at": accepted_at,
        }
    })))
}

async fn runtime_codex_worker_request(
    State(state): State<AppState>,
    Path(worker_id): Path<String>,
    headers: HeaderMap,
    Json(payload): Json<RuntimeCodexWorkerControlRequestEnvelope>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiErrorResponse>)> {
    let request_id = request_id(&headers);
    let access_token =
        bearer_token(&headers).ok_or_else(|| unauthorized_error("Unauthenticated."))?;
    let session = state
        .auth
        .session_from_access_token(&access_token)
        .await
        .map_err(map_auth_error)?;

    let normalized_worker_id = worker_id.trim().to_string();
    if normalized_worker_id.is_empty() {
        return Err(validation_error("worker_id", "Worker id is required."));
    }

    let control_request = payload.request;
    let normalized_control_request_id = control_request.request_id.trim().to_string();
    if normalized_control_request_id.is_empty() {
        return Err(validation_error(
            "request.request_id",
            "Request id is required.",
        ));
    }

    let normalized_method = control_request.method.trim().to_lowercase();
    if !is_codex_control_method_allowed(&normalized_method) {
        return Err(validation_error(
            "request.method",
            "Control method is not allowlisted.",
        ));
    }

    let replay_key = format!(
        "{}:{}:{}",
        session.user.id, normalized_worker_id, normalized_control_request_id
    );
    if let Some(replayed) = state
        .codex_control_receipts
        .entries
        .lock()
        .await
        .get(&replay_key)
        .cloned()
    {
        let replayed = mark_codex_control_replay(replayed);
        state.observability.audit(
            AuditEvent::new("runtime.codex.control.request.accepted", request_id.clone())
                .with_user_id(session.user.id.clone())
                .with_session_id(session.session.session_id.clone())
                .with_org_id(session.session.active_org_id.clone())
                .with_device_id(session.session.device_id.clone())
                .with_attribute("worker_id", normalized_worker_id.clone())
                .with_attribute("control_request_id", normalized_control_request_id.clone())
                .with_attribute("method", normalized_method.clone())
                .with_attribute("idempotent_replay", "true".to_string()),
        );
        state
            .observability
            .increment_counter("runtime.codex.control.request.accepted", &request_id);
        return Ok(ok_data(replayed));
    }

    let response =
        execute_codex_control_request(&state, &session, &normalized_method, &control_request)
            .await?;

    let mut envelope = serde_json::json!({
        "worker_id": normalized_worker_id.clone(),
        "request_id": normalized_control_request_id.clone(),
        "ok": true,
        "method": normalized_method.clone(),
        "idempotent_replay": false,
        "response": response,
    });

    if let Some(source) = control_request
        .source
        .as_deref()
        .and_then(|value| non_empty(value.to_string()))
    {
        envelope["source"] = serde_json::json!(source);
    }
    if let Some(request_version) = control_request
        .request_version
        .as_deref()
        .and_then(|value| non_empty(value.to_string()))
    {
        envelope["request_version"] = serde_json::json!(request_version);
    }
    if let Some(session_id) = control_request
        .session_id
        .as_deref()
        .and_then(|value| non_empty(value.to_string()))
    {
        envelope["session_id"] = serde_json::json!(session_id);
    }

    state
        .codex_control_receipts
        .entries
        .lock()
        .await
        .insert(replay_key, envelope.clone());

    state.observability.audit(
        AuditEvent::new("runtime.codex.control.request.accepted", request_id.clone())
            .with_user_id(session.user.id.clone())
            .with_session_id(session.session.session_id.clone())
            .with_org_id(session.session.active_org_id.clone())
            .with_device_id(session.session.device_id.clone())
            .with_attribute("worker_id", normalized_worker_id)
            .with_attribute("control_request_id", normalized_control_request_id)
            .with_attribute("method", normalized_method)
            .with_attribute("idempotent_replay", "false".to_string()),
    );
    state
        .observability
        .increment_counter("runtime.codex.control.request.accepted", &request_id);

    Ok(ok_data(envelope))
}

async fn execute_codex_control_request(
    state: &AppState,
    session: &SessionBundle,
    method: &str,
    control_request: &RuntimeCodexWorkerControlRequest,
) -> Result<serde_json::Value, (StatusCode, Json<ApiErrorResponse>)> {
    match method {
        "thread/start" => {
            let thread_id = codex_request_thread_id(control_request)
                .unwrap_or_else(|| format!("thread_{}", uuid::Uuid::new_v4().simple()));
            let mut payload = serde_json::json!({
                "thread_id": thread_id,
            });
            if let Some(text) = codex_request_text(control_request) {
                validate_codex_turn_text(&text)?;
                let appended = state
                    .codex_thread_store
                    .append_user_message(
                        &session.user.id,
                        &session.session.active_org_id,
                        &thread_id,
                        text,
                    )
                    .await
                    .map_err(map_thread_store_error)?;
                payload["message"] = serde_json::json!({
                    "id": appended.message.message_id,
                    "thread_id": appended.message.thread_id,
                    "role": appended.message.role,
                    "text": appended.message.text,
                });
            }
            Ok(payload)
        }
        "thread/resume" => {
            let thread_id = codex_request_thread_id(control_request).ok_or_else(|| {
                validation_error("request.params.thread_id", "Thread id is required.")
            })?;
            let _ = state
                .codex_thread_store
                .list_thread_messages_for_user(&session.user.id, &thread_id)
                .await
                .map_err(map_thread_store_error)?;
            Ok(serde_json::json!({
                "thread_id": thread_id,
            }))
        }
        "thread/list" => {
            let threads = state
                .codex_thread_store
                .list_threads_for_user(&session.user.id, Some(&session.session.active_org_id))
                .await
                .map_err(map_thread_store_error)?;
            Ok(serde_json::json!({
                "threads": threads,
            }))
        }
        "thread/read" => {
            let thread_id = codex_request_thread_id(control_request).ok_or_else(|| {
                validation_error("request.params.thread_id", "Thread id is required.")
            })?;
            let messages = state
                .codex_thread_store
                .list_thread_messages_for_user(&session.user.id, &thread_id)
                .await
                .map_err(map_thread_store_error)?;
            Ok(serde_json::json!({
                "thread_id": thread_id,
                "messages": messages,
            }))
        }
        "turn/start" => {
            let thread_id = codex_request_thread_id(control_request).ok_or_else(|| {
                validation_error("request.params.thread_id", "Thread id is required.")
            })?;
            let text = codex_request_text(control_request).ok_or_else(|| {
                validation_error("request.params.text", "Message text is required.")
            })?;
            validate_codex_turn_text(&text)?;

            let appended = state
                .codex_thread_store
                .append_user_message(
                    &session.user.id,
                    &session.session.active_org_id,
                    &thread_id,
                    text,
                )
                .await
                .map_err(map_thread_store_error)?;

            Ok(serde_json::json!({
                "thread_id": thread_id,
                "turn": {
                    "id": format!("turn_{}", uuid::Uuid::new_v4().simple()),
                },
                "message": {
                    "id": appended.message.message_id,
                    "thread_id": appended.message.thread_id,
                    "role": appended.message.role,
                    "text": appended.message.text,
                },
            }))
        }
        "turn/interrupt" => Ok(serde_json::json!({
            "thread_id": codex_request_thread_id(control_request),
            "turn_id": normalized_json_string(control_request.params.get("turn_id"))
                .or_else(|| normalized_json_string(control_request.params.get("turnId"))),
            "status": "interrupted",
        })),
        _ => Err(validation_error(
            "request.method",
            "Control method is not allowlisted.",
        )),
    }
}

fn is_codex_control_method_allowed(method: &str) -> bool {
    CODEX_CONTROL_METHOD_ALLOWLIST
        .iter()
        .any(|allowed| *allowed == method)
}

fn mark_codex_control_replay(mut payload: serde_json::Value) -> serde_json::Value {
    if let Some(object) = payload.as_object_mut() {
        object.insert("idempotent_replay".to_string(), serde_json::json!(true));
    }
    payload
}

fn codex_request_thread_id(request: &RuntimeCodexWorkerControlRequest) -> Option<String> {
    request
        .thread_id
        .as_deref()
        .and_then(|value| non_empty(value.to_string()))
        .or_else(|| normalized_json_string(request.params.get("thread_id")))
        .or_else(|| normalized_json_string(request.params.get("threadId")))
}

fn codex_request_text(request: &RuntimeCodexWorkerControlRequest) -> Option<String> {
    normalized_json_string(request.params.get("text"))
        .or_else(|| normalized_json_string(request.params.get("message")))
        .or_else(|| normalized_json_string(request.params.get("prompt")))
        .or_else(|| {
            request
                .params
                .get("input")
                .and_then(serde_json::Value::as_array)
                .and_then(|parts| {
                    parts.iter().find_map(|part| {
                        let part_type =
                            normalized_json_string(part.get("type")).unwrap_or_default();
                        if part_type == "text" {
                            normalized_json_string(part.get("text"))
                        } else {
                            None
                        }
                    })
                })
        })
}

fn normalized_json_string(value: Option<&serde_json::Value>) -> Option<String> {
    value
        .and_then(serde_json::Value::as_str)
        .and_then(|raw| non_empty(raw.to_string()))
}

fn validate_codex_turn_text(text: &str) -> Result<(), (StatusCode, Json<ApiErrorResponse>)> {
    let normalized = text.trim();
    if normalized.is_empty() {
        return Err(validation_error(
            "request.params.text",
            "Message text is required.",
        ));
    }
    if normalized.chars().count() > 20_000 {
        return Err(validation_error(
            "request.params.text",
            "Message text exceeds 20000 characters.",
        ));
    }
    Ok(())
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
    let requested_device_id = payload
        .device_id
        .and_then(non_empty)
        .or_else(|| header_string(&headers, "x-device-id"));

    let rotate = payload.rotate_refresh_token.unwrap_or(true);

    let refreshed = match state
        .auth
        .refresh_session(&token, requested_device_id.as_deref(), rotate)
        .await
    {
        Ok(refreshed) => refreshed,
        Err(error) => {
            emit_auth_failure_event(&state, &request_id, "auth.refresh.failed", &error);
            return Err(map_auth_error(error));
        }
    };

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

    let bundle = match state.auth.session_from_access_token(&access_token).await {
        Ok(bundle) => bundle,
        Err(error) => {
            emit_auth_failure_event(&state, &request_id, "auth.logout.failed", &error);
            return Err(map_auth_error(error));
        }
    };

    let revoked = match state
        .auth
        .revoke_session_by_access_token(&access_token)
        .await
    {
        Ok(revoked) => revoked,
        Err(error) => {
            emit_auth_failure_event(&state, &request_id, "auth.logout.failed", &error);
            return Err(map_auth_error(error));
        }
    };

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

    propagate_runtime_revocation(
        &state,
        &request_id,
        vec![revoked.session_id.clone()],
        Vec::new(),
        SessionRevocationReason::UserRequested,
    )
    .await?;

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

async fn propagate_runtime_revocation(
    state: &AppState,
    request_id: &str,
    session_ids: Vec<String>,
    device_ids: Vec<String>,
    reason: SessionRevocationReason,
) -> Result<(), (StatusCode, Json<ApiErrorResponse>)> {
    if session_ids.is_empty() && device_ids.is_empty() {
        return Ok(());
    }

    let Some(client) = state.runtime_revocation_client.as_ref() else {
        state.observability.audit(
            AuditEvent::new(
                "auth.sessions.revocation.propagation.skipped",
                request_id.to_string(),
            )
            .with_attribute(
                "reason",
                "runtime_revocation_client_not_configured".to_string(),
            )
            .with_attribute("session_count", session_ids.len().to_string())
            .with_attribute("device_count", device_ids.len().to_string()),
        );

        return Ok(());
    };

    client
        .revoke_sessions(session_ids.clone(), device_ids.clone(), reason)
        .await
        .map_err(|message| {
            error_response_with_status(
                StatusCode::SERVICE_UNAVAILABLE,
                ApiErrorCode::ServiceUnavailable,
                format!("Failed to propagate websocket session revocation: {message}"),
            )
        })?;

    state.observability.audit(
        AuditEvent::new(
            "auth.sessions.revocation.propagation.completed",
            request_id.to_string(),
        )
        .with_attribute("reason", reason.as_str().to_string())
        .with_attribute("session_count", session_ids.len().to_string())
        .with_attribute("device_count", device_ids.len().to_string()),
    );

    Ok(())
}

fn map_auth_error(error: AuthError) -> (StatusCode, Json<ApiErrorResponse>) {
    match error {
        AuthError::Validation { field, message } => validation_error(field, &message),
        AuthError::Unauthorized { message } => unauthorized_error(&message),
        AuthError::Forbidden { message } => forbidden_error(&message),
        AuthError::Conflict { message } => {
            error_response_with_status(StatusCode::CONFLICT, ApiErrorCode::Conflict, message)
        }
        AuthError::Provider { message } => error_response_with_status(
            StatusCode::SERVICE_UNAVAILABLE,
            ApiErrorCode::ServiceUnavailable,
            message,
        ),
    }
}

fn auth_error_reason(error: &AuthError) -> (&'static str, Option<&'static str>) {
    match error {
        AuthError::Validation { field, .. } => ("invalid_request", Some(*field)),
        AuthError::Unauthorized { .. } => ("unauthorized", None),
        AuthError::Forbidden { .. } => ("forbidden", None),
        AuthError::Conflict { .. } => ("conflict", None),
        AuthError::Provider { .. } => ("service_unavailable", None),
    }
}

fn emit_auth_failure_event(
    state: &AppState,
    request_id: &str,
    event_name: &str,
    error: &AuthError,
) {
    let (reason, field) = auth_error_reason(error);
    let mut event = AuditEvent::new(event_name, request_id.to_string())
        .with_outcome("failure")
        .with_attribute("reason", reason.to_string());

    if let Some(field) = field {
        event = event.with_attribute("field", field.to_string());
    }

    state.observability.audit(event);
    state
        .observability
        .increment_counter(event_name, request_id);
}

fn map_sync_error(error: SyncTokenError) -> (StatusCode, Json<ApiErrorResponse>) {
    match error {
        SyncTokenError::InvalidScope { message } => error_response_with_status(
            StatusCode::UNPROCESSABLE_ENTITY,
            ApiErrorCode::InvalidScope,
            message,
        ),
        SyncTokenError::InvalidRequest { message } => error_response_with_status(
            StatusCode::UNPROCESSABLE_ENTITY,
            ApiErrorCode::InvalidRequest,
            message,
        ),
        SyncTokenError::Unavailable { message } => error_response_with_status(
            StatusCode::SERVICE_UNAVAILABLE,
            ApiErrorCode::SyncTokenUnavailable,
            message,
        ),
    }
}

fn map_khala_error(error: KhalaTokenError) -> (StatusCode, Json<ApiErrorResponse>) {
    match error {
        KhalaTokenError::InvalidRequest { message } => error_response_with_status(
            StatusCode::UNPROCESSABLE_ENTITY,
            ApiErrorCode::InvalidRequest,
            message,
        ),
        KhalaTokenError::Unavailable { message } => error_response_with_status(
            StatusCode::SERVICE_UNAVAILABLE,
            ApiErrorCode::KhalaTokenUnavailable,
            message,
        ),
    }
}

fn map_thread_store_error(error: ThreadStoreError) -> (StatusCode, Json<ApiErrorResponse>) {
    match error {
        ThreadStoreError::NotFound => not_found_error("Thread not found."),
        ThreadStoreError::Forbidden => forbidden_error("Requested thread is not available."),
        ThreadStoreError::Persistence { message } => error_response_with_status(
            StatusCode::SERVICE_UNAVAILABLE,
            ApiErrorCode::ServiceUnavailable,
            message,
        ),
    }
}

fn map_autopilot_stream_thread_error(
    error: ThreadStoreError,
) -> (StatusCode, Json<ApiErrorResponse>) {
    match error {
        ThreadStoreError::NotFound | ThreadStoreError::Forbidden => not_found_error("Not found."),
        ThreadStoreError::Persistence { message } => error_response_with_status(
            StatusCode::SERVICE_UNAVAILABLE,
            ApiErrorCode::ServiceUnavailable,
            message,
        ),
    }
}

fn map_domain_store_error(error: DomainStoreError) -> (StatusCode, Json<ApiErrorResponse>) {
    match error {
        DomainStoreError::NotFound => not_found_error("Requested resource was not found."),
        DomainStoreError::Forbidden => forbidden_error("Forbidden."),
        DomainStoreError::Validation { field, message } => validation_error(field, &message),
        DomainStoreError::Conflict { message } => {
            error_response_with_status(StatusCode::CONFLICT, ApiErrorCode::Conflict, message)
        }
        DomainStoreError::Persistence { message } => error_response_with_status(
            StatusCode::SERVICE_UNAVAILABLE,
            ApiErrorCode::ServiceUnavailable,
            message,
        ),
    }
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
                "lastRefreshedAt": bundle.session.last_refreshed_at.map(timestamp),
                "revokedAt": bundle.session.revoked_at.map(timestamp),
                "revokedReason": bundle.session.revoked_reason,
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

fn maintenance_response() -> Response {
    let html = r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>We'll be right back | OpenAgents</title>
  <style>
    :root { color-scheme: dark; }
    * { box-sizing: border-box; }
    html, body {
      margin: 0;
      min-height: 100%;
      width: 100%;
      background: #0a0a0a;
      color: #f5f5f5;
      font-family: 'Berkeley Mono', ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, 'Liberation Mono', monospace;
    }
    body { position: relative; overflow: hidden; }
    .bg {
      position: fixed;
      inset: 0;
      pointer-events: none;
      background:
        radial-gradient(120% 85% at 50% 0%, rgba(255, 255, 255, 0.07) 0%, rgba(255, 255, 255, 0) 55%),
        radial-gradient(ellipse 100% 100% at 50% 50%, transparent 12%, rgba(0, 0, 0, 0.55) 60%, rgba(0, 0, 0, 0.88) 100%);
    }
    .grid {
      position: fixed;
      inset: 0;
      pointer-events: none;
      background-image: radial-gradient(circle at center, rgba(255, 255, 255, 0.1) 1px, transparent 1px);
      background-size: 48px 48px;
    }
    .container {
      position: fixed;
      inset: 0;
      z-index: 2;
      display: grid;
      place-items: center;
      padding: 2rem;
    }
    .panel {
      width: min(640px, 100%);
      border: 1px solid rgba(255, 255, 255, 0.2);
      background: rgba(10, 10, 10, 0.75);
      backdrop-filter: blur(2px);
      border-radius: 12px;
      padding: 2rem;
      box-shadow: 0 0 0 1px rgba(255, 255, 255, 0.04) inset;
    }
    h1 {
      margin: 0;
      font-size: clamp(1.6rem, 2.2vw, 2.25rem);
      letter-spacing: 0.01em;
      line-height: 1.15;
    }
    p {
      margin: 0.9rem 0 0;
      color: rgba(255, 255, 255, 0.8);
      line-height: 1.5;
      font-size: 0.98rem;
    }
    .status {
      margin-top: 1.4rem;
      display: inline-block;
      font-size: 0.8rem;
      letter-spacing: 0.08em;
      text-transform: uppercase;
      border: 1px solid rgba(255, 255, 255, 0.25);
      padding: 0.45rem 0.65rem;
      border-radius: 8px;
      color: rgba(255, 255, 255, 0.88);
    }
  </style>
</head>
<body>
  <div class="bg" aria-hidden="true"></div>
  <div class="grid" aria-hidden="true"></div>
  <main class="container">
    <section class="panel" role="status" aria-live="polite">
      <h1>We'll be right back.</h1>
      <p>OpenAgents is temporarily unavailable while we complete an infrastructure switch. Please check back shortly.</p>
      <span class="status">Maintenance in progress</span>
    </section>
  </main>
</body>
</html>
"#;

    (
        StatusCode::SERVICE_UNAVAILABLE,
        [
            (CONTENT_TYPE, "text/html; charset=utf-8"),
            (CACHE_CONTROL, MAINTENANCE_CACHE_CONTROL),
        ],
        html,
    )
        .into_response()
}

fn maintenance_path_is_allowed(path: &str, allowed_paths: &[String]) -> bool {
    if path.is_empty() {
        return false;
    }

    allowed_paths.iter().any(|allowed| {
        let candidate = allowed.trim();
        if candidate.is_empty() {
            return false;
        }

        if candidate.ends_with('*') {
            let prefix = candidate.trim_end_matches('*');
            return !prefix.is_empty() && path.starts_with(prefix);
        }

        path == candidate
    })
}

fn query_param_value(query: Option<&str>, key: &str) -> Option<String> {
    let query = query?;
    for pair in query.split('&') {
        let mut pieces = pair.splitn(2, '=');
        let candidate = pieces.next()?.trim();
        let value = pieces.next().unwrap_or_default().trim();
        if candidate == key && !value.is_empty() {
            return Some(value.to_string());
        }
    }
    None
}

fn local_test_login_signature_is_valid(uri: &axum::http::Uri, signing_key: &str) -> bool {
    let provided_signature = query_param_value(uri.query(), "signature")
        .map(|value| value.to_lowercase())
        .unwrap_or_default();
    if provided_signature.is_empty() {
        return false;
    }

    let unsigned_query = uri
        .query()
        .unwrap_or_default()
        .split('&')
        .filter(|segment| !segment.trim().is_empty())
        .filter(|segment| {
            let mut pieces = segment.splitn(2, '=');
            pieces.next().unwrap_or_default().trim() != "signature"
        })
        .collect::<Vec<_>>()
        .join("&");

    let canonical_payload = if unsigned_query.is_empty() {
        uri.path().to_string()
    } else {
        format!("{}?{unsigned_query}", uri.path())
    };

    let mut mac = match HmacSha256::new_from_slice(signing_key.as_bytes()) {
        Ok(value) => value,
        Err(_) => return false,
    };
    mac.update(canonical_payload.as_bytes());
    let expected = sha256_bytes_hex(&mac.finalize().into_bytes());

    expected == provided_signature
}

fn local_test_login_email_allowed(email: &str, allowed_emails: &[String]) -> bool {
    if allowed_emails.is_empty() {
        return false;
    }

    let normalized_email = email.trim().to_lowercase();
    if normalized_email.is_empty() {
        return false;
    }

    allowed_emails.iter().any(|allowed| {
        let candidate = allowed.trim().to_lowercase();
        !candidate.is_empty() && candidate == normalized_email
    })
}

fn maintenance_redirect_location(uri: &axum::http::Uri) -> String {
    let path = uri.path();
    let query = uri.query().unwrap_or_default();
    let filtered = query
        .split('&')
        .filter(|segment| !segment.trim().is_empty())
        .filter(|segment| {
            let mut pieces = segment.splitn(2, '=');
            pieces.next().unwrap_or_default().trim() != MAINTENANCE_BYPASS_QUERY_PARAM
        })
        .collect::<Vec<_>>();

    if filtered.is_empty() {
        if path.is_empty() {
            "/".to_string()
        } else {
            path.to_string()
        }
    } else {
        format!("{path}?{}", filtered.join("&"))
    }
}

fn maintenance_bypass_cookie_payload(token: &str, expires_at: u64) -> Option<String> {
    let payload = expires_at.to_string();
    let mut mac = HmacSha256::new_from_slice(token.as_bytes()).ok()?;
    mac.update(payload.as_bytes());
    let signature = URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes());
    Some(format!("{payload}.{signature}"))
}

fn maintenance_cookie_is_valid(cookie_value: &str, token: &str) -> bool {
    let mut parts = cookie_value.splitn(2, '.');
    let expires_at_raw = parts.next().unwrap_or_default();
    let signature_segment = parts.next().unwrap_or_default();
    if expires_at_raw.is_empty() || signature_segment.is_empty() {
        return false;
    }

    let expires_at = match expires_at_raw.parse::<u64>() {
        Ok(value) => value,
        Err(_) => return false,
    };
    let now = Utc::now().timestamp().max(0) as u64;
    if expires_at <= now {
        return false;
    }

    let signature = match URL_SAFE_NO_PAD.decode(signature_segment) {
        Ok(value) => value,
        Err(_) => return false,
    };

    let mut mac = match HmacSha256::new_from_slice(token.as_bytes()) {
        Ok(value) => value,
        Err(_) => return false,
    };
    mac.update(expires_at_raw.as_bytes());
    mac.verify_slice(&signature).is_ok()
}

fn maintenance_bypass_cookie(
    cookie_name: &str,
    cookie_payload: &str,
    max_age_seconds: u64,
) -> String {
    format!(
        "{cookie_name}={cookie_payload}; Path=/; HttpOnly; Secure; SameSite=Lax; Max-Age={max_age_seconds}"
    )
}

fn challenge_cookie(challenge_id: &str, max_age_seconds: u64) -> String {
    format!(
        "{CHALLENGE_COOKIE_NAME}={challenge_id}; Path=/; HttpOnly; SameSite=Lax; Max-Age={max_age_seconds}"
    )
}

fn auth_access_cookie(access_token: &str, max_age_seconds: u64) -> String {
    format!(
        "{AUTH_ACCESS_COOKIE_NAME}={access_token}; Path=/; HttpOnly; SameSite=Lax; Max-Age={max_age_seconds}"
    )
}

fn auth_refresh_cookie(refresh_token: &str, max_age_seconds: u64) -> String {
    format!(
        "{AUTH_REFRESH_COOKIE_NAME}={refresh_token}; Path=/; HttpOnly; SameSite=Lax; Max-Age={max_age_seconds}"
    )
}

fn local_test_auth_cookie(max_age_seconds: u64) -> String {
    format!(
        "{LOCAL_TEST_AUTH_COOKIE_NAME}=1; Path=/; HttpOnly; SameSite=Lax; Max-Age={max_age_seconds}"
    )
}

fn clear_cookie(name: &str) -> String {
    format!("{name}=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0")
}

fn append_set_cookie_header(
    response: &mut Response,
    cookie: &str,
) -> Result<(), (StatusCode, Json<ApiErrorResponse>)> {
    response
        .headers_mut()
        .append(SET_COOKIE, header_value(cookie)?);
    Ok(())
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
    normalized == "/" || normalized == "/chat" || normalized.starts_with("/chat/")
}

fn is_retired_web_route(path: &str) -> bool {
    let normalized = path.trim();
    normalized == "/aui" || normalized.starts_with("/aui/")
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
        .with_attribute("route_domain", decision.route_domain.clone())
        .with_attribute("cohort_key", decision.cohort_key.clone())
        .with_attribute("user_agent", user_agent.to_string());

    if let Some(bucket) = decision.cohort_bucket {
        event = event.with_attribute("cohort_bucket", bucket.to_string());
    }

    if let Some(rollback_target) = decision.rollback_target {
        event = event.with_attribute(
            "rollback_target",
            match rollback_target {
                RouteTarget::Legacy => "legacy".to_string(),
                RouteTarget::RustShell => "rust_shell".to_string(),
            },
        );
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

fn access_token_from_headers(headers: &HeaderMap) -> Option<String> {
    bearer_token(headers).or_else(|| extract_cookie_value(headers, AUTH_ACCESS_COOKIE_NAME))
}

fn non_empty(value: String) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn auth_api_signup_is_enabled(config: &Config) -> bool {
    config.auth_api_signup_enabled && config.auth_provider_mode == "mock"
}

fn normalize_register_token_name(
    token_name: Option<String>,
    default_token_name: &str,
) -> Result<String, (StatusCode, Json<ApiErrorResponse>)> {
    let normalized = token_name
        .and_then(non_empty)
        .unwrap_or_else(|| default_token_name.trim().to_string());
    if normalized.chars().count() > 120 {
        return Err(validation_error(
            "tokenName",
            "Token name may not be greater than 120 characters.",
        ));
    }

    Ok(normalized)
}

fn normalize_register_token_abilities(
    token_abilities: Option<Vec<String>>,
) -> Result<Vec<String>, (StatusCode, Json<ApiErrorResponse>)> {
    let Some(values) = token_abilities else {
        return Ok(vec!["*".to_string()]);
    };

    let mut normalized = Vec::new();
    for value in values {
        let Some(ability) = non_empty(value) else {
            continue;
        };
        if ability.chars().count() > 120 {
            return Err(validation_error(
                "tokenAbilities",
                "Token abilities may not be greater than 120 characters.",
            ));
        }
        normalized.push(ability);
    }

    if normalized.is_empty() {
        Ok(vec!["*".to_string()])
    } else {
        Ok(normalized)
    }
}

fn normalize_optional_display_name(
    value: Option<String>,
    field: &'static str,
) -> Result<Option<String>, (StatusCode, Json<ApiErrorResponse>)> {
    let normalized = value.and_then(non_empty);
    if let Some(candidate) = normalized.as_deref() {
        if candidate.chars().count() > 120 {
            return Err(validation_error(
                field,
                "Value may not be greater than 120 characters.",
            ));
        }
    }

    Ok(normalized)
}

fn normalize_autopilot_handle_seed(
    value: Option<String>,
) -> Result<Option<String>, (StatusCode, Json<ApiErrorResponse>)> {
    let normalized = value.and_then(non_empty);
    let Some(handle) = normalized.as_ref() else {
        return Ok(None);
    };

    if handle.chars().count() > 64 {
        return Err(validation_error(
            "handle",
            "Handle may not be greater than 64 characters.",
        ));
    }

    let valid = handle
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || matches!(character, ':' | '_' | '-'));
    if !valid {
        return Err(validation_error("handle", "Handle format is invalid."));
    }

    Ok(normalized)
}

fn normalize_optional_bounded_string(
    value: Option<String>,
    field: &'static str,
    max_chars: usize,
) -> Result<Option<String>, (StatusCode, Json<ApiErrorResponse>)> {
    let normalized = value.and_then(non_empty);
    if let Some(candidate) = normalized.as_deref() {
        if candidate.chars().count() > max_chars {
            return Err(validation_error(
                field,
                &format!("Value may not be greater than {max_chars} characters."),
            ));
        }
    }
    Ok(normalized)
}

fn normalize_autopilot_enum(
    value: Option<String>,
    field: &'static str,
    allowed: &[&str],
) -> Result<Option<String>, (StatusCode, Json<ApiErrorResponse>)> {
    let normalized = value.and_then(non_empty);
    let Some(candidate) = normalized.as_deref() else {
        return Ok(None);
    };

    if !allowed
        .iter()
        .any(|allowed_value| allowed_value == &candidate)
    {
        return Err(validation_error(
            field,
            &format!("Value must be one of: {}.", allowed.join(", ")),
        ));
    }

    Ok(normalized)
}

fn validate_optional_json_array(
    value: Option<serde_json::Value>,
    field: &'static str,
) -> Result<Option<serde_json::Value>, (StatusCode, Json<ApiErrorResponse>)> {
    match value {
        Some(value) if !value.is_array() => Err(validation_error(field, "Value must be an array.")),
        Some(value) => Ok(Some(value)),
        None => Ok(None),
    }
}

fn validate_optional_string_list_max(
    values: Option<Vec<String>>,
    field: &'static str,
    max_chars: usize,
) -> Result<Option<Vec<String>>, (StatusCode, Json<ApiErrorResponse>)> {
    let Some(values) = values else {
        return Ok(None);
    };

    for value in &values {
        if value.chars().count() > max_chars {
            return Err(validation_error(
                field,
                &format!("Array entries may not be greater than {max_chars} characters."),
            ));
        }
    }

    Ok(Some(values))
}

fn normalize_autopilot_profile_update(
    payload: UpdateAutopilotProfilePayload,
) -> Result<UpsertAutopilotProfileInput, (StatusCode, Json<ApiErrorResponse>)> {
    let owner_display_name =
        normalize_optional_display_name(payload.owner_display_name, "profile.ownerDisplayName")?;
    let persona_summary = payload.persona_summary.and_then(non_empty);
    let autopilot_voice =
        normalize_optional_bounded_string(payload.autopilot_voice, "profile.autopilotVoice", 64)?;
    let principles = validate_optional_json_array(payload.principles, "profile.principles")?;
    let preferences = validate_optional_json_array(payload.preferences, "profile.preferences")?;
    let onboarding_answers =
        validate_optional_json_array(payload.onboarding_answers, "profile.onboardingAnswers")?;

    if matches!(payload.schema_version, Some(0)) {
        return Err(validation_error(
            "profile.schemaVersion",
            "Schema version must be greater than or equal to 1.",
        ));
    }

    Ok(UpsertAutopilotProfileInput {
        owner_display_name,
        persona_summary,
        autopilot_voice,
        principles,
        preferences,
        onboarding_answers,
        schema_version: payload.schema_version,
    })
}

fn normalize_autopilot_policy_update(
    payload: UpdateAutopilotPolicyPayload,
) -> Result<UpsertAutopilotPolicyInput, (StatusCode, Json<ApiErrorResponse>)> {
    let model_provider =
        normalize_optional_bounded_string(payload.model_provider, "policy.modelProvider", 64)?;
    let model = normalize_optional_bounded_string(payload.model, "policy.model", 128)?;
    let tool_allowlist =
        validate_optional_string_list_max(payload.tool_allowlist, "policy.toolAllowlist", 128)?;
    let tool_denylist =
        validate_optional_string_list_max(payload.tool_denylist, "policy.toolDenylist", 128)?;
    let l402_allowed_hosts = validate_optional_string_list_max(
        payload.l402_allowed_hosts,
        "policy.l402AllowedHosts",
        255,
    )?;
    let data_policy = validate_optional_json_array(payload.data_policy, "policy.dataPolicy")?;

    if matches!(payload.l402_max_spend_msats_per_call, Some(0)) {
        return Err(validation_error(
            "policy.l402MaxSpendMsatsPerCall",
            "Value must be greater than or equal to 1.",
        ));
    }
    if matches!(payload.l402_max_spend_msats_per_day, Some(0)) {
        return Err(validation_error(
            "policy.l402MaxSpendMsatsPerDay",
            "Value must be greater than or equal to 1.",
        ));
    }

    Ok(UpsertAutopilotPolicyInput {
        model_provider,
        model,
        tool_allowlist,
        tool_denylist,
        l402_require_approval: payload.l402_require_approval,
        l402_max_spend_msats_per_call: payload.l402_max_spend_msats_per_call,
        l402_max_spend_msats_per_day: payload.l402_max_spend_msats_per_day,
        l402_allowed_hosts,
        data_policy,
    })
}

fn normalize_autopilot_reference(
    autopilot: String,
) -> Result<String, (StatusCode, Json<ApiErrorResponse>)> {
    let reference = autopilot.trim().to_string();
    if reference.is_empty() {
        return Err(validation_error(
            "autopilot",
            "Autopilot reference is required.",
        ));
    }
    Ok(reference)
}

fn autopilot_update_field_count(
    has_display_name: bool,
    has_status: bool,
    has_visibility: bool,
    has_avatar: bool,
    has_tagline: bool,
    has_profile_update: bool,
    has_policy_update: bool,
) -> usize {
    let mut count = 0usize;
    if has_display_name {
        count += 1;
    }
    if has_status {
        count += 1;
    }
    if has_visibility {
        count += 1;
    }
    if has_avatar {
        count += 1;
    }
    if has_tagline {
        count += 1;
    }
    if has_profile_update {
        count += 1;
    }
    if has_policy_update {
        count += 1;
    }
    count
}

fn autopilot_aggregate_payload(aggregate: &AutopilotAggregate) -> serde_json::Value {
    serde_json::json!({
        "id": aggregate.autopilot.id.clone(),
        "handle": aggregate.autopilot.handle.clone(),
        "displayName": aggregate.autopilot.display_name.clone(),
        "status": aggregate.autopilot.status.clone(),
        "visibility": aggregate.autopilot.visibility.clone(),
        "ownerUserId": aggregate.autopilot.owner_user_id.clone(),
        "avatar": aggregate.autopilot.avatar.clone(),
        "tagline": aggregate.autopilot.tagline.clone(),
        "configVersion": aggregate.autopilot.config_version,
        "profile": {
            "ownerDisplayName": aggregate.profile.owner_display_name.clone(),
            "personaSummary": aggregate.profile.persona_summary.clone(),
            "autopilotVoice": aggregate.profile.autopilot_voice.clone(),
            "principles": aggregate.profile.principles.clone().unwrap_or_else(|| serde_json::Value::Array(Vec::new())),
            "preferences": aggregate.profile.preferences.clone().unwrap_or_else(|| serde_json::Value::Array(Vec::new())),
            "onboardingAnswers": if aggregate.profile.onboarding_answers.is_array() {
                aggregate.profile.onboarding_answers.clone()
            } else {
                serde_json::Value::Array(Vec::new())
            },
            "schemaVersion": aggregate.profile.schema_version,
        },
        "policy": {
            "modelProvider": aggregate.policy.model_provider.clone(),
            "model": aggregate.policy.model.clone(),
            "toolAllowlist": aggregate.policy.tool_allowlist.clone(),
            "toolDenylist": aggregate.policy.tool_denylist.clone(),
            "l402RequireApproval": aggregate.policy.l402_require_approval,
            "l402MaxSpendMsatsPerCall": aggregate.policy.l402_max_spend_msats_per_call,
            "l402MaxSpendMsatsPerDay": aggregate.policy.l402_max_spend_msats_per_day,
            "l402AllowedHosts": aggregate.policy.l402_allowed_hosts.clone(),
            "dataPolicy": aggregate.policy.data_policy.clone().unwrap_or_else(|| serde_json::Value::Array(Vec::new())),
        },
        "createdAt": timestamp(aggregate.autopilot.created_at),
        "updatedAt": timestamp(aggregate.autopilot.updated_at),
    })
}

fn autopilot_thread_payload(thread: &AutopilotThreadProjection) -> serde_json::Value {
    serde_json::json!({
        "id": thread.id,
        "autopilotId": thread.autopilot_id,
        "title": thread.title,
        "createdAt": timestamp(thread.created_at),
        "updatedAt": timestamp(thread.updated_at),
    })
}

fn user_handle_from_email(email: &str) -> String {
    let local = email.split('@').next().unwrap_or_default();
    let mut output = String::with_capacity(local.len().min(64));
    let mut previous_dash = false;
    for character in local.chars() {
        let normalized = character.to_ascii_lowercase();
        if normalized.is_ascii_alphanumeric() {
            output.push(normalized);
            previous_dash = false;
            continue;
        }

        if !previous_dash {
            output.push('-');
            previous_dash = true;
        }
    }

    let trimmed = output.trim_matches('-');
    if trimmed.is_empty() {
        "user".to_string()
    } else {
        trimmed.chars().take(64).collect()
    }
}

fn thread_title(thread_id: &str, message_count: u32) -> String {
    if message_count == 0 {
        return "New Chat".to_string();
    }

    let normalized = thread_id.trim();
    if normalized.is_empty() {
        "Chat".to_string()
    } else {
        format!("Thread {normalized}")
    }
}

fn parse_rfc3339_utc(value: &str) -> Result<chrono::DateTime<Utc>, chrono::ParseError> {
    chrono::DateTime::parse_from_rfc3339(value).map(|parsed| parsed.with_timezone(&Utc))
}

fn resolve_session_revocation_target(
    payload: &RevokeSessionsRequest,
) -> Result<SessionRevocationTarget, (StatusCode, Json<ApiErrorResponse>)> {
    let session_id = payload.session_id.clone().and_then(non_empty);
    let device_id = payload.device_id.clone().and_then(non_empty);
    let revoke_all = payload.revoke_all_sessions.unwrap_or(false);

    let mut count = 0u8;
    if session_id.is_some() {
        count += 1;
    }
    if device_id.is_some() {
        count += 1;
    }
    if revoke_all {
        count += 1;
    }

    if count != 1 {
        return Err(validation_error(
            "target",
            "Provide exactly one revocation target: session_id, device_id, or revoke_all_sessions=true.",
        ));
    }

    if let Some(value) = session_id {
        return Ok(SessionRevocationTarget::SessionId(value));
    }
    if let Some(value) = device_id {
        return Ok(SessionRevocationTarget::DeviceId(value));
    }

    Ok(SessionRevocationTarget::AllSessions)
}

fn revocation_reason_label(reason: SessionRevocationReason) -> &'static str {
    reason.as_str()
}

fn timestamp(value: chrono::DateTime<Utc>) -> String {
    value.to_rfc3339_opts(SecondsFormat::Secs, true)
}

fn header_value(raw: &str) -> Result<HeaderValue, (StatusCode, Json<ApiErrorResponse>)> {
    HeaderValue::from_str(raw).map_err(|_| {
        error_response_with_status(
            StatusCode::INTERNAL_SERVER_ERROR,
            ApiErrorCode::InternalError,
            "Failed to build response headers.".to_string(),
        )
    })
}

#[cfg(test)]
mod tests {
    use std::net::SocketAddr;
    use std::path::PathBuf;
    use std::sync::Arc;

    use anyhow::Result;
    use axum::body::Body;
    use axum::extract::State;
    use axum::http::HeaderMap;
    use axum::http::header::{CACHE_CONTROL, CONTENT_TYPE, SET_COOKIE};
    use axum::http::{HeaderValue, Request, StatusCode};
    use axum::routing::post;
    use axum::{Json, Router};
    use hmac::Mac;
    use http_body_util::BodyExt;
    use serde_json::{Value, json};
    use tempfile::tempdir;
    use tokio::net::TcpListener;
    use tokio::sync::Mutex;
    use tokio::task::JoinHandle;
    use tower::ServiceExt;

    use crate::build_router;
    use crate::build_router_with_observability;
    use crate::config::Config;
    use crate::observability::{Observability, RecordingAuditSink};
    use crate::{CACHE_IMMUTABLE_ONE_YEAR, CACHE_MANIFEST, MAINTENANCE_CACHE_CONTROL};

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
            auth_local_test_login_enabled: false,
            auth_local_test_login_allowed_emails: vec![],
            auth_local_test_login_signing_key: None,
            auth_api_signup_enabled: false,
            auth_api_signup_allowed_domains: vec![],
            auth_api_signup_default_token_name: "api-bootstrap".to_string(),
            admin_emails: vec![
                "chris@openagents.com".to_string(),
                "routes@openagents.com".to_string(),
            ],
            khala_token_enabled: true,
            khala_token_signing_key: Some("khala-test-signing-key".to_string()),
            khala_token_issuer: "https://openagents.test".to_string(),
            khala_token_audience: "openagents-khala-test".to_string(),
            khala_token_subject_prefix: "user".to_string(),
            khala_token_key_id: "khala-auth-test-v1".to_string(),
            khala_token_claims_version: "oa_khala_claims_v1".to_string(),
            khala_token_ttl_seconds: 300,
            khala_token_min_ttl_seconds: 60,
            khala_token_max_ttl_seconds: 900,
            auth_store_path: None,
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
            runtime_sync_revoke_base_url: None,
            runtime_sync_revoke_path: "/internal/v1/sync/sessions/revoke".to_string(),
            runtime_signature_secret: None,
            runtime_signature_ttl_seconds: 60,
            runtime_internal_shared_secret: None,
            runtime_internal_key_id: "runtime-internal-v1".to_string(),
            runtime_internal_signature_ttl_seconds: 60,
            codex_thread_store_path: None,
            domain_store_path: None,
            maintenance_mode_enabled: false,
            maintenance_bypass_token: None,
            maintenance_bypass_cookie_name: "oa_maintenance_bypass".to_string(),
            maintenance_bypass_cookie_ttl_seconds: 900,
            maintenance_allowed_paths: vec!["/healthz".to_string(), "/readyz".to_string()],
            compat_control_enforced: false,
            compat_control_protocol_version: "openagents.control.v1".to_string(),
            compat_control_min_client_build_id: "00000000T000000Z".to_string(),
            compat_control_max_client_build_id: None,
            compat_control_min_schema_version: 1,
            compat_control_max_schema_version: 1,
        }
    }

    fn workos_required_config(static_dir: PathBuf) -> Config {
        let mut config = test_config(static_dir);
        config.auth_provider_mode = "workos".to_string();
        config.workos_client_id = None;
        config.workos_api_key = None;
        config
    }

    fn compat_enforced_config(static_dir: PathBuf) -> Config {
        let mut config = test_config(static_dir);
        config.compat_control_enforced = true;
        config.compat_control_protocol_version = "openagents.control.v1".to_string();
        config.compat_control_min_client_build_id = "20260221T120000Z".to_string();
        config.compat_control_max_client_build_id = Some("20260221T180000Z".to_string());
        config.compat_control_min_schema_version = 1;
        config.compat_control_max_schema_version = 1;
        config
    }

    fn maintenance_enabled_config(static_dir: PathBuf) -> Config {
        let mut config = test_config(static_dir);
        config.maintenance_mode_enabled = true;
        config.maintenance_bypass_token = Some("maintenance-token".to_string());
        config.maintenance_bypass_cookie_name = "oa_maintenance_bypass".to_string();
        config.maintenance_bypass_cookie_ttl_seconds = 300;
        config.maintenance_allowed_paths = vec!["/healthz".to_string(), "/readyz".to_string()];
        config
    }

    fn test_app_state(config: Config) -> super::AppState {
        let auth = super::AuthService::from_config(&config);
        let route_split = super::RouteSplitService::from_config(&config);
        let khala_token_issuer = super::KhalaTokenIssuer::from_config(&config);
        let sync_token_issuer = super::SyncTokenIssuer::from_config(&config);
        let codex_thread_store = super::CodexThreadStore::from_config(&config);
        let domain_store = super::DomainStore::from_config(&config);
        let runtime_revocation_client = super::RuntimeRevocationClient::from_config(&config);
        super::AppState {
            config: Arc::new(config),
            auth,
            observability: Observability::default(),
            route_split,
            khala_token_issuer,
            sync_token_issuer,
            codex_thread_store,
            _domain_store: domain_store,
            runtime_revocation_client,
            throttle_state: super::ThrottleState::default(),
            codex_control_receipts: super::CodexControlReceiptState::default(),
            runtime_internal_nonces: super::RuntimeInternalNonceState::default(),
            started_at: std::time::SystemTime::now(),
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

    fn all_set_cookie_values(response: &axum::response::Response) -> Vec<String> {
        response
            .headers()
            .get_all(SET_COOKIE)
            .iter()
            .filter_map(|value| value.to_str().ok())
            .map(ToString::to_string)
            .collect()
    }

    fn cookie_from_set_cookie_header(set_cookie: &str) -> Option<String> {
        set_cookie.split(';').next().map(|value| value.to_string())
    }

    fn cookie_value_for_name(response: &axum::response::Response, name: &str) -> Option<String> {
        all_set_cookie_values(response)
            .into_iter()
            .filter_map(|set_cookie| cookie_from_set_cookie_header(&set_cookie))
            .find_map(|cookie| {
                let mut parts = cookie.splitn(2, '=');
                let key = parts.next().unwrap_or_default();
                let value = parts.next().unwrap_or_default();
                if key == name && !value.is_empty() {
                    Some(value.to_string())
                } else {
                    None
                }
            })
    }

    fn signed_test_login_url(
        signing_key: &str,
        email: &str,
        expires: i64,
        name: Option<&str>,
    ) -> String {
        let mut unsigned = format!("/internal/test-login?email={email}&expires={expires}");
        if let Some(value) = name {
            unsigned.push_str("&name=");
            unsigned.push_str(value);
        }

        let mut mac =
            super::HmacSha256::new_from_slice(signing_key.as_bytes()).expect("valid signing key");
        mac.update(unsigned.as_bytes());
        let signature = super::sha256_bytes_hex(&mac.finalize().into_bytes());
        format!("{unsigned}&signature={signature}")
    }

    async fn start_runtime_revocation_stub(
        captured: Arc<Mutex<Vec<Value>>>,
    ) -> Result<(SocketAddr, JoinHandle<()>)> {
        let app = Router::new()
            .route(
                "/internal/v1/sync/sessions/revoke",
                post(
                    |State(captured): State<Arc<Mutex<Vec<Value>>>>,
                     headers: HeaderMap,
                     Json(payload): Json<Value>| async move {
                        let signature = headers
                            .get("x-oa-runtime-signature")
                            .and_then(|value| value.to_str().ok())
                            .unwrap_or_default()
                            .to_string();

                        captured.lock().await.push(json!({
                            "signature": signature,
                            "payload": payload,
                        }));

                        (StatusCode::OK, Json(json!({"data": {"ok": true}})))
                    },
                ),
            )
            .with_state(captured);

        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let addr = listener.local_addr()?;

        let handle = tokio::spawn(async move {
            axum::serve(listener, app.into_make_service())
                .await
                .expect("runtime revocation stub server failed");
        });

        Ok((addr, handle))
    }

    async fn authenticate_token(app: Router, email: &str) -> Result<String> {
        let send_request = Request::builder()
            .method("POST")
            .uri("/api/auth/email")
            .header("content-type", "application/json")
            .body(Body::from(json!({ "email": email }).to_string()))?;
        let send_response = app.clone().oneshot(send_request).await?;
        let cookie = cookie_value(&send_response).unwrap_or_default();

        let verify_request = Request::builder()
            .method("POST")
            .uri("/api/auth/verify")
            .header("content-type", "application/json")
            .header("x-client", "autopilot-ios")
            .header("cookie", cookie)
            .body(Body::from(r#"{"code":"123456"}"#))?;
        let verify_response = app.oneshot(verify_request).await?;
        let verify_body = read_json(verify_response).await?;

        Ok(verify_body["token"]
            .as_str()
            .unwrap_or_default()
            .to_string())
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
    async fn auth_email_route_enforces_throttle_limit() -> Result<()> {
        let app = build_router(test_config(std::env::temp_dir()));

        for _ in 0..super::THROTTLE_AUTH_EMAIL_LIMIT {
            let request = Request::builder()
                .method("POST")
                .uri("/api/auth/email")
                .header("content-type", "application/json")
                .header("x-forwarded-for", "203.0.113.10")
                .body(Body::from(r#"{"email":"throttle@openagents.com"}"#))?;
            let response = app.clone().oneshot(request).await?;
            assert_eq!(response.status(), StatusCode::OK);
        }

        let exceeded = Request::builder()
            .method("POST")
            .uri("/api/auth/email")
            .header("content-type", "application/json")
            .header("x-forwarded-for", "203.0.113.10")
            .body(Body::from(r#"{"email":"throttle@openagents.com"}"#))?;
        let response = app.oneshot(exceeded).await?;
        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
        let body = read_json(response).await?;
        assert_eq!(body["error"]["code"], "rate_limited");

        Ok(())
    }

    #[tokio::test]
    async fn thread_message_route_enforces_throttle_limit() -> Result<()> {
        let app = build_router(test_config(std::env::temp_dir()));
        let token = authenticate_token(app.clone(), "thread-throttle@openagents.com").await?;

        for index in 0..super::THROTTLE_THREAD_MESSAGE_LIMIT {
            let request = Request::builder()
                .method("POST")
                .uri("/api/runtime/threads/thread-1/messages")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {token}"))
                .header("x-forwarded-for", "198.51.100.22")
                .body(Body::from(format!(r#"{{"text":"message-{index}"}}"#)))?;
            let response = app.clone().oneshot(request).await?;
            assert_eq!(response.status(), StatusCode::OK);
        }

        let exceeded = Request::builder()
            .method("POST")
            .uri("/api/runtime/threads/thread-1/messages")
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {token}"))
            .header("x-forwarded-for", "198.51.100.22")
            .body(Body::from(r#"{"text":"over-limit"}"#))?;
        let response = app.oneshot(exceeded).await?;
        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
        let body = read_json(response).await?;
        assert_eq!(body["error"]["code"], "rate_limited");

        Ok(())
    }

    #[tokio::test]
    async fn route_split_override_requires_admin_email() -> Result<()> {
        let static_dir = tempdir()?;
        std::fs::write(
            static_dir.path().join("index.html"),
            "<!doctype html><html><body>rust shell</body></html>",
        )?;

        let mut config = test_config(static_dir.path().to_path_buf());
        config.admin_emails = vec!["admin@openagents.com".to_string()];
        let app = build_router(config);
        let token = authenticate_token(app.clone(), "not-admin@openagents.com").await?;

        let request = Request::builder()
            .method("POST")
            .uri("/api/v1/control/route-split/override")
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {token}"))
            .body(Body::from(r#"{"target":"legacy"}"#))?;
        let response = app.oneshot(request).await?;
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        let body = read_json(response).await?;
        assert_eq!(body["error"]["code"], "forbidden");

        Ok(())
    }

    #[tokio::test]
    async fn runtime_internal_signature_validation_rejects_nonce_replay() -> Result<()> {
        let mut config = test_config(std::env::temp_dir());
        config.runtime_internal_shared_secret = Some("runtime-internal-secret".to_string());
        config.runtime_internal_key_id = "runtime-internal-v1".to_string();
        config.runtime_internal_signature_ttl_seconds = 60;
        let state = test_app_state(config);

        let body = br#"{"provider":"resend","integration_id":"int_runtime"}"#;
        let timestamp = chrono::Utc::now().timestamp().to_string();
        let nonce = "nonce-runtime-internal-1";
        let body_hash = super::sha256_hex(body);
        let signing_payload = format!("{timestamp}\n{nonce}\n{body_hash}");

        let mut mac =
            super::HmacSha256::new_from_slice(b"runtime-internal-secret").expect("hmac key");
        mac.update(signing_payload.as_bytes());
        let signature = super::sha256_bytes_hex(&mac.finalize().into_bytes());

        let mut headers = HeaderMap::new();
        headers.insert(
            super::RUNTIME_INTERNAL_KEY_ID_HEADER,
            HeaderValue::from_static("runtime-internal-v1"),
        );
        headers.insert(
            super::RUNTIME_INTERNAL_TIMESTAMP_HEADER,
            HeaderValue::from_str(&timestamp)?,
        );
        headers.insert(
            super::RUNTIME_INTERNAL_NONCE_HEADER,
            HeaderValue::from_static(nonce),
        );
        headers.insert(
            super::RUNTIME_INTERNAL_BODY_HASH_HEADER,
            HeaderValue::from_str(&body_hash)?,
        );
        headers.insert(
            super::RUNTIME_INTERNAL_SIGNATURE_HEADER,
            HeaderValue::from_str(&signature)?,
        );

        let first = super::verify_runtime_internal_headers(&state, &headers, body).await;
        assert!(first.is_ok());

        let replay = super::verify_runtime_internal_headers(&state, &headers, body)
            .await
            .expect_err("expected nonce replay rejection");
        assert_eq!(replay.0, StatusCode::UNAUTHORIZED);
        assert_eq!(replay.1.0.error.code, "unauthorized");
        assert!(
            replay.1.0.error.message.contains("nonce replay"),
            "unexpected replay message: {}",
            replay.1.0.error.message
        );

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
    async fn maintenance_mode_blocks_non_allowed_routes_with_503() -> Result<()> {
        let static_dir = tempdir()?;
        std::fs::write(
            static_dir.path().join("index.html"),
            "<!doctype html><html><body>rust shell</body></html>",
        )?;
        let app = build_router(maintenance_enabled_config(static_dir.path().to_path_buf()));

        let request = Request::builder().uri("/").body(Body::empty())?;
        let response = app.oneshot(request).await?;
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(
            response
                .headers()
                .get(CACHE_CONTROL)
                .and_then(|value| value.to_str().ok()),
            Some(MAINTENANCE_CACHE_CONTROL)
        );

        let body = response.into_body().collect().await?.to_bytes();
        let html = String::from_utf8_lossy(&body);
        assert!(html.contains("Maintenance in progress"));
        Ok(())
    }

    #[tokio::test]
    async fn maintenance_mode_allows_health_and_readiness_routes() -> Result<()> {
        let static_dir = tempdir()?;
        let app = build_router(maintenance_enabled_config(static_dir.path().to_path_buf()));

        let health = Request::builder().uri("/healthz").body(Body::empty())?;
        let health_response = app.clone().oneshot(health).await?;
        assert_eq!(health_response.status(), StatusCode::OK);

        let ready = Request::builder().uri("/readyz").body(Body::empty())?;
        let ready_response = app.oneshot(ready).await?;
        assert_eq!(ready_response.status(), StatusCode::OK);
        Ok(())
    }

    #[tokio::test]
    async fn maintenance_mode_valid_bypass_sets_cookie_and_redirects() -> Result<()> {
        let static_dir = tempdir()?;
        std::fs::write(
            static_dir.path().join("index.html"),
            "<!doctype html><html><body>rust shell</body></html>",
        )?;
        let app = build_router(maintenance_enabled_config(static_dir.path().to_path_buf()));

        let request = Request::builder()
            .uri("/workspace?maintenance_bypass=maintenance-token")
            .body(Body::empty())?;
        let response = app.clone().oneshot(request).await?;
        assert_eq!(response.status(), StatusCode::TEMPORARY_REDIRECT);
        assert_eq!(
            response
                .headers()
                .get("location")
                .and_then(|value| value.to_str().ok()),
            Some("/workspace")
        );

        let set_cookie = response
            .headers()
            .get(SET_COOKIE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default()
            .to_string();
        assert!(set_cookie.contains("oa_maintenance_bypass="));
        assert!(set_cookie.contains("Secure"));
        assert!(set_cookie.contains("HttpOnly"));
        assert!(set_cookie.contains("Max-Age=300"));

        let cookie = set_cookie.split(';').next().unwrap_or_default().to_string();
        assert!(!cookie.contains("maintenance-token"));

        let follow_request = Request::builder()
            .uri("/workspace")
            .header("cookie", cookie)
            .body(Body::empty())?;
        let follow_response = app.oneshot(follow_request).await?;
        assert_eq!(follow_response.status(), StatusCode::OK);
        Ok(())
    }

    #[tokio::test]
    async fn maintenance_mode_invalid_bypass_token_does_not_grant_access() -> Result<()> {
        let static_dir = tempdir()?;
        std::fs::write(
            static_dir.path().join("index.html"),
            "<!doctype html><html><body>rust shell</body></html>",
        )?;
        let app = build_router(maintenance_enabled_config(static_dir.path().to_path_buf()));

        let request = Request::builder()
            .uri("/?maintenance_bypass=bad-token")
            .body(Body::empty())?;
        let response = app.oneshot(request).await?;
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert!(response.headers().get(SET_COOKIE).is_none());
        Ok(())
    }

    #[tokio::test]
    async fn maintenance_cookie_validation_enforces_signature_and_ttl() -> Result<()> {
        let token = "maintenance-token";
        let now = chrono::Utc::now().timestamp().max(0) as u64;

        let valid = super::maintenance_bypass_cookie_payload(token, now + 300).unwrap_or_default();
        assert!(super::maintenance_cookie_is_valid(&valid, token));

        let expired = super::maintenance_bypass_cookie_payload(token, now.saturating_sub(1))
            .unwrap_or_default();
        assert!(!super::maintenance_cookie_is_valid(&expired, token));
        assert!(!super::maintenance_cookie_is_valid("invalid", token));
        Ok(())
    }

    #[tokio::test]
    async fn maintenance_allowed_paths_can_include_control_endpoints() -> Result<()> {
        let static_dir = tempdir()?;
        let mut config = maintenance_enabled_config(static_dir.path().to_path_buf());
        config
            .maintenance_allowed_paths
            .push("/api/v1/control/status".to_string());
        let app = build_router(config);

        let request = Request::builder()
            .uri("/api/v1/control/status")
            .body(Body::empty())?;
        let response = app.oneshot(request).await?;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
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
    async fn service_worker_script_uses_no_store_cache_header() -> Result<()> {
        let static_dir = tempdir()?;
        std::fs::write(
            static_dir.path().join("sw.js"),
            "self.addEventListener('install', () => {});",
        )?;
        let app = build_router(test_config(static_dir.path().to_path_buf()));

        let request = Request::builder().uri("/sw.js").body(Body::empty())?;
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
    async fn openapi_route_serves_generated_minified_json() -> Result<()> {
        let app = build_router(test_config(std::env::temp_dir()));

        let request = Request::builder()
            .uri(super::ROUTE_OPENAPI_JSON)
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
        let content_type = response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default();
        assert!(content_type.starts_with("application/json"));

        let body = response.into_body().collect().await?.to_bytes();
        let body_text = String::from_utf8(body.to_vec())?;
        assert!(body_text.starts_with("{\"openapi\":\"3.0.2\""));
        assert!(!body_text.contains('\n'));

        let parsed = serde_json::from_str::<Value>(&body_text)?;
        assert_eq!(parsed["openapi"], "3.0.2");
        assert!(parsed["paths"]["/api/auth/email"].is_object());
        assert!(parsed["components"]["securitySchemes"]["bearerAuth"].is_object());

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
    async fn compatibility_gate_rejects_missing_client_version_headers() -> Result<()> {
        let static_dir = tempdir()?;
        let app = build_router(compat_enforced_config(static_dir.path().to_path_buf()));

        let request = Request::builder()
            .uri("/api/v1/control/status")
            .body(Body::empty())?;
        let response = app.oneshot(request).await?;

        assert_eq!(response.status(), StatusCode::UPGRADE_REQUIRED);
        assert_eq!(
            response
                .headers()
                .get(super::HEADER_OA_COMPAT_CODE)
                .and_then(|value| value.to_str().ok()),
            Some("invalid_client_build")
        );
        assert_eq!(
            response
                .headers()
                .get(super::HEADER_OA_COMPAT_UPGRADE_REQUIRED)
                .and_then(|value| value.to_str().ok()),
            Some("true")
        );
        let body = read_json(response).await?;
        assert_eq!(body["error"]["code"], "invalid_client_build");
        assert_eq!(body["compatibility"]["upgrade_required"], true);
        Ok(())
    }

    #[tokio::test]
    async fn compatibility_gate_rejects_client_below_minimum_build() -> Result<()> {
        let static_dir = tempdir()?;
        let app = build_router(compat_enforced_config(static_dir.path().to_path_buf()));

        let request = Request::builder()
            .uri("/api/v1/control/status")
            .header("x-oa-client-build-id", "20260221T110000Z")
            .header("x-oa-protocol-version", "openagents.control.v1")
            .header("x-oa-schema-version", "1")
            .body(Body::empty())?;
        let response = app.oneshot(request).await?;

        assert_eq!(response.status(), StatusCode::UPGRADE_REQUIRED);
        let body = read_json(response).await?;
        assert_eq!(body["error"]["code"], "upgrade_required");
        assert_eq!(
            body["compatibility"]["min_client_build_id"],
            "20260221T120000Z"
        );
        Ok(())
    }

    #[tokio::test]
    async fn compatibility_gate_allows_supported_client_version() -> Result<()> {
        let static_dir = tempdir()?;
        let app = build_router(compat_enforced_config(static_dir.path().to_path_buf()));

        let request = Request::builder()
            .uri("/api/v1/control/status")
            .header("x-oa-client-build-id", "20260221T130000Z")
            .header("x-oa-protocol-version", "openagents.control.v1")
            .header("x-oa-schema-version", "1")
            .body(Body::empty())?;
        let response = app.oneshot(request).await?;

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        Ok(())
    }

    #[tokio::test]
    async fn compatibility_gate_skips_auth_bootstrap_routes() -> Result<()> {
        let static_dir = tempdir()?;
        let app = build_router(compat_enforced_config(static_dir.path().to_path_buf()));

        let request = Request::builder()
            .method("POST")
            .uri("/api/auth/email")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"email":"compat-skip@openagents.com"}"#))?;
        let response = app.oneshot(request).await?;

        assert_eq!(response.status(), StatusCode::OK);
        Ok(())
    }

    #[tokio::test]
    async fn compatibility_rejections_emit_audit_with_surface_and_build() -> Result<()> {
        let static_dir = tempdir()?;
        let sink = RecordingAuditSink::default();
        let app = build_router_with_observability(
            compat_enforced_config(static_dir.path().to_path_buf()),
            Observability::new(Arc::new(sink.clone())),
        );

        let request = Request::builder()
            .uri("/api/v1/control/status")
            .header("x-client", "autopilot-ios")
            .header("x-oa-client-build-id", "20260221T110000Z")
            .header("x-oa-protocol-version", "openagents.control.v1")
            .header("x-oa-schema-version", "1")
            .body(Body::empty())?;

        let response = app.oneshot(request).await?;
        assert_eq!(response.status(), StatusCode::UPGRADE_REQUIRED);

        let events = sink.events();
        let compat_event = events
            .iter()
            .find(|event| event.event_name == "compatibility.rejected")
            .expect("missing compatibility rejection audit event");

        assert_eq!(
            compat_event.attributes.get("surface").map(String::as_str),
            Some("control_api")
        );
        assert_eq!(
            compat_event.attributes.get("client").map(String::as_str),
            Some("autopilot-ios")
        );
        assert_eq!(
            compat_event
                .attributes
                .get("client_build_id")
                .map(String::as_str),
            Some("20260221T110000Z")
        );

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
    async fn auth_register_is_not_found_when_disabled() -> Result<()> {
        let app = build_router(test_config(std::env::temp_dir()));

        let request = Request::builder()
            .method("POST")
            .uri("/api/auth/register")
            .header("content-type", "application/json")
            .body(Body::from(
                r#"{"email":"staging-user-1@staging.openagents.com"}"#,
            ))?;

        let response = app.oneshot(request).await?;
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        Ok(())
    }

    #[tokio::test]
    async fn auth_register_creates_user_and_returns_pat_when_enabled() -> Result<()> {
        let mut config = test_config(std::env::temp_dir());
        config.auth_api_signup_enabled = true;
        let app = build_router(config);

        let create_request = Request::builder()
            .method("POST")
            .uri("/api/auth/register")
            .header("content-type", "application/json")
            .body(Body::from(
                r#"{"email":"staging-user-1@staging.openagents.com","name":"Staging User 1","tokenName":"staging-e2e"}"#,
            ))?;
        let create_response = app.clone().oneshot(create_request).await?;
        assert_eq!(create_response.status(), StatusCode::CREATED);
        let create_body = read_json(create_response).await?;
        assert_eq!(create_body["data"]["created"], true);
        assert_eq!(
            create_body["data"]["user"]["email"],
            "staging-user-1@staging.openagents.com"
        );
        assert_eq!(create_body["data"]["tokenName"], "staging-e2e");

        let token = create_body["data"]["token"]
            .as_str()
            .unwrap_or_default()
            .to_string();
        assert!(!token.is_empty());

        let second_request = Request::builder()
            .method("POST")
            .uri("/api/auth/register")
            .header("content-type", "application/json")
            .body(Body::from(
                r#"{"email":"staging-user-1@staging.openagents.com","name":"Updated Name"}"#,
            ))?;
        let second_response = app.oneshot(second_request).await?;
        assert_eq!(second_response.status(), StatusCode::OK);
        let second_body = read_json(second_response).await?;
        assert_eq!(second_body["data"]["created"], false);
        assert_eq!(second_body["data"]["user"]["name"], "Updated Name");

        Ok(())
    }

    #[tokio::test]
    async fn auth_register_enforces_allowed_domains_and_can_create_autopilot() -> Result<()> {
        let mut config = test_config(std::env::temp_dir());
        config.auth_api_signup_enabled = true;
        config.auth_api_signup_allowed_domains = vec!["staging.openagents.com".to_string()];
        let app = build_router(config);

        let blocked_request = Request::builder()
            .method("POST")
            .uri("/api/auth/register")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"email":"blocked@example.com"}"#))?;
        let blocked_response = app.clone().oneshot(blocked_request).await?;
        assert_eq!(blocked_response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        let blocked_body = read_json(blocked_response).await?;
        assert_eq!(blocked_body["error"]["code"], "invalid_request");

        let allowed_request = Request::builder()
            .method("POST")
            .uri("/api/auth/register")
            .header("content-type", "application/json")
            .body(Body::from(
                r#"{"email":"creator@staging.openagents.com","createAutopilot":true,"autopilotDisplayName":"Creator Agent"}"#,
            ))?;
        let allowed_response = app.oneshot(allowed_request).await?;
        assert_eq!(allowed_response.status(), StatusCode::CREATED);
        let allowed_body = read_json(allowed_response).await?;
        assert_eq!(
            allowed_body["data"]["autopilot"]["displayName"],
            "Creator Agent"
        );
        assert!(allowed_body["data"]["autopilot"]["id"].as_str().is_some());

        Ok(())
    }

    #[tokio::test]
    async fn login_page_redirects_home_when_already_authenticated() -> Result<()> {
        let app = build_router(test_config(std::env::temp_dir()));

        let send_request = Request::builder()
            .method("POST")
            .uri("/api/auth/email")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"email":"already-authed@example.com"}"#))?;
        let send_response = app.clone().oneshot(send_request).await?;
        let challenge_cookie = cookie_value(&send_response).unwrap_or_default();

        let verify_request = Request::builder()
            .method("POST")
            .uri("/api/auth/verify")
            .header("content-type", "application/json")
            .header("cookie", challenge_cookie)
            .body(Body::from(r#"{"code":"123456"}"#))?;
        let verify_response = app.clone().oneshot(verify_request).await?;
        let verify_body = read_json(verify_response).await?;
        let access_token = verify_body["token"]
            .as_str()
            .unwrap_or_default()
            .to_string();

        let login_request = Request::builder()
            .uri("/login")
            .header("authorization", format!("Bearer {access_token}"))
            .body(Body::empty())?;
        let login_response = app.oneshot(login_request).await?;
        assert_eq!(login_response.status(), StatusCode::TEMPORARY_REDIRECT);
        assert_eq!(
            login_response
                .headers()
                .get("location")
                .and_then(|value| value.to_str().ok()),
            Some("/")
        );

        Ok(())
    }

    #[tokio::test]
    async fn web_login_email_and_verify_routes_set_auth_cookies() -> Result<()> {
        let app = build_router(test_config(std::env::temp_dir()));

        let send_request = Request::builder()
            .method("POST")
            .uri("/login/email")
            .header("content-type", "application/x-www-form-urlencoded")
            .body(Body::from("email=web-login%40openagents.com"))?;
        let send_response = app.clone().oneshot(send_request).await?;
        assert_eq!(send_response.status(), StatusCode::TEMPORARY_REDIRECT);
        assert_eq!(
            send_response
                .headers()
                .get("location")
                .and_then(|value| value.to_str().ok()),
            Some("/login?status=code-sent")
        );
        let challenge_cookie = cookie_value_for_name(&send_response, super::CHALLENGE_COOKIE_NAME)
            .expect("missing challenge cookie");

        let verify_request = Request::builder()
            .method("POST")
            .uri("/login/verify")
            .header("content-type", "application/x-www-form-urlencoded")
            .header(
                "cookie",
                format!("{}={challenge_cookie}", super::CHALLENGE_COOKIE_NAME),
            )
            .body(Body::from("code=123456"))?;
        let verify_response = app.clone().oneshot(verify_request).await?;
        assert_eq!(verify_response.status(), StatusCode::TEMPORARY_REDIRECT);
        assert_eq!(
            verify_response
                .headers()
                .get("location")
                .and_then(|value| value.to_str().ok()),
            Some("/")
        );

        let set_cookies = all_set_cookie_values(&verify_response);
        assert!(
            set_cookies
                .iter()
                .any(|value| value.starts_with(&format!("{}=", super::AUTH_ACCESS_COOKIE_NAME)))
        );
        assert!(
            set_cookies
                .iter()
                .any(|value| value.starts_with(&format!("{}=", super::AUTH_REFRESH_COOKIE_NAME)))
        );
        assert!(
            set_cookies
                .iter()
                .any(|value| value.starts_with(&format!("{}=;", super::CHALLENGE_COOKIE_NAME)))
        );

        let access_cookie = cookie_value_for_name(&verify_response, super::AUTH_ACCESS_COOKIE_NAME)
            .expect("missing access cookie");
        let login_request = Request::builder()
            .uri("/login")
            .header(
                "cookie",
                format!("{}={access_cookie}", super::AUTH_ACCESS_COOKIE_NAME),
            )
            .body(Body::empty())?;
        let login_response = app.oneshot(login_request).await?;
        assert_eq!(login_response.status(), StatusCode::TEMPORARY_REDIRECT);
        assert_eq!(
            login_response
                .headers()
                .get("location")
                .and_then(|value| value.to_str().ok()),
            Some("/")
        );

        Ok(())
    }

    #[tokio::test]
    async fn web_logout_clears_auth_cookies_and_redirects() -> Result<()> {
        let app = build_router(test_config(std::env::temp_dir()));

        let send_request = Request::builder()
            .method("POST")
            .uri("/login/email")
            .header("content-type", "application/x-www-form-urlencoded")
            .body(Body::from("email=logout-user%40openagents.com"))?;
        let send_response = app.clone().oneshot(send_request).await?;
        let challenge_cookie = cookie_value_for_name(&send_response, super::CHALLENGE_COOKIE_NAME)
            .expect("missing challenge cookie");

        let verify_request = Request::builder()
            .method("POST")
            .uri("/login/verify")
            .header("content-type", "application/x-www-form-urlencoded")
            .header(
                "cookie",
                format!("{}={challenge_cookie}", super::CHALLENGE_COOKIE_NAME),
            )
            .body(Body::from("code=123456"))?;
        let verify_response = app.clone().oneshot(verify_request).await?;
        let access_cookie = cookie_value_for_name(&verify_response, super::AUTH_ACCESS_COOKIE_NAME)
            .expect("missing access cookie");
        let refresh_cookie =
            cookie_value_for_name(&verify_response, super::AUTH_REFRESH_COOKIE_NAME)
                .expect("missing refresh cookie");

        let logout_request = Request::builder()
            .method("POST")
            .uri("/logout")
            .header(
                "cookie",
                format!(
                    "{}={access_cookie}; {}={refresh_cookie}",
                    super::AUTH_ACCESS_COOKIE_NAME,
                    super::AUTH_REFRESH_COOKIE_NAME
                ),
            )
            .body(Body::empty())?;
        let logout_response = app.oneshot(logout_request).await?;
        assert_eq!(logout_response.status(), StatusCode::TEMPORARY_REDIRECT);
        assert_eq!(
            logout_response
                .headers()
                .get("location")
                .and_then(|value| value.to_str().ok()),
            Some("/")
        );

        let set_cookies = all_set_cookie_values(&logout_response);
        assert!(
            set_cookies
                .iter()
                .any(|value| value.starts_with(&format!("{}=;", super::AUTH_ACCESS_COOKIE_NAME)))
        );
        assert!(
            set_cookies
                .iter()
                .any(|value| value.starts_with(&format!("{}=;", super::AUTH_REFRESH_COOKIE_NAME)))
        );

        Ok(())
    }

    #[tokio::test]
    async fn local_test_login_route_enforces_gates_and_accepts_valid_signature() -> Result<()> {
        let mut config = test_config(std::env::temp_dir());
        config.auth_local_test_login_enabled = true;
        config.auth_local_test_login_allowed_emails = vec!["tester@openagents.com".to_string()];
        config.auth_local_test_login_signing_key = Some("local-test-signing-key".to_string());
        let app = build_router(config);

        let unsigned_request = Request::builder()
            .uri("/internal/test-login?email=tester@openagents.com&expires=4102444800")
            .body(Body::empty())?;
        let unsigned_response = app.clone().oneshot(unsigned_request).await?;
        assert_eq!(unsigned_response.status(), StatusCode::FORBIDDEN);

        let blocked_url = signed_test_login_url(
            "local-test-signing-key",
            "blocked@example.com",
            4_102_444_800,
            Some("MaintenanceTester"),
        );
        let blocked_request = Request::builder().uri(blocked_url).body(Body::empty())?;
        let blocked_response = app.clone().oneshot(blocked_request).await?;
        assert_eq!(blocked_response.status(), StatusCode::FORBIDDEN);

        let allowed_url = signed_test_login_url(
            "local-test-signing-key",
            "tester@openagents.com",
            4_102_444_800,
            Some("MaintenanceTester"),
        );
        let allowed_request = Request::builder().uri(allowed_url).body(Body::empty())?;
        let allowed_response = app.oneshot(allowed_request).await?;
        assert_eq!(allowed_response.status(), StatusCode::TEMPORARY_REDIRECT);
        assert_eq!(
            allowed_response
                .headers()
                .get("location")
                .and_then(|value| value.to_str().ok()),
            Some("/")
        );
        let set_cookies = all_set_cookie_values(&allowed_response);
        assert!(
            set_cookies
                .iter()
                .any(|value| value.starts_with(&format!("{}=", super::AUTH_ACCESS_COOKIE_NAME)))
        );
        assert!(set_cookies.iter().any(|value| {
            value.starts_with(&format!("{}=1", super::LOCAL_TEST_AUTH_COOKIE_NAME))
        }));

        Ok(())
    }

    #[tokio::test]
    async fn local_test_login_route_returns_not_found_when_disabled() -> Result<()> {
        let mut config = test_config(std::env::temp_dir());
        config.auth_local_test_login_enabled = false;
        config.auth_local_test_login_allowed_emails = vec!["tester@openagents.com".to_string()];
        config.auth_local_test_login_signing_key = Some("local-test-signing-key".to_string());
        let app = build_router(config);

        let request = Request::builder()
            .uri(signed_test_login_url(
                "local-test-signing-key",
                "tester@openagents.com",
                4_102_444_800,
                Some("MaintenanceTester"),
            ))
            .body(Body::empty())?;
        let response = app.oneshot(request).await?;
        assert_eq!(response.status(), StatusCode::NOT_FOUND);

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
    async fn refresh_token_is_single_use_and_replay_revokes_session() -> Result<()> {
        let app = build_router(test_config(std::env::temp_dir()));

        let send_request = Request::builder()
            .method("POST")
            .uri("/api/auth/email")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"email":"refresh-replay@example.com"}"#))?;
        let send_response = app.clone().oneshot(send_request).await?;
        let cookie = cookie_value(&send_response).unwrap_or_default();

        let verify_request = Request::builder()
            .method("POST")
            .uri("/api/auth/verify")
            .header("content-type", "application/json")
            .header("x-client", "autopilot-ios")
            .header("x-device-id", "ios-replay-device")
            .header("cookie", cookie)
            .body(Body::from(r#"{"code":"123456"}"#))?;
        let verify_response = app.clone().oneshot(verify_request).await?;
        assert_eq!(verify_response.status(), StatusCode::OK);
        let verify_body = read_json(verify_response).await?;
        let refresh_token = verify_body["refreshToken"]
            .as_str()
            .unwrap_or_default()
            .to_string();

        let rotate_request = Request::builder()
            .method("POST")
            .uri("/api/auth/refresh")
            .header("content-type", "application/json")
            .header("x-device-id", "ios-replay-device")
            .body(Body::from(format!(
                r#"{{"refresh_token":"{refresh_token}","rotate_refresh_token":true,"device_id":"ios-replay-device"}}"#
            )))?;
        let rotate_response = app.clone().oneshot(rotate_request).await?;
        assert_eq!(rotate_response.status(), StatusCode::OK);
        let rotate_body = read_json(rotate_response).await?;
        let rotated_access_token = rotate_body["token"]
            .as_str()
            .unwrap_or_default()
            .to_string();

        let replay_request = Request::builder()
            .method("POST")
            .uri("/api/auth/refresh")
            .header("content-type", "application/json")
            .header("x-device-id", "ios-replay-device")
            .body(Body::from(format!(
                r#"{{"refresh_token":"{refresh_token}","rotate_refresh_token":true,"device_id":"ios-replay-device"}}"#
            )))?;
        let replay_response = app.clone().oneshot(replay_request).await?;
        assert_eq!(replay_response.status(), StatusCode::UNAUTHORIZED);

        let session_after_replay = Request::builder()
            .uri("/api/auth/session")
            .header("authorization", format!("Bearer {rotated_access_token}"))
            .body(Body::empty())?;
        let session_after_replay_response = app.oneshot(session_after_replay).await?;
        assert_eq!(
            session_after_replay_response.status(),
            StatusCode::UNAUTHORIZED
        );

        Ok(())
    }

    #[tokio::test]
    async fn session_listing_and_device_revocation_are_supported() -> Result<()> {
        let app = build_router(test_config(std::env::temp_dir()));

        let send_a = Request::builder()
            .method("POST")
            .uri("/api/auth/email")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"email":"device-revoke@example.com"}"#))?;
        let send_a_response = app.clone().oneshot(send_a).await?;
        let cookie_a = cookie_value(&send_a_response).unwrap_or_default();

        let verify_a = Request::builder()
            .method("POST")
            .uri("/api/auth/verify")
            .header("content-type", "application/json")
            .header("x-client", "autopilot-ios")
            .header("x-device-id", "ios-device-a")
            .header("cookie", cookie_a)
            .body(Body::from(r#"{"code":"123456"}"#))?;
        let verify_a_response = app.clone().oneshot(verify_a).await?;
        let verify_a_body = read_json(verify_a_response).await?;
        let token_a = verify_a_body["token"]
            .as_str()
            .unwrap_or_default()
            .to_string();

        let send_b = Request::builder()
            .method("POST")
            .uri("/api/auth/email")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"email":"device-revoke@example.com"}"#))?;
        let send_b_response = app.clone().oneshot(send_b).await?;
        let cookie_b = cookie_value(&send_b_response).unwrap_or_default();

        let verify_b = Request::builder()
            .method("POST")
            .uri("/api/auth/verify")
            .header("content-type", "application/json")
            .header("x-client", "autopilot-ios")
            .header("x-device-id", "ios-device-b")
            .header("cookie", cookie_b)
            .body(Body::from(r#"{"code":"123456"}"#))?;
        let verify_b_response = app.clone().oneshot(verify_b).await?;
        let verify_b_body = read_json(verify_b_response).await?;
        let token_b = verify_b_body["token"]
            .as_str()
            .unwrap_or_default()
            .to_string();

        let list_request = Request::builder()
            .uri("/api/auth/sessions")
            .header("authorization", format!("Bearer {token_a}"))
            .body(Body::empty())?;
        let list_response = app.clone().oneshot(list_request).await?;
        assert_eq!(list_response.status(), StatusCode::OK);
        let list_body = read_json(list_response).await?;
        let sessions = list_body["data"]["sessions"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        assert_eq!(sessions.len(), 2);

        let revoke_request = Request::builder()
            .method("POST")
            .uri("/api/auth/sessions/revoke")
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {token_a}"))
            .body(Body::from(
                r#"{"device_id":"ios-device-b","reason":"user_requested","include_current":false}"#,
            ))?;
        let revoke_response = app.clone().oneshot(revoke_request).await?;
        assert_eq!(revoke_response.status(), StatusCode::OK);
        let revoke_body = read_json(revoke_response).await?;
        let revoked_sessions = revoke_body["revokedSessionIds"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        assert_eq!(revoked_sessions.len(), 1);

        let current_a = Request::builder()
            .uri("/api/auth/session")
            .header("authorization", format!("Bearer {token_a}"))
            .body(Body::empty())?;
        let current_a_response = app.clone().oneshot(current_a).await?;
        assert_eq!(current_a_response.status(), StatusCode::OK);

        let current_b = Request::builder()
            .uri("/api/auth/session")
            .header("authorization", format!("Bearer {token_b}"))
            .body(Body::empty())?;
        let current_b_response = app.oneshot(current_b).await?;
        assert_eq!(current_b_response.status(), StatusCode::UNAUTHORIZED);

        Ok(())
    }

    #[tokio::test]
    async fn global_revocation_supports_include_current_toggle() -> Result<()> {
        let app = build_router(test_config(std::env::temp_dir()));

        let send_a = Request::builder()
            .method("POST")
            .uri("/api/auth/email")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"email":"global-revoke@example.com"}"#))?;
        let send_a_response = app.clone().oneshot(send_a).await?;
        let cookie_a = cookie_value(&send_a_response).unwrap_or_default();

        let verify_a = Request::builder()
            .method("POST")
            .uri("/api/auth/verify")
            .header("content-type", "application/json")
            .header("x-client", "autopilot-ios")
            .header("x-device-id", "ios-global-a")
            .header("cookie", cookie_a)
            .body(Body::from(r#"{"code":"123456"}"#))?;
        let verify_a_response = app.clone().oneshot(verify_a).await?;
        let verify_a_body = read_json(verify_a_response).await?;
        let token_a = verify_a_body["token"]
            .as_str()
            .unwrap_or_default()
            .to_string();

        let send_b = Request::builder()
            .method("POST")
            .uri("/api/auth/email")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"email":"global-revoke@example.com"}"#))?;
        let send_b_response = app.clone().oneshot(send_b).await?;
        let cookie_b = cookie_value(&send_b_response).unwrap_or_default();

        let verify_b = Request::builder()
            .method("POST")
            .uri("/api/auth/verify")
            .header("content-type", "application/json")
            .header("x-client", "autopilot-ios")
            .header("x-device-id", "ios-global-b")
            .header("cookie", cookie_b)
            .body(Body::from(r#"{"code":"123456"}"#))?;
        let verify_b_response = app.clone().oneshot(verify_b).await?;
        let verify_b_body = read_json(verify_b_response).await?;
        let token_b = verify_b_body["token"]
            .as_str()
            .unwrap_or_default()
            .to_string();

        let revoke_others_request = Request::builder()
            .method("POST")
            .uri("/api/auth/sessions/revoke")
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {token_a}"))
            .body(Body::from(
                r#"{"revoke_all_sessions":true,"include_current":false,"reason":"user_requested"}"#,
            ))?;
        let revoke_others_response = app.clone().oneshot(revoke_others_request).await?;
        assert_eq!(revoke_others_response.status(), StatusCode::OK);

        let current_a = Request::builder()
            .uri("/api/auth/session")
            .header("authorization", format!("Bearer {token_a}"))
            .body(Body::empty())?;
        let current_a_response = app.clone().oneshot(current_a).await?;
        assert_eq!(current_a_response.status(), StatusCode::OK);

        let current_b = Request::builder()
            .uri("/api/auth/session")
            .header("authorization", format!("Bearer {token_b}"))
            .body(Body::empty())?;
        let current_b_response = app.clone().oneshot(current_b).await?;
        assert_eq!(current_b_response.status(), StatusCode::UNAUTHORIZED);

        let revoke_all_request = Request::builder()
            .method("POST")
            .uri("/api/auth/sessions/revoke")
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {token_a}"))
            .body(Body::from(
                r#"{"revoke_all_sessions":true,"include_current":true,"reason":"user_requested"}"#,
            ))?;
        let revoke_all_response = app.clone().oneshot(revoke_all_request).await?;
        assert_eq!(revoke_all_response.status(), StatusCode::OK);

        let current_a_after = Request::builder()
            .uri("/api/auth/session")
            .header("authorization", format!("Bearer {token_a}"))
            .body(Body::empty())?;
        let current_a_after_response = app.oneshot(current_a_after).await?;
        assert_eq!(current_a_after_response.status(), StatusCode::UNAUTHORIZED);

        Ok(())
    }

    #[tokio::test]
    async fn logout_propagates_runtime_revocation_when_configured() -> Result<()> {
        let captured = Arc::new(Mutex::new(Vec::<Value>::new()));
        let (runtime_addr, runtime_handle) =
            start_runtime_revocation_stub(captured.clone()).await?;

        let mut config = test_config(std::env::temp_dir());
        config.runtime_sync_revoke_base_url = Some(format!("http://{runtime_addr}"));
        config.runtime_sync_revoke_path = "/internal/v1/sync/sessions/revoke".to_string();
        config.runtime_signature_secret = Some("runtime-signature-secret".to_string());
        config.runtime_signature_ttl_seconds = 60;

        let app = build_router(config);

        let send_request = Request::builder()
            .method("POST")
            .uri("/api/auth/email")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"email":"runtime-revoke@openagents.com"}"#))?;
        let send_response = app.clone().oneshot(send_request).await?;
        let cookie = cookie_value(&send_response).unwrap_or_default();

        let verify_request = Request::builder()
            .method("POST")
            .uri("/api/auth/verify")
            .header("content-type", "application/json")
            .header("x-client", "autopilot-ios")
            .header("x-device-id", "ios-runtime-revoke")
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
        let logout_body = read_json(logout_response).await?;
        let revoked_session_id = logout_body["sessionId"]
            .as_str()
            .unwrap_or_default()
            .to_string();
        assert!(!revoked_session_id.is_empty());

        let records = captured.lock().await.clone();
        assert_eq!(records.len(), 1);
        assert!(
            records[0]["signature"]
                .as_str()
                .unwrap_or_default()
                .starts_with("v1.")
        );
        assert_eq!(records[0]["payload"]["reason"], "user_requested");
        assert_eq!(
            records[0]["payload"]["session_ids"]
                .as_array()
                .cloned()
                .unwrap_or_default(),
            vec![Value::String(revoked_session_id)]
        );

        runtime_handle.abort();

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
    async fn me_route_returns_user_profile_and_thread_summaries() -> Result<()> {
        let app = build_router(test_config(std::env::temp_dir()));
        let token = authenticate_token(app.clone(), "me-route@openagents.com").await?;

        for thread_id in ["thread-a", "thread-b"] {
            let request = Request::builder()
                .method("POST")
                .uri(format!("/api/runtime/threads/{thread_id}/messages"))
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::from(r#"{"text":"hello"}"#))?;
            let response = app.clone().oneshot(request).await?;
            assert_eq!(response.status(), StatusCode::OK);
        }

        let me_request = Request::builder()
            .uri("/api/me?chat_limit=1")
            .header("authorization", format!("Bearer {token}"))
            .body(Body::empty())?;
        let me_response = app.oneshot(me_request).await?;
        assert_eq!(me_response.status(), StatusCode::OK);
        let me_body = read_json(me_response).await?;
        assert_eq!(me_body["data"]["user"]["email"], "me-route@openagents.com");
        assert_eq!(
            me_body["data"]["chatThreads"].as_array().map(Vec::len),
            Some(1)
        );
        assert_eq!(me_body["data"]["chatThreads"][0]["id"], "thread-b");

        Ok(())
    }

    #[tokio::test]
    async fn autopilot_crud_routes_support_create_list_show_and_update() -> Result<()> {
        let app = build_router(test_config(std::env::temp_dir()));
        let token = authenticate_token(app.clone(), "autopilot-owner@openagents.com").await?;

        let create_request = Request::builder()
            .method("POST")
            .uri("/api/autopilots")
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {token}"))
            .body(Body::from(
                r#"{"handle":"ep212-bot","displayName":"EP212 Bot","status":"active","visibility":"private"}"#,
            ))?;
        let create_response = app.clone().oneshot(create_request).await?;
        assert_eq!(create_response.status(), StatusCode::CREATED);
        let create_body = read_json(create_response).await?;
        let autopilot_id = create_body["data"]["id"]
            .as_str()
            .unwrap_or_default()
            .to_string();
        assert!(autopilot_id.starts_with("ap_"));
        assert_eq!(create_body["data"]["handle"], json!("ep212-bot"));
        assert_eq!(create_body["data"]["displayName"], json!("EP212 Bot"));
        assert_eq!(create_body["data"]["configVersion"], json!(1));

        let list_request = Request::builder()
            .method("GET")
            .uri("/api/autopilots?limit=200")
            .header("authorization", format!("Bearer {token}"))
            .body(Body::empty())?;
        let list_response = app.clone().oneshot(list_request).await?;
        assert_eq!(list_response.status(), StatusCode::OK);
        let list_body = read_json(list_response).await?;
        let listed = list_body["data"].as_array().cloned().unwrap_or_default();
        assert!(!listed.is_empty());
        assert!(
            listed
                .iter()
                .any(|row| row["id"] == json!(autopilot_id.clone()))
        );

        let show_by_id_request = Request::builder()
            .method("GET")
            .uri(format!("/api/autopilots/{autopilot_id}"))
            .header("authorization", format!("Bearer {token}"))
            .body(Body::empty())?;
        let show_by_id_response = app.clone().oneshot(show_by_id_request).await?;
        assert_eq!(show_by_id_response.status(), StatusCode::OK);
        let show_by_id_body = read_json(show_by_id_response).await?;
        assert_eq!(show_by_id_body["data"]["handle"], json!("ep212-bot"));

        let show_by_handle_request = Request::builder()
            .method("GET")
            .uri("/api/autopilots/ep212-bot")
            .header("authorization", format!("Bearer {token}"))
            .body(Body::empty())?;
        let show_by_handle_response = app.clone().oneshot(show_by_handle_request).await?;
        assert_eq!(show_by_handle_response.status(), StatusCode::OK);
        let show_by_handle_body = read_json(show_by_handle_response).await?;
        assert_eq!(
            show_by_handle_body["data"]["id"],
            json!(autopilot_id.clone())
        );

        let update_request = Request::builder()
            .method("PATCH")
            .uri(format!("/api/autopilots/{autopilot_id}"))
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {token}"))
            .body(Body::from(
                r#"{"displayName":"EP212 Bot Updated","profile":{"ownerDisplayName":"Chris","personaSummary":"Pragmatic and concise","autopilotVoice":"calm and direct"},"policy":{"toolAllowlist":["openagents_api"],"toolDenylist":["lightning_l402_fetch"],"l402RequireApproval":true,"l402MaxSpendMsatsPerCall":100000,"l402AllowedHosts":["sats4ai.com"]}}"#,
            ))?;
        let update_response = app.clone().oneshot(update_request).await?;
        assert_eq!(update_response.status(), StatusCode::OK);
        let update_body = read_json(update_response).await?;
        assert_eq!(
            update_body["data"]["displayName"],
            json!("EP212 Bot Updated")
        );
        assert_eq!(update_body["data"]["configVersion"], json!(2));
        assert_eq!(
            update_body["data"]["profile"]["ownerDisplayName"],
            json!("Chris")
        );
        assert_eq!(
            update_body["data"]["policy"]["toolAllowlist"][0],
            json!("openagents_api")
        );
        assert_eq!(
            update_body["data"]["policy"]["l402MaxSpendMsatsPerCall"],
            json!(100000)
        );

        Ok(())
    }

    #[tokio::test]
    async fn autopilot_thread_routes_support_create_and_list() -> Result<()> {
        let app = build_router(test_config(std::env::temp_dir()));
        let token = authenticate_token(app.clone(), "autopilot-threads@openagents.com").await?;

        let create_autopilot_request = Request::builder()
            .method("POST")
            .uri("/api/autopilots")
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {token}"))
            .body(Body::from(
                r#"{"handle":"thread-bot","displayName":"Thread Bot"}"#,
            ))?;
        let create_autopilot_response = app.clone().oneshot(create_autopilot_request).await?;
        assert_eq!(create_autopilot_response.status(), StatusCode::CREATED);
        let create_autopilot_body = read_json(create_autopilot_response).await?;
        let autopilot_id = create_autopilot_body["data"]["id"]
            .as_str()
            .unwrap_or_default()
            .to_string();
        assert!(autopilot_id.starts_with("ap_"));

        let create_thread_request = Request::builder()
            .method("POST")
            .uri(format!("/api/autopilots/{autopilot_id}/threads"))
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {token}"))
            .body(Body::from(r#"{"title":"Autopilot test thread"}"#))?;
        let create_thread_response = app.clone().oneshot(create_thread_request).await?;
        assert_eq!(create_thread_response.status(), StatusCode::CREATED);
        let create_thread_body = read_json(create_thread_response).await?;
        let thread_id = create_thread_body["data"]["id"]
            .as_str()
            .unwrap_or_default()
            .to_string();
        assert!(thread_id.starts_with("thread_"));
        assert_eq!(
            create_thread_body["data"]["autopilotId"],
            json!(autopilot_id.clone())
        );
        assert_eq!(
            create_thread_body["data"]["title"],
            json!("Autopilot test thread")
        );

        let create_default_title_request = Request::builder()
            .method("POST")
            .uri(format!("/api/autopilots/{autopilot_id}/threads"))
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {token}"))
            .body(Body::from(r#"{}"#))?;
        let create_default_title_response =
            app.clone().oneshot(create_default_title_request).await?;
        assert_eq!(create_default_title_response.status(), StatusCode::CREATED);
        let create_default_title_body = read_json(create_default_title_response).await?;
        assert_eq!(
            create_default_title_body["data"]["title"],
            json!("New conversation")
        );

        let list_threads_request = Request::builder()
            .method("GET")
            .uri(format!("/api/autopilots/{autopilot_id}/threads?limit=200"))
            .header("authorization", format!("Bearer {token}"))
            .body(Body::empty())?;
        let list_threads_response = app.clone().oneshot(list_threads_request).await?;
        assert_eq!(list_threads_response.status(), StatusCode::OK);
        let list_threads_body = read_json(list_threads_response).await?;
        let listed_threads = list_threads_body["data"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        assert!(listed_threads.len() >= 2);
        assert!(listed_threads.iter().all(|row| {
            row["autopilotId"] == json!(autopilot_id.clone()) && row["id"].is_string()
        }));
        assert!(
            listed_threads
                .iter()
                .any(|row| row["id"] == json!(thread_id.clone()))
        );

        Ok(())
    }

    #[tokio::test]
    async fn autopilot_stream_route_bootstraps_codex_and_returns_ws_delivery() -> Result<()> {
        let app = build_router(test_config(std::env::temp_dir()));
        let token = authenticate_token(app.clone(), "autopilot-stream@openagents.com").await?;

        let create_autopilot_request = Request::builder()
            .method("POST")
            .uri("/api/autopilots")
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {token}"))
            .body(Body::from(
                r#"{"handle":"stream-bot","displayName":"Stream Bot"}"#,
            ))?;
        let create_autopilot_response = app.clone().oneshot(create_autopilot_request).await?;
        assert_eq!(create_autopilot_response.status(), StatusCode::CREATED);
        let create_autopilot_body = read_json(create_autopilot_response).await?;
        let autopilot_id = create_autopilot_body["data"]["id"]
            .as_str()
            .unwrap_or_default()
            .to_string();

        let stream_request = Request::builder()
            .method("POST")
            .uri(format!("/api/autopilots/{autopilot_id}/stream"))
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {token}"))
            .body(Body::from(
                r#"{"messages":[{"id":"m1","role":"user","content":"hello from autopilot stream alias"}]}"#,
            ))?;
        let stream_response = app.clone().oneshot(stream_request).await?;
        assert_eq!(stream_response.status(), StatusCode::OK);
        let stream_body = read_json(stream_response).await?;
        let thread_id = stream_body["data"]["threadId"]
            .as_str()
            .unwrap_or_default()
            .to_string();
        assert!(thread_id.starts_with("thread_"));
        assert_eq!(stream_body["data"]["accepted"], json!(true));
        assert_eq!(
            stream_body["data"]["autopilotId"],
            json!(autopilot_id.clone())
        );
        assert_eq!(
            stream_body["data"]["delivery"]["transport"],
            json!("khala_ws")
        );
        let delivery_topic = stream_body["data"]["delivery"]["topic"]
            .as_str()
            .unwrap_or_default();
        assert!(delivery_topic.ends_with(":worker_events"));
        assert_eq!(
            stream_body["data"]["control"]["method"],
            json!("turn/start")
        );
        assert_eq!(
            stream_body["data"]["response"]["thread_id"],
            json!(thread_id.clone())
        );

        let list_request = Request::builder()
            .method("GET")
            .uri(format!("/api/autopilots/{autopilot_id}/threads"))
            .header("authorization", format!("Bearer {token}"))
            .body(Body::empty())?;
        let list_response = app.clone().oneshot(list_request).await?;
        assert_eq!(list_response.status(), StatusCode::OK);
        let list_body = read_json(list_response).await?;
        let listed = list_body["data"].as_array().cloned().unwrap_or_default();
        assert!(
            listed
                .iter()
                .any(|row| row["id"] == json!(thread_id.clone()))
        );

        let resume_request = Request::builder()
            .method("POST")
            .uri(format!("/api/autopilots/{autopilot_id}/stream"))
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {token}"))
            .body(Body::from(format!(
                r#"{{"conversationId":"{thread_id}","messages":[{{"id":"m2","role":"user","content":"continue this thread"}}]}}"#
            )))?;
        let resume_response = app.oneshot(resume_request).await?;
        assert_eq!(resume_response.status(), StatusCode::OK);
        let resume_body = read_json(resume_response).await?;
        assert_eq!(resume_body["data"]["threadId"], json!(thread_id));

        Ok(())
    }

    #[tokio::test]
    async fn autopilot_routes_enforce_owner_boundary() -> Result<()> {
        let app = build_router(test_config(std::env::temp_dir()));
        let owner_token =
            authenticate_token(app.clone(), "autopilot-owner-a@openagents.com").await?;
        let other_token =
            authenticate_token(app.clone(), "autopilot-owner-b@openagents.com").await?;

        let create_request = Request::builder()
            .method("POST")
            .uri("/api/autopilots")
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {owner_token}"))
            .body(Body::from(
                r#"{"handle":"owner-bot","displayName":"Owner Bot"}"#,
            ))?;
        let create_response = app.clone().oneshot(create_request).await?;
        assert_eq!(create_response.status(), StatusCode::CREATED);
        let create_body = read_json(create_response).await?;
        let autopilot_id = create_body["data"]["id"]
            .as_str()
            .unwrap_or_default()
            .to_string();

        let other_show_request = Request::builder()
            .method("GET")
            .uri(format!("/api/autopilots/{autopilot_id}"))
            .header("authorization", format!("Bearer {other_token}"))
            .body(Body::empty())?;
        let other_show_response = app.clone().oneshot(other_show_request).await?;
        assert_eq!(other_show_response.status(), StatusCode::NOT_FOUND);

        let other_update_request = Request::builder()
            .method("PATCH")
            .uri(format!("/api/autopilots/{autopilot_id}"))
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {other_token}"))
            .body(Body::from(r#"{"displayName":"Hacked"}"#))?;
        let other_update_response = app.clone().oneshot(other_update_request).await?;
        assert_eq!(other_update_response.status(), StatusCode::NOT_FOUND);

        let other_show_handle_request = Request::builder()
            .method("GET")
            .uri("/api/autopilots/owner-bot")
            .header("authorization", format!("Bearer {other_token}"))
            .body(Body::empty())?;
        let other_show_handle_response = app.clone().oneshot(other_show_handle_request).await?;
        assert_eq!(other_show_handle_response.status(), StatusCode::NOT_FOUND);

        let other_threads_request = Request::builder()
            .method("GET")
            .uri(format!("/api/autopilots/{autopilot_id}/threads"))
            .header("authorization", format!("Bearer {other_token}"))
            .body(Body::empty())?;
        let other_threads_response = app.clone().oneshot(other_threads_request).await?;
        assert_eq!(other_threads_response.status(), StatusCode::NOT_FOUND);

        let other_create_thread_request = Request::builder()
            .method("POST")
            .uri(format!("/api/autopilots/{autopilot_id}/threads"))
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {other_token}"))
            .body(Body::from(r#"{"title":"intruder thread"}"#))?;
        let other_create_thread_response = app.clone().oneshot(other_create_thread_request).await?;
        assert_eq!(other_create_thread_response.status(), StatusCode::NOT_FOUND);

        let other_stream_request = Request::builder()
            .method("POST")
            .uri(format!("/api/autopilots/{autopilot_id}/stream"))
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {other_token}"))
            .body(Body::from(
                r#"{"messages":[{"id":"m1","role":"user","content":"intruder stream"}]}"#,
            ))?;
        let other_stream_response = app.oneshot(other_stream_request).await?;
        assert_eq!(other_stream_response.status(), StatusCode::NOT_FOUND);

        Ok(())
    }

    #[tokio::test]
    async fn autopilot_routes_enforce_validation_semantics() -> Result<()> {
        let app = build_router(test_config(std::env::temp_dir()));
        let token = authenticate_token(app.clone(), "autopilot-validation@openagents.com").await?;

        let bad_create_request = Request::builder()
            .method("POST")
            .uri("/api/autopilots")
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {token}"))
            .body(Body::from(r#"{"handle":"invalid handle!"}"#))?;
        let bad_create_response = app.clone().oneshot(bad_create_request).await?;
        assert_eq!(
            bad_create_response.status(),
            StatusCode::UNPROCESSABLE_ENTITY
        );
        let bad_create_body = read_json(bad_create_response).await?;
        assert_eq!(bad_create_body["error"]["code"], json!("invalid_request"));

        let create_request = Request::builder()
            .method("POST")
            .uri("/api/autopilots")
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {token}"))
            .body(Body::from(r#"{"handle":"valid-bot"}"#))?;
        let create_response = app.clone().oneshot(create_request).await?;
        assert_eq!(create_response.status(), StatusCode::CREATED);
        let create_body = read_json(create_response).await?;
        let autopilot_id = create_body["data"]["id"]
            .as_str()
            .unwrap_or_default()
            .to_string();

        let bad_update_request = Request::builder()
            .method("PATCH")
            .uri(format!("/api/autopilots/{autopilot_id}"))
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {token}"))
            .body(Body::from(
                r#"{"status":"ACTIVE","profile":{"schemaVersion":0},"policy":{"l402MaxSpendMsatsPerCall":0}}"#,
            ))?;
        let bad_update_response = app.clone().oneshot(bad_update_request).await?;
        assert_eq!(
            bad_update_response.status(),
            StatusCode::UNPROCESSABLE_ENTITY
        );
        let bad_update_body = read_json(bad_update_response).await?;
        assert_eq!(bad_update_body["error"]["code"], json!("invalid_request"));

        let oversized_title = "x".repeat(201);
        let bad_thread_request = Request::builder()
            .method("POST")
            .uri(format!("/api/autopilots/{autopilot_id}/threads"))
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {token}"))
            .body(Body::from(format!(r#"{{"title":"{oversized_title}"}}"#)))?;
        let bad_thread_response = app.clone().oneshot(bad_thread_request).await?;
        assert_eq!(
            bad_thread_response.status(),
            StatusCode::UNPROCESSABLE_ENTITY
        );
        let bad_thread_body = read_json(bad_thread_response).await?;
        assert_eq!(bad_thread_body["error"]["code"], json!("invalid_request"));

        let bad_stream_request = Request::builder()
            .method("POST")
            .uri(format!("/api/autopilots/{autopilot_id}/stream"))
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {token}"))
            .body(Body::from(
                r#"{"messages":[{"id":"m1","role":"assistant","content":"missing user prompt"}]}"#,
            ))?;
        let bad_stream_response = app.clone().oneshot(bad_stream_request).await?;
        assert_eq!(
            bad_stream_response.status(),
            StatusCode::UNPROCESSABLE_ENTITY
        );
        let bad_stream_body = read_json(bad_stream_response).await?;
        assert_eq!(bad_stream_body["error"]["code"], json!("invalid_request"));

        let missing_thread_request = Request::builder()
            .method("POST")
            .uri(format!("/api/autopilots/{autopilot_id}/stream"))
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {token}"))
            .body(Body::from(
                r#"{"conversationId":"thread_missing","messages":[{"id":"m2","role":"user","content":"hello"}]}"#,
            ))?;
        let missing_thread_response = app.oneshot(missing_thread_request).await?;
        assert_eq!(missing_thread_response.status(), StatusCode::NOT_FOUND);

        Ok(())
    }

    #[tokio::test]
    async fn settings_profile_routes_support_read_update_delete() -> Result<()> {
        let app = build_router(test_config(std::env::temp_dir()));
        let token = authenticate_token(app.clone(), "profile-user@openagents.com").await?;

        let show_request = Request::builder()
            .uri("/api/settings/profile")
            .header("authorization", format!("Bearer {token}"))
            .body(Body::empty())?;
        let show_response = app.clone().oneshot(show_request).await?;
        assert_eq!(show_response.status(), StatusCode::OK);
        let show_body = read_json(show_response).await?;
        assert_eq!(show_body["data"]["email"], "profile-user@openagents.com");

        let update_request = Request::builder()
            .method("PATCH")
            .uri("/api/settings/profile")
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {token}"))
            .body(Body::from(r#"{"name":"Updated Name"}"#))?;
        let update_response = app.clone().oneshot(update_request).await?;
        assert_eq!(update_response.status(), StatusCode::OK);
        let update_body = read_json(update_response).await?;
        assert_eq!(update_body["data"]["name"], "Updated Name");

        let wrong_delete_request = Request::builder()
            .method("DELETE")
            .uri("/api/settings/profile")
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {token}"))
            .body(Body::from(r#"{"email":"wrong@openagents.com"}"#))?;
        let wrong_delete_response = app.clone().oneshot(wrong_delete_request).await?;
        assert_eq!(
            wrong_delete_response.status(),
            StatusCode::UNPROCESSABLE_ENTITY
        );
        let wrong_delete_body = read_json(wrong_delete_response).await?;
        assert_eq!(
            wrong_delete_body["message"],
            "Email confirmation does not match the authenticated user."
        );

        let delete_request = Request::builder()
            .method("DELETE")
            .uri("/api/settings/profile")
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {token}"))
            .body(Body::from(r#"{"email":"profile-user@openagents.com"}"#))?;
        let delete_response = app.clone().oneshot(delete_request).await?;
        assert_eq!(delete_response.status(), StatusCode::OK);
        let delete_body = read_json(delete_response).await?;
        assert_eq!(delete_body["data"]["deleted"], true);

        let show_after_delete_request = Request::builder()
            .uri("/api/settings/profile")
            .header("authorization", format!("Bearer {token}"))
            .body(Body::empty())?;
        let show_after_delete_response = app.oneshot(show_after_delete_request).await?;
        assert_eq!(
            show_after_delete_response.status(),
            StatusCode::UNAUTHORIZED
        );

        Ok(())
    }

    #[tokio::test]
    async fn personal_access_token_routes_support_current_and_bulk_revocation() -> Result<()> {
        let app = build_router(test_config(std::env::temp_dir()));
        let session_token =
            authenticate_token(app.clone(), "token-lifecycle@openagents.com").await?;

        let create_request = Request::builder()
            .method("POST")
            .uri("/api/tokens")
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {session_token}"))
            .body(Body::from(
                r#"{"name":"api-cli","abilities":["chat:read","chat:write"]}"#,
            ))?;
        let create_response = app.clone().oneshot(create_request).await?;
        assert_eq!(create_response.status(), StatusCode::CREATED);
        let create_body = read_json(create_response).await?;
        let pat_token = create_body["data"]["token"]
            .as_str()
            .unwrap_or_default()
            .to_string();
        assert!(pat_token.starts_with("oa_pat_"));

        let list_by_pat_request = Request::builder()
            .uri("/api/tokens")
            .header("authorization", format!("Bearer {pat_token}"))
            .body(Body::empty())?;
        let list_by_pat_response = app.clone().oneshot(list_by_pat_request).await?;
        assert_eq!(list_by_pat_response.status(), StatusCode::OK);
        let list_by_pat_body = read_json(list_by_pat_response).await?;
        let tokens = list_by_pat_body["data"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        assert!(tokens.iter().any(|token| token["isCurrent"] == true));

        let delete_current_request = Request::builder()
            .method("DELETE")
            .uri("/api/tokens/current")
            .header("authorization", format!("Bearer {pat_token}"))
            .body(Body::empty())?;
        let delete_current_response = app.clone().oneshot(delete_current_request).await?;
        assert_eq!(delete_current_response.status(), StatusCode::OK);
        let delete_current_body = read_json(delete_current_response).await?;
        assert_eq!(delete_current_body["data"]["deleted"], true);

        for name in ["bulk-a", "bulk-b"] {
            let request = Request::builder()
                .method("POST")
                .uri("/api/tokens")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {session_token}"))
                .body(Body::from(format!(r#"{{"name":"{name}"}}"#)))?;
            let response = app.clone().oneshot(request).await?;
            assert_eq!(response.status(), StatusCode::CREATED);
        }

        let delete_all_request = Request::builder()
            .method("DELETE")
            .uri("/api/tokens")
            .header("authorization", format!("Bearer {session_token}"))
            .body(Body::empty())?;
        let delete_all_response = app.clone().oneshot(delete_all_request).await?;
        assert_eq!(delete_all_response.status(), StatusCode::OK);
        let delete_all_body = read_json(delete_all_response).await?;
        assert_eq!(delete_all_body["data"]["deletedCount"], 2);

        let final_list_request = Request::builder()
            .uri("/api/tokens")
            .header("authorization", format!("Bearer {session_token}"))
            .body(Body::empty())?;
        let final_list_response = app.oneshot(final_list_request).await?;
        assert_eq!(final_list_response.status(), StatusCode::OK);
        let final_list_body = read_json(final_list_response).await?;
        assert_eq!(final_list_body["data"].as_array().map(Vec::len), Some(0));

        Ok(())
    }

    #[tokio::test]
    async fn khala_token_route_mints_and_surfaces_configuration_errors() -> Result<()> {
        let app = build_router(test_config(std::env::temp_dir()));
        let token = authenticate_token(app.clone(), "khala-route@openagents.com").await?;

        let mint_request = Request::builder()
            .method("POST")
            .uri("/api/khala/token")
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {token}"))
            .body(Body::from(
                r#"{"scope":["codex:read","codex:write"],"workspace_id":"workspace_42","role":"admin"}"#,
            ))?;
        let mint_response = app.clone().oneshot(mint_request).await?;
        assert_eq!(mint_response.status(), StatusCode::OK);
        let mint_body = read_json(mint_response).await?;
        assert_eq!(mint_body["data"]["token_type"], "Bearer");
        assert_eq!(mint_body["data"]["issuer"], "https://openagents.test");
        assert_eq!(mint_body["data"]["audience"], "openagents-khala-test");
        assert_eq!(mint_body["data"]["claims_version"], "oa_khala_claims_v1");

        let mut config = test_config(std::env::temp_dir());
        config.khala_token_signing_key = None;
        let misconfigured_app = build_router(config);
        let misconfigured_token = authenticate_token(
            misconfigured_app.clone(),
            "khala-misconfigured@openagents.com",
        )
        .await?;
        let unavailable_request = Request::builder()
            .method("POST")
            .uri("/api/khala/token")
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {misconfigured_token}"))
            .body(Body::from("{}"))?;
        let unavailable_response = misconfigured_app.oneshot(unavailable_request).await?;
        assert_eq!(
            unavailable_response.status(),
            StatusCode::SERVICE_UNAVAILABLE
        );
        let unavailable_body = read_json(unavailable_response).await?;
        assert_eq!(unavailable_body["error"]["code"], "khala_token_unavailable");

        Ok(())
    }

    #[tokio::test]
    async fn sync_token_route_accepts_personal_access_token_auth() -> Result<()> {
        let app = build_router(test_config(std::env::temp_dir()));
        let session_token = authenticate_token(app.clone(), "sync-pat@openagents.com").await?;

        let create_pat_request = Request::builder()
            .method("POST")
            .uri("/api/tokens")
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {session_token}"))
            .body(Body::from(r#"{"name":"sync-pat"}"#))?;
        let create_pat_response = app.clone().oneshot(create_pat_request).await?;
        assert_eq!(create_pat_response.status(), StatusCode::CREATED);
        let create_pat_body = read_json(create_pat_response).await?;
        let pat_token = create_pat_body["data"]["token"]
            .as_str()
            .unwrap_or_default()
            .to_string();

        let sync_request = Request::builder()
            .method("POST")
            .uri("/api/sync/token")
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {pat_token}"))
            .body(Body::from(
                r#"{"scopes":["runtime.codex_worker_events"],"device_id":"mobile:custom"}"#,
            ))?;
        let sync_response = app.oneshot(sync_request).await?;
        assert_eq!(sync_response.status(), StatusCode::OK);
        let sync_body = read_json(sync_response).await?;
        assert_eq!(sync_body["data"]["token_type"], "Bearer");
        assert!(
            sync_body["data"]["session_id"]
                .as_str()
                .unwrap_or_default()
                .starts_with("pat:")
        );

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
    async fn runtime_thread_read_paths_return_projected_threads_and_messages() -> Result<()> {
        let app = build_router(test_config(std::env::temp_dir()));
        let token = authenticate_token(app.clone(), "thread-read@openagents.com").await?;

        for thread_id in ["thread-42", "thread-42", "thread-99"] {
            let request = Request::builder()
                .method("POST")
                .uri(format!("/api/runtime/threads/{thread_id}/messages"))
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::from(r#"{"text":"hello"}"#))?;
            let response = app.clone().oneshot(request).await?;
            assert_eq!(response.status(), StatusCode::OK);
        }

        let list_threads_request = Request::builder()
            .method("GET")
            .uri("/api/runtime/threads")
            .header("authorization", format!("Bearer {token}"))
            .body(Body::empty())?;
        let list_threads_response = app.clone().oneshot(list_threads_request).await?;
        assert_eq!(list_threads_response.status(), StatusCode::OK);
        let list_threads_body = read_json(list_threads_response).await?;
        assert_eq!(
            list_threads_body["data"]["threads"]
                .as_array()
                .map(Vec::len),
            Some(2)
        );

        let list_messages_request = Request::builder()
            .method("GET")
            .uri("/api/runtime/threads/thread-42/messages")
            .header("authorization", format!("Bearer {token}"))
            .body(Body::empty())?;
        let list_messages_response = app.oneshot(list_messages_request).await?;
        assert_eq!(list_messages_response.status(), StatusCode::OK);
        let list_messages_body = read_json(list_messages_response).await?;
        assert_eq!(
            list_messages_body["data"]["messages"]
                .as_array()
                .map(Vec::len),
            Some(2)
        );

        Ok(())
    }

    #[tokio::test]
    async fn runtime_thread_message_read_path_enforces_owner_boundary() -> Result<()> {
        let app = build_router(test_config(std::env::temp_dir()));
        let owner_token = authenticate_token(app.clone(), "thread-owner@openagents.com").await?;
        let other_token = authenticate_token(app.clone(), "thread-other@openagents.com").await?;

        let append_request = Request::builder()
            .method("POST")
            .uri("/api/runtime/threads/thread-private/messages")
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {owner_token}"))
            .body(Body::from(r#"{"text":"private"}"#))?;
        let append_response = app.clone().oneshot(append_request).await?;
        assert_eq!(append_response.status(), StatusCode::OK);

        let read_other_request = Request::builder()
            .method("GET")
            .uri("/api/runtime/threads/thread-private/messages")
            .header("authorization", format!("Bearer {other_token}"))
            .body(Body::empty())?;
        let read_other_response = app.oneshot(read_other_request).await?;
        assert_eq!(read_other_response.status(), StatusCode::FORBIDDEN);

        Ok(())
    }

    #[tokio::test]
    async fn legacy_chats_aliases_map_to_codex_threads() -> Result<()> {
        let app = build_router(test_config(std::env::temp_dir()));
        let token = authenticate_token(app.clone(), "legacy-chats@openagents.com").await?;

        let create_request = Request::builder()
            .method("POST")
            .uri("/api/chats")
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {token}"))
            .body(Body::from(r#"{"title":"Migration Chat"}"#))?;
        let create_response = app.clone().oneshot(create_request).await?;
        assert_eq!(create_response.status(), StatusCode::CREATED);
        assert_eq!(
            create_response
                .headers()
                .get("x-oa-legacy-chat-retired")
                .and_then(|value| value.to_str().ok()),
            Some("true")
        );
        assert_eq!(
            create_response
                .headers()
                .get("x-oa-legacy-chat-canonical")
                .and_then(|value| value.to_str().ok()),
            Some("/api/runtime/threads")
        );
        let create_body = read_json(create_response).await?;
        let conversation_id = create_body["data"]["id"]
            .as_str()
            .unwrap_or_default()
            .to_string();
        assert!(conversation_id.starts_with("thread_"));

        let runtime_threads_request = Request::builder()
            .method("GET")
            .uri("/api/runtime/threads")
            .header("authorization", format!("Bearer {token}"))
            .body(Body::empty())?;
        let runtime_threads_response = app.clone().oneshot(runtime_threads_request).await?;
        assert_eq!(runtime_threads_response.status(), StatusCode::OK);
        let runtime_threads_body = read_json(runtime_threads_response).await?;
        let contains_thread = runtime_threads_body["data"]["threads"]
            .as_array()
            .map(|threads| {
                threads
                    .iter()
                    .any(|thread| thread["thread_id"] == json!(conversation_id.clone()))
            })
            .unwrap_or(false);
        assert!(contains_thread);

        let send_message_request = Request::builder()
            .method("POST")
            .uri(format!("/api/runtime/threads/{conversation_id}/messages"))
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {token}"))
            .body(Body::from(r#"{"text":"legacy bridge message"}"#))?;
        let send_message_response = app.clone().oneshot(send_message_request).await?;
        assert_eq!(send_message_response.status(), StatusCode::OK);

        let show_request = Request::builder()
            .method("GET")
            .uri(format!("/api/chats/{conversation_id}"))
            .header("authorization", format!("Bearer {token}"))
            .body(Body::empty())?;
        let show_response = app.clone().oneshot(show_request).await?;
        assert_eq!(show_response.status(), StatusCode::OK);
        let show_body = read_json(show_response).await?;
        assert_eq!(
            show_body["data"]["conversation"]["id"],
            json!(conversation_id.clone())
        );
        assert_eq!(
            show_body["data"]["messages"]
                .as_array()
                .map(Vec::len)
                .unwrap_or_default(),
            1
        );
        assert_eq!(
            show_body["data"]["messages"][0]["content"],
            json!("legacy bridge message")
        );
        assert_eq!(
            show_body["data"]["runs"]
                .as_array()
                .map(Vec::len)
                .unwrap_or_default(),
            0
        );

        let runs_request = Request::builder()
            .method("GET")
            .uri(format!("/api/chats/{conversation_id}/runs"))
            .header("authorization", format!("Bearer {token}"))
            .body(Body::empty())?;
        let runs_response = app.clone().oneshot(runs_request).await?;
        assert_eq!(runs_response.status(), StatusCode::OK);
        let runs_body = read_json(runs_response).await?;
        assert_eq!(
            runs_body["data"]
                .as_array()
                .map(Vec::len)
                .unwrap_or_default(),
            0
        );

        let events_request = Request::builder()
            .method("GET")
            .uri(format!(
                "/api/chats/{conversation_id}/runs/run_legacy/events"
            ))
            .header("authorization", format!("Bearer {token}"))
            .body(Body::empty())?;
        let events_response = app.oneshot(events_request).await?;
        assert_eq!(events_response.status(), StatusCode::OK);
        let events_body = read_json(events_response).await?;
        assert_eq!(events_body["data"]["run"]["status"], json!("retired"));
        assert_eq!(
            events_body["data"]["events"]
                .as_array()
                .map(Vec::len)
                .unwrap_or_default(),
            0
        );

        Ok(())
    }

    #[tokio::test]
    async fn legacy_chat_show_rejects_cross_user_access() -> Result<()> {
        let app = build_router(test_config(std::env::temp_dir()));
        let owner_token = authenticate_token(app.clone(), "legacy-owner@openagents.com").await?;
        let other_token = authenticate_token(app.clone(), "legacy-other@openagents.com").await?;

        let create_request = Request::builder()
            .method("POST")
            .uri("/api/chats")
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {owner_token}"))
            .body(Body::from(r#"{"title":"Owner Chat"}"#))?;
        let create_response = app.clone().oneshot(create_request).await?;
        assert_eq!(create_response.status(), StatusCode::CREATED);
        let create_body = read_json(create_response).await?;
        let conversation_id = create_body["data"]["id"]
            .as_str()
            .unwrap_or_default()
            .to_string();
        assert!(!conversation_id.is_empty());

        let other_show_request = Request::builder()
            .method("GET")
            .uri(format!("/api/chats/{conversation_id}"))
            .header("authorization", format!("Bearer {other_token}"))
            .body(Body::empty())?;
        let other_show_response = app.oneshot(other_show_request).await?;
        assert_eq!(other_show_response.status(), StatusCode::FORBIDDEN);

        Ok(())
    }

    #[tokio::test]
    async fn legacy_chat_stream_alias_bridges_to_codex_control_request() -> Result<()> {
        let app = build_router(test_config(std::env::temp_dir()));
        let token = authenticate_token(app.clone(), "legacy-stream@openagents.com").await?;

        let stream_request = Request::builder()
            .method("POST")
            .uri("/api/chat/stream")
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {token}"))
            .body(Body::from(
                r#"{"id":"thread-stream-alias","messages":[{"role":"user","content":"bridge hello"}]}"#,
            ))?;
        let stream_response = app.clone().oneshot(stream_request).await?;
        assert_eq!(stream_response.status(), StatusCode::OK);
        assert_eq!(
            stream_response
                .headers()
                .get("x-oa-legacy-chat-retired")
                .and_then(|value| value.to_str().ok()),
            Some("true")
        );
        assert_eq!(
            stream_response
                .headers()
                .get("x-oa-legacy-chat-canonical")
                .and_then(|value| value.to_str().ok()),
            Some("/api/runtime/codex/workers/:worker_id/requests")
        );
        assert_eq!(
            stream_response
                .headers()
                .get("x-oa-legacy-chat-stream-protocol")
                .and_then(|value| value.to_str().ok()),
            Some("disabled")
        );
        let stream_body = read_json(stream_response).await?;
        assert_eq!(stream_body["data"]["retired"], json!(true));
        assert_eq!(stream_body["data"]["stream_protocol"], json!("disabled"));
        assert_eq!(
            stream_body["data"]["response"]["thread_id"],
            json!("thread-stream-alias")
        );

        let messages_request = Request::builder()
            .method("GET")
            .uri("/api/runtime/threads/thread-stream-alias/messages")
            .header("authorization", format!("Bearer {token}"))
            .body(Body::empty())?;
        let messages_response = app.oneshot(messages_request).await?;
        assert_eq!(messages_response.status(), StatusCode::OK);
        let messages_body = read_json(messages_response).await?;
        assert_eq!(
            messages_body["data"]["messages"][0]["text"],
            json!("bridge hello")
        );

        Ok(())
    }

    #[tokio::test]
    async fn legacy_chats_stream_alias_uses_path_thread_id_and_accepts_structured_content()
    -> Result<()> {
        let app = build_router(test_config(std::env::temp_dir()));
        let token = authenticate_token(app.clone(), "legacy-stream-path@openagents.com").await?;

        let stream_request = Request::builder()
            .method("POST")
            .uri("/api/chats/thread-stream-path/stream")
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {token}"))
            .body(Body::from(
                r#"{"messages":[{"role":"user","content":[{"type":"text","text":"path bridge"}]}]}"#,
            ))?;
        let stream_response = app.clone().oneshot(stream_request).await?;
        assert_eq!(stream_response.status(), StatusCode::OK);
        let stream_body = read_json(stream_response).await?;
        assert_eq!(
            stream_body["data"]["response"]["thread_id"],
            json!("thread-stream-path")
        );

        let messages_request = Request::builder()
            .method("GET")
            .uri("/api/runtime/threads/thread-stream-path/messages")
            .header("authorization", format!("Bearer {token}"))
            .body(Body::empty())?;
        let messages_response = app.oneshot(messages_request).await?;
        assert_eq!(messages_response.status(), StatusCode::OK);
        let messages_body = read_json(messages_response).await?;
        assert_eq!(
            messages_body["data"]["messages"][0]["text"],
            json!("path bridge")
        );

        Ok(())
    }

    #[tokio::test]
    async fn legacy_chat_stream_alias_rejects_payload_without_user_text() -> Result<()> {
        let app = build_router(test_config(std::env::temp_dir()));
        let token = authenticate_token(app.clone(), "legacy-stream-bad@openagents.com").await?;

        let stream_request = Request::builder()
            .method("POST")
            .uri("/api/chat/stream")
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {token}"))
            .body(Body::from(
                r#"{"messages":[{"role":"assistant","content":"no user message"}]}"#,
            ))?;
        let stream_response = app.oneshot(stream_request).await?;
        assert_eq!(stream_response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        let stream_body = read_json(stream_response).await?;
        assert_eq!(stream_body["error"]["code"], json!("invalid_request"));

        Ok(())
    }

    #[tokio::test]
    async fn runtime_codex_control_request_accepts_turn_start_and_persists_message() -> Result<()> {
        let app = build_router(test_config(std::env::temp_dir()));
        let token = authenticate_token(app.clone(), "codex-control@openagents.com").await?;

        let control_request = Request::builder()
            .method("POST")
            .uri("/api/runtime/codex/workers/desktopw%3Ashared/requests")
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {token}"))
            .body(Body::from(
                r#"{"request":{"request_id":"req_turn_1","method":"turn/start","params":{"thread_id":"thread-control-1","text":"continue"}}}"#,
            ))?;
        let control_response = app.clone().oneshot(control_request).await?;
        assert_eq!(control_response.status(), StatusCode::OK);
        let control_body = read_json(control_response).await?;
        assert_eq!(control_body["data"]["method"], "turn/start");
        assert_eq!(control_body["data"]["request_id"], "req_turn_1");
        assert_eq!(control_body["data"]["idempotent_replay"], false);
        assert_eq!(
            control_body["data"]["response"]["thread_id"],
            "thread-control-1"
        );
        assert!(
            control_body["data"]["response"]["turn"]["id"]
                .as_str()
                .unwrap_or_default()
                .starts_with("turn_")
        );

        let list_messages_request = Request::builder()
            .method("GET")
            .uri("/api/runtime/threads/thread-control-1/messages")
            .header("authorization", format!("Bearer {token}"))
            .body(Body::empty())?;
        let list_messages_response = app.oneshot(list_messages_request).await?;
        assert_eq!(list_messages_response.status(), StatusCode::OK);
        let list_messages_body = read_json(list_messages_response).await?;
        assert_eq!(
            list_messages_body["data"]["messages"][0]["text"],
            serde_json::json!("continue")
        );

        Ok(())
    }

    #[tokio::test]
    async fn runtime_codex_control_request_replays_duplicate_request_ids() -> Result<()> {
        let app = build_router(test_config(std::env::temp_dir()));
        let token = authenticate_token(app.clone(), "codex-replay@openagents.com").await?;
        let payload = r#"{"request":{"request_id":"req_replay_1","method":"thread/start","params":{"thread_id":"thread-replay-1"}}}"#;

        let first_request = Request::builder()
            .method("POST")
            .uri("/api/runtime/codex/workers/desktopw%3Ashared/requests")
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {token}"))
            .body(Body::from(payload))?;
        let first_response = app.clone().oneshot(first_request).await?;
        assert_eq!(first_response.status(), StatusCode::OK);
        let first_body = read_json(first_response).await?;
        assert_eq!(first_body["data"]["idempotent_replay"], false);

        let second_request = Request::builder()
            .method("POST")
            .uri("/api/runtime/codex/workers/desktopw%3Ashared/requests")
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {token}"))
            .body(Body::from(payload))?;
        let second_response = app.oneshot(second_request).await?;
        assert_eq!(second_response.status(), StatusCode::OK);
        let second_body = read_json(second_response).await?;
        assert_eq!(second_body["data"]["idempotent_replay"], true);
        assert_eq!(
            second_body["data"]["response"]["thread_id"],
            serde_json::json!("thread-replay-1")
        );

        Ok(())
    }

    #[tokio::test]
    async fn runtime_codex_control_request_rejects_non_allowlisted_methods() -> Result<()> {
        let app = build_router(test_config(std::env::temp_dir()));
        let token = authenticate_token(app.clone(), "codex-invalid@openagents.com").await?;

        let request = Request::builder()
            .method("POST")
            .uri("/api/runtime/codex/workers/desktopw%3Ashared/requests")
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {token}"))
            .body(Body::from(
                r#"{"request":{"request_id":"req_bad_1","method":"shell/exec","params":{}}}"#,
            ))?;
        let response = app.oneshot(request).await?;
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        let body = read_json(response).await?;
        assert_eq!(body["error"]["code"], "invalid_request");

        Ok(())
    }

    #[tokio::test]
    async fn auth_failure_paths_emit_failure_audit_events() -> Result<()> {
        let static_dir = tempdir()?;
        let sink = Arc::new(RecordingAuditSink::default());
        let app = build_router_with_observability(
            test_config(static_dir.path().to_path_buf()),
            Observability::new(sink.clone()),
        );

        let send_request = Request::builder()
            .method("POST")
            .uri("/api/auth/email")
            .header("x-request-id", "req-auth-send")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"email":"failure-audit@openagents.com"}"#))?;
        let send_response = app.clone().oneshot(send_request).await?;
        assert_eq!(send_response.status(), StatusCode::OK);
        let cookie = cookie_value(&send_response).unwrap_or_default();

        let verify_request = Request::builder()
            .method("POST")
            .uri("/api/auth/verify")
            .header("x-request-id", "req-auth-verify-failed")
            .header("content-type", "application/json")
            .header("x-client", "autopilot-ios")
            .header("cookie", cookie)
            .body(Body::from(r#"{"code":"000000"}"#))?;
        let verify_response = app.clone().oneshot(verify_request).await?;
        assert_eq!(verify_response.status(), StatusCode::UNPROCESSABLE_ENTITY);

        let refresh_request = Request::builder()
            .method("POST")
            .uri("/api/auth/refresh")
            .header("x-request-id", "req-auth-refresh-failed")
            .header("content-type", "application/json")
            .body(Body::from(
                r#"{"refresh_token":"oa_rt_invalid","rotate_refresh_token":true}"#,
            ))?;
        let refresh_response = app.clone().oneshot(refresh_request).await?;
        assert_eq!(refresh_response.status(), StatusCode::UNAUTHORIZED);

        let valid_session_token =
            authenticate_token(app.clone(), "logout-failure@openagents.com").await?;
        let create_pat_request = Request::builder()
            .method("POST")
            .uri("/api/tokens")
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {valid_session_token}"))
            .body(Body::from(r#"{"name":"logout-failure-pat"}"#))?;
        let create_pat_response = app.clone().oneshot(create_pat_request).await?;
        assert_eq!(create_pat_response.status(), StatusCode::CREATED);
        let create_pat_body = read_json(create_pat_response).await?;
        let pat_token = create_pat_body["data"]["token"]
            .as_str()
            .unwrap_or_default()
            .to_string();

        let logout_request = Request::builder()
            .method("POST")
            .uri("/api/auth/logout")
            .header("x-request-id", "req-auth-logout-failed")
            .header("authorization", format!("Bearer {pat_token}"))
            .body(Body::empty())?;
        let logout_response = app.oneshot(logout_request).await?;
        assert_eq!(logout_response.status(), StatusCode::UNAUTHORIZED);

        let events = sink.events();
        let verify_failed = events
            .iter()
            .find(|event| event.event_name == "auth.verify.failed")
            .expect("missing auth.verify.failed audit event");
        assert_eq!(verify_failed.request_id, "req-auth-verify-failed");
        assert_eq!(verify_failed.outcome, "failure");
        assert_eq!(
            verify_failed.attributes.get("reason").map(String::as_str),
            Some("invalid_request")
        );

        let refresh_failed = events
            .iter()
            .find(|event| event.event_name == "auth.refresh.failed")
            .expect("missing auth.refresh.failed audit event");
        assert_eq!(refresh_failed.request_id, "req-auth-refresh-failed");
        assert_eq!(refresh_failed.outcome, "failure");
        assert_eq!(
            refresh_failed.attributes.get("reason").map(String::as_str),
            Some("unauthorized")
        );

        let logout_failed = events
            .iter()
            .find(|event| event.event_name == "auth.logout.failed")
            .expect("missing auth.logout.failed audit event");
        assert_eq!(logout_failed.request_id, "req-auth-logout-failed");
        assert_eq!(logout_failed.outcome, "failure");
        assert_eq!(
            logout_failed.attributes.get("reason").map(String::as_str),
            Some("unauthorized")
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

        let root_request = Request::builder()
            .uri("/")
            .header("x-oa-route-key", "user:route")
            .body(Body::empty())?;
        let root_response = app.clone().oneshot(root_request).await?;
        assert_eq!(root_response.status(), StatusCode::OK);
        let root_body = root_response.into_body().collect().await?.to_bytes();
        let root_html = String::from_utf8_lossy(&root_body);
        assert!(root_html.contains("rust shell"));

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

    #[tokio::test]
    async fn retired_aui_route_returns_not_found() -> Result<()> {
        let static_dir = tempdir()?;
        std::fs::write(
            static_dir.path().join("index.html"),
            "<!doctype html><html><body>rust shell</body></html>",
        )?;
        let app = build_router(test_config(static_dir.path().to_path_buf()));

        let request = Request::builder().uri("/aui").body(Body::empty())?;
        let response = app.oneshot(request).await?;
        assert_eq!(response.status(), StatusCode::NOT_FOUND);

        Ok(())
    }

    #[tokio::test]
    async fn route_split_domain_override_only_affects_selected_route_group() -> Result<()> {
        let static_dir = tempdir()?;
        std::fs::write(
            static_dir.path().join("index.html"),
            "<!doctype html><html><body>rust shell</body></html>",
        )?;
        let mut config = test_config(static_dir.path().to_path_buf());
        config.route_split_mode = "rust".to_string();
        config.route_split_rust_routes = vec!["/".to_string()];
        let app = build_router(config);
        let token = authenticate_token(app.clone(), "routes@openagents.com").await?;

        let override_request = Request::builder()
            .method("POST")
            .uri("/api/v1/control/route-split/override")
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {token}"))
            .body(Body::from(r#"{"target":"legacy","domain":"billing_l402"}"#))?;
        let override_response = app.clone().oneshot(override_request).await?;
        assert_eq!(override_response.status(), StatusCode::OK);

        let billing_request = Request::builder()
            .uri("/l402/paywalls")
            .header("x-oa-route-key", "user:route")
            .body(Body::empty())?;
        let billing_response = app.clone().oneshot(billing_request).await?;
        assert_eq!(billing_response.status(), StatusCode::TEMPORARY_REDIRECT);
        assert_eq!(
            billing_response
                .headers()
                .get("location")
                .and_then(|value| value.to_str().ok()),
            Some("https://legacy.openagents.test/l402/paywalls")
        );

        let settings_request = Request::builder()
            .uri("/settings/profile")
            .header("x-oa-route-key", "user:route")
            .body(Body::empty())?;
        let settings_response = app.oneshot(settings_request).await?;
        assert_eq!(settings_response.status(), StatusCode::OK);
        let body = settings_response.into_body().collect().await?.to_bytes();
        let html = String::from_utf8_lossy(&body);
        assert!(html.contains("rust shell"));

        Ok(())
    }

    #[tokio::test]
    async fn route_split_status_exposes_rollback_matrix_and_domain_overrides() -> Result<()> {
        let static_dir = tempdir()?;
        std::fs::write(
            static_dir.path().join("index.html"),
            "<!doctype html><html><body>rust shell</body></html>",
        )?;
        let mut config = test_config(static_dir.path().to_path_buf());
        config.route_split_mode = "rust".to_string();
        config.route_split_rust_routes = vec!["/".to_string()];
        let app = build_router(config);
        let token = authenticate_token(app.clone(), "routes@openagents.com").await?;

        let override_request = Request::builder()
            .method("POST")
            .uri("/api/v1/control/route-split/override")
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {token}"))
            .body(Body::from(
                r#"{"target":"rollback","domain":"billing_l402"}"#,
            ))?;
        let override_response = app.clone().oneshot(override_request).await?;
        assert_eq!(override_response.status(), StatusCode::OK);

        let status_request = Request::builder()
            .method("GET")
            .uri("/api/v1/control/route-split/status")
            .header("authorization", format!("Bearer {token}"))
            .body(Body::empty())?;
        let status_response = app.oneshot(status_request).await?;
        assert_eq!(status_response.status(), StatusCode::OK);

        let body = read_json(status_response).await?;
        assert_eq!(
            body["data"]["rollback_matrix"]["billing_l402"],
            json!("legacy")
        );
        assert_eq!(
            body["data"]["rollback_matrix"]["chat_pilot"],
            json!("rust_shell")
        );
        assert_eq!(
            body["data"]["domain_overrides"]["billing_l402"],
            json!("legacy")
        );

        Ok(())
    }
}
