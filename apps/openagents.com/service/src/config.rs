use std::env;
use std::net::SocketAddr;
use std::path::PathBuf;

use thiserror::Error;

const DEFAULT_BIND_ADDR: &str = "127.0.0.1:8787";
const DEFAULT_LOG_FILTER: &str = "info";
const DEFAULT_STATIC_DIR: &str = "apps/openagents.com/service/static";
const DEFAULT_DESKTOP_DOWNLOAD_URL: &str =
    "https://github.com/OpenAgentsInc/openagents/releases/latest";
const DEFAULT_AUTH_PROVIDER_MODE: &str = "workos";
const DEFAULT_WORKOS_API_BASE_URL: &str = "https://api.workos.com";
const DEFAULT_MOCK_MAGIC_CODE: &str = "123456";
const DEFAULT_AUTH_CHALLENGE_TTL_SECONDS: u64 = 600;
const DEFAULT_AUTH_ACCESS_TTL_SECONDS: u64 = 3600;
const DEFAULT_AUTH_REFRESH_TTL_SECONDS: u64 = 2_592_000;
const DEFAULT_AUTH_LOCAL_TEST_LOGIN_ENABLED: bool = false;
const DEFAULT_AUTH_LOCAL_TEST_LOGIN_ALLOWED_EMAILS: &str = "";
const DEFAULT_AUTH_LOCAL_TEST_LOGIN_SIGNING_KEY: &str = "";
const DEFAULT_AUTH_API_SIGNUP_ENABLED: bool = false;
const DEFAULT_AUTH_API_SIGNUP_ALLOWED_DOMAINS: &str = "";
const DEFAULT_AUTH_API_SIGNUP_DEFAULT_TOKEN_NAME: &str = "api-bootstrap";
const DEFAULT_ADMIN_EMAILS: &str = "";
const DEFAULT_SYNC_TOKEN_ISSUER: &str = "https://openagents.com";
const DEFAULT_SYNC_TOKEN_AUDIENCE: &str = "openagents-sync";
const DEFAULT_SYNC_TOKEN_KEY_ID: &str = "sync-auth-v1";
const DEFAULT_SYNC_TOKEN_CLAIMS_VERSION: &str = "oa_sync_claims_v1";
const DEFAULT_SYNC_TOKEN_TTL_SECONDS: u32 = 300;
const DEFAULT_SYNC_TOKEN_MIN_TTL_SECONDS: u32 = 60;
const DEFAULT_SYNC_TOKEN_MAX_TTL_SECONDS: u32 = 900;
const DEFAULT_SYNC_ALLOWED_SCOPES: &str =
    "runtime.codex_worker_events,runtime.codex_worker_summaries,runtime.run_summaries";
const DEFAULT_SYNC_DEFAULT_SCOPES: &str = "runtime.codex_worker_events";
const DEFAULT_ROUTE_SPLIT_MODE: &str = "rust";
const DEFAULT_ROUTE_SPLIT_RUST_ROUTES: &str = "/";
const DEFAULT_ROUTE_SPLIT_COHORT_PERCENTAGE: u8 = 100;
const DEFAULT_ROUTE_SPLIT_SALT: &str = "openagents-route-split-v1";
const DEFAULT_RUNTIME_SYNC_REVOKE_PATH: &str = "/internal/v1/sync/sessions/revoke";
const DEFAULT_RUNTIME_SIGNATURE_TTL_SECONDS: u64 = 60;
const DEFAULT_RUNTIME_INTERNAL_KEY_ID: &str = "runtime-internal-v1";
const DEFAULT_RUNTIME_INTERNAL_SIGNATURE_TTL_SECONDS: u64 = 60;
const DEFAULT_RUNTIME_INTERNAL_SECRET_FETCH_PATH: &str =
    "/api/internal/runtime/integrations/secrets/fetch";
