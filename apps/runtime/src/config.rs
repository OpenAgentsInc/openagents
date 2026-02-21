use std::{
    collections::HashSet,
    env,
    net::{AddrParseError, SocketAddr},
};

use thiserror::Error;

use crate::fanout::{FanoutLimitConfig, FanoutTierLimits, QosTier};

#[derive(Clone, Debug)]
pub struct Config {
    pub service_name: String,
    pub bind_addr: SocketAddr,
    pub build_sha: String,
    pub authority_write_mode: AuthorityWriteMode,
    pub fanout_driver: String,
    pub fanout_queue_capacity: usize,
    pub khala_poll_default_limit: usize,
    pub khala_poll_max_limit: usize,
    pub khala_poll_min_interval_ms: u64,
    pub khala_slow_consumer_lag_threshold: u64,
    pub khala_slow_consumer_max_strikes: u32,
    pub khala_consumer_registry_capacity: usize,
    pub khala_reconnect_base_backoff_ms: u64,
    pub khala_reconnect_jitter_ms: u64,
    pub khala_enforce_origin: bool,
    pub khala_allowed_origins: HashSet<String>,
    pub khala_run_events_publish_rate_per_second: u32,
    pub khala_worker_lifecycle_publish_rate_per_second: u32,
    pub khala_codex_worker_events_publish_rate_per_second: u32,
    pub khala_fallback_publish_rate_per_second: u32,
    pub khala_run_events_replay_budget_events: u64,
    pub khala_worker_lifecycle_replay_budget_events: u64,
    pub khala_codex_worker_events_replay_budget_events: u64,
    pub khala_fallback_replay_budget_events: u64,
    pub khala_run_events_max_payload_bytes: usize,
    pub khala_worker_lifecycle_max_payload_bytes: usize,
    pub khala_codex_worker_events_max_payload_bytes: usize,
    pub khala_fallback_max_payload_bytes: usize,
    pub sync_token_signing_key: String,
    pub sync_token_issuer: String,
    pub sync_token_audience: String,
    pub sync_token_require_jti: bool,
    pub sync_token_max_age_seconds: u64,
    pub sync_revoked_jtis: HashSet<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AuthorityWriteMode {
    RustActive,
    ShadowOnly,
    ReadOnly,
}

impl AuthorityWriteMode {
    #[must_use]
    pub fn writes_enabled(&self) -> bool {
        matches!(self, Self::RustActive)
    }

    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::RustActive => "rust_active",
            Self::ShadowOnly => "shadow_only",
            Self::ReadOnly => "read_only",
        }
    }
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("invalid RUNTIME_BIND_ADDR: {0}")]
    BindAddrParse(#[from] AddrParseError),
    #[error("invalid RUNTIME_AUTHORITY_WRITE_MODE: {0}")]
    InvalidAuthorityWriteMode(String),
    #[error("invalid RUNTIME_FANOUT_QUEUE_CAPACITY: {0}")]
    InvalidFanoutQueueCapacity(String),
    #[error("invalid RUNTIME_KHALA_POLL_DEFAULT_LIMIT: {0}")]
    InvalidKhalaPollDefaultLimit(String),
    #[error("invalid RUNTIME_KHALA_POLL_MAX_LIMIT: {0}")]
    InvalidKhalaPollMaxLimit(String),
    #[error("invalid RUNTIME_KHALA_POLL_MIN_INTERVAL_MS: {0}")]
    InvalidKhalaPollMinIntervalMs(String),
    #[error("invalid RUNTIME_KHALA_SLOW_CONSUMER_LAG_THRESHOLD: {0}")]
    InvalidKhalaSlowConsumerLagThreshold(String),
    #[error("invalid RUNTIME_KHALA_SLOW_CONSUMER_MAX_STRIKES: {0}")]
    InvalidKhalaSlowConsumerMaxStrikes(String),
    #[error("invalid RUNTIME_KHALA_CONSUMER_REGISTRY_CAPACITY: {0}")]
    InvalidKhalaConsumerRegistryCapacity(String),
    #[error("invalid RUNTIME_KHALA_RECONNECT_BASE_BACKOFF_MS: {0}")]
    InvalidKhalaReconnectBaseBackoffMs(String),
    #[error("invalid RUNTIME_KHALA_RECONNECT_JITTER_MS: {0}")]
    InvalidKhalaReconnectJitterMs(String),
    #[error("invalid RUNTIME_KHALA_ENFORCE_ORIGIN: {0}")]
    InvalidKhalaEnforceOrigin(String),
    #[error("invalid khala publish rate limit setting: {0}")]
    InvalidKhalaPublishRateLimit(String),
    #[error("invalid khala replay budget setting: {0}")]
    InvalidKhalaReplayBudget(String),
    #[error("invalid khala max payload bytes setting: {0}")]
    InvalidKhalaMaxPayloadBytes(String),
    #[error("invalid RUNTIME_SYNC_TOKEN_REQUIRE_JTI: {0}")]
    InvalidSyncTokenRequireJti(String),
    #[error("invalid RUNTIME_SYNC_TOKEN_MAX_AGE_SECONDS: {0}")]
    InvalidSyncTokenMaxAgeSeconds(String),
}

