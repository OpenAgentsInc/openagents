use std::env;
use std::net::SocketAddr;
use std::path::PathBuf;

use thiserror::Error;

const DEFAULT_BIND_ADDR: &str = "127.0.0.1:8787";
const DEFAULT_LOG_FILTER: &str = "info";
const DEFAULT_STATIC_DIR: &str = "../web-shell/dist";
const DEFAULT_AUTH_PROVIDER_MODE: &str = "auto";
const DEFAULT_WORKOS_API_BASE_URL: &str = "https://api.workos.com";
const DEFAULT_MOCK_MAGIC_CODE: &str = "123456";
const DEFAULT_AUTH_CHALLENGE_TTL_SECONDS: u64 = 600;
const DEFAULT_AUTH_ACCESS_TTL_SECONDS: u64 = 3600;
const DEFAULT_AUTH_REFRESH_TTL_SECONDS: u64 = 2_592_000;
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

#[derive(Debug, Clone)]
pub struct Config {
    pub bind_addr: SocketAddr,
    pub log_filter: String,
    pub static_dir: PathBuf,
    pub auth_provider_mode: String,
    pub workos_client_id: Option<String>,
    pub workos_api_key: Option<String>,
    pub workos_api_base_url: String,
    pub mock_magic_code: String,
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

        Ok(Self {
            bind_addr,
            log_filter,
            static_dir,
            auth_provider_mode,
            workos_client_id,
            workos_api_key,
            workos_api_base_url,
            mock_magic_code,
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
        })
    }
}

fn parse_csv(value: String) -> Vec<String> {
    value
        .split(',')
        .map(|segment| segment.trim().to_string())
        .filter(|segment| !segment.is_empty())
        .collect()
}