const DEFAULT_RUNTIME_INTERNAL_SECRET_CACHE_TTL_MS: u64 = 60_000;
const DEFAULT_RUNTIME_SIGNING_KEY_ID: &str = "runtime-v1";
const DEFAULT_RUNTIME_COMMS_DELIVERY_INGEST_PATH: &str = "/internal/v1/comms/delivery-events";
const DEFAULT_RUNTIME_COMMS_DELIVERY_TIMEOUT_MS: u64 = 10_000;
const DEFAULT_RUNTIME_COMMS_DELIVERY_MAX_RETRIES: u32 = 2;
const DEFAULT_RUNTIME_COMMS_DELIVERY_RETRY_BACKOFF_MS: u64 = 200;
const DEFAULT_LIQUIDITY_STATS_POOL_IDS: &str = "llp-main";
const DEFAULT_RESEND_WEBHOOK_TOLERANCE_SECONDS: u64 = 300;
const DEFAULT_GOOGLE_OAUTH_SCOPES: &str = "https://www.googleapis.com/auth/gmail.readonly https://www.googleapis.com/auth/gmail.compose https://www.googleapis.com/auth/gmail.send";
const DEFAULT_GOOGLE_OAUTH_TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
const DEFAULT_GOOGLE_GMAIL_API_BASE_URL: &str = "https://gmail.googleapis.com";
const DEFAULT_RUNTIME_DRIVER: &str = "control_service";
const DEFAULT_RUNTIME_FORCE_CONTROL_SERVICE: bool = false;
const DEFAULT_RUNTIME_CANARY_USER_PERCENT: u8 = 0;
const DEFAULT_RUNTIME_CANARY_AUTOPILOT_PERCENT: u8 = 0;
const DEFAULT_RUNTIME_CANARY_SEED: &str = "runtime-canary-v1";
const DEFAULT_RUNTIME_OVERRIDES_ENABLED: bool = true;
const DEFAULT_RUNTIME_SHADOW_ENABLED: bool = false;
const DEFAULT_RUNTIME_SHADOW_SAMPLE_RATE: f64 = 1.0;
const DEFAULT_RUNTIME_SHADOW_MAX_CAPTURE_BYTES: u64 = 200_000;
const DEFAULT_MAINTENANCE_MODE_ENABLED: bool = false;
const DEFAULT_MAINTENANCE_BYPASS_COOKIE_NAME: &str = "oa_maintenance_bypass";
const DEFAULT_MAINTENANCE_BYPASS_COOKIE_TTL_SECONDS: u64 = 900;
const DEFAULT_MAINTENANCE_ALLOWED_PATHS: &str = "/healthz,/readyz";
const DEFAULT_COMPAT_CONTROL_ENFORCED: bool = false;
const DEFAULT_COMPAT_CONTROL_PROTOCOL_VERSION: &str = "openagents.control.v1";
const DEFAULT_COMPAT_CONTROL_MIN_CLIENT_BUILD_ID: &str = "00000000T000000Z";
const DEFAULT_COMPAT_CONTROL_MIN_SCHEMA_VERSION: u32 = 1;
const DEFAULT_COMPAT_CONTROL_MAX_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone)]
pub struct Config {
    pub bind_addr: SocketAddr,
    pub log_filter: String,
    pub static_dir: PathBuf,
    pub desktop_download_url: String,
    pub auth_provider_mode: String,
    pub workos_client_id: Option<String>,
    pub workos_api_key: Option<String>,
    pub workos_api_base_url: String,
    pub mock_magic_code: String,
    pub auth_local_test_login_enabled: bool,
    pub auth_local_test_login_allowed_emails: Vec<String>,
    pub auth_local_test_login_signing_key: Option<String>,
    pub auth_api_signup_enabled: bool,
    pub auth_api_signup_allowed_domains: Vec<String>,
    pub auth_api_signup_default_token_name: String,
    pub admin_emails: Vec<String>,
    pub auth_store_path: Option<PathBuf>,
    pub auth_challenge_ttl_seconds: u64,
    pub auth_access_ttl_seconds: u64,
    pub auth_refresh_ttl_seconds: u64,
    pub sync_token_enabled: bool,
    pub sync_token_signing_key: Option<String>,
    pub sync_token_issuer: String,
    pub sync_token_audience: String,
    pub sync_token_key_id: String,
    pub sync_token_claims_version: String,
    pub sync_token_ttl_seconds: u32,
    pub sync_token_min_ttl_seconds: u32,
    pub sync_token_max_ttl_seconds: u32,
    pub sync_token_allowed_scopes: Vec<String>,
    pub sync_token_default_scopes: Vec<String>,
    pub route_split_enabled: bool,
    pub route_split_mode: String,
    pub route_split_rust_routes: Vec<String>,
    pub route_split_cohort_percentage: u8,
    pub route_split_salt: String,
    pub route_split_force_legacy: bool,
    pub route_split_legacy_base_url: Option<String>,
    pub runtime_sync_revoke_base_url: Option<String>,
    pub runtime_sync_revoke_path: String,
    pub runtime_signature_secret: Option<String>,
    pub runtime_signature_ttl_seconds: u64,
    pub runtime_internal_shared_secret: Option<String>,
    pub runtime_internal_key_id: String,
    pub runtime_internal_signature_ttl_seconds: u64,
    pub runtime_internal_secret_fetch_path: String,
    pub runtime_internal_secret_cache_ttl_ms: u64,
    pub runtime_base_url: Option<String>,
    pub liquidity_stats_pool_ids: Vec<String>,
    pub runtime_signing_key: Option<String>,
    pub runtime_signing_key_id: String,
    pub runtime_comms_delivery_ingest_path: String,
    pub runtime_comms_delivery_timeout_ms: u64,
    pub runtime_comms_delivery_max_retries: u32,
    pub runtime_comms_delivery_retry_backoff_ms: u64,
    pub smoke_stream_secret: Option<String>,
    pub resend_webhook_secret: Option<String>,
    pub resend_webhook_tolerance_seconds: u64,
    pub google_oauth_client_id: Option<String>,
    pub google_oauth_client_secret: Option<String>,
    pub google_oauth_redirect_uri: Option<String>,
    pub google_oauth_scopes: String,
    pub google_oauth_token_url: String,
    pub google_gmail_api_base_url: String,
    pub runtime_driver: String,
    pub runtime_force_driver: Option<String>,
    pub runtime_force_control_service: bool,
    pub runtime_canary_user_percent: u8,
    pub runtime_canary_autopilot_percent: u8,
    pub runtime_canary_seed: String,
    pub runtime_overrides_enabled: bool,
    pub runtime_shadow_enabled: bool,
    pub runtime_shadow_sample_rate: f64,
    pub runtime_shadow_max_capture_bytes: u64,
    pub codex_thread_store_path: Option<PathBuf>,
    pub domain_store_path: Option<PathBuf>,
    pub maintenance_mode_enabled: bool,
    pub maintenance_bypass_token: Option<String>,
    pub maintenance_bypass_cookie_name: String,
    pub maintenance_bypass_cookie_ttl_seconds: u64,
    pub maintenance_allowed_paths: Vec<String>,
    pub compat_control_enforced: bool,
    pub compat_control_protocol_version: String,
    pub compat_control_min_client_build_id: String,
    pub compat_control_max_client_build_id: Option<String>,
    pub compat_control_min_schema_version: u32,
    pub compat_control_max_schema_version: u32,
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("invalid OA_CONTROL_BIND_ADDR value '{value}': {source}")]
    InvalidBindAddr {
        value: String,
        source: std::net::AddrParseError,
    },
}