impl Config {
    pub fn from_env() -> Result<Self, ConfigError> {
        let bind_addr = env::var("RUNTIME_BIND_ADDR")
            .unwrap_or_else(|_| "127.0.0.1:4100".to_string())
            .parse()?;
        let service_name =
            env::var("RUNTIME_SERVICE_NAME").unwrap_or_else(|_| "runtime".to_string());
        let build_sha = env::var("RUNTIME_BUILD_SHA").unwrap_or_else(|_| "dev".to_string());
        let authority_write_mode = parse_authority_write_mode(
            env::var("RUNTIME_AUTHORITY_WRITE_MODE")
                .unwrap_or_else(|_| "rust_active".to_string())
                .as_str(),
        )?;
        let fanout_driver =
            env::var("RUNTIME_FANOUT_DRIVER").unwrap_or_else(|_| "memory".to_string());
        let fanout_queue_capacity = env::var("RUNTIME_FANOUT_QUEUE_CAPACITY")
            .unwrap_or_else(|_| "1024".to_string())
            .parse::<usize>()
            .map_err(|error| ConfigError::InvalidFanoutQueueCapacity(error.to_string()))?;
        let khala_poll_default_limit = env::var("RUNTIME_KHALA_POLL_DEFAULT_LIMIT")
            .unwrap_or_else(|_| "100".to_string())
            .parse::<usize>()
            .map_err(|error| ConfigError::InvalidKhalaPollDefaultLimit(error.to_string()))?;
        let khala_poll_max_limit = env::var("RUNTIME_KHALA_POLL_MAX_LIMIT")
            .unwrap_or_else(|_| "200".to_string())
            .parse::<usize>()
            .map_err(|error| ConfigError::InvalidKhalaPollMaxLimit(error.to_string()))?;
        let khala_poll_min_interval_ms = env::var("RUNTIME_KHALA_POLL_MIN_INTERVAL_MS")
            .unwrap_or_else(|_| "250".to_string())
            .parse::<u64>()
            .map_err(|error| ConfigError::InvalidKhalaPollMinIntervalMs(error.to_string()))?;
        let khala_slow_consumer_lag_threshold =
            env::var("RUNTIME_KHALA_SLOW_CONSUMER_LAG_THRESHOLD")
                .unwrap_or_else(|_| "300".to_string())
                .parse::<u64>()
                .map_err(|error| {
                    ConfigError::InvalidKhalaSlowConsumerLagThreshold(error.to_string())
                })?;
        let khala_slow_consumer_max_strikes = env::var("RUNTIME_KHALA_SLOW_CONSUMER_MAX_STRIKES")
            .unwrap_or_else(|_| "3".to_string())
            .parse::<u32>()
            .map_err(|error| ConfigError::InvalidKhalaSlowConsumerMaxStrikes(error.to_string()))?;
        let khala_consumer_registry_capacity = env::var("RUNTIME_KHALA_CONSUMER_REGISTRY_CAPACITY")
            .unwrap_or_else(|_| "4096".to_string())
            .parse::<usize>()
            .map_err(|error| {
                ConfigError::InvalidKhalaConsumerRegistryCapacity(error.to_string())
            })?;
        let khala_reconnect_base_backoff_ms = env::var("RUNTIME_KHALA_RECONNECT_BASE_BACKOFF_MS")
            .unwrap_or_else(|_| "400".to_string())
            .parse::<u64>()
            .map_err(|error| ConfigError::InvalidKhalaReconnectBaseBackoffMs(error.to_string()))?;
        let khala_reconnect_jitter_ms = env::var("RUNTIME_KHALA_RECONNECT_JITTER_MS")
            .unwrap_or_else(|_| "250".to_string())
            .parse::<u64>()
            .map_err(|error| ConfigError::InvalidKhalaReconnectJitterMs(error.to_string()))?;
        let khala_enforce_origin =
            parse_bool_env("RUNTIME_KHALA_ENFORCE_ORIGIN", true).map_err(|error| {
                ConfigError::InvalidKhalaEnforceOrigin(format!(
                    "RUNTIME_KHALA_ENFORCE_ORIGIN: {error}"
                ))
            })?;
        let khala_allowed_origins = env::var("RUNTIME_KHALA_ALLOWED_ORIGINS")
            .unwrap_or_else(|_| "https://openagents.com,https://www.openagents.com".to_string())
            .split(',')
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(normalize_origin_value)
            .collect::<HashSet<_>>();
        let parse_publish_rate = |key: &str, default: &str| -> Result<u32, ConfigError> {
            env::var(key)
                .unwrap_or_else(|_| default.to_string())
                .parse::<u32>()
                .map(|value| value.max(1))
                .map_err(|error| {
                    ConfigError::InvalidKhalaPublishRateLimit(format!("{key}: {error}"))
                })
        };
        let parse_replay_budget = |key: &str, default: &str| -> Result<u64, ConfigError> {
            env::var(key)
                .unwrap_or_else(|_| default.to_string())
                .parse::<u64>()
                .map(|value| value.max(1))
                .map_err(|error| ConfigError::InvalidKhalaReplayBudget(format!("{key}: {error}")))
        };
        let parse_max_payload = |key: &str, default: &str| -> Result<usize, ConfigError> {
            env::var(key)
                .unwrap_or_else(|_| default.to_string())
                .parse::<usize>()
                .map(|value| value.max(1))
                .map_err(|error| {
                    ConfigError::InvalidKhalaMaxPayloadBytes(format!("{key}: {error}"))
                })
        };
        let khala_run_events_publish_rate_per_second =
            parse_publish_rate("RUNTIME_KHALA_RUN_EVENTS_PUBLISH_RATE_PER_SECOND", "240")?;
        let khala_worker_lifecycle_publish_rate_per_second = parse_publish_rate(
            "RUNTIME_KHALA_WORKER_LIFECYCLE_PUBLISH_RATE_PER_SECOND",
            "180",
        )?;
        let khala_codex_worker_events_publish_rate_per_second = parse_publish_rate(
            "RUNTIME_KHALA_CODEX_WORKER_EVENTS_PUBLISH_RATE_PER_SECOND",
            "240",
        )?;
        let khala_fallback_publish_rate_per_second =
            parse_publish_rate("RUNTIME_KHALA_FALLBACK_PUBLISH_RATE_PER_SECOND", "90")?;
        let khala_run_events_replay_budget_events =
            parse_replay_budget("RUNTIME_KHALA_RUN_EVENTS_REPLAY_BUDGET_EVENTS", "20000")?;
        let khala_worker_lifecycle_replay_budget_events = parse_replay_budget(
            "RUNTIME_KHALA_WORKER_LIFECYCLE_REPLAY_BUDGET_EVENTS",
            "10000",
        )?;
        let khala_codex_worker_events_replay_budget_events = parse_replay_budget(
            "RUNTIME_KHALA_CODEX_WORKER_EVENTS_REPLAY_BUDGET_EVENTS",
            "3000",
        )?;
        let khala_fallback_replay_budget_events =
            parse_replay_budget("RUNTIME_KHALA_FALLBACK_REPLAY_BUDGET_EVENTS", "500")?;
        let khala_run_events_max_payload_bytes =
            parse_max_payload("RUNTIME_KHALA_RUN_EVENTS_MAX_PAYLOAD_BYTES", "262144")?;
        let khala_worker_lifecycle_max_payload_bytes =
            parse_max_payload("RUNTIME_KHALA_WORKER_LIFECYCLE_MAX_PAYLOAD_BYTES", "65536")?;
        let khala_codex_worker_events_max_payload_bytes = parse_max_payload(
            "RUNTIME_KHALA_CODEX_WORKER_EVENTS_MAX_PAYLOAD_BYTES",
            "131072",
        )?;
        let khala_fallback_max_payload_bytes =
            parse_max_payload("RUNTIME_KHALA_FALLBACK_MAX_PAYLOAD_BYTES", "65536")?;
        let khala_poll_max_limit = khala_poll_max_limit.max(1);
        let khala_poll_default_limit = khala_poll_default_limit.max(1).min(khala_poll_max_limit);
        let khala_slow_consumer_max_strikes = khala_slow_consumer_max_strikes.max(1);
        let khala_consumer_registry_capacity = khala_consumer_registry_capacity.max(1);
        let sync_token_signing_key = env::var("RUNTIME_SYNC_TOKEN_SIGNING_KEY")
            .unwrap_or_else(|_| "dev-sync-key".to_string());
        let sync_token_issuer = env::var("RUNTIME_SYNC_TOKEN_ISSUER")
            .unwrap_or_else(|_| "https://openagents.com".to_string());
        let sync_token_audience = env::var("RUNTIME_SYNC_TOKEN_AUDIENCE")
            .unwrap_or_else(|_| "openagents-sync".to_string());
        let sync_token_require_jti = parse_bool_env("RUNTIME_SYNC_TOKEN_REQUIRE_JTI", true)
            .map_err(|error| {
                ConfigError::InvalidSyncTokenRequireJti(format!(
                    "RUNTIME_SYNC_TOKEN_REQUIRE_JTI: {error}"
                ))
            })?;
        let sync_token_max_age_seconds = env::var("RUNTIME_SYNC_TOKEN_MAX_AGE_SECONDS")
            .unwrap_or_else(|_| "300".to_string())
            .parse::<u64>()
            .map_err(|error| ConfigError::InvalidSyncTokenMaxAgeSeconds(error.to_string()))?
            .max(1);
        let sync_revoked_jtis = env::var("RUNTIME_SYNC_REVOKED_JTIS")
            .ok()
            .map(|raw| {
                raw.split(',')
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(ToString::to_string)
                    .collect::<HashSet<_>>()
            })
            .unwrap_or_default();
        Ok(Self {
            service_name,
            bind_addr,
            build_sha,
            authority_write_mode,
            fanout_driver,
            fanout_queue_capacity,
            khala_poll_default_limit,
            khala_poll_max_limit,
            khala_poll_min_interval_ms,
            khala_slow_consumer_lag_threshold,
            khala_slow_consumer_max_strikes,
            khala_consumer_registry_capacity,
            khala_reconnect_base_backoff_ms,
            khala_reconnect_jitter_ms,
            khala_enforce_origin,
            khala_allowed_origins,
            khala_run_events_publish_rate_per_second,
            khala_worker_lifecycle_publish_rate_per_second,
            khala_codex_worker_events_publish_rate_per_second,
            khala_fallback_publish_rate_per_second,
            khala_run_events_replay_budget_events,
            khala_worker_lifecycle_replay_budget_events,
            khala_codex_worker_events_replay_budget_events,
            khala_fallback_replay_budget_events,
            khala_run_events_max_payload_bytes,
            khala_worker_lifecycle_max_payload_bytes,
            khala_codex_worker_events_max_payload_bytes,
            khala_fallback_max_payload_bytes,
            sync_token_signing_key,
            sync_token_issuer,
            sync_token_audience,
            sync_token_require_jti,
            sync_token_max_age_seconds,
            sync_revoked_jtis,
        })
    }

