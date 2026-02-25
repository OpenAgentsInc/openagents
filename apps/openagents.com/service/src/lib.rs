use std::collections::{BTreeMap, HashMap, VecDeque};
use std::path::Path as FsPath;
use std::sync::Arc;
use std::time::SystemTime;

use autopilot_app::{InboxAuditEntry, InboxSnapshot, InboxThreadSummary};
use autopilot_inbox_domain::{
    PolicyDecision, classify_thread, compose_local_draft, infer_style_signature_from_bodies,
    risk_to_str,
};
use axum::body::to_bytes;
use axum::body::{Body, Bytes};
use axum::extract::{Form, Path, Query, Request, State};
use axum::http::header::{
    ACCEPT_ENCODING, AUTHORIZATION, CACHE_CONTROL, CONTENT_ENCODING, CONTENT_TYPE, COOKIE, ETAG,
    IF_NONE_MATCH, SET_COOKIE, VARY,
};
use axum::http::{HeaderMap, HeaderValue, Method, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::get;
use axum::{Json, Router};
use base64::Engine as _;
use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use chrono::{Duration, NaiveDate, SecondsFormat, Utc};
use hmac::{Hmac, Mac};
use openagents_client_core::compatibility::{
    ClientCompatibilityHandshake, CompatibilityFailure, CompatibilitySurface, CompatibilityWindow,
    negotiate_compatibility,
};
use openagents_l402::{Bolt11, WwwAuthenticateParser};
use openagents_runtime_client::{
    RuntimeClientError, RuntimeInternalClient, RuntimeWorkerHeartbeatRequest,
    RuntimeWorkerRegisterRequest, RuntimeWorkerTransitionRequest,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::sync::Mutex;
use tower::ServiceBuilder;
use tower_http::request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer};
use tower_http::trace::TraceLayer;
use uuid::Uuid;

pub mod api_envelope;
pub mod auth;
mod auth_routes;
pub mod codex_threads;
pub mod config;
pub mod domain_store;
pub mod khala_token;
pub mod observability;
pub mod openapi;
mod render;
mod route_domains;
pub mod route_split;
mod runtime_admin;
pub mod runtime_ownership;
pub mod runtime_routing;
mod stats;
mod sync_handlers;
pub mod sync_token;
pub mod vercel_sse_adapter;
pub mod web_htmx;
pub mod web_maud;

use crate::api_envelope::{
    ApiErrorCode, ApiErrorDetail, ApiErrorResponse, error_response_with_status, forbidden_error,
    not_found_error, ok_data, unauthorized_error, validation_error,
};
use crate::auth::{
    AuthError, AuthService, AuthUser, PolicyCheckRequest, SessionBundle, SessionRevocationReason,
    SessionRevocationRequest, SessionRevocationTarget,
};
use crate::auth_routes::{
    auth_register, current_session, list_sessions, local_test_login, login_email, login_page,
    login_verify, me, revoke_sessions, send_email_code, verify_email_code, web_logout,
};
use crate::codex_threads::{
    AutopilotThreadProjection, CodexThreadStore, ThreadMessageProjection, ThreadProjection,
    ThreadStoreError,
};
use crate::config::Config;
use crate::domain_store::{
    AutopilotAggregate, CreateAutopilotInput, CreateL402PaywallInput, CreateShoutInput,
    DomainStore, DomainStoreError, L402GatewayEventRecord, L402PaywallRecord, L402ReceiptRecord,
    MarkWebhookEventVerifiedInput, RecordInboxAuditInput, RecordL402GatewayEventInput,
    RecordL402ReceiptInput, RecordWebhookEventInput, SendWhisperInput, ShoutRecord,
    UpdateAutopilotInput, UpdateL402PaywallInput, UpsertAutopilotPolicyInput,
    UpsertAutopilotProfileInput, UpsertGoogleIntegrationInput, UpsertInboxThreadStateInput,
    UpsertL402ApprovalTaskInput, UpsertL402CredentialInput, UpsertResendIntegrationInput,
    UpsertRuntimeDriverOverrideInput, UpsertUserSparkWalletInput, UserIntegrationRecord,
    UserSparkWalletRecord, WhisperRecord, ZoneCount,
};
use crate::khala_token::{KhalaTokenError, KhalaTokenIssueRequest, KhalaTokenIssuer};
use crate::observability::{AuditEvent, Observability};
use crate::openapi::{
    ROUTE_AGENT_PAYMENTS_BALANCE, ROUTE_AGENT_PAYMENTS_INVOICE, ROUTE_AGENT_PAYMENTS_PAY,
    ROUTE_AGENT_PAYMENTS_SEND_SPARK, ROUTE_AGENT_PAYMENTS_WALLET, ROUTE_AGENTS_ME_BALANCE,
    ROUTE_AGENTS_ME_WALLET, ROUTE_AUTH_EMAIL, ROUTE_AUTH_LOGOUT, ROUTE_AUTH_REFRESH,
    ROUTE_AUTH_REGISTER, ROUTE_AUTH_SESSION, ROUTE_AUTH_SESSIONS, ROUTE_AUTH_SESSIONS_REVOKE,
    ROUTE_AUTH_VERIFY, ROUTE_AUTOPILOTS, ROUTE_AUTOPILOTS_BY_ID, ROUTE_AUTOPILOTS_STREAM,
    ROUTE_AUTOPILOTS_THREADS, ROUTE_INBOX_REFRESH, ROUTE_INBOX_REPLY_SEND,
    ROUTE_INBOX_THREAD_APPROVE, ROUTE_INBOX_THREAD_DETAIL, ROUTE_INBOX_THREAD_REJECT,
    ROUTE_INBOX_THREADS, ROUTE_KHALA_TOKEN, ROUTE_L402_DEPLOYMENTS, ROUTE_L402_PAYWALL_BY_ID,
    ROUTE_L402_PAYWALLS, ROUTE_L402_SETTLEMENTS, ROUTE_L402_TRANSACTION_BY_ID,
    ROUTE_L402_TRANSACTIONS, ROUTE_L402_WALLET, ROUTE_LEGACY_CHAT_STREAM,
    ROUTE_LEGACY_CHATS_STREAM, ROUTE_LIGHTNING_OPS_CONTROL_PLANE_MUTATION,
    ROUTE_LIGHTNING_OPS_CONTROL_PLANE_QUERY, ROUTE_ME, ROUTE_OPENAPI_JSON, ROUTE_ORGS_ACTIVE,
    ROUTE_ORGS_MEMBERSHIPS, ROUTE_PAYMENTS_INVOICE, ROUTE_PAYMENTS_PAY, ROUTE_PAYMENTS_SEND_SPARK,
    ROUTE_POLICY_AUTHORIZE, ROUTE_RUNTIME_CODEX_WORKER_BY_ID, ROUTE_RUNTIME_CODEX_WORKER_EVENTS,
    ROUTE_RUNTIME_CODEX_WORKER_REQUESTS, ROUTE_RUNTIME_CODEX_WORKER_STOP,
    ROUTE_RUNTIME_CODEX_WORKER_STREAM, ROUTE_RUNTIME_CODEX_WORKERS,
    ROUTE_RUNTIME_INTERNAL_SECRET_FETCH, ROUTE_RUNTIME_SKILLS_RELEASE,
    ROUTE_RUNTIME_SKILLS_SKILL_SPEC_PUBLISH, ROUTE_RUNTIME_SKILLS_SKILL_SPECS,
    ROUTE_RUNTIME_SKILLS_TOOL_SPECS, ROUTE_RUNTIME_THREAD_MESSAGES, ROUTE_RUNTIME_THREADS,
    ROUTE_RUNTIME_TOOLS_EXECUTE, ROUTE_RUNTIME_WORKER_BY_ID, ROUTE_RUNTIME_WORKER_HEARTBEAT,
    ROUTE_RUNTIME_WORKER_STATUS, ROUTE_RUNTIME_WORKERS, ROUTE_SETTINGS_AUTOPILOT,
    ROUTE_SETTINGS_INTEGRATIONS_GOOGLE, ROUTE_SETTINGS_INTEGRATIONS_GOOGLE_CALLBACK,
    ROUTE_SETTINGS_INTEGRATIONS_GOOGLE_REDIRECT, ROUTE_SETTINGS_INTEGRATIONS_RESEND,
    ROUTE_SETTINGS_INTEGRATIONS_RESEND_TEST, ROUTE_SETTINGS_PROFILE, ROUTE_SHOUTS,
    ROUTE_SHOUTS_ZONES, ROUTE_SMOKE_STREAM, ROUTE_SPACETIME_TOKEN, ROUTE_SYNC_TOKEN, ROUTE_TOKENS,
    ROUTE_TOKENS_BY_ID, ROUTE_TOKENS_CURRENT, ROUTE_V1_AUTH_SESSION, ROUTE_V1_AUTH_SESSIONS,
    ROUTE_V1_AUTH_SESSIONS_REVOKE, ROUTE_V1_CONTROL_ROUTE_SPLIT_EVALUATE,
    ROUTE_V1_CONTROL_ROUTE_SPLIT_OVERRIDE, ROUTE_V1_CONTROL_ROUTE_SPLIT_STATUS,
    ROUTE_V1_CONTROL_RUNTIME_ROUTING_EVALUATE, ROUTE_V1_CONTROL_RUNTIME_ROUTING_OVERRIDE,
    ROUTE_V1_CONTROL_RUNTIME_ROUTING_STATUS, ROUTE_V1_CONTROL_STATUS, ROUTE_V1_SPACETIME_TOKEN,
    ROUTE_V1_SYNC_TOKEN, ROUTE_WEBHOOKS_RESEND, ROUTE_WHISPERS, ROUTE_WHISPERS_READ,
    openapi_document,
};
use crate::render::{
    apply_html_security_headers, apply_static_security_headers,
    chat_thread_select_fragment_response, chat_views_for_bundle,
    feed_items_append_fragment_response, feed_main_select_fragment_response,
    session_view_from_bundle, web_fragment_response, web_response_for_page, web_shell_entry,
};
use crate::route_domains::{build_protected_api_router, build_public_api_router};
use crate::route_split::{
    HtmxModeDecision, HtmxModeTarget, RouteSplitDecision, RouteSplitService, RouteTarget,
};
use crate::runtime_admin::{
    route_split_evaluate, route_split_override, route_split_status, runtime_routing_evaluate,
    runtime_routing_override, runtime_routing_status,
};
use crate::runtime_ownership::{RuntimeRouteOwner, runtime_route_owner, runtime_route_ownership};
use crate::runtime_routing::{RuntimeDriver, RuntimeRoutingResolveInput, RuntimeRoutingService};
use crate::stats::{
    compute_fleet_fragment, compute_main_fragment, compute_metrics_fragment, compute_page,
    stats_main_fragment, stats_metrics_fragment, stats_page, stats_pools_fragment,
    web_compute_provider_disable,
};
use crate::sync_handlers::sync_token;
use crate::sync_token::{SyncTokenError, SyncTokenIssueRequest, SyncTokenIssuer};
use crate::web_htmx::{
    HtmxRequest, classify_request as classify_htmx_request,
    notice_response as htmx_notice_response, redirect_response as htmx_redirect_response,
    set_push_url_header as htmx_set_push_url_header, set_trigger_header as htmx_set_trigger_header,
};
use crate::web_maud::{
    ChatMessageView, ChatThreadView, ComputeDeviceView, ComputeMetricsView, ComputeProviderView,
    FeedItemView, FeedZoneView, IntegrationStatusView, L402DeploymentView, L402PaywallView,
    L402TransactionView, L402WalletSummaryView, LiquidityPoolView, LiquidityStatsMetricsView,
    SessionView, WebBody, WebPage,
    render_chat_thread_select_fragment as render_maud_chat_thread_select_fragment,
    render_compute_fleet_fragment as render_maud_compute_fleet_fragment,
    render_compute_metrics_fragment as render_maud_compute_metrics_fragment,
    render_feed_items_append_fragment as render_maud_feed_items_append_fragment,
    render_feed_main_select_fragment as render_maud_feed_main_select_fragment,
    render_main_fragment as render_maud_main_fragment, render_page as render_maud_page,
    render_stats_metrics_fragment as render_maud_stats_metrics_fragment,
    render_stats_pools_fragment as render_maud_stats_pools_fragment,
};

const SERVICE_NAME: &str = "openagents-control-service";
const CHALLENGE_COOKIE_NAME: &str = "oa_magic_challenge";
const AUTH_ACCESS_COOKIE_NAME: &str = "oa_access_token";
const AUTH_REFRESH_COOKIE_NAME: &str = "oa_refresh_token";
const LOCAL_TEST_AUTH_COOKIE_NAME: &str = "oa_local_test_auth";
const CACHE_API_NO_STORE: &str = "no-store";
const CACHE_IMMUTABLE_ONE_YEAR: &str = "public, max-age=31536000, immutable";
const CACHE_SHORT_LIVED: &str = "public, max-age=60";
const CACHE_MANIFEST: &str = "no-cache, no-store, must-revalidate";
const HTML_CONTENT_SECURITY_POLICY: &str = "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data: blob:; font-src 'self' data:; connect-src 'self' ws: wss:; object-src 'none'; base-uri 'self'; frame-ancestors 'none'; form-action 'self'";
const HTML_REFERRER_POLICY: &str = "strict-origin-when-cross-origin";
const HTML_X_FRAME_OPTIONS: &str = "DENY";
const HTML_PERMISSIONS_POLICY: &str = "camera=(), microphone=(), geolocation=()";
const X_CONTENT_TYPE_OPTIONS_NOSNIFF: &str = "nosniff";
const CORS_ALLOW_HEADERS: &str = "authorization,content-type,x-request-id,x-xsrf-token,x-client";
const CORS_ALLOW_METHODS: &str = "GET,POST,PATCH,DELETE,OPTIONS";
const CORS_MAX_AGE_SECONDS: &str = "600";
const CORS_VARY_HEADERS: &str =
    "Origin, Access-Control-Request-Method, Access-Control-Request-Headers";
const HEADER_OA_CLIENT_BUILD_ID: &str = "x-oa-client-build-id";
const HEADER_OA_COMPAT_CODE: &str = "x-oa-compatibility-code";
const HEADER_OA_COMPAT_MAX_BUILD: &str = "x-oa-compatibility-max-client-build-id";
const HEADER_OA_COMPAT_MAX_SCHEMA: &str = "x-oa-compatibility-max-schema-version";
const HEADER_OA_COMPAT_MIN_BUILD: &str = "x-oa-compatibility-min-client-build-id";
const HEADER_OA_COMPAT_MIN_SCHEMA: &str = "x-oa-compatibility-min-schema-version";
const HEADER_OA_COMPAT_PROTOCOL: &str = "x-oa-compatibility-protocol-version";
const HEADER_OA_COMPAT_UPGRADE_REQUIRED: &str = "x-oa-compatibility-upgrade-required";
const HEADER_OA_COMPAT_MIGRATION_DOC: &str = "x-oa-compat-migration-doc";
const HEADER_OA_COMPAT_SUNSET_DATE: &str = "x-oa-compat-sunset-date";
const HEADER_OA_PROTOCOL_VERSION: &str = "x-oa-protocol-version";
const HEADER_OA_SMOKE: &str = "x-oa-smoke";
const HEADER_OA_SMOKE_SECRET: &str = "x-oa-smoke-secret";
const HEADER_OA_SCHEMA_VERSION: &str = "x-oa-schema-version";
const HEADER_X_FORWARDED_FOR: &str = "x-forwarded-for";
const HEADER_X_REAL_IP: &str = "x-real-ip";
const HEADER_CONTENT_SECURITY_POLICY: &str = "content-security-policy";
const HEADER_PERMISSIONS_POLICY: &str = "permissions-policy";
const HEADER_REFERRER_POLICY: &str = "referrer-policy";
const RUNTIME_INTERNAL_BODY_HASH_HEADER: &str = "x-oa-internal-body-sha256";
const RUNTIME_INTERNAL_KEY_ID_HEADER: &str = "x-oa-internal-key-id";
const RUNTIME_INTERNAL_NONCE_HEADER: &str = "x-oa-internal-nonce";
const RUNTIME_INTERNAL_SIGNATURE_HEADER: &str = "x-oa-internal-signature";
const RUNTIME_INTERNAL_TIMESTAMP_HEADER: &str = "x-oa-internal-timestamp";
const HEADER_X_CONTENT_TYPE_OPTIONS: &str = "x-content-type-options";
const HEADER_X_FRAME_OPTIONS: &str = "x-frame-options";
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
const COMPATIBILITY_LANE_MIGRATION_DOC: &str =
    "docs/audits/2026-02-25-oa-audit-phase5-compatibility-lane-signoff.md";
const COMPATIBILITY_LANE_SUNSET_DATE_HTTP: &str = "Tue, 30 Jun 2026 00:00:00 GMT";
const COMPATIBILITY_LANE_SUNSET_DATE_ISO: &str = "2026-06-30";
const RESEND_WEBHOOK_PROVIDER: &str = "resend";
const RESEND_SVIX_ID_HEADER: &str = "svix-id";
const RESEND_SVIX_TIMESTAMP_HEADER: &str = "svix-timestamp";
const RESEND_SVIX_SIGNATURE_HEADER: &str = "svix-signature";
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
const AUTOPILOT_AUTHENTICATED_TOOLS: &[&str] = &[
    "openagents_api",
    "lightning_l402_fetch",
    "lightning_l402_approve",
    "lightning_l402_paywall_create",
    "lightning_l402_paywall_update",
    "lightning_l402_paywall_delete",
];
const AUTOPILOT_ALL_TOOLS: &[&str] = &[
    "chat_login",
    "openagents_api",
    "lightning_l402_fetch",
    "lightning_l402_approve",
    "lightning_l402_paywall_create",
    "lightning_l402_paywall_update",
    "lightning_l402_paywall_delete",
];
const AUTOPILOT_GUEST_ALLOWED_TOOLS: &[&str] = &["chat_login", "openagents_api"];
const RUNTIME_TOOL_PACK_CODING_V1: &str = "coding.v1";
const RUNTIME_TOOL_PACK_LIGHTNING_V1: &str = "lightning.v1";
const RUNTIME_TOOL_PACK_L402_ALIAS_V1: &str = "l402.v1";
type HmacSha256 = Hmac<Sha256>;

#[derive(Clone)]
struct AppState {
    config: Arc<Config>,
    auth: AuthService,
    observability: Observability,
    route_split: RouteSplitService,
    runtime_routing: RuntimeRoutingService,
    khala_token_issuer: KhalaTokenIssuer,
    sync_token_issuer: SyncTokenIssuer,
    codex_thread_store: CodexThreadStore,
    _domain_store: DomainStore,
    runtime_revocation_client: Option<RuntimeRevocationClient>,
    throttle_state: ThrottleState,
    codex_control_receipts: CodexControlReceiptState,
    runtime_tool_receipts: RuntimeToolReceiptState,
    runtime_skill_registry: RuntimeSkillRegistryState,
    runtime_workers: RuntimeWorkerState,
    lightning_ops_control_plane: LightningOpsControlPlaneState,
    runtime_internal_nonces: RuntimeInternalNonceState,
    google_oauth_states: GoogleOauthStateStore,
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

#[derive(Clone, Default)]
struct RuntimeToolReceiptState {
    entries: Arc<Mutex<HashMap<String, serde_json::Value>>>,
}

#[derive(Clone, Default)]
struct RuntimeSkillRegistryState {
    tool_specs: Arc<Mutex<HashMap<String, serde_json::Value>>>,
    skill_specs: Arc<Mutex<HashMap<String, serde_json::Value>>>,
    releases: Arc<Mutex<HashMap<String, serde_json::Value>>>,
}

#[derive(Clone, Default)]
struct RuntimeWorkerState {
    workers: Arc<Mutex<HashMap<String, RuntimeWorkerRecord>>>,
    events: Arc<Mutex<HashMap<String, Vec<RuntimeWorkerEventRecord>>>>,
}

#[derive(Clone, Default)]
struct GoogleOauthStateStore {
    entries: Arc<Mutex<HashMap<String, String>>>,
}

#[derive(Clone, Default)]
struct LightningOpsControlPlaneState {
    store: Arc<Mutex<LightningOpsControlPlaneStore>>,
}

#[derive(Default)]
struct LightningOpsControlPlaneStore {
    deployments: HashMap<String, LightningOpsDeploymentRecord>,
    gateway_events: Vec<LightningOpsGatewayEventRecord>,
    invoices: HashMap<String, LightningOpsInvoiceRecord>,
    settlements: HashMap<String, LightningOpsSettlementRecord>,
    global_security: Option<LightningOpsGlobalSecurityRecord>,
    owner_controls: HashMap<String, LightningOpsOwnerControlRecord>,
    credential_roles: HashMap<String, LightningOpsCredentialRoleRecord>,
}

#[derive(Clone)]
struct LightningOpsDeploymentRecord {
    deployment_id: String,
    paywall_id: Option<String>,
    owner_id: Option<String>,
    config_hash: String,
    image_digest: Option<String>,
    status: String,
    diagnostics: Option<serde_json::Value>,
    metadata: Option<serde_json::Value>,
    request_id: Option<String>,
    applied_at_ms: Option<i64>,
    rolled_back_from: Option<String>,
    created_at: chrono::DateTime<Utc>,
    updated_at: chrono::DateTime<Utc>,
}

#[derive(Clone)]
struct LightningOpsGatewayEventRecord {
    event_id: String,
    paywall_id: String,
    owner_id: String,
    event_type: String,
    level: String,
    request_id: Option<String>,
    metadata: Option<serde_json::Value>,
    created_at: chrono::DateTime<Utc>,
}

#[derive(Clone)]
struct LightningOpsInvoiceRecord {
    invoice_id: String,
    paywall_id: String,
    owner_id: String,
    amount_msats: i64,
    status: String,
    payment_hash: Option<String>,
    payment_request: Option<String>,
    payment_proof_ref: Option<String>,
    request_id: Option<String>,
    settled_at_ms: Option<i64>,
    created_at: chrono::DateTime<Utc>,
    updated_at: chrono::DateTime<Utc>,
}

#[derive(Clone)]
struct LightningOpsSettlementRecord {
    settlement_id: String,
    paywall_id: String,
    owner_id: String,
    invoice_id: Option<String>,
    amount_msats: i64,
    payment_proof_ref: String,
    request_id: Option<String>,
    metadata: Option<serde_json::Value>,
    created_at: chrono::DateTime<Utc>,
}

#[derive(Clone)]
struct LightningOpsGlobalSecurityRecord {
    global_pause: bool,
    deny_reason_code: Option<String>,
    deny_reason: Option<String>,
    updated_by: Option<String>,
    updated_at_ms: i64,
}

#[derive(Clone)]
struct LightningOpsOwnerControlRecord {
    owner_id: String,
    kill_switch: bool,
    deny_reason_code: Option<String>,
    deny_reason: Option<String>,
    updated_by: Option<String>,
    updated_at_ms: i64,
}

#[derive(Clone)]
struct LightningOpsCredentialRoleRecord {
    role: String,
    status: String,
    version: i64,
    fingerprint: Option<String>,
    note: Option<String>,
    updated_at_ms: i64,
    last_rotated_at_ms: Option<i64>,
    revoked_at_ms: Option<i64>,
}

#[derive(Clone)]
struct RuntimeWorkerRecord {
    worker_id: String,
    owner_user_id: String,
    status: String,
    latest_seq: u64,
    workspace_ref: Option<String>,
    codex_home_ref: Option<String>,
    adapter: String,
    metadata: Option<serde_json::Value>,
    started_at: Option<chrono::DateTime<Utc>>,
    stopped_at: Option<chrono::DateTime<Utc>>,
    last_heartbeat_at: Option<chrono::DateTime<Utc>>,
    heartbeat_stale_after_ms: u64,
    updated_at: chrono::DateTime<Utc>,
}

#[derive(Clone)]
struct RuntimeWorkerEventRecord {
    seq: u64,
    event_type: String,
    payload: serde_json::Value,
    occurred_at: chrono::DateTime<Utc>,
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

#[derive(Debug, Deserialize, Default)]
struct L402AutopilotQuery {
    #[serde(default)]
    autopilot: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct L402TransactionsQuery {
    #[serde(default)]
    autopilot: Option<String>,
    #[serde(default, alias = "perPage")]
    per_page: Option<usize>,
    #[serde(default)]
    page: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct L402PaywallCreateRequestPayload {
    name: String,
    #[serde(alias = "hostRegexp")]
    host_regexp: String,
    #[serde(alias = "pathRegexp")]
    path_regexp: String,
    #[serde(alias = "priceMsats")]
    price_msats: u64,
    upstream: String,
    #[serde(default)]
    enabled: Option<bool>,
    #[serde(default, alias = "metadata")]
    meta: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize, Default)]
struct L402PaywallUpdateRequestPayload {
    #[serde(default)]
    name: Option<String>,
    #[serde(default, alias = "hostRegexp")]
    host_regexp: Option<String>,
    #[serde(default, alias = "pathRegexp")]
    path_regexp: Option<String>,
    #[serde(default, alias = "priceMsats")]
    price_msats: Option<u64>,
    #[serde(default)]
    upstream: Option<String>,
    #[serde(default)]
    enabled: Option<bool>,
    #[serde(default, alias = "metadata")]
    meta: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize, Default)]
struct AgentPaymentsWalletUpsertRequestPayload {
    #[serde(default)]
    mnemonic: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AgentPaymentsCreateInvoiceRequestPayload {
    #[serde(alias = "amountSats")]
    amount_sats: u64,
    #[serde(default)]
    description: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AgentPaymentsPayInvoiceRequestPayload {
    invoice: String,
    #[serde(default, alias = "maxAmountSats")]
    max_amount_sats: Option<u64>,
    #[serde(default, alias = "maxAmountMsats")]
    max_amount_msats: Option<u64>,
    #[serde(default, alias = "timeoutMs")]
    timeout_ms: Option<u64>,
    #[serde(default)]
    host: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AgentPaymentsSendSparkRequestPayload {
    #[serde(alias = "sparkAddress")]
    spark_address: String,
    #[serde(alias = "amountSats")]
    amount_sats: u64,
    #[serde(default, alias = "timeoutMs")]
    timeout_ms: Option<u64>,
}

#[derive(Debug, Clone)]
struct AgentInvoicePaymentResult {
    payment_id: Option<String>,
    preimage: String,
    status: String,
    raw: serde_json::Value,
}

#[derive(Debug, Clone)]
struct AgentInvoicePaymentError {
    status: StatusCode,
    code: String,
    message: String,
}

#[derive(Debug, Clone)]
struct AgentWalletExecutorConfig {
    base_url: String,
    auth_token: String,
    timeout_ms: u64,
}

#[derive(Debug, Deserialize, Default)]
struct ShoutsIndexQuery {
    #[serde(default)]
    zone: Option<String>,
    #[serde(default)]
    limit: Option<String>,
    #[serde(default, alias = "beforeId")]
    before_id: Option<String>,
    #[serde(default)]
    since: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct FeedPageQuery {
    #[serde(default)]
    zone: Option<String>,
    #[serde(default)]
    limit: Option<String>,
    #[serde(default, alias = "beforeId")]
    before_id: Option<String>,
    #[serde(default)]
    since: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct ShoutsZonesQuery {
    #[serde(default)]
    limit: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct ShoutStoreRequestPayload {
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    zone: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct WhispersIndexQuery {
    #[serde(default)]
    with: Option<String>,
    #[serde(default)]
    limit: Option<String>,
    #[serde(default, alias = "beforeId")]
    before_id: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct WhisperStoreRequestPayload {
    #[serde(default, alias = "recipientId")]
    recipient_id: Option<String>,
    #[serde(default, alias = "recipientHandle")]
    recipient_handle: Option<String>,
    #[serde(default)]
    body: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct UpsertResendIntegrationRequestPayload {
    #[serde(default, alias = "resendApiKey")]
    resend_api_key: Option<String>,
    #[serde(default, alias = "senderEmail")]
    sender_email: Option<String>,
    #[serde(default, alias = "senderName")]
    sender_name: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct GoogleOauthCallbackQuery {
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    state: Option<String>,
    #[serde(default)]
    code: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct InboxThreadsQuery {
    #[serde(default)]
    limit: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct InboxRefreshRequestPayload {
    #[serde(default)]
    limit: Option<u64>,
}

#[derive(Debug, Deserialize, Default)]
struct InboxDraftActionRequestPayload {
    #[serde(default)]
    detail: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct InboxSendReplyRequestPayload {
    #[serde(default)]
    body: Option<String>,
}

#[derive(Debug, Serialize)]
struct InboxSnapshotEnvelope {
    request_id: String,
    source: String,
    snapshot: InboxSnapshot,
}

#[derive(Debug, Serialize)]
struct InboxThreadDetailResponse {
    request_id: String,
    thread_id: String,
    thread: InboxThreadSummary,
    messages: Vec<InboxThreadMessage>,
    audit_log: Vec<InboxAuditEntry>,
}

#[derive(Debug, Serialize, Clone)]
struct InboxThreadMessage {
    id: String,
    from: Option<String>,
    to: Option<String>,
    subject: Option<String>,
    snippet: String,
    body: String,
    created_at: Option<String>,
}

#[derive(Debug, Serialize)]
struct InboxReplySendResponse {
    request_id: String,
    thread_id: String,
    message_id: String,
    status: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
struct GoogleIntegrationSecretPayload {
    #[serde(default)]
    refresh_token: Option<String>,
    #[serde(default)]
    access_token: Option<String>,
    #[serde(default)]
    scope: Option<String>,
    #[serde(default)]
    token_type: Option<String>,
    #[serde(default)]
    expires_at: Option<String>,
    #[serde(default)]
    obtained_at: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct GmailThreadListResponse {
    #[serde(default)]
    threads: Vec<GmailThreadListItem>,
}

#[derive(Debug, Deserialize, Default)]
struct GmailThreadListItem {
    id: String,
}

#[derive(Debug, Deserialize, Default)]
struct GmailThreadResponse {
    id: String,
    #[serde(default)]
    snippet: Option<String>,
    #[serde(default)]
    messages: Vec<GmailMessage>,
}

#[derive(Debug, Deserialize, Default)]
struct GmailMessage {
    id: String,
    #[serde(default)]
    snippet: Option<String>,
    #[serde(default, rename = "internalDate")]
    internal_date: Option<String>,
    #[serde(default)]
    payload: Option<GmailMessagePayload>,
}

#[derive(Debug, Deserialize, Default)]
struct GmailMessagePayload {
    #[serde(default, rename = "mimeType")]
    mime_type: Option<String>,
    #[serde(default)]
    headers: Vec<GmailHeader>,
    #[serde(default)]
    body: Option<GmailBody>,
    #[serde(default)]
    parts: Vec<GmailMessagePayload>,
}

#[derive(Debug, Deserialize, Default)]
struct GmailHeader {
    name: String,
    value: String,
}

#[derive(Debug, Deserialize, Default)]
struct GmailBody {
    #[serde(default)]
    data: Option<String>,
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
struct UpdateSettingsAutopilotRequestPayload {
    #[serde(default, alias = "displayName")]
    display_name: Option<String>,
    #[serde(default)]
    tagline: Option<String>,
    #[serde(default, alias = "ownerDisplayName")]
    owner_display_name: Option<String>,
    #[serde(default, alias = "personaSummary")]
    persona_summary: Option<String>,
    #[serde(default, alias = "autopilotVoice")]
    autopilot_voice: Option<String>,
    #[serde(default, alias = "principlesText")]
    principles_text: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SyncTokenRequestPayload {
    #[serde(default)]
    scopes: Vec<String>,
    #[serde(default)]
    streams: Vec<String>,
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
struct RuntimeRoutingEvaluateRequest {
    thread_id: String,
    #[serde(default)]
    user_id: Option<String>,
    #[serde(default)]
    autopilot_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RuntimeRoutingOverrideRequest {
    scope_type: String,
    scope_id: String,
    driver: String,
    #[serde(default)]
    is_active: Option<bool>,
    #[serde(default)]
    reason: Option<String>,
    #[serde(default)]
    meta: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct SendThreadMessageRequest {
    text: String,
}

#[derive(Debug, Deserialize, Default)]
struct WebChatSendForm {
    text: String,
}

#[derive(Debug, Deserialize, Default)]
struct WebShoutForm {
    body: String,
    #[serde(default)]
    zone: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct WebL402PaywallCreateForm {
    name: String,
    host_regexp: String,
    path_regexp: String,
    price_msats: String,
    upstream: String,
    #[serde(default)]
    enabled: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct WebRouteSplitEvaluateForm {
    path: String,
    #[serde(default)]
    cohort_key: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct WebRouteSplitOverrideForm {
    target: String,
    #[serde(default)]
    domain: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct WebRuntimeRoutingEvaluateForm {
    thread_id: String,
    #[serde(default)]
    autopilot_id: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct WebRuntimeRoutingOverrideForm {
    scope_type: String,
    scope_id: String,
    driver: String,
    #[serde(default)]
    is_active: Option<String>,
    #[serde(default)]
    reason: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct WebLightningOpsForm {
    function_name: String,
    #[serde(default)]
    args_json: Option<String>,
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
struct RuntimeCodexWorkersListQuery {
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    workspace_ref: Option<String>,
    #[serde(default)]
    limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct RuntimeCodexWorkerCreateRequestPayload {
    #[serde(default)]
    worker_id: Option<String>,
    #[serde(default)]
    workspace_ref: Option<String>,
    #[serde(default)]
    codex_home_ref: Option<String>,
    #[serde(default)]
    adapter: Option<String>,
    #[serde(default)]
    metadata: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct RuntimeCodexWorkerEventEnvelope {
    event: RuntimeCodexWorkerEventPayload,
}

#[derive(Debug, Deserialize)]
struct RuntimeCodexWorkerEventPayload {
    event_type: String,
    #[serde(default)]
    payload: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct RuntimeCodexWorkerStopRequestPayload {
    #[serde(default)]
    reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RuntimeCodexWorkerStreamQuery {
    #[serde(default)]
    cursor: Option<u64>,
    #[serde(default)]
    tail_ms: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct RuntimeToolsExecuteRequestPayload {
    #[serde(alias = "toolPack")]
    tool_pack: String,
    #[serde(default)]
    mode: Option<String>,
    #[serde(default)]
    manifest: Option<serde_json::Value>,
    #[serde(default, alias = "manifestRef")]
    manifest_ref: Option<serde_json::Value>,
    request: serde_json::Value,
    #[serde(default)]
    policy: Option<serde_json::Value>,
    #[serde(default, alias = "runId")]
    run_id: Option<String>,
    #[serde(default, alias = "threadId")]
    thread_id: Option<String>,
    #[serde(default, alias = "userId")]
    user_id: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct RuntimeToolSpecUpsertRequestPayload {
    tool_spec: serde_json::Value,
    #[serde(default)]
    state: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RuntimeSkillSpecUpsertRequestPayload {
    skill_spec: serde_json::Value,
    #[serde(default)]
    state: Option<String>,
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

fn assert_runtime_route_owner(method: &str, path: &'static str, expected: RuntimeRouteOwner) {
    debug_assert_eq!(
        runtime_route_owner(method, path),
        Some(expected),
        "runtime ownership mismatch for {} {}",
        method,
        path
    );
}

pub fn build_router(config: Config) -> Router {
    build_router_with_observability(config, Observability::default())
}

pub fn build_router_with_observability(config: Config, observability: Observability) -> Router {
    let auth = AuthService::from_config(&config);
    let route_split = RouteSplitService::from_config(&config);
    let runtime_routing = RuntimeRoutingService::from_config(&config);
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
        runtime_routing,
        khala_token_issuer,
        sync_token_issuer,
        codex_thread_store,
        _domain_store: domain_store,
        runtime_revocation_client,
        throttle_state: ThrottleState::default(),
        codex_control_receipts: CodexControlReceiptState::default(),
        runtime_tool_receipts: RuntimeToolReceiptState::default(),
        runtime_skill_registry: RuntimeSkillRegistryState::default(),
        runtime_workers: RuntimeWorkerState::default(),
        lightning_ops_control_plane: LightningOpsControlPlaneState::default(),
        runtime_internal_nonces: RuntimeInternalNonceState::default(),
        google_oauth_states: GoogleOauthStateStore::default(),
        started_at: SystemTime::now(),
    };
    let compatibility_state = state.clone();
    let maintenance_state = state.clone();
    let auth_email_throttle_state = state.clone();
    let thread_message_throttle_state = state.clone();
    let codex_control_request_throttle_state = state.clone();
    let authenticated_routes_state = state.clone();
    let runtime_internal_state = state.clone();
    let workos_session_state = state.clone();
    let admin_state = state.clone();
    let runtime_internal_secret_fetch_path =
        normalize_route_path(&state.config.runtime_internal_secret_fetch_path);

    // Runtime route ownership contract guards.
    assert_runtime_route_owner(
        "POST",
        ROUTE_RUNTIME_TOOLS_EXECUTE,
        RuntimeRouteOwner::ControlService,
    );
    assert_runtime_route_owner(
        "GET",
        ROUTE_RUNTIME_SKILLS_TOOL_SPECS,
        RuntimeRouteOwner::ControlService,
    );
    assert_runtime_route_owner(
        "POST",
        ROUTE_RUNTIME_SKILLS_TOOL_SPECS,
        RuntimeRouteOwner::ControlService,
    );
    assert_runtime_route_owner(
        "GET",
        ROUTE_RUNTIME_SKILLS_SKILL_SPECS,
        RuntimeRouteOwner::ControlService,
    );
    assert_runtime_route_owner(
        "POST",
        ROUTE_RUNTIME_SKILLS_SKILL_SPECS,
        RuntimeRouteOwner::ControlService,
    );
    assert_runtime_route_owner(
        "POST",
        ROUTE_RUNTIME_SKILLS_SKILL_SPEC_PUBLISH,
        RuntimeRouteOwner::ControlService,
    );
    assert_runtime_route_owner(
        "GET",
        ROUTE_RUNTIME_SKILLS_RELEASE,
        RuntimeRouteOwner::ControlService,
    );
    assert_runtime_route_owner(
        "GET",
        ROUTE_RUNTIME_THREADS,
        RuntimeRouteOwner::ControlService,
    );
    assert_runtime_route_owner(
        "GET",
        ROUTE_RUNTIME_THREAD_MESSAGES,
        RuntimeRouteOwner::ControlService,
    );
    assert_runtime_route_owner(
        "POST",
        ROUTE_RUNTIME_THREAD_MESSAGES,
        RuntimeRouteOwner::ControlService,
    );
    assert_runtime_route_owner(
        "GET",
        ROUTE_RUNTIME_CODEX_WORKERS,
        RuntimeRouteOwner::ControlService,
    );
    assert_runtime_route_owner(
        "POST",
        ROUTE_RUNTIME_CODEX_WORKERS,
        RuntimeRouteOwner::ControlService,
    );
    assert_runtime_route_owner(
        "GET",
        ROUTE_RUNTIME_CODEX_WORKER_BY_ID,
        RuntimeRouteOwner::ControlService,
    );
    assert_runtime_route_owner(
        "GET",
        ROUTE_RUNTIME_CODEX_WORKER_STREAM,
        RuntimeRouteOwner::ControlService,
    );
    assert_runtime_route_owner(
        "POST",
        ROUTE_RUNTIME_CODEX_WORKER_EVENTS,
        RuntimeRouteOwner::ControlService,
    );
    assert_runtime_route_owner(
        "POST",
        ROUTE_RUNTIME_CODEX_WORKER_STOP,
        RuntimeRouteOwner::ControlService,
    );
    assert_runtime_route_owner(
        "POST",
        ROUTE_RUNTIME_CODEX_WORKER_REQUESTS,
        RuntimeRouteOwner::ControlService,
    );
    assert_runtime_route_owner(
        "GET",
        ROUTE_RUNTIME_WORKERS,
        RuntimeRouteOwner::RuntimeService,
    );
    assert_runtime_route_owner(
        "POST",
        ROUTE_RUNTIME_WORKERS,
        RuntimeRouteOwner::RuntimeService,
    );
    assert_runtime_route_owner(
        "GET",
        ROUTE_RUNTIME_WORKER_BY_ID,
        RuntimeRouteOwner::RuntimeService,
    );
    assert_runtime_route_owner(
        "POST",
        ROUTE_RUNTIME_WORKER_HEARTBEAT,
        RuntimeRouteOwner::RuntimeService,
    );
    assert_runtime_route_owner(
        "POST",
        ROUTE_RUNTIME_WORKER_STATUS,
        RuntimeRouteOwner::RuntimeService,
    );

    let public_api_router = build_public_api_router(
        runtime_internal_secret_fetch_path.as_str(),
        runtime_internal_state,
        auth_email_throttle_state,
        state.clone(),
    );

    let protected_api_router = build_protected_api_router(
        thread_message_throttle_state,
        codex_control_request_throttle_state,
        admin_state,
        workos_session_state,
        authenticated_routes_state,
    );

    Router::new()
        .route("/", get(landing_page))
        .route("/download-desktop", get(download_desktop_redirect))
        .route("/healthz", get(health))
        .route("/readyz", get(readiness))
        .merge(public_api_router)
        .merge(protected_api_router)
        .route(ROUTE_OPENAPI_JSON, get(openapi_spec))
        .with_state(state)
        .layer(middleware::from_fn_with_state(
            maintenance_state,
            maintenance_mode_gate,
        ))
        .layer(middleware::from_fn_with_state(
            compatibility_state,
            control_compatibility_gate,
        ))
        .layer(middleware::from_fn(compatibility_lane_sunset_header_gate))
        .layer(middleware::from_fn(api_non_http_behavior_gate))
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

async fn landing_page() -> Response {
    const LANDING_HTML: &str = r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <title>OpenAgents Desktop</title>
  <style>
    :root { color-scheme: light; }
    body {
      margin: 0;
      min-height: 100vh;
      display: grid;
      place-items: center;
      background: linear-gradient(165deg, #f3f7ff 0%, #dce8ff 100%);
      font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
      color: #111827;
    }
    main {
      width: min(680px, calc(100% - 3rem));
      border-radius: 20px;
      background: rgba(255, 255, 255, 0.94);
      box-shadow: 0 20px 60px rgba(17, 24, 39, 0.12);
      padding: 2.5rem;
    }
    h1 {
      margin: 0 0 0.75rem;
      font-size: 2rem;
      line-height: 1.15;
    }
    p {
      margin: 0 0 1.5rem;
      color: #374151;
      line-height: 1.5;
    }
    a {
      display: inline-block;
      text-decoration: none;
      background: #0f172a;
      color: #ffffff;
      padding: 0.8rem 1.2rem;
      border-radius: 0.75rem;
      font-weight: 600;
    }
  </style>
</head>
<body>
  <main>
    <h1>OpenAgents runs in the desktop app.</h1>
    <p>This web surface is intentionally limited to desktop distribution.</p>
    <a href="/download-desktop">Download desktop app</a>
  </main>
</body>
</html>
"#;

    let mut response = (
        StatusCode::OK,
        [(CONTENT_TYPE, "text/html; charset=utf-8")],
        LANDING_HTML,
    )
        .into_response();
    response
        .headers_mut()
        .insert(CACHE_CONTROL, HeaderValue::from_static(CACHE_API_NO_STORE));
    apply_html_security_headers(response.headers_mut());
    response
}

async fn download_desktop_redirect(State(state): State<AppState>) -> Response {
    Redirect::temporary(&state.config.desktop_download_url).into_response()
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
        let request_id = request_id(&parts.headers);
        state.observability.audit(
            AuditEvent::new("runtime.internal.auth.rejected", request_id.clone())
                .with_attribute("path", parts.uri.path().to_string())
                .with_attribute("error_code", response.1.0.error.code.to_string()),
        );
        state
            .observability
            .increment_counter("runtime.internal.auth.rejected", &request_id);
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
            runtime_internal_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_auth_misconfigured",
                "runtime internal auth misconfigured",
            )
        })?;

    let provided_key_id =
        header_string(headers, RUNTIME_INTERNAL_KEY_ID_HEADER).unwrap_or_default();
    if provided_key_id.is_empty() || provided_key_id != state.config.runtime_internal_key_id {
        return Err(runtime_internal_error(
            StatusCode::UNAUTHORIZED,
            "invalid_key_id",
            "invalid key id",
        ));
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
        return Err(runtime_internal_error(
            StatusCode::UNAUTHORIZED,
            "missing_auth_headers",
            "missing auth headers",
        ));
    }

    let timestamp_epoch = timestamp.parse::<i64>().map_err(|_| {
        runtime_internal_error(
            StatusCode::UNAUTHORIZED,
            "invalid_timestamp",
            "invalid timestamp",
        )
    })?;

    let now_epoch = Utc::now().timestamp();
    let ttl_seconds = state.config.runtime_internal_signature_ttl_seconds as i64;
    if (now_epoch - timestamp_epoch).abs() > ttl_seconds {
        return Err(runtime_internal_error(
            StatusCode::UNAUTHORIZED,
            "signature_expired",
            "signature expired",
        ));
    }

    let computed_body_hash = sha256_hex(body);
    if computed_body_hash != provided_body_hash {
        return Err(runtime_internal_error(
            StatusCode::UNAUTHORIZED,
            "body_hash_mismatch",
            "body hash mismatch",
        ));
    }

    let payload = format!("{timestamp}\n{nonce}\n{computed_body_hash}");
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).map_err(|_| {
        runtime_internal_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal_auth_misconfigured",
            "runtime internal auth misconfigured",
        )
    })?;
    mac.update(payload.as_bytes());
    let expected_signature = sha256_bytes_hex(&mac.finalize().into_bytes());
    if expected_signature != provided_signature {
        return Err(runtime_internal_error(
            StatusCode::UNAUTHORIZED,
            "invalid_signature",
            "invalid signature",
        ));
    }

    let replay_key = format!("{provided_key_id}:{nonce}");
    let expires_at = timestamp_epoch + ttl_seconds + RUNTIME_INTERNAL_NONCE_GRACE_SECONDS;
    let mut entries = state.runtime_internal_nonces.entries.lock().await;
    entries.retain(|_, expiry| *expiry > now_epoch);
    if entries.contains_key(&replay_key) {
        return Err(runtime_internal_error(
            StatusCode::UNAUTHORIZED,
            "nonce_replay",
            "nonce replay detected",
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

fn is_api_request_path(path: &str) -> bool {
    path == "/api" || path.starts_with("/api/")
}

fn apply_api_non_http_headers(headers: &mut HeaderMap, origin: Option<&HeaderValue>) {
    let allow_origin = origin
        .cloned()
        .unwrap_or_else(|| HeaderValue::from_static("*"));
    headers.insert("access-control-allow-origin", allow_origin);
    headers.insert(
        "access-control-allow-methods",
        HeaderValue::from_static(CORS_ALLOW_METHODS),
    );
    headers.insert(
        "access-control-allow-headers",
        HeaderValue::from_static(CORS_ALLOW_HEADERS),
    );
    headers.insert(
        "access-control-max-age",
        HeaderValue::from_static(CORS_MAX_AGE_SECONDS),
    );
    headers.insert("vary", HeaderValue::from_static(CORS_VARY_HEADERS));
}

async fn api_non_http_behavior_gate(request: Request, next: Next) -> Response {
    let path = request.uri().path().to_string();
    let is_api = is_api_request_path(&path);
    let origin = request.headers().get("origin").cloned();

    if is_api && request.method() == Method::OPTIONS {
        let mut response = StatusCode::NO_CONTENT.into_response();
        apply_api_non_http_headers(response.headers_mut(), origin.as_ref());
        response
            .headers_mut()
            .insert(CACHE_CONTROL, HeaderValue::from_static(CACHE_API_NO_STORE));
        return response;
    }

    let mut response = next.run(request).await;
    if is_api {
        apply_api_non_http_headers(response.headers_mut(), origin.as_ref());
        if !response.headers().contains_key(CACHE_CONTROL) {
            response
                .headers_mut()
                .insert(CACHE_CONTROL, HeaderValue::from_static(CACHE_API_NO_STORE));
        }
    }
    response
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

async fn compatibility_lane_sunset_header_gate(request: Request, next: Next) -> Response {
    let path = request.uri().path().to_string();
    let mut response = next.run(request).await;
    if compatibility_lane_is_sunset_path(&path) {
        response.headers_mut().insert(
            "sunset",
            HeaderValue::from_static(COMPATIBILITY_LANE_SUNSET_DATE_HTTP),
        );
        response.headers_mut().insert(
            HEADER_OA_COMPAT_SUNSET_DATE,
            HeaderValue::from_static(COMPATIBILITY_LANE_SUNSET_DATE_ISO),
        );
        response.headers_mut().insert(
            HEADER_OA_COMPAT_MIGRATION_DOC,
            HeaderValue::from_static(COMPATIBILITY_LANE_MIGRATION_DOC),
        );
    }

    response
}

fn compatibility_lane_is_sunset_path(path: &str) -> bool {
    if path.starts_with("/api/v1/control/") {
        return true;
    }
    if path.starts_with("/api/v1/auth/")
        || path == ROUTE_V1_SYNC_TOKEN
        || path == ROUTE_V1_SPACETIME_TOKEN
    {
        return true;
    }
    if path == "/api/chat/guest-session" || path == ROUTE_LEGACY_CHAT_STREAM {
        return true;
    }
    if path == "/api/chats" || path.starts_with("/api/chats/") {
        return true;
    }

    false
}

fn compatibility_surface_for_path(path: &str) -> Option<CompatibilitySurface> {
    if path.starts_with("/api/v1/control/") {
        return Some(CompatibilitySurface::ControlApi);
    }

    if path == ROUTE_V1_SYNC_TOKEN || path == ROUTE_V1_SPACETIME_TOKEN {
        return Some(CompatibilitySurface::ControlApi);
    }

    if path == ROUTE_LEGACY_CHAT_STREAM || is_legacy_chats_stream_path(path) {
        return Some(CompatibilitySurface::ControlApi);
    }

    None
}

fn is_legacy_chats_stream_path(path: &str) -> bool {
    let Some(remainder) = path.strip_prefix("/api/chats/") else {
        return false;
    };
    let Some(conversation_id) = remainder.strip_suffix("/stream") else {
        return false;
    };

    !conversation_id.is_empty() && !conversation_id.contains('/')
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

async fn openapi_spec(
    headers: HeaderMap,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiErrorResponse>)> {
    let document = openapi_document();
    let encoded = serde_json::to_vec(&document).map_err(|_| {
        error_response_with_status(
            StatusCode::INTERNAL_SERVER_ERROR,
            ApiErrorCode::InternalError,
            "Failed to generate OpenAPI document.".to_string(),
        )
    })?;
    let etag = static_etag(&encoded);

    if if_none_match_matches(Some(&headers), &etag) {
        let mut response = Response::new(Body::empty());
        *response.status_mut() = StatusCode::NOT_MODIFIED;
        response
            .headers_mut()
            .insert(CACHE_CONTROL, HeaderValue::from_static(CACHE_MANIFEST));
        response.headers_mut().insert(
            ETAG,
            HeaderValue::from_str(&etag).map_err(|_| {
                error_response_with_status(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    ApiErrorCode::InternalError,
                    "Failed to build OpenAPI etag header.".to_string(),
                )
            })?,
        );
        return Ok(response);
    }

    let mut response = Response::new(Body::from(encoded));
    *response.status_mut() = StatusCode::OK;
    response
        .headers_mut()
        .insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    response
        .headers_mut()
        .insert(CACHE_CONTROL, HeaderValue::from_static(CACHE_MANIFEST));
    response.headers_mut().insert(
        ETAG,
        HeaderValue::from_str(&etag).map_err(|_| {
            error_response_with_status(
                StatusCode::INTERNAL_SERVER_ERROR,
                ApiErrorCode::InternalError,
                "Failed to build OpenAPI etag header.".to_string(),
            )
        })?,
    );

    Ok(response)
}

async fn smoke_stream(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response, (StatusCode, Json<ApiErrorResponse>)> {
    let expected_secret = state
        .config
        .smoke_stream_secret
        .as_deref()
        .unwrap_or_default()
        .trim()
        .to_string();
    let provided_secret = header_string(&headers, HEADER_OA_SMOKE_SECRET).unwrap_or_default();
    if expected_secret.is_empty()
        || provided_secret.is_empty()
        || !constant_time_eq(expected_secret.as_bytes(), provided_secret.as_bytes())
    {
        return Err(unauthorized_error("Unauthenticated."));
    }

    let mut response = ok_data(serde_json::json!({
        "status": "ok",
        "stream_protocol": "spacetime_ws",
        "delivery": {
            "transport": "spacetime_ws",
            "topic": "runtime.codex_worker_events",
            "scope": "runtime.codex_worker_events",
            "syncTokenRoute": ROUTE_SPACETIME_TOKEN,
            "sseEnabled": false,
        },
        "event_contract": {
            "chat": ["turn.start", "turn.finish", "turn.error", "turn.tool"],
            "worker": ["worker.event", "worker.response", "worker.stopped"],
        },
    }))
    .into_response();
    response
        .headers_mut()
        .insert(HEADER_OA_SMOKE, HeaderValue::from_static("1"));
    response
        .headers_mut()
        .insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
        .headers_mut()
        .insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

    Ok(response)
}

async fn static_manifest(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiErrorResponse>)> {
    let manifest_path = state.config.static_dir.join("manifest.json");
    let response = build_static_response(&manifest_path, CACHE_MANIFEST, Some(&headers))
        .await
        .map_err(map_static_error)?;
    Ok(response)
}

async fn static_service_worker(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiErrorResponse>)> {
    let service_worker_path = state.config.static_dir.join("sw.js");
    let response = build_static_response(&service_worker_path, CACHE_MANIFEST, Some(&headers))
        .await
        .map_err(map_static_error)?;
    Ok(response)
}

async fn static_asset(
    State(state): State<AppState>,
    Path(path): Path<String>,
    headers: HeaderMap,
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

    let response = build_static_response(&asset_path, cache_control, Some(&headers))
        .await
        .map_err(map_static_error)?;
    Ok(response)
}

async fn build_static_response(
    file_path: &FsPath,
    cache_control: &'static str,
    request_headers: Option<&HeaderMap>,
) -> Result<axum::response::Response, StaticResponseError> {
    let (served_path, content_encoding) = resolve_static_variant_path(file_path, request_headers);
    let bytes = tokio::fs::read(&served_path).await.map_err(|source| {
        if source.kind() == std::io::ErrorKind::NotFound {
            StaticResponseError::NotFound(format!(
                "Static file '{}' was not found.",
                served_path.display()
            ))
        } else {
            StaticResponseError::Io(source)
        }
    })?;
    let etag = static_etag(&bytes);

    if if_none_match_matches(request_headers, &etag) {
        let mut response = axum::response::Response::new(Body::empty());
        *response.status_mut() = StatusCode::NOT_MODIFIED;
        response
            .headers_mut()
            .insert(CACHE_CONTROL, HeaderValue::from_static(cache_control));
        response
            .headers_mut()
            .insert(VARY, HeaderValue::from_static("Accept-Encoding"));
        if let Some(content_encoding) = content_encoding {
            response
                .headers_mut()
                .insert(CONTENT_ENCODING, HeaderValue::from_static(content_encoding));
        }
        response.headers_mut().insert(
            ETAG,
            HeaderValue::from_str(&etag)
                .map_err(|_| StaticResponseError::InvalidHeader(etag.clone()))?,
        );
        apply_static_security_headers(response.headers_mut());
        return Ok(response);
    }

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
    response
        .headers_mut()
        .insert(VARY, HeaderValue::from_static("Accept-Encoding"));
    if let Some(content_encoding) = content_encoding {
        response
            .headers_mut()
            .insert(CONTENT_ENCODING, HeaderValue::from_static(content_encoding));
    }
    response.headers_mut().insert(
        ETAG,
        HeaderValue::from_str(&etag).map_err(|_| StaticResponseError::InvalidHeader(etag))?,
    );
    apply_static_security_headers(response.headers_mut());

    Ok(response)
}

fn resolve_static_variant_path(
    file_path: &FsPath,
    request_headers: Option<&HeaderMap>,
) -> (std::path::PathBuf, Option<&'static str>) {
    if accepts_static_encoding(request_headers, "br") {
        let mut compressed = file_path.as_os_str().to_os_string();
        compressed.push(".br");
        let compressed_path = std::path::PathBuf::from(compressed);
        if compressed_path.is_file() {
            return (compressed_path, Some("br"));
        }
    }

    if accepts_static_encoding(request_headers, "gzip") {
        let mut compressed = file_path.as_os_str().to_os_string();
        compressed.push(".gz");
        let compressed_path = std::path::PathBuf::from(compressed);
        if compressed_path.is_file() {
            return (compressed_path, Some("gzip"));
        }
    }

    (file_path.to_path_buf(), None)
}

fn accepts_static_encoding(request_headers: Option<&HeaderMap>, encoding: &str) -> bool {
    let Some(header_value) = request_headers
        .and_then(|headers| headers.get(ACCEPT_ENCODING))
        .and_then(|value| value.to_str().ok())
    else {
        return false;
    };

    for token in header_value.split(',') {
        let mut parts = token.trim().split(';');
        let Some(name) = parts.next().map(|value| value.trim().to_ascii_lowercase()) else {
            continue;
        };
        if name != encoding && name != "*" {
            continue;
        }

        let mut quality = 1.0f32;
        for part in parts {
            let part = part.trim();
            if let Some(value) = part.strip_prefix("q=") {
                quality = value.trim().parse::<f32>().unwrap_or(0.0);
            }
        }

        if quality > 0.0 {
            return true;
        }
    }

    false
}

fn static_etag(bytes: &[u8]) -> String {
    format!("\"{}\"", sha256_hex(bytes))
}

fn if_none_match_matches(request_headers: Option<&HeaderMap>, etag: &str) -> bool {
    let Some(header_value) = request_headers
        .and_then(|headers| headers.get(IF_NONE_MATCH))
        .and_then(|value| value.to_str().ok())
    else {
        return false;
    };

    let expected = normalize_etag_token(etag);
    for candidate in header_value.split(',') {
        let normalized = normalize_etag_token(candidate);
        if normalized == "*" || normalized == expected {
            return true;
        }
    }

    false
}

fn normalize_etag_token(value: &str) -> String {
    let trimmed = value.trim();
    let without_weak = trimmed.strip_prefix("W/").unwrap_or(trimmed).trim();
    without_weak.to_string()
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

async fn web_chat_new_thread(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let htmx = classify_htmx_request(&headers);
    let htmx_fragment_enabled =
        state.route_split.htmx_mode_for_path("/chat").await.mode == HtmxModeTarget::Fragment;
    let is_htmx = htmx.is_hx_request && htmx_fragment_enabled;
    let session = match session_bundle_from_headers(&state, &headers).await {
        Ok(session) => session,
        Err(_) => {
            if is_htmx {
                return htmx_redirect_response("/login");
            }
            return Redirect::temporary("/login").into_response();
        }
    };

    let thread_id = format!("thread_{}", Uuid::new_v4().simple());
    match state
        .codex_thread_store
        .create_thread_for_user(&session.user.id, &session.session.active_org_id, &thread_id)
        .await
    {
        Ok(thread) => {
            let location = format!("/chat/{}?status=thread-created", thread.thread_id);
            if is_htmx {
                match chat_views_for_bundle(&state, &session, Some(thread.thread_id.clone())).await
                {
                    Ok(views) => {
                        let session_view = session_view_from_bundle(&session);
                        chat_thread_select_fragment_response(
                            Some(&session_view),
                            Some("thread-created"),
                            &views.threads,
                            views.active_thread_id.as_deref(),
                            &views.messages,
                            Some(&format!("/chat/{}", thread.thread_id)),
                        )
                    }
                    Err(_) => htmx_notice_response(
                        "chat-status",
                        "thread-create-failed",
                        true,
                        StatusCode::UNPROCESSABLE_ENTITY,
                    ),
                }
            } else {
                Redirect::temporary(&location).into_response()
            }
        }
        Err(_) => {
            if is_htmx {
                htmx_notice_response(
                    "chat-status",
                    "thread-create-failed",
                    true,
                    StatusCode::UNPROCESSABLE_ENTITY,
                )
            } else {
                Redirect::temporary("/chat?status=thread-create-failed").into_response()
            }
        }
    }
}

async fn web_chat_thread_fragment(
    State(state): State<AppState>,
    Path(thread_id): Path<String>,
    headers: HeaderMap,
) -> Response {
    let htmx = classify_htmx_request(&headers);
    let htmx_fragment_enabled =
        state.route_split.htmx_mode_for_path("/chat").await.mode == HtmxModeTarget::Fragment;
    let is_htmx = htmx.is_hx_request && htmx_fragment_enabled;
    let normalized_thread_id = thread_id.trim().to_string();
    if !is_htmx {
        let location = if normalized_thread_id.is_empty() {
            "/chat".to_string()
        } else {
            format!("/chat/{normalized_thread_id}")
        };
        return Redirect::temporary(&location).into_response();
    }
    let session_bundle = match session_bundle_from_headers(&state, &headers).await {
        Ok(session) => session,
        Err(_) => return htmx_redirect_response("/login"),
    };
    if normalized_thread_id.is_empty() {
        return htmx_notice_response(
            "chat-status",
            "message-send-failed",
            true,
            StatusCode::UNPROCESSABLE_ENTITY,
        );
    }

    let views =
        match chat_views_for_bundle(&state, &session_bundle, Some(normalized_thread_id)).await {
            Ok(views) => views,
            Err(_) => {
                return htmx_notice_response(
                    "chat-status",
                    "message-send-failed",
                    true,
                    StatusCode::UNPROCESSABLE_ENTITY,
                );
            }
        };
    let session_view = session_view_from_bundle(&session_bundle);
    let push_url = views
        .active_thread_id
        .as_deref()
        .map(|thread_id| format!("/chat/{thread_id}"));
    chat_thread_select_fragment_response(
        Some(&session_view),
        None,
        &views.threads,
        views.active_thread_id.as_deref(),
        &views.messages,
        push_url.as_deref(),
    )
}

async fn web_chat_send_message(
    State(state): State<AppState>,
    Path(thread_id): Path<String>,
    headers: HeaderMap,
    Form(payload): Form<WebChatSendForm>,
) -> Response {
    let htmx = classify_htmx_request(&headers);
    let htmx_fragment_enabled =
        state.route_split.htmx_mode_for_path("/chat").await.mode == HtmxModeTarget::Fragment;
    let is_htmx = htmx.is_hx_request && htmx_fragment_enabled;
    let session = match session_bundle_from_headers(&state, &headers).await {
        Ok(session) => session,
        Err(_) => {
            if is_htmx {
                return htmx_redirect_response("/login");
            }
            return Redirect::temporary("/login").into_response();
        }
    };

    let normalized_thread_id = thread_id.trim().to_string();
    if normalized_thread_id.is_empty() {
        if is_htmx {
            return htmx_notice_response(
                "chat-status",
                "message-send-failed",
                true,
                StatusCode::UNPROCESSABLE_ENTITY,
            );
        }
        return Redirect::temporary("/chat?status=message-send-failed").into_response();
    }

    let text = payload.text.trim().to_string();
    if text.is_empty() {
        if is_htmx {
            return htmx_notice_response(
                "chat-status",
                "empty-body",
                true,
                StatusCode::UNPROCESSABLE_ENTITY,
            );
        }
        return Redirect::temporary(&format!("/chat/{normalized_thread_id}?status=empty-body"))
            .into_response();
    }
    if text.chars().count() > 20_000 {
        if is_htmx {
            return htmx_notice_response(
                "chat-status",
                "message-send-failed",
                true,
                StatusCode::UNPROCESSABLE_ENTITY,
            );
        }
        return Redirect::temporary(&format!(
            "/chat/{normalized_thread_id}?status=message-send-failed"
        ))
        .into_response();
    }

    match state
        .codex_thread_store
        .append_user_message(
            &session.user.id,
            &session.session.active_org_id,
            &normalized_thread_id,
            text,
        )
        .await
    {
        Ok(_) => {
            let location = format!("/chat/{normalized_thread_id}?status=message-sent");
            if is_htmx {
                let mut response =
                    htmx_notice_response("chat-status", "message-sent", false, StatusCode::OK);
                htmx_set_trigger_header(&mut response, "chat-message-sent");
                response
            } else {
                Redirect::temporary(&location).into_response()
            }
        }
        Err(_) => {
            if is_htmx {
                htmx_notice_response(
                    "chat-status",
                    "message-send-failed",
                    true,
                    StatusCode::UNPROCESSABLE_ENTITY,
                )
            } else {
                Redirect::temporary(&format!(
                    "/chat/{normalized_thread_id}?status=message-send-failed"
                ))
                .into_response()
            }
        }
    }
}

async fn web_feed_shout(
    State(state): State<AppState>,
    headers: HeaderMap,
    Form(payload): Form<WebShoutForm>,
) -> Response {
    let htmx = classify_htmx_request(&headers);
    let htmx_fragment_enabled =
        state.route_split.htmx_mode_for_path("/feed").await.mode == HtmxModeTarget::Fragment;
    let is_htmx = htmx.is_hx_request && htmx_fragment_enabled;
    let session = match session_bundle_from_headers(&state, &headers).await {
        Ok(session) => session,
        Err(_) => {
            if is_htmx {
                return htmx_redirect_response("/login");
            }
            return Redirect::temporary("/login").into_response();
        }
    };

    let body = payload.body.trim().to_string();
    if body.is_empty() {
        if is_htmx {
            return htmx_notice_response(
                "feed-status",
                "empty-body",
                true,
                StatusCode::UNPROCESSABLE_ENTITY,
            );
        }
        return Redirect::temporary("/feed?status=empty-body").into_response();
    }
    if body.chars().count() > 2000 {
        if is_htmx {
            return htmx_notice_response(
                "feed-status",
                "shout-post-failed",
                true,
                StatusCode::UNPROCESSABLE_ENTITY,
            );
        }
        return Redirect::temporary("/feed?status=shout-post-failed").into_response();
    }

    let zone = match normalize_shout_zone(payload.zone.as_deref(), "zone") {
        Ok(zone) => zone,
        Err(_) => {
            if is_htmx {
                return htmx_notice_response(
                    "feed-status",
                    "invalid-zone",
                    true,
                    StatusCode::UNPROCESSABLE_ENTITY,
                );
            }
            return Redirect::temporary("/feed?status=invalid-zone").into_response();
        }
    };

    let result = state
        ._domain_store
        .create_shout(CreateShoutInput {
            user_id: session.user.id,
            zone: zone.clone(),
            body,
        })
        .await;

    if result.is_err() {
        if is_htmx {
            return htmx_notice_response(
                "feed-status",
                "shout-post-failed",
                true,
                StatusCode::UNPROCESSABLE_ENTITY,
            );
        }
        return Redirect::temporary("/feed?status=shout-post-failed").into_response();
    }

    if is_htmx {
        let mut response =
            htmx_notice_response("feed-status", "shout-posted", false, StatusCode::OK);
        htmx_set_trigger_header(&mut response, "feed-shout-posted");
        return response;
    }

    if let Some(zone) = zone {
        let location = format!("/feed?zone={zone}&status=shout-posted");
        Redirect::temporary(&location).into_response()
    } else {
        Redirect::temporary("/feed?status=shout-posted").into_response()
    }
}

async fn web_settings_profile_update(
    State(state): State<AppState>,
    headers: HeaderMap,
    Form(payload): Form<UpdateProfileRequestPayload>,
) -> Response {
    let htmx = classify_htmx_request(&headers);
    let request_id = request_id(&headers);
    let bundle = match session_bundle_from_headers(&state, &headers).await {
        Ok(bundle) => bundle,
        Err(_) => {
            if htmx.is_hx_request {
                return htmx_redirect_response("/login");
            }
            return Redirect::temporary("/login").into_response();
        }
    };

    let normalized_name = payload.name.trim().to_string();
    if normalized_name.is_empty() || normalized_name.chars().count() > 255 {
        if htmx.is_hx_request {
            return htmx_notice_response(
                "settings-status",
                "profile-update-failed",
                true,
                StatusCode::UNPROCESSABLE_ENTITY,
            );
        }
        return Redirect::temporary("/settings/profile?status=profile-update-failed")
            .into_response();
    }

    let updated_user = match state
        .auth
        .update_profile_name(&bundle.user.id, normalized_name)
        .await
    {
        Ok(user) => user,
        Err(_) => {
            if htmx.is_hx_request {
                return htmx_notice_response(
                    "settings-status",
                    "profile-update-failed",
                    true,
                    StatusCode::UNPROCESSABLE_ENTITY,
                );
            }
            return Redirect::temporary("/settings/profile?status=profile-update-failed")
                .into_response();
        }
    };

    state.observability.audit(
        AuditEvent::new("profile.updated", request_id.clone())
            .with_user_id(updated_user.id.clone())
            .with_session_id(bundle.session.session_id.clone())
            .with_org_id(bundle.session.active_org_id.clone())
            .with_device_id(bundle.session.device_id.clone())
            .with_attribute("field_updated", "name".to_string())
            .with_attribute("source", "web".to_string()),
    );
    state
        .observability
        .increment_counter("profile.updated", &request_id);

    if htmx.is_hx_request {
        return htmx_notice_response("settings-status", "profile-updated", false, StatusCode::OK);
    }

    Redirect::temporary("/settings/profile?status=profile-updated").into_response()
}

async fn web_settings_profile_delete(
    State(state): State<AppState>,
    headers: HeaderMap,
    Form(payload): Form<DeleteProfileRequestPayload>,
) -> Response {
    let htmx = classify_htmx_request(&headers);
    let request_id = request_id(&headers);
    let bundle = match session_bundle_from_headers(&state, &headers).await {
        Ok(bundle) => bundle,
        Err(_) => {
            if htmx.is_hx_request {
                return htmx_redirect_response("/login");
            }
            return Redirect::temporary("/login").into_response();
        }
    };

    let normalized_email = payload.email.trim().to_lowercase();
    let user_email = bundle.user.email.trim().to_lowercase();
    if normalized_email.is_empty() || normalized_email != user_email {
        if htmx.is_hx_request {
            return htmx_notice_response(
                "settings-status",
                "profile-delete-failed",
                true,
                StatusCode::UNPROCESSABLE_ENTITY,
            );
        }
        return Redirect::temporary("/settings/profile?status=profile-delete-failed")
            .into_response();
    }

    let deleted_user = match state.auth.delete_profile(&bundle.user.id).await {
        Ok(user) => user,
        Err(_) => {
            if htmx.is_hx_request {
                return htmx_notice_response(
                    "settings-status",
                    "profile-delete-failed",
                    true,
                    StatusCode::UNPROCESSABLE_ENTITY,
                );
            }
            return Redirect::temporary("/settings/profile?status=profile-delete-failed")
                .into_response();
        }
    };

    state.observability.audit(
        AuditEvent::new("profile.deleted", request_id.clone())
            .with_user_id(deleted_user.id)
            .with_session_id(bundle.session.session_id)
            .with_org_id(bundle.session.active_org_id)
            .with_device_id(bundle.session.device_id)
            .with_attribute("source", "web".to_string()),
    );
    state
        .observability
        .increment_counter("profile.deleted", &request_id);

    let mut response = if htmx.is_hx_request {
        htmx_redirect_response("/login?status=profile-deleted")
    } else {
        Redirect::temporary("/login?status=profile-deleted").into_response()
    };
    let _ = append_set_cookie_header(&mut response, &clear_cookie(AUTH_ACCESS_COOKIE_NAME));
    let _ = append_set_cookie_header(&mut response, &clear_cookie(AUTH_REFRESH_COOKIE_NAME));
    let _ = append_set_cookie_header(&mut response, &clear_cookie(LOCAL_TEST_AUTH_COOKIE_NAME));
    response
}

async fn web_settings_resend_upsert(
    State(state): State<AppState>,
    headers: HeaderMap,
    Form(payload): Form<UpsertResendIntegrationRequestPayload>,
) -> Response {
    let htmx = classify_htmx_request(&headers);
    let request_id = request_id(&headers);
    let bundle = match session_bundle_from_headers(&state, &headers).await {
        Ok(bundle) => bundle,
        Err(_) => {
            if htmx.is_hx_request {
                return htmx_redirect_response("/login");
            }
            return Redirect::temporary("/login").into_response();
        }
    };

    let api_key = payload
        .resend_api_key
        .and_then(non_empty)
        .filter(|value| value.chars().count() >= 8 && value.chars().count() <= 4096);
    let Some(api_key) = api_key else {
        if htmx.is_hx_request {
            return htmx_notice_response(
                "settings-status",
                "settings-action-failed",
                true,
                StatusCode::UNPROCESSABLE_ENTITY,
            );
        }
        return Redirect::temporary("/settings/profile?status=settings-action-failed")
            .into_response();
    };
    let sender_email = match normalize_optional_email(payload.sender_email, "sender_email") {
        Ok(value) => value,
        Err(_) => {
            if htmx.is_hx_request {
                return htmx_notice_response(
                    "settings-status",
                    "settings-action-failed",
                    true,
                    StatusCode::UNPROCESSABLE_ENTITY,
                );
            }
            return Redirect::temporary("/settings/profile?status=settings-action-failed")
                .into_response();
        }
    };
    let sender_name =
        match normalize_optional_bounded_trimmed_string(payload.sender_name, "sender_name", 255) {
            Ok(value) => value,
            Err(_) => {
                if htmx.is_hx_request {
                    return htmx_notice_response(
                        "settings-status",
                        "settings-action-failed",
                        true,
                        StatusCode::UNPROCESSABLE_ENTITY,
                    );
                }
                return Redirect::temporary("/settings/profile?status=settings-action-failed")
                    .into_response();
            }
        };

    let result = match state
        ._domain_store
        .upsert_resend_integration(UpsertResendIntegrationInput {
            user_id: bundle.user.id.clone(),
            api_key,
            sender_email,
            sender_name,
        })
        .await
    {
        Ok(result) => result,
        Err(_) => {
            if htmx.is_hx_request {
                return htmx_notice_response(
                    "settings-status",
                    "settings-action-failed",
                    true,
                    StatusCode::UNPROCESSABLE_ENTITY,
                );
            }
            return Redirect::temporary("/settings/profile?status=settings-action-failed")
                .into_response();
        }
    };
    let status = match result.action.as_str() {
        "secret_created" => "resend-connected",
        "secret_rotated" => "resend-rotated",
        _ => "resend-updated",
    };

    state.observability.audit(
        AuditEvent::new("settings.integrations.resend.upserted", request_id.clone())
            .with_user_id(bundle.user.id)
            .with_session_id(bundle.session.session_id)
            .with_org_id(bundle.session.active_org_id)
            .with_device_id(bundle.session.device_id)
            .with_attribute("action", result.action)
            .with_attribute("status", status.to_string()),
    );
    state
        .observability
        .increment_counter("settings.integrations.resend.upserted", &request_id);

    if htmx.is_hx_request {
        return htmx_notice_response("settings-status", status, false, StatusCode::OK);
    }

    Redirect::temporary(&format!("/settings/profile?status={status}")).into_response()
}

async fn web_settings_resend_test(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let htmx = classify_htmx_request(&headers);
    let request_id = request_id(&headers);
    let bundle = match session_bundle_from_headers(&state, &headers).await {
        Ok(bundle) => bundle,
        Err(_) => {
            if htmx.is_hx_request {
                return htmx_redirect_response("/login");
            }
            return Redirect::temporary("/login").into_response();
        }
    };

    let integration = match state
        ._domain_store
        .find_active_integration_secret(&bundle.user.id, "resend")
        .await
    {
        Ok(Some(integration)) => integration,
        _ => {
            if htmx.is_hx_request {
                return htmx_notice_response(
                    "settings-status",
                    "settings-action-failed",
                    true,
                    StatusCode::UNPROCESSABLE_ENTITY,
                );
            }
            return Redirect::temporary("/settings/profile?status=settings-action-failed")
                .into_response();
        }
    };

    if state
        ._domain_store
        .audit_integration_test_request(&bundle.user.id, "resend")
        .await
        .is_err()
    {
        if htmx.is_hx_request {
            return htmx_notice_response(
                "settings-status",
                "settings-action-failed",
                true,
                StatusCode::UNPROCESSABLE_ENTITY,
            );
        }
        return Redirect::temporary("/settings/profile?status=settings-action-failed")
            .into_response();
    }

    state.observability.audit(
        AuditEvent::new(
            "settings.integrations.resend.test_requested",
            request_id.clone(),
        )
        .with_user_id(bundle.user.id)
        .with_session_id(bundle.session.session_id)
        .with_org_id(bundle.session.active_org_id)
        .with_device_id(bundle.session.device_id)
        .with_attribute("integration_id", integration.id.to_string()),
    );
    state
        .observability
        .increment_counter("settings.integrations.resend.test_requested", &request_id);

    if htmx.is_hx_request {
        return htmx_notice_response(
            "settings-status",
            "resend-test-queued",
            false,
            StatusCode::OK,
        );
    }

    Redirect::temporary("/settings/profile?status=resend-test-queued").into_response()
}

async fn web_settings_resend_disconnect(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Response {
    let htmx = classify_htmx_request(&headers);
    let request_id = request_id(&headers);
    let bundle = match session_bundle_from_headers(&state, &headers).await {
        Ok(bundle) => bundle,
        Err(_) => {
            if htmx.is_hx_request {
                return htmx_redirect_response("/login");
            }
            return Redirect::temporary("/login").into_response();
        }
    };

    let revoked = match state
        ._domain_store
        .revoke_integration(&bundle.user.id, "resend")
        .await
    {
        Ok(revoked) => revoked,
        Err(_) => {
            if htmx.is_hx_request {
                return htmx_notice_response(
                    "settings-status",
                    "settings-action-failed",
                    true,
                    StatusCode::UNPROCESSABLE_ENTITY,
                );
            }
            return Redirect::temporary("/settings/profile?status=settings-action-failed")
                .into_response();
        }
    };

    state.observability.audit(
        AuditEvent::new(
            "settings.integrations.resend.disconnected",
            request_id.clone(),
        )
        .with_user_id(bundle.user.id)
        .with_session_id(bundle.session.session_id)
        .with_org_id(bundle.session.active_org_id)
        .with_device_id(bundle.session.device_id)
        .with_attribute("had_integration", revoked.is_some().to_string()),
    );
    state
        .observability
        .increment_counter("settings.integrations.resend.disconnected", &request_id);

    if htmx.is_hx_request {
        return htmx_notice_response(
            "settings-status",
            "resend-disconnected",
            false,
            StatusCode::OK,
        );
    }

    Redirect::temporary("/settings/profile?status=resend-disconnected").into_response()
}

async fn web_settings_google_connect(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Response {
    let htmx = classify_htmx_request(&headers);
    match settings_integrations_google_redirect(State(state), headers.clone()).await {
        Ok(response) => response,
        Err(_) => {
            if htmx.is_hx_request {
                htmx_notice_response(
                    "settings-status",
                    "settings-action-failed",
                    true,
                    StatusCode::UNPROCESSABLE_ENTITY,
                )
            } else {
                Redirect::temporary("/settings/profile?status=settings-action-failed")
                    .into_response()
            }
        }
    }
}

async fn web_settings_google_disconnect(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Response {
    let htmx = classify_htmx_request(&headers);
    let request_id = request_id(&headers);
    let bundle = match session_bundle_from_headers(&state, &headers).await {
        Ok(bundle) => bundle,
        Err(_) => {
            if htmx.is_hx_request {
                return htmx_redirect_response("/login");
            }
            return Redirect::temporary("/login").into_response();
        }
    };

    let revoked = match state
        ._domain_store
        .revoke_integration(&bundle.user.id, "google")
        .await
    {
        Ok(revoked) => revoked,
        Err(_) => {
            if htmx.is_hx_request {
                return htmx_notice_response(
                    "settings-status",
                    "settings-action-failed",
                    true,
                    StatusCode::UNPROCESSABLE_ENTITY,
                );
            }
            return Redirect::temporary("/settings/profile?status=settings-action-failed")
                .into_response();
        }
    };

    state.observability.audit(
        AuditEvent::new(
            "settings.integrations.google.disconnected",
            request_id.clone(),
        )
        .with_user_id(bundle.user.id)
        .with_session_id(bundle.session.session_id)
        .with_org_id(bundle.session.active_org_id)
        .with_device_id(bundle.session.device_id)
        .with_attribute("had_integration", revoked.is_some().to_string()),
    );
    state
        .observability
        .increment_counter("settings.integrations.google.disconnected", &request_id);

    if htmx.is_hx_request {
        return htmx_notice_response(
            "settings-status",
            "google-disconnected",
            false,
            StatusCode::OK,
        );
    }

    Redirect::temporary("/settings/profile?status=google-disconnected").into_response()
}

fn web_l402_status_response(
    is_hx_request: bool,
    status: &str,
    is_error: bool,
    status_code: StatusCode,
) -> Response {
    if is_hx_request {
        return htmx_notice_response("billing-status", status, is_error, status_code);
    }
    Redirect::temporary(&format!("/l402?status={status}")).into_response()
}

async fn web_l402_paywall_create(
    State(state): State<AppState>,
    headers: HeaderMap,
    Form(payload): Form<WebL402PaywallCreateForm>,
) -> Response {
    let htmx = classify_htmx_request(&headers);
    let bundle = match session_bundle_from_headers(&state, &headers).await {
        Ok(bundle) => bundle,
        Err(_) => {
            if htmx.is_hx_request {
                return htmx_redirect_response("/login");
            }
            return Redirect::temporary("/login").into_response();
        }
    };
    let _ = bundle;

    let price_msats = payload
        .price_msats
        .trim()
        .parse::<u64>()
        .ok()
        .filter(|value| *value > 0);
    let Some(price_msats) = price_msats else {
        return web_l402_status_response(
            htmx.is_hx_request,
            "l402-action-failed",
            true,
            StatusCode::UNPROCESSABLE_ENTITY,
        );
    };

    let api_payload = L402PaywallCreateRequestPayload {
        name: payload.name,
        host_regexp: payload.host_regexp,
        path_regexp: payload.path_regexp,
        price_msats,
        upstream: payload.upstream,
        enabled: Some(payload.enabled.is_some()),
        meta: None,
    };

    match l402_paywall_create(State(state), headers.clone(), Json(api_payload)).await {
        Ok(_) => web_l402_status_response(
            htmx.is_hx_request,
            "l402-paywall-created",
            false,
            StatusCode::OK,
        ),
        Err(_) => web_l402_status_response(
            htmx.is_hx_request,
            "l402-action-failed",
            true,
            StatusCode::UNPROCESSABLE_ENTITY,
        ),
    }
}

async fn web_l402_paywall_toggle(
    State(state): State<AppState>,
    Path(paywall_id): Path<String>,
    headers: HeaderMap,
) -> Response {
    let htmx = classify_htmx_request(&headers);
    let bundle = match session_bundle_from_headers(&state, &headers).await {
        Ok(bundle) => bundle,
        Err(_) => {
            if htmx.is_hx_request {
                return htmx_redirect_response("/login");
            }
            return Redirect::temporary("/login").into_response();
        }
    };

    let existing = match state
        ._domain_store
        .list_l402_paywalls_for_owner(&bundle.user.id, false)
        .await
    {
        Ok(rows) => rows.into_iter().find(|row| row.id == paywall_id),
        Err(_) => None,
    };
    let Some(existing) = existing else {
        return web_l402_status_response(
            htmx.is_hx_request,
            "l402-action-failed",
            true,
            StatusCode::UNPROCESSABLE_ENTITY,
        );
    };

    let update_payload = L402PaywallUpdateRequestPayload {
        enabled: Some(!existing.enabled),
        ..L402PaywallUpdateRequestPayload::default()
    };

    match l402_paywall_update(
        State(state),
        Path(existing.id),
        headers.clone(),
        Json(update_payload),
    )
    .await
    {
        Ok(_) => web_l402_status_response(
            htmx.is_hx_request,
            "l402-paywall-updated",
            false,
            StatusCode::OK,
        ),
        Err(_) => web_l402_status_response(
            htmx.is_hx_request,
            "l402-action-failed",
            true,
            StatusCode::UNPROCESSABLE_ENTITY,
        ),
    }
}

async fn web_l402_paywall_delete(
    State(state): State<AppState>,
    Path(paywall_id): Path<String>,
    headers: HeaderMap,
) -> Response {
    let htmx = classify_htmx_request(&headers);
    let bundle = match session_bundle_from_headers(&state, &headers).await {
        Ok(bundle) => bundle,
        Err(_) => {
            if htmx.is_hx_request {
                return htmx_redirect_response("/login");
            }
            return Redirect::temporary("/login").into_response();
        }
    };
    let _ = bundle;

    match l402_paywall_delete(State(state), Path(paywall_id), headers.clone()).await {
        Ok(_) => web_l402_status_response(
            htmx.is_hx_request,
            "l402-paywall-deleted",
            false,
            StatusCode::OK,
        ),
        Err(_) => web_l402_status_response(
            htmx.is_hx_request,
            "l402-action-failed",
            true,
            StatusCode::UNPROCESSABLE_ENTITY,
        ),
    }
}

fn web_admin_status_response(
    is_hx_request: bool,
    status: &str,
    is_error: bool,
    status_code: StatusCode,
) -> Response {
    if is_hx_request {
        return htmx_notice_response("admin-status", status, is_error, status_code);
    }
    Redirect::temporary(&format!("/admin?status={status}")).into_response()
}

fn html_escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn web_admin_result_fragment_response(
    is_hx_request: bool,
    status: &str,
    is_error: bool,
    status_code: StatusCode,
    payload: serde_json::Value,
) -> Response {
    if is_hx_request {
        let pretty = serde_json::to_string_pretty(&payload).unwrap_or_else(|_| "{}".to_string());
        let status_text = match status {
            "admin-action-completed" => "Admin action completed.",
            "admin-action-failed" => "Admin action failed.",
            "admin-forbidden" => "Admin role required.",
            _ => "Action completed.",
        };
        let status_class = if is_error {
            "oa-notice error"
        } else {
            "oa-notice"
        };
        let fragment = format!(
            "<article id=\"admin-result\" class=\"oa-card\"><h3>Result</h3><pre class=\"oa-json\">{}</pre></article><div id=\"admin-status\" class=\"{}\" hx-swap-oob=\"outerHTML\">{}</div>",
            html_escape(&pretty),
            status_class,
            html_escape(status_text)
        );
        let mut response = crate::web_htmx::fragment_response(fragment, status_code);
        apply_html_security_headers(response.headers_mut());
        return response;
    }

    Redirect::temporary(&format!("/admin?status={status}")).into_response()
}

async fn web_admin_bundle(
    state: &AppState,
    headers: &HeaderMap,
    is_hx_request: bool,
) -> Result<SessionBundle, Response> {
    let bundle = session_bundle_from_headers(state, headers)
        .await
        .map_err(|_| {
            if is_hx_request {
                htmx_redirect_response("/login")
            } else {
                Redirect::temporary("/login").into_response()
            }
        })?;
    if !is_admin_email(&bundle.user.email, &state.config.admin_emails) {
        return Err(web_admin_status_response(
            is_hx_request,
            "admin-forbidden",
            true,
            StatusCode::FORBIDDEN,
        ));
    }
    Ok(bundle)
}

fn parse_web_lightning_ops_args(
    args_json: Option<String>,
) -> Result<serde_json::Map<String, serde_json::Value>, serde_json::Value> {
    let raw = args_json
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("{}");
    let value = serde_json::from_str::<serde_json::Value>(raw).map_err(|_| {
        serde_json::json!({
            "ok": false,
            "error": {
                "code": "invalid_args_json",
                "message": "args_json must be a valid JSON object",
            }
        })
    })?;
    let object = value.as_object().cloned().ok_or_else(|| {
        serde_json::json!({
            "ok": false,
            "error": {
                "code": "invalid_args_json",
                "message": "args_json must decode to a JSON object",
            }
        })
    })?;
    Ok(object)
}

async fn web_admin_route_split_evaluate(
    State(state): State<AppState>,
    headers: HeaderMap,
    Form(payload): Form<WebRouteSplitEvaluateForm>,
) -> Response {
    let htmx = classify_htmx_request(&headers);
    if let Err(response) = web_admin_bundle(&state, &headers, htmx.is_hx_request).await {
        return response;
    }

    let path = payload.path.trim().to_string();
    if path.is_empty() {
        return web_admin_result_fragment_response(
            htmx.is_hx_request,
            "admin-action-failed",
            true,
            StatusCode::UNPROCESSABLE_ENTITY,
            serde_json::json!({
                "ok": false,
                "error": {
                    "code": "invalid_path",
                    "message": "Path is required.",
                }
            }),
        );
    }

    let cohort_key = payload
        .cohort_key
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| resolve_route_cohort_key(&headers));
    let decision = state.route_split.evaluate(&path, &cohort_key).await;
    web_admin_result_fragment_response(
        htmx.is_hx_request,
        "admin-action-completed",
        false,
        StatusCode::OK,
        serde_json::json!({
            "ok": true,
            "action": "route_split.evaluate",
            "decision": decision,
        }),
    )
}

async fn web_admin_route_split_override(
    State(state): State<AppState>,
    headers: HeaderMap,
    Form(payload): Form<WebRouteSplitOverrideForm>,
) -> Response {
    let htmx = classify_htmx_request(&headers);
    let bundle = match web_admin_bundle(&state, &headers, htmx.is_hx_request).await {
        Ok(bundle) => bundle,
        Err(response) => return response,
    };
    let request_id = request_id(&headers);

    let normalized_target = payload.target.trim().to_ascii_lowercase();
    let normalized_domain = payload
        .domain
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_lowercase());
    let mut override_kind = "route_target";
    let mut htmx_mode: Option<&'static str> = None;

    let apply = match normalized_target.as_str() {
        "legacy" => {
            if let Some(domain) = normalized_domain.as_deref() {
                state
                    .route_split
                    .set_domain_override_target(domain, Some(RouteTarget::Legacy))
                    .await
            } else {
                state
                    .route_split
                    .set_override_target(Some(RouteTarget::Legacy))
                    .await;
                Ok(())
            }
        }
        "rust" | "rust_shell" => {
            if let Some(domain) = normalized_domain.as_deref() {
                state
                    .route_split
                    .set_domain_override_target(domain, Some(RouteTarget::RustShell))
                    .await
            } else {
                state
                    .route_split
                    .set_override_target(Some(RouteTarget::RustShell))
                    .await;
                Ok(())
            }
        }
        "clear" | "default" => {
            if let Some(domain) = normalized_domain.as_deref() {
                state
                    .route_split
                    .set_domain_override_target(domain, None)
                    .await
            } else {
                state.route_split.set_override_target(None).await;
                Ok(())
            }
        }
        "htmx_fragment" | "htmx_on" => {
            let Some(domain) = normalized_domain.as_deref() else {
                return web_admin_result_fragment_response(
                    htmx.is_hx_request,
                    "admin-action-failed",
                    true,
                    StatusCode::UNPROCESSABLE_ENTITY,
                    serde_json::json!({
                        "ok": false,
                        "error": {
                            "code": "missing_domain",
                            "message": "Domain is required for HTMX overrides.",
                        }
                    }),
                );
            };
            override_kind = "htmx_mode";
            htmx_mode = Some("fragment");
            state
                .route_split
                .set_domain_htmx_mode(domain, Some(HtmxModeTarget::Fragment))
                .await
        }
        "htmx_full_page" | "htmx_off" => {
            let Some(domain) = normalized_domain.as_deref() else {
                return web_admin_result_fragment_response(
                    htmx.is_hx_request,
                    "admin-action-failed",
                    true,
                    StatusCode::UNPROCESSABLE_ENTITY,
                    serde_json::json!({
                        "ok": false,
                        "error": {
                            "code": "missing_domain",
                            "message": "Domain is required for HTMX overrides.",
                        }
                    }),
                );
            };
            override_kind = "htmx_mode";
            htmx_mode = Some("full_page");
            state
                .route_split
                .set_domain_htmx_mode(domain, Some(HtmxModeTarget::FullPage))
                .await
        }
        "htmx_rollback" => {
            let Some(domain) = normalized_domain.as_deref() else {
                return web_admin_result_fragment_response(
                    htmx.is_hx_request,
                    "admin-action-failed",
                    true,
                    StatusCode::UNPROCESSABLE_ENTITY,
                    serde_json::json!({
                        "ok": false,
                        "error": {
                            "code": "missing_domain",
                            "message": "Domain is required for HTMX overrides.",
                        }
                    }),
                );
            };
            let Some(rollback_mode) = state.route_split.htmx_rollback_mode_for_domain(Some(domain))
            else {
                return web_admin_result_fragment_response(
                    htmx.is_hx_request,
                    "admin-action-failed",
                    true,
                    StatusCode::UNPROCESSABLE_ENTITY,
                    serde_json::json!({
                        "ok": false,
                        "error": {
                            "code": "invalid_domain",
                            "message": "Unknown route domain.",
                        }
                    }),
                );
            };
            override_kind = "htmx_mode";
            htmx_mode = Some(match rollback_mode {
                HtmxModeTarget::Fragment => "fragment",
                HtmxModeTarget::FullPage => "full_page",
            });
            state
                .route_split
                .set_domain_htmx_mode(domain, Some(rollback_mode))
                .await
        }
        "htmx_clear" => {
            let Some(domain) = normalized_domain.as_deref() else {
                return web_admin_result_fragment_response(
                    htmx.is_hx_request,
                    "admin-action-failed",
                    true,
                    StatusCode::UNPROCESSABLE_ENTITY,
                    serde_json::json!({
                        "ok": false,
                        "error": {
                            "code": "missing_domain",
                            "message": "Domain is required for HTMX overrides.",
                        }
                    }),
                );
            };
            override_kind = "htmx_mode";
            htmx_mode = None;
            state.route_split.set_domain_htmx_mode(domain, None).await
        }
        "rollback" => {
            if let Some(domain) = normalized_domain.as_deref() {
                if let Some(rollback_target) =
                    state.route_split.rollback_target_for_domain(Some(domain))
                {
                    state
                        .route_split
                        .set_domain_override_target(domain, Some(rollback_target))
                        .await
                } else {
                    Err("Unknown route domain.".to_string())
                }
            } else {
                if let Some(global_target) = state.route_split.rollback_target_for_domain(None) {
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
                Ok(())
            }
        }
        _ => Err("Target must be one of: legacy, rust, rollback, clear, htmx_fragment, htmx_full_page, htmx_rollback, htmx_clear.".to_string()),
    };

    if let Err(message) = apply {
        return web_admin_result_fragment_response(
            htmx.is_hx_request,
            "admin-action-failed",
            true,
            StatusCode::UNPROCESSABLE_ENTITY,
            serde_json::json!({
                "ok": false,
                "error": {
                    "code": "invalid_target",
                    "message": message,
                }
            }),
        );
    }

    let status = state.route_split.status().await;
    let scope = normalized_domain
        .clone()
        .map(|domain| format!("domain:{domain}"))
        .unwrap_or_else(|| "global".to_string());
    let event_name = if override_kind == "htmx_mode" {
        "route.split.htmx.override.updated"
    } else {
        "route.split.override.updated"
    };
    state.observability.audit(
        AuditEvent::new(event_name, request_id.clone())
            .with_user_id(bundle.user.id)
            .with_session_id(bundle.session.session_id)
            .with_org_id(bundle.session.active_org_id)
            .with_device_id(bundle.session.device_id)
            .with_attribute("target", normalized_target)
            .with_attribute("scope", scope)
            .with_attribute("override_kind", override_kind)
            .with_attribute("htmx_mode", htmx_mode.unwrap_or("clear")),
    );
    state
        .observability
        .increment_counter(event_name, &request_id);

    web_admin_result_fragment_response(
        htmx.is_hx_request,
        "admin-action-completed",
        false,
        StatusCode::OK,
        serde_json::json!({
            "ok": true,
            "action": "route_split.override",
            "status": status,
        }),
    )
}

async fn web_admin_runtime_routing_evaluate(
    State(state): State<AppState>,
    headers: HeaderMap,
    Form(payload): Form<WebRuntimeRoutingEvaluateForm>,
) -> Response {
    let htmx = classify_htmx_request(&headers);
    let bundle = match web_admin_bundle(&state, &headers, htmx.is_hx_request).await {
        Ok(bundle) => bundle,
        Err(response) => return response,
    };
    let thread_id = payload.thread_id.trim().to_string();
    if thread_id.is_empty() {
        return web_admin_result_fragment_response(
            htmx.is_hx_request,
            "admin-action-failed",
            true,
            StatusCode::UNPROCESSABLE_ENTITY,
            serde_json::json!({
                "ok": false,
                "error": {
                    "code": "invalid_thread_id",
                    "message": "Thread id is required.",
                }
            }),
        );
    }

    let decision = state
        .runtime_routing
        .resolve(
            &state._domain_store,
            &state.codex_thread_store,
            RuntimeRoutingResolveInput {
                user_id: bundle.user.id.clone(),
                thread_id,
                autopilot_id: payload.autopilot_id,
            },
        )
        .await;
    web_admin_result_fragment_response(
        htmx.is_hx_request,
        "admin-action-completed",
        false,
        StatusCode::OK,
        serde_json::json!({
            "ok": true,
            "action": "runtime_routing.evaluate",
            "decision": decision,
        }),
    )
}

async fn web_admin_runtime_routing_override(
    State(state): State<AppState>,
    headers: HeaderMap,
    Form(payload): Form<WebRuntimeRoutingOverrideForm>,
) -> Response {
    let htmx = classify_htmx_request(&headers);
    let bundle = match web_admin_bundle(&state, &headers, htmx.is_hx_request).await {
        Ok(bundle) => bundle,
        Err(response) => return response,
    };
    let request_id = request_id(&headers);

    let scope_type = payload.scope_type.trim().to_ascii_lowercase();
    if !matches!(scope_type.as_str(), "user" | "autopilot") {
        return web_admin_result_fragment_response(
            htmx.is_hx_request,
            "admin-action-failed",
            true,
            StatusCode::UNPROCESSABLE_ENTITY,
            serde_json::json!({
                "ok": false,
                "error": {
                    "code": "invalid_scope_type",
                    "message": "Scope type must be one of: user, autopilot.",
                }
            }),
        );
    }

    let scope_id = payload.scope_id.trim().to_string();
    if scope_id.is_empty() || scope_id.chars().count() > 160 {
        return web_admin_result_fragment_response(
            htmx.is_hx_request,
            "admin-action-failed",
            true,
            StatusCode::UNPROCESSABLE_ENTITY,
            serde_json::json!({
                "ok": false,
                "error": {
                    "code": "invalid_scope_id",
                    "message": "Scope id is required and must be <= 160 characters.",
                }
            }),
        );
    }

    let Some(driver) = RuntimeDriver::parse(&payload.driver) else {
        return web_admin_result_fragment_response(
            htmx.is_hx_request,
            "admin-action-failed",
            true,
            StatusCode::UNPROCESSABLE_ENTITY,
            serde_json::json!({
                "ok": false,
                "error": {
                    "code": "invalid_driver",
                    "message": "Driver must be one of: control_service, runtime_service (legacy/elixir accepted).",
                }
            }),
        );
    };

    let reason = payload
        .reason
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string);
    if reason
        .as_deref()
        .map(|value| value.chars().count() > 255)
        .unwrap_or(false)
    {
        return web_admin_result_fragment_response(
            htmx.is_hx_request,
            "admin-action-failed",
            true,
            StatusCode::UNPROCESSABLE_ENTITY,
            serde_json::json!({
                "ok": false,
                "error": {
                    "code": "invalid_reason",
                    "message": "Reason may not be greater than 255 characters.",
                }
            }),
        );
    }

    let override_record = match state
        ._domain_store
        .upsert_runtime_driver_override(UpsertRuntimeDriverOverrideInput {
            scope_type,
            scope_id,
            driver: driver.as_str().to_string(),
            is_active: payload.is_active.is_some(),
            reason,
            meta: None,
        })
        .await
    {
        Ok(record) => record,
        Err(error) => {
            return web_admin_result_fragment_response(
                htmx.is_hx_request,
                "admin-action-failed",
                true,
                StatusCode::UNPROCESSABLE_ENTITY,
                serde_json::json!({
                    "ok": false,
                    "error": {
                        "code": "runtime_override_failed",
                        "message": error.to_string(),
                    }
                }),
            );
        }
    };

    state.observability.audit(
        AuditEvent::new("runtime.routing.override.updated", request_id.clone())
            .with_user_id(bundle.user.id)
            .with_session_id(bundle.session.session_id)
            .with_org_id(bundle.session.active_org_id)
            .with_device_id(bundle.session.device_id)
            .with_attribute("scope_type", override_record.scope_type.clone())
            .with_attribute("scope_id", override_record.scope_id.clone())
            .with_attribute("driver", override_record.driver.clone())
            .with_attribute("is_active", override_record.is_active.to_string()),
    );
    state
        .observability
        .increment_counter("runtime.routing.override.updated", &request_id);

    let status = state.runtime_routing.status(&state._domain_store).await;
    web_admin_result_fragment_response(
        htmx.is_hx_request,
        "admin-action-completed",
        false,
        StatusCode::OK,
        serde_json::json!({
            "ok": true,
            "action": "runtime_routing.override",
            "override": override_record,
            "status": status,
        }),
    )
}

async fn web_admin_lightning_ops_query(
    State(state): State<AppState>,
    headers: HeaderMap,
    Form(payload): Form<WebLightningOpsForm>,
) -> Response {
    let htmx = classify_htmx_request(&headers);
    if let Err(response) = web_admin_bundle(&state, &headers, htmx.is_hx_request).await {
        return response;
    }
    let args = match parse_web_lightning_ops_args(payload.args_json) {
        Ok(args) => args,
        Err(error_payload) => {
            return web_admin_result_fragment_response(
                htmx.is_hx_request,
                "admin-action-failed",
                true,
                StatusCode::UNPROCESSABLE_ENTITY,
                error_payload,
            );
        }
    };
    let request_payload = serde_json::json!({
        "functionName": payload.function_name,
        "args": args,
    });

    match lightning_ops_control_plane_query(State(state), Json(request_payload)).await {
        Ok((status_code, Json(body))) => web_admin_result_fragment_response(
            htmx.is_hx_request,
            "admin-action-completed",
            false,
            status_code,
            body,
        ),
        Err((status_code, Json(body))) => web_admin_result_fragment_response(
            htmx.is_hx_request,
            "admin-action-failed",
            true,
            status_code,
            body,
        ),
    }
}

async fn web_admin_lightning_ops_mutation(
    State(state): State<AppState>,
    headers: HeaderMap,
    Form(payload): Form<WebLightningOpsForm>,
) -> Response {
    let htmx = classify_htmx_request(&headers);
    if let Err(response) = web_admin_bundle(&state, &headers, htmx.is_hx_request).await {
        return response;
    }
    let args = match parse_web_lightning_ops_args(payload.args_json) {
        Ok(args) => args,
        Err(error_payload) => {
            return web_admin_result_fragment_response(
                htmx.is_hx_request,
                "admin-action-failed",
                true,
                StatusCode::UNPROCESSABLE_ENTITY,
                error_payload,
            );
        }
    };
    let request_payload = serde_json::json!({
        "functionName": payload.function_name,
        "args": args,
    });

    match lightning_ops_control_plane_mutation(State(state), Json(request_payload)).await {
        Ok((status_code, Json(body))) => web_admin_result_fragment_response(
            htmx.is_hx_request,
            "admin-action-completed",
            false,
            status_code,
            body,
        ),
        Err((status_code, Json(body))) => web_admin_result_fragment_response(
            htmx.is_hx_request,
            "admin-action-failed",
            true,
            status_code,
            body,
        ),
    }
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
    let autopilot_id = autopilot.autopilot.id.clone();
    let autopilot_config_version = autopilot.autopilot.config_version;
    let prompt_context = autopilot_prompt_context(&autopilot);
    let tool_policy_audit = autopilot_tool_resolution_audit(&autopilot, true);
    let runtime_binding_payload = autopilot_runtime_binding_payload(&autopilot);
    let runtime_binding_worker_id = autopilot_runtime_binding_worker_ref(&autopilot);

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
                    &autopilot_id,
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
                    &autopilot_id,
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
        .or(runtime_binding_worker_id)
        .unwrap_or_else(|| "desktopw:shared".to_string());
    let thread_id = thread.id.clone();
    let control_request_id = format!("autopilot_stream_{}", uuid::Uuid::new_v4().simple());
    let control_request = RuntimeCodexWorkerControlRequest {
        request_id: control_request_id.clone(),
        method: "turn/start".to_string(),
        params: serde_json::json!({
            "thread_id": thread_id.clone(),
            "text": text,
            "autopilot_id": autopilot_id.clone(),
            "autopilot_config_version": autopilot_config_version,
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
            .with_attribute("autopilot_id", autopilot_id.clone())
            .with_attribute("thread_id", thread_id.clone())
            .with_attribute("worker_id", worker_id.clone())
            .with_attribute("method", "turn/start".to_string())
            .with_attribute(
                "policy_applied",
                tool_policy_audit["policyApplied"]
                    .as_bool()
                    .unwrap_or(false)
                    .to_string(),
            )
            .with_attribute("transport", "spacetime_ws".to_string()),
    );
    state
        .observability
        .increment_counter("autopilot.stream.bootstrap.accepted", &request_id);

    Ok(ok_data(serde_json::json!({
        "accepted": true,
        "autopilotId": autopilot_id,
        "autopilotConfigVersion": autopilot_config_version,
        "threadId": thread_id.clone(),
        "conversationId": thread_id,
        "streamProtocol": "disabled",
        "promptContext": prompt_context,
        "toolPolicy": tool_policy_audit,
        "runtimeBinding": runtime_binding_payload,
        "delivery": {
            "transport": "spacetime_ws",
            "topic": worker_events_topic,
            "scope": "runtime.codex_worker_events",
            "syncTokenRoute": ROUTE_SPACETIME_TOKEN,
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

async fn settings_autopilot_update(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<UpdateSettingsAutopilotRequestPayload>,
) -> Result<Response, (StatusCode, Json<ApiErrorResponse>)> {
    let request_id = request_id(&headers);
    let bundle = session_bundle_from_headers(&state, &headers).await?;

    let display_name =
        normalize_optional_bounded_trimmed_string(payload.display_name, "displayName", 120)?;
    let tagline = normalize_optional_bounded_trimmed_string(payload.tagline, "tagline", 255)?;
    let owner_display_name = normalize_optional_bounded_trimmed_string(
        payload.owner_display_name,
        "ownerDisplayName",
        120,
    )?;
    let persona_summary = payload
        .persona_summary
        .map(|value| value.trim().to_string());
    let autopilot_voice =
        normalize_optional_bounded_trimmed_string(payload.autopilot_voice, "autopilotVoice", 64)?;
    let principles = split_principles_text(payload.principles_text);

    let autopilot = match state
        ._domain_store
        .list_autopilots_for_owner(&bundle.user.id, 1)
        .await
        .map_err(map_domain_store_error)?
        .into_iter()
        .next()
    {
        Some(existing) => existing,
        None => {
            let create_display_name = display_name.clone().unwrap_or_else(|| {
                let owner_name = bundle.user.name.trim();
                if owner_name.is_empty() {
                    "Autopilot".to_string()
                } else {
                    format!("{owner_name} Autopilot")
                }
            });

            state
                ._domain_store
                .create_autopilot(CreateAutopilotInput {
                    owner_user_id: bundle.user.id.clone(),
                    owner_display_name: bundle.user.name.clone(),
                    display_name: create_display_name,
                    handle_seed: None,
                    avatar: None,
                    status: Some("active".to_string()),
                    visibility: Some("private".to_string()),
                    tagline: None,
                })
                .await
                .map_err(map_domain_store_error)?
        }
    };

    let effective_display_name =
        display_name.unwrap_or_else(|| autopilot.autopilot.display_name.clone());
    let effective_tagline =
        tagline.unwrap_or_else(|| autopilot.autopilot.tagline.clone().unwrap_or_default());
    let effective_owner_display_name =
        owner_display_name.unwrap_or_else(|| bundle.user.name.clone());
    let effective_persona_summary = persona_summary.unwrap_or_default();
    let effective_autopilot_voice = autopilot_voice.unwrap_or_default();

    let updated = state
        ._domain_store
        .update_owned_autopilot(
            &bundle.user.id,
            &autopilot.autopilot.id,
            UpdateAutopilotInput {
                display_name: Some(effective_display_name),
                tagline: Some(effective_tagline),
                profile: Some(UpsertAutopilotProfileInput {
                    owner_display_name: Some(effective_owner_display_name),
                    persona_summary: Some(effective_persona_summary),
                    autopilot_voice: Some(effective_autopilot_voice),
                    principles: Some(serde_json::Value::Array(
                        principles
                            .into_iter()
                            .map(serde_json::Value::String)
                            .collect(),
                    )),
                    ..UpsertAutopilotProfileInput::default()
                }),
                ..UpdateAutopilotInput::default()
            },
        )
        .await
        .map_err(map_domain_store_error)?;

    state.observability.audit(
        AuditEvent::new("settings.autopilot.updated", request_id.clone())
            .with_user_id(bundle.user.id.clone())
            .with_session_id(bundle.session.session_id.clone())
            .with_org_id(bundle.session.active_org_id.clone())
            .with_device_id(bundle.session.device_id.clone())
            .with_attribute("autopilot_id", updated.autopilot.id.clone())
            .with_attribute("source", "settings_profile".to_string()),
    );
    state
        .observability
        .increment_counter("settings.autopilot.updated", &request_id);

    Ok((
        StatusCode::OK,
        Json(serde_json::json!({
            "data": {
                "status": "autopilot-updated",
                "autopilot": autopilot_aggregate_payload(&updated),
            }
        })),
    )
        .into_response())
}

async fn settings_integrations_resend_upsert(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<UpsertResendIntegrationRequestPayload>,
) -> Result<Response, (StatusCode, Json<ApiErrorResponse>)> {
    let request_id = request_id(&headers);
    let bundle = session_bundle_from_headers(&state, &headers).await?;
    let api_key = payload.resend_api_key.and_then(non_empty).ok_or_else(|| {
        validation_error("resend_api_key", "The resend_api_key field is required.")
    })?;
    let api_key_len = api_key.chars().count();
    if api_key_len < 8 {
        return Err(validation_error(
            "resend_api_key",
            "The resend_api_key field must be at least 8 characters.",
        ));
    }
    if api_key_len > 4096 {
        return Err(validation_error(
            "resend_api_key",
            "The resend_api_key field may not be greater than 4096 characters.",
        ));
    }
    let sender_email = normalize_optional_email(payload.sender_email, "sender_email")?;
    let sender_name =
        normalize_optional_bounded_trimmed_string(payload.sender_name, "sender_name", 255)?;

    let result = state
        ._domain_store
        .upsert_resend_integration(UpsertResendIntegrationInput {
            user_id: bundle.user.id.clone(),
            api_key,
            sender_email,
            sender_name,
        })
        .await
        .map_err(map_domain_store_error)?;
    let status = match result.action.as_str() {
        "secret_created" => "resend-connected",
        "secret_rotated" => "resend-rotated",
        _ => "resend-updated",
    };

    state.observability.audit(
        AuditEvent::new("settings.integrations.resend.upserted", request_id.clone())
            .with_user_id(bundle.user.id)
            .with_session_id(bundle.session.session_id)
            .with_org_id(bundle.session.active_org_id)
            .with_device_id(bundle.session.device_id)
            .with_attribute("action", result.action.clone())
            .with_attribute("status", status.to_string()),
    );
    state
        .observability
        .increment_counter("settings.integrations.resend.upserted", &request_id);

    let integration = integration_payload(Some(&result.integration), "resend");
    Ok(settings_integration_response(
        &headers,
        status,
        Some(result.action),
        integration,
    ))
}

async fn settings_integrations_resend_disconnect(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response, (StatusCode, Json<ApiErrorResponse>)> {
    let request_id = request_id(&headers);
    let bundle = session_bundle_from_headers(&state, &headers).await?;
    let revoked = state
        ._domain_store
        .revoke_integration(&bundle.user.id, "resend")
        .await
        .map_err(map_domain_store_error)?;

    state.observability.audit(
        AuditEvent::new(
            "settings.integrations.resend.disconnected",
            request_id.clone(),
        )
        .with_user_id(bundle.user.id)
        .with_session_id(bundle.session.session_id)
        .with_org_id(bundle.session.active_org_id)
        .with_device_id(bundle.session.device_id)
        .with_attribute("had_integration", revoked.is_some().to_string()),
    );
    state
        .observability
        .increment_counter("settings.integrations.resend.disconnected", &request_id);

    Ok(settings_integration_response(
        &headers,
        "resend-disconnected",
        Some("secret_revoked".to_string()),
        integration_payload(revoked.as_ref(), "resend"),
    ))
}

async fn settings_integrations_resend_test(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response, (StatusCode, Json<ApiErrorResponse>)> {
    let request_id = request_id(&headers);
    let bundle = session_bundle_from_headers(&state, &headers).await?;
    let integration = state
        ._domain_store
        .find_active_integration_secret(&bundle.user.id, "resend")
        .await
        .map_err(map_domain_store_error)?
        .ok_or_else(|| {
            validation_error(
                "resend",
                "Connect an active Resend key before running a test.",
            )
        })?;

    state
        ._domain_store
        .audit_integration_test_request(&bundle.user.id, "resend")
        .await
        .map_err(map_domain_store_error)?;

    state.observability.audit(
        AuditEvent::new(
            "settings.integrations.resend.test_requested",
            request_id.clone(),
        )
        .with_user_id(bundle.user.id)
        .with_session_id(bundle.session.session_id)
        .with_org_id(bundle.session.active_org_id)
        .with_device_id(bundle.session.device_id)
        .with_attribute("integration_id", integration.id.to_string()),
    );
    state
        .observability
        .increment_counter("settings.integrations.resend.test_requested", &request_id);

    Ok(settings_integration_response(
        &headers,
        "resend-test-queued",
        Some("test_requested".to_string()),
        integration_payload(Some(&integration), "resend"),
    ))
}

async fn settings_integrations_google_redirect(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response, (StatusCode, Json<ApiErrorResponse>)> {
    let request_id = request_id(&headers);
    let bundle = session_bundle_from_headers(&state, &headers).await?;

    let client_id = state
        .config
        .google_oauth_client_id
        .clone()
        .filter(|value| !value.trim().is_empty());
    let redirect_uri = state
        .config
        .google_oauth_redirect_uri
        .clone()
        .filter(|value| !value.trim().is_empty());
    let scopes = state.config.google_oauth_scopes.trim().to_string();
    if client_id.is_none() || redirect_uri.is_none() || scopes.is_empty() {
        return Err(validation_error(
            "google",
            "Google OAuth is not configured on this environment.",
        ));
    }

    let oauth_state = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
    {
        let mut entries = state.google_oauth_states.entries.lock().await;
        entries.insert(bundle.user.id.clone(), oauth_state.clone());
    }

    let mut redirect = reqwest::Url::parse("https://accounts.google.com/o/oauth2/v2/auth")
        .expect("hardcoded Google OAuth authorize URL must be valid");
    redirect
        .query_pairs_mut()
        .append_pair("client_id", client_id.as_deref().unwrap_or_default())
        .append_pair("redirect_uri", redirect_uri.as_deref().unwrap_or_default())
        .append_pair("response_type", "code")
        .append_pair("access_type", "offline")
        .append_pair("prompt", "consent")
        .append_pair("scope", &scopes)
        .append_pair("state", &oauth_state);

    state.observability.audit(
        AuditEvent::new(
            "settings.integrations.google.redirected",
            request_id.clone(),
        )
        .with_user_id(bundle.user.id)
        .with_session_id(bundle.session.session_id)
        .with_org_id(bundle.session.active_org_id)
        .with_device_id(bundle.session.device_id),
    );
    state
        .observability
        .increment_counter("settings.integrations.google.redirected", &request_id);

    Ok(Redirect::to(redirect.as_ref()).into_response())
}

async fn settings_integrations_google_callback(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<GoogleOauthCallbackQuery>,
) -> Result<Response, (StatusCode, Json<ApiErrorResponse>)> {
    let request_id = request_id(&headers);
    let bundle = session_bundle_from_headers(&state, &headers).await?;

    if let Some(error) = query.error.and_then(non_empty) {
        return Err(validation_error(
            "google",
            &format!("Google authorization failed: {error}"),
        ));
    }

    let incoming_state = query.state.and_then(non_empty).ok_or_else(|| {
        validation_error(
            "google",
            "OAuth state mismatch. Please retry connecting Google.",
        )
    })?;
    let expected_state = {
        let mut entries = state.google_oauth_states.entries.lock().await;
        entries.remove(&bundle.user.id)
    }
    .unwrap_or_default();
    if expected_state.is_empty() || expected_state != incoming_state {
        return Err(validation_error(
            "google",
            "OAuth state mismatch. Please retry connecting Google.",
        ));
    }

    let code = query.code.and_then(non_empty).ok_or_else(|| {
        validation_error(
            "google",
            "Google callback did not include an authorization code.",
        )
    })?;

    let client_id = state
        .config
        .google_oauth_client_id
        .clone()
        .filter(|value| !value.trim().is_empty());
    let client_secret = state
        .config
        .google_oauth_client_secret
        .clone()
        .filter(|value| !value.trim().is_empty());
    let redirect_uri = state
        .config
        .google_oauth_redirect_uri
        .clone()
        .filter(|value| !value.trim().is_empty());
    if client_id.is_none() || client_secret.is_none() || redirect_uri.is_none() {
        return Err(validation_error(
            "google",
            "Google OAuth is not configured on this environment.",
        ));
    }

    let response = reqwest::Client::new()
        .post(state.config.google_oauth_token_url.clone())
        .header("accept", "application/json")
        .form(&[
            ("code", code.as_str()),
            ("client_id", client_id.as_deref().unwrap_or_default()),
            (
                "client_secret",
                client_secret.as_deref().unwrap_or_default(),
            ),
            ("redirect_uri", redirect_uri.as_deref().unwrap_or_default()),
            ("grant_type", "authorization_code"),
        ])
        .timeout(std::time::Duration::from_secs(15))
        .send()
        .await
        .map_err(|_| {
            validation_error(
                "google",
                "Google token exchange failed. Please reconnect and try again.",
            )
        })?;
    if !response.status().is_success() {
        return Err(validation_error(
            "google",
            "Google token exchange failed. Please reconnect and try again.",
        ));
    }

    let token_payload = response.json::<serde_json::Value>().await.map_err(|_| {
        validation_error(
            "google",
            "Google token exchange returned an invalid payload.",
        )
    })?;
    if !token_payload.is_object() {
        return Err(validation_error(
            "google",
            "Google token exchange returned an invalid payload.",
        ));
    }

    let result = state
        ._domain_store
        .upsert_google_integration(UpsertGoogleIntegrationInput {
            user_id: bundle.user.id.clone(),
            refresh_token: normalized_json_string(token_payload.get("refresh_token")),
            access_token: normalized_json_string(token_payload.get("access_token")),
            scope: normalized_json_string(token_payload.get("scope")),
            token_type: normalized_json_string(token_payload.get("token_type")),
            expires_at: resolve_google_token_expiry(&token_payload),
        })
        .await
        .map_err(map_domain_store_error)?;
    let status = match result.action.as_str() {
        "secret_created" => "google-connected",
        "secret_rotated" => "google-rotated",
        _ => "google-updated",
    };

    state.observability.audit(
        AuditEvent::new(
            "settings.integrations.google.callback.completed",
            request_id.clone(),
        )
        .with_user_id(bundle.user.id)
        .with_session_id(bundle.session.session_id)
        .with_org_id(bundle.session.active_org_id)
        .with_device_id(bundle.session.device_id)
        .with_attribute("action", result.action.clone())
        .with_attribute("status", status.to_string()),
    );
    state.observability.increment_counter(
        "settings.integrations.google.callback.completed",
        &request_id,
    );

    Ok(Redirect::to(&format!("/settings/integrations?status={status}")).into_response())
}

async fn settings_integrations_google_disconnect(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response, (StatusCode, Json<ApiErrorResponse>)> {
    let request_id = request_id(&headers);
    let bundle = session_bundle_from_headers(&state, &headers).await?;
    let revoked = state
        ._domain_store
        .revoke_integration(&bundle.user.id, "google")
        .await
        .map_err(map_domain_store_error)?;

    state.observability.audit(
        AuditEvent::new(
            "settings.integrations.google.disconnected",
            request_id.clone(),
        )
        .with_user_id(bundle.user.id)
        .with_session_id(bundle.session.session_id)
        .with_org_id(bundle.session.active_org_id)
        .with_device_id(bundle.session.device_id)
        .with_attribute("had_integration", revoked.is_some().to_string()),
    );
    state
        .observability
        .increment_counter("settings.integrations.google.disconnected", &request_id);

    Ok(settings_integration_response(
        &headers,
        "google-disconnected",
        Some("secret_revoked".to_string()),
        integration_payload(revoked.as_ref(), "google"),
    ))
}

async fn inbox_threads_index(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<InboxThreadsQuery>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiErrorResponse>)> {
    let request_id = request_id(&headers);
    let bundle = session_bundle_from_headers(&state, &headers).await?;
    let limit = parse_inbox_limit(query.limit.as_deref())?;
    let snapshot = inbox_snapshot_for_user(&state, &bundle.user.id, limit, &request_id).await?;

    state.observability.audit(
        AuditEvent::new("inbox.threads.listed", request_id.clone())
            .with_user_id(bundle.user.id.clone())
            .with_session_id(bundle.session.session_id.clone())
            .with_org_id(bundle.session.active_org_id.clone())
            .with_device_id(bundle.session.device_id.clone())
            .with_attribute("thread_count", snapshot.threads.len().to_string()),
    );
    state
        .observability
        .increment_counter("inbox.threads.listed", &request_id);

    Ok(ok_data(InboxSnapshotEnvelope {
        request_id,
        source: "threads".to_string(),
        snapshot,
    }))
}

async fn inbox_refresh(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<InboxRefreshRequestPayload>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiErrorResponse>)> {
    let request_id = request_id(&headers);
    let bundle = session_bundle_from_headers(&state, &headers).await?;
    let limit = payload.limit.unwrap_or(20).clamp(1, 100) as usize;
    let snapshot = inbox_snapshot_for_user(&state, &bundle.user.id, limit, &request_id).await?;

    state
        ._domain_store
        .record_inbox_audit(RecordInboxAuditInput {
            user_id: bundle.user.id.clone(),
            thread_id: "system".to_string(),
            action: "refresh".to_string(),
            detail: format!("gmail inbox refreshed ({} threads)", snapshot.threads.len()),
        })
        .await
        .map_err(map_domain_store_error)?;

    state.observability.audit(
        AuditEvent::new("inbox.refreshed", request_id.clone())
            .with_user_id(bundle.user.id.clone())
            .with_session_id(bundle.session.session_id.clone())
            .with_org_id(bundle.session.active_org_id.clone())
            .with_device_id(bundle.session.device_id.clone())
            .with_attribute("thread_count", snapshot.threads.len().to_string()),
    );
    state
        .observability
        .increment_counter("inbox.refreshed", &request_id);

    Ok(ok_data(InboxSnapshotEnvelope {
        request_id,
        source: "refresh".to_string(),
        snapshot,
    }))
}

async fn inbox_thread_detail(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(thread_id): Path<String>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiErrorResponse>)> {
    let request_id = request_id(&headers);
    let bundle = session_bundle_from_headers(&state, &headers).await?;
    let thread_id = normalize_optional_bounded_string(Some(thread_id), "thread_id", 255)?
        .ok_or_else(|| validation_error("thread_id", "The thread_id field is required."))?;

    let detail =
        inbox_thread_detail_for_user(&state, &bundle.user.id, &thread_id, &request_id).await?;

    state
        ._domain_store
        .record_inbox_audit(RecordInboxAuditInput {
            user_id: bundle.user.id.clone(),
            thread_id: thread_id.clone(),
            action: "select_thread".to_string(),
            detail: "thread detail loaded".to_string(),
        })
        .await
        .map_err(map_domain_store_error)?;

    state.observability.audit(
        AuditEvent::new("inbox.thread.viewed", request_id.clone())
            .with_user_id(bundle.user.id.clone())
            .with_session_id(bundle.session.session_id.clone())
            .with_org_id(bundle.session.active_org_id.clone())
            .with_device_id(bundle.session.device_id.clone())
            .with_attribute("thread_id", thread_id.clone()),
    );
    state
        .observability
        .increment_counter("inbox.thread.viewed", &request_id);

    Ok(ok_data(detail))
}

async fn inbox_thread_approve(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(thread_id): Path<String>,
    Json(payload): Json<InboxDraftActionRequestPayload>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiErrorResponse>)> {
    let request_id = request_id(&headers);
    let bundle = session_bundle_from_headers(&state, &headers).await?;
    let thread_id = normalize_optional_bounded_string(Some(thread_id), "thread_id", 255)?
        .ok_or_else(|| validation_error("thread_id", "The thread_id field is required."))?;

    let existing = state
        ._domain_store
        .inbox_thread_state(&bundle.user.id, &thread_id)
        .await
        .map_err(map_domain_store_error)?;
    let draft_preview = existing.as_ref().and_then(|row| row.draft_preview.clone());

    state
        ._domain_store
        .upsert_inbox_thread_state(UpsertInboxThreadStateInput {
            user_id: bundle.user.id.clone(),
            thread_id: thread_id.clone(),
            pending_approval: false,
            decision: Some("approved".to_string()),
            draft_preview,
            source: Some("inbox_api.approve".to_string()),
        })
        .await
        .map_err(map_domain_store_error)?;
    state
        ._domain_store
        .record_inbox_audit(RecordInboxAuditInput {
            user_id: bundle.user.id.clone(),
            thread_id: thread_id.clone(),
            action: "approve_draft".to_string(),
            detail: payload
                .detail
                .and_then(non_empty)
                .unwrap_or_else(|| "draft approved".to_string()),
        })
        .await
        .map_err(map_domain_store_error)?;

    let snapshot = inbox_snapshot_for_user(&state, &bundle.user.id, 20, &request_id).await?;

    state.observability.audit(
        AuditEvent::new("inbox.draft.approved", request_id.clone())
            .with_user_id(bundle.user.id.clone())
            .with_session_id(bundle.session.session_id.clone())
            .with_org_id(bundle.session.active_org_id.clone())
            .with_device_id(bundle.session.device_id.clone())
            .with_attribute("thread_id", thread_id),
    );
    state
        .observability
        .increment_counter("inbox.draft.approved", &request_id);

    Ok(ok_data(InboxSnapshotEnvelope {
        request_id,
        source: "approve_draft".to_string(),
        snapshot,
    }))
}

async fn inbox_thread_reject(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(thread_id): Path<String>,
    Json(payload): Json<InboxDraftActionRequestPayload>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiErrorResponse>)> {
    let request_id = request_id(&headers);
    let bundle = session_bundle_from_headers(&state, &headers).await?;
    let thread_id = normalize_optional_bounded_string(Some(thread_id), "thread_id", 255)?
        .ok_or_else(|| validation_error("thread_id", "The thread_id field is required."))?;

    let existing = state
        ._domain_store
        .inbox_thread_state(&bundle.user.id, &thread_id)
        .await
        .map_err(map_domain_store_error)?;
    let draft_preview = existing.as_ref().and_then(|row| row.draft_preview.clone());

    state
        ._domain_store
        .upsert_inbox_thread_state(UpsertInboxThreadStateInput {
            user_id: bundle.user.id.clone(),
            thread_id: thread_id.clone(),
            pending_approval: true,
            decision: Some("rejected".to_string()),
            draft_preview,
            source: Some("inbox_api.reject".to_string()),
        })
        .await
        .map_err(map_domain_store_error)?;
    state
        ._domain_store
        .record_inbox_audit(RecordInboxAuditInput {
            user_id: bundle.user.id.clone(),
            thread_id: thread_id.clone(),
            action: "reject_draft".to_string(),
            detail: payload
                .detail
                .and_then(non_empty)
                .unwrap_or_else(|| "draft rejected for manual revision".to_string()),
        })
        .await
        .map_err(map_domain_store_error)?;

    let snapshot = inbox_snapshot_for_user(&state, &bundle.user.id, 20, &request_id).await?;

    state.observability.audit(
        AuditEvent::new("inbox.draft.rejected", request_id.clone())
            .with_user_id(bundle.user.id.clone())
            .with_session_id(bundle.session.session_id.clone())
            .with_org_id(bundle.session.active_org_id.clone())
            .with_device_id(bundle.session.device_id.clone())
            .with_attribute("thread_id", thread_id),
    );
    state
        .observability
        .increment_counter("inbox.draft.rejected", &request_id);

    Ok(ok_data(InboxSnapshotEnvelope {
        request_id,
        source: "reject_draft".to_string(),
        snapshot,
    }))
}

async fn inbox_thread_reply_send(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(thread_id): Path<String>,
    Json(payload): Json<InboxSendReplyRequestPayload>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiErrorResponse>)> {
    let request_id = request_id(&headers);
    let bundle = session_bundle_from_headers(&state, &headers).await?;
    let thread_id = normalize_optional_bounded_string(Some(thread_id), "thread_id", 255)?
        .ok_or_else(|| validation_error("thread_id", "The thread_id field is required."))?;

    let detail =
        inbox_thread_detail_for_user(&state, &bundle.user.id, &thread_id, &request_id).await?;
    let latest_message = detail.messages.last().cloned().ok_or_else(|| {
        validation_error(
            "thread_id",
            "Inbox thread has no messages and cannot be replied to.",
        )
    })?;

    let to_header = latest_message
        .from
        .as_deref()
        .and_then(|value| non_empty(value.to_string()))
        .ok_or_else(|| validation_error("thread_id", "Could not resolve recipient from thread."))?;
    let to_address = normalize_reply_recipient(to_header.as_str())
        .ok_or_else(|| validation_error("thread_id", "Could not resolve recipient from thread."))?;

    let draft_body = payload
        .body
        .and_then(non_empty)
        .or_else(|| {
            if detail.thread.draft_preview.trim().is_empty() {
                None
            } else {
                Some(detail.thread.draft_preview.clone())
            }
        })
        .ok_or_else(|| validation_error("body", "Reply body is required to send this thread."))?;

    let subject = normalize_reply_subject(
        latest_message
            .subject
            .as_deref()
            .unwrap_or(detail.thread.subject.as_str()),
    );
    let in_reply_to = if latest_message.id.trim().is_empty() {
        None
    } else {
        Some(format!("<{}>", latest_message.id))
    };

    let mut raw_lines = vec![
        format!("To: {to_address}"),
        format!("Subject: {subject}"),
        "MIME-Version: 1.0".to_string(),
        "Content-Type: text/plain; charset=UTF-8".to_string(),
    ];
    if let Some(message_id) = in_reply_to.as_ref() {
        raw_lines.push(format!("In-Reply-To: {message_id}"));
        raw_lines.push(format!("References: {message_id}"));
    }
    raw_lines.push(String::new());
    raw_lines.push(draft_body.clone());
    let raw = URL_SAFE_NO_PAD.encode(raw_lines.join("\r\n"));

    let send_payload = serde_json::json!({
        "threadId": thread_id,
        "raw": raw,
    });
    let send_url = format!(
        "{}/gmail/v1/users/me/messages/send",
        google_gmail_api_base_url(state.config.as_ref())
    );
    let response = gmail_json_request(
        &state,
        &bundle.user.id,
        reqwest::Method::POST,
        send_url.as_str(),
        Some(send_payload),
        &request_id,
    )
    .await?;
    let message_id = normalized_json_string(response.get("id"))
        .unwrap_or_else(|| format!("msg_{}", Uuid::new_v4().simple()));

    state
        ._domain_store
        .upsert_inbox_thread_state(UpsertInboxThreadStateInput {
            user_id: bundle.user.id.clone(),
            thread_id: thread_id.clone(),
            pending_approval: false,
            decision: Some("sent".to_string()),
            draft_preview: Some(draft_body),
            source: Some("inbox_api.send".to_string()),
        })
        .await
        .map_err(map_domain_store_error)?;
    state
        ._domain_store
        .record_inbox_audit(RecordInboxAuditInput {
            user_id: bundle.user.id.clone(),
            thread_id: thread_id.clone(),
            action: "send_reply".to_string(),
            detail: format!("reply sent to {to_address} ({message_id})"),
        })
        .await
        .map_err(map_domain_store_error)?;

    state.observability.audit(
        AuditEvent::new("inbox.reply.sent", request_id.clone())
            .with_user_id(bundle.user.id.clone())
            .with_session_id(bundle.session.session_id.clone())
            .with_org_id(bundle.session.active_org_id.clone())
            .with_device_id(bundle.session.device_id.clone())
            .with_attribute("thread_id", thread_id.clone())
            .with_attribute("message_id", message_id.clone()),
    );
    state
        .observability
        .increment_counter("inbox.reply.sent", &request_id);

    Ok(ok_data(InboxReplySendResponse {
        request_id,
        thread_id,
        message_id,
        status: "sent".to_string(),
    }))
}

async fn inbox_snapshot_for_user(
    state: &AppState,
    user_id: &str,
    limit: usize,
    request_id: &str,
) -> Result<InboxSnapshot, (StatusCode, Json<ApiErrorResponse>)> {
    let thread_rows = gmail_thread_summaries_for_user(state, user_id, limit, request_id).await?;
    let audit_rows = state
        ._domain_store
        .list_inbox_audits_for_user(user_id, None, 200)
        .await
        .map_err(map_domain_store_error)?;
    let audit_log = audit_rows
        .into_iter()
        .map(|entry| InboxAuditEntry {
            thread_id: entry.thread_id,
            action: entry.action,
            detail: entry.detail,
            created_at: timestamp(entry.created_at),
        })
        .collect::<Vec<_>>();

    Ok(InboxSnapshot {
        selected_thread_id: thread_rows.first().map(|row| row.id.clone()),
        threads: thread_rows,
        audit_log,
    })
}

async fn inbox_thread_detail_for_user(
    state: &AppState,
    user_id: &str,
    thread_id: &str,
    request_id: &str,
) -> Result<InboxThreadDetailResponse, (StatusCode, Json<ApiErrorResponse>)> {
    let gmail_thread = gmail_fetch_thread_for_user(state, user_id, thread_id, request_id).await?;
    let persisted = state
        ._domain_store
        .inbox_thread_state(user_id, thread_id)
        .await
        .map_err(map_domain_store_error)?;
    let summary = inbox_summary_from_gmail_thread(thread_id, &gmail_thread, persisted.as_ref());
    let audit_rows = state
        ._domain_store
        .list_inbox_audits_for_user(user_id, Some(thread_id), 100)
        .await
        .map_err(map_domain_store_error)?;
    let audit_log = audit_rows
        .into_iter()
        .map(|entry| InboxAuditEntry {
            thread_id: entry.thread_id,
            action: entry.action,
            detail: entry.detail,
            created_at: timestamp(entry.created_at),
        })
        .collect::<Vec<_>>();
    let messages = gmail_thread
        .messages
        .iter()
        .map(inbox_message_from_gmail)
        .collect::<Vec<_>>();

    Ok(InboxThreadDetailResponse {
        request_id: request_id.to_string(),
        thread_id: thread_id.to_string(),
        thread: summary,
        messages,
        audit_log,
    })
}

async fn gmail_thread_summaries_for_user(
    state: &AppState,
    user_id: &str,
    limit: usize,
    request_id: &str,
) -> Result<Vec<InboxThreadSummary>, (StatusCode, Json<ApiErrorResponse>)> {
    let base_url = google_gmail_api_base_url(state.config.as_ref());
    let list_url = format!(
        "{base_url}/gmail/v1/users/me/threads?maxResults={}",
        limit.clamp(1, 100)
    );
    let response = gmail_json_request(
        state,
        user_id,
        reqwest::Method::GET,
        list_url.as_str(),
        None,
        request_id,
    )
    .await?;
    let list = serde_json::from_value::<GmailThreadListResponse>(response).map_err(|_| {
        error_response_with_status(
            StatusCode::SERVICE_UNAVAILABLE,
            ApiErrorCode::ServiceUnavailable,
            "Google mailbox list payload was invalid.",
        )
    })?;

    let state_rows = state
        ._domain_store
        .list_inbox_thread_states_for_user(user_id)
        .await
        .map_err(map_domain_store_error)?;
    let state_by_thread_id = state_rows
        .into_iter()
        .map(|row| (row.thread_id.clone(), row))
        .collect::<HashMap<_, _>>();

    let mut rows = Vec::with_capacity(list.threads.len());
    for thread in list.threads {
        let detail = gmail_fetch_thread_for_user(state, user_id, &thread.id, request_id).await?;
        let summary = inbox_summary_from_gmail_thread(
            &thread.id,
            &detail,
            state_by_thread_id.get(&thread.id),
        );
        rows.push(summary);
    }

    rows.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
    Ok(rows)
}

async fn gmail_fetch_thread_for_user(
    state: &AppState,
    user_id: &str,
    thread_id: &str,
    request_id: &str,
) -> Result<GmailThreadResponse, (StatusCode, Json<ApiErrorResponse>)> {
    let url = format!(
        "{}/gmail/v1/users/me/threads/{thread_id}?format=full",
        google_gmail_api_base_url(state.config.as_ref())
    );
    let response = gmail_json_request(
        state,
        user_id,
        reqwest::Method::GET,
        url.as_str(),
        None,
        request_id,
    )
    .await?;

    serde_json::from_value::<GmailThreadResponse>(response).map_err(|_| {
        error_response_with_status(
            StatusCode::SERVICE_UNAVAILABLE,
            ApiErrorCode::ServiceUnavailable,
            "Google mailbox thread payload was invalid.",
        )
    })
}

fn inbox_summary_from_gmail_thread(
    thread_id: &str,
    thread: &GmailThreadResponse,
    persisted_state: Option<&crate::domain_store::InboxThreadStateRecord>,
) -> InboxThreadSummary {
    let latest = thread.messages.last();
    let subject = latest
        .and_then(gmail_message_subject)
        .or_else(|| gmail_thread_subject(thread))
        .unwrap_or_else(|| "No subject".to_string());
    let from_address = latest
        .and_then(gmail_message_from)
        .unwrap_or_else(|| "unknown@unknown".to_string());
    let snippet = latest
        .and_then(|message| message.snippet.clone())
        .or_else(|| thread.snippet.clone())
        .unwrap_or_default();
    let body = latest
        .map(gmail_message_body)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| snippet.clone());

    let decision = classify_thread(subject.as_str(), body.as_str(), snippet.as_str());
    let style_signature = infer_style_signature_from_bodies([body.as_str()]);
    let generated_draft = compose_local_draft(
        decision.category,
        subject.as_str(),
        body.as_str(),
        None,
        None,
        None,
        style_signature.as_str(),
    );

    let pending_approval = persisted_state
        .map(|row| row.pending_approval)
        .unwrap_or(matches!(
            decision.policy,
            PolicyDecision::DraftOnly | PolicyDecision::SendWithApproval
        ));
    let draft_preview = persisted_state
        .and_then(|row| row.draft_preview.clone())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(generated_draft);
    let updated_at = latest
        .and_then(|message| gmail_internal_date_to_timestamp(message.internal_date.as_deref()))
        .unwrap_or_else(|| timestamp(Utc::now()));

    InboxThreadSummary {
        id: thread_id.to_string(),
        subject,
        from_address,
        snippet,
        category: decision.category.as_str().to_string(),
        risk: risk_to_str(decision.risk).to_string(),
        policy: decision.policy.as_str().to_string(),
        draft_preview,
        pending_approval,
        updated_at,
    }
}

fn inbox_message_from_gmail(message: &GmailMessage) -> InboxThreadMessage {
    InboxThreadMessage {
        id: message.id.clone(),
        from: gmail_message_from(message),
        to: gmail_message_to(message),
        subject: gmail_message_subject(message),
        snippet: message.snippet.clone().unwrap_or_default(),
        body: gmail_message_body(message),
        created_at: gmail_internal_date_to_timestamp(message.internal_date.as_deref()),
    }
}

fn gmail_message_subject(message: &GmailMessage) -> Option<String> {
    gmail_message_header_value(message, "subject")
}

fn gmail_message_from(message: &GmailMessage) -> Option<String> {
    gmail_message_header_value(message, "from")
}

fn gmail_message_to(message: &GmailMessage) -> Option<String> {
    gmail_message_header_value(message, "to")
}

fn gmail_thread_subject(thread: &GmailThreadResponse) -> Option<String> {
    thread.messages.iter().rev().find_map(gmail_message_subject)
}

fn gmail_message_header_value(message: &GmailMessage, name: &str) -> Option<String> {
    let payload = message.payload.as_ref()?;
    payload
        .headers
        .iter()
        .find(|header| header.name.eq_ignore_ascii_case(name))
        .map(|header| header.value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn gmail_message_body(message: &GmailMessage) -> String {
    message
        .payload
        .as_ref()
        .and_then(gmail_payload_body)
        .unwrap_or_else(|| message.snippet.clone().unwrap_or_default())
}

fn gmail_payload_body(payload: &GmailMessagePayload) -> Option<String> {
    if let Some(data) = payload
        .body
        .as_ref()
        .and_then(|body| body.data.as_deref())
        .and_then(gmail_base64_decode)
    {
        let text = data.trim().to_string();
        if !text.is_empty() {
            return Some(text);
        }
    }

    for part in &payload.parts {
        if let Some(text) = gmail_payload_body(part) {
            let trimmed = text.trim().to_string();
            if !trimmed.is_empty() {
                return Some(trimmed);
            }
        }
    }

    None
}

fn gmail_base64_decode(input: &str) -> Option<String> {
    URL_SAFE_NO_PAD
        .decode(input.as_bytes())
        .or_else(|_| STANDARD.decode(input.as_bytes()))
        .ok()
        .and_then(|bytes| String::from_utf8(bytes).ok())
}

fn gmail_internal_date_to_timestamp(value: Option<&str>) -> Option<String> {
    let millis = value?.parse::<i64>().ok()?;
    let dt = chrono::DateTime::<Utc>::from_timestamp_millis(millis)?;
    Some(timestamp(dt))
}

fn google_gmail_api_base_url(config: &Config) -> &str {
    config.google_gmail_api_base_url.as_str()
}

fn normalize_reply_recipient(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    if trimmed.contains('<') && trimmed.contains('>') {
        let start = trimmed.find('<')?;
        let end = trimmed.rfind('>')?;
        if end > start + 1 {
            let email = trimmed[start + 1..end].trim().to_string();
            return (!email.is_empty()).then_some(email);
        }
    }
    Some(trimmed.to_string())
}

fn normalize_reply_subject(raw: &str) -> String {
    let subject = raw.trim();
    if subject.is_empty() {
        return "Re: (no subject)".to_string();
    }
    if subject.to_ascii_lowercase().starts_with("re:") {
        subject.to_string()
    } else {
        format!("Re: {subject}")
    }
}

fn parse_inbox_limit(raw: Option<&str>) -> Result<usize, (StatusCode, Json<ApiErrorResponse>)> {
    let Some(raw) = raw else {
        return Ok(20);
    };
    let parsed = raw
        .trim()
        .parse::<usize>()
        .map_err(|_| validation_error("limit", "The limit field must be an integer."))?;
    if !(1..=100).contains(&parsed) {
        return Err(validation_error(
            "limit",
            "The limit field must be between 1 and 100.",
        ));
    }
    Ok(parsed)
}

async fn gmail_json_request(
    state: &AppState,
    user_id: &str,
    method: reqwest::Method,
    url: &str,
    body: Option<serde_json::Value>,
    request_id: &str,
) -> Result<serde_json::Value, (StatusCode, Json<ApiErrorResponse>)> {
    let client = reqwest::Client::new();
    let mut access_token = google_access_token_for_user(state, user_id, false).await?;
    let mut refreshed_after_401 = false;
    let max_attempts = 3usize;

    for attempt in 1..=max_attempts {
        let mut request = client
            .request(method.clone(), url)
            .header("accept", "application/json")
            .bearer_auth(access_token.as_str())
            .timeout(std::time::Duration::from_secs(15));
        if let Some(body) = body.as_ref() {
            request = request.json(body);
        }

        let response = match request.send().await {
            Ok(value) => value,
            Err(_) => {
                if attempt < max_attempts {
                    tokio::time::sleep(std::time::Duration::from_millis(200 * attempt as u64))
                        .await;
                    continue;
                }
                state
                    .observability
                    .increment_counter("inbox.gmail.request.failed", request_id);
                return Err(error_response_with_status(
                    StatusCode::SERVICE_UNAVAILABLE,
                    ApiErrorCode::ServiceUnavailable,
                    "Google mailbox request failed.",
                ));
            }
        };

        let status = response.status();
        let bytes = response.bytes().await.map_err(|_| {
            error_response_with_status(
                StatusCode::SERVICE_UNAVAILABLE,
                ApiErrorCode::ServiceUnavailable,
                "Google mailbox response could not be read.",
            )
        })?;

        if status == StatusCode::UNAUTHORIZED && !refreshed_after_401 {
            access_token = google_access_token_for_user(state, user_id, true).await?;
            refreshed_after_401 = true;
            continue;
        }

        if !status.is_success() {
            if attempt < max_attempts
                && (status.is_server_error() || status == StatusCode::TOO_MANY_REQUESTS)
            {
                tokio::time::sleep(std::time::Duration::from_millis(200 * attempt as u64)).await;
                continue;
            }
            state
                .observability
                .increment_counter("inbox.gmail.request.failed", request_id);
            return Err(map_gmail_error_status(status));
        }

        let value = serde_json::from_slice::<serde_json::Value>(&bytes).map_err(|_| {
            error_response_with_status(
                StatusCode::SERVICE_UNAVAILABLE,
                ApiErrorCode::ServiceUnavailable,
                "Google mailbox response payload was invalid.",
            )
        })?;
        return Ok(value);
    }

    Err(error_response_with_status(
        StatusCode::SERVICE_UNAVAILABLE,
        ApiErrorCode::ServiceUnavailable,
        "Google mailbox request failed after retries.",
    ))
}

async fn google_access_token_for_user(
    state: &AppState,
    user_id: &str,
    force_refresh: bool,
) -> Result<String, (StatusCode, Json<ApiErrorResponse>)> {
    let integration = state
        ._domain_store
        .find_active_integration_secret(user_id, "google")
        .await
        .map_err(map_domain_store_error)?
        .ok_or_else(|| {
            validation_error(
                "google",
                "Connect an active Google integration before using inbox.",
            )
        })?;
    let secret_raw = integration
        .encrypted_secret
        .as_deref()
        .and_then(|value| non_empty(value.to_string()))
        .ok_or_else(|| {
            validation_error(
                "google",
                "Google integration secret is missing. Reconnect Google.",
            )
        })?;
    let secret = serde_json::from_str::<GoogleIntegrationSecretPayload>(secret_raw.as_str())
        .map_err(|_| {
            error_response_with_status(
                StatusCode::SERVICE_UNAVAILABLE,
                ApiErrorCode::ServiceUnavailable,
                "Google integration secret payload is invalid.",
            )
        })?;

    let is_fresh = secret
        .expires_at
        .as_deref()
        .and_then(|value| parse_rfc3339_utc(value).ok())
        .map(|expiry| expiry > Utc::now() + Duration::seconds(30))
        .unwrap_or(false);
    if !force_refresh {
        if let Some(token) = secret
            .access_token
            .as_deref()
            .and_then(|value| non_empty(value.to_string()))
        {
            if is_fresh {
                return Ok(token);
            }
        }
    }

    let refresh_token = secret
        .refresh_token
        .as_deref()
        .and_then(|value| non_empty(value.to_string()))
        .ok_or_else(|| {
            validation_error(
                "google",
                "Google refresh token is missing. Reconnect Google integration.",
            )
        })?;
    let client_id = state
        .config
        .google_oauth_client_id
        .clone()
        .and_then(non_empty)
        .ok_or_else(|| {
            error_response_with_status(
                StatusCode::SERVICE_UNAVAILABLE,
                ApiErrorCode::ServiceUnavailable,
                "Google OAuth client id is not configured.",
            )
        })?;
    let client_secret = state
        .config
        .google_oauth_client_secret
        .clone()
        .and_then(non_empty)
        .ok_or_else(|| {
            error_response_with_status(
                StatusCode::SERVICE_UNAVAILABLE,
                ApiErrorCode::ServiceUnavailable,
                "Google OAuth client secret is not configured.",
            )
        })?;

    let refresh_response = reqwest::Client::new()
        .post(state.config.google_oauth_token_url.clone())
        .header("accept", "application/json")
        .form(&[
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token.as_str()),
            ("client_id", client_id.as_str()),
            ("client_secret", client_secret.as_str()),
        ])
        .timeout(std::time::Duration::from_secs(15))
        .send()
        .await
        .map_err(|_| {
            error_response_with_status(
                StatusCode::SERVICE_UNAVAILABLE,
                ApiErrorCode::ServiceUnavailable,
                "Google token refresh request failed.",
            )
        })?;
    if !refresh_response.status().is_success() {
        return Err(error_response_with_status(
            StatusCode::SERVICE_UNAVAILABLE,
            ApiErrorCode::ServiceUnavailable,
            "Google token refresh failed. Reconnect Google integration.",
        ));
    }
    let token_payload = refresh_response
        .json::<serde_json::Value>()
        .await
        .map_err(|_| {
            error_response_with_status(
                StatusCode::SERVICE_UNAVAILABLE,
                ApiErrorCode::ServiceUnavailable,
                "Google token refresh payload was invalid.",
            )
        })?;
    let access_token = normalized_json_string(token_payload.get("access_token"))
        .and_then(non_empty)
        .ok_or_else(|| {
            error_response_with_status(
                StatusCode::SERVICE_UNAVAILABLE,
                ApiErrorCode::ServiceUnavailable,
                "Google token refresh payload did not include an access token.",
            )
        })?;

    state
        ._domain_store
        .upsert_google_integration(UpsertGoogleIntegrationInput {
            user_id: user_id.to_string(),
            refresh_token: Some(refresh_token),
            access_token: Some(access_token.clone()),
            scope: normalized_json_string(token_payload.get("scope")).or(secret.scope),
            token_type: normalized_json_string(token_payload.get("token_type"))
                .or(secret.token_type),
            expires_at: resolve_google_token_expiry(&token_payload),
        })
        .await
        .map_err(map_domain_store_error)?;

    Ok(access_token)
}

fn map_gmail_error_status(status: StatusCode) -> (StatusCode, Json<ApiErrorResponse>) {
    if status == StatusCode::NOT_FOUND {
        return not_found_error("Inbox thread not found.");
    }
    if status == StatusCode::TOO_MANY_REQUESTS {
        return error_response_with_status(
            StatusCode::TOO_MANY_REQUESTS,
            ApiErrorCode::RateLimited,
            "Google mailbox rate limit exceeded. Retry shortly.",
        );
    }
    if status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN {
        return forbidden_error("Google mailbox access was denied. Reconnect Google integration.");
    }
    if status.is_client_error() {
        return validation_error("gmail", "Google mailbox request was rejected.");
    }

    error_response_with_status(
        StatusCode::SERVICE_UNAVAILABLE,
        ApiErrorCode::ServiceUnavailable,
        "Google mailbox request failed.",
    )
}

fn settings_integration_response(
    _headers: &HeaderMap,
    status: &str,
    action: Option<String>,
    integration: serde_json::Value,
) -> Response {
    (
        StatusCode::OK,
        Json(serde_json::json!({
            "data": {
                "status": status,
                "action": action,
                "integration": integration,
            }
        })),
    )
        .into_response()
}

fn integration_payload(
    integration: Option<&UserIntegrationRecord>,
    provider: &str,
) -> serde_json::Value {
    let Some(integration) = integration else {
        return serde_json::json!({
            "provider": provider,
            "status": "inactive",
            "connected": false,
            "secretLast4": serde_json::Value::Null,
            "connectedAt": serde_json::Value::Null,
            "disconnectedAt": serde_json::Value::Null,
            "metadata": serde_json::json!({}),
        });
    };

    let has_secret = integration
        .encrypted_secret
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .is_some();
    let connected = integration.status == "active" && has_secret;

    serde_json::json!({
        "provider": integration.provider,
        "status": integration.status,
        "connected": connected,
        "secretLast4": integration.secret_last4,
        "connectedAt": integration.connected_at.map(timestamp),
        "disconnectedAt": integration.disconnected_at.map(timestamp),
        "metadata": integration.metadata.clone().unwrap_or_else(|| serde_json::json!({})),
    })
}

fn normalize_optional_email(
    value: Option<String>,
    field: &'static str,
) -> Result<Option<String>, (StatusCode, Json<ApiErrorResponse>)> {
    let Some(value) = value.and_then(non_empty) else {
        return Ok(None);
    };
    if value.chars().count() > 255 || !value.contains('@') {
        return Err(validation_error(
            field,
            &format!("The {field} field must be a valid email address."),
        ));
    }
    Ok(Some(value.to_lowercase()))
}

fn resolve_google_token_expiry(payload: &serde_json::Value) -> Option<chrono::DateTime<Utc>> {
    if let Some(seconds) = payload
        .get("expires_in")
        .and_then(|value| value.as_i64())
        .filter(|value| *value > 0)
    {
        return Some(Utc::now() + Duration::seconds(seconds));
    }

    if let Some(seconds) = payload
        .get("expires_in")
        .and_then(|value| value.as_u64())
        .filter(|value| *value > 0)
    {
        return Some(Utc::now() + Duration::seconds(seconds as i64));
    }

    payload
        .get("expires_at")
        .and_then(serde_json::Value::as_str)
        .and_then(|value| parse_rfc3339_utc(value).ok())
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

#[derive(Debug, Clone, Serialize)]
struct ResendWebhookNormalizedPayload {
    event_id: String,
    provider: String,
    event_type: String,
    delivery_state: String,
    message_id: Option<String>,
    integration_id: Option<String>,
    user_id: Option<String>,
    recipient: Option<String>,
    occurred_at: String,
    reason: Option<String>,
    payload: serde_json::Value,
}

#[derive(Debug, Default)]
struct RuntimeDeliveryForwardResult {
    ok: bool,
    attempts_made: u32,
    status: Option<u16>,
    body: Option<serde_json::Value>,
    error: Option<String>,
}

async fn webhooks_resend_store(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let raw_body = String::from_utf8_lossy(&body).to_string();
    let raw_payload = serde_json::from_slice::<serde_json::Value>(&body)
        .ok()
        .filter(serde_json::Value::is_object)
        .unwrap_or_else(|| serde_json::json!({}));

    let svix_id = header_string(&headers, RESEND_SVIX_ID_HEADER).unwrap_or_default();
    let svix_timestamp = header_string(&headers, RESEND_SVIX_TIMESTAMP_HEADER).unwrap_or_default();
    let svix_signature = header_string(&headers, RESEND_SVIX_SIGNATURE_HEADER).unwrap_or_default();

    let idempotency_key =
        resend_webhook_idempotency_key(RESEND_WEBHOOK_PROVIDER, &svix_id, &raw_body);
    let external_event_id = non_empty(svix_id.clone());
    let signature_valid = verify_resend_webhook_signature(
        &state.config,
        &raw_body,
        &svix_id,
        &svix_timestamp,
        &svix_signature,
    );

    if !signature_valid {
        let invalid_event = match state
            ._domain_store
            .upsert_invalid_webhook_event(RecordWebhookEventInput {
                provider: RESEND_WEBHOOK_PROVIDER.to_string(),
                idempotency_key: idempotency_key.clone(),
                external_event_id: external_event_id.clone(),
                event_type: None,
                delivery_state: None,
                message_id: None,
                integration_id: None,
                user_id: None,
                recipient: None,
                signature_valid: false,
                status: Some("invalid_signature".to_string()),
                normalized_payload: None,
                raw_payload: Some(raw_payload.clone()),
            })
            .await
        {
            Ok(row) => row,
            Err(error) => return map_domain_store_error(error).into_response(),
        };

        return (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({
                "error": {
                    "code": "invalid_signature",
                    "message": "invalid webhook signature",
                },
                "audit": {
                    "event_id": invalid_event.id,
                },
            })),
        )
            .into_response();
    }

    let normalized = normalize_resend_webhook_payload(&raw_payload, external_event_id.as_deref());
    let normalized_value = normalized
        .as_ref()
        .and_then(|payload| serde_json::to_value(payload).ok());
    let normalized_hash = normalized_value
        .as_ref()
        .map(resend_webhook_normalized_hash);
    let status = if normalized.is_some() {
        "received".to_string()
    } else {
        "ignored".to_string()
    };
    let event_type = raw_payload
        .get("type")
        .and_then(serde_json::Value::as_str)
        .and_then(|value| non_empty(value.to_string()));

    let existing = match state
        ._domain_store
        .webhook_event_by_idempotency_key(&idempotency_key)
        .await
    {
        Ok(row) => row,
        Err(error) => return map_domain_store_error(error).into_response(),
    };

    if let Some(existing) = existing {
        if !existing.signature_valid {
            let verified = match state
                ._domain_store
                .mark_webhook_event_verified(MarkWebhookEventVerifiedInput {
                    webhook_event_id: existing.id,
                    status: status.clone(),
                    event_type: event_type.clone(),
                    delivery_state: normalized
                        .as_ref()
                        .map(|value| value.delivery_state.clone()),
                    message_id: normalized
                        .as_ref()
                        .and_then(|value| value.message_id.clone()),
                    integration_id: normalized
                        .as_ref()
                        .and_then(|value| value.integration_id.clone()),
                    user_id: normalized.as_ref().and_then(|value| value.user_id.clone()),
                    recipient: normalized
                        .as_ref()
                        .and_then(|value| value.recipient.clone()),
                    normalized_hash: normalized_hash.clone(),
                    normalized_payload: normalized_value.clone(),
                    raw_payload: Some(raw_payload.clone()),
                })
                .await
            {
                Ok(Some(row)) => row,
                Ok(None) => {
                    return not_found_error("Webhook event not found after verification update.")
                        .into_response();
                }
                Err(error) => return map_domain_store_error(error).into_response(),
            };

            if should_dispatch_resend_webhook(&verified.status, normalized.as_ref()) {
                spawn_resend_webhook_forward_task(state.clone(), verified.id);
            }

            return (
                StatusCode::ACCEPTED,
                Json(serde_json::json!({
                    "data": {
                        "event_id": verified.id,
                        "status": verified.status,
                        "idempotent_replay": false,
                    }
                })),
            )
                .into_response();
        }

        if existing
            .normalized_hash
            .as_deref()
            .zip(normalized_hash.as_deref())
            .map(|(saved, incoming)| saved != incoming)
            .unwrap_or(false)
        {
            return (
                StatusCode::CONFLICT,
                Json(serde_json::json!({
                    "error": {
                        "code": "idempotency_conflict",
                        "message": "webhook idempotency key conflicts with different normalized payload",
                    }
                })),
            )
                .into_response();
        }

        if should_dispatch_resend_webhook(&existing.status, normalized.as_ref()) {
            spawn_resend_webhook_forward_task(state.clone(), existing.id);
        }

        return (
            StatusCode::OK,
            Json(serde_json::json!({
                "data": {
                    "event_id": existing.id,
                    "status": existing.status,
                    "idempotent_replay": true,
                }
            })),
        )
            .into_response();
    }

    let inserted = match state
        ._domain_store
        .record_webhook_event(RecordWebhookEventInput {
            provider: RESEND_WEBHOOK_PROVIDER.to_string(),
            idempotency_key,
            external_event_id,
            event_type,
            delivery_state: normalized
                .as_ref()
                .map(|value| value.delivery_state.clone()),
            message_id: normalized
                .as_ref()
                .and_then(|value| value.message_id.clone()),
            integration_id: normalized
                .as_ref()
                .and_then(|value| value.integration_id.clone()),
            user_id: normalized.as_ref().and_then(|value| value.user_id.clone()),
            recipient: normalized
                .as_ref()
                .and_then(|value| value.recipient.clone()),
            signature_valid: true,
            status: Some(status.clone()),
            normalized_payload: normalized_value,
            raw_payload: Some(raw_payload),
        })
        .await
    {
        Ok(result) => result,
        Err(error) => return map_domain_store_error(error).into_response(),
    };

    if should_dispatch_resend_webhook(&status, normalized.as_ref()) {
        spawn_resend_webhook_forward_task(state.clone(), inserted.event.id);
    }

    (
        StatusCode::ACCEPTED,
        Json(serde_json::json!({
            "data": {
                "event_id": inserted.event.id,
                "status": status,
                "idempotent_replay": false,
            }
        })),
    )
        .into_response()
}

fn should_dispatch_resend_webhook(
    status: &str,
    normalized: Option<&ResendWebhookNormalizedPayload>,
) -> bool {
    normalized.is_some() && matches!(status, "received" | "failed")
}

fn resend_webhook_idempotency_key(provider: &str, svix_id: &str, raw_body: &str) -> String {
    let external_event_id = svix_id.trim();
    if !external_event_id.is_empty() {
        return format!("{provider}:{external_event_id}");
    }
    format!("{provider}:body:{}", sha256_hex(raw_body.as_bytes()))
}

fn resend_webhook_normalized_hash(payload: &serde_json::Value) -> String {
    sha256_hex(payload.to_string().as_bytes())
}

fn verify_resend_webhook_signature(
    config: &Config,
    payload: &str,
    svix_id: &str,
    svix_timestamp: &str,
    svix_signature: &str,
) -> bool {
    let Some(secret) = config
        .resend_webhook_secret
        .as_deref()
        .and_then(|value| non_empty(value.to_string()))
    else {
        return false;
    };

    let svix_id = svix_id.trim();
    let svix_timestamp = svix_timestamp.trim();
    let svix_signature = svix_signature.trim();
    if svix_id.is_empty() || svix_timestamp.is_empty() || svix_signature.is_empty() {
        return false;
    }

    if !svix_timestamp.chars().all(|value| value.is_ascii_digit()) {
        return false;
    }

    let Ok(timestamp) = svix_timestamp.parse::<i64>() else {
        return false;
    };
    let tolerance = config.resend_webhook_tolerance_seconds.max(1) as i64;
    let now = Utc::now().timestamp();
    if (now - timestamp).abs() > tolerance {
        return false;
    }

    let secret_bytes = resolve_resend_webhook_secret_bytes(&secret);
    let signed_content = format!("{svix_id}.{svix_timestamp}.{payload}");
    let Ok(mut mac) = HmacSha256::new_from_slice(secret_bytes.as_slice()) else {
        return false;
    };
    mac.update(signed_content.as_bytes());
    let expected = STANDARD.encode(mac.finalize().into_bytes());

    extract_resend_svix_signatures(svix_signature)
        .into_iter()
        .any(|candidate| !candidate.is_empty() && candidate == expected)
}

fn resolve_resend_webhook_secret_bytes(secret: &str) -> Vec<u8> {
    if let Some(encoded) = secret.strip_prefix("whsec_") {
        if let Ok(decoded) = STANDARD.decode(encoded) {
            if !decoded.is_empty() {
                return decoded;
            }
        }
        return encoded.as_bytes().to_vec();
    }
    secret.as_bytes().to_vec()
}

fn extract_resend_svix_signatures(header: &str) -> Vec<String> {
    let mut signatures = Vec::new();
    for token in header.split_whitespace() {
        let trimmed = token.trim();
        if trimmed.is_empty() {
            continue;
        }

        let (version, value) = if let Some((version, value)) = trimmed.split_once(',') {
            (version, value)
        } else if let Some((version, value)) = trimmed.split_once('=') {
            (version, value)
        } else {
            continue;
        };

        if version.trim() != "v1" {
            continue;
        }
        signatures.push(value.trim().to_string());
    }
    signatures
}

fn normalize_resend_webhook_payload(
    raw_payload: &serde_json::Value,
    external_event_id: Option<&str>,
) -> Option<ResendWebhookNormalizedPayload> {
    let event_type = raw_payload
        .get("type")
        .and_then(serde_json::Value::as_str)
        .and_then(|value| non_empty(value.to_string()))?;

    let delivery_state = match event_type.as_str() {
        "email.delivered" => "delivered",
        "email.bounced" => "bounced",
        "email.complained" => "complained",
        "email.suppressed" | "email.unsubscribed" => "unsubscribed",
        _ => return None,
    }
    .to_string();

    let data = raw_payload
        .get("data")
        .and_then(serde_json::Value::as_object)
        .cloned()
        .unwrap_or_default();
    let tags = normalize_resend_webhook_tags(data.get("tags"));

    let recipient = data
        .get("to")
        .and_then(serde_json::Value::as_array)
        .and_then(|values| values.first())
        .and_then(serde_json::Value::as_str)
        .and_then(|value| non_empty(value.to_string()))
        .or_else(|| {
            data.get("email")
                .and_then(serde_json::Value::as_str)
                .and_then(|value| non_empty(value.to_string()))
        });

    let occurred_at = raw_payload
        .get("created_at")
        .and_then(serde_json::Value::as_str)
        .and_then(|value| non_empty(value.to_string()))
        .or_else(|| {
            data.get("created_at")
                .and_then(serde_json::Value::as_str)
                .and_then(|value| non_empty(value.to_string()))
        })
        .unwrap_or_else(|| timestamp(Utc::now()));

    let reason = data
        .get("reason")
        .and_then(serde_json::Value::as_str)
        .and_then(|value| non_empty(value.to_string()))
        .or_else(|| {
            data.get("bounce")
                .and_then(serde_json::Value::as_object)
                .and_then(|bounce| bounce.get("reason"))
                .and_then(serde_json::Value::as_str)
                .and_then(|value| non_empty(value.to_string()))
        })
        .or_else(|| {
            data.get("suppression")
                .and_then(serde_json::Value::as_object)
                .and_then(|suppression| suppression.get("reason"))
                .and_then(serde_json::Value::as_str)
                .and_then(|value| non_empty(value.to_string()))
        });

    let message_id = data
        .get("email_id")
        .and_then(serde_json::Value::as_str)
        .and_then(|value| non_empty(value.to_string()))
        .or_else(|| {
            data.get("id")
                .and_then(serde_json::Value::as_str)
                .and_then(|value| non_empty(value.to_string()))
        });

    let user_id = tags
        .get("user_id")
        .and_then(|value| non_empty(value.to_string()));
    let integration_id = tags
        .get("integration_id")
        .and_then(|value| non_empty(value.to_string()));

    let event_id = external_event_id
        .and_then(|value| non_empty(value.to_string()))
        .unwrap_or_else(|| format!("resend_{}", sha256_hex(raw_payload.to_string().as_bytes())));

    Some(ResendWebhookNormalizedPayload {
        event_id,
        provider: RESEND_WEBHOOK_PROVIDER.to_string(),
        event_type: event_type.clone(),
        delivery_state,
        message_id,
        integration_id,
        user_id,
        recipient,
        occurred_at,
        reason,
        payload: serde_json::json!({
            "raw_type": event_type,
            "tags": tags,
            "raw": raw_payload,
        }),
    })
}

fn normalize_resend_webhook_tags(
    tags_value: Option<&serde_json::Value>,
) -> BTreeMap<String, String> {
    let mut tags = BTreeMap::new();
    let Some(values) = tags_value.and_then(serde_json::Value::as_array) else {
        return tags;
    };

    for entry in values {
        if let Some(entry_obj) = entry.as_object() {
            let name = entry_obj
                .get("name")
                .and_then(serde_json::Value::as_str)
                .and_then(|value| non_empty(value.to_string()));
            let value = entry_obj.get("value").and_then(|value| match value {
                serde_json::Value::String(value) => non_empty(value.to_string()),
                serde_json::Value::Number(value) => non_empty(value.to_string()),
                serde_json::Value::Bool(value) => Some(value.to_string()),
                _ => None,
            });
            if let (Some(name), Some(value)) = (name, value) {
                tags.insert(name, value);
            }
            continue;
        }

        if let Some(entry_text) = entry.as_str() {
            if let Some((key, value)) = entry_text.split_once(':') {
                if let (Some(key), Some(value)) = (
                    non_empty(key.trim().to_string()),
                    non_empty(value.trim().to_string()),
                ) {
                    tags.insert(key, value);
                }
            }
        }
    }

    tags
}

fn spawn_resend_webhook_forward_task(state: AppState, webhook_event_id: u64) {
    tokio::spawn(async move {
        if let Err(error) = forward_resend_webhook_to_runtime(state, webhook_event_id).await {
            tracing::warn!(webhook_event_id, error = %error, "resend webhook runtime forward failed");
        }
    });
}

async fn forward_resend_webhook_to_runtime(
    state: AppState,
    webhook_event_id: u64,
) -> Result<(), String> {
    let Some(event) = state
        ._domain_store
        .webhook_event_by_id(webhook_event_id)
        .await
        .map_err(|error| format!("failed to read webhook event: {error}"))?
    else {
        return Ok(());
    };

    if !event.signature_valid {
        return Ok(());
    }

    let Some(payload) = event.normalized_payload.clone() else {
        return Ok(());
    };
    if !payload.is_object() {
        state
            ._domain_store
            .mark_webhook_event_forward_failed(
                webhook_event_id,
                Some(0),
                None,
                None,
                Some("runtime_payload_decode_failed".to_string()),
            )
            .await
            .map_err(|error| {
                format!("failed to mark webhook event payload decode failure: {error}")
            })?;
        return Err("runtime payload decode failed".to_string());
    }

    state
        ._domain_store
        .mark_webhook_event_forwarding(webhook_event_id)
        .await
        .map_err(|error| format!("failed to mark webhook event forwarding: {error}"))?;

    let result = runtime_forward_delivery_payload(&state, webhook_event_id, &payload).await;
    if !result.ok {
        state
            ._domain_store
            .mark_webhook_event_forward_failed(
                webhook_event_id,
                Some(result.attempts_made),
                result.status,
                result.body.clone(),
                result
                    .error
                    .clone()
                    .or_else(|| Some("runtime_forward_failed".to_string())),
            )
            .await
            .map_err(|error| format!("failed to mark webhook event forward failure: {error}"))?;
        return Err(result
            .error
            .unwrap_or_else(|| "runtime delivery forwarding failed".to_string()));
    }

    state
        ._domain_store
        .mark_webhook_event_forwarded(
            webhook_event_id,
            Some(result.attempts_made),
            result.status,
            result.body.clone(),
        )
        .await
        .map_err(|error| format!("failed to mark webhook event forwarded: {error}"))?;

    if let Some(user_id) = webhook_projection_user_id(payload.get("user_id")) {
        let provider = payload
            .get("provider")
            .and_then(serde_json::Value::as_str)
            .and_then(|value| non_empty(value.to_string()))
            .unwrap_or_else(|| RESEND_WEBHOOK_PROVIDER.to_string());
        let integration_id = payload
            .get("integration_id")
            .and_then(serde_json::Value::as_str)
            .and_then(|value| non_empty(value.to_string()));
        let occurred_at = payload
            .get("occurred_at")
            .and_then(serde_json::Value::as_str)
            .and_then(|value| parse_rfc3339_utc(value).ok())
            .or_else(|| Some(Utc::now()));
        let projection = state
            ._domain_store
            .upsert_delivery_projection(crate::domain_store::UpsertDeliveryProjectionInput {
                user_id: user_id.clone(),
                provider: provider.clone(),
                integration_id: integration_id.clone(),
                last_state: payload
                    .get("delivery_state")
                    .and_then(serde_json::Value::as_str)
                    .and_then(|value| non_empty(value.to_string())),
                last_event_at: occurred_at,
                last_message_id: payload
                    .get("message_id")
                    .and_then(serde_json::Value::as_str)
                    .and_then(|value| non_empty(value.to_string())),
                last_recipient: payload
                    .get("recipient")
                    .and_then(serde_json::Value::as_str)
                    .and_then(|value| non_empty(value.to_string())),
                runtime_event_id: payload
                    .get("event_id")
                    .and_then(serde_json::Value::as_str)
                    .and_then(|value| non_empty(value.to_string())),
                source: Some("runtime_forwarder".to_string()),
                last_webhook_event_id: Some(webhook_event_id),
            })
            .await
            .map_err(|error| format!("failed to upsert delivery projection: {error}"))?;

        state
            ._domain_store
            .audit_delivery_projection_updated(&user_id, &provider, &projection)
            .await
            .map_err(|error| format!("failed to audit delivery projection update: {error}"))?;
    }

    Ok(())
}

fn webhook_projection_user_id(value: Option<&serde_json::Value>) -> Option<String> {
    let value = value?;
    match value {
        serde_json::Value::String(value) => non_empty(value.to_string()),
        serde_json::Value::Number(value) => value.as_u64().and_then(|value| {
            if value > 0 {
                Some(value.to_string())
            } else {
                None
            }
        }),
        _ => None,
    }
}

async fn runtime_forward_delivery_payload(
    state: &AppState,
    webhook_event_id: u64,
    payload: &serde_json::Value,
) -> RuntimeDeliveryForwardResult {
    let Some(base_url) = state
        .config
        .runtime_base_url
        .as_deref()
        .and_then(|value| non_empty(value.to_string()))
    else {
        return RuntimeDeliveryForwardResult {
            ok: false,
            attempts_made: 0,
            status: None,
            body: None,
            error: Some("runtime_forward_misconfigured".to_string()),
        };
    };
    let Some(signing_key) = state
        .config
        .runtime_signing_key
        .as_deref()
        .and_then(|value| non_empty(value.to_string()))
    else {
        return RuntimeDeliveryForwardResult {
            ok: false,
            attempts_made: 0,
            status: None,
            body: None,
            error: Some("runtime_forward_misconfigured".to_string()),
        };
    };

    let ingest_path = normalize_route_path(&state.config.runtime_comms_delivery_ingest_path);
    let url = format!("{}{}", base_url.trim_end_matches('/'), ingest_path);
    let attempts = state
        .config
        .runtime_comms_delivery_max_retries
        .saturating_add(1)
        .max(1);
    let backoff_ms = state.config.runtime_comms_delivery_retry_backoff_ms;
    let timeout_ms = state.config.runtime_comms_delivery_timeout_ms.max(500);
    let timeout = std::time::Duration::from_millis(timeout_ms);
    let payload_bytes = serde_json::to_vec(payload).unwrap_or_else(|_| b"{}".to_vec());
    let payload_hash = sha256_hex(&payload_bytes);

    let mut last_status = None;
    let mut last_body = None;
    let mut last_error: Option<String> = None;
    let mut attempts_made = 0u32;

    for attempt in 1..=attempts {
        attempts_made = attempt as u32;
        let signature = match runtime_forward_signature_token(
            signing_key.as_str(),
            state.config.runtime_signature_ttl_seconds.max(1),
        ) {
            Ok(value) => value,
            Err(error) => {
                return RuntimeDeliveryForwardResult {
                    ok: false,
                    attempts_made,
                    status: None,
                    body: None,
                    error: Some(format!("runtime_signature_failed:{error}")),
                };
            }
        };

        let response = reqwest::Client::new()
            .post(url.as_str())
            .header("x-oa-runtime-signature", signature)
            .header("x-oa-runtime-body-sha256", payload_hash.as_str())
            .header(
                "x-oa-runtime-key-id",
                state.config.runtime_signing_key_id.as_str(),
            )
            .header("x-request-id", format!("req_{}", Uuid::new_v4().simple()))
            .header("content-type", "application/json")
            .timeout(timeout)
            .json(payload)
            .send()
            .await;

        match response {
            Ok(response) => {
                let status = response.status();
                let status_code = status.as_u16();
                let body = match response.bytes().await {
                    Ok(bytes) => serde_json::from_slice::<serde_json::Value>(&bytes)
                        .ok()
                        .or_else(|| {
                            non_empty(String::from_utf8_lossy(&bytes).to_string())
                                .map(serde_json::Value::String)
                        }),
                    Err(_) => None,
                };

                last_status = Some(status_code);
                last_body = body.clone();
                if status.is_success() {
                    return RuntimeDeliveryForwardResult {
                        ok: true,
                        attempts_made,
                        status: Some(status_code),
                        body,
                        error: None,
                    };
                }

                last_error = Some(format!("runtime_http_{status_code}"));
            }
            Err(error) => {
                last_error = Some(error.to_string());
            }
        }

        if attempt < attempts {
            if let Err(error) = state
                ._domain_store
                .mark_webhook_event_retrying(
                    webhook_event_id,
                    attempts_made,
                    last_status,
                    last_body.clone(),
                    last_error.clone(),
                )
                .await
            {
                tracing::warn!(
                    webhook_event_id,
                    attempt = attempts_made,
                    error = %error,
                    "failed to persist webhook retry transition"
                );
            }

            if backoff_ms > 0 {
                tokio::time::sleep(std::time::Duration::from_millis(backoff_ms)).await;
            }
        }
    }

    RuntimeDeliveryForwardResult {
        ok: false,
        attempts_made,
        status: last_status,
        body: last_body,
        error: last_error.or_else(|| Some("runtime_forward_failed".to_string())),
    }
}

fn runtime_forward_signature_token(
    secret: &str,
    ttl_seconds: u64,
) -> Result<String, anyhow::Error> {
    let now = Utc::now().timestamp().max(0) as u64;
    let payload = serde_json::json!({
        "iat": now,
        "exp": now + ttl_seconds.max(1),
        "nonce": format!("nonce-{}", Uuid::new_v4().simple()),
    });
    let payload_bytes = serde_json::to_vec(&payload)?;
    let payload_segment = URL_SAFE_NO_PAD.encode(payload_bytes);

    let mut mac = HmacSha256::new_from_slice(secret.as_bytes())?;
    mac.update(payload_segment.as_bytes());
    let signature = mac.finalize().into_bytes();
    let signature_segment = URL_SAFE_NO_PAD.encode(signature);

    Ok(format!("v1.{payload_segment}.{signature_segment}"))
}

async fn shouts_index(
    State(state): State<AppState>,
    Query(query): Query<ShoutsIndexQuery>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiErrorResponse>)> {
    let zone = normalize_shout_zone(query.zone.as_deref(), "zone")?;
    let limit = parse_shout_limit(query.limit.as_deref(), "limit", 50, 200)?;
    let before_id = parse_shout_optional_u64(query.before_id.as_deref(), "before_id")?;
    let since = query
        .since
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| {
            parse_rfc3339_utc(value)
                .map_err(|_| validation_error("since", "The since field must be a valid date."))
        })
        .transpose()?;

    let rows = state
        ._domain_store
        .list_shouts(zone.as_deref(), limit, before_id, since)
        .await
        .map_err(map_domain_store_error)?;

    let mut author_cache: HashMap<String, serde_json::Value> = HashMap::new();
    let mut data = Vec::with_capacity(rows.len());
    for shout in &rows {
        data.push(shout_payload(&state, shout, &mut author_cache).await);
    }

    let next_cursor = if rows.len() == limit {
        rows.last().map(|row| row.id.to_string())
    } else {
        None
    };

    Ok((
        StatusCode::OK,
        Json(serde_json::json!({
            "data": data,
            "meta": {
                "nextCursor": next_cursor,
            }
        })),
    ))
}

const COMPUTE_DEFAULT_CAPABILITY: &str = "oa.sandbox_run.v1";
const COMPUTE_DASHBOARD_TIMEOUT_MS: u64 = 1_500;

async fn feed_page(
    State(state): State<AppState>,
    headers: HeaderMap,
    uri: axum::http::Uri,
    Query(query): Query<FeedPageQuery>,
) -> Result<Response, (StatusCode, Json<ApiErrorResponse>)> {
    let zone = normalize_shout_zone(query.zone.as_deref(), "zone").and_then(|zone| match zone {
        Some(value) if value == "all" => Ok(None),
        _ => Ok(zone),
    })?;
    let limit = parse_shout_limit(query.limit.as_deref(), "limit", 50, 200)?;
    let before_id = parse_shout_optional_u64(query.before_id.as_deref(), "before_id")?;
    let since = parse_feed_since(query.since.as_deref())?;

    let rows = state
        ._domain_store
        .list_shouts(zone.as_deref(), limit, before_id, since)
        .await
        .map_err(map_domain_store_error)?;
    let zones = state
        ._domain_store
        .top_shout_zones(20)
        .await
        .map_err(map_domain_store_error)?;

    let status = query_param_value(uri.query(), "status");
    let next_cursor = if rows.len() == limit {
        rows.last().map(|row| row.id.to_string())
    } else {
        None
    };
    let since_query = query
        .since
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string);
    let items = feed_items_for_web(&state, &rows).await;
    let zone_views = feed_zones_for_web(&zones, zone.as_deref());
    let session = session_bundle_from_headers(&state, &headers)
        .await
        .ok()
        .map(|bundle| SessionView {
            display_name: bundle.user.name.clone(),
            email: bundle.user.email.clone(),
        });

    let page = WebPage {
        title: format!("Feed ({limit})"),
        path: "/feed".to_string(),
        session,
        body: WebBody::Feed {
            status,
            items,
            zones: zone_views,
            next_cursor,
            current_zone: zone,
            page_limit: limit as u64,
            since: since_query,
        },
    };

    Ok(web_response_for_page(&state, &headers, &uri, page).await)
}

async fn feed_main_fragment(
    State(state): State<AppState>,
    headers: HeaderMap,
    uri: axum::http::Uri,
    Query(query): Query<FeedPageQuery>,
) -> Result<Response, (StatusCode, Json<ApiErrorResponse>)> {
    let htmx = classify_htmx_request(&headers);
    let htmx_mode = state.route_split.htmx_mode_for_path("/feed").await;
    if !htmx.is_hx_request || htmx_mode.mode == HtmxModeTarget::FullPage {
        let suffix = uri
            .query()
            .map(|query| format!("?{query}"))
            .unwrap_or_default();
        return Ok(Redirect::temporary(&format!("/feed{suffix}")).into_response());
    }

    let zone = normalize_shout_zone(query.zone.as_deref(), "zone").and_then(|zone| match zone {
        Some(value) if value == "all" => Ok(None),
        _ => Ok(zone),
    })?;
    let limit = parse_shout_limit(query.limit.as_deref(), "limit", 50, 200)?;
    let before_id = parse_shout_optional_u64(query.before_id.as_deref(), "before_id")?;
    let since = parse_feed_since(query.since.as_deref())?;
    let rows = state
        ._domain_store
        .list_shouts(zone.as_deref(), limit, before_id, since)
        .await
        .map_err(map_domain_store_error)?;
    let zones = state
        ._domain_store
        .top_shout_zones(20)
        .await
        .map_err(map_domain_store_error)?;
    let status = query_param_value(uri.query(), "status");
    let next_cursor = if rows.len() == limit {
        rows.last().map(|row| row.id.to_string())
    } else {
        None
    };
    let since_query = query
        .since
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string);
    let items = feed_items_for_web(&state, &rows).await;
    let zone_views = feed_zones_for_web(&zones, zone.as_deref());
    let session = session_bundle_from_headers(&state, &headers)
        .await
        .ok()
        .map(|bundle| SessionView {
            display_name: bundle.user.name.clone(),
            email: bundle.user.email.clone(),
        });

    Ok(feed_main_select_fragment_response(
        session.as_ref(),
        status.as_deref(),
        &items,
        &zone_views,
        next_cursor.as_deref(),
        zone.as_deref(),
        limit as u64,
        since_query.as_deref(),
    ))
}

async fn feed_items_fragment(
    State(state): State<AppState>,
    headers: HeaderMap,
    uri: axum::http::Uri,
    Query(query): Query<FeedPageQuery>,
) -> Result<Response, (StatusCode, Json<ApiErrorResponse>)> {
    let htmx = classify_htmx_request(&headers);
    let htmx_mode = state.route_split.htmx_mode_for_path("/feed").await;
    if !htmx.is_hx_request || htmx_mode.mode == HtmxModeTarget::FullPage {
        let suffix = uri
            .query()
            .map(|query| format!("?{query}"))
            .unwrap_or_default();
        return Ok(Redirect::temporary(&format!("/feed{suffix}")).into_response());
    }

    let zone = normalize_shout_zone(query.zone.as_deref(), "zone").and_then(|zone| match zone {
        Some(value) if value == "all" => Ok(None),
        _ => Ok(zone),
    })?;
    let limit = parse_shout_limit(query.limit.as_deref(), "limit", 50, 200)?;
    let before_id = parse_shout_optional_u64(query.before_id.as_deref(), "before_id")?;
    let since = parse_feed_since(query.since.as_deref())?;
    let rows = state
        ._domain_store
        .list_shouts(zone.as_deref(), limit, before_id, since)
        .await
        .map_err(map_domain_store_error)?;
    let next_cursor = if rows.len() == limit {
        rows.last().map(|row| row.id.to_string())
    } else {
        None
    };
    let since_query = query
        .since
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string);
    let items = feed_items_for_web(&state, &rows).await;

    Ok(feed_items_append_fragment_response(
        &items,
        next_cursor.as_deref(),
        zone.as_deref(),
        limit as u64,
        since_query.as_deref(),
    ))
}

async fn shouts_zones(
    State(state): State<AppState>,
    Query(query): Query<ShoutsZonesQuery>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiErrorResponse>)> {
    let limit = parse_shout_limit(query.limit.as_deref(), "limit", 20, 100)?;
    let zones = state
        ._domain_store
        .top_shout_zones(limit)
        .await
        .map_err(map_domain_store_error)?;

    Ok((
        StatusCode::OK,
        Json(serde_json::json!({
            "data": zones,
        })),
    ))
}

async fn shouts_store(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<ShoutStoreRequestPayload>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiErrorResponse>)> {
    let request_id = request_id(&headers);
    let bundle = session_bundle_from_headers(&state, &headers).await?;

    let body_raw = payload
        .body
        .or(payload.text)
        .and_then(non_empty)
        .ok_or_else(|| validation_error("body", "The body field is required."))?;
    if body_raw.chars().count() > 2000 {
        return Err(validation_error(
            "body",
            "The body field may not be greater than 2000 characters.",
        ));
    }
    let zone = normalize_shout_zone(payload.zone.as_deref(), "zone")?;

    let shout = state
        ._domain_store
        .create_shout(CreateShoutInput {
            user_id: bundle.user.id.clone(),
            zone,
            body: body_raw,
        })
        .await
        .map_err(map_domain_store_error)?;

    let mut author_cache = HashMap::new();
    let data = shout_payload(&state, &shout, &mut author_cache).await;

    state.observability.audit(
        AuditEvent::new("shouts.created", request_id.clone())
            .with_user_id(bundle.user.id)
            .with_session_id(bundle.session.session_id)
            .with_org_id(bundle.session.active_org_id)
            .with_device_id(bundle.session.device_id)
            .with_attribute("shout_id", shout.id.to_string())
            .with_attribute(
                "zone",
                shout.zone.clone().unwrap_or_else(|| "none".to_string()),
            )
            .with_attribute("body_length", shout.body.chars().count().to_string()),
    );
    state
        .observability
        .increment_counter("shouts.created", &request_id);

    Ok((
        StatusCode::CREATED,
        Json(serde_json::json!({
            "data": data,
        })),
    ))
}

async fn whispers_index(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<WhispersIndexQuery>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiErrorResponse>)> {
    let request_id = request_id(&headers);
    let bundle = session_bundle_from_headers(&state, &headers).await?;
    let with = normalize_whisper_lookup(query.with.as_deref(), "with")?;
    let with_user = if let Some(lookup) = with.as_deref() {
        Some(
            state
                .auth
                .user_by_id_or_handle(lookup)
                .await
                .ok_or_else(|| not_found_error("Not found."))?,
        )
    } else {
        None
    };
    let limit = parse_shout_limit(query.limit.as_deref(), "limit", 50, 200)?;
    let before_id = parse_shout_optional_u64(query.before_id.as_deref(), "before_id")?;

    let rows = state
        ._domain_store
        .list_whispers_for(
            &bundle.user.id,
            with_user.as_ref().map(|user| user.id.as_str()),
            limit,
            before_id,
        )
        .await
        .map_err(map_domain_store_error)?;

    let mut user_cache: HashMap<String, serde_json::Value> = HashMap::new();
    let mut data = Vec::with_capacity(rows.len());
    for whisper in &rows {
        data.push(whisper_payload(&state, whisper, &mut user_cache).await);
    }

    let next_cursor = if rows.len() == limit {
        rows.last().map(|row| row.id.to_string())
    } else {
        None
    };

    let mut audit = AuditEvent::new("whispers.list_viewed", request_id.clone())
        .with_user_id(bundle.user.id)
        .with_session_id(bundle.session.session_id)
        .with_org_id(bundle.session.active_org_id)
        .with_device_id(bundle.session.device_id)
        .with_attribute("limit", limit.to_string())
        .with_attribute("returned_count", rows.len().to_string());
    if let Some(with_user) = with_user.as_ref() {
        audit = audit
            .with_attribute("with_user_id", with_user.id.clone())
            .with_attribute("with_user_handle", user_handle_from_email(&with_user.email));
    }
    state.observability.audit(audit);
    state
        .observability
        .increment_counter("whispers.list_viewed", &request_id);

    Ok((
        StatusCode::OK,
        Json(serde_json::json!({
            "data": data,
            "meta": {
                "nextCursor": next_cursor,
                "with": with_user
                    .as_ref()
                    .map(|user| user_handle_from_email(&user.email)),
            }
        })),
    ))
}

async fn whispers_store(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<WhisperStoreRequestPayload>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiErrorResponse>)> {
    let request_id = request_id(&headers);
    let bundle = session_bundle_from_headers(&state, &headers).await?;
    let recipient = resolve_whisper_recipient(&state, &payload).await?;
    let body = payload
        .body
        .and_then(non_empty)
        .ok_or_else(|| validation_error("body", "The body field is required."))?;
    if body.chars().count() > 5000 {
        return Err(validation_error(
            "body",
            "The body field may not be greater than 5000 characters.",
        ));
    }
    if recipient.id == bundle.user.id {
        return Err(validation_error("recipientId", "Cannot whisper yourself."));
    }

    let whisper = state
        ._domain_store
        .send_whisper(SendWhisperInput {
            sender_id: bundle.user.id.clone(),
            recipient_id: recipient.id.clone(),
            body,
        })
        .await
        .map_err(map_domain_store_error)?;

    let mut user_cache = HashMap::new();
    let data = whisper_payload(&state, &whisper, &mut user_cache).await;

    state.observability.audit(
        AuditEvent::new("whispers.sent", request_id.clone())
            .with_user_id(bundle.user.id)
            .with_session_id(bundle.session.session_id)
            .with_org_id(bundle.session.active_org_id)
            .with_device_id(bundle.session.device_id)
            .with_attribute("whisper_id", whisper.id.to_string())
            .with_attribute("recipient_id", whisper.recipient_id.clone())
            .with_attribute("recipient_handle", user_handle_from_email(&recipient.email))
            .with_attribute("body_length", whisper.body.chars().count().to_string()),
    );
    state
        .observability
        .increment_counter("whispers.sent", &request_id);

    Ok((
        StatusCode::CREATED,
        Json(serde_json::json!({
            "data": data,
        })),
    ))
}

async fn whispers_read(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiErrorResponse>)> {
    let request_id = request_id(&headers);
    let bundle = session_bundle_from_headers(&state, &headers).await?;
    let whisper_id = id
        .trim()
        .parse::<u64>()
        .map_err(|_| not_found_error("Not found."))?;

    let whisper = state
        ._domain_store
        .whisper_by_id(whisper_id)
        .await
        .map_err(map_domain_store_error)?
        .ok_or_else(|| not_found_error("Not found."))?;
    if whisper.recipient_id != bundle.user.id {
        return Err(forbidden_error("Forbidden."));
    }

    let updated = state
        ._domain_store
        .mark_whisper_read(whisper_id, &bundle.user.id)
        .await
        .map_err(map_domain_store_error)?;

    let mut user_cache = HashMap::new();
    let data = whisper_payload(&state, &updated, &mut user_cache).await;

    state.observability.audit(
        AuditEvent::new("whispers.marked_read", request_id.clone())
            .with_user_id(bundle.user.id)
            .with_session_id(bundle.session.session_id)
            .with_org_id(bundle.session.active_org_id)
            .with_device_id(bundle.session.device_id)
            .with_attribute("whisper_id", updated.id.to_string())
            .with_attribute("sender_id", updated.sender_id.clone())
            .with_attribute("recipient_id", updated.recipient_id.clone()),
    );
    state
        .observability
        .increment_counter("whispers.marked_read", &request_id);

    Ok((StatusCode::OK, Json(serde_json::json!({ "data": data }))))
}

fn normalize_whisper_lookup(
    raw: Option<&str>,
    field: &'static str,
) -> Result<Option<String>, (StatusCode, Json<ApiErrorResponse>)> {
    let Some(raw) = raw.map(str::trim) else {
        return Ok(None);
    };
    if raw.is_empty() {
        return Ok(None);
    }

    normalize_whisper_identifier(raw, field, true).map(Some)
}

fn normalize_whisper_identifier(
    raw: &str,
    field: &'static str,
    lowercase: bool,
) -> Result<String, (StatusCode, Json<ApiErrorResponse>)> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(validation_error(
            field,
            &format!("The {field} field is required."),
        ));
    }
    if trimmed.chars().count() > 64 {
        return Err(validation_error(
            field,
            &format!("The {field} field may not be greater than 64 characters."),
        ));
    }

    let normalized = if lowercase {
        trimmed.to_ascii_lowercase()
    } else {
        trimmed.to_string()
    };
    let valid = normalized
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || matches!(character, ':' | '_' | '-'));
    if !valid {
        return Err(validation_error(
            field,
            &format!("The {field} field format is invalid."),
        ));
    }
    Ok(normalized)
}

async fn resolve_whisper_recipient(
    state: &AppState,
    payload: &WhisperStoreRequestPayload,
) -> Result<AuthUser, (StatusCode, Json<ApiErrorResponse>)> {
    let recipient_id = payload.recipient_id.clone().and_then(non_empty);
    let recipient_handle = payload.recipient_handle.clone().and_then(non_empty);

    if recipient_id.is_some() && recipient_handle.is_some() {
        return Err(validation_error(
            "recipientId",
            "The recipientId field prohibits recipientHandle.",
        ));
    }

    let lookup = if let Some(recipient_id) = recipient_id {
        normalize_whisper_identifier(&recipient_id, "recipientId", false)?
    } else if let Some(recipient_handle) = recipient_handle {
        normalize_whisper_identifier(&recipient_handle, "recipientHandle", true)?
    } else {
        return Err(validation_error(
            "recipientId",
            "The recipientId field is required when recipientHandle is not present.",
        ));
    };

    state
        .auth
        .user_by_id_or_handle(&lookup)
        .await
        .ok_or_else(|| not_found_error("Not found."))
}

fn parse_shout_limit(
    raw: Option<&str>,
    field: &'static str,
    default: usize,
    max: usize,
) -> Result<usize, (StatusCode, Json<ApiErrorResponse>)> {
    let Some(raw) = raw.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(default);
    };
    let parsed = raw
        .parse::<usize>()
        .map_err(|_| validation_error(field, &format!("The {field} field must be an integer.")))?;
    if parsed < 1 {
        return Err(validation_error(
            field,
            &format!("The {field} field must be at least 1."),
        ));
    }
    Ok(parsed.min(max))
}

fn parse_feed_since(
    raw: Option<&str>,
) -> Result<Option<chrono::DateTime<Utc>>, (StatusCode, Json<ApiErrorResponse>)> {
    let Some(raw) = raw.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    if let Ok(parsed) = parse_rfc3339_utc(raw) {
        return Ok(Some(parsed));
    }
    if let Ok(parsed_date) = NaiveDate::parse_from_str(raw, "%Y-%m-%d") {
        if let Some(parsed_datetime) = parsed_date.and_hms_opt(0, 0, 0) {
            return Ok(Some(parsed_datetime.and_utc()));
        }
    }

    Err(validation_error(
        "since",
        "The since field must be a valid date.",
    ))
}

fn parse_shout_optional_u64(
    raw: Option<&str>,
    field: &'static str,
) -> Result<Option<u64>, (StatusCode, Json<ApiErrorResponse>)> {
    let Some(raw) = raw.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    let parsed = raw
        .parse::<u64>()
        .map_err(|_| validation_error(field, &format!("The {field} field must be an integer.")))?;
    if parsed < 1 {
        return Err(validation_error(
            field,
            &format!("The {field} field must be at least 1."),
        ));
    }
    Ok(Some(parsed))
}

fn normalize_shout_zone(
    raw: Option<&str>,
    field: &'static str,
) -> Result<Option<String>, (StatusCode, Json<ApiErrorResponse>)> {
    let Some(raw) = raw.map(str::trim) else {
        return Ok(None);
    };
    if raw.is_empty() {
        return Ok(None);
    }
    if raw.chars().count() > 64 {
        return Err(validation_error(
            field,
            &format!("The {field} field may not be greater than 64 characters."),
        ));
    }

    let normalized = raw.to_ascii_lowercase();
    let valid = normalized
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || matches!(character, ':' | '_' | '-'));
    if !valid {
        return Err(validation_error(
            field,
            &format!("The {field} field format is invalid."),
        ));
    }

    Ok(Some(normalized))
}

async fn shout_payload(
    state: &AppState,
    shout: &ShoutRecord,
    author_cache: &mut HashMap<String, serde_json::Value>,
) -> serde_json::Value {
    let author = if let Some(cached) = author_cache.get(&shout.user_id).cloned() {
        cached
    } else {
        let resolved = if let Some(user) = state.auth.user_by_id(&shout.user_id).await {
            serde_json::json!({
                "id": user.id,
                "name": user.name,
                "handle": user_handle_from_email(&user.email),
                "avatar": serde_json::Value::Null,
            })
        } else {
            serde_json::json!({
                "id": shout.user_id.clone(),
                "name": "Unknown",
                "handle": "user",
                "avatar": serde_json::Value::Null,
            })
        };
        author_cache.insert(shout.user_id.clone(), resolved.clone());
        resolved
    };

    serde_json::json!({
        "id": shout.id,
        "zone": shout.zone.clone(),
        "body": shout.body.clone(),
        "visibility": shout.visibility.clone(),
        "author": author,
        "createdAt": timestamp(shout.created_at),
        "updatedAt": timestamp(shout.updated_at),
    })
}

async fn feed_items_for_web(state: &AppState, rows: &[ShoutRecord]) -> Vec<FeedItemView> {
    let mut author_handle_cache: HashMap<String, String> = HashMap::new();
    let mut items = Vec::with_capacity(rows.len());

    for row in rows {
        let author_handle = if let Some(cached) = author_handle_cache.get(&row.user_id) {
            cached.clone()
        } else {
            let resolved = if let Some(user) = state.auth.user_by_id(&row.user_id).await {
                user_handle_from_email(&user.email)
            } else {
                "user".to_string()
            };
            author_handle_cache.insert(row.user_id.clone(), resolved.clone());
            resolved
        };

        let zone = row
            .zone
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("global")
            .to_string();

        items.push(FeedItemView {
            id: row.id.to_string(),
            zone,
            author_handle,
            body: row.body.clone(),
            created_at: timestamp(row.created_at),
        });
    }

    items
}

fn feed_zones_for_web(zones: &[ZoneCount], active_zone: Option<&str>) -> Vec<FeedZoneView> {
    zones
        .iter()
        .map(|zone| FeedZoneView {
            zone: zone.zone.clone(),
            count_24h: zone.count24h,
            is_active: active_zone
                .map(|active| active.eq_ignore_ascii_case(&zone.zone))
                .unwrap_or(false),
        })
        .collect()
}

async fn whisper_user_payload(
    state: &AppState,
    user_id: &str,
    user_cache: &mut HashMap<String, serde_json::Value>,
) -> serde_json::Value {
    if let Some(cached) = user_cache.get(user_id).cloned() {
        return cached;
    }

    let resolved = if let Some(user) = state.auth.user_by_id(user_id).await {
        serde_json::json!({
            "id": user.id,
            "name": user.name,
            "handle": user_handle_from_email(&user.email),
            "avatar": serde_json::Value::Null,
        })
    } else {
        serde_json::json!({
            "id": user_id,
            "name": "Unknown",
            "handle": "user",
            "avatar": serde_json::Value::Null,
        })
    };
    user_cache.insert(user_id.to_string(), resolved.clone());
    resolved
}

async fn whisper_payload(
    state: &AppState,
    whisper: &WhisperRecord,
    user_cache: &mut HashMap<String, serde_json::Value>,
) -> serde_json::Value {
    let sender = whisper_user_payload(state, &whisper.sender_id, user_cache).await;
    let recipient = whisper_user_payload(state, &whisper.recipient_id, user_cache).await;

    serde_json::json!({
        "id": whisper.id,
        "body": whisper.body.clone(),
        "sender": sender,
        "recipient": recipient,
        "readAt": whisper.read_at.map(timestamp),
        "createdAt": timestamp(whisper.created_at),
        "updatedAt": timestamp(whisper.updated_at),
    })
}

async fn runtime_internal_secret_fetch(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<serde_json::Value>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiErrorResponse>)> {
    let request_id = request_id(&headers);
    let (lookup_user_id, scope_user_id) = parse_runtime_internal_user_id(payload.get("user_id"))?;
    let provider = normalize_optional_bounded_string(
        normalized_json_string(payload.get("provider")),
        "provider",
        32,
    )?
    .ok_or_else(|| validation_error("provider", "The provider field is required."))?
    .to_lowercase();
    if !matches!(provider.as_str(), "resend" | "google") {
        return Err(validation_error(
            "provider",
            "The selected provider is invalid.",
        ));
    }
    let integration_id = normalize_optional_bounded_string(
        normalized_json_string(payload.get("integration_id")),
        "integration_id",
        160,
    )?
    .ok_or_else(|| validation_error("integration_id", "The integration_id field is required."))?;
    let run_id = normalize_optional_bounded_string(
        normalized_json_string(payload.get("run_id")),
        "run_id",
        160,
    )?
    .ok_or_else(|| validation_error("run_id", "The run_id field is required."))?;
    let tool_call_id = normalize_optional_bounded_string(
        normalized_json_string(payload.get("tool_call_id")),
        "tool_call_id",
        160,
    )?
    .ok_or_else(|| validation_error("tool_call_id", "The tool_call_id field is required."))?;
    let org_id = normalize_optional_bounded_string(
        normalized_json_string(payload.get("org_id")),
        "org_id",
        160,
    )?;

    let integration = state
        ._domain_store
        .find_active_integration_secret(&lookup_user_id, &provider)
        .await
        .map_err(map_domain_store_error)?;
    let Some(integration) = integration else {
        let mut audit = AuditEvent::new(
            "runtime.internal.secret.fetch.not_found",
            request_id.clone(),
        )
        .with_attribute("user_id", lookup_user_id)
        .with_attribute("provider", provider.clone())
        .with_attribute("integration_id", integration_id.clone())
        .with_attribute("run_id", run_id.clone())
        .with_attribute("tool_call_id", tool_call_id.clone());
        if let Some(org_id) = org_id.clone() {
            audit = audit.with_attribute("org_id", org_id);
        }
        state.observability.audit(audit);
        state
            .observability
            .increment_counter("runtime.internal.secret.fetch.not_found", &request_id);
        return Err(runtime_internal_error(
            StatusCode::NOT_FOUND,
            "secret_not_found",
            "active provider secret not found",
        ));
    };

    let secret = integration
        .encrypted_secret
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .ok_or_else(|| {
            runtime_internal_error(
                StatusCode::NOT_FOUND,
                "secret_not_found",
                "active provider secret not found",
            )
        })?;

    let mut audit = AuditEvent::new("runtime.internal.secret.fetch.accepted", request_id.clone())
        .with_attribute("user_id", integration.user_id.clone())
        .with_attribute("provider", integration.provider.clone())
        .with_attribute("integration_id", integration_id.clone())
        .with_attribute("run_id", run_id.clone())
        .with_attribute("tool_call_id", tool_call_id.clone());
    if let Some(org_id) = org_id.clone() {
        audit = audit.with_attribute("org_id", org_id);
    }
    state.observability.audit(audit);
    state
        .observability
        .increment_counter("runtime.internal.secret.fetch.accepted", &request_id);

    Ok(ok_data(serde_json::json!({
        "provider": integration.provider,
        "secret": secret,
        "cache_ttl_ms": state.config.runtime_internal_secret_cache_ttl_ms,
        "scope": {
            "user_id": scope_user_id,
            "provider": provider,
            "integration_id": integration_id,
            "run_id": run_id,
            "tool_call_id": tool_call_id,
            "org_id": org_id,
        },
        "fetched_at": timestamp(Utc::now()),
    })))
}

async fn agent_payments_wallet(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiErrorResponse>)> {
    let request_id = request_id(&headers);
    let bundle = session_bundle_from_headers(&state, &headers).await?;
    let wallet = state
        ._domain_store
        .find_user_spark_wallet(&bundle.user.id)
        .await
        .map_err(map_domain_store_error)?;

    let Some(wallet) = wallet else {
        state.observability.audit(
            AuditEvent::new("agent_payments.wallet_missing", request_id.clone())
                .with_user_id(bundle.user.id)
                .with_session_id(bundle.session.session_id)
                .with_org_id(bundle.session.active_org_id)
                .with_device_id(bundle.session.device_id),
        );
        state
            .observability
            .increment_counter("agent_payments.wallet_missing", &request_id);
        return Err(not_found_error("wallet_not_found"));
    };

    state.observability.audit(
        AuditEvent::new("agent_payments.wallet_viewed", request_id.clone())
            .with_user_id(bundle.user.id)
            .with_session_id(bundle.session.session_id)
            .with_org_id(bundle.session.active_org_id)
            .with_device_id(bundle.session.device_id)
            .with_attribute("wallet_id", wallet.wallet_id.clone())
            .with_attribute("status", wallet.status.clone()),
    );
    state
        .observability
        .increment_counter("agent_payments.wallet_viewed", &request_id);

    Ok(ok_data(serde_json::json!({
        "wallet": agent_payments_wallet_payload(&wallet)
    })))
}

async fn agent_payments_upsert_wallet(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<AgentPaymentsWalletUpsertRequestPayload>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiErrorResponse>)> {
    let request_id = request_id(&headers);
    let bundle = session_bundle_from_headers(&state, &headers).await?;
    let mnemonic = payload.mnemonic.and_then(non_empty);
    if let Some(candidate) = mnemonic.as_deref() {
        if candidate.chars().count() < 20 {
            return Err(validation_error(
                "mnemonic",
                "The mnemonic field must be at least 20 characters.",
            ));
        }
    }

    let action = if mnemonic.is_some() {
        "imported"
    } else {
        "ensured"
    };
    let wallet = upsert_agent_wallet_for_user(&state, &bundle.user.id, mnemonic).await?;

    state.observability.audit(
        AuditEvent::new("agent_payments.wallet_upserted", request_id.clone())
            .with_user_id(bundle.user.id)
            .with_session_id(bundle.session.session_id)
            .with_org_id(bundle.session.active_org_id)
            .with_device_id(bundle.session.device_id)
            .with_attribute("action", action.to_string())
            .with_attribute("wallet_id", wallet.wallet_id.clone())
            .with_attribute("status", wallet.status.clone()),
    );
    state
        .observability
        .increment_counter("agent_payments.wallet_upserted", &request_id);

    Ok(ok_data(serde_json::json!({
        "wallet": agent_payments_wallet_payload(&wallet),
        "action": action,
    })))
}

async fn agent_payments_balance(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiErrorResponse>)> {
    let request_id = request_id(&headers);
    let bundle = session_bundle_from_headers(&state, &headers).await?;
    let existing = state
        ._domain_store
        .find_user_spark_wallet(&bundle.user.id)
        .await
        .map_err(map_domain_store_error)?;
    let Some(existing) = existing else {
        state.observability.audit(
            AuditEvent::new("agent_payments.balance_wallet_missing", request_id.clone())
                .with_user_id(bundle.user.id)
                .with_session_id(bundle.session.session_id)
                .with_org_id(bundle.session.active_org_id)
                .with_device_id(bundle.session.device_id),
        );
        state
            .observability
            .increment_counter("agent_payments.balance_wallet_missing", &request_id);
        return Err(not_found_error("wallet_not_found"));
    };

    let wallet = sync_agent_wallet(&state, existing).await?;
    state.observability.audit(
        AuditEvent::new("agent_payments.balance_viewed", request_id.clone())
            .with_user_id(bundle.user.id)
            .with_session_id(bundle.session.session_id)
            .with_org_id(bundle.session.active_org_id)
            .with_device_id(bundle.session.device_id)
            .with_attribute("wallet_id", wallet.wallet_id.clone())
            .with_attribute(
                "balance_sats",
                wallet.last_balance_sats.unwrap_or_default().to_string(),
            ),
    );
    state
        .observability
        .increment_counter("agent_payments.balance_viewed", &request_id);

    Ok(ok_data(serde_json::json!({
        "walletId": wallet.wallet_id,
        "balanceSats": wallet.last_balance_sats,
        "sparkAddress": wallet.spark_address,
        "lightningAddress": wallet.lightning_address,
        "lastSyncedAt": wallet.last_synced_at.map(timestamp),
    })))
}

async fn agent_payments_create_invoice(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<AgentPaymentsCreateInvoiceRequestPayload>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiErrorResponse>)> {
    let request_id = request_id(&headers);
    let bundle = session_bundle_from_headers(&state, &headers).await?;

    if payload.amount_sats < 1 {
        return Err(validation_error(
            "amountSats",
            "The amountSats field must be at least 1.",
        ));
    }
    let description = normalize_optional_bounded_string(payload.description, "description", 200)?;
    let wallet = ensure_agent_wallet_for_user(&state, &bundle.user.id).await?;
    let executor_result = agent_wallet_executor_request_json(
        "wallets/create-invoice",
        serde_json::json!({
            "walletId": wallet.wallet_id,
            "mnemonic": wallet.mnemonic,
            "amountSats": payload.amount_sats,
            "description": description.clone(),
        }),
    )
    .await?;

    let payment_request = json_first_string(&executor_result, &["paymentRequest", "invoice"])
        .ok_or_else(|| {
            error_response_with_status(
                StatusCode::BAD_GATEWAY,
                ApiErrorCode::ServiceUnavailable,
                "wallet executor response did not include paymentRequest".to_string(),
            )
        })?;

    state.observability.audit(
        AuditEvent::new("agent_payments.invoice_created", request_id.clone())
            .with_user_id(bundle.user.id)
            .with_session_id(bundle.session.session_id)
            .with_org_id(bundle.session.active_org_id)
            .with_device_id(bundle.session.device_id)
            .with_attribute("amount_sats", payload.amount_sats.to_string()),
    );
    state
        .observability
        .increment_counter("agent_payments.invoice_created", &request_id);

    let mut invoice_payload = serde_json::json!({
        "paymentRequest": payment_request,
        "amountSats": payload.amount_sats,
        "description": description,
        "expiresAt": executor_result.get("expiresAt").cloned().unwrap_or(serde_json::Value::Null),
        "raw": executor_result,
    });
    if let Some(receipt) = invoice_payload
        .get("raw")
        .and_then(|raw| raw.get("receipt"))
        .cloned()
    {
        invoice_payload["receipt"] = receipt;
    }

    Ok(ok_data(serde_json::json!({
        "invoice": invoice_payload,
    })))
}

async fn agent_payments_pay_invoice(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<AgentPaymentsPayInvoiceRequestPayload>,
) -> Result<Response, (StatusCode, Json<ApiErrorResponse>)> {
    let request_id = request_id(&headers);
    let bundle = session_bundle_from_headers(&state, &headers).await?;
    let invoice = non_empty(payload.invoice).ok_or_else(|| {
        validation_error(
            "invoice",
            "The invoice field is required and must be a string.",
        )
    })?;
    if invoice.chars().count() < 20 {
        return Err(validation_error(
            "invoice",
            "The invoice field must be at least 20 characters.",
        ));
    }

    if let Some(value) = payload.max_amount_sats {
        if value < 1 {
            return Err(validation_error(
                "maxAmountSats",
                "The maxAmountSats field must be at least 1.",
            ));
        }
    }
    if let Some(value) = payload.max_amount_msats {
        if value < 1000 {
            return Err(validation_error(
                "maxAmountMsats",
                "The maxAmountMsats field must be at least 1000.",
            ));
        }
    }
    if let Some(host) = payload.host.as_deref() {
        if host.trim().chars().count() > 255 {
            return Err(validation_error(
                "host",
                "Value may not be greater than 255 characters.",
            ));
        }
    }

    let timeout_ms = payload.timeout_ms.unwrap_or(12_000);
    if !(1_000..=120_000).contains(&timeout_ms) {
        return Err(validation_error(
            "timeoutMs",
            "The timeoutMs field must be between 1000 and 120000.",
        ));
    }

    let quoted_amount_msats = Bolt11::amount_msats(&invoice);
    let max_amount_msats = payload
        .max_amount_msats
        .or_else(|| {
            payload
                .max_amount_sats
                .map(|value| value.saturating_mul(1000))
        })
        .or(quoted_amount_msats);

    let Some(max_amount_msats) = max_amount_msats.filter(|value| *value > 0) else {
        state.observability.audit(
            AuditEvent::new("agent_payments.invoice_pay_rejected", request_id.clone())
                .with_user_id(bundle.user.id)
                .with_session_id(bundle.session.session_id)
                .with_org_id(bundle.session.active_org_id)
                .with_device_id(bundle.session.device_id)
                .with_attribute("reason", "max_amount_missing".to_string()),
        );
        state
            .observability
            .increment_counter("agent_payments.invoice_pay_rejected", &request_id);
        return Ok(
            (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(serde_json::json!({
                    "error": {
                        "code": "max_amount_missing",
                        "message": "Unable to resolve max payment amount; provide maxAmountSats or maxAmountMsats.",
                    }
                })),
            )
                .into_response(),
        );
    };

    let wallet = ensure_agent_wallet_for_user(&state, &bundle.user.id).await?;
    let payer_kind = l402_invoice_payer_kind();
    let payment = match agent_payments_pay_invoice_with_adapter(
        &state,
        payer_kind.as_str(),
        &bundle.user.id,
        &wallet,
        &invoice,
        max_amount_msats,
        timeout_ms,
        payload.host.clone(),
    )
    .await
    {
        Ok(result) => result,
        Err(error) => {
            state.observability.audit(
                AuditEvent::new("agent_payments.invoice_pay_failed", request_id.clone())
                    .with_user_id(bundle.user.id)
                    .with_session_id(bundle.session.session_id)
                    .with_org_id(bundle.session.active_org_id)
                    .with_device_id(bundle.session.device_id)
                    .with_attribute(
                        "quoted_amount_msats",
                        quoted_amount_msats.unwrap_or_default().to_string(),
                    )
                    .with_attribute("max_amount_msats", max_amount_msats.to_string())
                    .with_attribute("payment_backend", payer_kind)
                    .with_attribute("error_code", error.code.clone())
                    .with_attribute("error_message", error.message.clone()),
            );
            state
                .observability
                .increment_counter("agent_payments.invoice_pay_failed", &request_id);
            return Ok((
                error.status,
                Json(serde_json::json!({
                    "error": {
                        "code": error.code,
                        "message": error.message,
                    }
                })),
            )
                .into_response());
        }
    };

    state.observability.audit(
        AuditEvent::new("agent_payments.invoice_paid", request_id.clone())
            .with_user_id(bundle.user.id)
            .with_session_id(bundle.session.session_id)
            .with_org_id(bundle.session.active_org_id)
            .with_device_id(bundle.session.device_id)
            .with_attribute("payment_id", payment.payment_id.clone().unwrap_or_default())
            .with_attribute("status", payment.status.clone())
            .with_attribute("payment_backend", payer_kind)
            .with_attribute("max_amount_msats", max_amount_msats.to_string()),
    );
    state
        .observability
        .increment_counter("agent_payments.invoice_paid", &request_id);

    Ok(ok_data(serde_json::json!({
        "payment": {
            "paymentId": payment.payment_id,
            "preimage": payment.preimage.clone(),
            "proofReference": format!("preimage:{}", payment.preimage.chars().take(16).collect::<String>()),
            "quotedAmountMsats": quoted_amount_msats,
            "maxAmountMsats": max_amount_msats,
            "status": payment.status,
            "raw": payment.raw,
        }
    }))
    .into_response())
}

async fn agent_payments_pay_invoice_with_adapter(
    state: &AppState,
    payer_kind: &str,
    user_id: &str,
    wallet: &UserSparkWalletRecord,
    invoice: &str,
    max_amount_msats: u64,
    timeout_ms: u64,
    host: Option<String>,
) -> Result<AgentInvoicePaymentResult, AgentInvoicePaymentError> {
    agent_payments_invoice_cap_guard(invoice, max_amount_msats)?;

    match payer_kind {
        "fake" if cfg!(test) => {
            agent_payments_pay_invoice_fake(invoice, max_amount_msats, timeout_ms, host).await
        }
        "spark_wallet" => {
            agent_payments_pay_invoice_spark(
                state,
                user_id,
                wallet,
                invoice,
                max_amount_msats,
                timeout_ms,
                host,
            )
            .await
        }
        "lnd_rest" => agent_payments_pay_invoice_lnd(invoice, max_amount_msats, timeout_ms).await,
        "fake" => Err(agent_payments_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "invoice_payer_invalid",
            "Synthetic invoice payer is not allowed outside test builds.",
        )),
        _ => Err(agent_payments_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "invoice_payer_invalid",
            format!("Unknown L402 invoice payer: {payer_kind}"),
        )),
    }
}

fn agent_payments_invoice_cap_guard(
    invoice: &str,
    max_amount_msats: u64,
) -> Result<(), AgentInvoicePaymentError> {
    let Some(quoted_amount_msats) = Bolt11::amount_msats(invoice) else {
        return Ok(());
    };
    if quoted_amount_msats > max_amount_msats {
        return Err(agent_payments_error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "quoted_amount_exceeds_cap",
            "Quoted invoice amount exceeds maxAmountMsats.",
        ));
    }
    Ok(())
}

fn l402_invoice_payer_kind() -> String {
    env_non_empty_any(&["L402_INVOICE_PAYER", "OA_L402_INVOICE_PAYER"])
        .unwrap_or_else(|| "spark_wallet".to_string())
        .to_ascii_lowercase()
}

async fn agent_payments_pay_invoice_fake(
    invoice: &str,
    max_amount_msats: u64,
    timeout_ms: u64,
    host: Option<String>,
) -> Result<AgentInvoicePaymentResult, AgentInvoicePaymentError> {
    let preimage = sha256_hex(format!("preimage:{invoice}").as_bytes());
    let payment_hash = sha256_hex(format!("payment:{invoice}").as_bytes());
    let payment_id = format!("fake:{}", &payment_hash[..16]);

    Ok(AgentInvoicePaymentResult {
        payment_id: Some(payment_id.clone()),
        preimage: preimage.clone(),
        status: "completed".to_string(),
        raw: serde_json::json!({
            "paymentId": payment_id,
            "preimage": preimage,
            "status": "completed",
            "maxAmountMsats": max_amount_msats,
            "timeoutMs": timeout_ms,
            "host": host.and_then(non_empty).map(|value| value.to_lowercase()),
        }),
    })
}

async fn agent_payments_pay_invoice_spark(
    _state: &AppState,
    _user_id: &str,
    wallet: &UserSparkWalletRecord,
    invoice: &str,
    max_amount_msats: u64,
    timeout_ms: u64,
    host: Option<String>,
) -> Result<AgentInvoicePaymentResult, AgentInvoicePaymentError> {
    let Some(base_url) = env_non_empty_any(&[
        "SPARK_EXECUTOR_BASE_URL",
        "OA_LIGHTNING_WALLET_EXECUTOR_BASE_URL",
    ]) else {
        return Err(agent_payments_error(
            StatusCode::BAD_GATEWAY,
            "spark_executor_not_configured",
            "Spark wallet executor is not configured in this environment. Set SPARK_EXECUTOR_BASE_URL and SPARK_EXECUTOR_AUTH_TOKEN.",
        ));
    };

    let executor_timeout_ms = env_u64("SPARK_EXECUTOR_TIMEOUT_MS", 20_000);
    let timeout_seconds = executor_timeout_ms.div_ceil(1000).max(1);
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(timeout_seconds))
        .build()
        .map_err(|error| {
            agent_payments_error(
                StatusCode::BAD_GATEWAY,
                "spark_executor_error",
                format!("Failed to build Spark executor client: {error}"),
            )
        })?;

    let mut request = client
        .post(format!(
            "{}/wallets/pay-bolt11",
            base_url.trim_end_matches('/')
        ))
        .header(reqwest::header::ACCEPT, "application/json")
        .header(reqwest::header::CONTENT_TYPE, "application/json");

    let Some(auth_token) = env_non_empty_any(&[
        "SPARK_EXECUTOR_AUTH_TOKEN",
        "OA_LIGHTNING_WALLET_EXECUTOR_AUTH_TOKEN",
    ]) else {
        return Err(agent_payments_error(
            StatusCode::BAD_GATEWAY,
            "spark_executor_not_configured",
            "Spark wallet executor auth token is not configured in this environment. Set SPARK_EXECUTOR_AUTH_TOKEN or OA_LIGHTNING_WALLET_EXECUTOR_AUTH_TOKEN.",
        ));
    };
    request = request.bearer_auth(auth_token);

    let normalized_host = host.and_then(non_empty).map(|value| value.to_lowercase());
    let mut payload = serde_json::json!({
        "walletId": wallet.wallet_id,
        "mnemonic": wallet.mnemonic,
        "invoice": invoice,
        "maxAmountMsats": max_amount_msats,
        "timeoutMs": timeout_ms.max(1_000),
    });
    if let Some(value) = normalized_host {
        payload["host"] = serde_json::json!(value);
    }

    let response = request.json(&payload).send().await.map_err(|error| {
        agent_payments_error(
            StatusCode::BAD_GATEWAY,
            "spark_executor_error",
            format!("Spark executor request failed: {error}"),
        )
    })?;
    let status = response.status();
    let body_text = response.text().await.map_err(|error| {
        agent_payments_error(
            StatusCode::BAD_GATEWAY,
            "spark_executor_error",
            format!("Spark executor response read failed: {error}"),
        )
    })?;
    let body = serde_json::from_str::<serde_json::Value>(&body_text).map_err(|error| {
        agent_payments_error(
            StatusCode::BAD_GATEWAY,
            "spark_executor_error",
            format!("Spark executor returned non-JSON response: {error}"),
        )
    })?;

    let payload_failed = body
        .get("ok")
        .and_then(serde_json::Value::as_bool)
        .map(|ok| !ok)
        .unwrap_or(false);
    if !status.is_success() || payload_failed {
        let error = body.get("error").and_then(serde_json::Value::as_object);
        let code = error
            .and_then(|value| value.get("code"))
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("spark_executor_error");
        let message = error
            .and_then(|value| value.get("message"))
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| {
                format!(
                    "Spark executor request failed with HTTP {}",
                    status.as_u16()
                )
            });
        return Err(agent_payments_error(StatusCode::BAD_GATEWAY, code, message));
    }

    let Some(result) = json_result_object(&body) else {
        return Err(agent_payments_error(
            StatusCode::BAD_GATEWAY,
            "spark_executor_error",
            "Spark executor response did not include an object result.",
        ));
    };

    let Some(preimage) = json_first_string(
        result,
        &[
            "preimage",
            "paymentPreimage",
            "payment.preimage",
            "payment.paymentPreimage",
        ],
    ) else {
        return Err(agent_payments_error(
            StatusCode::BAD_GATEWAY,
            "spark_executor_error",
            "Spark wallet payer did not return a payment preimage.",
        ));
    };
    let preimage = normalize_preimage_hex(&preimage)?;
    let payment_id = json_first_string(
        result,
        &[
            "paymentId",
            "paymentHash",
            "payment.paymentId",
            "payment.paymentHash",
        ],
    );
    let status = json_first_string(result, &["status", "payment.status"])
        .unwrap_or_else(|| "completed".to_string());

    Ok(AgentInvoicePaymentResult {
        payment_id,
        preimage,
        status,
        raw: result.clone(),
    })
}

async fn agent_payments_pay_invoice_lnd(
    invoice: &str,
    _max_amount_msats: u64,
    timeout_ms: u64,
) -> Result<AgentInvoicePaymentResult, AgentInvoicePaymentError> {
    let Some(base_url) = env_non_empty("LND_REST_BASE_URL") else {
        return Err(agent_payments_error(
            StatusCode::BAD_GATEWAY,
            "lnd_rest_not_configured",
            "LND REST payer not configured. Set LND_REST_BASE_URL and LND_REST_MACAROON_HEX.",
        ));
    };
    let Some(macaroon_hex) = env_non_empty("LND_REST_MACAROON_HEX") else {
        return Err(agent_payments_error(
            StatusCode::BAD_GATEWAY,
            "lnd_rest_not_configured",
            "LND REST payer not configured. Set LND_REST_BASE_URL and LND_REST_MACAROON_HEX.",
        ));
    };

    let timeout_seconds = timeout_ms.div_ceil(1000).max(1);
    let mut client_builder =
        reqwest::Client::builder().timeout(std::time::Duration::from_secs(timeout_seconds));

    if let Some(cert_base64) = env_non_empty("LND_REST_TLS_CERT_BASE64") {
        let decoded = STANDARD.decode(cert_base64).map_err(|error| {
            agent_payments_error(
                StatusCode::BAD_GATEWAY,
                "lnd_rest_invalid_tls_cert",
                format!("Invalid base64 in LND_REST_TLS_CERT_BASE64: {error}"),
            )
        })?;
        let certificate = reqwest::Certificate::from_pem(&decoded)
            .or_else(|_| reqwest::Certificate::from_der(&decoded))
            .map_err(|error| {
                agent_payments_error(
                    StatusCode::BAD_GATEWAY,
                    "lnd_rest_invalid_tls_cert",
                    format!("Invalid certificate in LND_REST_TLS_CERT_BASE64: {error}"),
                )
            })?;
        client_builder = client_builder.add_root_certificate(certificate);
    } else if !env_bool("LND_REST_TLS_VERIFY", true) {
        client_builder = client_builder.danger_accept_invalid_certs(true);
    }

    let client = client_builder.build().map_err(|error| {
        agent_payments_error(
            StatusCode::BAD_GATEWAY,
            "lnd_rest_error",
            format!("Failed to build LND REST client: {error}"),
        )
    })?;

    let response = client
        .post(format!(
            "{}/v1/channels/transactions",
            base_url.trim_end_matches('/')
        ))
        .header("Grpc-Metadata-macaroon", macaroon_hex)
        .json(&serde_json::json!({
            "payment_request": invoice,
        }))
        .send()
        .await
        .map_err(|error| {
            agent_payments_error(
                StatusCode::BAD_GATEWAY,
                "lnd_rest_error",
                format!("LND REST pay request failed: {error}"),
            )
        })?;

    let status = response.status();
    let body_text = response.text().await.map_err(|error| {
        agent_payments_error(
            StatusCode::BAD_GATEWAY,
            "lnd_rest_error",
            format!("LND REST pay response read failed: {error}"),
        )
    })?;
    if !status.is_success() {
        return Err(agent_payments_error(
            StatusCode::BAD_GATEWAY,
            "lnd_rest_error",
            format!(
                "LND REST pay failed: HTTP {} {}",
                status.as_u16(),
                body_text
            ),
        ));
    }

    let body = serde_json::from_str::<serde_json::Value>(&body_text).map_err(|error| {
        agent_payments_error(
            StatusCode::BAD_GATEWAY,
            "lnd_rest_error",
            format!("LND REST pay failed: invalid JSON response ({error})"),
        )
    })?;
    let Some(object) = body.as_object() else {
        return Err(agent_payments_error(
            StatusCode::BAD_GATEWAY,
            "lnd_rest_error",
            "LND REST pay failed: invalid JSON response",
        ));
    };

    if let Some(payment_error) = object
        .get("payment_error")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return Err(agent_payments_error(
            StatusCode::BAD_GATEWAY,
            "lnd_rest_error",
            format!("LND REST pay failed: {payment_error}"),
        ));
    }

    let Some(preimage_raw) = object
        .get("payment_preimage")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return Err(agent_payments_error(
            StatusCode::BAD_GATEWAY,
            "lnd_rest_error",
            "LND REST pay failed: missing payment_preimage",
        ));
    };
    let preimage = normalize_preimage_hex(preimage_raw).map_err(|_| {
        agent_payments_error(
            StatusCode::BAD_GATEWAY,
            "lnd_rest_error",
            "LND REST pay failed: payment_preimage is neither hex nor base64",
        )
    })?;

    Ok(AgentInvoicePaymentResult {
        payment_id: object
            .get("payment_hash")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string),
        preimage,
        status: "completed".to_string(),
        raw: body,
    })
}

fn agent_payments_error(
    status: StatusCode,
    code: &str,
    message: impl Into<String>,
) -> AgentInvoicePaymentError {
    AgentInvoicePaymentError {
        status,
        code: code.to_string(),
        message: message.into(),
    }
}

fn json_result_object(payload: &serde_json::Value) -> Option<&serde_json::Value> {
    let result = payload
        .get("result")
        .or_else(|| payload.get("status"))
        .or_else(|| payload.get("data"))
        .unwrap_or(payload);
    result.as_object().map(|_| result)
}

fn json_first_string(payload: &serde_json::Value, paths: &[&str]) -> Option<String> {
    for path in paths {
        let mut current = payload;
        let mut found = true;
        for segment in path.split('.') {
            let Some(next) = current.get(segment) else {
                found = false;
                break;
            };
            current = next;
        }
        if !found {
            continue;
        }
        if let Some(value) = current
            .as_str()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            return Some(value.to_string());
        }
    }
    None
}

fn json_first_u64(payload: &serde_json::Value, paths: &[&str]) -> Option<u64> {
    for path in paths {
        let mut current = payload;
        let mut found = true;
        for segment in path.split('.') {
            let Some(next) = current.get(segment) else {
                found = false;
                break;
            };
            current = next;
        }
        if !found {
            continue;
        }
        if let Some(value) = current.as_u64() {
            return Some(value);
        }
        if let Some(value) = current.as_i64().and_then(|value| u64::try_from(value).ok()) {
            return Some(value);
        }
        if let Some(value) = current
            .as_str()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .and_then(|value| value.parse::<u64>().ok())
        {
            return Some(value);
        }
    }
    None
}

fn normalize_preimage_hex(preimage: &str) -> Result<String, AgentInvoicePaymentError> {
    let value = preimage.trim();
    if value.len() == 64 && value.chars().all(|ch| ch.is_ascii_hexdigit()) {
        return Ok(value.to_ascii_lowercase());
    }
    let decoded = STANDARD.decode(value).map_err(|_| {
        agent_payments_error(
            StatusCode::BAD_GATEWAY,
            "invoice_payer_error",
            "payment preimage is neither hex nor base64",
        )
    })?;
    Ok(bytes_to_hex(&decoded))
}

fn bytes_to_hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;

    let mut output = String::with_capacity(bytes.len().saturating_mul(2));
    for byte in bytes {
        let _ = write!(&mut output, "{byte:02x}");
    }
    output
}

fn env_non_empty_any(keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| env_non_empty(key))
}

fn env_non_empty(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn env_u64(key: &str, default: u64) -> u64 {
    std::env::var(key)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(default)
}

fn env_bool(key: &str, default: bool) -> bool {
    std::env::var(key)
        .ok()
        .map(|value| value.trim().to_ascii_lowercase())
        .map(|value| match value.as_str() {
            "1" | "true" | "yes" => true,
            "0" | "false" | "no" => false,
            _ => default,
        })
        .unwrap_or(default)
}

fn agent_wallet_executor_api_config()
-> Result<AgentWalletExecutorConfig, (StatusCode, Json<ApiErrorResponse>)> {
    let base_url = env_non_empty_any(&[
        "SPARK_EXECUTOR_BASE_URL",
        "OA_LIGHTNING_WALLET_EXECUTOR_BASE_URL",
    ])
    .ok_or_else(|| {
        error_response_with_status(
            StatusCode::SERVICE_UNAVAILABLE,
            ApiErrorCode::ServiceUnavailable,
            "Spark wallet executor base URL is not configured.".to_string(),
        )
    })?;
    let auth_token = env_non_empty_any(&[
        "SPARK_EXECUTOR_AUTH_TOKEN",
        "OA_LIGHTNING_WALLET_EXECUTOR_AUTH_TOKEN",
    ])
    .ok_or_else(|| {
        error_response_with_status(
            StatusCode::SERVICE_UNAVAILABLE,
            ApiErrorCode::ServiceUnavailable,
            "Spark wallet executor auth token is not configured.".to_string(),
        )
    })?;
    let timeout_ms = env_u64("SPARK_EXECUTOR_TIMEOUT_MS", 20_000).clamp(1_000, 120_000);

    Ok(AgentWalletExecutorConfig {
        base_url,
        auth_token,
        timeout_ms,
    })
}

fn map_wallet_executor_status_to_api_error(status: StatusCode) -> ApiErrorCode {
    if status == StatusCode::UNPROCESSABLE_ENTITY {
        ApiErrorCode::InvalidRequest
    } else if status == StatusCode::UNAUTHORIZED {
        ApiErrorCode::Unauthorized
    } else if status == StatusCode::NOT_FOUND {
        ApiErrorCode::NotFound
    } else {
        ApiErrorCode::ServiceUnavailable
    }
}

async fn agent_wallet_executor_request_json(
    path: &str,
    payload: serde_json::Value,
) -> Result<serde_json::Value, (StatusCode, Json<ApiErrorResponse>)> {
    let config = agent_wallet_executor_api_config()?;
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_millis(config.timeout_ms))
        .build()
        .map_err(|error| {
            error_response_with_status(
                StatusCode::SERVICE_UNAVAILABLE,
                ApiErrorCode::ServiceUnavailable,
                format!("Failed to build wallet executor client: {error}"),
            )
        })?;

    let response = client
        .post(format!(
            "{}/{}",
            config.base_url.trim_end_matches('/'),
            path.trim_start_matches('/')
        ))
        .header(reqwest::header::ACCEPT, "application/json")
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .bearer_auth(config.auth_token)
        .json(&payload)
        .send()
        .await
        .map_err(|error| {
            error_response_with_status(
                StatusCode::BAD_GATEWAY,
                ApiErrorCode::ServiceUnavailable,
                format!("Wallet executor request failed: {error}"),
            )
        })?;

    let status = response.status();
    let body_text = response.text().await.map_err(|error| {
        error_response_with_status(
            StatusCode::BAD_GATEWAY,
            ApiErrorCode::ServiceUnavailable,
            format!("Wallet executor response read failed: {error}"),
        )
    })?;
    let body = serde_json::from_str::<serde_json::Value>(&body_text).unwrap_or_else(|_| {
        serde_json::json!({
            "raw": body_text,
        })
    });

    let payload_failed = body
        .get("ok")
        .and_then(serde_json::Value::as_bool)
        .map(|value| !value)
        .unwrap_or(false);
    if !status.is_success() || payload_failed {
        let error_obj = body.get("error").and_then(serde_json::Value::as_object);
        let error_code = error_obj
            .and_then(|object| object.get("code"))
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("wallet_executor_error");
        let error_message = error_obj
            .and_then(|object| object.get("message"))
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| format!("Wallet executor returned HTTP {}", status.as_u16()));
        return Err(error_response_with_status(
            if status.is_client_error() || status.is_server_error() {
                status
            } else {
                StatusCode::BAD_GATEWAY
            },
            map_wallet_executor_status_to_api_error(status),
            format!("{error_code}: {error_message}"),
        ));
    }

    let result = json_result_object(&body).cloned().ok_or_else(|| {
        error_response_with_status(
            StatusCode::BAD_GATEWAY,
            ApiErrorCode::ServiceUnavailable,
            "Wallet executor response did not include an object result.".to_string(),
        )
    })?;

    Ok(result)
}

fn l402_allowlist_hosts_from_env() -> Vec<String> {
    env_non_empty("L402_ALLOWLIST_HOSTS")
        .map(|value| {
            value
                .split(',')
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(|value| value.to_ascii_lowercase())
                .collect::<Vec<_>>()
        })
        .unwrap_or_else(|| vec!["sats4ai.com".to_string(), "l402.openagents.com".to_string()])
}

async fn agent_payments_send_spark(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<AgentPaymentsSendSparkRequestPayload>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiErrorResponse>)> {
    let request_id = request_id(&headers);
    let bundle = session_bundle_from_headers(&state, &headers).await?;
    let spark_address = non_empty(payload.spark_address).ok_or_else(|| {
        validation_error(
            "sparkAddress",
            "The sparkAddress field is required and must be a string.",
        )
    })?;
    if spark_address.chars().count() < 3 || spark_address.chars().count() > 255 {
        return Err(validation_error(
            "sparkAddress",
            "The sparkAddress field must be between 3 and 255 characters.",
        ));
    }
    if payload.amount_sats < 1 {
        return Err(validation_error(
            "amountSats",
            "The amountSats field must be at least 1.",
        ));
    }

    let timeout_ms = payload.timeout_ms.unwrap_or(12_000);
    if !(1_000..=120_000).contains(&timeout_ms) {
        return Err(validation_error(
            "timeoutMs",
            "The timeoutMs field must be between 1000 and 120000.",
        ));
    }

    let wallet = ensure_agent_wallet_for_user(&state, &bundle.user.id).await?;
    let executor_result = agent_wallet_executor_request_json(
        "wallets/send-spark",
        serde_json::json!({
            "walletId": wallet.wallet_id,
            "mnemonic": wallet.mnemonic,
            "sparkAddress": spark_address.clone(),
            "amountSats": payload.amount_sats,
            "timeoutMs": timeout_ms,
        }),
    )
    .await?;
    let payment_id = json_first_string(&executor_result, &["paymentId"]);
    let status =
        json_first_string(&executor_result, &["status"]).unwrap_or_else(|| "completed".to_string());

    state.observability.audit(
        AuditEvent::new("agent_payments.spark_sent", request_id.clone())
            .with_user_id(bundle.user.id)
            .with_session_id(bundle.session.session_id)
            .with_org_id(bundle.session.active_org_id)
            .with_device_id(bundle.session.device_id)
            .with_attribute("spark_address", spark_address.clone())
            .with_attribute("amount_sats", payload.amount_sats.to_string())
            .with_attribute("payment_id", payment_id.clone().unwrap_or_default()),
    );
    state
        .observability
        .increment_counter("agent_payments.spark_sent", &request_id);

    Ok(ok_data(serde_json::json!({
        "transfer": {
            "sparkAddress": spark_address,
            "amountSats": payload.amount_sats,
            "status": status,
            "paymentId": payment_id,
            "raw": executor_result,
        }
    })))
}

async fn upsert_agent_wallet_for_user(
    state: &AppState,
    user_id: &str,
    mnemonic_override: Option<String>,
) -> Result<UserSparkWalletRecord, (StatusCode, Json<ApiErrorResponse>)> {
    let existing = state
        ._domain_store
        .find_user_spark_wallet(user_id)
        .await
        .map_err(map_domain_store_error)?;
    let now = Utc::now();

    let wallet_id = existing
        .as_ref()
        .map(|wallet| wallet.wallet_id.clone())
        .unwrap_or_else(|| format!("wallet_{}", Uuid::new_v4().simple()));
    let mnemonic =
        mnemonic_override.or_else(|| existing.as_ref().map(|wallet| wallet.mnemonic.clone()));

    let mut payload = serde_json::json!({
        "walletId": wallet_id.clone(),
    });
    if let Some(mnemonic) = mnemonic.clone() {
        payload["mnemonic"] = serde_json::json!(mnemonic);
    }

    let executor_result = agent_wallet_executor_request_json("wallets/create", payload).await?;
    let resolved_mnemonic = json_first_string(&executor_result, &["mnemonic"])
        .or(mnemonic)
        .ok_or_else(|| {
            error_response_with_status(
                StatusCode::BAD_GATEWAY,
                ApiErrorCode::ServiceUnavailable,
                "wallet executor response did not include mnemonic".to_string(),
            )
        })?;
    let spark_address = json_first_string(&executor_result, &["sparkAddress"]).or_else(|| {
        existing
            .as_ref()
            .and_then(|wallet| wallet.spark_address.clone())
    });
    let lightning_address =
        json_first_string(&executor_result, &["lightningAddress"]).or_else(|| {
            existing
                .as_ref()
                .and_then(|wallet| wallet.lightning_address.clone())
        });
    let identity_pubkey = json_first_string(&executor_result, &["identityPubkey"]).or_else(|| {
        existing
            .as_ref()
            .and_then(|wallet| wallet.identity_pubkey.clone())
    });
    let last_balance_sats = json_first_u64(&executor_result, &["balanceSats"]).or_else(|| {
        existing
            .as_ref()
            .and_then(|wallet| wallet.last_balance_sats)
    });
    let status = json_first_string(&executor_result, &["status"])
        .or_else(|| existing.as_ref().map(|wallet| wallet.status.clone()))
        .unwrap_or_else(|| "active".to_string());
    let provider = existing
        .as_ref()
        .map(|wallet| wallet.provider.clone())
        .unwrap_or_else(|| "spark_executor".to_string());

    state
        ._domain_store
        .upsert_user_spark_wallet(UpsertUserSparkWalletInput {
            user_id: user_id.to_string(),
            wallet_id,
            mnemonic: resolved_mnemonic,
            spark_address,
            lightning_address,
            identity_pubkey,
            last_balance_sats,
            status: Some(status),
            provider: Some(provider),
            last_error: existing
                .as_ref()
                .and_then(|wallet| wallet.last_error.clone()),
            meta: existing.as_ref().and_then(|wallet| wallet.meta.clone()),
            last_synced_at: Some(now),
        })
        .await
        .map_err(map_domain_store_error)
}

async fn ensure_agent_wallet_for_user(
    state: &AppState,
    user_id: &str,
) -> Result<UserSparkWalletRecord, (StatusCode, Json<ApiErrorResponse>)> {
    upsert_agent_wallet_for_user(state, user_id, None).await
}

async fn sync_agent_wallet(
    state: &AppState,
    wallet: UserSparkWalletRecord,
) -> Result<UserSparkWalletRecord, (StatusCode, Json<ApiErrorResponse>)> {
    let UserSparkWalletRecord {
        user_id,
        wallet_id,
        mnemonic,
        spark_address,
        lightning_address,
        identity_pubkey,
        last_balance_sats,
        status,
        provider,
        last_error,
        meta,
        last_synced_at: _,
        created_at: _,
        updated_at: _,
    } = wallet;
    let executor_result = agent_wallet_executor_request_json(
        "wallets/status",
        serde_json::json!({
            "walletId": wallet_id.clone(),
            "mnemonic": mnemonic.clone(),
        }),
    )
    .await?;

    state
        ._domain_store
        .upsert_user_spark_wallet(UpsertUserSparkWalletInput {
            user_id,
            wallet_id: wallet_id.clone(),
            mnemonic: json_first_string(&executor_result, &["mnemonic"]).unwrap_or(mnemonic),
            spark_address: json_first_string(&executor_result, &["sparkAddress"]).or(spark_address),
            lightning_address: json_first_string(&executor_result, &["lightningAddress"])
                .or(lightning_address),
            identity_pubkey: json_first_string(&executor_result, &["identityPubkey"])
                .or(identity_pubkey),
            last_balance_sats: json_first_u64(&executor_result, &["balanceSats"])
                .or(last_balance_sats),
            status: Some(json_first_string(&executor_result, &["status"]).unwrap_or(status)),
            provider: Some(provider),
            last_error,
            meta,
            last_synced_at: Some(Utc::now()),
        })
        .await
        .map_err(map_domain_store_error)
}

fn agent_payments_wallet_payload(wallet: &UserSparkWalletRecord) -> serde_json::Value {
    serde_json::json!({
        "id": wallet.wallet_id,
        "walletId": wallet.wallet_id,
        "sparkAddress": wallet.spark_address,
        "lightningAddress": wallet.lightning_address,
        "identityPubkey": wallet.identity_pubkey,
        "balanceSats": wallet.last_balance_sats,
        "status": wallet.status,
        "provider": wallet.provider,
        "lastError": wallet.last_error,
        "lastSyncedAt": wallet.last_synced_at.map(timestamp),
        "createdAt": timestamp(wallet.created_at),
        "updatedAt": timestamp(wallet.updated_at),
    })
}

#[derive(Debug, Clone)]
struct L402AutopilotFilterValue {
    id: String,
    handle: String,
}

#[derive(Debug, Clone)]
struct L402ReceiptView {
    event_id: u64,
    thread_id: String,
    thread_title: String,
    run_id: String,
    run_status: Option<String>,
    run_started_at: Option<String>,
    run_completed_at: Option<String>,
    created_at: String,
    status: String,
    host: String,
    scope: Option<String>,
    paid: bool,
    cache_hit: bool,
    cache_status: Option<String>,
    amount_msats: Option<i64>,
    quoted_amount_msats: Option<i64>,
    max_spend_msats: Option<i64>,
    proof_reference: Option<String>,
    deny_code: Option<String>,
    task_id: Option<String>,
    approval_required: bool,
    response_status_code: Option<i64>,
    response_body_sha256: Option<String>,
    tool_call_id: Option<String>,
    raw_payload: serde_json::Value,
}

struct LightningOpsInvoiceLifecycleInput {
    invoice_id: String,
    paywall_id: String,
    owner_id: String,
    amount_msats: i64,
    status: String,
    payment_hash: Option<String>,
    payment_request: Option<String>,
    payment_proof_ref: Option<String>,
    request_id: Option<String>,
    settled_at_ms: Option<i64>,
}

async fn resolve_l402_autopilot_filter(
    state: &AppState,
    owner_user_id: &str,
    candidate: Option<String>,
) -> Result<Option<L402AutopilotFilterValue>, (StatusCode, Json<ApiErrorResponse>)> {
    let Some(reference) = candidate.and_then(non_empty) else {
        return Ok(None);
    };

    match state
        ._domain_store
        .resolve_autopilot_filter_for_owner(owner_user_id, &reference)
        .await
    {
        Ok(autopilot) => Ok(Some(L402AutopilotFilterValue {
            id: autopilot.id,
            handle: autopilot.handle,
        })),
        Err(DomainStoreError::NotFound) => Err(not_found_error("autopilot_not_found")),
        Err(DomainStoreError::Forbidden) => Err(forbidden_error("autopilot_forbidden")),
        Err(error) => Err(map_domain_store_error(error)),
    }
}

type LightningOpsControlPlaneResponse =
    Result<(StatusCode, Json<serde_json::Value>), (StatusCode, Json<serde_json::Value>)>;

async fn lightning_ops_control_plane_query(
    State(state): State<AppState>,
    Json(payload): Json<serde_json::Value>,
) -> LightningOpsControlPlaneResponse {
    let (function_name, mut args) = lightning_ops_payload(payload)?;
    lightning_ops_assert_secret(&mut args)?;

    match function_name.as_str() {
        "lightning/ops:listPaywallControlPlaneState" => {
            let statuses = lightning_ops_statuses(&args);
            let mut rows = state
                ._domain_store
                .list_all_l402_paywalls(true)
                .await
                .map_err(lightning_ops_map_domain_store_error)?;
            rows.sort_by(|left, right| left.created_at.cmp(&right.created_at));

            let paywalls = rows
                .iter()
                .map(lightning_ops_paywall_payload)
                .filter(|payload| {
                    if statuses.is_empty() {
                        return true;
                    }
                    payload
                        .get("status")
                        .and_then(serde_json::Value::as_str)
                        .map(|status| statuses.iter().any(|value| value == status))
                        .unwrap_or(false)
                })
                .collect::<Vec<_>>();

            Ok((
                StatusCode::OK,
                Json(serde_json::json!({
                    "ok": true,
                    "paywalls": paywalls,
                })),
            ))
        }
        "lightning/security:getControlPlaneSecurityState" => {
            let store = state.lightning_ops_control_plane.store.lock().await;

            let global = store
                .global_security
                .as_ref()
                .map(lightning_ops_global_security_payload)
                .unwrap_or_else(|| {
                    serde_json::json!({
                        "stateId": "global",
                        "globalPause": false,
                        "updatedAtMs": 0,
                    })
                });

            let mut owner_controls = store
                .owner_controls
                .values()
                .cloned()
                .collect::<Vec<LightningOpsOwnerControlRecord>>();
            owner_controls.sort_by(|left, right| left.owner_id.cmp(&right.owner_id));

            let mut credential_roles = store
                .credential_roles
                .values()
                .cloned()
                .collect::<Vec<LightningOpsCredentialRoleRecord>>();
            credential_roles.sort_by(|left, right| left.role.cmp(&right.role));

            Ok((
                StatusCode::OK,
                Json(serde_json::json!({
                    "ok": true,
                    "global": global,
                    "ownerControls": owner_controls
                        .iter()
                        .map(lightning_ops_owner_control_payload)
                        .collect::<Vec<_>>(),
                    "credentialRoles": credential_roles
                        .iter()
                        .map(lightning_ops_credential_role_payload)
                        .collect::<Vec<_>>(),
                })),
            ))
        }
        _ => Err(lightning_ops_error(
            StatusCode::NOT_FOUND,
            "unsupported_function",
            format!("unsupported function: {function_name}"),
        )),
    }
}

async fn lightning_ops_control_plane_mutation(
    State(state): State<AppState>,
    Json(payload): Json<serde_json::Value>,
) -> LightningOpsControlPlaneResponse {
    let (function_name, mut args) = lightning_ops_payload(payload)?;
    lightning_ops_assert_secret(&mut args)?;

    match function_name.as_str() {
        "lightning/ops:recordGatewayCompileIntent" => {
            let deployment_id = lightning_ops_optional_string(&args, "deploymentId")
                .unwrap_or_else(|| Uuid::new_v4().to_string());
            let config_hash = lightning_ops_required_string(&args, "configHash")?;
            let status = lightning_ops_required_string(&args, "status")?;
            let now = Utc::now();

            let mut store = state.lightning_ops_control_plane.store.lock().await;
            let entry = store
                .deployments
                .entry(deployment_id.clone())
                .or_insert_with(|| LightningOpsDeploymentRecord {
                    deployment_id: deployment_id.clone(),
                    paywall_id: None,
                    owner_id: None,
                    config_hash: config_hash.clone(),
                    image_digest: None,
                    status: status.clone(),
                    diagnostics: None,
                    metadata: None,
                    request_id: None,
                    applied_at_ms: None,
                    rolled_back_from: None,
                    created_at: now,
                    updated_at: now,
                });

            entry.paywall_id = lightning_ops_optional_string(&args, "paywallId");
            entry.owner_id = lightning_ops_optional_string(&args, "ownerId");
            entry.config_hash = config_hash;
            entry.image_digest = lightning_ops_optional_string(&args, "imageDigest");
            entry.status = status;
            entry.diagnostics = lightning_ops_optional_json(&args, "diagnostics");
            entry.metadata = lightning_ops_optional_json(&args, "metadata");
            entry.request_id = lightning_ops_optional_string(&args, "requestId");
            entry.applied_at_ms = lightning_ops_optional_i64(&args, "appliedAtMs");
            entry.rolled_back_from = lightning_ops_optional_string(&args, "rolledBackFrom");
            entry.updated_at = now;

            Ok((
                StatusCode::OK,
                Json(serde_json::json!({
                    "ok": true,
                    "deployment": lightning_ops_deployment_payload(entry),
                })),
            ))
        }
        "lightning/ops:recordGatewayDeploymentEvent" => {
            let paywall_id = lightning_ops_required_string(&args, "paywallId")?;
            let owner_id = lightning_ops_required_string(&args, "ownerId")?;
            let event_type = lightning_ops_required_string(&args, "eventType")?;
            let level = lightning_ops_required_string(&args, "level")?;
            let now = Utc::now();
            let event = LightningOpsGatewayEventRecord {
                event_id: format!("evt_{}", Uuid::new_v4().simple()),
                paywall_id,
                owner_id,
                event_type,
                level,
                request_id: lightning_ops_optional_string(&args, "requestId"),
                metadata: lightning_ops_optional_json(&args, "metadata"),
                created_at: now,
            };

            state
                .lightning_ops_control_plane
                .store
                .lock()
                .await
                .gateway_events
                .push(event.clone());

            Ok((
                StatusCode::OK,
                Json(serde_json::json!({
                    "ok": true,
                    "event": lightning_ops_gateway_event_payload(&event),
                })),
            ))
        }
        "lightning/settlements:ingestInvoiceLifecycle" => {
            let input = lightning_ops_invoice_lifecycle_input(&args, None)?;
            let mut store = state.lightning_ops_control_plane.store.lock().await;
            let (changed, invoice) = lightning_ops_apply_invoice_lifecycle(&mut store, input)?;

            Ok((
                StatusCode::OK,
                Json(serde_json::json!({
                    "ok": true,
                    "changed": changed,
                    "invoice": lightning_ops_invoice_payload(&invoice),
                })),
            ))
        }
        "lightning/settlements:ingestSettlement" => {
            let settlement_id = lightning_ops_required_string(&args, "settlementId")?;
            let invoice_id = lightning_ops_optional_string(&args, "invoiceId");
            let now = Utc::now();

            let mut store = state.lightning_ops_control_plane.store.lock().await;
            let invoice_payload = if invoice_id.is_some() {
                let input = lightning_ops_invoice_lifecycle_input(&args, Some("settled"))?;
                let (_, invoice) = lightning_ops_apply_invoice_lifecycle(&mut store, input)?;
                Some(invoice)
            } else {
                None
            };

            if let Some(existing) = store.settlements.get(&settlement_id).cloned() {
                let mut payload = serde_json::json!({
                    "ok": true,
                    "existed": true,
                    "settlement": lightning_ops_settlement_payload(&existing),
                });
                if let Some(invoice) = invoice_payload.as_ref() {
                    payload["invoice"] = lightning_ops_invoice_payload(invoice);
                }
                return Ok((StatusCode::OK, Json(payload)));
            }

            let payment_proof_type = lightning_ops_required_string(&args, "paymentProofType")?;
            if payment_proof_type != "lightning_preimage" {
                return Err(lightning_ops_invalid_arguments(
                    "invalid_payment_proof_type",
                ));
            }
            let preimage = lightning_ops_required_string(&args, "paymentProofValue")?
                .trim()
                .to_lowercase();
            if preimage.is_empty()
                || !preimage
                    .chars()
                    .all(|value| value.is_ascii_hexdigit() && !value.is_ascii_uppercase())
            {
                return Err(lightning_ops_invalid_arguments("invalid_preimage"));
            }

            let settlement = LightningOpsSettlementRecord {
                settlement_id,
                paywall_id: lightning_ops_required_string(&args, "paywallId")?,
                owner_id: lightning_ops_required_string(&args, "ownerId")?,
                invoice_id,
                amount_msats: lightning_ops_required_i64(&args, "amountMsats")?,
                payment_proof_ref: format!(
                    "lightning_preimage:{}",
                    preimage.chars().take(24).collect::<String>()
                ),
                request_id: lightning_ops_optional_string(&args, "requestId"),
                metadata: lightning_ops_optional_json(&args, "metadata"),
                created_at: now,
            };
            store
                .settlements
                .insert(settlement.settlement_id.clone(), settlement.clone());

            let mut payload = serde_json::json!({
                "ok": true,
                "existed": false,
                "settlement": lightning_ops_settlement_payload(&settlement),
            });
            if let Some(invoice) = invoice_payload.as_ref() {
                payload["invoice"] = lightning_ops_invoice_payload(invoice);
            }
            Ok((StatusCode::OK, Json(payload)))
        }
        "lightning/security:setGlobalPause" => {
            let active = lightning_ops_bool(&args, "active");
            let now = Utc::now();
            let now_ms = now.timestamp_millis();
            let global = LightningOpsGlobalSecurityRecord {
                global_pause: active,
                deny_reason_code: if active {
                    Some("global_pause_active".to_string())
                } else {
                    None
                },
                deny_reason: if active {
                    Some(
                        lightning_ops_optional_string(&args, "reason")
                            .unwrap_or_else(|| "Global paywall pause is active".to_string()),
                    )
                } else {
                    None
                },
                updated_by: lightning_ops_optional_string(&args, "updatedBy"),
                updated_at_ms: now_ms,
            };

            state
                .lightning_ops_control_plane
                .store
                .lock()
                .await
                .global_security = Some(global.clone());

            Ok((
                StatusCode::OK,
                Json(serde_json::json!({
                    "ok": true,
                    "global": lightning_ops_global_security_payload(&global),
                })),
            ))
        }
        "lightning/security:setOwnerKillSwitch" => {
            let owner_id = lightning_ops_required_string(&args, "ownerId")?;
            let active = lightning_ops_bool(&args, "active");
            let now = Utc::now();
            let now_ms = now.timestamp_millis();
            let owner_control = LightningOpsOwnerControlRecord {
                owner_id: owner_id.clone(),
                kill_switch: active,
                deny_reason_code: if active {
                    Some("owner_kill_switch_active".to_string())
                } else {
                    None
                },
                deny_reason: if active {
                    Some(
                        lightning_ops_optional_string(&args, "reason")
                            .unwrap_or_else(|| "Owner kill switch is active".to_string()),
                    )
                } else {
                    None
                },
                updated_by: lightning_ops_optional_string(&args, "updatedBy"),
                updated_at_ms: now_ms,
            };

            state
                .lightning_ops_control_plane
                .store
                .lock()
                .await
                .owner_controls
                .insert(owner_id, owner_control.clone());

            Ok((
                StatusCode::OK,
                Json(serde_json::json!({
                    "ok": true,
                    "ownerControl": lightning_ops_owner_control_payload(&owner_control),
                })),
            ))
        }
        "lightning/security:rotateCredentialRole" => {
            let role = lightning_ops_required_string(&args, "role")?;
            let now = Utc::now();
            let now_ms = now.timestamp_millis();
            let mut store = state.lightning_ops_control_plane.store.lock().await;
            let next_version = store
                .credential_roles
                .get(&role)
                .map(|row| row.version.saturating_add(1).max(1))
                .unwrap_or(1);

            let record = LightningOpsCredentialRoleRecord {
                role: role.clone(),
                status: "rotating".to_string(),
                version: next_version,
                fingerprint: lightning_ops_optional_string(&args, "fingerprint"),
                note: lightning_ops_optional_string(&args, "note"),
                updated_at_ms: now_ms,
                last_rotated_at_ms: Some(now_ms),
                revoked_at_ms: None,
            };
            store.credential_roles.insert(role, record.clone());

            Ok((
                StatusCode::OK,
                Json(serde_json::json!({
                    "ok": true,
                    "role": lightning_ops_credential_role_payload(&record),
                })),
            ))
        }
        "lightning/security:activateCredentialRole" => {
            let role = lightning_ops_required_string(&args, "role")?;
            let now = Utc::now();
            let now_ms = now.timestamp_millis();
            let mut store = state.lightning_ops_control_plane.store.lock().await;
            let next_version = if let Some(existing) = store.credential_roles.get(&role) {
                if existing.status == "rotating" {
                    existing.version.max(1)
                } else {
                    existing.version.saturating_add(1).max(1)
                }
            } else {
                1
            };

            let record = LightningOpsCredentialRoleRecord {
                role: role.clone(),
                status: "active".to_string(),
                version: next_version,
                fingerprint: lightning_ops_optional_string(&args, "fingerprint"),
                note: lightning_ops_optional_string(&args, "note"),
                updated_at_ms: now_ms,
                last_rotated_at_ms: Some(now_ms),
                revoked_at_ms: None,
            };
            store.credential_roles.insert(role, record.clone());

            Ok((
                StatusCode::OK,
                Json(serde_json::json!({
                    "ok": true,
                    "role": lightning_ops_credential_role_payload(&record),
                })),
            ))
        }
        "lightning/security:revokeCredentialRole" => {
            let role = lightning_ops_required_string(&args, "role")?;
            let now = Utc::now();
            let now_ms = now.timestamp_millis();
            let mut store = state.lightning_ops_control_plane.store.lock().await;
            let version = store
                .credential_roles
                .get(&role)
                .map(|record| record.version.max(1))
                .unwrap_or(1);

            let record = LightningOpsCredentialRoleRecord {
                role: role.clone(),
                status: "revoked".to_string(),
                version,
                fingerprint: None,
                note: lightning_ops_optional_string(&args, "note"),
                updated_at_ms: now_ms,
                last_rotated_at_ms: None,
                revoked_at_ms: Some(now_ms),
            };
            store.credential_roles.insert(role, record.clone());

            Ok((
                StatusCode::OK,
                Json(serde_json::json!({
                    "ok": true,
                    "role": lightning_ops_credential_role_payload(&record),
                })),
            ))
        }
        _ => Err(lightning_ops_error(
            StatusCode::NOT_FOUND,
            "unsupported_function",
            format!("unsupported function: {function_name}"),
        )),
    }
}

fn lightning_ops_payload(
    payload: serde_json::Value,
) -> Result<
    (String, serde_json::Map<String, serde_json::Value>),
    (StatusCode, Json<serde_json::Value>),
> {
    let object = payload
        .as_object()
        .ok_or_else(|| lightning_ops_invalid_arguments("invalid_payload"))?;
    let function_name = object
        .get("functionName")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| lightning_ops_invalid_arguments("missing_functionName"))?
        .to_string();
    if function_name.chars().count() > 180 {
        return Err(lightning_ops_invalid_arguments("invalid_functionName"));
    }

    let args = object
        .get("args")
        .and_then(serde_json::Value::as_object)
        .cloned()
        .ok_or_else(|| lightning_ops_invalid_arguments("missing_args"))?;

    Ok((function_name, args))
}

fn lightning_ops_assert_secret(
    args: &mut serde_json::Map<String, serde_json::Value>,
) -> Result<(), (StatusCode, Json<serde_json::Value>)> {
    let expected = lightning_ops_expected_secret();
    let Some(expected) = expected else {
        return Err(lightning_ops_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "ops_secret_unconfigured",
            "lightning ops secret is not configured",
        ));
    };

    let provided = args
        .get("secret")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .unwrap_or_default()
        .to_string();
    if provided.is_empty() || provided != expected {
        return Err(lightning_ops_error(
            StatusCode::UNAUTHORIZED,
            "invalid_ops_secret",
            "invalid lightning ops secret",
        ));
    }
    args.remove("secret");
    Ok(())
}

fn lightning_ops_expected_secret() -> Option<String> {
    std::env::var("OA_LIGHTNING_OPS_SECRET")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .or_else(|| {
            if cfg!(test) {
                Some("ops-secret-test".to_string())
            } else {
                None
            }
        })
}

fn lightning_ops_statuses(args: &serde_json::Map<String, serde_json::Value>) -> Vec<String> {
    let Some(values) = args.get("statuses").and_then(serde_json::Value::as_array) else {
        return vec!["active".to_string(), "paused".to_string()];
    };
    values
        .iter()
        .filter_map(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .collect()
}

fn lightning_ops_paywall_payload(paywall: &L402PaywallRecord) -> serde_json::Value {
    let status = if paywall.deleted_at.is_some() {
        "archived"
    } else if paywall.enabled {
        "active"
    } else {
        "paused"
    };
    let paywall_id = paywall.id.clone();
    let owner_id = format!("owner_{}", paywall.owner_user_id);
    let created_at_ms = paywall.created_at.timestamp_millis();
    let updated_at_ms = paywall.updated_at.timestamp_millis();
    let protocol = if paywall.upstream.to_ascii_lowercase().starts_with("http://") {
        "http"
    } else {
        "https"
    };
    let timeout_ms =
        lightning_ops_meta_i64(paywall.meta.as_ref(), "timeoutMs").unwrap_or_else(|| {
            std::env::var("OA_LIGHTNING_OPS_CONTROL_PLANE_TIMEOUT_MS")
                .ok()
                .and_then(|value| value.trim().parse::<i64>().ok())
                .filter(|value| *value > 0)
                .unwrap_or(6000)
        });
    let priority = lightning_ops_meta_i64(paywall.meta.as_ref(), "priority").unwrap_or_else(|| {
        std::env::var("OA_LIGHTNING_OPS_CONTROL_PLANE_PRIORITY")
            .ok()
            .and_then(|value| value.trim().parse::<i64>().ok())
            .filter(|value| *value >= 0)
            .unwrap_or(10)
    });

    let mut policy = serde_json::Map::new();
    policy.insert("paywallId".to_string(), serde_json::json!(paywall_id));
    policy.insert("ownerId".to_string(), serde_json::json!(owner_id));
    policy.insert("pricingMode".to_string(), serde_json::json!("fixed"));
    policy.insert(
        "fixedAmountMsats".to_string(),
        serde_json::json!(paywall.price_msats),
    );
    if let Some(value) = lightning_ops_meta_i64(paywall.meta.as_ref(), "maxPerRequestMsats") {
        policy.insert("maxPerRequestMsats".to_string(), serde_json::json!(value));
    }
    if let Some(value) = lightning_ops_meta_string_list(paywall.meta.as_ref(), "allowedHosts") {
        policy.insert("allowedHosts".to_string(), serde_json::json!(value));
    }
    if let Some(value) = lightning_ops_meta_string_list(paywall.meta.as_ref(), "blockedHosts") {
        policy.insert("blockedHosts".to_string(), serde_json::json!(value));
    }
    if let Some(value) = lightning_ops_meta_i64(paywall.meta.as_ref(), "quotaPerMinute") {
        policy.insert("quotaPerMinute".to_string(), serde_json::json!(value));
    }
    if let Some(value) = lightning_ops_meta_i64(paywall.meta.as_ref(), "quotaPerDay") {
        policy.insert("quotaPerDay".to_string(), serde_json::json!(value));
    }
    policy.insert("killSwitch".to_string(), serde_json::json!(false));
    policy.insert("createdAtMs".to_string(), serde_json::json!(created_at_ms));
    policy.insert("updatedAtMs".to_string(), serde_json::json!(updated_at_ms));

    serde_json::json!({
        "paywallId": paywall.id,
        "ownerId": format!("owner_{}", paywall.owner_user_id),
        "name": paywall.name,
        "status": status,
        "createdAtMs": created_at_ms,
        "updatedAtMs": updated_at_ms,
        "policy": policy,
        "routes": [
            {
                "routeId": format!("route_{}", paywall.id),
                "paywallId": paywall.id,
                "ownerId": format!("owner_{}", paywall.owner_user_id),
                "hostPattern": paywall.host_regexp,
                "pathPattern": paywall.path_regexp,
                "upstreamUrl": paywall.upstream,
                "protocol": protocol,
                "timeoutMs": timeout_ms,
                "priority": priority,
                "createdAtMs": created_at_ms,
                "updatedAtMs": updated_at_ms,
            }
        ]
    })
}

fn lightning_ops_deployment_payload(record: &LightningOpsDeploymentRecord) -> serde_json::Value {
    let mut payload = serde_json::json!({
        "deploymentId": record.deployment_id,
        "configHash": record.config_hash,
        "status": record.status,
        "createdAtMs": record.created_at.timestamp_millis(),
        "updatedAtMs": record.updated_at.timestamp_millis(),
    });
    if let Some(value) = record.paywall_id.as_ref() {
        payload["paywallId"] = serde_json::json!(value);
    }
    if let Some(value) = record.owner_id.as_ref() {
        payload["ownerId"] = serde_json::json!(value);
    }
    if let Some(value) = record.image_digest.as_ref() {
        payload["imageDigest"] = serde_json::json!(value);
    }
    if let Some(value) = record.diagnostics.as_ref() {
        payload["diagnostics"] = value.clone();
    }
    if let Some(value) = record.applied_at_ms {
        payload["appliedAtMs"] = serde_json::json!(value);
    }
    if let Some(value) = record.rolled_back_from.as_ref() {
        payload["rolledBackFrom"] = serde_json::json!(value);
    }
    payload
}

fn lightning_ops_gateway_event_payload(
    record: &LightningOpsGatewayEventRecord,
) -> serde_json::Value {
    let mut payload = serde_json::json!({
        "eventId": record.event_id,
        "paywallId": record.paywall_id,
        "ownerId": record.owner_id,
        "eventType": record.event_type,
        "level": record.level,
        "createdAtMs": record.created_at.timestamp_millis(),
    });
    if let Some(value) = record.request_id.as_ref() {
        payload["requestId"] = serde_json::json!(value);
    }
    if let Some(value) = record.metadata.as_ref() {
        payload["metadata"] = value.clone();
    }
    payload
}

fn lightning_ops_invoice_payload(record: &LightningOpsInvoiceRecord) -> serde_json::Value {
    let mut payload = serde_json::json!({
        "invoiceId": record.invoice_id,
        "paywallId": record.paywall_id,
        "ownerId": record.owner_id,
        "amountMsats": record.amount_msats,
        "status": record.status,
        "createdAtMs": record.created_at.timestamp_millis(),
        "updatedAtMs": record.updated_at.timestamp_millis(),
    });
    if let Some(value) = record.payment_hash.as_ref() {
        payload["paymentHash"] = serde_json::json!(value);
    }
    if let Some(value) = record.payment_request.as_ref() {
        payload["paymentRequest"] = serde_json::json!(value);
    }
    if let Some(value) = record.payment_proof_ref.as_ref() {
        payload["paymentProofRef"] = serde_json::json!(value);
    }
    if let Some(value) = record.request_id.as_ref() {
        payload["requestId"] = serde_json::json!(value);
    }
    if let Some(value) = record.settled_at_ms {
        payload["settledAtMs"] = serde_json::json!(value);
    }
    payload
}

fn lightning_ops_settlement_payload(record: &LightningOpsSettlementRecord) -> serde_json::Value {
    let mut payload = serde_json::json!({
        "settlementId": record.settlement_id,
        "paywallId": record.paywall_id,
        "ownerId": record.owner_id,
        "amountMsats": record.amount_msats,
        "paymentProofRef": record.payment_proof_ref,
        "createdAtMs": record.created_at.timestamp_millis(),
    });
    if let Some(value) = record.invoice_id.as_ref() {
        payload["invoiceId"] = serde_json::json!(value);
    }
    if let Some(value) = record.request_id.as_ref() {
        payload["requestId"] = serde_json::json!(value);
    }
    if let Some(value) = record.metadata.as_ref() {
        payload["metadata"] = value.clone();
    }
    payload
}

fn lightning_ops_global_security_payload(
    record: &LightningOpsGlobalSecurityRecord,
) -> serde_json::Value {
    let mut payload = serde_json::json!({
        "stateId": "global",
        "globalPause": record.global_pause,
        "updatedAtMs": record.updated_at_ms,
    });
    if let Some(value) = record.deny_reason_code.as_ref() {
        payload["denyReasonCode"] = serde_json::json!(value);
    }
    if let Some(value) = record.deny_reason.as_ref() {
        payload["denyReason"] = serde_json::json!(value);
    }
    if let Some(value) = record.updated_by.as_ref() {
        payload["updatedBy"] = serde_json::json!(value);
    }
    payload
}

fn lightning_ops_owner_control_payload(
    record: &LightningOpsOwnerControlRecord,
) -> serde_json::Value {
    let mut payload = serde_json::json!({
        "ownerId": record.owner_id,
        "killSwitch": record.kill_switch,
        "updatedAtMs": record.updated_at_ms,
    });
    if let Some(value) = record.deny_reason_code.as_ref() {
        payload["denyReasonCode"] = serde_json::json!(value);
    }
    if let Some(value) = record.deny_reason.as_ref() {
        payload["denyReason"] = serde_json::json!(value);
    }
    if let Some(value) = record.updated_by.as_ref() {
        payload["updatedBy"] = serde_json::json!(value);
    }
    payload
}

fn lightning_ops_credential_role_payload(
    record: &LightningOpsCredentialRoleRecord,
) -> serde_json::Value {
    let mut payload = serde_json::json!({
        "role": record.role,
        "status": record.status,
        "version": record.version,
        "updatedAtMs": record.updated_at_ms,
    });
    if let Some(value) = record.fingerprint.as_ref() {
        payload["fingerprint"] = serde_json::json!(value);
    }
    if let Some(value) = record.note.as_ref() {
        payload["note"] = serde_json::json!(value);
    }
    if let Some(value) = record.last_rotated_at_ms {
        payload["lastRotatedAtMs"] = serde_json::json!(value);
    }
    if let Some(value) = record.revoked_at_ms {
        payload["revokedAtMs"] = serde_json::json!(value);
    }
    payload
}

fn lightning_ops_invoice_lifecycle_input(
    args: &serde_json::Map<String, serde_json::Value>,
    status_override: Option<&str>,
) -> Result<LightningOpsInvoiceLifecycleInput, (StatusCode, Json<serde_json::Value>)> {
    let status = match status_override {
        Some(value) => value.to_string(),
        None => lightning_ops_required_string(args, "status")?,
    };
    if lightning_ops_invoice_status_rank(&status).is_none() {
        return Err(lightning_ops_invalid_arguments("invalid_invoice_status"));
    }

    Ok(LightningOpsInvoiceLifecycleInput {
        invoice_id: lightning_ops_required_string(args, "invoiceId")?,
        paywall_id: lightning_ops_required_string(args, "paywallId")?,
        owner_id: lightning_ops_required_string(args, "ownerId")?,
        amount_msats: lightning_ops_required_i64(args, "amountMsats")?,
        status,
        payment_hash: lightning_ops_optional_string(args, "paymentHash"),
        payment_request: lightning_ops_optional_string(args, "paymentRequest"),
        payment_proof_ref: lightning_ops_optional_string(args, "paymentProofRef"),
        request_id: lightning_ops_optional_string(args, "requestId"),
        settled_at_ms: lightning_ops_optional_i64(args, "settledAtMs"),
    })
}

fn lightning_ops_apply_invoice_lifecycle(
    store: &mut LightningOpsControlPlaneStore,
    input: LightningOpsInvoiceLifecycleInput,
) -> Result<(bool, LightningOpsInvoiceRecord), (StatusCode, Json<serde_json::Value>)> {
    let now = Utc::now();
    let now_ms = now.timestamp_millis();
    if let Some(existing) = store.invoices.get(&input.invoice_id).cloned() {
        let next_status = lightning_ops_choose_invoice_status(&existing.status, &input.status);
        let next_payment_hash = existing.payment_hash.clone().or(input.payment_hash.clone());
        let next_payment_request = existing
            .payment_request
            .clone()
            .or(input.payment_request.clone());
        let next_payment_proof_ref = existing
            .payment_proof_ref
            .clone()
            .or(input.payment_proof_ref.clone());
        let next_request_id = existing.request_id.clone().or(input.request_id.clone());
        let next_settled_at_ms = if next_status == "settled" {
            existing
                .settled_at_ms
                .or(input.settled_at_ms)
                .or(Some(now_ms))
        } else {
            existing.settled_at_ms
        };

        let changed = next_status != existing.status
            || input.amount_msats != existing.amount_msats
            || next_payment_hash != existing.payment_hash
            || next_payment_request != existing.payment_request
            || next_payment_proof_ref != existing.payment_proof_ref
            || next_request_id != existing.request_id
            || next_settled_at_ms != existing.settled_at_ms
            || input.paywall_id != existing.paywall_id
            || input.owner_id != existing.owner_id;

        let updated = LightningOpsInvoiceRecord {
            invoice_id: input.invoice_id.clone(),
            paywall_id: input.paywall_id,
            owner_id: input.owner_id,
            amount_msats: input.amount_msats,
            status: next_status,
            payment_hash: next_payment_hash,
            payment_request: next_payment_request,
            payment_proof_ref: next_payment_proof_ref,
            request_id: next_request_id,
            settled_at_ms: next_settled_at_ms,
            created_at: existing.created_at,
            updated_at: now,
        };
        store.invoices.insert(input.invoice_id, updated.clone());
        return Ok((changed, updated));
    }

    let settled_at_ms = if input.status == "settled" {
        input.settled_at_ms.or(Some(now_ms))
    } else {
        None
    };

    let created = LightningOpsInvoiceRecord {
        invoice_id: input.invoice_id.clone(),
        paywall_id: input.paywall_id,
        owner_id: input.owner_id,
        amount_msats: input.amount_msats,
        status: input.status,
        payment_hash: input.payment_hash,
        payment_request: input.payment_request,
        payment_proof_ref: input.payment_proof_ref,
        request_id: input.request_id,
        settled_at_ms,
        created_at: now,
        updated_at: now,
    };
    store.invoices.insert(input.invoice_id, created.clone());
    Ok((true, created))
}

fn lightning_ops_choose_invoice_status(current: &str, incoming: &str) -> String {
    let current_rank = lightning_ops_invoice_status_rank(current).unwrap_or(0);
    let incoming_rank = lightning_ops_invoice_status_rank(incoming).unwrap_or(0);
    if incoming_rank > current_rank {
        incoming.to_string()
    } else {
        current.to_string()
    }
}

fn lightning_ops_invoice_status_rank(status: &str) -> Option<i32> {
    match status {
        "open" => Some(0),
        "canceled" | "expired" => Some(1),
        "settled" => Some(2),
        _ => None,
    }
}

fn lightning_ops_required_string(
    args: &serde_json::Map<String, serde_json::Value>,
    field: &'static str,
) -> Result<String, (StatusCode, Json<serde_json::Value>)> {
    lightning_ops_optional_string(args, field)
        .ok_or_else(|| lightning_ops_invalid_arguments(format!("missing_{field}")))
}

fn lightning_ops_optional_string(
    args: &serde_json::Map<String, serde_json::Value>,
    field: &'static str,
) -> Option<String> {
    args.get(field)
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn lightning_ops_required_i64(
    args: &serde_json::Map<String, serde_json::Value>,
    field: &'static str,
) -> Result<i64, (StatusCode, Json<serde_json::Value>)> {
    lightning_ops_optional_i64(args, field)
        .ok_or_else(|| lightning_ops_invalid_arguments(format!("missing_{field}")))
}

fn lightning_ops_optional_i64(
    args: &serde_json::Map<String, serde_json::Value>,
    field: &'static str,
) -> Option<i64> {
    let value = args.get(field)?;
    match value {
        serde_json::Value::Number(number) => number
            .as_i64()
            .or_else(|| number.as_u64().and_then(|raw| i64::try_from(raw).ok())),
        serde_json::Value::String(raw) => raw.trim().parse::<i64>().ok(),
        _ => None,
    }
}

fn lightning_ops_optional_json(
    args: &serde_json::Map<String, serde_json::Value>,
    field: &'static str,
) -> Option<serde_json::Value> {
    args.get(field).and_then(|value| {
        if value.is_object() || value.is_array() {
            Some(value.clone())
        } else {
            None
        }
    })
}

fn lightning_ops_bool(
    args: &serde_json::Map<String, serde_json::Value>,
    field: &'static str,
) -> bool {
    args.get(field)
        .and_then(|value| match value {
            serde_json::Value::Bool(boolean) => Some(*boolean),
            serde_json::Value::Number(number) => number
                .as_i64()
                .map(|value| value != 0)
                .or_else(|| number.as_u64().map(|value| value != 0)),
            serde_json::Value::String(raw) => {
                let normalized = raw.trim().to_ascii_lowercase();
                if normalized.is_empty() || normalized == "0" {
                    Some(false)
                } else if matches!(normalized.as_str(), "false" | "off" | "no") {
                    Some(false)
                } else {
                    Some(true)
                }
            }
            _ => None,
        })
        .unwrap_or(false)
}

fn lightning_ops_meta_i64(meta: Option<&serde_json::Value>, key: &'static str) -> Option<i64> {
    let object = meta.and_then(serde_json::Value::as_object)?;
    match object.get(key) {
        Some(serde_json::Value::Number(number)) => number
            .as_i64()
            .or_else(|| number.as_u64().and_then(|raw| i64::try_from(raw).ok())),
        Some(serde_json::Value::String(raw)) => raw.trim().parse::<i64>().ok(),
        _ => None,
    }
}

fn lightning_ops_meta_string_list(
    meta: Option<&serde_json::Value>,
    key: &'static str,
) -> Option<Vec<String>> {
    let object = meta.and_then(serde_json::Value::as_object)?;
    let values = object.get(key)?.as_array()?;
    Some(
        values
            .iter()
            .filter_map(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string)
            .collect::<Vec<_>>(),
    )
}

fn lightning_ops_map_domain_store_error(
    error: DomainStoreError,
) -> (StatusCode, Json<serde_json::Value>) {
    match error {
        DomainStoreError::NotFound => lightning_ops_invalid_arguments("not_found"),
        DomainStoreError::Forbidden => lightning_ops_invalid_arguments("forbidden"),
        DomainStoreError::Validation { message, .. } => lightning_ops_invalid_arguments(message),
        DomainStoreError::Conflict { message } => lightning_ops_invalid_arguments(message),
        DomainStoreError::Persistence { message } => lightning_ops_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "service_unavailable",
            message,
        ),
    }
}

fn lightning_ops_invalid_arguments(
    message: impl Into<String>,
) -> (StatusCode, Json<serde_json::Value>) {
    lightning_ops_error(
        StatusCode::UNPROCESSABLE_ENTITY,
        "invalid_arguments",
        message.into(),
    )
}

fn lightning_ops_error(
    status: StatusCode,
    code: &'static str,
    message: impl Into<String>,
) -> (StatusCode, Json<serde_json::Value>) {
    (
        status,
        Json(serde_json::json!({
            "error": {
                "code": code,
                "message": message.into(),
            }
        })),
    )
}

async fn l402_wallet(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<L402AutopilotQuery>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiErrorResponse>)> {
    let bundle = session_bundle_from_headers(&state, &headers).await?;
    let autopilot_filter =
        resolve_l402_autopilot_filter(&state, &bundle.user.id, query.autopilot).await?;
    let autopilot_id = autopilot_filter.as_ref().map(|value| value.id.as_str());

    let receipts = state
        ._domain_store
        .list_l402_receipts_for_user(&bundle.user.id, autopilot_id, 200, 0)
        .await
        .map_err(map_domain_store_error)?
        .into_iter()
        .map(map_l402_receipt_row)
        .collect::<Vec<_>>();

    let total_paid_msats: i64 = receipts
        .iter()
        .filter(|receipt| receipt.paid)
        .filter_map(|receipt| receipt.amount_msats)
        .sum();

    let summary = serde_json::json!({
        "totalAttempts": receipts.len(),
        "paidCount": receipts.iter().filter(|receipt| receipt.paid).count(),
        "cachedCount": receipts
            .iter()
            .filter(|receipt| receipt.status == "cached" || receipt.cache_status.as_deref() == Some("hit"))
            .count(),
        "blockedCount": receipts.iter().filter(|receipt| receipt.status == "blocked").count(),
        "failedCount": receipts.iter().filter(|receipt| receipt.status == "failed").count(),
        "totalPaidMsats": total_paid_msats,
        "totalPaidSats": l402_msats_to_sats(Some(total_paid_msats)).unwrap_or(0.0),
    });

    let last_paid = receipts
        .iter()
        .find(|receipt| receipt.paid)
        .map(l402_receipt_payload)
        .unwrap_or(serde_json::Value::Null);

    let recent = receipts
        .iter()
        .take(20)
        .map(l402_receipt_payload)
        .collect::<Vec<_>>();

    let spark_wallet = state
        ._domain_store
        .find_user_spark_wallet(&bundle.user.id)
        .await
        .map_err(map_domain_store_error)?;

    Ok(ok_data(serde_json::json!({
        "summary": summary,
        "lastPaid": last_paid,
        "recent": recent,
        "sparkWallet": l402_wallet_payload(spark_wallet.as_ref()),
        "settings": l402_settings_payload(),
        "filter": {
            "autopilot": l402_filter_payload(autopilot_filter.as_ref())
        }
    })))
}

async fn l402_transactions(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<L402TransactionsQuery>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiErrorResponse>)> {
    let bundle = session_bundle_from_headers(&state, &headers).await?;
    let autopilot_filter =
        resolve_l402_autopilot_filter(&state, &bundle.user.id, query.autopilot).await?;
    let autopilot_id = autopilot_filter.as_ref().map(|value| value.id.as_str());

    let per_page = query.per_page.unwrap_or(30).clamp(1, 200);
    let page = query.page.unwrap_or(1).max(1);
    let offset = page.saturating_sub(1).saturating_mul(per_page);

    let total = state
        ._domain_store
        .count_l402_receipts_for_user(&bundle.user.id, autopilot_id)
        .await
        .map_err(map_domain_store_error)?;
    let rows = state
        ._domain_store
        .list_l402_receipts_for_user(&bundle.user.id, autopilot_id, per_page, offset)
        .await
        .map_err(map_domain_store_error)?
        .into_iter()
        .map(map_l402_receipt_row)
        .collect::<Vec<_>>();

    let last_page = if total == 0 {
        1
    } else {
        ((total - 1) / per_page as u64) + 1
    };

    Ok(ok_data(serde_json::json!({
        "transactions": rows.iter().map(l402_receipt_payload).collect::<Vec<_>>(),
        "pagination": {
            "currentPage": page,
            "lastPage": last_page,
            "perPage": per_page,
            "total": total,
            "hasMorePages": (page as u64) < last_page,
        },
        "filter": {
            "autopilot": l402_filter_payload(autopilot_filter.as_ref())
        }
    })))
}

async fn l402_transaction_show(
    State(state): State<AppState>,
    Path(event_id): Path<String>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiErrorResponse>)> {
    let bundle = session_bundle_from_headers(&state, &headers).await?;
    let normalized_event_id = event_id
        .trim()
        .parse::<u64>()
        .map_err(|_| not_found_error("Not found."))?;

    let row = state
        ._domain_store
        .find_l402_receipt_for_user(&bundle.user.id, normalized_event_id)
        .await
        .map_err(map_domain_store_error)?
        .ok_or_else(|| not_found_error("Not found."))?;

    Ok(ok_data(serde_json::json!({
        "transaction": l402_receipt_payload(&map_l402_receipt_row(row))
    })))
}

async fn l402_paywalls(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<L402AutopilotQuery>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiErrorResponse>)> {
    let bundle = session_bundle_from_headers(&state, &headers).await?;
    let autopilot_filter =
        resolve_l402_autopilot_filter(&state, &bundle.user.id, query.autopilot).await?;
    let autopilot_id = autopilot_filter.as_ref().map(|value| value.id.as_str());

    let receipts = state
        ._domain_store
        .list_l402_receipts_for_user(&bundle.user.id, autopilot_id, 500, 0)
        .await
        .map_err(map_domain_store_error)?
        .into_iter()
        .map(map_l402_receipt_row)
        .collect::<Vec<_>>();

    let mut grouped: HashMap<String, Vec<L402ReceiptView>> = HashMap::new();
    for receipt in receipts.iter().cloned() {
        let scope = receipt.scope.clone().unwrap_or_default();
        let key = format!("{}|{}", receipt.host, scope);
        grouped.entry(key).or_default().push(receipt);
    }

    let mut paywalls = grouped
        .into_iter()
        .map(|(key, mut items)| {
            let mut parts = key.splitn(2, '|');
            let host = parts.next().unwrap_or("unknown").to_string();
            let scope = parts.next().unwrap_or_default().to_string();
            items.sort_by(|left, right| right.created_at.cmp(&left.created_at));

            let total_paid_msats: i64 = items
                .iter()
                .filter(|receipt| receipt.paid)
                .filter_map(|receipt| receipt.amount_msats)
                .sum();

            let last = items.first();
            serde_json::json!({
                "host": host,
                "scope": scope,
                "attempts": items.len(),
                "paid": items.iter().filter(|receipt| receipt.paid).count(),
                "cached": items
                    .iter()
                    .filter(|receipt| receipt.status == "cached" || receipt.cache_status.as_deref() == Some("hit"))
                    .count(),
                "blocked": items.iter().filter(|receipt| receipt.status == "blocked").count(),
                "failed": items.iter().filter(|receipt| receipt.status == "failed").count(),
                "totalPaidMsats": total_paid_msats,
                "totalPaidSats": l402_msats_to_sats(Some(total_paid_msats)).unwrap_or(0.0),
                "lastAttemptAt": last.map(|receipt| receipt.created_at.clone()),
                "lastStatus": last
                    .map(|receipt| receipt.status.clone())
                    .unwrap_or_else(|| "unknown".to_string()),
            })
        })
        .collect::<Vec<_>>();

    paywalls.sort_by(|left, right| {
        let right_value = right
            .get("lastAttemptAt")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        let left_value = left
            .get("lastAttemptAt")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        right_value.cmp(left_value)
    });

    Ok(ok_data(serde_json::json!({
        "paywalls": paywalls,
        "summary": {
            "uniqueTargets": receipts
                .iter()
                .map(|receipt| format!("{}|{}", receipt.host, receipt.scope.clone().unwrap_or_default()))
                .collect::<std::collections::HashSet<_>>()
                .len(),
            "totalAttempts": receipts.len(),
            "totalPaidCount": receipts.iter().filter(|receipt| receipt.paid).count(),
        },
        "filter": {
            "autopilot": l402_filter_payload(autopilot_filter.as_ref())
        }
    })))
}

async fn l402_settlements(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<L402AutopilotQuery>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiErrorResponse>)> {
    let bundle = session_bundle_from_headers(&state, &headers).await?;
    let autopilot_filter =
        resolve_l402_autopilot_filter(&state, &bundle.user.id, query.autopilot).await?;
    let autopilot_id = autopilot_filter.as_ref().map(|value| value.id.as_str());

    let receipts = state
        ._domain_store
        .list_l402_receipts_for_user(&bundle.user.id, autopilot_id, 500, 0)
        .await
        .map_err(map_domain_store_error)?
        .into_iter()
        .map(map_l402_receipt_row)
        .collect::<Vec<_>>();

    let settlements = receipts
        .iter()
        .filter(|receipt| receipt.paid)
        .cloned()
        .collect::<Vec<_>>();

    let mut daily_map: HashMap<String, Vec<L402ReceiptView>> = HashMap::new();
    for receipt in settlements.iter().cloned() {
        let date = receipt.created_at.chars().take(10).collect::<String>();
        daily_map.entry(date).or_default().push(receipt);
    }

    let mut daily = daily_map
        .into_iter()
        .map(|(date, items)| {
            let total_msats: i64 = items
                .iter()
                .filter_map(|receipt| receipt.amount_msats)
                .sum();
            serde_json::json!({
                "date": date,
                "count": items.len(),
                "totalMsats": total_msats,
                "totalSats": l402_msats_to_sats(Some(total_msats)).unwrap_or(0.0),
            })
        })
        .collect::<Vec<_>>();
    daily.sort_by(|left, right| {
        right
            .get("date")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .cmp(
                left.get("date")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or_default(),
            )
    });

    let total_msats: i64 = settlements
        .iter()
        .filter_map(|receipt| receipt.amount_msats)
        .sum();

    Ok(ok_data(serde_json::json!({
        "summary": {
            "settledCount": settlements.len(),
            "totalMsats": total_msats,
            "totalSats": l402_msats_to_sats(Some(total_msats)).unwrap_or(0.0),
            "latestSettlementAt": settlements.first().map(|receipt| receipt.created_at.clone()),
        },
        "daily": daily,
        "settlements": settlements.iter().take(100).map(l402_receipt_payload).collect::<Vec<_>>(),
        "filter": {
            "autopilot": l402_filter_payload(autopilot_filter.as_ref())
        }
    })))
}

async fn l402_deployments(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<L402AutopilotQuery>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiErrorResponse>)> {
    let bundle = session_bundle_from_headers(&state, &headers).await?;
    let autopilot_filter =
        resolve_l402_autopilot_filter(&state, &bundle.user.id, query.autopilot).await?;
    let autopilot_id = autopilot_filter.as_ref().map(|value| value.id.as_str());

    let allowed_types = [
        "l402_gateway_deployment",
        "l402_gateway_event",
        "l402_executor_heartbeat",
        "l402_paywall_created",
        "l402_paywall_updated",
        "l402_paywall_deleted",
    ];

    let deployments = state
        ._domain_store
        .list_l402_gateway_events_for_user(&bundle.user.id, autopilot_id, 100)
        .await
        .map_err(map_domain_store_error)?
        .into_iter()
        .filter(|event| allowed_types.contains(&event.event_type.as_str()))
        .map(l402_gateway_event_payload)
        .collect::<Vec<_>>();

    Ok(ok_data(serde_json::json!({
        "deployments": deployments,
        "configSnapshot": l402_deployments_config_snapshot_payload(),
        "filter": {
            "autopilot": l402_filter_payload(autopilot_filter.as_ref())
        }
    })))
}

async fn l402_paywall_create(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<L402PaywallCreateRequestPayload>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiErrorResponse>)> {
    let request_id = request_id(&headers);
    let bundle = session_bundle_from_headers(&state, &headers).await?;

    let name = validate_l402_required_string(payload.name, "name", 120)?;
    let host_regexp = validate_l402_regex_body(payload.host_regexp, "hostRegexp", false)?;
    let path_regexp = validate_l402_regex_body(payload.path_regexp, "pathRegexp", true)?;
    let price_msats = validate_l402_price_msats(payload.price_msats, "priceMsats")?;
    let upstream = validate_l402_upstream(payload.upstream, "upstream")?;
    let metadata = validate_optional_json_object_or_array(payload.meta, "metadata")?;

    let paywall = state
        ._domain_store
        .create_l402_paywall(CreateL402PaywallInput {
            owner_user_id: bundle.user.id.clone(),
            name,
            host_regexp,
            path_regexp,
            price_msats,
            upstream,
            enabled: payload.enabled,
            meta: metadata,
        })
        .await
        .map_err(map_domain_store_error)?;

    let mutation_event = state
        ._domain_store
        .record_l402_gateway_event(RecordL402GatewayEventInput {
            user_id: bundle.user.id.clone(),
            autopilot_id: None,
            event_type: "l402_paywall_created".to_string(),
            payload: serde_json::json!({
                "paywallId": paywall.id.clone(),
                "name": paywall.name.clone(),
                "priceMsats": paywall.price_msats,
                "enabled": paywall.enabled,
            }),
            created_at: Some(Utc::now()),
        })
        .await
        .map_err(map_domain_store_error)?;

    state.observability.audit(
        AuditEvent::new("l402.paywall.created", request_id.clone())
            .with_user_id(bundle.user.id.clone())
            .with_session_id(bundle.session.session_id.clone())
            .with_org_id(bundle.session.active_org_id.clone())
            .with_device_id(bundle.session.device_id.clone())
            .with_attribute("paywall_id", paywall.id.clone())
            .with_attribute("price_msats", paywall.price_msats.to_string())
            .with_attribute("enabled", paywall.enabled.to_string()),
    );
    state
        .observability
        .increment_counter("l402.paywall.created", &request_id);

    Ok((
        StatusCode::CREATED,
        Json(serde_json::json!({
            "data": {
                "paywall": l402_paywall_payload(&paywall),
                "deployment": l402_paywall_deployment_payload(&mutation_event),
                "mutationEventId": mutation_event.id,
            }
        })),
    ))
}

async fn l402_paywall_update(
    State(state): State<AppState>,
    Path(paywall_id): Path<String>,
    headers: HeaderMap,
    Json(payload): Json<L402PaywallUpdateRequestPayload>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiErrorResponse>)> {
    let request_id = request_id(&headers);
    let bundle = session_bundle_from_headers(&state, &headers).await?;
    let paywall_id = non_empty(paywall_id).ok_or_else(|| not_found_error("Not found."))?;

    let name = validate_l402_optional_mutable_string(payload.name, "name", 120)?;
    let host_regexp = validate_l402_optional_regex_body(payload.host_regexp, "hostRegexp", false)?;
    let path_regexp = validate_l402_optional_regex_body(payload.path_regexp, "pathRegexp", true)?;
    let price_msats = payload
        .price_msats
        .map(|value| validate_l402_price_msats(value, "priceMsats"))
        .transpose()?;
    let upstream = payload
        .upstream
        .map(|value| validate_l402_upstream(value, "upstream"))
        .transpose()?;
    let metadata = validate_optional_json_object_or_array(payload.meta, "metadata")?;

    if name.is_none()
        && host_regexp.is_none()
        && path_regexp.is_none()
        && price_msats.is_none()
        && upstream.is_none()
        && payload.enabled.is_none()
        && metadata.is_none()
    {
        return Err(validation_error(
            "payload",
            "At least one mutable paywall field must be provided.",
        ));
    }

    let paywall = state
        ._domain_store
        .update_owned_l402_paywall(
            &bundle.user.id,
            &paywall_id,
            UpdateL402PaywallInput {
                name,
                host_regexp,
                path_regexp,
                price_msats,
                upstream,
                enabled: payload.enabled,
                meta: metadata,
                last_reconcile_status: Some("applied".to_string()),
                last_reconcile_error: Some(String::new()),
                last_reconciled_at: Some(Utc::now()),
            },
        )
        .await
        .map_err(map_domain_store_error)?;

    let mutation_event = state
        ._domain_store
        .record_l402_gateway_event(RecordL402GatewayEventInput {
            user_id: bundle.user.id.clone(),
            autopilot_id: None,
            event_type: "l402_paywall_updated".to_string(),
            payload: serde_json::json!({
                "paywallId": paywall.id.clone(),
                "priceMsats": paywall.price_msats,
                "enabled": paywall.enabled,
            }),
            created_at: Some(Utc::now()),
        })
        .await
        .map_err(map_domain_store_error)?;

    state.observability.audit(
        AuditEvent::new("l402.paywall.updated", request_id.clone())
            .with_user_id(bundle.user.id.clone())
            .with_session_id(bundle.session.session_id.clone())
            .with_org_id(bundle.session.active_org_id.clone())
            .with_device_id(bundle.session.device_id.clone())
            .with_attribute("paywall_id", paywall.id.clone())
            .with_attribute("mutation_event_id", mutation_event.id.to_string()),
    );
    state
        .observability
        .increment_counter("l402.paywall.updated", &request_id);

    Ok(ok_data(serde_json::json!({
        "paywall": l402_paywall_payload(&paywall),
        "deployment": l402_paywall_deployment_payload(&mutation_event),
        "mutationEventId": mutation_event.id,
    })))
}

async fn l402_paywall_delete(
    State(state): State<AppState>,
    Path(paywall_id): Path<String>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiErrorResponse>)> {
    let request_id = request_id(&headers);
    let bundle = session_bundle_from_headers(&state, &headers).await?;
    let paywall_id = non_empty(paywall_id).ok_or_else(|| not_found_error("Not found."))?;

    let paywall = state
        ._domain_store
        .soft_delete_owned_l402_paywall(&bundle.user.id, &paywall_id)
        .await
        .map_err(map_domain_store_error)?;

    let mutation_event = state
        ._domain_store
        .record_l402_gateway_event(RecordL402GatewayEventInput {
            user_id: bundle.user.id.clone(),
            autopilot_id: None,
            event_type: "l402_paywall_deleted".to_string(),
            payload: serde_json::json!({
                "paywallId": paywall.id.clone(),
                "deletedAt": paywall.deleted_at.map(timestamp),
            }),
            created_at: Some(Utc::now()),
        })
        .await
        .map_err(map_domain_store_error)?;

    state.observability.audit(
        AuditEvent::new("l402.paywall.deleted", request_id.clone())
            .with_user_id(bundle.user.id.clone())
            .with_session_id(bundle.session.session_id.clone())
            .with_org_id(bundle.session.active_org_id.clone())
            .with_device_id(bundle.session.device_id.clone())
            .with_attribute("paywall_id", paywall.id.clone())
            .with_attribute("mutation_event_id", mutation_event.id.to_string()),
    );
    state
        .observability
        .increment_counter("l402.paywall.deleted", &request_id);

    Ok(ok_data(serde_json::json!({
        "deleted": true,
        "paywall": l402_paywall_payload(&paywall),
        "deployment": l402_paywall_deployment_payload(&mutation_event),
        "mutationEventId": mutation_event.id,
    })))
}

fn l402_paywall_payload(paywall: &L402PaywallRecord) -> serde_json::Value {
    let metadata = paywall
        .meta
        .clone()
        .unwrap_or_else(|| serde_json::Value::Array(Vec::new()));

    serde_json::json!({
        "id": paywall.id,
        "ownerUserId": paywall.owner_user_id,
        "name": paywall.name,
        "hostRegexp": paywall.host_regexp,
        "pathRegexp": paywall.path_regexp,
        "priceMsats": paywall.price_msats,
        "upstream": paywall.upstream,
        "enabled": paywall.enabled,
        "metadata": metadata,
        "lastReconcileStatus": paywall.last_reconcile_status,
        "lastReconcileError": paywall.last_reconcile_error,
        "lastReconciledAt": paywall.last_reconciled_at.map(timestamp),
        "createdAt": timestamp(paywall.created_at),
        "updatedAt": timestamp(paywall.updated_at),
        "deletedAt": paywall.deleted_at.map(timestamp),
    })
}

fn l402_paywall_deployment_payload(event: &L402GatewayEventRecord) -> serde_json::Value {
    serde_json::json!({
        "status": "applied",
        "eventType": event.event_type,
        "eventId": event.id,
        "reverted": false,
    })
}

fn validate_l402_required_string(
    value: String,
    field: &'static str,
    max_chars: usize,
) -> Result<String, (StatusCode, Json<ApiErrorResponse>)> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(validation_error(field, "The field is required."));
    }
    if trimmed.chars().count() > max_chars {
        return Err(validation_error(
            field,
            &format!("Value may not be greater than {max_chars} characters."),
        ));
    }
    Ok(trimmed.to_string())
}

fn validate_l402_optional_mutable_string(
    value: Option<String>,
    field: &'static str,
    max_chars: usize,
) -> Result<Option<String>, (StatusCode, Json<ApiErrorResponse>)> {
    let Some(value) = value else {
        return Ok(None);
    };

    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(validation_error(
            field,
            "The field is required when present.",
        ));
    }
    if trimmed.chars().count() > max_chars {
        return Err(validation_error(
            field,
            &format!("Value may not be greater than {max_chars} characters."),
        ));
    }

    Ok(Some(trimmed.to_string()))
}

fn validate_l402_regex_body(
    value: String,
    field: &'static str,
    must_start_with_path_anchor: bool,
) -> Result<String, (StatusCode, Json<ApiErrorResponse>)> {
    let normalized = validate_l402_required_string(value, field, 255)?;

    if must_start_with_path_anchor && !normalized.starts_with("^/") {
        return Err(validation_error(
            field,
            "The field must start with '^/' to scope path matching.",
        ));
    }

    if !must_start_with_path_anchor && !normalized.contains('.') && !normalized.contains("\\.") {
        return Err(validation_error(
            field,
            "The field must include an explicit host pattern.",
        ));
    }

    Ok(normalized)
}

fn validate_l402_optional_regex_body(
    value: Option<String>,
    field: &'static str,
    must_start_with_path_anchor: bool,
) -> Result<Option<String>, (StatusCode, Json<ApiErrorResponse>)> {
    let Some(value) = value else {
        return Ok(None);
    };
    validate_l402_regex_body(value, field, must_start_with_path_anchor).map(Some)
}

fn validate_l402_price_msats(
    value: u64,
    field: &'static str,
) -> Result<u64, (StatusCode, Json<ApiErrorResponse>)> {
    const MAX_PRICE_MSATS: u64 = 1_000_000_000_000;
    if value < 1 {
        return Err(validation_error(field, "The value must be at least 1."));
    }
    if value > MAX_PRICE_MSATS {
        return Err(validation_error(
            field,
            "The value may not be greater than 1000000000000.",
        ));
    }
    Ok(value)
}

fn validate_l402_upstream(
    value: String,
    field: &'static str,
) -> Result<String, (StatusCode, Json<ApiErrorResponse>)> {
    let normalized = validate_l402_required_string(value, field, 2048)?;
    let parsed = reqwest::Url::parse(&normalized)
        .map_err(|_| validation_error(field, "The field must be a valid URL."))?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err(validation_error(
            field,
            "The field must start with http:// or https://.",
        ));
    }
    let host = parsed
        .host_str()
        .ok_or_else(|| validation_error(field, "The field must include a valid host."))?;
    if l402_upstream_host_is_forbidden(host) {
        return Err(validation_error(
            field,
            "The upstream host is not allowed for self-serve paywalls.",
        ));
    }
    Ok(normalized)
}

fn l402_upstream_host_is_forbidden(host: &str) -> bool {
    let normalized = host.trim().to_ascii_lowercase();
    if normalized.is_empty() || normalized == "localhost" || normalized.ends_with(".local") {
        return true;
    }

    let Ok(ip) = normalized.parse::<std::net::IpAddr>() else {
        return false;
    };
    match ip {
        std::net::IpAddr::V4(value) => {
            value.is_private()
                || value.is_loopback()
                || value.is_link_local()
                || value.is_broadcast()
                || value.is_documentation()
                || value.is_unspecified()
        }
        std::net::IpAddr::V6(value) => {
            value.is_loopback()
                || value.is_unspecified()
                || value.is_multicast()
                || value.is_unique_local()
                || value.is_unicast_link_local()
        }
    }
}

fn map_l402_receipt_row(row: L402ReceiptRecord) -> L402ReceiptView {
    let payload = if row.payload.is_object() {
        row.payload
    } else {
        serde_json::json!({})
    };

    let amount_msats = l402_optional_i64(payload.get("amountMsats"));
    let quoted_amount_msats = l402_optional_i64(payload.get("quotedAmountMsats"));
    let max_spend_msats = l402_optional_i64(payload.get("maxSpendMsats"));

    L402ReceiptView {
        event_id: row.id,
        thread_id: row.thread_id,
        thread_title: row
            .thread_title
            .unwrap_or_else(|| "Conversation".to_string()),
        run_id: row.run_id,
        run_status: row.run_status,
        run_started_at: row.run_started_at.map(timestamp),
        run_completed_at: row.run_completed_at.map(timestamp),
        created_at: timestamp(row.created_at),
        status: l402_optional_string(payload.get("status"))
            .unwrap_or_else(|| "unknown".to_string()),
        host: l402_optional_string(payload.get("host")).unwrap_or_else(|| "unknown".to_string()),
        scope: l402_optional_string(payload.get("scope")),
        paid: l402_optional_bool(payload.get("paid")).unwrap_or(false),
        cache_hit: l402_optional_bool(payload.get("cacheHit")).unwrap_or(false),
        cache_status: l402_optional_string(payload.get("cacheStatus")),
        amount_msats,
        quoted_amount_msats,
        max_spend_msats,
        proof_reference: l402_optional_string(payload.get("proofReference")),
        deny_code: l402_optional_string(payload.get("denyCode")),
        task_id: l402_optional_string(payload.get("taskId")),
        approval_required: l402_optional_bool(payload.get("approvalRequired")).unwrap_or(false),
        response_status_code: l402_optional_i64(payload.get("responseStatusCode")),
        response_body_sha256: l402_optional_string(payload.get("responseBodySha256")),
        tool_call_id: l402_optional_string(payload.get("tool_call_id"))
            .or_else(|| l402_optional_string(payload.get("toolCallId"))),
        raw_payload: payload,
    }
}

fn l402_receipt_payload(receipt: &L402ReceiptView) -> serde_json::Value {
    serde_json::json!({
        "eventId": receipt.event_id,
        "threadId": receipt.thread_id,
        "threadTitle": receipt.thread_title,
        "runId": receipt.run_id,
        "runStatus": receipt.run_status,
        "runStartedAt": receipt.run_started_at,
        "runCompletedAt": receipt.run_completed_at,
        "createdAt": receipt.created_at,
        "status": receipt.status,
        "host": receipt.host,
        "scope": receipt.scope,
        "paid": receipt.paid,
        "cacheHit": receipt.cache_hit,
        "cacheStatus": receipt.cache_status,
        "amountMsats": receipt.amount_msats,
        "amountSats": l402_msats_to_sats(receipt.amount_msats),
        "quotedAmountMsats": receipt.quoted_amount_msats,
        "quotedAmountSats": l402_msats_to_sats(receipt.quoted_amount_msats),
        "maxSpendMsats": receipt.max_spend_msats,
        "maxSpendSats": l402_msats_to_sats(receipt.max_spend_msats),
        "proofReference": receipt.proof_reference,
        "denyCode": receipt.deny_code,
        "taskId": receipt.task_id,
        "approvalRequired": receipt.approval_required,
        "responseStatusCode": receipt.response_status_code,
        "responseBodySha256": receipt.response_body_sha256,
        "toolCallId": receipt.tool_call_id,
        "rawPayload": receipt.raw_payload,
    })
}

fn l402_gateway_event_payload(event: L402GatewayEventRecord) -> serde_json::Value {
    let payload = if event.payload.is_object() {
        event.payload
    } else {
        serde_json::json!({})
    };

    serde_json::json!({
        "eventId": event.id,
        "type": event.event_type,
        "createdAt": timestamp(event.created_at),
        "payload": payload,
    })
}

fn l402_filter_payload(filter: Option<&L402AutopilotFilterValue>) -> serde_json::Value {
    filter
        .map(|value| {
            serde_json::json!({
                "id": value.id,
                "handle": value.handle,
            })
        })
        .unwrap_or(serde_json::Value::Null)
}

fn l402_wallet_payload(wallet: Option<&UserSparkWalletRecord>) -> serde_json::Value {
    wallet
        .map(|value| {
            serde_json::json!({
                "walletId": value.wallet_id,
                "sparkAddress": value.spark_address,
                "lightningAddress": value.lightning_address,
                "identityPubkey": value.identity_pubkey,
                "balanceSats": value.last_balance_sats,
                "status": value.status,
                "provider": value.provider,
                "lastError": value.last_error,
                "lastSyncedAt": value.last_synced_at.map(timestamp),
            })
        })
        .unwrap_or(serde_json::Value::Null)
}

fn l402_settings_payload() -> serde_json::Value {
    let invoice_payer = l402_invoice_payer_kind();
    let allowlist_hosts = l402_allowlist_hosts_from_env();
    serde_json::json!({
        "enforceHostAllowlist": env_bool("L402_ENFORCE_HOST_ALLOWLIST", false),
        "allowlistHosts": allowlist_hosts,
        "invoicePayer": invoice_payer,
        "credentialTtlSeconds": env_u64("L402_CREDENTIAL_TTL_SECONDS", 600),
        "paymentTimeoutMs": env_u64("L402_PAYMENT_TIMEOUT_MS", 12_000),
        "responseMaxBytes": env_u64("L402_RESPONSE_MAX_BYTES", 65_536),
        "responsePreviewBytes": env_u64("L402_RESPONSE_PREVIEW_BYTES", 1_024),
    })
}

fn l402_deployments_config_snapshot_payload() -> serde_json::Value {
    let invoice_payer = l402_invoice_payer_kind();
    let allowlist_hosts = l402_allowlist_hosts_from_env();
    serde_json::json!({
        "enforceHostAllowlist": env_bool("L402_ENFORCE_HOST_ALLOWLIST", false),
        "allowlistHosts": allowlist_hosts,
        "invoicePayer": invoice_payer,
        "credentialTtlSeconds": env_u64("L402_CREDENTIAL_TTL_SECONDS", 600),
        "paymentTimeoutMs": env_u64("L402_PAYMENT_TIMEOUT_MS", 12_000),
        "demoPresets": vec![
            "sats4ai",
            "ep212_openagents_premium",
            "ep212_openagents_expensive",
        ],
    })
}

fn l402_optional_string(value: Option<&serde_json::Value>) -> Option<String> {
    let value = value?.as_str()?.trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

fn l402_optional_i64(value: Option<&serde_json::Value>) -> Option<i64> {
    let value = value?;
    if let Some(int_value) = value.as_i64() {
        return Some(int_value);
    }
    if let Some(uint_value) = value.as_u64() {
        return i64::try_from(uint_value).ok();
    }
    value
        .as_str()
        .and_then(|raw| raw.trim().parse::<i64>().ok())
}

fn l402_optional_bool(value: Option<&serde_json::Value>) -> Option<bool> {
    let value = value?;
    if let Some(bool_value) = value.as_bool() {
        return Some(bool_value);
    }
    if let Some(int_value) = value.as_i64() {
        return match int_value {
            1 => Some(true),
            0 => Some(false),
            _ => None,
        };
    }
    value.as_str().and_then(|raw| match raw.trim() {
        "1" | "true" | "TRUE" => Some(true),
        "0" | "false" | "FALSE" => Some(false),
        _ => None,
    })
}

fn l402_msats_to_sats(msats: Option<i64>) -> Option<f64> {
    msats.map(|value| (value as f64 / 1000.0 * 1000.0).round() / 1000.0)
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

async fn legacy_chat_guest_session_retired(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response, (StatusCode, Json<ApiErrorResponse>)> {
    let audit_request_id = request_id(&headers);
    let access_token = access_token_from_headers(&headers).ok_or_else(|| {
        unauthorized_error("Codex chat requires an authenticated ChatGPT account.")
    })?;
    let session = state
        .auth
        .session_from_access_token(&access_token)
        .await
        .map_err(map_auth_error)?;

    state.observability.audit(
        AuditEvent::new(
            "legacy.chat.guest_session.retired",
            audit_request_id.clone(),
        )
        .with_user_id(session.user.id.clone())
        .with_session_id(session.session.session_id.clone())
        .with_org_id(session.session.active_org_id.clone())
        .with_device_id(session.session.device_id.clone())
        .with_attribute("canonical", "/api/runtime/threads".to_string())
        .with_attribute("auth_policy", "codex_auth_required".to_string()),
    );
    state
        .observability
        .increment_counter("legacy.chat.guest_session.retired", &audit_request_id);

    let mut response = legacy_chat_data_response(
        serde_json::json!({
            "retired": true,
            "status": "codex_auth_required",
            "canonical": "/api/runtime/threads",
            "message": "Guest chat is retired. Authenticate with a ChatGPT account to use Codex threads."
        }),
        StatusCode::GONE,
    );
    response.headers_mut().insert(
        "x-oa-chat-auth-policy",
        HeaderValue::from_static("codex-auth-required"),
    );
    Ok(response)
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

    let normalized_request =
        vercel_sse_adapter::normalize_legacy_stream_request(conversation_id.as_deref(), &payload)
            .map_err(map_legacy_stream_adapter_error)?;
    let thread_id = normalized_request.thread_id;
    let text = normalized_request.user_text;
    let worker_id = normalized_request.worker_id;
    validate_codex_turn_text(&text)?;
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
    let stream_preview = vercel_sse_adapter::build_turn_start_preview(
        &thread_id,
        bridge_response
            .get("turn")
            .and_then(|turn| turn.get("id"))
            .and_then(serde_json::Value::as_str),
    )
    .ok();
    let preview_event_count = stream_preview
        .as_ref()
        .map(|preview| preview.events.len())
        .unwrap_or(0);
    let preview_wire_bytes = stream_preview
        .as_ref()
        .map(|preview| preview.wire.len())
        .unwrap_or(0);

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
        .with_attribute("worker_id", worker_id_for_response.clone())
        .with_attribute("adapter_preview_events", preview_event_count.to_string())
        .with_attribute("adapter_preview_wire_bytes", preview_wire_bytes.to_string()),
    );
    state
        .observability
        .increment_counter("legacy.chat.stream.bridge.accepted", &audit_request_id);

    let wire = stream_preview
        .map(|preview| preview.wire)
        .unwrap_or_else(|| {
            vercel_sse_adapter::serialize_sse(&[
                serde_json::json!({
                    "type": "error",
                    "code": "adapter_preview_unavailable",
                    "message": "stream preview unavailable",
                    "retryable": false
                }),
                serde_json::json!({
                    "type": "finish",
                    "status": "error"
                }),
            ])
        });

    Ok(legacy_chat_stream_sse_response(&state.config, wire))
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
    (status, Json(serde_json::json!({ "data": payload }))).into_response()
}

fn legacy_chat_stream_sse_response(config: &Config, wire: String) -> Response {
    let mut response = (StatusCode::OK, Body::from(wire)).into_response();
    response.headers_mut().insert(
        CONTENT_TYPE,
        HeaderValue::from_static("text/event-stream; charset=utf-8"),
    );
    response.headers_mut().insert(
        CACHE_CONTROL,
        HeaderValue::from_static("no-cache, no-transform"),
    );
    response
        .headers_mut()
        .insert("connection", HeaderValue::from_static("keep-alive"));
    response
        .headers_mut()
        .insert("x-accel-buffering", HeaderValue::from_static("no"));
    response.headers_mut().insert(
        "x-vercel-ai-ui-message-stream",
        HeaderValue::from_static("v1"),
    );
    if let Ok(value) = HeaderValue::from_str(&config.compat_control_protocol_version) {
        response
            .headers_mut()
            .insert(HEADER_OA_PROTOCOL_VERSION, value);
    }
    if let Ok(value) = HeaderValue::from_str(&config.compat_control_min_client_build_id) {
        response
            .headers_mut()
            .insert(HEADER_OA_COMPAT_MIN_BUILD, value);
    }
    if let Some(max_build_id) = &config.compat_control_max_client_build_id
        && let Ok(value) = HeaderValue::from_str(max_build_id)
    {
        response
            .headers_mut()
            .insert(HEADER_OA_COMPAT_MAX_BUILD, value);
    }
    if let Ok(value) = HeaderValue::from_str(&config.compat_control_min_schema_version.to_string())
    {
        response
            .headers_mut()
            .insert(HEADER_OA_COMPAT_MIN_SCHEMA, value);
    }
    if let Ok(value) = HeaderValue::from_str(&config.compat_control_max_schema_version.to_string())
    {
        response
            .headers_mut()
            .insert(HEADER_OA_COMPAT_MAX_SCHEMA, value);
    }
    response
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

#[derive(Debug, Clone)]
struct CodingToolInvocation {
    integration_id: String,
    operation: String,
    repository: String,
    issue_number: Option<u64>,
    pull_number: Option<u64>,
    body: Option<String>,
    tool_call_id: Option<String>,
}

#[derive(Debug, Clone)]
struct L402ToolInvocation {
    operation: String,
    url: Option<String>,
    method: String,
    host: Option<String>,
    scope: Option<String>,
    request_headers: Option<serde_json::Value>,
    request_body: Option<serde_json::Value>,
    max_spend_msats: Option<u64>,
    require_approval: Option<bool>,
    timeout_ms: u64,
    task_id: Option<String>,
    autopilot_id: Option<String>,
    tool_call_id: Option<String>,
}

#[derive(Debug, Clone)]
struct L402ToolPolicy {
    require_approval: bool,
    max_spend_msats_per_call: Option<u64>,
    max_spend_msats_per_day: Option<u64>,
    allowed_hosts: Vec<String>,
    credential_ttl_seconds: i64,
}

#[derive(Debug, Clone)]
struct L402ToolExecutionResult {
    state: &'static str,
    decision: &'static str,
    reason_code: &'static str,
    result: serde_json::Value,
    error: Option<serde_json::Value>,
    receipt_status: String,
    paid: bool,
    cache_hit: bool,
    cache_status: Option<String>,
    amount_msats: Option<u64>,
    quoted_amount_msats: Option<u64>,
    max_spend_msats: Option<u64>,
    proof_reference: Option<String>,
    deny_code: Option<String>,
    task_id: Option<String>,
    approval_required: bool,
    response_status_code: Option<u16>,
    response_body_sha256: Option<String>,
}

#[derive(Debug)]
struct L402HttpResponseSnapshot {
    status: StatusCode,
    body: String,
    body_sha256: String,
    challenge_header: Option<String>,
}

#[derive(Debug, Clone)]
struct L402DailySpendSummary {
    spent_msats_today: u64,
}

async fn runtime_tools_execute(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<RuntimeToolsExecuteRequestPayload>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiErrorResponse>)> {
    let request_id = request_id(&headers);
    let access_token =
        bearer_token(&headers).ok_or_else(|| unauthorized_error("Unauthenticated."))?;
    let session = state
        .auth
        .session_from_access_token(&access_token)
        .await
        .map_err(map_auth_error)?;

    let raw_tool_pack = payload.tool_pack.trim();
    if raw_tool_pack.is_empty() {
        return Err(validation_error(
            "tool_pack",
            "The tool_pack field is required.",
        ));
    }
    if raw_tool_pack.chars().count() > 120 {
        return Err(validation_error(
            "tool_pack",
            "The tool_pack field may not be greater than 120 characters.",
        ));
    }
    let normalized_tool_pack = raw_tool_pack.to_ascii_lowercase();
    let canonical_tool_pack = match normalized_tool_pack.as_str() {
        RUNTIME_TOOL_PACK_CODING_V1 => RUNTIME_TOOL_PACK_CODING_V1,
        RUNTIME_TOOL_PACK_LIGHTNING_V1 | RUNTIME_TOOL_PACK_L402_ALIAS_V1 => {
            RUNTIME_TOOL_PACK_LIGHTNING_V1
        }
        _ => {
            return Err(validation_error(
                "tool_pack",
                "Only coding.v1 and lightning.v1 are currently supported.",
            ));
        }
    };

    let mode = payload
        .mode
        .and_then(non_empty)
        .unwrap_or_else(|| "execute".to_string())
        .to_lowercase();
    if !matches!(mode.as_str(), "execute" | "replay") {
        return Err(validation_error("mode", "The selected mode is invalid."));
    }

    let run_id = normalize_optional_bounded_string(payload.run_id, "run_id", 160)?;
    let thread_id = normalize_optional_bounded_string(payload.thread_id, "thread_id", 160)?;
    let requested_user_id = parse_optional_positive_u64(payload.user_id, "user_id")?;
    let authenticated_user_id = runtime_tools_principal_user_id(&session.user.id);

    if requested_user_id
        .map(|requested| requested != authenticated_user_id)
        .unwrap_or(false)
    {
        return Err(forbidden_error(
            "user_id does not match authenticated principal",
        ));
    }

    let manifest = validate_optional_json_object_or_array(payload.manifest, "manifest")?;
    let manifest_ref =
        validate_optional_json_object_or_array(payload.manifest_ref, "manifest_ref")?;
    if manifest.is_none() && manifest_ref.is_none() {
        return Err(error_response_with_status(
            StatusCode::UNPROCESSABLE_ENTITY,
            ApiErrorCode::InvalidRequest,
            "manifest or manifest_ref is required",
        ));
    }

    let mut request = payload.request;
    if !request.is_object() {
        return Err(validation_error(
            "request",
            "The request field must be an object.",
        ));
    }
    let policy = validate_optional_json_object_or_array(payload.policy, "policy")?
        .unwrap_or_else(|| serde_json::Value::Array(Vec::new()));

    {
        let request_object = request
            .as_object_mut()
            .ok_or_else(|| validation_error("request", "The request field must be an object."))?;
        request_object.insert(
            "user_id".to_string(),
            serde_json::json!(authenticated_user_id),
        );
        if let Some(run_id) = run_id.as_ref() {
            if !request_object.contains_key("run_id") {
                request_object.insert("run_id".to_string(), serde_json::json!(run_id));
            }
        }
        if let Some(thread_id) = thread_id.as_ref() {
            if !request_object.contains_key("thread_id") {
                request_object.insert("thread_id".to_string(), serde_json::json!(thread_id));
            }
        }
    }
    let request_object = request
        .as_object()
        .ok_or_else(|| validation_error("request", "The request field must be an object."))?;

    let receipt_seed = serde_json::json!({
        "tool_pack": canonical_tool_pack,
        "manifest": manifest.clone().unwrap_or_else(|| serde_json::Value::Array(Vec::new())),
        "manifest_ref": manifest_ref.clone().unwrap_or_else(|| serde_json::Value::Array(Vec::new())),
        "request": request,
        "policy": policy,
        "run_id": run_id.clone(),
        "thread_id": thread_id.clone(),
        "user_id": authenticated_user_id,
    });
    let replay_hash_hex = runtime_tools_replay_hash_hex(&receipt_seed);
    let replay_hash = format!("sha256:{replay_hash_hex}");
    let replay_key = format!("{authenticated_user_id}:{replay_hash_hex}");

    if let Some(replayed) = state
        .runtime_tool_receipts
        .entries
        .lock()
        .await
        .get(&replay_key)
        .cloned()
    {
        let replayed = mark_runtime_tools_replay(replayed);
        let replay_operation = replayed
            .get("request")
            .and_then(|request| request.get("operation"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_string();
        let replay_decision = replayed
            .get("decision")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("allowed")
            .to_string();
        let replay_reason = replayed
            .get("reason_code")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("replay")
            .to_string();
        state.observability.audit(
            AuditEvent::new("runtime.tools.execute.accepted", request_id.clone())
                .with_user_id(session.user.id.clone())
                .with_session_id(session.session.session_id.clone())
                .with_org_id(session.session.active_org_id.clone())
                .with_device_id(session.session.device_id.clone())
                .with_attribute("tool_pack", canonical_tool_pack.to_string())
                .with_attribute("operation", replay_operation)
                .with_attribute("mode", mode.clone())
                .with_attribute("idempotent_replay", "true".to_string())
                .with_attribute("decision", replay_decision)
                .with_attribute("reason_code", replay_reason),
        );
        state
            .observability
            .increment_counter("runtime.tools.execute.accepted", &request_id);
        return Ok(ok_data(replayed));
    }

    let effective_run_id = run_id
        .clone()
        .unwrap_or_else(|| format!("run_tools_{}", &replay_hash_hex[..16]));
    let effective_thread_id = thread_id
        .clone()
        .unwrap_or_else(|| format!("thread_tools_{}", &replay_hash_hex[..16]));

    let (response_payload, operation_for_audit, decision_for_audit, reason_for_audit) =
        if canonical_tool_pack == RUNTIME_TOOL_PACK_CODING_V1 {
            let invocation = parse_coding_tool_invocation(request_object)?;
            let evaluation = evaluate_coding_policy(&invocation, manifest.as_ref(), &policy);
            let execution_result =
                build_coding_execution_result(&invocation, &evaluation, &replay_hash_hex);
            let mut response_payload = serde_json::json!({
                "state": evaluation.state,
                "decision": evaluation.decision,
                "reason_code": evaluation.reason_code,
                "tool_pack": RUNTIME_TOOL_PACK_CODING_V1,
                "mode": mode.clone(),
                "idempotentReplay": false,
                "receipt": {
                    "receipt_id": format!("coding_{}", &replay_hash_hex[..24]),
                    "replay_hash": replay_hash,
                },
                "policy": {
                    "writeApproved": evaluation.write_approved,
                    "writeOperationsMode": evaluation.write_operations_mode,
                    "maxPerCallSats": evaluation.max_per_call_sats,
                    "operationCostSats": evaluation.operation_cost_sats,
                },
                "request": {
                    "integration_id": invocation.integration_id.clone(),
                    "operation": invocation.operation.clone(),
                    "repository": invocation.repository.clone(),
                    "issue_number": invocation.issue_number,
                    "pull_number": invocation.pull_number,
                    "tool_call_id": invocation.tool_call_id.clone(),
                    "run_id": run_id.clone(),
                    "thread_id": thread_id.clone(),
                    "user_id": authenticated_user_id,
                },
                "result": execution_result,
            });
            if let Some(message) = evaluation.denial_message.clone() {
                response_payload["error"] = serde_json::json!({
                    "code": "policy_denied",
                    "message": message,
                });
            }
            (
                response_payload,
                invocation.operation,
                evaluation.decision.to_string(),
                evaluation.reason_code.to_string(),
            )
        } else {
            let invocation = parse_l402_tool_invocation(request_object)?;
            let policy_settings = parse_l402_tool_policy(&policy);
            let operation_name = invocation.operation.clone();
            let tool_call_id = invocation.tool_call_id.clone();
            let execution = execute_l402_tool(
                &state,
                &session.user.id,
                &effective_run_id,
                &effective_thread_id,
                &replay_hash_hex,
                &policy,
                &invocation,
            )
            .await?;

            let mut receipt_payload = serde_json::json!({
                "status": execution.receipt_status,
                "host": invocation.host.clone(),
                "scope": invocation.scope.clone(),
                "paid": execution.paid,
                "cacheHit": execution.cache_hit,
                "cacheStatus": execution.cache_status,
                "amountMsats": execution.amount_msats,
                "quotedAmountMsats": execution.quoted_amount_msats,
                "maxSpendMsats": execution.max_spend_msats,
                "proofReference": execution.proof_reference,
                "denyCode": execution.deny_code,
                "taskId": execution.task_id.clone(),
                "approvalRequired": execution.approval_required,
                "responseStatusCode": execution.response_status_code,
                "responseBodySha256": execution.response_body_sha256,
                "toolCallId": tool_call_id,
                "toolPack": RUNTIME_TOOL_PACK_LIGHTNING_V1,
                "operation": operation_name.clone(),
                "result": execution.result.clone(),
            });
            if let Some(host) = execution
                .result
                .get("host")
                .and_then(serde_json::Value::as_str)
            {
                receipt_payload["host"] = serde_json::json!(host);
            }
            if let Some(scope) = execution
                .result
                .get("scope")
                .and_then(serde_json::Value::as_str)
            {
                receipt_payload["scope"] = serde_json::json!(scope);
            }

            let autopilot_id = invocation.autopilot_id.clone();
            state
                ._domain_store
                .record_l402_receipt(RecordL402ReceiptInput {
                    user_id: session.user.id.clone(),
                    thread_id: effective_thread_id.clone(),
                    run_id: effective_run_id.clone(),
                    autopilot_id,
                    thread_title: None,
                    run_status: Some(execution.state.to_string()),
                    run_started_at: Some(Utc::now()),
                    run_completed_at: Some(Utc::now()),
                    payload: receipt_payload,
                    created_at: Some(Utc::now()),
                })
                .await
                .map_err(map_domain_store_error)?;

            let mut response_payload = serde_json::json!({
                "state": execution.state,
                "decision": execution.decision,
                "reason_code": execution.reason_code,
                "tool_pack": RUNTIME_TOOL_PACK_LIGHTNING_V1,
                "mode": mode.clone(),
                "idempotentReplay": false,
                "receipt": {
                    "receipt_id": format!("l402_{}", &replay_hash_hex[..24]),
                    "replay_hash": replay_hash,
                },
                "policy": {
                    "requireApproval": policy_settings.require_approval,
                    "maxSpendMsatsPerCall": policy_settings.max_spend_msats_per_call,
                    "maxSpendMsatsPerDay": policy_settings.max_spend_msats_per_day,
                    "allowedHosts": policy_settings.allowed_hosts,
                },
                "request": {
                    "operation": operation_name.clone(),
                    "url": invocation.url,
                    "method": invocation.method,
                    "host": invocation.host,
                    "scope": invocation.scope,
                    "task_id": invocation.task_id,
                    "tool_call_id": invocation.tool_call_id,
                    "run_id": effective_run_id,
                    "thread_id": effective_thread_id,
                    "user_id": authenticated_user_id,
                },
                "result": execution.result,
            });
            if let Some(error) = execution.error {
                response_payload["error"] = error;
            }

            (
                response_payload,
                operation_name,
                execution.decision.to_string(),
                execution.reason_code.to_string(),
            )
        };

    state
        .runtime_tool_receipts
        .entries
        .lock()
        .await
        .insert(replay_key, response_payload.clone());

    state.observability.audit(
        AuditEvent::new("runtime.tools.execute.accepted", request_id.clone())
            .with_user_id(session.user.id.clone())
            .with_session_id(session.session.session_id.clone())
            .with_org_id(session.session.active_org_id.clone())
            .with_device_id(session.session.device_id.clone())
            .with_attribute("tool_pack", canonical_tool_pack.to_string())
            .with_attribute("operation", operation_for_audit)
            .with_attribute("mode", mode)
            .with_attribute("idempotent_replay", "false".to_string())
            .with_attribute("decision", decision_for_audit)
            .with_attribute("reason_code", reason_for_audit),
    );
    state
        .observability
        .increment_counter("runtime.tools.execute.accepted", &request_id);

    Ok(ok_data(response_payload))
}

async fn runtime_skill_tool_specs_list(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiErrorResponse>)> {
    let _ = session_bundle_from_headers(&state, &headers).await?;

    let mut by_key: HashMap<String, serde_json::Value> = HashMap::new();
    for spec in builtin_tool_specs() {
        let key = registry_item_key(&spec, "tool_id");
        by_key.insert(key, spec);
    }
    {
        let stored = state.runtime_skill_registry.tool_specs.lock().await;
        for (key, spec) in stored.iter() {
            by_key.insert(key.clone(), spec.clone());
        }
    }

    let mut payload = by_key.into_values().collect::<Vec<_>>();
    payload.sort_by_key(|item| registry_sort_key(item, "tool_id"));
    Ok(ok_data(payload))
}

async fn runtime_skill_tool_spec_store(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<RuntimeToolSpecUpsertRequestPayload>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiErrorResponse>)> {
    let request_id = request_id(&headers);
    let bundle = session_bundle_from_headers(&state, &headers).await?;
    let state_value = normalize_registry_state(payload.state, "state")?;
    let tool_spec = payload
        .tool_spec
        .as_object()
        .ok_or_else(|| validation_error("tool_spec", "The tool_spec field must be an array."))?;
    let (tool_id, version, tool_pack) = validate_tool_spec_schema(tool_spec)?;

    let now = timestamp(Utc::now());
    let key = registry_key(&tool_id, version);
    let existing_created_at = {
        let store = state.runtime_skill_registry.tool_specs.lock().await;
        store
            .get(&key)
            .and_then(|entry| entry.get("created_at"))
            .cloned()
            .unwrap_or_else(|| serde_json::json!(now.clone()))
    };

    let record = serde_json::json!({
        "tool_id": tool_id.clone(),
        "version": version,
        "tool_pack": tool_pack,
        "state": state_value.clone(),
        "tool_spec": payload.tool_spec,
        "created_at": existing_created_at,
        "updated_at": now,
    });

    state
        .runtime_skill_registry
        .tool_specs
        .lock()
        .await
        .insert(key, record.clone());

    state.observability.audit(
        AuditEvent::new("runtime.skills.tool_spec.upserted", request_id.clone())
            .with_user_id(bundle.user.id.clone())
            .with_session_id(bundle.session.session_id.clone())
            .with_org_id(bundle.session.active_org_id.clone())
            .with_device_id(bundle.session.device_id.clone())
            .with_attribute("tool_id", tool_id)
            .with_attribute("version", version.to_string())
            .with_attribute("state", state_value),
    );
    state
        .observability
        .increment_counter("runtime.skills.tool_spec.upserted", &request_id);

    Ok((
        StatusCode::CREATED,
        Json(serde_json::json!({ "data": record })),
    ))
}

async fn runtime_skill_specs_list(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiErrorResponse>)> {
    let _ = session_bundle_from_headers(&state, &headers).await?;

    let mut by_key: HashMap<String, serde_json::Value> = HashMap::new();
    for spec in builtin_skill_specs() {
        let key = registry_item_key(&spec, "skill_id");
        by_key.insert(key, spec);
    }
    {
        let stored = state.runtime_skill_registry.skill_specs.lock().await;
        for (key, spec) in stored.iter() {
            by_key.insert(key.clone(), spec.clone());
        }
    }

    let mut payload = by_key.into_values().collect::<Vec<_>>();
    payload.sort_by_key(|item| registry_sort_key(item, "skill_id"));
    Ok(ok_data(payload))
}

async fn runtime_skill_spec_store(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<RuntimeSkillSpecUpsertRequestPayload>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiErrorResponse>)> {
    let request_id = request_id(&headers);
    let bundle = session_bundle_from_headers(&state, &headers).await?;
    let state_value = normalize_registry_state(payload.state, "state")?;
    let skill_spec = payload
        .skill_spec
        .as_object()
        .ok_or_else(|| validation_error("skill_spec", "The skill_spec field must be an array."))?;
    let (skill_id, version) = validate_skill_spec_schema(skill_spec)?;

    let now = timestamp(Utc::now());
    let key = registry_key(&skill_id, version);
    let existing_created_at = {
        let store = state.runtime_skill_registry.skill_specs.lock().await;
        store
            .get(&key)
            .and_then(|entry| entry.get("created_at"))
            .cloned()
            .unwrap_or_else(|| serde_json::json!(now.clone()))
    };

    let record = serde_json::json!({
        "skill_id": skill_id.clone(),
        "version": version,
        "state": state_value.clone(),
        "skill_spec": payload.skill_spec,
        "created_at": existing_created_at,
        "updated_at": now,
    });

    state
        .runtime_skill_registry
        .skill_specs
        .lock()
        .await
        .insert(key, record.clone());

    state.observability.audit(
        AuditEvent::new("runtime.skills.skill_spec.upserted", request_id.clone())
            .with_user_id(bundle.user.id.clone())
            .with_session_id(bundle.session.session_id.clone())
            .with_org_id(bundle.session.active_org_id.clone())
            .with_device_id(bundle.session.device_id.clone())
            .with_attribute("skill_id", skill_id)
            .with_attribute("version", version.to_string())
            .with_attribute("state", state_value),
    );
    state
        .observability
        .increment_counter("runtime.skills.skill_spec.upserted", &request_id);

    Ok((
        StatusCode::CREATED,
        Json(serde_json::json!({ "data": record })),
    ))
}

async fn runtime_skill_spec_publish(
    State(state): State<AppState>,
    Path((skill_id, version)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiErrorResponse>)> {
    let request_id = request_id(&headers);
    let bundle = session_bundle_from_headers(&state, &headers).await?;
    let normalized_skill_id = skill_id.trim().to_string();
    if normalized_skill_id.is_empty() {
        return Err(validation_error(
            "skill_id",
            "The skill_id route parameter is required.",
        ));
    }
    let normalized_version = version.trim().parse::<u64>().map_err(|_| {
        validation_error("version", "The version route parameter must be an integer.")
    })?;
    if normalized_version < 1 {
        return Err(validation_error(
            "version",
            "The version route parameter must be at least 1.",
        ));
    }

    let key = registry_key(&normalized_skill_id, normalized_version);
    let mut skill_spec_row = {
        let mut skill_specs = state.runtime_skill_registry.skill_specs.lock().await;
        if !skill_specs.contains_key(&key) {
            if let Some(builtin) = builtin_skill_spec(&normalized_skill_id, normalized_version) {
                skill_specs.insert(key.clone(), builtin);
            }
        }
        let row = skill_specs
            .get(&key)
            .cloned()
            .ok_or_else(|| not_found_error("Not found."))?;
        if let Some(existing) = skill_specs.get_mut(&key) {
            existing["state"] = serde_json::json!("published");
            existing["updated_at"] = serde_json::json!(timestamp(Utc::now()));
        }
        row
    };

    let skill_spec_payload = skill_spec_row
        .get("skill_spec")
        .cloned()
        .unwrap_or_else(|| serde_json::Value::Object(serde_json::Map::new()));
    let bundle_hash = runtime_tools_replay_hash_hex(&skill_spec_payload);
    let now = timestamp(Utc::now());
    let release = serde_json::json!({
        "release_id": format!("skillrel_{}", &bundle_hash[..12]),
        "skill_id": normalized_skill_id.clone(),
        "version": normalized_version,
        "bundle_hash": bundle_hash,
        "published_at": now,
        "bundle": {
            "bundle_format": "agent_skills.v1",
            "skill_spec": skill_spec_payload,
        }
    });

    state
        .runtime_skill_registry
        .releases
        .lock()
        .await
        .insert(key, release.clone());

    skill_spec_row["state"] = serde_json::json!("published");

    state.observability.audit(
        AuditEvent::new("runtime.skills.skill_spec.published", request_id.clone())
            .with_user_id(bundle.user.id.clone())
            .with_session_id(bundle.session.session_id.clone())
            .with_org_id(bundle.session.active_org_id.clone())
            .with_device_id(bundle.session.device_id.clone())
            .with_attribute("skill_id", normalized_skill_id)
            .with_attribute("version", normalized_version.to_string())
            .with_attribute(
                "release_id",
                release["release_id"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string(),
            ),
    );
    state
        .observability
        .increment_counter("runtime.skills.skill_spec.published", &request_id);

    Ok((
        StatusCode::CREATED,
        Json(serde_json::json!({ "data": release })),
    ))
}

async fn runtime_skill_release_show(
    State(state): State<AppState>,
    Path((skill_id, version)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiErrorResponse>)> {
    let _ = session_bundle_from_headers(&state, &headers).await?;
    let normalized_skill_id = skill_id.trim().to_string();
    if normalized_skill_id.is_empty() {
        return Err(validation_error(
            "skill_id",
            "The skill_id route parameter is required.",
        ));
    }
    let normalized_version = version.trim().parse::<u64>().map_err(|_| {
        validation_error("version", "The version route parameter must be an integer.")
    })?;
    if normalized_version < 1 {
        return Err(validation_error(
            "version",
            "The version route parameter must be at least 1.",
        ));
    }

    let key = registry_key(&normalized_skill_id, normalized_version);
    let release = state
        .runtime_skill_registry
        .releases
        .lock()
        .await
        .get(&key)
        .cloned()
        .ok_or_else(|| not_found_error("Not found."))?;
    Ok(ok_data(release))
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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeWorkerRegisterRequestPayload {
    worker_id: Option<String>,
    workspace_ref: Option<String>,
    codex_home_ref: Option<String>,
    adapter: Option<String>,
    metadata: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeWorkerHeartbeatRequestPayload {
    metadata_patch: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeWorkerTransitionRequestPayload {
    status: String,
    reason: Option<String>,
}

fn runtime_internal_client(
    state: &AppState,
) -> Result<RuntimeInternalClient, (StatusCode, Json<ApiErrorResponse>)> {
    RuntimeInternalClient::from_base_url(
        state.config.runtime_base_url.as_deref(),
        COMPUTE_DASHBOARD_TIMEOUT_MS,
    )
    .map_err(|error| {
        tracing::warn!(error = %error, "runtime client unavailable");
        error_response_with_status(
            StatusCode::SERVICE_UNAVAILABLE,
            ApiErrorCode::ServiceUnavailable,
            "Runtime is not configured.",
        )
    })
}

fn runtime_client_service_error(
    action: &str,
    error: RuntimeClientError,
) -> (StatusCode, Json<ApiErrorResponse>) {
    error_response_with_status(
        StatusCode::SERVICE_UNAVAILABLE,
        ApiErrorCode::ServiceUnavailable,
        format!("Failed to {action}: {error}"),
    )
}

async fn runtime_workers_index(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiErrorResponse>)> {
    let bundle = session_bundle_from_headers(&state, &headers).await?;
    let client = runtime_internal_client(&state)?;
    let owner_user_id = runtime_tools_principal_user_id(&bundle.user.id);
    let response = client
        .list_workers_json(owner_user_id)
        .await
        .map_err(|error| runtime_client_service_error("fetch runtime workers", error))?;
    let workers = response
        .get("workers")
        .cloned()
        .filter(|value| value.is_array())
        .unwrap_or_else(|| serde_json::Value::Array(Vec::new()));
    Ok(ok_data(workers))
}

async fn runtime_workers_create(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<RuntimeWorkerRegisterRequestPayload>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiErrorResponse>)> {
    let bundle = session_bundle_from_headers(&state, &headers).await?;
    let client = runtime_internal_client(&state)?;

    let worker_id = normalize_optional_bounded_string(payload.worker_id, "worker_id", 160)?;
    let workspace_ref =
        normalize_optional_bounded_string(payload.workspace_ref, "workspace_ref", 255)?;
    let codex_home_ref =
        normalize_optional_bounded_string(payload.codex_home_ref, "codex_home_ref", 255)?;
    let adapter = normalize_optional_bounded_string(payload.adapter, "adapter", 120)?;
    let metadata = validate_optional_json_object_or_array(payload.metadata, "metadata")?
        .unwrap_or_else(|| serde_json::json!({}));
    let owner_user_id = runtime_tools_principal_user_id(&bundle.user.id);

    let request = RuntimeWorkerRegisterRequest {
        worker_id,
        owner_user_id,
        workspace_ref,
        codex_home_ref,
        adapter,
        metadata,
    };

    let response = client
        .register_worker(&request)
        .await
        .map_err(|error| runtime_client_service_error("register runtime worker", error))?;
    let snapshot = response
        .get("worker")
        .cloned()
        .unwrap_or_else(|| serde_json::Value::Null);
    Ok(ok_data(snapshot))
}

async fn runtime_worker_show(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(worker_id): Path<String>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiErrorResponse>)> {
    let bundle = session_bundle_from_headers(&state, &headers).await?;
    let client = runtime_internal_client(&state)?;

    let worker_id = normalize_optional_bounded_string(Some(worker_id), "worker_id", 160)?
        .unwrap_or_else(|| "worker".to_string());
    let owner_user_id = runtime_tools_principal_user_id(&bundle.user.id);
    let response = client
        .get_worker_json(&worker_id, owner_user_id)
        .await
        .map_err(|error| runtime_client_service_error("fetch runtime worker", error))?;
    let snapshot = response
        .get("worker")
        .cloned()
        .unwrap_or_else(|| serde_json::Value::Null);
    Ok(ok_data(snapshot))
}

async fn runtime_worker_heartbeat(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(worker_id): Path<String>,
    Json(payload): Json<RuntimeWorkerHeartbeatRequestPayload>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiErrorResponse>)> {
    let bundle = session_bundle_from_headers(&state, &headers).await?;
    let client = runtime_internal_client(&state)?;

    let worker_id = normalize_optional_bounded_string(Some(worker_id), "worker_id", 160)?
        .unwrap_or_else(|| "worker".to_string());
    let owner_user_id = runtime_tools_principal_user_id(&bundle.user.id);
    let metadata_patch =
        validate_optional_json_object_or_array(payload.metadata_patch, "metadataPatch")?
            .unwrap_or_else(|| serde_json::json!({}));

    let request = RuntimeWorkerHeartbeatRequest {
        owner_user_id,
        metadata_patch,
    };
    let response = client
        .heartbeat_worker(&worker_id, &request)
        .await
        .map_err(|error| runtime_client_service_error("heartbeat runtime worker", error))?;
    let snapshot = response
        .get("worker")
        .cloned()
        .unwrap_or_else(|| serde_json::Value::Null);
    Ok(ok_data(snapshot))
}

async fn runtime_worker_transition(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(worker_id): Path<String>,
    Json(payload): Json<RuntimeWorkerTransitionRequestPayload>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiErrorResponse>)> {
    let bundle = session_bundle_from_headers(&state, &headers).await?;
    let client = runtime_internal_client(&state)?;

    let normalized_status = payload.status.trim().to_lowercase();
    if !matches!(
        normalized_status.as_str(),
        "starting" | "running" | "stopping" | "stopped" | "failed"
    ) {
        return Err(validation_error(
            "status",
            "The selected status is invalid.",
        ));
    }
    let worker_id = normalize_optional_bounded_string(Some(worker_id), "worker_id", 160)?
        .unwrap_or_else(|| "worker".to_string());
    let owner_user_id = runtime_tools_principal_user_id(&bundle.user.id);
    let request = RuntimeWorkerTransitionRequest {
        owner_user_id,
        status: normalized_status,
        reason: payload.reason,
    };
    let response = client
        .transition_worker(&worker_id, &request)
        .await
        .map_err(|error| runtime_client_service_error("transition runtime worker", error))?;
    let snapshot = response
        .get("worker")
        .cloned()
        .unwrap_or_else(|| serde_json::Value::Null);
    Ok(ok_data(snapshot))
}

async fn runtime_codex_workers_index(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<RuntimeCodexWorkersListQuery>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiErrorResponse>)> {
    let bundle = session_bundle_from_headers(&state, &headers).await?;
    let status_filter = query
        .status
        .and_then(non_empty)
        .map(|value| value.to_lowercase());
    if status_filter
        .as_deref()
        .map(|value| {
            !matches!(
                value,
                "starting" | "running" | "stopping" | "stopped" | "failed"
            )
        })
        .unwrap_or(false)
    {
        return Err(validation_error(
            "status",
            "The selected status is invalid.",
        ));
    }
    let workspace_ref =
        normalize_optional_bounded_string(query.workspace_ref, "workspace_ref", 255)?;
    let limit = query.limit.unwrap_or(50).clamp(1, 200);

    let now = Utc::now();
    let workers = state.runtime_workers.workers.lock().await;
    let mut payload = workers
        .values()
        .filter(|worker| worker.owner_user_id == bundle.user.id)
        .filter(|worker| {
            status_filter
                .as_deref()
                .map(|status| worker.status == status)
                .unwrap_or(true)
        })
        .filter(|worker| {
            workspace_ref
                .as_deref()
                .map(|workspace| worker.workspace_ref.as_deref() == Some(workspace))
                .unwrap_or(true)
        })
        .map(|worker| runtime_worker_snapshot_payload(worker, now))
        .collect::<Vec<_>>();
    payload.sort_by(|left, right| {
        let left_updated = left
            .get("updated_at")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        let right_updated = right
            .get("updated_at")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        right_updated.cmp(left_updated)
    });
    payload.truncate(limit);

    Ok(ok_data(payload))
}

async fn runtime_codex_workers_create(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<RuntimeCodexWorkerCreateRequestPayload>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiErrorResponse>)> {
    let bundle = session_bundle_from_headers(&state, &headers).await?;
    let worker_id = normalize_optional_bounded_string(payload.worker_id, "worker_id", 160)?
        .unwrap_or_else(|| format!("codexw_{}", uuid::Uuid::new_v4().simple()));
    let workspace_ref =
        normalize_optional_bounded_string(payload.workspace_ref, "workspace_ref", 255)?;
    let codex_home_ref =
        normalize_optional_bounded_string(payload.codex_home_ref, "codex_home_ref", 255)?;
    let adapter = normalize_optional_bounded_string(payload.adapter, "adapter", 120)?
        .unwrap_or_else(|| "in_memory".to_string());
    let metadata = validate_optional_json_object_or_array(payload.metadata, "metadata")?;

    let now = Utc::now();
    let mut workers = state.runtime_workers.workers.lock().await;
    let (status, latest_seq, idempotent_replay) =
        if let Some(existing) = workers.get_mut(&worker_id) {
            if existing.owner_user_id != bundle.user.id {
                return Err(forbidden_error(
                    "worker does not belong to authenticated principal",
                ));
            }
            let replay = existing.status == "running";
            existing.status = "running".to_string();
            existing.workspace_ref = workspace_ref.clone().or(existing.workspace_ref.clone());
            existing.codex_home_ref = codex_home_ref.clone().or(existing.codex_home_ref.clone());
            existing.adapter = adapter.clone();
            if metadata.is_some() {
                existing.metadata = metadata.clone();
            }
            existing.stopped_at = None;
            existing.last_heartbeat_at = Some(now);
            existing.updated_at = now;
            (existing.status.clone(), existing.latest_seq, replay)
        } else {
            workers.insert(
                worker_id.clone(),
                RuntimeWorkerRecord {
                    worker_id: worker_id.clone(),
                    owner_user_id: bundle.user.id.clone(),
                    status: "running".to_string(),
                    latest_seq: 0,
                    workspace_ref,
                    codex_home_ref,
                    adapter,
                    metadata,
                    started_at: Some(now),
                    stopped_at: None,
                    last_heartbeat_at: Some(now),
                    heartbeat_stale_after_ms: 120_000,
                    updated_at: now,
                },
            );
            ("running".to_string(), 0, false)
        };

    Ok((
        StatusCode::ACCEPTED,
        Json(serde_json::json!({
            "data": {
                "workerId": worker_id,
                "status": status,
                "latestSeq": latest_seq,
                "idempotentReplay": idempotent_replay,
            }
        })),
    ))
}

async fn runtime_codex_worker_show(
    State(state): State<AppState>,
    Path(worker_id): Path<String>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiErrorResponse>)> {
    let bundle = session_bundle_from_headers(&state, &headers).await?;
    let normalized_worker_id = worker_id.trim().to_string();
    if normalized_worker_id.is_empty() {
        return Err(validation_error("worker_id", "Worker id is required."));
    }

    let now = Utc::now();
    let workers = state.runtime_workers.workers.lock().await;
    let worker = workers
        .get(&normalized_worker_id)
        .ok_or_else(|| not_found_error("Not found."))?;
    if worker.owner_user_id != bundle.user.id {
        return Err(forbidden_error(
            "worker does not belong to authenticated principal",
        ));
    }

    Ok(ok_data(runtime_worker_snapshot_payload(worker, now)))
}

async fn runtime_codex_worker_stream(
    State(state): State<AppState>,
    Path(worker_id): Path<String>,
    headers: HeaderMap,
    Query(query): Query<RuntimeCodexWorkerStreamQuery>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiErrorResponse>)> {
    let bundle = session_bundle_from_headers(&state, &headers).await?;
    let normalized_worker_id = worker_id.trim().to_string();
    if normalized_worker_id.is_empty() {
        return Err(validation_error("worker_id", "Worker id is required."));
    }
    if query
        .tail_ms
        .map(|tail_ms| !(1..=120_000).contains(&tail_ms))
        .unwrap_or(false)
    {
        return Err(validation_error(
            "tail_ms",
            "The tail_ms field must be between 1 and 120000.",
        ));
    }

    let now = Utc::now();
    let worker = {
        let workers = state.runtime_workers.workers.lock().await;
        let worker = workers
            .get(&normalized_worker_id)
            .cloned()
            .ok_or_else(|| not_found_error("Not found."))?;
        if worker.owner_user_id != bundle.user.id {
            return Err(forbidden_error(
                "worker does not belong to authenticated principal",
            ));
        }
        worker
    };

    let cursor = query.cursor.unwrap_or(0);
    let events = {
        let event_log = state.runtime_workers.events.lock().await;
        event_log
            .get(&normalized_worker_id)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .filter(|event| event.seq > cursor)
            .map(|event| runtime_worker_event_payload(&event))
            .collect::<Vec<_>>()
    };

    Ok(ok_data(serde_json::json!({
        "worker_id": normalized_worker_id,
        "stream_protocol": "spacetime_ws",
        "cursor": cursor,
        "tail_ms": query.tail_ms.unwrap_or(15_000),
        "delivery": {
            "transport": "spacetime_ws",
            "topic": org_worker_events_topic(&bundle.session.active_org_id),
            "scope": "runtime.codex_worker_events",
            "syncTokenRoute": ROUTE_SPACETIME_TOKEN,
        },
        "snapshot": runtime_worker_snapshot_payload(&worker, now),
        "events": events,
    })))
}

async fn runtime_codex_worker_events(
    State(state): State<AppState>,
    Path(worker_id): Path<String>,
    headers: HeaderMap,
    Json(payload): Json<RuntimeCodexWorkerEventEnvelope>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiErrorResponse>)> {
    let bundle = session_bundle_from_headers(&state, &headers).await?;
    let normalized_worker_id = worker_id.trim().to_string();
    if normalized_worker_id.is_empty() {
        return Err(validation_error("worker_id", "Worker id is required."));
    }

    let event_type = payload.event.event_type.trim().to_string();
    if event_type.is_empty() {
        return Err(validation_error(
            "event.event_type",
            "The event.event_type field is required.",
        ));
    }
    if event_type.chars().count() > 160 || !event_type.starts_with("worker.") {
        return Err(validation_error(
            "event.event_type",
            "The event.event_type field format is invalid.",
        ));
    }
    let event_payload = payload
        .event
        .payload
        .unwrap_or_else(|| serde_json::Value::Object(serde_json::Map::new()));
    if !event_payload.is_object() {
        return Err(validation_error(
            "event.payload",
            "The event.payload field must be an object.",
        ));
    }
    validate_worker_handshake_payload(&event_type, &event_payload)?;

    ensure_runtime_worker_runnable(&state, &bundle.user.id, &normalized_worker_id, true).await?;
    let occurred_at = parse_event_occurred_at(&event_payload).unwrap_or_else(Utc::now);
    let seq = append_runtime_worker_event(
        &state,
        &normalized_worker_id,
        &event_type,
        event_payload.clone(),
        occurred_at,
    )
    .await?;

    Ok((
        StatusCode::ACCEPTED,
        Json(serde_json::json!({
            "data": {
                "worker_id": normalized_worker_id,
                "seq": seq,
                "event_type": event_type,
                "payload": event_payload,
                "occurred_at": timestamp(occurred_at),
            }
        })),
    ))
}

async fn runtime_codex_worker_stop(
    State(state): State<AppState>,
    Path(worker_id): Path<String>,
    headers: HeaderMap,
    Json(payload): Json<RuntimeCodexWorkerStopRequestPayload>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiErrorResponse>)> {
    let bundle = session_bundle_from_headers(&state, &headers).await?;
    let normalized_worker_id = worker_id.trim().to_string();
    if normalized_worker_id.is_empty() {
        return Err(validation_error("worker_id", "Worker id is required."));
    }
    let reason = normalize_optional_bounded_string(payload.reason, "reason", 255)?;

    let now = Utc::now();
    let (idempotent_replay, seq) = {
        let mut workers = state.runtime_workers.workers.lock().await;
        let worker = workers
            .get_mut(&normalized_worker_id)
            .ok_or_else(|| not_found_error("Not found."))?;
        if worker.owner_user_id != bundle.user.id {
            return Err(forbidden_error(
                "worker does not belong to authenticated principal",
            ));
        }
        if worker.status == "stopped" {
            (true, worker.latest_seq)
        } else {
            worker.latest_seq = worker.latest_seq.saturating_add(1);
            worker.status = "stopped".to_string();
            worker.stopped_at = Some(now);
            worker.updated_at = now;
            let seq = worker.latest_seq;
            let payload = serde_json::json!({
                "reason": reason.clone().unwrap_or_else(|| "requested".to_string())
            });
            let mut events = state.runtime_workers.events.lock().await;
            let entry = events.entry(normalized_worker_id.clone()).or_default();
            entry.push(RuntimeWorkerEventRecord {
                seq,
                event_type: "worker.stopped".to_string(),
                payload,
                occurred_at: now,
            });
            (false, seq)
        }
    };

    Ok((
        StatusCode::ACCEPTED,
        Json(serde_json::json!({
            "data": {
                "worker_id": normalized_worker_id,
                "status": "stopped",
                "seq": seq,
                "idempotent_replay": idempotent_replay,
                "idempotentReplay": idempotent_replay,
            }
        })),
    ))
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
    ensure_runtime_worker_runnable(&state, &session.user.id, &normalized_worker_id, true).await?;

    let response =
        execute_codex_control_request(&state, &session, &normalized_method, &control_request)
            .await?;

    let mut envelope = serde_json::json!({
        "worker_id": normalized_worker_id.clone(),
        "request_id": normalized_control_request_id.clone(),
        "ok": true,
        "method": normalized_method.clone(),
        "idempotent_replay": false,
        "response": response.clone(),
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

    let event_payload = serde_json::json!({
        "request_id": normalized_control_request_id.clone(),
        "method": normalized_method.clone(),
        "response": response,
    });
    let _ = append_runtime_worker_event(
        &state,
        &normalized_worker_id,
        "worker.response",
        event_payload,
        Utc::now(),
    )
    .await?;

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

fn parse_runtime_internal_user_id(
    value: Option<&serde_json::Value>,
) -> Result<(String, serde_json::Value), (StatusCode, Json<ApiErrorResponse>)> {
    let Some(value) = value else {
        return Err(validation_error(
            "user_id",
            "The user_id field is required.",
        ));
    };

    match value {
        serde_json::Value::String(raw) => {
            let normalized = non_empty(raw.to_string())
                .ok_or_else(|| validation_error("user_id", "The user_id field is required."))?;
            Ok((normalized.clone(), serde_json::Value::String(normalized)))
        }
        serde_json::Value::Number(number) => {
            let Some(user_id) = number.as_u64() else {
                return Err(validation_error(
                    "user_id",
                    "The user_id field must be an integer greater than 0.",
                ));
            };
            if user_id == 0 {
                return Err(validation_error(
                    "user_id",
                    "The user_id field must be an integer greater than 0.",
                ));
            }
            Ok((
                user_id.to_string(),
                serde_json::Value::Number(serde_json::Number::from(user_id)),
            ))
        }
        _ => Err(validation_error(
            "user_id",
            "The user_id field must be a string or integer.",
        )),
    }
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
        vec![revoked.device_id.clone()],
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
    let runtime_routing_status = state.runtime_routing.status(&state._domain_store).await;

    let response = serde_json::json!({
        "data": {
            "service": SERVICE_NAME,
            "authProvider": state.auth.provider_name(),
            "activeOrgId": bundle.session.active_org_id,
            "memberships": bundle.memberships,
            "compatibility": {
                "protocolVersion": state.config.compat_control_protocol_version,
                "minClientBuildId": state.config.compat_control_min_client_build_id,
                "maxClientBuildId": state.config.compat_control_max_client_build_id,
                "minSchemaVersion": state.config.compat_control_min_schema_version,
                "maxSchemaVersion": state.config.compat_control_max_schema_version,
            },
            "routeSplit": route_split_status,
            "runtimeRouting": runtime_routing_status,
            "runtimeRouteOwnership": runtime_route_ownership(),
            "syncCutover": {
                "defaultTransport": "spacetime_ws",
                "khalaEmergencyModeEnabled": state.config.khala_token_enabled,
                "khalaTokenRoute": ROUTE_KHALA_TOKEN,
                "spacetimeTokenRoute": ROUTE_SPACETIME_TOKEN,
            }
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

fn map_legacy_stream_adapter_error(
    error: vercel_sse_adapter::AdapterError,
) -> (StatusCode, Json<ApiErrorResponse>) {
    match error {
        vercel_sse_adapter::AdapterError::MissingConversationId => {
            validation_error("conversation_id", "Conversation id is required.")
        }
        vercel_sse_adapter::AdapterError::MissingUserText => validation_error(
            "messages",
            "Legacy stream payload must include user message text.",
        ),
        _ => validation_error("messages", &error.to_string()),
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
        "{CHALLENGE_COOKIE_NAME}={challenge_id}; Path=/; HttpOnly; Secure; SameSite=Lax; Max-Age={max_age_seconds}"
    )
}

fn auth_access_cookie(access_token: &str, max_age_seconds: u64) -> String {
    format!(
        "{AUTH_ACCESS_COOKIE_NAME}={access_token}; Path=/; HttpOnly; Secure; SameSite=Lax; Max-Age={max_age_seconds}"
    )
}

fn auth_refresh_cookie(refresh_token: &str, max_age_seconds: u64) -> String {
    format!(
        "{AUTH_REFRESH_COOKIE_NAME}={refresh_token}; Path=/; HttpOnly; Secure; SameSite=Lax; Max-Age={max_age_seconds}"
    )
}

fn local_test_auth_cookie(max_age_seconds: u64) -> String {
    format!(
        "{LOCAL_TEST_AUTH_COOKIE_NAME}=1; Path=/; HttpOnly; Secure; SameSite=Lax; Max-Age={max_age_seconds}"
    )
}

fn clear_cookie(name: &str) -> String {
    format!("{name}=; Path=/; HttpOnly; Secure; SameSite=Lax; Max-Age=0")
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

fn normalize_route_path(raw_path: &str) -> String {
    let normalized = raw_path.trim();
    if normalized.is_empty() {
        return ROUTE_RUNTIME_INTERNAL_SECRET_FETCH.to_string();
    }
    if normalized.starts_with('/') {
        normalized.to_string()
    } else {
        format!("/{normalized}")
    }
}

fn runtime_internal_error(
    status: StatusCode,
    code: &'static str,
    message: impl Into<String>,
) -> (StatusCode, Json<ApiErrorResponse>) {
    let message = message.into();
    (
        status,
        Json(ApiErrorResponse {
            message: message.clone(),
            error: ApiErrorDetail { code, message },
            errors: None,
        }),
    )
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

fn htmx_mode_label(mode: HtmxModeTarget) -> &'static str {
    match mode {
        HtmxModeTarget::Fragment => "fragment",
        HtmxModeTarget::FullPage => "full_page",
    }
}

fn emit_htmx_mode_decision_audit(
    state: &AppState,
    request_id: &str,
    decision: &HtmxModeDecision,
    htmx: &HtmxRequest,
) {
    let mut event = AuditEvent::new("route.htmx.mode.decision", request_id.to_string())
        .with_attribute("path", decision.path.clone())
        .with_attribute("route_domain", decision.route_domain.clone())
        .with_attribute("mode", htmx_mode_label(decision.mode).to_string())
        .with_attribute("reason", decision.reason.clone())
        .with_attribute(
            "rollback_mode",
            htmx_mode_label(decision.rollback_mode).to_string(),
        )
        .with_attribute("is_hx_request", htmx.is_hx_request.to_string())
        .with_attribute("hx_boosted", htmx.boosted.to_string())
        .with_attribute(
            "hx_history_restore_request",
            htmx.history_restore_request.to_string(),
        );

    if let Some(target) = htmx.target.as_ref() {
        event = event.with_attribute("hx_target", target.clone());
    }
    if let Some(trigger) = htmx.trigger.as_ref() {
        event = event.with_attribute("hx_trigger", trigger.clone());
    }
    if let Some(current_url) = htmx.current_url.as_ref() {
        event = event.with_attribute("hx_current_url", current_url.clone());
    }

    state.observability.audit(event);
    state
        .observability
        .increment_counter("route.htmx.mode.decision", request_id);
}

fn header_string(headers: &HeaderMap, key: &str) -> Option<String> {
    headers
        .get(key)
        .and_then(|value| value.to_str().ok())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    let mut diff: u8 = 0;
    for (lhs, rhs) in left.iter().zip(right.iter()) {
        diff |= lhs ^ rhs;
    }
    diff == 0
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

fn normalize_optional_bounded_trimmed_string(
    value: Option<String>,
    field: &'static str,
    max_chars: usize,
) -> Result<Option<String>, (StatusCode, Json<ApiErrorResponse>)> {
    let Some(candidate) = value else {
        return Ok(None);
    };

    let trimmed = candidate.trim().to_string();
    if trimmed.chars().count() > max_chars {
        return Err(validation_error(
            field,
            &format!("Value may not be greater than {max_chars} characters."),
        ));
    }

    Ok(Some(trimmed))
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

fn split_principles_text(value: Option<String>) -> Vec<String> {
    value
        .unwrap_or_default()
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(ToString::to_string)
        .collect()
}

#[derive(Debug, Clone)]
struct CodingPolicyEvaluation {
    state: &'static str,
    decision: &'static str,
    reason_code: &'static str,
    denial_message: Option<String>,
    write_approved: bool,
    write_operations_mode: String,
    max_per_call_sats: Option<u64>,
    operation_cost_sats: u64,
}

fn validate_optional_json_object_or_array(
    value: Option<serde_json::Value>,
    field: &'static str,
) -> Result<Option<serde_json::Value>, (StatusCode, Json<ApiErrorResponse>)> {
    let Some(value) = value else {
        return Ok(None);
    };

    if value.is_object() || value.is_array() {
        return Ok(Some(value));
    }

    Err(validation_error(
        field,
        &format!("The {field} field must be an array."),
    ))
}

fn parse_optional_positive_u64(
    value: Option<serde_json::Value>,
    field: &'static str,
) -> Result<Option<u64>, (StatusCode, Json<ApiErrorResponse>)> {
    let Some(value) = value else {
        return Ok(None);
    };
    if value.is_null() {
        return Ok(None);
    }

    let parsed = positive_u64_from_value(&value).ok_or_else(|| {
        validation_error(field, &format!("The {field} field must be an integer."))
    })?;
    if parsed < 1 {
        return Err(validation_error(
            field,
            &format!("The {field} field must be at least 1."),
        ));
    }
    Ok(Some(parsed))
}

fn positive_u64_from_value(value: &serde_json::Value) -> Option<u64> {
    if let Some(number) = value.as_u64() {
        return Some(number);
    }
    let text = value.as_str()?.trim();
    if text.is_empty() {
        return None;
    }
    text.parse::<u64>().ok()
}

fn runtime_tools_principal_user_id(principal_id: &str) -> u64 {
    let digest = Sha256::digest(principal_id.as_bytes());
    let mut bytes = [0u8; 8];
    bytes.copy_from_slice(&digest[..8]);
    u64::from_be_bytes(bytes).max(1)
}

fn parse_coding_tool_invocation(
    request: &serde_json::Map<String, serde_json::Value>,
) -> Result<CodingToolInvocation, (StatusCode, Json<ApiErrorResponse>)> {
    let integration_id = normalized_json_string(request.get("integration_id"))
        .or_else(|| normalized_json_string(request.get("integrationId")))
        .ok_or_else(|| {
            validation_error(
                "request.integration_id",
                "The request.integration_id field is required.",
            )
        })?;

    let operation = normalized_json_string(request.get("operation"))
        .map(|value| value.to_lowercase())
        .ok_or_else(|| {
            validation_error(
                "request.operation",
                "The request.operation field is required.",
            )
        })?;
    if !matches!(
        operation.as_str(),
        "get_issue" | "get_pull_request" | "add_issue_comment"
    ) {
        return Err(validation_error(
            "request.operation",
            "The selected request.operation is invalid.",
        ));
    }

    let repository = normalized_json_string(request.get("repository")).ok_or_else(|| {
        validation_error(
            "request.repository",
            "The request.repository field is required.",
        )
    })?;
    if !repository.contains('/') || repository.starts_with('/') || repository.ends_with('/') {
        return Err(validation_error(
            "request.repository",
            "The request.repository field format is invalid.",
        ));
    }

    let issue_number = positive_u64_from_value(
        request
            .get("issue_number")
            .or_else(|| request.get("issueNumber"))
            .unwrap_or(&serde_json::Value::Null),
    );
    let pull_number = positive_u64_from_value(
        request
            .get("pull_number")
            .or_else(|| request.get("pullNumber"))
            .unwrap_or(&serde_json::Value::Null),
    );
    let body = normalized_json_string(request.get("body"));
    let tool_call_id = normalized_json_string(request.get("tool_call_id"))
        .or_else(|| normalized_json_string(request.get("toolCallId")));

    match operation.as_str() {
        "get_issue" => {
            if issue_number.is_none() {
                return Err(validation_error(
                    "request.issue_number",
                    "The request.issue_number field is required for get_issue.",
                ));
            }
        }
        "get_pull_request" => {
            if pull_number.is_none() {
                return Err(validation_error(
                    "request.pull_number",
                    "The request.pull_number field is required for get_pull_request.",
                ));
            }
        }
        "add_issue_comment" => {
            if issue_number.is_none() {
                return Err(validation_error(
                    "request.issue_number",
                    "The request.issue_number field is required for add_issue_comment.",
                ));
            }
            if body.is_none() {
                return Err(validation_error(
                    "request.body",
                    "The request.body field is required for add_issue_comment.",
                ));
            }
        }
        _ => {}
    }

    Ok(CodingToolInvocation {
        integration_id,
        operation,
        repository,
        issue_number,
        pull_number,
        body,
        tool_call_id,
    })
}

fn parse_l402_tool_invocation(
    request: &serde_json::Map<String, serde_json::Value>,
) -> Result<L402ToolInvocation, (StatusCode, Json<ApiErrorResponse>)> {
    let operation = normalized_json_string(request.get("operation"))
        .map(|value| value.to_lowercase())
        .ok_or_else(|| {
            validation_error(
                "request.operation",
                "The request.operation field is required.",
            )
        })?;
    if !matches!(
        operation.as_str(),
        "lightning_l402_fetch" | "lightning_l402_approve"
    ) {
        return Err(validation_error(
            "request.operation",
            "The selected request.operation is invalid.",
        ));
    }

    let tool_call_id = normalized_json_string(request.get("tool_call_id"))
        .or_else(|| normalized_json_string(request.get("toolCallId")));
    let autopilot_id = normalized_json_string(request.get("autopilot_id"))
        .or_else(|| normalized_json_string(request.get("autopilotId")));

    if operation == "lightning_l402_approve" {
        let task_id = normalized_json_string(request.get("task_id"))
            .or_else(|| normalized_json_string(request.get("taskId")))
            .ok_or_else(|| {
                validation_error(
                    "request.task_id",
                    "The request.task_id field is required for lightning_l402_approve.",
                )
            })?;
        return Ok(L402ToolInvocation {
            operation,
            url: None,
            method: "GET".to_string(),
            host: None,
            scope: None,
            request_headers: None,
            request_body: None,
            max_spend_msats: None,
            require_approval: None,
            timeout_ms: 20_000,
            task_id: Some(task_id),
            autopilot_id,
            tool_call_id,
        });
    }

    let url = normalized_json_string(request.get("url")).ok_or_else(|| {
        validation_error(
            "request.url",
            "The request.url field is required for lightning_l402_fetch.",
        )
    })?;
    let parsed_url = reqwest::Url::parse(&url).map_err(|_| {
        validation_error("request.url", "The request.url field must be a valid URL.")
    })?;
    if !matches!(parsed_url.scheme(), "http" | "https") {
        return Err(validation_error(
            "request.url",
            "The request.url field must use http:// or https://.",
        ));
    }
    if parsed_url.host_str().is_none() {
        return Err(validation_error(
            "request.url",
            "The request.url field must include a valid host.",
        ));
    }

    let method = normalized_json_string(request.get("method"))
        .unwrap_or_else(|| "GET".to_string())
        .to_ascii_uppercase();
    if !matches!(
        method.as_str(),
        "GET" | "POST" | "PUT" | "PATCH" | "DELETE" | "HEAD"
    ) {
        return Err(validation_error(
            "request.method",
            "The selected request.method is invalid.",
        ));
    }

    let request_headers = match request.get("headers") {
        Some(serde_json::Value::Object(_)) | None => request.get("headers").cloned(),
        Some(_) => {
            return Err(validation_error(
                "request.headers",
                "The request.headers field must be an object.",
            ));
        }
    };
    if let Some(headers) = request_headers.as_ref() {
        l402_validate_headers(headers)?;
    }

    let request_body = request.get("body").cloned();
    let host = normalized_json_string(request.get("host"));
    let scope = normalized_json_string(request.get("scope"));
    let max_spend_msats = request
        .get("max_spend_msats")
        .or_else(|| request.get("maxSpendMsats"))
        .and_then(positive_u64_from_value);
    let require_approval = request
        .get("require_approval")
        .or_else(|| request.get("requireApproval"))
        .and_then(l402_bool_from_value);
    let timeout_ms = request
        .get("timeout_ms")
        .or_else(|| request.get("timeoutMs"))
        .and_then(positive_u64_from_value)
        .unwrap_or(20_000)
        .clamp(1_000, 120_000);

    Ok(L402ToolInvocation {
        operation,
        url: Some(url),
        method,
        host,
        scope,
        request_headers,
        request_body,
        max_spend_msats,
        require_approval,
        timeout_ms,
        task_id: None,
        autopilot_id,
        tool_call_id,
    })
}

fn l402_bool_from_value(value: &serde_json::Value) -> Option<bool> {
    value.as_bool().or_else(|| {
        value.as_str().and_then(|raw| {
            let normalized = raw.trim().to_ascii_lowercase();
            match normalized.as_str() {
                "1" | "true" | "yes" => Some(true),
                "0" | "false" | "no" => Some(false),
                _ => None,
            }
        })
    })
}

fn l402_validate_headers(
    value: &serde_json::Value,
) -> Result<(), (StatusCode, Json<ApiErrorResponse>)> {
    let Some(object) = value.as_object() else {
        return Err(validation_error(
            "request.headers",
            "The request.headers field must be an object.",
        ));
    };
    if object.len() > 40 {
        return Err(validation_error(
            "request.headers",
            "The request.headers field may not include more than 40 entries.",
        ));
    }
    for (key, value) in object {
        if key.trim().is_empty() || key.chars().count() > 120 {
            return Err(validation_error(
                "request.headers",
                "Header keys must be between 1 and 120 characters.",
            ));
        }
        let Some(text) = value.as_str() else {
            return Err(validation_error(
                "request.headers",
                "Header values must be strings.",
            ));
        };
        if text.chars().count() > 1024 {
            return Err(validation_error(
                "request.headers",
                "Header values may not be greater than 1024 characters.",
            ));
        }
    }
    Ok(())
}

fn parse_l402_tool_policy(policy: &serde_json::Value) -> L402ToolPolicy {
    let mut require_approval = true;
    let mut max_spend_msats_per_call = None;
    let mut max_spend_msats_per_day = None;
    let mut allowed_hosts = Vec::new();

    if let Some(object) = policy.as_object() {
        require_approval = object
            .get("l402_require_approval")
            .or_else(|| object.get("l402RequireApproval"))
            .and_then(l402_bool_from_value)
            .unwrap_or(true);
        max_spend_msats_per_call = object
            .get("l402_max_spend_msats_per_call")
            .or_else(|| object.get("l402MaxSpendMsatsPerCall"))
            .and_then(positive_u64_from_value);
        max_spend_msats_per_day = object
            .get("l402_max_spend_msats_per_day")
            .or_else(|| object.get("l402MaxSpendMsatsPerDay"))
            .and_then(positive_u64_from_value);
        allowed_hosts = object
            .get("l402_allowed_hosts")
            .or_else(|| object.get("l402AllowedHosts"))
            .and_then(serde_json::Value::as_array)
            .map(|values| {
                values
                    .iter()
                    .filter_map(serde_json::Value::as_str)
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(|value| value.to_ascii_lowercase())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
    }

    let credential_ttl_seconds =
        env_u64("L402_CREDENTIAL_TTL_SECONDS", 600).clamp(30, 86_400) as i64;

    L402ToolPolicy {
        require_approval,
        max_spend_msats_per_call,
        max_spend_msats_per_day,
        allowed_hosts,
        credential_ttl_seconds,
    }
}

fn l402_host_is_allowed(host: &str, allowed_hosts: &[String]) -> bool {
    if allowed_hosts.is_empty() {
        return true;
    }
    let host = host.trim().to_ascii_lowercase();
    if host.is_empty() {
        return false;
    }
    allowed_hosts.iter().any(|allowed| {
        host == *allowed
            || host
                .strip_suffix(allowed)
                .map(|prefix| prefix.ends_with('.'))
                .unwrap_or(false)
    })
}

fn l402_scope_from_request(invocation: &L402ToolInvocation, parsed_url: &reqwest::Url) -> String {
    if let Some(scope) = invocation.scope.as_ref().map(|value| value.trim()) {
        if !scope.is_empty() {
            return scope.to_ascii_lowercase();
        }
    }
    let path = parsed_url.path().trim();
    if path.is_empty() || path == "/" {
        "root".to_string()
    } else {
        path.to_ascii_lowercase()
    }
}

fn l402_authorization_header(macaroon: &str, preimage: &str) -> String {
    format!("L402 {macaroon}:{preimage}")
}

fn l402_response_body_value(body: &str) -> serde_json::Value {
    if body.trim().is_empty() {
        return serde_json::Value::Null;
    }
    serde_json::from_str::<serde_json::Value>(body)
        .unwrap_or_else(|_| serde_json::json!({ "raw": body }))
}

async fn execute_l402_http_request(
    invocation: &L402ToolInvocation,
    authorization: Option<String>,
) -> Result<L402HttpResponseSnapshot, (StatusCode, Json<ApiErrorResponse>)> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_millis(invocation.timeout_ms))
        .build()
        .map_err(|error| {
            error_response_with_status(
                StatusCode::BAD_GATEWAY,
                ApiErrorCode::ServiceUnavailable,
                format!("Failed to create L402 HTTP client: {error}"),
            )
        })?;

    let url = invocation.url.clone().ok_or_else(|| {
        error_response_with_status(
            StatusCode::UNPROCESSABLE_ENTITY,
            ApiErrorCode::InvalidRequest,
            "L402 fetch request did not include URL.".to_string(),
        )
    })?;
    let method = reqwest::Method::from_bytes(invocation.method.as_bytes()).map_err(|_| {
        validation_error("request.method", "The selected request.method is invalid.")
    })?;
    let mut request = client.request(method, &url);

    if let Some(headers) = invocation
        .request_headers
        .as_ref()
        .and_then(serde_json::Value::as_object)
    {
        for (key, value) in headers {
            let name = key.trim();
            let Some(text) = value.as_str().map(str::trim) else {
                continue;
            };
            if text.is_empty() {
                continue;
            }
            let lower_name = name.to_ascii_lowercase();
            if matches!(
                lower_name.as_str(),
                "authorization" | "host" | "content-length"
            ) {
                continue;
            }
            request = request.header(name, text);
        }
    }
    if let Some(value) = authorization.as_deref() {
        request = request.header(reqwest::header::AUTHORIZATION, value);
    }

    if let Some(body) = invocation.request_body.as_ref() {
        if !body.is_null() {
            request = if body.is_object() || body.is_array() {
                request.json(body)
            } else if let Some(text) = body.as_str() {
                request.body(text.to_string())
            } else {
                request.body(body.to_string())
            };
        }
    }

    let response = request.send().await.map_err(|error| {
        error_response_with_status(
            StatusCode::BAD_GATEWAY,
            ApiErrorCode::ServiceUnavailable,
            format!("L402 request failed: {error}"),
        )
    })?;
    let status = response.status();
    let challenge_header = response
        .headers()
        .get(reqwest::header::WWW_AUTHENTICATE)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    let body = response.text().await.map_err(|error| {
        error_response_with_status(
            StatusCode::BAD_GATEWAY,
            ApiErrorCode::ServiceUnavailable,
            format!("L402 response read failed: {error}"),
        )
    })?;
    let body_sha256 = sha256_hex(body.as_bytes());

    Ok(L402HttpResponseSnapshot {
        status,
        body,
        body_sha256,
        challenge_header,
    })
}

async fn l402_daily_spend_summary(
    state: &AppState,
    user_id: &str,
    autopilot_id: Option<&str>,
) -> Result<L402DailySpendSummary, (StatusCode, Json<ApiErrorResponse>)> {
    let receipts = state
        ._domain_store
        .list_l402_receipts_for_user(user_id, autopilot_id, 1000, 0)
        .await
        .map_err(map_domain_store_error)?;

    let today = Utc::now().date_naive();
    let mut spent_msats_today = 0u64;
    for row in receipts {
        if row.created_at.date_naive() != today {
            continue;
        }
        let paid = row
            .payload
            .get("paid")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);
        if !paid {
            continue;
        }
        let amount = row
            .payload
            .get("amountMsats")
            .and_then(positive_u64_from_value)
            .unwrap_or(0);
        spent_msats_today = spent_msats_today.saturating_add(amount);
    }

    Ok(L402DailySpendSummary { spent_msats_today })
}

async fn execute_l402_paid_fetch(
    state: &AppState,
    user_id: &str,
    host: &str,
    scope: &str,
    invocation: &L402ToolInvocation,
    challenge_macaroon: &str,
    challenge_invoice: &str,
    max_spend_msats: u64,
    policy: &L402ToolPolicy,
) -> Result<L402ToolExecutionResult, (StatusCode, Json<ApiErrorResponse>)> {
    let quoted_amount_msats = Bolt11::amount_msats(challenge_invoice);
    if quoted_amount_msats
        .map(|quoted| quoted > max_spend_msats)
        .unwrap_or(false)
    {
        return Ok(L402ToolExecutionResult {
            state: "blocked",
            decision: "denied",
            reason_code: "policy_denied.max_spend_per_call_exceeded",
            result: serde_json::json!({
                "blocked": true,
                "host": host,
                "scope": scope,
                "quotedAmountMsats": quoted_amount_msats,
                "maxSpendMsats": max_spend_msats,
            }),
            error: Some(serde_json::json!({
                "code": "policy_denied.max_spend_per_call_exceeded",
                "message": "Quoted L402 invoice amount exceeds configured max spend per call.",
            })),
            receipt_status: "blocked".to_string(),
            paid: false,
            cache_hit: false,
            cache_status: None,
            amount_msats: None,
            quoted_amount_msats,
            max_spend_msats: Some(max_spend_msats),
            proof_reference: None,
            deny_code: Some("policy_denied.max_spend_per_call_exceeded".to_string()),
            task_id: None,
            approval_required: false,
            response_status_code: Some(StatusCode::PAYMENT_REQUIRED.as_u16()),
            response_body_sha256: None,
        });
    }

    let wallet = ensure_agent_wallet_for_user(state, user_id).await?;
    let payer_kind = l402_invoice_payer_kind();
    let payment = match agent_payments_pay_invoice_with_adapter(
        state,
        payer_kind.as_str(),
        user_id,
        &wallet,
        challenge_invoice,
        max_spend_msats,
        invocation.timeout_ms,
        Some(host.to_string()),
    )
    .await
    {
        Ok(result) => result,
        Err(error) => {
            return Ok(L402ToolExecutionResult {
                state: "failed",
                decision: "failed",
                reason_code: "invoice_pay_failed",
                result: serde_json::json!({
                    "host": host,
                    "scope": scope,
                    "quotedAmountMsats": quoted_amount_msats,
                    "maxSpendMsats": max_spend_msats,
                    "paymentBackend": payer_kind,
                }),
                error: Some(serde_json::json!({
                    "code": error.code,
                    "message": error.message,
                })),
                receipt_status: "failed".to_string(),
                paid: false,
                cache_hit: false,
                cache_status: None,
                amount_msats: None,
                quoted_amount_msats,
                max_spend_msats: Some(max_spend_msats),
                proof_reference: None,
                deny_code: Some("invoice_pay_failed".to_string()),
                task_id: None,
                approval_required: false,
                response_status_code: Some(StatusCode::PAYMENT_REQUIRED.as_u16()),
                response_body_sha256: None,
            });
        }
    };

    state
        ._domain_store
        .upsert_l402_credential(UpsertL402CredentialInput {
            user_id: user_id.to_string(),
            host: host.to_string(),
            scope: scope.to_string(),
            macaroon: challenge_macaroon.to_string(),
            preimage: payment.preimage.clone(),
            expires_at: Utc::now() + Duration::seconds(policy.credential_ttl_seconds),
        })
        .await
        .map_err(map_domain_store_error)?;

    let authorization = l402_authorization_header(challenge_macaroon, &payment.preimage);
    let second = execute_l402_http_request(invocation, Some(authorization)).await?;
    let succeeded = second.status.is_success();

    Ok(L402ToolExecutionResult {
        state: if succeeded { "succeeded" } else { "failed" },
        decision: if succeeded { "allowed" } else { "failed" },
        reason_code: if succeeded {
            "paid_fetch_completed"
        } else {
            "paid_fetch_failed"
        },
        result: serde_json::json!({
            "host": host,
            "scope": scope,
            "quotedAmountMsats": quoted_amount_msats,
            "maxSpendMsats": max_spend_msats,
            "response": {
                "statusCode": second.status.as_u16(),
                "body": l402_response_body_value(second.body.as_str()),
                "bodySha256": second.body_sha256.clone(),
            },
            "payment": {
                "paymentId": payment.payment_id,
                "proofReference": format!("preimage:{}", payment.preimage.chars().take(16).collect::<String>()),
            }
        }),
        error: if succeeded {
            None
        } else {
            Some(serde_json::json!({
                "code": "paid_fetch_failed",
                "message": format!("Paid retry returned HTTP {}", second.status.as_u16()),
            }))
        },
        receipt_status: if succeeded {
            "paid".to_string()
        } else {
            "failed".to_string()
        },
        paid: succeeded,
        cache_hit: false,
        cache_status: None,
        amount_msats: quoted_amount_msats.or(Some(max_spend_msats)),
        quoted_amount_msats,
        max_spend_msats: Some(max_spend_msats),
        proof_reference: Some(format!(
            "preimage:{}",
            payment.preimage.chars().take(16).collect::<String>()
        )),
        deny_code: None,
        task_id: None,
        approval_required: false,
        response_status_code: Some(second.status.as_u16()),
        response_body_sha256: Some(second.body_sha256),
    })
}

async fn execute_l402_tool(
    state: &AppState,
    user_id: &str,
    run_id: &str,
    thread_id: &str,
    replay_hash_hex: &str,
    policy_payload: &serde_json::Value,
    invocation: &L402ToolInvocation,
) -> Result<L402ToolExecutionResult, (StatusCode, Json<ApiErrorResponse>)> {
    let policy = parse_l402_tool_policy(policy_payload);

    if invocation.operation == "lightning_l402_approve" {
        let task_id = invocation.task_id.clone().ok_or_else(|| {
            validation_error(
                "request.task_id",
                "The request.task_id field is required for lightning_l402_approve.",
            )
        })?;
        let consumed = state
            ._domain_store
            .consume_l402_approval_task_for_user(user_id, &task_id)
            .await
            .map_err(map_domain_store_error)?
            .ok_or_else(|| not_found_error("approval task not found"))?;

        if consumed.task.expires_at <= Utc::now() {
            return Ok(L402ToolExecutionResult {
                state: "blocked",
                decision: "denied",
                reason_code: "approval_task_expired",
                result: serde_json::json!({
                    "taskId": consumed.task.task_id,
                    "status": "expired",
                    "host": consumed.task.host,
                    "scope": consumed.task.scope,
                }),
                error: Some(serde_json::json!({
                    "code": "approval_task_expired",
                    "message": "Approval task expired before execution.",
                })),
                receipt_status: "blocked".to_string(),
                paid: false,
                cache_hit: false,
                cache_status: None,
                amount_msats: None,
                quoted_amount_msats: Bolt11::amount_msats(consumed.task.challenge_invoice.as_str()),
                max_spend_msats: Some(consumed.task.max_spend_msats),
                proof_reference: None,
                deny_code: Some("approval_task_expired".to_string()),
                task_id: Some(consumed.task.task_id),
                approval_required: true,
                response_status_code: None,
                response_body_sha256: None,
            });
        }

        if !consumed.consumed {
            if let Some(existing) = state
                ._domain_store
                .find_latest_l402_receipt_for_user_by_task_id(user_id, &task_id)
                .await
                .map_err(map_domain_store_error)?
            {
                let payload = existing.payload;
                let result_payload = payload
                    .get("result")
                    .cloned()
                    .unwrap_or_else(|| serde_json::json!({ "taskId": task_id }));
                return Ok(L402ToolExecutionResult {
                    state: "succeeded",
                    decision: "allowed",
                    reason_code: "approval_already_executed",
                    result: result_payload,
                    error: None,
                    receipt_status: payload
                        .get("status")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or("paid")
                        .to_string(),
                    paid: payload
                        .get("paid")
                        .and_then(serde_json::Value::as_bool)
                        .unwrap_or(false),
                    cache_hit: payload
                        .get("cacheHit")
                        .and_then(serde_json::Value::as_bool)
                        .unwrap_or(false),
                    cache_status: payload
                        .get("cacheStatus")
                        .and_then(serde_json::Value::as_str)
                        .map(str::to_string),
                    amount_msats: payload.get("amountMsats").and_then(positive_u64_from_value),
                    quoted_amount_msats: payload
                        .get("quotedAmountMsats")
                        .and_then(positive_u64_from_value),
                    max_spend_msats: payload
                        .get("maxSpendMsats")
                        .and_then(positive_u64_from_value),
                    proof_reference: payload
                        .get("proofReference")
                        .and_then(serde_json::Value::as_str)
                        .map(str::to_string),
                    deny_code: payload
                        .get("denyCode")
                        .and_then(serde_json::Value::as_str)
                        .map(str::to_string),
                    task_id: Some(task_id),
                    approval_required: true,
                    response_status_code: payload
                        .get("responseStatusCode")
                        .and_then(positive_u64_from_value)
                        .and_then(|value| u16::try_from(value).ok()),
                    response_body_sha256: payload
                        .get("responseBodySha256")
                        .and_then(serde_json::Value::as_str)
                        .map(str::to_string),
                });
            }
        }

        let approved_invocation = L402ToolInvocation {
            operation: "lightning_l402_fetch".to_string(),
            url: Some(consumed.task.url.clone()),
            method: consumed.task.method.clone(),
            host: Some(consumed.task.host.clone()),
            scope: Some(consumed.task.scope.clone()),
            request_headers: consumed.task.request_headers.clone(),
            request_body: consumed.task.request_body.clone(),
            max_spend_msats: Some(consumed.task.max_spend_msats),
            require_approval: Some(false),
            timeout_ms: invocation.timeout_ms,
            task_id: Some(consumed.task.task_id.clone()),
            autopilot_id: consumed.task.autopilot_id.clone(),
            tool_call_id: consumed.task.tool_call_id.clone(),
        };
        let mut result = execute_l402_paid_fetch(
            state,
            user_id,
            consumed.task.host.as_str(),
            consumed.task.scope.as_str(),
            &approved_invocation,
            consumed.task.challenge_macaroon.as_str(),
            consumed.task.challenge_invoice.as_str(),
            consumed.task.max_spend_msats,
            &policy,
        )
        .await?;
        result.task_id = Some(consumed.task.task_id);
        result.approval_required = true;
        return Ok(result);
    }

    let url = invocation.url.clone().ok_or_else(|| {
        validation_error(
            "request.url",
            "The request.url field is required for lightning_l402_fetch.",
        )
    })?;
    let parsed_url = reqwest::Url::parse(&url).map_err(|_| {
        validation_error("request.url", "The request.url field must be a valid URL.")
    })?;
    let host = invocation
        .host
        .clone()
        .or_else(|| parsed_url.host_str().map(str::to_string))
        .unwrap_or_default()
        .to_ascii_lowercase();
    if host.is_empty() {
        return Err(validation_error(
            "request.url",
            "The request.url field must include a host.",
        ));
    }
    if !l402_host_is_allowed(&host, &policy.allowed_hosts) {
        return Ok(L402ToolExecutionResult {
            state: "blocked",
            decision: "denied",
            reason_code: "policy_denied.host_not_allowed",
            result: serde_json::json!({
                "blocked": true,
                "host": host,
                "allowedHosts": policy.allowed_hosts,
            }),
            error: Some(serde_json::json!({
                "code": "policy_denied.host_not_allowed",
                "message": "Requested host is not in L402 allowed hosts policy.",
            })),
            receipt_status: "blocked".to_string(),
            paid: false,
            cache_hit: false,
            cache_status: None,
            amount_msats: None,
            quoted_amount_msats: None,
            max_spend_msats: invocation
                .max_spend_msats
                .or(policy.max_spend_msats_per_call),
            proof_reference: None,
            deny_code: Some("policy_denied.host_not_allowed".to_string()),
            task_id: None,
            approval_required: false,
            response_status_code: None,
            response_body_sha256: None,
        });
    }
    let scope = l402_scope_from_request(invocation, &parsed_url);
    let credential = state
        ._domain_store
        .find_active_l402_credential_for_user(user_id, &host, &scope)
        .await
        .map_err(map_domain_store_error)?;
    let authorization = credential
        .as_ref()
        .map(|row| l402_authorization_header(row.macaroon.as_str(), row.preimage.as_str()));
    let first = execute_l402_http_request(invocation, authorization).await?;

    if first.status != StatusCode::PAYMENT_REQUIRED {
        let succeeded = first.status.is_success();
        return Ok(L402ToolExecutionResult {
            state: if succeeded { "succeeded" } else { "failed" },
            decision: if succeeded { "allowed" } else { "failed" },
            reason_code: if succeeded {
                if credential.is_some() {
                    "cached_fetch_completed"
                } else {
                    "fetch_completed_without_challenge"
                }
            } else {
                "fetch_failed"
            },
            result: serde_json::json!({
                "host": host,
                "scope": scope,
                "response": {
                    "statusCode": first.status.as_u16(),
                    "body": l402_response_body_value(first.body.as_str()),
                    "bodySha256": first.body_sha256.clone(),
                }
            }),
            error: if succeeded {
                None
            } else {
                Some(serde_json::json!({
                    "code": "fetch_failed",
                    "message": format!("Upstream returned HTTP {}", first.status.as_u16()),
                }))
            },
            receipt_status: if succeeded {
                if credential.is_some() {
                    "cached".to_string()
                } else {
                    "succeeded".to_string()
                }
            } else {
                "failed".to_string()
            },
            paid: false,
            cache_hit: credential.is_some() && succeeded,
            cache_status: if credential.is_some() && succeeded {
                Some("hit".to_string())
            } else {
                None
            },
            amount_msats: None,
            quoted_amount_msats: None,
            max_spend_msats: invocation
                .max_spend_msats
                .or(policy.max_spend_msats_per_call),
            proof_reference: None,
            deny_code: None,
            task_id: None,
            approval_required: false,
            response_status_code: Some(first.status.as_u16()),
            response_body_sha256: Some(first.body_sha256),
        });
    }

    let parser = WwwAuthenticateParser;
    let challenge = parser.parse_l402_challenge(first.challenge_header.as_deref());
    let Some(challenge) = challenge else {
        return Ok(L402ToolExecutionResult {
            state: "failed",
            decision: "failed",
            reason_code: "l402_challenge_missing",
            result: serde_json::json!({
                "host": host,
                "scope": scope,
                "response": {
                    "statusCode": first.status.as_u16(),
                    "body": l402_response_body_value(first.body.as_str()),
                    "bodySha256": first.body_sha256.clone(),
                }
            }),
            error: Some(serde_json::json!({
                "code": "l402_challenge_missing",
                "message": "Upstream returned 402 but no valid L402 challenge header was present.",
            })),
            receipt_status: "failed".to_string(),
            paid: false,
            cache_hit: false,
            cache_status: None,
            amount_msats: None,
            quoted_amount_msats: None,
            max_spend_msats: invocation
                .max_spend_msats
                .or(policy.max_spend_msats_per_call),
            proof_reference: None,
            deny_code: Some("l402_challenge_missing".to_string()),
            task_id: None,
            approval_required: false,
            response_status_code: Some(first.status.as_u16()),
            response_body_sha256: Some(first.body_sha256),
        });
    };

    let quoted_amount_msats = Bolt11::amount_msats(challenge.invoice.as_str());
    let max_spend_msats = invocation
        .max_spend_msats
        .or(policy.max_spend_msats_per_call)
        .or(quoted_amount_msats)
        .unwrap_or(100_000);
    if quoted_amount_msats
        .map(|quoted| quoted > max_spend_msats)
        .unwrap_or(false)
    {
        return Ok(L402ToolExecutionResult {
            state: "blocked",
            decision: "denied",
            reason_code: "policy_denied.max_spend_per_call_exceeded",
            result: serde_json::json!({
                "blocked": true,
                "host": host,
                "scope": scope,
                "quotedAmountMsats": quoted_amount_msats,
                "maxSpendMsats": max_spend_msats,
            }),
            error: Some(serde_json::json!({
                "code": "policy_denied.max_spend_per_call_exceeded",
                "message": "Quoted L402 invoice amount exceeds configured max spend per call.",
            })),
            receipt_status: "blocked".to_string(),
            paid: false,
            cache_hit: false,
            cache_status: None,
            amount_msats: None,
            quoted_amount_msats,
            max_spend_msats: Some(max_spend_msats),
            proof_reference: None,
            deny_code: Some("policy_denied.max_spend_per_call_exceeded".to_string()),
            task_id: None,
            approval_required: false,
            response_status_code: Some(first.status.as_u16()),
            response_body_sha256: Some(first.body_sha256),
        });
    }

    if let Some(max_daily) = policy.max_spend_msats_per_day {
        let summary =
            l402_daily_spend_summary(state, user_id, invocation.autopilot_id.as_deref()).await?;
        let projected = summary
            .spent_msats_today
            .saturating_add(quoted_amount_msats.unwrap_or(max_spend_msats));
        if projected > max_daily {
            return Ok(L402ToolExecutionResult {
                state: "blocked",
                decision: "denied",
                reason_code: "policy_denied.max_spend_per_day_exceeded",
                result: serde_json::json!({
                    "blocked": true,
                    "host": host,
                    "scope": scope,
                    "quotedAmountMsats": quoted_amount_msats,
                    "maxSpendMsatsPerDay": max_daily,
                    "spentMsatsToday": summary.spent_msats_today,
                    "projectedMsatsToday": projected,
                }),
                error: Some(serde_json::json!({
                    "code": "policy_denied.max_spend_per_day_exceeded",
                    "message": "L402 daily spend policy would be exceeded by this payment.",
                })),
                receipt_status: "blocked".to_string(),
                paid: false,
                cache_hit: false,
                cache_status: None,
                amount_msats: None,
                quoted_amount_msats,
                max_spend_msats: Some(max_spend_msats),
                proof_reference: None,
                deny_code: Some("policy_denied.max_spend_per_day_exceeded".to_string()),
                task_id: None,
                approval_required: false,
                response_status_code: Some(first.status.as_u16()),
                response_body_sha256: Some(first.body_sha256),
            });
        }
    }

    let require_approval = invocation
        .require_approval
        .unwrap_or(policy.require_approval);
    if require_approval {
        let task_id = format!("l402tsk_{}", &replay_hash_hex[..24]);
        let expires_at = Utc::now() + Duration::minutes(30);
        let task = state
            ._domain_store
            .upsert_l402_approval_task(UpsertL402ApprovalTaskInput {
                task_id: task_id.clone(),
                user_id: user_id.to_string(),
                thread_id: thread_id.to_string(),
                run_id: run_id.to_string(),
                autopilot_id: invocation.autopilot_id.clone(),
                host: host.clone(),
                scope: scope.clone(),
                method: invocation.method.clone(),
                url,
                request_headers: invocation.request_headers.clone(),
                request_body: invocation.request_body.clone(),
                challenge_macaroon: challenge.macaroon.clone(),
                challenge_invoice: challenge.invoice.clone(),
                max_spend_msats,
                tool_call_id: invocation.tool_call_id.clone(),
                expires_at,
            })
            .await
            .map_err(map_domain_store_error)?;
        return Ok(L402ToolExecutionResult {
            state: "blocked",
            decision: "approval_requested",
            reason_code: "approval_required",
            result: serde_json::json!({
                "taskId": task.task_id,
                "status": "approval_requested",
                "host": task.host,
                "scope": task.scope,
                "quotedAmountMsats": quoted_amount_msats,
                "maxSpendMsats": max_spend_msats,
                "expiresAt": timestamp(task.expires_at),
            }),
            error: None,
            receipt_status: "blocked".to_string(),
            paid: false,
            cache_hit: false,
            cache_status: None,
            amount_msats: None,
            quoted_amount_msats,
            max_spend_msats: Some(max_spend_msats),
            proof_reference: None,
            deny_code: None,
            task_id: Some(task_id),
            approval_required: true,
            response_status_code: Some(first.status.as_u16()),
            response_body_sha256: Some(first.body_sha256),
        });
    }

    execute_l402_paid_fetch(
        state,
        user_id,
        host.as_str(),
        scope.as_str(),
        invocation,
        challenge.macaroon.as_str(),
        challenge.invoice.as_str(),
        max_spend_msats,
        &policy,
    )
    .await
}

fn evaluate_coding_policy(
    invocation: &CodingToolInvocation,
    manifest: Option<&serde_json::Value>,
    policy: &serde_json::Value,
) -> CodingPolicyEvaluation {
    let manifest_capabilities = manifest
        .and_then(|manifest| manifest.get("capabilities"))
        .and_then(serde_json::Value::as_array)
        .map(|capabilities| {
            capabilities
                .iter()
                .filter_map(serde_json::Value::as_str)
                .map(|value| value.trim().to_lowercase())
                .filter(|value| !value.is_empty())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let write_operations_mode = manifest
        .and_then(|manifest| manifest.get("policy"))
        .and_then(serde_json::Value::as_object)
        .and_then(|policy| policy.get("write_operations_mode"))
        .and_then(serde_json::Value::as_str)
        .map(|value| value.trim().to_lowercase())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "open".to_string());

    let write_approved = match policy {
        serde_json::Value::Object(policy_object) => policy_object
            .get("write_approved")
            .and_then(|value| {
                value.as_bool().or_else(|| {
                    value
                        .as_str()
                        .map(|text| matches!(text.trim(), "1" | "true" | "yes"))
                })
            })
            .unwrap_or(false),
        _ => false,
    };

    let max_per_call_sats = match policy {
        serde_json::Value::Object(policy_object) => policy_object
            .get("budget")
            .and_then(serde_json::Value::as_object)
            .and_then(|budget| budget.get("max_per_call_sats"))
            .and_then(positive_u64_from_value),
        _ => None,
    };

    let operation_cost_sats = match invocation.operation.as_str() {
        "add_issue_comment" => 50,
        "get_pull_request" => 10,
        _ => 5,
    };

    if !manifest_capabilities.is_empty()
        && !manifest_capabilities
            .iter()
            .any(|capability| capability == &invocation.operation)
    {
        return CodingPolicyEvaluation {
            state: "blocked",
            decision: "denied",
            reason_code: "policy_denied.capability_not_allowed",
            denial_message: Some(
                "Requested operation is not listed in manifest capabilities.".to_string(),
            ),
            write_approved,
            write_operations_mode,
            max_per_call_sats,
            operation_cost_sats,
        };
    }

    if invocation.operation == "add_issue_comment"
        && write_operations_mode == "enforce"
        && !write_approved
    {
        return CodingPolicyEvaluation {
            state: "blocked",
            decision: "denied",
            reason_code: "policy_denied.write_approval_required",
            denial_message: Some(
                "Write operation requires explicit policy.write_approved=true.".to_string(),
            ),
            write_approved,
            write_operations_mode,
            max_per_call_sats,
            operation_cost_sats,
        };
    }

    if max_per_call_sats
        .map(|max| operation_cost_sats > max)
        .unwrap_or(false)
    {
        return CodingPolicyEvaluation {
            state: "blocked",
            decision: "denied",
            reason_code: "policy_denied.max_per_call_budget_exceeded",
            denial_message: Some("Policy budget max_per_call_sats was exceeded.".to_string()),
            write_approved,
            write_operations_mode,
            max_per_call_sats,
            operation_cost_sats,
        };
    }

    CodingPolicyEvaluation {
        state: "succeeded",
        decision: "allowed",
        reason_code: "policy_allowed.default",
        denial_message: None,
        write_approved,
        write_operations_mode,
        max_per_call_sats,
        operation_cost_sats,
    }
}

fn build_coding_execution_result(
    invocation: &CodingToolInvocation,
    evaluation: &CodingPolicyEvaluation,
    replay_hash_hex: &str,
) -> serde_json::Value {
    if evaluation.decision != "allowed" {
        return serde_json::json!({
            "blocked": true,
            "operation": invocation.operation.clone(),
            "repository": invocation.repository.clone(),
        });
    }

    match invocation.operation.as_str() {
        "get_issue" => {
            let issue_number = invocation.issue_number.unwrap_or_default();
            serde_json::json!({
                "integration_id": invocation.integration_id.clone(),
                "operation": invocation.operation.clone(),
                "repository": invocation.repository.clone(),
                "issue_number": issue_number,
                "issue": {
                    "number": issue_number,
                    "title": format!("Issue #{issue_number}"),
                    "url": format!("https://github.com/{}/issues/{issue_number}", invocation.repository),
                },
            })
        }
        "get_pull_request" => {
            let pull_number = invocation.pull_number.unwrap_or_default();
            serde_json::json!({
                "integration_id": invocation.integration_id.clone(),
                "operation": invocation.operation.clone(),
                "repository": invocation.repository.clone(),
                "pull_number": pull_number,
                "pull_request": {
                    "number": pull_number,
                    "title": format!("Pull request #{pull_number}"),
                    "url": format!("https://github.com/{}/pull/{pull_number}", invocation.repository),
                },
            })
        }
        "add_issue_comment" => {
            let issue_number = invocation.issue_number.unwrap_or_default();
            serde_json::json!({
                "integration_id": invocation.integration_id.clone(),
                "operation": invocation.operation.clone(),
                "repository": invocation.repository.clone(),
                "issue_number": issue_number,
                "comment": {
                    "id": format!("comment_{}", &replay_hash_hex[..16]),
                    "body": invocation.body.clone().unwrap_or_default(),
                    "url": format!("https://github.com/{}/issues/{issue_number}#issuecomment-{}", invocation.repository, &replay_hash_hex[..12]),
                },
            })
        }
        _ => serde_json::json!({
            "blocked": true,
        }),
    }
}

fn runtime_tools_replay_hash_hex(seed: &serde_json::Value) -> String {
    let canonical = canonicalize_json(seed);
    let encoded = serde_json::to_vec(&canonical).unwrap_or_else(|_| b"{}".to_vec());
    sha256_hex(&encoded)
}

fn canonicalize_json(value: &serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(object) => {
            let mut keys = object.keys().cloned().collect::<Vec<_>>();
            keys.sort();
            let mut normalized = serde_json::Map::new();
            for key in keys {
                if let Some(entry) = object.get(&key) {
                    normalized.insert(key, canonicalize_json(entry));
                }
            }
            serde_json::Value::Object(normalized)
        }
        serde_json::Value::Array(values) => {
            serde_json::Value::Array(values.iter().map(canonicalize_json).collect())
        }
        _ => value.clone(),
    }
}

fn mark_runtime_tools_replay(mut payload: serde_json::Value) -> serde_json::Value {
    if let Some(object) = payload.as_object_mut() {
        object.insert("idempotentReplay".to_string(), serde_json::json!(true));
    }
    payload
}

fn runtime_worker_snapshot_payload(
    worker: &RuntimeWorkerRecord,
    now: chrono::DateTime<Utc>,
) -> serde_json::Value {
    let (heartbeat_state, heartbeat_age_ms) = runtime_worker_heartbeat_state(worker, now);
    serde_json::json!({
        "worker_id": worker.worker_id,
        "status": worker.status,
        "latest_seq": worker.latest_seq,
        "workspace_ref": worker.workspace_ref,
        "codex_home_ref": worker.codex_home_ref,
        "adapter": worker.adapter,
        "metadata": worker.metadata,
        "started_at": worker.started_at.map(timestamp),
        "stopped_at": worker.stopped_at.map(timestamp),
        "last_heartbeat_at": worker.last_heartbeat_at.map(timestamp),
        "heartbeat_age_ms": heartbeat_age_ms,
        "heartbeat_stale_after_ms": worker.heartbeat_stale_after_ms,
        "heartbeat_state": heartbeat_state,
        "updated_at": timestamp(worker.updated_at),
        "khala_projection": {
            "status": "in_sync",
            "lag_events": 0,
            "last_runtime_seq": worker.latest_seq,
            "last_projected_at": timestamp(worker.updated_at),
        }
    })
}

fn runtime_worker_event_payload(event: &RuntimeWorkerEventRecord) -> serde_json::Value {
    serde_json::json!({
        "seq": event.seq,
        "event_type": event.event_type,
        "payload": event.payload,
        "occurred_at": timestamp(event.occurred_at),
    })
}

fn runtime_worker_heartbeat_state(
    worker: &RuntimeWorkerRecord,
    now: chrono::DateTime<Utc>,
) -> (&'static str, Option<u64>) {
    let age_ms = worker.last_heartbeat_at.map(|heartbeat| {
        now.signed_duration_since(heartbeat)
            .num_milliseconds()
            .max(0) as u64
    });
    match worker.status.as_str() {
        "failed" => ("failed", age_ms),
        "stopped" => ("stopped", age_ms),
        _ => match age_ms {
            None => ("missing", None),
            Some(age_ms) if age_ms > worker.heartbeat_stale_after_ms => ("stale", Some(age_ms)),
            Some(age_ms) => ("fresh", Some(age_ms)),
        },
    }
}

async fn ensure_runtime_worker_runnable(
    state: &AppState,
    owner_user_id: &str,
    worker_id: &str,
    auto_provision: bool,
) -> Result<(), (StatusCode, Json<ApiErrorResponse>)> {
    let mut workers = state.runtime_workers.workers.lock().await;
    if let Some(worker) = workers.get_mut(worker_id) {
        if worker.owner_user_id != owner_user_id {
            return Err(forbidden_error(
                "worker does not belong to authenticated principal",
            ));
        }
        if matches!(worker.status.as_str(), "stopping" | "stopped" | "failed") {
            return Err(error_response_with_status(
                StatusCode::CONFLICT,
                ApiErrorCode::Conflict,
                "worker is stopped; create or reattach to resume",
            ));
        }
        return Ok(());
    }

    if !auto_provision {
        return Err(not_found_error("Not found."));
    }

    let now = Utc::now();
    workers.insert(
        worker_id.to_string(),
        RuntimeWorkerRecord {
            worker_id: worker_id.to_string(),
            owner_user_id: owner_user_id.to_string(),
            status: "running".to_string(),
            latest_seq: 0,
            workspace_ref: None,
            codex_home_ref: None,
            adapter: "in_memory".to_string(),
            metadata: None,
            started_at: Some(now),
            stopped_at: None,
            last_heartbeat_at: Some(now),
            heartbeat_stale_after_ms: 120_000,
            updated_at: now,
        },
    );

    Ok(())
}

async fn append_runtime_worker_event(
    state: &AppState,
    worker_id: &str,
    event_type: &str,
    payload: serde_json::Value,
    occurred_at: chrono::DateTime<Utc>,
) -> Result<u64, (StatusCode, Json<ApiErrorResponse>)> {
    let seq = {
        let mut workers = state.runtime_workers.workers.lock().await;
        let worker = workers
            .get_mut(worker_id)
            .ok_or_else(|| not_found_error("Not found."))?;
        worker.latest_seq = worker.latest_seq.saturating_add(1);
        worker.updated_at = occurred_at;
        if event_type == "worker.stopped" {
            worker.status = "stopped".to_string();
            worker.stopped_at = Some(occurred_at);
        }
        if event_type == "worker.heartbeat" {
            worker.last_heartbeat_at = Some(occurred_at);
        }
        worker.latest_seq
    };

    let mut events = state.runtime_workers.events.lock().await;
    events
        .entry(worker_id.to_string())
        .or_default()
        .push(RuntimeWorkerEventRecord {
            seq,
            event_type: event_type.to_string(),
            payload,
            occurred_at,
        });

    Ok(seq)
}

fn validate_worker_handshake_payload(
    event_type: &str,
    payload: &serde_json::Value,
) -> Result<(), (StatusCode, Json<ApiErrorResponse>)> {
    if event_type != "worker.event" {
        return Ok(());
    }

    let method = payload
        .get("method")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .unwrap_or_default();
    if !matches!(method, "ios/handshake" | "desktop/handshake_ack") {
        return Ok(());
    }

    for field in ["source", "method", "handshake_id", "occurred_at"] {
        let present = payload
            .get(field)
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .is_some();
        if !present {
            return Err(validation_error(
                "event.payload",
                "Handshake payload is missing required fields.",
            ));
        }
    }
    let occurred_at = payload
        .get("occurred_at")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    if parse_rfc3339_utc(occurred_at).is_err() {
        return Err(validation_error(
            "event.payload.occurred_at",
            "The occurred_at field must be a valid RFC3339 timestamp.",
        ));
    }

    if method == "ios/handshake"
        && payload
            .get("device_id")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .is_none()
    {
        return Err(validation_error(
            "event.payload.device_id",
            "ios/handshake requires device_id.",
        ));
    }
    if method == "desktop/handshake_ack"
        && payload
            .get("desktop_session_id")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .is_none()
    {
        return Err(validation_error(
            "event.payload.desktop_session_id",
            "desktop/handshake_ack requires desktop_session_id.",
        ));
    }

    Ok(())
}

fn parse_event_occurred_at(payload: &serde_json::Value) -> Option<chrono::DateTime<Utc>> {
    payload
        .get("occurred_at")
        .and_then(serde_json::Value::as_str)
        .and_then(|value| parse_rfc3339_utc(value).ok())
}

fn builtin_tool_specs() -> Vec<serde_json::Value> {
    vec![
        serde_json::json!({
            "tool_id": "github.primary",
            "version": 1,
            "tool_pack": RUNTIME_TOOL_PACK_CODING_V1,
            "state": "published",
            "tool_spec": {
                "tool_id": "github.primary",
                "version": 1,
                "tool_pack": RUNTIME_TOOL_PACK_CODING_V1,
                "name": "GitHub Primary",
                "execution_kind": "http",
                "integration_manifest": {
                    "manifest_version": "coding.integration.v1",
                    "integration_id": "github.primary",
                    "provider": "github",
                    "status": "active",
                    "tool_pack": RUNTIME_TOOL_PACK_CODING_V1,
                    "capabilities": ["get_issue", "get_pull_request", "add_issue_comment"],
                }
            },
            "created_at": "2026-02-22T00:00:00Z",
            "updated_at": "2026-02-22T00:00:00Z"
        }),
        serde_json::json!({
            "tool_id": "lightning_l402_fetch",
            "version": 1,
            "tool_pack": RUNTIME_TOOL_PACK_LIGHTNING_V1,
            "state": "published",
            "tool_spec": {
                "tool_id": "lightning_l402_fetch",
                "version": 1,
                "tool_pack": RUNTIME_TOOL_PACK_LIGHTNING_V1,
                "name": "Lightning L402 Fetch",
                "execution_kind": "http",
                "integration_manifest": {
                    "manifest_version": "lightning.integration.v1",
                    "integration_id": "lightning.l402",
                    "provider": "lightning",
                    "status": "active",
                    "tool_pack": RUNTIME_TOOL_PACK_LIGHTNING_V1,
                    "capabilities": ["lightning_l402_fetch", "lightning_l402_approve"],
                }
            },
            "created_at": "2026-02-22T00:00:00Z",
            "updated_at": "2026-02-22T00:00:00Z"
        }),
        serde_json::json!({
            "tool_id": "lightning_l402_approve",
            "version": 1,
            "tool_pack": RUNTIME_TOOL_PACK_LIGHTNING_V1,
            "state": "published",
            "tool_spec": {
                "tool_id": "lightning_l402_approve",
                "version": 1,
                "tool_pack": RUNTIME_TOOL_PACK_LIGHTNING_V1,
                "name": "Lightning L402 Approve",
                "execution_kind": "http",
                "integration_manifest": {
                    "manifest_version": "lightning.integration.v1",
                    "integration_id": "lightning.l402",
                    "provider": "lightning",
                    "status": "active",
                    "tool_pack": RUNTIME_TOOL_PACK_LIGHTNING_V1,
                    "capabilities": ["lightning_l402_fetch", "lightning_l402_approve"],
                }
            },
            "created_at": "2026-02-22T00:00:00Z",
            "updated_at": "2026-02-22T00:00:00Z"
        }),
    ]
}

fn builtin_skill_specs() -> Vec<serde_json::Value> {
    vec![
        serde_json::json!({
            "skill_id": "github-coding",
            "version": 1,
            "state": "published",
            "skill_spec": {
                "skill_id": "github-coding",
                "version": 1,
                "name": "GitHub Coding",
                "description": "Default GitHub coding workflow skill",
                "allowed_tools": [{"tool_id": "github.primary", "version": 1}],
                "compatibility": {"runtime": "runtime"},
            },
            "created_at": "2026-02-22T00:00:00Z",
            "updated_at": "2026-02-22T00:00:00Z"
        }),
        serde_json::json!({
            "skill_id": "lightning-l402",
            "version": 1,
            "state": "published",
            "skill_spec": {
                "skill_id": "lightning-l402",
                "version": 1,
                "name": "Lightning L402",
                "description": "Fetch and approve L402-protected resources.",
                "allowed_tools": [
                    {"tool_id": "lightning_l402_fetch", "version": 1},
                    {"tool_id": "lightning_l402_approve", "version": 1}
                ],
                "compatibility": {"runtime": "runtime"},
            },
            "created_at": "2026-02-22T00:00:00Z",
            "updated_at": "2026-02-22T00:00:00Z"
        }),
    ]
}

fn builtin_skill_spec(skill_id: &str, version: u64) -> Option<serde_json::Value> {
    builtin_skill_specs().into_iter().find(|item| {
        item.get("skill_id")
            .and_then(serde_json::Value::as_str)
            .map(|candidate| candidate == skill_id)
            .unwrap_or(false)
            && item
                .get("version")
                .and_then(serde_json::Value::as_u64)
                .map(|candidate| candidate == version)
                .unwrap_or(false)
    })
}

fn registry_key(id: &str, version: u64) -> String {
    format!("{}::{version}", id.trim().to_lowercase())
}

fn registry_item_key(item: &serde_json::Value, id_field: &str) -> String {
    let id = item
        .get(id_field)
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    let version = item
        .get("version")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    registry_key(id, version)
}

fn registry_sort_key(item: &serde_json::Value, id_field: &str) -> String {
    let id = item
        .get(id_field)
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_lowercase();
    let version = item
        .get("version")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    format!("{id}:{version:020}")
}

fn normalize_registry_state(
    value: Option<String>,
    field: &'static str,
) -> Result<String, (StatusCode, Json<ApiErrorResponse>)> {
    let normalized = value
        .and_then(non_empty)
        .unwrap_or_else(|| "validated".to_string())
        .to_lowercase();
    if !matches!(
        normalized.as_str(),
        "draft" | "validated" | "published" | "deprecated"
    ) {
        return Err(validation_error(field, "The selected state is invalid."));
    }
    Ok(normalized)
}

fn validate_tool_spec_schema(
    tool_spec: &serde_json::Map<String, serde_json::Value>,
) -> Result<(String, u64, String), (StatusCode, Json<ApiErrorResponse>)> {
    let tool_id = tool_spec
        .get("tool_id")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .ok_or_else(|| {
            validation_error(
                "tool_spec.tool_id",
                "The tool_spec.tool_id field is required.",
            )
        })?;
    if tool_id.chars().count() > 160 {
        return Err(validation_error(
            "tool_spec.tool_id",
            "The tool_spec.tool_id field may not be greater than 160 characters.",
        ));
    }

    let version = tool_spec
        .get("version")
        .and_then(positive_u64_from_value)
        .ok_or_else(|| {
            validation_error(
                "tool_spec.version",
                "The tool_spec.version field must be an integer.",
            )
        })?;
    if version < 1 {
        return Err(validation_error(
            "tool_spec.version",
            "The tool_spec.version field must be at least 1.",
        ));
    }

    let tool_pack = tool_spec
        .get("tool_pack")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .ok_or_else(|| {
            validation_error(
                "tool_spec.tool_pack",
                "The tool_spec.tool_pack field is required.",
            )
        })?;
    let normalized_tool_pack = tool_pack.to_ascii_lowercase();
    let canonical_tool_pack = match normalized_tool_pack.as_str() {
        RUNTIME_TOOL_PACK_CODING_V1 => RUNTIME_TOOL_PACK_CODING_V1,
        RUNTIME_TOOL_PACK_LIGHTNING_V1 | RUNTIME_TOOL_PACK_L402_ALIAS_V1 => {
            RUNTIME_TOOL_PACK_LIGHTNING_V1
        }
        _ => {
            return Err(validation_error(
                "tool_spec.tool_pack",
                "Only coding.v1 and lightning.v1 tool packs are currently supported.",
            ));
        }
    };
    if !matches!(
        canonical_tool_pack,
        RUNTIME_TOOL_PACK_CODING_V1 | RUNTIME_TOOL_PACK_LIGHTNING_V1
    ) {
        return Err(validation_error(
            "tool_spec.tool_pack",
            "The selected tool_spec.tool_pack is invalid.",
        ));
    }

    if let Some(integration_manifest) = tool_spec.get("integration_manifest") {
        if !integration_manifest.is_object() {
            return Err(validation_error(
                "tool_spec.integration_manifest",
                "The tool_spec.integration_manifest field must be an array.",
            ));
        }
        let manifest_version = integration_manifest
            .get("manifest_version")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty());
        if manifest_version.is_none() {
            return Err(validation_error(
                "tool_spec.integration_manifest.manifest_version",
                "The tool_spec.integration_manifest.manifest_version field is required.",
            ));
        }
    }

    Ok((tool_id, version, canonical_tool_pack.to_string()))
}

fn validate_skill_spec_schema(
    skill_spec: &serde_json::Map<String, serde_json::Value>,
) -> Result<(String, u64), (StatusCode, Json<ApiErrorResponse>)> {
    let skill_id = skill_spec
        .get("skill_id")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .ok_or_else(|| {
            validation_error(
                "skill_spec.skill_id",
                "The skill_spec.skill_id field is required.",
            )
        })?;
    if skill_id.chars().count() > 160 {
        return Err(validation_error(
            "skill_spec.skill_id",
            "The skill_spec.skill_id field may not be greater than 160 characters.",
        ));
    }

    let version = skill_spec
        .get("version")
        .and_then(positive_u64_from_value)
        .ok_or_else(|| {
            validation_error(
                "skill_spec.version",
                "The skill_spec.version field must be an integer.",
            )
        })?;
    if version < 1 {
        return Err(validation_error(
            "skill_spec.version",
            "The skill_spec.version field must be at least 1.",
        ));
    }

    if let Some(allowed_tools) = skill_spec.get("allowed_tools") {
        let entries = allowed_tools.as_array().ok_or_else(|| {
            validation_error(
                "skill_spec.allowed_tools",
                "The skill_spec.allowed_tools field must be an array.",
            )
        })?;
        for entry in entries {
            let entry_object = entry.as_object().ok_or_else(|| {
                validation_error(
                    "skill_spec.allowed_tools",
                    "Each skill_spec.allowed_tools entry must be an object.",
                )
            })?;
            let tool_id = entry_object
                .get("tool_id")
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty());
            if tool_id.is_none() {
                return Err(validation_error(
                    "skill_spec.allowed_tools",
                    "Each allowed tool entry requires tool_id.",
                ));
            }
            let tool_version = entry_object
                .get("version")
                .and_then(positive_u64_from_value)
                .filter(|value| *value >= 1);
            if tool_version.is_none() {
                return Err(validation_error(
                    "skill_spec.allowed_tools",
                    "Each allowed tool entry requires version >= 1.",
                ));
            }
        }
    }

    Ok((skill_id, version))
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

fn autopilot_prompt_context(aggregate: &AutopilotAggregate) -> Option<String> {
    let mut lines = vec![
        format!("autopilot_id={}", aggregate.autopilot.id),
        format!(
            "config_version={}",
            aggregate.autopilot.config_version.max(1)
        ),
        format!("handle={}", aggregate.autopilot.handle),
    ];

    if let Some(display_name) = non_empty(aggregate.autopilot.display_name.clone()) {
        lines.push(format!("display_name={display_name}"));
    }
    if let Some(owner_display_name) = non_empty(aggregate.profile.owner_display_name.clone()) {
        lines.push(format!("owner_display_name={owner_display_name}"));
    }
    if let Some(tagline) = aggregate.autopilot.tagline.clone().and_then(non_empty) {
        lines.push(format!("tagline={tagline}"));
    }
    if let Some(persona_summary) = aggregate
        .profile
        .persona_summary
        .clone()
        .and_then(non_empty)
    {
        lines.push(format!("persona_summary={persona_summary}"));
    }
    if let Some(autopilot_voice) = aggregate
        .profile
        .autopilot_voice
        .clone()
        .and_then(non_empty)
    {
        lines.push(format!("autopilot_voice={autopilot_voice}"));
    }

    let principles = value_string_array(aggregate.profile.principles.as_ref());
    if !principles.is_empty() {
        lines.push(format!("principles={}", principles.join(" | ")));
    }

    if let Some(preferences) = aggregate
        .profile
        .preferences
        .as_ref()
        .and_then(serde_json::Value::as_object)
    {
        if let Some(user_preferences) = preferences
            .get("user")
            .and_then(serde_json::Value::as_object)
        {
            if let Some(address_as) = normalized_json_string(user_preferences.get("addressAs")) {
                lines.push(format!("preferred_address={address_as}"));
            }
            if let Some(time_zone) = normalized_json_string(user_preferences.get("timeZone")) {
                lines.push(format!("time_zone={time_zone}"));
            }
        }

        if let Some(character_preferences) = preferences
            .get("character")
            .and_then(serde_json::Value::as_object)
        {
            let boundaries = character_preferences
                .get("boundaries")
                .and_then(serde_json::Value::as_array)
                .map(|values| {
                    values
                        .iter()
                        .filter_map(|value| {
                            non_empty(value.as_str().unwrap_or_default().to_string())
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            if !boundaries.is_empty() {
                lines.push(format!("boundaries={}", boundaries.join(" | ")));
            }
        }
    }

    if let Some(onboarding) = aggregate
        .profile
        .onboarding_answers
        .as_object()
        .and_then(|object| object.get("bootstrapState"))
        .and_then(serde_json::Value::as_object)
    {
        let status = normalized_json_string(onboarding.get("status"));
        let stage = normalized_json_string(onboarding.get("stage"));
        if status.is_some() || stage.is_some() {
            let label = format!(
                "{}{}",
                status.unwrap_or_else(|| "unknown".to_string()),
                stage.map(|value| format!(" @ {value}")).unwrap_or_default()
            );
            lines.push(format!("onboarding={label}"));
        }
    }

    if !aggregate.policy.tool_allowlist.is_empty() {
        lines.push(format!(
            "tool_allowlist={}",
            aggregate.policy.tool_allowlist.join(",")
        ));
    }
    if !aggregate.policy.tool_denylist.is_empty() {
        lines.push(format!(
            "tool_denylist={}",
            aggregate.policy.tool_denylist.join(",")
        ));
    }
    lines.push(format!(
        "l402_require_approval={}",
        if aggregate.policy.l402_require_approval {
            "true"
        } else {
            "false"
        }
    ));
    if let Some(max_spend_per_call) = aggregate.policy.l402_max_spend_msats_per_call {
        lines.push(format!(
            "l402_max_spend_msats_per_call={max_spend_per_call}"
        ));
    }
    if let Some(max_spend_per_day) = aggregate.policy.l402_max_spend_msats_per_day {
        lines.push(format!("l402_max_spend_msats_per_day={max_spend_per_day}"));
    }
    if !aggregate.policy.l402_allowed_hosts.is_empty() {
        lines.push(format!(
            "l402_allowed_hosts={}",
            aggregate.policy.l402_allowed_hosts.join(",")
        ));
    }

    let context = lines.join("\n").trim().to_string();
    if context.is_empty() {
        return None;
    }

    Some(context.chars().take(3200).collect())
}

fn autopilot_tool_resolution_audit(
    aggregate: &AutopilotAggregate,
    session_authenticated: bool,
) -> serde_json::Value {
    if !session_authenticated {
        let available_tools = tool_name_list_from_static(AUTOPILOT_ALL_TOOLS);
        let guest_allowlist = tool_name_list_from_static(AUTOPILOT_GUEST_ALLOWED_TOOLS);
        let exposed_tools = guest_allowlist
            .iter()
            .filter(|name| available_tools.contains(*name))
            .cloned()
            .collect::<Vec<_>>();
        let removed_by_auth_gate = available_tools
            .iter()
            .filter(|name| !exposed_tools.contains(*name))
            .cloned()
            .collect::<Vec<_>>();
        return serde_json::json!({
            "policyApplied": false,
            "authRestricted": true,
            "sessionAuthenticated": false,
            "autopilotId": serde_json::Value::Null,
            "availableTools": available_tools,
            "exposedTools": exposed_tools,
            "allowlist": guest_allowlist,
            "denylist": [],
            "removedByAllowlist": [],
            "removedByDenylist": [],
            "removedByAuthGate": removed_by_auth_gate,
        });
    }

    let available_tools = tool_name_list_from_static(AUTOPILOT_AUTHENTICATED_TOOLS);
    let allowlist = tool_name_list(&aggregate.policy.tool_allowlist);
    let denylist = tool_name_list(&aggregate.policy.tool_denylist);
    let (exposed_tools, removed_by_allowlist, removed_by_denylist) =
        resolve_tool_names(&available_tools, &allowlist, &denylist);

    serde_json::json!({
        "policyApplied": true,
        "authRestricted": false,
        "sessionAuthenticated": true,
        "autopilotId": aggregate.autopilot.id.clone(),
        "availableTools": available_tools,
        "exposedTools": exposed_tools,
        "allowlist": allowlist,
        "denylist": denylist,
        "removedByAllowlist": removed_by_allowlist,
        "removedByDenylist": removed_by_denylist,
        "removedByAuthGate": [],
    })
}

fn autopilot_runtime_binding_worker_ref(aggregate: &AutopilotAggregate) -> Option<String> {
    let binding = aggregate
        .runtime_bindings
        .iter()
        .find(|binding| binding.is_primary)
        .or_else(|| aggregate.runtime_bindings.first())?;
    binding.runtime_ref.clone().and_then(non_empty)
}

fn autopilot_runtime_binding_payload(aggregate: &AutopilotAggregate) -> serde_json::Value {
    let Some(binding) = aggregate
        .runtime_bindings
        .iter()
        .find(|binding| binding.is_primary)
        .or_else(|| aggregate.runtime_bindings.first())
    else {
        return serde_json::Value::Null;
    };

    serde_json::json!({
        "id": binding.id,
        "runtimeType": binding.runtime_type,
        "runtimeRef": binding.runtime_ref,
        "isPrimary": binding.is_primary,
        "driverHint": runtime_driver_hint(&binding.runtime_type),
        "lastSeenAt": binding.last_seen_at.map(timestamp),
        "meta": binding.meta,
        "createdAt": timestamp(binding.created_at),
        "updatedAt": timestamp(binding.updated_at),
    })
}

fn runtime_driver_hint(runtime_type: &str) -> Option<&'static str> {
    match runtime_type.trim().to_ascii_lowercase().as_str() {
        "runtime_service" | "runtime" | "elixir" => Some("runtime_service"),
        "control_service" | "control" | "legacy" | "laravel" | "openagents.com" => {
            Some("control_service")
        }
        _ => None,
    }
}

fn resolve_tool_names(
    available_tool_names: &[String],
    allowlist: &[String],
    denylist: &[String],
) -> (Vec<String>, Vec<String>, Vec<String>) {
    let mut candidate = Vec::new();
    let mut removed_by_allowlist = Vec::new();
    for name in available_tool_names {
        if !allowlist.is_empty() && !allowlist.contains(name) {
            removed_by_allowlist.push(name.clone());
            continue;
        }
        candidate.push(name.clone());
    }

    let mut exposed = Vec::new();
    let mut removed_by_denylist = Vec::new();
    for name in candidate {
        if denylist.contains(&name) {
            removed_by_denylist.push(name);
            continue;
        }
        exposed.push(name);
    }

    (exposed, removed_by_allowlist, removed_by_denylist)
}

fn tool_name_list_from_static(values: &[&str]) -> Vec<String> {
    let mut names = Vec::new();
    for value in values {
        if let Some(normalized) = normalize_tool_name(value) {
            if !names.contains(&normalized) {
                names.push(normalized);
            }
        }
    }
    names
}

fn tool_name_list(values: &[String]) -> Vec<String> {
    let mut names = Vec::new();
    for value in values {
        if let Some(normalized) = normalize_tool_name(value) {
            if !names.contains(&normalized) {
                names.push(normalized);
            }
        }
    }
    names
}

fn normalize_tool_name(value: &str) -> Option<String> {
    let normalized = value.trim().to_ascii_lowercase();
    non_empty(normalized)
}

fn value_string_array(value: Option<&serde_json::Value>) -> Vec<String> {
    value
        .and_then(serde_json::Value::as_array)
        .map(|entries| {
            entries
                .iter()
                .filter_map(|entry| entry.as_str().and_then(|raw| non_empty(raw.to_string())))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
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
mod tests;