impl Config {
    pub fn from_env() -> Result<Self, ConfigError> {
        let bind_addr_raw = env::var("OA_CONTROL_BIND_ADDR")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_BIND_ADDR.to_string());

        let bind_addr = bind_addr_raw
            .parse()
            .map_err(|source| ConfigError::InvalidBindAddr {
                value: bind_addr_raw,
                source,
            })?;

        let log_filter = env::var("OA_CONTROL_LOG_FILTER")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_LOG_FILTER.to_string());

        let static_dir = env::var("OA_CONTROL_STATIC_DIR")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(DEFAULT_STATIC_DIR));

        let desktop_download_url = env::var("OA_DESKTOP_DOWNLOAD_URL")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_DESKTOP_DOWNLOAD_URL.to_string());

        let auth_provider_mode = env::var("OA_AUTH_PROVIDER_MODE")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_AUTH_PROVIDER_MODE.to_string())
            .trim()
            .to_lowercase();

        let workos_client_id = env::var("WORKOS_CLIENT_ID")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());

        let workos_api_key = env::var("WORKOS_API_KEY")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());

        let workos_api_base_url = env::var("OA_WORKOS_API_BASE_URL")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_WORKOS_API_BASE_URL.to_string());

        let mock_magic_code = env::var("OA_AUTH_MOCK_MAGIC_CODE")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_MOCK_MAGIC_CODE.to_string());

        let auth_local_test_login_enabled = env::var("OA_AUTH_LOCAL_TEST_LOGIN_ENABLED")
            .ok()
            .map(|value| matches!(value.trim().to_lowercase().as_str(), "1" | "true" | "yes"))
            .unwrap_or(DEFAULT_AUTH_LOCAL_TEST_LOGIN_ENABLED);

        let auth_local_test_login_allowed_emails = parse_csv(
            env::var("OA_AUTH_LOCAL_TEST_LOGIN_ALLOWED_EMAILS")
                .ok()
                .unwrap_or_else(|| DEFAULT_AUTH_LOCAL_TEST_LOGIN_ALLOWED_EMAILS.to_string()),
        )
        .into_iter()
        .map(|email| email.to_lowercase())
        .collect();

        let auth_local_test_login_signing_key = env::var("OA_AUTH_LOCAL_TEST_LOGIN_SIGNING_KEY")
            .ok()
            .or_else(|| {
                let default = DEFAULT_AUTH_LOCAL_TEST_LOGIN_SIGNING_KEY.trim();
                if default.is_empty() {
                    None
                } else {
                    Some(default.to_string())
                }
            })
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());

        let auth_api_signup_enabled = env::var("OA_AUTH_API_SIGNUP_ENABLED")
            .ok()
            .map(|value| matches!(value.trim().to_lowercase().as_str(), "1" | "true" | "yes"))
            .unwrap_or(DEFAULT_AUTH_API_SIGNUP_ENABLED);

        let auth_api_signup_allowed_domains = parse_csv(
            env::var("OA_AUTH_API_SIGNUP_ALLOWED_DOMAINS")
                .ok()
                .unwrap_or_else(|| DEFAULT_AUTH_API_SIGNUP_ALLOWED_DOMAINS.to_string()),
        )
        .into_iter()
        .map(|domain| domain.to_lowercase())
        .collect();

        let auth_api_signup_default_token_name = env::var("OA_AUTH_API_SIGNUP_DEFAULT_TOKEN_NAME")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_AUTH_API_SIGNUP_DEFAULT_TOKEN_NAME.to_string());

        let admin_emails = parse_csv(
            env::var("OA_ADMIN_EMAILS")
                .ok()
                .unwrap_or_else(|| DEFAULT_ADMIN_EMAILS.to_string()),
        )
        .into_iter()
        .map(|email| email.to_lowercase())
        .collect();

        let auth_store_path = env::var("OA_AUTH_STORE_PATH")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .map(PathBuf::from);

        let auth_challenge_ttl_seconds = env::var("OA_AUTH_CHALLENGE_TTL_SECONDS")
            .ok()
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(DEFAULT_AUTH_CHALLENGE_TTL_SECONDS);

        let auth_access_ttl_seconds = env::var("OA_AUTH_ACCESS_TTL_SECONDS")
            .ok()
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(DEFAULT_AUTH_ACCESS_TTL_SECONDS);

        let auth_refresh_ttl_seconds = env::var("OA_AUTH_REFRESH_TTL_SECONDS")
            .ok()
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(DEFAULT_AUTH_REFRESH_TTL_SECONDS);

        let sync_token_enabled = env::var("OA_SYNC_TOKEN_ENABLED")
            .ok()
            .map(|value| matches!(value.trim().to_lowercase().as_str(), "1" | "true" | "yes"))
            .unwrap_or(true);

        let sync_token_signing_key = env::var("OA_SYNC_TOKEN_SIGNING_KEY")
            .ok()
            .or_else(|| env::var("SYNC_TOKEN_SIGNING_KEY").ok())
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());

        let sync_token_issuer = env::var("OA_SYNC_TOKEN_ISSUER")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_SYNC_TOKEN_ISSUER.to_string());

        let sync_token_audience = env::var("OA_SYNC_TOKEN_AUDIENCE")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_SYNC_TOKEN_AUDIENCE.to_string());

        let sync_token_key_id = env::var("OA_SYNC_TOKEN_KEY_ID")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_SYNC_TOKEN_KEY_ID.to_string());

        let sync_token_claims_version = env::var("OA_SYNC_TOKEN_CLAIMS_VERSION")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_SYNC_TOKEN_CLAIMS_VERSION.to_string());

        let sync_token_ttl_seconds = env::var("OA_SYNC_TOKEN_TTL_SECONDS")
            .ok()
            .and_then(|value| value.parse::<u32>().ok())
            .unwrap_or(DEFAULT_SYNC_TOKEN_TTL_SECONDS);

        let sync_token_min_ttl_seconds = env::var("OA_SYNC_TOKEN_MIN_TTL_SECONDS")
            .ok()
            .and_then(|value| value.parse::<u32>().ok())
            .unwrap_or(DEFAULT_SYNC_TOKEN_MIN_TTL_SECONDS);

        let sync_token_max_ttl_seconds = env::var("OA_SYNC_TOKEN_MAX_TTL_SECONDS")
            .ok()
            .and_then(|value| value.parse::<u32>().ok())
            .unwrap_or(DEFAULT_SYNC_TOKEN_MAX_TTL_SECONDS);

        let sync_token_allowed_scopes = parse_csv(
            env::var("OA_SYNC_TOKEN_ALLOWED_SCOPES")
                .ok()
                .unwrap_or_else(|| DEFAULT_SYNC_ALLOWED_SCOPES.to_string()),
        );

        let sync_token_default_scopes = parse_csv(
            env::var("OA_SYNC_TOKEN_DEFAULT_SCOPES")
                .ok()
                .unwrap_or_else(|| DEFAULT_SYNC_DEFAULT_SCOPES.to_string()),
        );

        let route_split_enabled = env::var("OA_ROUTE_SPLIT_ENABLED")
            .ok()
            .map(|value| matches!(value.trim().to_lowercase().as_str(), "1" | "true" | "yes"))
            .unwrap_or(true);

        let route_split_mode = env::var("OA_ROUTE_SPLIT_MODE")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_ROUTE_SPLIT_MODE.to_string())
            .trim()
            .to_lowercase();

        let route_split_rust_routes = parse_csv(
            env::var("OA_ROUTE_SPLIT_RUST_ROUTES")
                .ok()
                .unwrap_or_else(|| DEFAULT_ROUTE_SPLIT_RUST_ROUTES.to_string()),
        );

        let route_split_cohort_percentage = env::var("OA_ROUTE_SPLIT_COHORT_PERCENTAGE")
            .ok()
            .and_then(|value| value.parse::<u8>().ok())
            .unwrap_or(DEFAULT_ROUTE_SPLIT_COHORT_PERCENTAGE)
            .min(100);

        let route_split_salt = env::var("OA_ROUTE_SPLIT_SALT")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_ROUTE_SPLIT_SALT.to_string());

        let route_split_force_legacy = env::var("OA_ROUTE_SPLIT_FORCE_LEGACY")
            .ok()
            .map(|value| matches!(value.trim().to_lowercase().as_str(), "1" | "true" | "yes"))
            .unwrap_or(false);

        let route_split_legacy_base_url = env::var("OA_ROUTE_SPLIT_LEGACY_BASE_URL")
            .ok()
            .map(|value| value.trim().trim_end_matches('/').to_string())
            .filter(|value| !value.is_empty());

        let runtime_sync_revoke_base_url = env::var("OA_RUNTIME_SYNC_REVOKE_BASE_URL")
            .ok()
            .map(|value| value.trim().trim_end_matches('/').to_string())
            .filter(|value| !value.is_empty());

        let runtime_sync_revoke_path = env::var("OA_RUNTIME_SYNC_REVOKE_PATH")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_RUNTIME_SYNC_REVOKE_PATH.to_string());

        let runtime_signature_secret = env::var("OA_RUNTIME_SIGNATURE_SECRET")
            .ok()
            .or_else(|| env::var("RUNTIME_SIGNATURE_SECRET").ok())
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());

        let runtime_signature_ttl_seconds = env::var("OA_RUNTIME_SIGNATURE_TTL_SECONDS")
            .ok()
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(DEFAULT_RUNTIME_SIGNATURE_TTL_SECONDS);

        let runtime_internal_shared_secret = env::var("OA_RUNTIME_INTERNAL_SHARED_SECRET")
            .ok()
            .or_else(|| env::var("RUNTIME_INTERNAL_SHARED_SECRET").ok())
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());

        let runtime_internal_key_id = env::var("OA_RUNTIME_INTERNAL_KEY_ID")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_RUNTIME_INTERNAL_KEY_ID.to_string());

        let runtime_internal_signature_ttl_seconds =
            env::var("OA_RUNTIME_INTERNAL_SIGNATURE_TTL_SECONDS")
                .ok()
                .and_then(|value| value.parse::<u64>().ok())
                .unwrap_or(DEFAULT_RUNTIME_INTERNAL_SIGNATURE_TTL_SECONDS)
                .max(1);

        let runtime_internal_secret_fetch_path = env::var("OA_RUNTIME_INTERNAL_SECRET_FETCH_PATH")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| DEFAULT_RUNTIME_INTERNAL_SECRET_FETCH_PATH.to_string());

        let runtime_internal_secret_cache_ttl_ms =
            env::var("OA_RUNTIME_INTERNAL_SECRET_CACHE_TTL_MS")
                .ok()
                .and_then(|value| value.parse::<u64>().ok())
                .unwrap_or(DEFAULT_RUNTIME_INTERNAL_SECRET_CACHE_TTL_MS);

        let runtime_base_url = env::var("OA_RUNTIME_BASE_URL")
            .ok()
            .or_else(|| env::var("OA_RUNTIME_ELIXIR_BASE_URL").ok())
            .map(|value| value.trim().trim_end_matches('/').to_string())
            .filter(|value| !value.is_empty());

        let liquidity_stats_pool_ids = parse_csv(
            env::var("OA_LIQUIDITY_STATS_POOL_IDS")
                .ok()
                .unwrap_or_else(|| DEFAULT_LIQUIDITY_STATS_POOL_IDS.to_string()),
        );

        let runtime_signing_key = env::var("OA_RUNTIME_SIGNING_KEY")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .or_else(|| runtime_signature_secret.clone());

        let runtime_signing_key_id = env::var("OA_RUNTIME_SIGNING_KEY_ID")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| DEFAULT_RUNTIME_SIGNING_KEY_ID.to_string());

        let runtime_comms_delivery_ingest_path = env::var("OA_RUNTIME_COMMS_DELIVERY_INGEST_PATH")
            .ok()
            .or_else(|| env::var("OA_RUNTIME_ELIXIR_COMMS_DELIVERY_INGEST_PATH").ok())
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| DEFAULT_RUNTIME_COMMS_DELIVERY_INGEST_PATH.to_string());

        let runtime_comms_delivery_timeout_ms = env::var("OA_RUNTIME_COMMS_DELIVERY_TIMEOUT_MS")
            .ok()
            .or_else(|| env::var("OA_RUNTIME_ELIXIR_COMMS_DELIVERY_TIMEOUT_MS").ok())
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(DEFAULT_RUNTIME_COMMS_DELIVERY_TIMEOUT_MS)
            .max(500);

        let runtime_comms_delivery_max_retries = env::var("OA_RUNTIME_COMMS_DELIVERY_MAX_RETRIES")
            .ok()
            .or_else(|| env::var("OA_RUNTIME_ELIXIR_COMMS_DELIVERY_MAX_RETRIES").ok())
            .and_then(|value| value.parse::<u32>().ok())
            .unwrap_or(DEFAULT_RUNTIME_COMMS_DELIVERY_MAX_RETRIES);

        let runtime_comms_delivery_retry_backoff_ms =
            env::var("OA_RUNTIME_COMMS_DELIVERY_RETRY_BACKOFF_MS")
                .ok()
                .or_else(|| env::var("OA_RUNTIME_ELIXIR_COMMS_DELIVERY_RETRY_BACKOFF_MS").ok())
                .and_then(|value| value.parse::<u64>().ok())
                .unwrap_or(DEFAULT_RUNTIME_COMMS_DELIVERY_RETRY_BACKOFF_MS);

        let smoke_stream_secret = env::var("OA_SMOKE_SECRET")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());

        let resend_webhook_secret = env::var("OA_RESEND_WEBHOOK_SECRET")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());

        let resend_webhook_tolerance_seconds = env::var("OA_RESEND_WEBHOOK_TOLERANCE_SECONDS")
            .ok()
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(DEFAULT_RESEND_WEBHOOK_TOLERANCE_SECONDS)
            .max(1);

        let google_oauth_client_id = env::var("GOOGLE_OAUTH_CLIENT_ID")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());

        let google_oauth_client_secret = env::var("GOOGLE_OAUTH_CLIENT_SECRET")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());

        let google_oauth_redirect_uri = env::var("GOOGLE_OAUTH_REDIRECT_URI")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());

        let google_oauth_scopes = env::var("GOOGLE_OAUTH_SCOPES")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| DEFAULT_GOOGLE_OAUTH_SCOPES.to_string());

        let google_oauth_token_url = env::var("GOOGLE_OAUTH_TOKEN_URL")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| DEFAULT_GOOGLE_OAUTH_TOKEN_URL.to_string());

        let google_gmail_api_base_url = env::var("GOOGLE_GMAIL_API_BASE_URL")
            .ok()
            .map(|value| value.trim().trim_end_matches('/').to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| DEFAULT_GOOGLE_GMAIL_API_BASE_URL.to_string());

        let runtime_driver = env::var("OA_RUNTIME_ROUTING_DRIVER")
            .ok()
            .or_else(|| env::var("OA_RUNTIME_DRIVER").ok())
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_RUNTIME_DRIVER.to_string())
            .trim()
            .to_lowercase();

        let runtime_force_driver = env::var("OA_RUNTIME_ROUTING_FORCE_DRIVER")
            .ok()
            .or_else(|| env::var("OA_RUNTIME_FORCE_DRIVER").ok())
            .map(|value| value.trim().to_lowercase())
            .filter(|value| !value.is_empty());

        let runtime_force_control_service = env::var("OA_RUNTIME_FORCE_CONTROL_SERVICE")
            .ok()
            .or_else(|| env::var("OA_RUNTIME_FORCE_LEGACY").ok())
            .map(|value| matches!(value.trim().to_lowercase().as_str(), "1" | "true" | "yes"))
            .unwrap_or(DEFAULT_RUNTIME_FORCE_CONTROL_SERVICE);

        let runtime_canary_user_percent = env::var("OA_RUNTIME_CANARY_USER_PERCENT")
            .ok()
            .and_then(|value| value.parse::<u8>().ok())
            .unwrap_or(DEFAULT_RUNTIME_CANARY_USER_PERCENT)
            .min(100);

        let runtime_canary_autopilot_percent = env::var("OA_RUNTIME_CANARY_AUTOPILOT_PERCENT")
            .ok()
            .and_then(|value| value.parse::<u8>().ok())
            .unwrap_or(DEFAULT_RUNTIME_CANARY_AUTOPILOT_PERCENT)
            .min(100);

        let runtime_canary_seed = env::var("OA_RUNTIME_CANARY_SEED")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_RUNTIME_CANARY_SEED.to_string());

        let runtime_overrides_enabled = env::var("OA_RUNTIME_OVERRIDES_ENABLED")
            .ok()
            .map(|value| matches!(value.trim().to_lowercase().as_str(), "1" | "true" | "yes"))
            .unwrap_or(DEFAULT_RUNTIME_OVERRIDES_ENABLED);

        let runtime_shadow_enabled = env::var("OA_RUNTIME_SHADOW_ENABLED")
            .ok()
            .map(|value| matches!(value.trim().to_lowercase().as_str(), "1" | "true" | "yes"))
            .unwrap_or(DEFAULT_RUNTIME_SHADOW_ENABLED);

        let runtime_shadow_sample_rate = env::var("OA_RUNTIME_SHADOW_SAMPLE_RATE")
            .ok()
            .and_then(|value| value.parse::<f64>().ok())
            .unwrap_or(DEFAULT_RUNTIME_SHADOW_SAMPLE_RATE);

        let runtime_shadow_max_capture_bytes = env::var("OA_RUNTIME_SHADOW_MAX_CAPTURE_BYTES")
            .ok()
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(DEFAULT_RUNTIME_SHADOW_MAX_CAPTURE_BYTES);

        let codex_thread_store_path = env::var("OA_CODEX_THREAD_STORE_PATH")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .map(PathBuf::from);

        let domain_store_path = env::var("OA_DOMAIN_STORE_PATH")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .map(PathBuf::from);

        let maintenance_mode_enabled = env::var("OA_MAINTENANCE_MODE_ENABLED")
            .ok()
            .map(|value| matches!(value.trim().to_lowercase().as_str(), "1" | "true" | "yes"))
            .unwrap_or(DEFAULT_MAINTENANCE_MODE_ENABLED);

        let maintenance_bypass_token = env::var("OA_MAINTENANCE_BYPASS_TOKEN")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());

        let maintenance_bypass_cookie_name = env::var("OA_MAINTENANCE_BYPASS_COOKIE_NAME")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| DEFAULT_MAINTENANCE_BYPASS_COOKIE_NAME.to_string());

        let maintenance_bypass_cookie_ttl_seconds =
            env::var("OA_MAINTENANCE_BYPASS_COOKIE_TTL_SECONDS")
                .ok()
                .and_then(|value| value.parse::<u64>().ok())
                .unwrap_or(DEFAULT_MAINTENANCE_BYPASS_COOKIE_TTL_SECONDS)
                .max(60);

        let maintenance_allowed_paths = parse_csv(
            env::var("OA_MAINTENANCE_ALLOWED_PATHS")
                .ok()
                .unwrap_or_else(|| DEFAULT_MAINTENANCE_ALLOWED_PATHS.to_string()),
        );

        let compat_control_enforced = env::var("OA_COMPAT_CONTROL_ENFORCED")
            .ok()
            .map(|value| matches!(value.trim().to_lowercase().as_str(), "1" | "true" | "yes"))
            .unwrap_or(DEFAULT_COMPAT_CONTROL_ENFORCED);

        let compat_control_protocol_version = env::var("OA_COMPAT_CONTROL_PROTOCOL_VERSION")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_COMPAT_CONTROL_PROTOCOL_VERSION.to_string());

        let compat_control_min_client_build_id = env::var("OA_COMPAT_CONTROL_MIN_CLIENT_BUILD_ID")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_COMPAT_CONTROL_MIN_CLIENT_BUILD_ID.to_string());

        let compat_control_max_client_build_id = env::var("OA_COMPAT_CONTROL_MAX_CLIENT_BUILD_ID")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());

        let compat_control_min_schema_version = env::var("OA_COMPAT_CONTROL_MIN_SCHEMA_VERSION")
            .ok()
            .and_then(|value| value.parse::<u32>().ok())
            .unwrap_or(DEFAULT_COMPAT_CONTROL_MIN_SCHEMA_VERSION);

        let compat_control_max_schema_version = env::var("OA_COMPAT_CONTROL_MAX_SCHEMA_VERSION")
            .ok()
            .and_then(|value| value.parse::<u32>().ok())
            .unwrap_or(DEFAULT_COMPAT_CONTROL_MAX_SCHEMA_VERSION)
            .max(compat_control_min_schema_version);

        Ok(Self {
            bind_addr,
            log_filter,
            static_dir,
            desktop_download_url,
            auth_provider_mode,
            workos_client_id,
            workos_api_key,
            workos_api_base_url,
            mock_magic_code,
            auth_local_test_login_enabled,
            auth_local_test_login_allowed_emails,
            auth_local_test_login_signing_key,
            auth_api_signup_enabled,
            auth_api_signup_allowed_domains,
            auth_api_signup_default_token_name,
            admin_emails,
            auth_store_path,
            auth_challenge_ttl_seconds,
            auth_access_ttl_seconds,
            auth_refresh_ttl_seconds,
            sync_token_enabled,
            sync_token_signing_key,
            sync_token_issuer,
            sync_token_audience,
            sync_token_key_id,
            sync_token_claims_version,
            sync_token_ttl_seconds,
            sync_token_min_ttl_seconds,
            sync_token_max_ttl_seconds,
            sync_token_allowed_scopes,
            sync_token_default_scopes,
            route_split_enabled,
            route_split_mode,
            route_split_rust_routes,
            route_split_cohort_percentage,
            route_split_salt,
            route_split_force_legacy,
            route_split_legacy_base_url,
            runtime_sync_revoke_base_url,
            runtime_sync_revoke_path,
            runtime_signature_secret,
            runtime_signature_ttl_seconds,
            runtime_internal_shared_secret,
            runtime_internal_key_id,
            runtime_internal_signature_ttl_seconds,
            runtime_internal_secret_fetch_path,
            runtime_internal_secret_cache_ttl_ms,
            runtime_base_url,
            liquidity_stats_pool_ids,
            runtime_signing_key,
            runtime_signing_key_id,
            runtime_comms_delivery_ingest_path,
            runtime_comms_delivery_timeout_ms,
            runtime_comms_delivery_max_retries,
            runtime_comms_delivery_retry_backoff_ms,
            smoke_stream_secret,
            resend_webhook_secret,
            resend_webhook_tolerance_seconds,
            google_oauth_client_id,
            google_oauth_client_secret,
            google_oauth_redirect_uri,
            google_oauth_scopes,
            google_oauth_token_url,
            google_gmail_api_base_url,
            runtime_driver,
            runtime_force_driver,
            runtime_force_control_service,
            runtime_canary_user_percent,
            runtime_canary_autopilot_percent,
            runtime_canary_seed,
            runtime_overrides_enabled,
            runtime_shadow_enabled,
            runtime_shadow_sample_rate,
            runtime_shadow_max_capture_bytes,
            codex_thread_store_path,
            domain_store_path,
            maintenance_mode_enabled,
            maintenance_bypass_token,
            maintenance_bypass_cookie_name,
            maintenance_bypass_cookie_ttl_seconds,
            maintenance_allowed_paths,
            compat_control_enforced,
            compat_control_protocol_version,
            compat_control_min_client_build_id,
            compat_control_max_client_build_id,
            compat_control_min_schema_version,
            compat_control_max_schema_version,
        })
    }
}