    #[must_use]
    pub fn khala_fanout_limits(&self) -> FanoutLimitConfig {
        FanoutLimitConfig {
            run_events: FanoutTierLimits {
                qos_tier: QosTier::Warm,
                replay_budget_events: self.khala_run_events_replay_budget_events,
                max_publish_per_second: self.khala_run_events_publish_rate_per_second,
                max_payload_bytes: self.khala_run_events_max_payload_bytes,
            },
            worker_lifecycle: FanoutTierLimits {
                qos_tier: QosTier::Warm,
                replay_budget_events: self.khala_worker_lifecycle_replay_budget_events,
                max_publish_per_second: self.khala_worker_lifecycle_publish_rate_per_second,
                max_payload_bytes: self.khala_worker_lifecycle_max_payload_bytes,
            },
            codex_worker_events: FanoutTierLimits {
                qos_tier: QosTier::Hot,
                replay_budget_events: self.khala_codex_worker_events_replay_budget_events,
                max_publish_per_second: self.khala_codex_worker_events_publish_rate_per_second,
                max_payload_bytes: self.khala_codex_worker_events_max_payload_bytes,
            },
            fallback: FanoutTierLimits {
                qos_tier: QosTier::Cold,
                replay_budget_events: self.khala_fallback_replay_budget_events,
                max_publish_per_second: self.khala_fallback_publish_rate_per_second,
                max_payload_bytes: self.khala_fallback_max_payload_bytes,
            },
        }
    }
}

fn parse_bool_env(key: &str, default: bool) -> Result<bool, String> {
    let raw = env::var(key).unwrap_or_else(|_| {
        if default {
            "true".to_string()
        } else {
            "false".to_string()
        }
    });
    match raw.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Ok(true),
        "0" | "false" | "no" | "off" => Ok(false),
        other => Err(other.to_string()),
    }
}

fn normalize_origin_value(value: &str) -> String {
    value.trim().trim_end_matches('/').to_ascii_lowercase()
}

fn parse_authority_write_mode(raw: &str) -> Result<AuthorityWriteMode, ConfigError> {
    match raw.trim().to_lowercase().as_str() {
        "rust_active" => Ok(AuthorityWriteMode::RustActive),
        "shadow_only" => Ok(AuthorityWriteMode::ShadowOnly),
        "read_only" => Ok(AuthorityWriteMode::ReadOnly),
        other => Err(ConfigError::InvalidAuthorityWriteMode(other.to_string())),
    }
}