#[cfg(test)]
impl Config {
    #[must_use]
    pub fn for_tests(static_dir: PathBuf) -> Self {
        Self {
            bind_addr: SocketAddr::from(([127, 0, 0, 1], 0)),
            log_filter: "debug".to_string(),
            static_dir,
            desktop_download_url: DEFAULT_DESKTOP_DOWNLOAD_URL.to_string(),
            auth_provider_mode: "mock".to_string(),
            workos_client_id: None,
            workos_api_key: None,
            workos_api_base_url: DEFAULT_WORKOS_API_BASE_URL.to_string(),
            mock_magic_code: DEFAULT_MOCK_MAGIC_CODE.to_string(),
            auth_local_test_login_enabled: false,
            auth_local_test_login_allowed_emails: Vec::new(),
            auth_local_test_login_signing_key: None,
            auth_api_signup_enabled: false,
            auth_api_signup_allowed_domains: Vec::new(),
            auth_api_signup_default_token_name: DEFAULT_AUTH_API_SIGNUP_DEFAULT_TOKEN_NAME
                .to_string(),
            admin_emails: vec![
                "chris@openagents.com".to_string(),
                "routes@openagents.com".to_string(),
            ],
            auth_store_path: None,
            auth_challenge_ttl_seconds: DEFAULT_AUTH_CHALLENGE_TTL_SECONDS,
            auth_access_ttl_seconds: DEFAULT_AUTH_ACCESS_TTL_SECONDS,
            auth_refresh_ttl_seconds: 86_400,
            sync_token_enabled: true,
            sync_token_signing_key: Some("sync-test-signing-key".to_string()),
            sync_token_issuer: "https://openagents.test".to_string(),
            sync_token_audience: "openagents-sync-test".to_string(),
            sync_token_key_id: "sync-auth-test-v1".to_string(),
            sync_token_claims_version: DEFAULT_SYNC_TOKEN_CLAIMS_VERSION.to_string(),
            sync_token_ttl_seconds: DEFAULT_SYNC_TOKEN_TTL_SECONDS,
            sync_token_min_ttl_seconds: DEFAULT_SYNC_TOKEN_MIN_TTL_SECONDS,
            sync_token_max_ttl_seconds: DEFAULT_SYNC_TOKEN_MAX_TTL_SECONDS,
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
            runtime_sync_revoke_path: DEFAULT_RUNTIME_SYNC_REVOKE_PATH.to_string(),
            runtime_signature_secret: None,
            runtime_signature_ttl_seconds: DEFAULT_RUNTIME_SIGNATURE_TTL_SECONDS,
            runtime_internal_shared_secret: None,
            runtime_internal_key_id: DEFAULT_RUNTIME_INTERNAL_KEY_ID.to_string(),
            runtime_internal_signature_ttl_seconds: DEFAULT_RUNTIME_INTERNAL_SIGNATURE_TTL_SECONDS,
            runtime_internal_secret_fetch_path: DEFAULT_RUNTIME_INTERNAL_SECRET_FETCH_PATH
                .to_string(),
            runtime_internal_secret_cache_ttl_ms: DEFAULT_RUNTIME_INTERNAL_SECRET_CACHE_TTL_MS,
            runtime_base_url: None,
            liquidity_stats_pool_ids: vec!["llp-main".to_string()],
            runtime_signing_key: None,
            runtime_signing_key_id: DEFAULT_RUNTIME_SIGNING_KEY_ID.to_string(),
            runtime_comms_delivery_ingest_path: DEFAULT_RUNTIME_COMMS_DELIVERY_INGEST_PATH
                .to_string(),
            runtime_comms_delivery_timeout_ms: DEFAULT_RUNTIME_COMMS_DELIVERY_TIMEOUT_MS,
            runtime_comms_delivery_max_retries: DEFAULT_RUNTIME_COMMS_DELIVERY_MAX_RETRIES,
            runtime_comms_delivery_retry_backoff_ms:
                DEFAULT_RUNTIME_COMMS_DELIVERY_RETRY_BACKOFF_MS,
            smoke_stream_secret: Some("secret".to_string()),
            resend_webhook_secret: None,
            resend_webhook_tolerance_seconds: DEFAULT_RESEND_WEBHOOK_TOLERANCE_SECONDS,
            google_oauth_client_id: None,
            google_oauth_client_secret: None,
            google_oauth_redirect_uri: None,
            google_oauth_scopes: "https://www.googleapis.com/auth/gmail.readonly".to_string(),
            google_oauth_token_url: DEFAULT_GOOGLE_OAUTH_TOKEN_URL.to_string(),
            google_gmail_api_base_url: DEFAULT_GOOGLE_GMAIL_API_BASE_URL.to_string(),
            runtime_driver: "control_service".to_string(),
            runtime_force_driver: None,
            runtime_force_control_service: false,
            runtime_canary_user_percent: 0,
            runtime_canary_autopilot_percent: 0,
            runtime_canary_seed: DEFAULT_RUNTIME_CANARY_SEED.to_string(),
            runtime_overrides_enabled: true,
            runtime_shadow_enabled: false,
            runtime_shadow_sample_rate: 1.0,
            runtime_shadow_max_capture_bytes: DEFAULT_RUNTIME_SHADOW_MAX_CAPTURE_BYTES,
            codex_thread_store_path: None,
            domain_store_path: None,
            maintenance_mode_enabled: false,
            maintenance_bypass_token: None,
            maintenance_bypass_cookie_name: DEFAULT_MAINTENANCE_BYPASS_COOKIE_NAME.to_string(),
            maintenance_bypass_cookie_ttl_seconds: DEFAULT_MAINTENANCE_BYPASS_COOKIE_TTL_SECONDS,
            maintenance_allowed_paths: vec!["/healthz".to_string(), "/readyz".to_string()],
            compat_control_enforced: false,
            compat_control_protocol_version: DEFAULT_COMPAT_CONTROL_PROTOCOL_VERSION.to_string(),
            compat_control_min_client_build_id: DEFAULT_COMPAT_CONTROL_MIN_CLIENT_BUILD_ID
                .to_string(),
            compat_control_max_client_build_id: None,
            compat_control_min_schema_version: DEFAULT_COMPAT_CONTROL_MIN_SCHEMA_VERSION,
            compat_control_max_schema_version: DEFAULT_COMPAT_CONTROL_MAX_SCHEMA_VERSION,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Config;
    use std::path::PathBuf;

    #[test]
    fn test_fixture_covers_all_config_fields() {
        let config = Config::for_tests(PathBuf::from("."));
        assert_eq!(config.bind_addr.port(), 0);
        assert!(!config.route_split_rust_routes.is_empty());
        assert!(config.sync_token_signing_key.is_some());
    }
}

fn parse_csv(value: String) -> Vec<String> {
    value
        .split(',')
        .map(|segment| segment.trim().to_string())
        .filter(|segment| !segment.is_empty())
        .collect()
}
