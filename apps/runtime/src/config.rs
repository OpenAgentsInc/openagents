use std::{
    collections::HashSet,
    env,
    net::{AddrParseError, SocketAddr},
};

use thiserror::Error;

use crate::credit::service::CreditPolicyConfig;
use crate::fanout::{FanoutLimitConfig, FanoutTierLimits, QosTier};
use nostr::nsec_to_private_key;

#[derive(Clone, Debug)]
pub struct LiquidityPoolWithdrawThrottleConfig {
    pub lp_mode_enabled: bool,
    pub stress_liability_ratio_bps: u32,
    pub halt_liability_ratio_bps: u32,
    pub stress_connected_ratio_bps: u32,
    pub halt_connected_ratio_bps: u32,
    pub stress_outbound_coverage_bps: u32,
    pub halt_outbound_coverage_bps: u32,
    pub stress_extra_delay_hours: i64,
    pub halt_extra_delay_hours: i64,
    pub stress_execution_cap_per_tick: u32,
}

impl Default for LiquidityPoolWithdrawThrottleConfig {
    fn default() -> Self {
        Self {
            lp_mode_enabled: false,
            stress_liability_ratio_bps: 2_500,
            halt_liability_ratio_bps: 5_000,
            stress_connected_ratio_bps: 7_500,
            halt_connected_ratio_bps: 4_000,
            stress_outbound_coverage_bps: 10_000,
            halt_outbound_coverage_bps: 5_000,
            stress_extra_delay_hours: 24,
            halt_extra_delay_hours: 72,
            stress_execution_cap_per_tick: 5,
        }
    }
}

#[derive(Clone, Debug)]
pub struct HydraFxPolicyConfig {
    pub allowed_pairs: HashSet<String>,
    pub max_spread_bps: u32,
    pub max_fee_bps: u32,
    pub min_quote_ttl_seconds: u32,
    pub max_quote_ttl_seconds: u32,
}

impl Default for HydraFxPolicyConfig {
    fn default() -> Self {
        Self {
            allowed_pairs: HashSet::from([
                "USD->BTC_LN".to_string(),
                "USDT->BTC_LN".to_string(),
                "BTC_LN->USD".to_string(),
            ]),
            max_spread_bps: 300,
            max_fee_bps: 150,
            min_quote_ttl_seconds: 5,
            max_quote_ttl_seconds: 300,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Config {
    pub service_name: String,
    pub bind_addr: SocketAddr,
    pub build_sha: String,
    pub db_url: Option<String>,
    pub authority_write_mode: AuthorityWriteMode,
    pub fanout_driver: String,
    pub fanout_queue_capacity: usize,
    pub khala_poll_default_limit: usize,
    pub khala_poll_max_limit: usize,
    pub khala_outbound_queue_limit: usize,
    pub khala_fair_topic_slice_limit: usize,
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
    pub verifier_strict: bool,
    pub verifier_allowed_signer_pubkeys: HashSet<String>,
    pub bridge_nostr_relays: Vec<String>,
    pub bridge_nostr_secret_key: Option<[u8; 32]>,
    pub liquidity_wallet_executor_base_url: Option<String>,
    pub liquidity_wallet_executor_auth_token: Option<String>,
    pub liquidity_wallet_executor_timeout_ms: u64,
    pub liquidity_quote_ttl_seconds: u64,
    pub liquidity_pool_withdraw_delay_hours: i64,
    pub liquidity_pool_withdraw_throttle: LiquidityPoolWithdrawThrottleConfig,
    pub liquidity_pool_snapshot_worker_enabled: bool,
    pub liquidity_pool_snapshot_pool_ids: Vec<String>,
    pub liquidity_pool_snapshot_interval_seconds: u64,
    pub liquidity_pool_snapshot_jitter_seconds: u64,
    pub liquidity_pool_snapshot_retention_count: i64,
    pub hydra_fx_policy: HydraFxPolicyConfig,
    pub credit_policy: CreditPolicyConfig,
    pub treasury_reconciliation_enabled: bool,
    pub treasury_reservation_ttl_seconds: u64,
    pub treasury_reconciliation_interval_seconds: u64,
    pub treasury_reconciliation_max_jobs: usize,
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
    #[error("invalid RUNTIME_KHALA_OUTBOUND_QUEUE_LIMIT: {0}")]
    InvalidKhalaOutboundQueueLimit(String),
    #[error("invalid RUNTIME_KHALA_FAIR_TOPIC_SLICE_LIMIT: {0}")]
    InvalidKhalaFairTopicSliceLimit(String),
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
    #[error("invalid RUNTIME_VERIFIER_STRICT: {0}")]
    InvalidVerifierStrict(String),
    #[error("invalid RUNTIME_VERIFIER_ALLOWED_SIGNER_PUBKEYS: {0}")]
    InvalidVerifierAllowedSignerPubkeys(String),
    #[error("invalid RUNTIME_BRIDGE_NOSTR_RELAYS: {0}")]
    InvalidBridgeNostrRelays(String),
    #[error("invalid RUNTIME_BRIDGE_NOSTR_SECRET_KEY: {0}")]
    InvalidBridgeNostrSecretKey(String),
    #[error("invalid RUNTIME_TREASURY_RECONCILIATION_ENABLED: {0}")]
    InvalidTreasuryReconciliationEnabled(String),
    #[error("invalid RUNTIME_TREASURY_RESERVATION_TTL_SECONDS: {0}")]
    InvalidTreasuryReservationTtlSeconds(String),
    #[error("invalid RUNTIME_TREASURY_RECONCILIATION_INTERVAL_SECONDS: {0}")]
    InvalidTreasuryReconciliationIntervalSeconds(String),
    #[error("invalid RUNTIME_TREASURY_RECONCILIATION_MAX_JOBS: {0}")]
    InvalidTreasuryReconciliationMaxJobs(String),
    #[error("invalid RUNTIME_LIQUIDITY_WALLET_EXECUTOR_TIMEOUT_MS: {0}")]
    InvalidLiquidityWalletExecutorTimeoutMs(String),
    #[error("invalid RUNTIME_LIQUIDITY_QUOTE_TTL_SECONDS: {0}")]
    InvalidLiquidityQuoteTtlSeconds(String),
    #[error("invalid RUNTIME_LIQUIDITY_POOL_WITHDRAW_DELAY_HOURS: {0}")]
    InvalidLiquidityPoolWithdrawDelayHours(String),
    #[error("invalid liquidity pool withdrawal throttle config: {0}")]
    InvalidLiquidityPoolWithdrawThrottleConfig(String),
    #[error("invalid RUNTIME_LIQUIDITY_POOL_SNAPSHOT_WORKER_ENABLED: {0}")]
    InvalidLiquidityPoolSnapshotWorkerEnabled(String),
    #[error("invalid RUNTIME_LIQUIDITY_POOL_SNAPSHOT_INTERVAL_SECONDS: {0}")]
    InvalidLiquidityPoolSnapshotIntervalSeconds(String),
    #[error("invalid RUNTIME_LIQUIDITY_POOL_SNAPSHOT_JITTER_SECONDS: {0}")]
    InvalidLiquidityPoolSnapshotJitterSeconds(String),
    #[error("invalid RUNTIME_LIQUIDITY_POOL_SNAPSHOT_RETENTION_COUNT: {0}")]
    InvalidLiquidityPoolSnapshotRetentionCount(String),
    #[error("invalid hydra fx policy config: {0}")]
    InvalidHydraFxPolicyConfig(String),
    #[error("invalid credit policy config: {0}")]
    InvalidCreditPolicyConfig(String),
}

impl Config {
    pub fn from_env() -> Result<Self, ConfigError> {
        let bind_addr = env::var("RUNTIME_BIND_ADDR")
            .unwrap_or_else(|_| "127.0.0.1:4100".to_string())
            .parse()?;
        let service_name =
            env::var("RUNTIME_SERVICE_NAME").unwrap_or_else(|_| "runtime".to_string());
        let build_sha = env::var("RUNTIME_BUILD_SHA").unwrap_or_else(|_| "dev".to_string());
        let db_url = env::var("DB_URL")
            .or_else(|_| env::var("DATABASE_URL"))
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
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
        let khala_outbound_queue_limit = env::var("RUNTIME_KHALA_OUTBOUND_QUEUE_LIMIT")
            .unwrap_or_else(|_| "200".to_string())
            .parse::<usize>()
            .map_err(|error| ConfigError::InvalidKhalaOutboundQueueLimit(error.to_string()))?;
        let khala_fair_topic_slice_limit = env::var("RUNTIME_KHALA_FAIR_TOPIC_SLICE_LIMIT")
            .unwrap_or_else(|_| "50".to_string())
            .parse::<usize>()
            .map_err(|error| ConfigError::InvalidKhalaFairTopicSliceLimit(error.to_string()))?;
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

        let bridge_nostr_relays = env::var("RUNTIME_BRIDGE_NOSTR_RELAYS")
            .unwrap_or_default()
            .split(',')
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .collect::<Vec<_>>();
        let bridge_nostr_secret_key_raw = env::var("RUNTIME_BRIDGE_NOSTR_SECRET_KEY")
            .unwrap_or_default()
            .trim()
            .to_string();
        let bridge_nostr_secret_key = if bridge_nostr_secret_key_raw.is_empty() {
            None
        } else {
            Some(parse_bridge_nostr_secret_key(&bridge_nostr_secret_key_raw)?)
        };
        if !bridge_nostr_relays.is_empty() && bridge_nostr_secret_key.is_none() {
            return Err(ConfigError::InvalidBridgeNostrSecretKey(
                "bridge relays configured but secret key missing".to_string(),
            ));
        }

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
        let khala_outbound_queue_limit = khala_outbound_queue_limit.max(1);
        let khala_fair_topic_slice_limit = khala_fair_topic_slice_limit.max(1);
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

        let verifier_strict =
            parse_bool_env("RUNTIME_VERIFIER_STRICT", false).map_err(|error| {
                ConfigError::InvalidVerifierStrict(format!("RUNTIME_VERIFIER_STRICT: {error}"))
            })?;
        let verifier_allowed_signer_pubkeys_raw =
            env::var("RUNTIME_VERIFIER_ALLOWED_SIGNER_PUBKEYS").unwrap_or_default();
        let mut verifier_allowed_signer_pubkeys = HashSet::new();
        for raw in verifier_allowed_signer_pubkeys_raw.split(',') {
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                continue;
            }
            let normalized = trimmed.to_ascii_lowercase();
            if normalized.len() != 64 || !normalized.chars().all(|c| c.is_ascii_hexdigit()) {
                return Err(ConfigError::InvalidVerifierAllowedSignerPubkeys(format!(
                    "expected 64-char hex pubkey, got: {trimmed}"
                )));
            }
            verifier_allowed_signer_pubkeys.insert(normalized);
        }

        let treasury_reconciliation_enabled =
            parse_bool_env("RUNTIME_TREASURY_RECONCILIATION_ENABLED", false).map_err(|error| {
                ConfigError::InvalidTreasuryReconciliationEnabled(format!(
                    "RUNTIME_TREASURY_RECONCILIATION_ENABLED: {error}"
                ))
            })?;
        let treasury_reservation_ttl_seconds = env::var("RUNTIME_TREASURY_RESERVATION_TTL_SECONDS")
            .unwrap_or_else(|_| "3600".to_string())
            .parse::<u64>()
            .map_err(|error| ConfigError::InvalidTreasuryReservationTtlSeconds(error.to_string()))?
            .max(1);
        let treasury_reconciliation_interval_seconds =
            env::var("RUNTIME_TREASURY_RECONCILIATION_INTERVAL_SECONDS")
                .unwrap_or_else(|_| "60".to_string())
                .parse::<u64>()
                .map_err(|error| {
                    ConfigError::InvalidTreasuryReconciliationIntervalSeconds(error.to_string())
                })?
                .max(1);
        let treasury_reconciliation_max_jobs = env::var("RUNTIME_TREASURY_RECONCILIATION_MAX_JOBS")
            .unwrap_or_else(|_| "200".to_string())
            .parse::<usize>()
            .map_err(|error| ConfigError::InvalidTreasuryReconciliationMaxJobs(error.to_string()))?
            .clamp(1, 2000);

        let liquidity_wallet_executor_base_url =
            env::var("RUNTIME_LIQUIDITY_WALLET_EXECUTOR_BASE_URL")
                .unwrap_or_default()
                .trim()
                .to_string();
        let liquidity_wallet_executor_base_url = liquidity_wallet_executor_base_url
            .trim_end_matches('/')
            .to_string();
        let liquidity_wallet_executor_base_url = (!liquidity_wallet_executor_base_url.is_empty())
            .then_some(liquidity_wallet_executor_base_url);

        let liquidity_wallet_executor_auth_token =
            env::var("RUNTIME_LIQUIDITY_WALLET_EXECUTOR_AUTH_TOKEN")
                .unwrap_or_default()
                .trim()
                .to_string();
        let liquidity_wallet_executor_auth_token = (!liquidity_wallet_executor_auth_token
            .is_empty())
        .then_some(liquidity_wallet_executor_auth_token);

        let liquidity_wallet_executor_timeout_ms =
            env::var("RUNTIME_LIQUIDITY_WALLET_EXECUTOR_TIMEOUT_MS")
                .unwrap_or_else(|_| "12000".to_string())
                .parse::<u64>()
                .map_err(|error| {
                    ConfigError::InvalidLiquidityWalletExecutorTimeoutMs(error.to_string())
                })?
                .max(250)
                .min(120_000);

        let liquidity_quote_ttl_seconds = env::var("RUNTIME_LIQUIDITY_QUOTE_TTL_SECONDS")
            .unwrap_or_else(|_| "60".to_string())
            .parse::<u64>()
            .map_err(|error| ConfigError::InvalidLiquidityQuoteTtlSeconds(error.to_string()))?
            .max(5)
            .min(3600);

        let liquidity_pool_withdraw_delay_hours =
            env::var("RUNTIME_LIQUIDITY_POOL_WITHDRAW_DELAY_HOURS")
                .unwrap_or_else(|_| "24".to_string())
                .parse::<i64>()
                .map_err(|error| {
                    ConfigError::InvalidLiquidityPoolWithdrawDelayHours(error.to_string())
                })?
                .clamp(0, 168);
        let liquidity_pool_withdraw_throttle =
            parse_liquidity_pool_withdraw_throttle_from_env(|key| env::var(key).ok())?;
        let liquidity_pool_snapshot_worker_enabled =
            parse_bool_env("RUNTIME_LIQUIDITY_POOL_SNAPSHOT_WORKER_ENABLED", true).map_err(
                |error| ConfigError::InvalidLiquidityPoolSnapshotWorkerEnabled(error.to_string()),
            )?;
        let liquidity_pool_snapshot_pool_ids = env::var("RUNTIME_LIQUIDITY_POOL_SNAPSHOT_POOL_IDS")
            .unwrap_or_else(|_| "llp-main".to_string())
            .split(',')
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string)
            .collect::<Vec<_>>();
        let liquidity_pool_snapshot_interval_seconds =
            env::var("RUNTIME_LIQUIDITY_POOL_SNAPSHOT_INTERVAL_SECONDS")
                .unwrap_or_else(|_| "60".to_string())
                .parse::<u64>()
                .map_err(|error| {
                    ConfigError::InvalidLiquidityPoolSnapshotIntervalSeconds(error.to_string())
                })?
                .clamp(1, 3600);
        let liquidity_pool_snapshot_jitter_seconds =
            env::var("RUNTIME_LIQUIDITY_POOL_SNAPSHOT_JITTER_SECONDS")
                .unwrap_or_else(|_| "5".to_string())
                .parse::<u64>()
                .map_err(|error| {
                    ConfigError::InvalidLiquidityPoolSnapshotJitterSeconds(error.to_string())
                })?
                .clamp(0, 60);
        let liquidity_pool_snapshot_retention_count =
            env::var("RUNTIME_LIQUIDITY_POOL_SNAPSHOT_RETENTION_COUNT")
                .unwrap_or_else(|_| "120".to_string())
                .parse::<i64>()
                .map_err(|error| {
                    ConfigError::InvalidLiquidityPoolSnapshotRetentionCount(error.to_string())
                })?
                .clamp(1, 10_000);
        let hydra_fx_policy = parse_hydra_fx_policy_from_env(|key| env::var(key).ok())?;
        let credit_policy = parse_credit_policy_from_env(|key| env::var(key).ok())?;
        Ok(Self {
            service_name,
            bind_addr,
            build_sha,
            db_url,
            authority_write_mode,
            fanout_driver,
            fanout_queue_capacity,
            khala_poll_default_limit,
            khala_poll_max_limit,
            khala_outbound_queue_limit,
            khala_fair_topic_slice_limit,
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
            verifier_strict,
            verifier_allowed_signer_pubkeys,
            bridge_nostr_relays,
            bridge_nostr_secret_key,
            liquidity_wallet_executor_base_url,
            liquidity_wallet_executor_auth_token,
            liquidity_wallet_executor_timeout_ms,
            liquidity_quote_ttl_seconds,
            liquidity_pool_withdraw_delay_hours,
            liquidity_pool_withdraw_throttle,
            liquidity_pool_snapshot_worker_enabled,
            liquidity_pool_snapshot_pool_ids,
            liquidity_pool_snapshot_interval_seconds,
            liquidity_pool_snapshot_jitter_seconds,
            liquidity_pool_snapshot_retention_count,
            hydra_fx_policy,
            credit_policy,
            treasury_reconciliation_enabled,
            treasury_reservation_ttl_seconds,
            treasury_reconciliation_interval_seconds,
            treasury_reconciliation_max_jobs,
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

fn parse_credit_policy_from_env(
    lookup: impl Fn(&str) -> Option<String>,
) -> Result<CreditPolicyConfig, ConfigError> {
    let defaults = CreditPolicyConfig::default();

    let max_sats_per_envelope = parse_u64_env_lookup(
        &lookup,
        "RUNTIME_CREDIT_MAX_SATS_PER_ENVELOPE",
        defaults.max_sats_per_envelope,
        1,
        u64::MAX,
    )?;
    let max_outstanding_envelopes_per_agent = parse_u64_env_lookup(
        &lookup,
        "RUNTIME_CREDIT_MAX_OUTSTANDING_ENVELOPES_PER_AGENT",
        defaults.max_outstanding_envelopes_per_agent,
        1,
        u64::MAX,
    )?;
    let max_offer_ttl_seconds = parse_u64_env_lookup(
        &lookup,
        "RUNTIME_CREDIT_MAX_OFFER_TTL_SECONDS",
        defaults.max_offer_ttl_seconds,
        1,
        86_400,
    )?;

    let underwriting_history_days = parse_i64_env_lookup(
        &lookup,
        "RUNTIME_CREDIT_UNDERWRITING_HISTORY_DAYS",
        defaults.underwriting_history_days,
        1,
        365,
    )?;
    let underwriting_base_sats = parse_u64_env_lookup(
        &lookup,
        "RUNTIME_CREDIT_UNDERWRITING_BASE_SATS",
        defaults.underwriting_base_sats,
        1,
        u64::MAX,
    )?;
    let underwriting_k = parse_f64_env_lookup(
        &lookup,
        "RUNTIME_CREDIT_UNDERWRITING_K",
        defaults.underwriting_k,
        0.0,
        10_000.0,
    )?;
    let underwriting_default_penalty_multiplier = parse_f64_env_lookup(
        &lookup,
        "RUNTIME_CREDIT_UNDERWRITING_DEFAULT_PENALTY_MULTIPLIER",
        defaults.underwriting_default_penalty_multiplier,
        0.0,
        100.0,
    )?;

    let min_fee_bps = parse_u32_env_lookup(
        &lookup,
        "RUNTIME_CREDIT_MIN_FEE_BPS",
        defaults.min_fee_bps,
        0,
        100_000,
    )?;
    let max_fee_bps = parse_u32_env_lookup(
        &lookup,
        "RUNTIME_CREDIT_MAX_FEE_BPS",
        defaults.max_fee_bps,
        min_fee_bps,
        100_000,
    )?;
    let fee_risk_scaler = parse_f64_env_lookup(
        &lookup,
        "RUNTIME_CREDIT_FEE_RISK_SCALER",
        defaults.fee_risk_scaler,
        0.0,
        100_000.0,
    )?;

    let health_window_seconds = parse_i64_env_lookup(
        &lookup,
        "RUNTIME_CREDIT_HEALTH_WINDOW_SECONDS",
        defaults.health_window_seconds,
        60,
        7 * 24 * 60 * 60,
    )?;
    let health_settlement_sample_limit = parse_u32_env_lookup(
        &lookup,
        "RUNTIME_CREDIT_HEALTH_SETTLEMENT_SAMPLE_LIMIT",
        defaults.health_settlement_sample_limit,
        1,
        100_000,
    )?;
    let health_ln_pay_sample_limit = parse_u32_env_lookup(
        &lookup,
        "RUNTIME_CREDIT_HEALTH_LN_PAY_SAMPLE_LIMIT",
        defaults.health_ln_pay_sample_limit,
        1,
        100_000,
    )?;
    let circuit_breaker_min_sample = parse_u64_env_lookup(
        &lookup,
        "RUNTIME_CREDIT_CIRCUIT_BREAKER_MIN_SAMPLE",
        defaults.circuit_breaker_min_sample,
        1,
        100_000,
    )?;
    let loss_rate_halt_threshold = parse_f64_env_lookup(
        &lookup,
        "RUNTIME_CREDIT_LOSS_RATE_HALT_THRESHOLD",
        defaults.loss_rate_halt_threshold,
        0.0,
        1.0,
    )?;
    let ln_failure_rate_halt_threshold = parse_f64_env_lookup(
        &lookup,
        "RUNTIME_CREDIT_LN_FAILURE_RATE_HALT_THRESHOLD",
        defaults.ln_failure_rate_halt_threshold,
        0.0,
        1.0,
    )?;
    let ln_failure_large_settlement_cap_sats = parse_u64_env_lookup(
        &lookup,
        "RUNTIME_CREDIT_LN_FAILURE_LARGE_SETTLEMENT_CAP_SATS",
        defaults.ln_failure_large_settlement_cap_sats,
        1,
        u64::MAX,
    )?;

    Ok(CreditPolicyConfig {
        max_sats_per_envelope,
        max_outstanding_envelopes_per_agent,
        max_offer_ttl_seconds,
        underwriting_history_days,
        underwriting_base_sats,
        underwriting_k,
        underwriting_default_penalty_multiplier,
        min_fee_bps,
        max_fee_bps,
        fee_risk_scaler,
        health_window_seconds,
        health_settlement_sample_limit,
        health_ln_pay_sample_limit,
        circuit_breaker_min_sample,
        loss_rate_halt_threshold,
        ln_failure_rate_halt_threshold,
        ln_failure_large_settlement_cap_sats,
    })
}

fn parse_liquidity_pool_withdraw_throttle_from_env(
    lookup: impl Fn(&str) -> Option<String>,
) -> Result<LiquidityPoolWithdrawThrottleConfig, ConfigError> {
    let defaults = LiquidityPoolWithdrawThrottleConfig::default();

    let lp_mode_enabled = parse_with_lookup(
        &lookup,
        "RUNTIME_LIQUIDITY_POOL_LP_MODE_ENABLED",
        defaults.lp_mode_enabled,
        |raw| match raw.trim().to_ascii_lowercase().as_str() {
            "1" | "true" | "yes" | "on" => Ok(true),
            "0" | "false" | "no" | "off" => Ok(false),
            other => Err(ConfigError::InvalidLiquidityPoolWithdrawThrottleConfig(
                format!("RUNTIME_LIQUIDITY_POOL_LP_MODE_ENABLED: {other}"),
            )),
        },
    )?;

    let stress_liability_ratio_bps = parse_with_lookup(
        &lookup,
        "RUNTIME_LIQUIDITY_POOL_WITHDRAW_THROTTLE_STRESS_LIABILITY_RATIO_BPS",
        defaults.stress_liability_ratio_bps,
        |raw| {
            raw.parse::<u32>()
                .map_err(|error| {
                    ConfigError::InvalidLiquidityPoolWithdrawThrottleConfig(format!(
                        "RUNTIME_LIQUIDITY_POOL_WITHDRAW_THROTTLE_STRESS_LIABILITY_RATIO_BPS: {error}"
                    ))
                })
                .map(|value| value.clamp(1, 10_000))
        },
    )?;
    let halt_liability_ratio_bps = parse_with_lookup(
        &lookup,
        "RUNTIME_LIQUIDITY_POOL_WITHDRAW_THROTTLE_HALT_LIABILITY_RATIO_BPS",
        defaults.halt_liability_ratio_bps,
        |raw| {
            raw.parse::<u32>()
                .map_err(|error| {
                    ConfigError::InvalidLiquidityPoolWithdrawThrottleConfig(format!(
                        "RUNTIME_LIQUIDITY_POOL_WITHDRAW_THROTTLE_HALT_LIABILITY_RATIO_BPS: {error}"
                    ))
                })
                .map(|value| value.clamp(1, 10_000))
        },
    )?;
    let stress_connected_ratio_bps = parse_with_lookup(
        &lookup,
        "RUNTIME_LIQUIDITY_POOL_WITHDRAW_THROTTLE_STRESS_CONNECTED_RATIO_BPS",
        defaults.stress_connected_ratio_bps,
        |raw| {
            raw.parse::<u32>()
                .map_err(|error| {
                    ConfigError::InvalidLiquidityPoolWithdrawThrottleConfig(format!(
                        "RUNTIME_LIQUIDITY_POOL_WITHDRAW_THROTTLE_STRESS_CONNECTED_RATIO_BPS: {error}"
                    ))
                })
                .map(|value| value.clamp(0, 10_000))
        },
    )?;
    let halt_connected_ratio_bps = parse_with_lookup(
        &lookup,
        "RUNTIME_LIQUIDITY_POOL_WITHDRAW_THROTTLE_HALT_CONNECTED_RATIO_BPS",
        defaults.halt_connected_ratio_bps,
        |raw| {
            raw.parse::<u32>()
                .map_err(|error| {
                    ConfigError::InvalidLiquidityPoolWithdrawThrottleConfig(format!(
                        "RUNTIME_LIQUIDITY_POOL_WITHDRAW_THROTTLE_HALT_CONNECTED_RATIO_BPS: {error}"
                    ))
                })
                .map(|value| value.clamp(0, 10_000))
        },
    )?;
    let stress_outbound_coverage_bps = parse_with_lookup(
        &lookup,
        "RUNTIME_LIQUIDITY_POOL_WITHDRAW_THROTTLE_STRESS_OUTBOUND_COVERAGE_BPS",
        defaults.stress_outbound_coverage_bps,
        |raw| {
            raw.parse::<u32>()
                .map_err(|error| {
                    ConfigError::InvalidLiquidityPoolWithdrawThrottleConfig(format!(
                        "RUNTIME_LIQUIDITY_POOL_WITHDRAW_THROTTLE_STRESS_OUTBOUND_COVERAGE_BPS: {error}"
                    ))
                })
                .map(|value| value.clamp(0, 50_000))
        },
    )?;
    let halt_outbound_coverage_bps = parse_with_lookup(
        &lookup,
        "RUNTIME_LIQUIDITY_POOL_WITHDRAW_THROTTLE_HALT_OUTBOUND_COVERAGE_BPS",
        defaults.halt_outbound_coverage_bps,
        |raw| {
            raw.parse::<u32>()
                .map_err(|error| {
                    ConfigError::InvalidLiquidityPoolWithdrawThrottleConfig(format!(
                        "RUNTIME_LIQUIDITY_POOL_WITHDRAW_THROTTLE_HALT_OUTBOUND_COVERAGE_BPS: {error}"
                    ))
                })
                .map(|value| value.clamp(0, 50_000))
        },
    )?;
    let stress_extra_delay_hours = parse_with_lookup(
        &lookup,
        "RUNTIME_LIQUIDITY_POOL_WITHDRAW_THROTTLE_STRESS_EXTRA_DELAY_HOURS",
        defaults.stress_extra_delay_hours,
        |raw| {
            raw.parse::<i64>()
                .map_err(|error| {
                    ConfigError::InvalidLiquidityPoolWithdrawThrottleConfig(format!(
                        "RUNTIME_LIQUIDITY_POOL_WITHDRAW_THROTTLE_STRESS_EXTRA_DELAY_HOURS: {error}"
                    ))
                })
                .map(|value| value.clamp(0, 336))
        },
    )?;
    let halt_extra_delay_hours = parse_with_lookup(
        &lookup,
        "RUNTIME_LIQUIDITY_POOL_WITHDRAW_THROTTLE_HALT_EXTRA_DELAY_HOURS",
        defaults.halt_extra_delay_hours,
        |raw| {
            raw.parse::<i64>()
                .map_err(|error| {
                    ConfigError::InvalidLiquidityPoolWithdrawThrottleConfig(format!(
                        "RUNTIME_LIQUIDITY_POOL_WITHDRAW_THROTTLE_HALT_EXTRA_DELAY_HOURS: {error}"
                    ))
                })
                .map(|value| value.clamp(0, 336))
        },
    )?;
    let stress_execution_cap_per_tick = parse_with_lookup(
        &lookup,
        "RUNTIME_LIQUIDITY_POOL_WITHDRAW_THROTTLE_STRESS_EXECUTION_CAP_PER_TICK",
        defaults.stress_execution_cap_per_tick,
        |raw| {
            raw.parse::<u32>()
                .map_err(|error| {
                    ConfigError::InvalidLiquidityPoolWithdrawThrottleConfig(format!(
                        "RUNTIME_LIQUIDITY_POOL_WITHDRAW_THROTTLE_STRESS_EXECUTION_CAP_PER_TICK: {error}"
                    ))
                })
                .map(|value| value.clamp(1, 5_000))
        },
    )?;

    let mut config = LiquidityPoolWithdrawThrottleConfig {
        lp_mode_enabled,
        stress_liability_ratio_bps,
        halt_liability_ratio_bps,
        stress_connected_ratio_bps,
        halt_connected_ratio_bps,
        stress_outbound_coverage_bps,
        halt_outbound_coverage_bps,
        stress_extra_delay_hours,
        halt_extra_delay_hours,
        stress_execution_cap_per_tick,
    };

    if config.halt_liability_ratio_bps < config.stress_liability_ratio_bps {
        config.halt_liability_ratio_bps = config.stress_liability_ratio_bps;
    }
    if config.halt_connected_ratio_bps > config.stress_connected_ratio_bps {
        config.halt_connected_ratio_bps = config.stress_connected_ratio_bps;
    }
    if config.halt_outbound_coverage_bps > config.stress_outbound_coverage_bps {
        config.halt_outbound_coverage_bps = config.stress_outbound_coverage_bps;
    }
    if config.halt_extra_delay_hours < config.stress_extra_delay_hours {
        config.halt_extra_delay_hours = config.stress_extra_delay_hours;
    }

    Ok(config)
}

fn parse_u64_env_lookup(
    lookup: &impl Fn(&str) -> Option<String>,
    key: &str,
    default: u64,
    min: u64,
    max: u64,
) -> Result<u64, ConfigError> {
    parse_with_lookup(lookup, key, default, |raw| {
        raw.parse::<u64>()
            .map_err(|error| ConfigError::InvalidCreditPolicyConfig(format!("{key}: {error}")))
            .map(|value| value.clamp(min, max))
    })
}

fn parse_u32_env_lookup(
    lookup: &impl Fn(&str) -> Option<String>,
    key: &str,
    default: u32,
    min: u32,
    max: u32,
) -> Result<u32, ConfigError> {
    parse_with_lookup(lookup, key, default, |raw| {
        raw.parse::<u32>()
            .map_err(|error| ConfigError::InvalidCreditPolicyConfig(format!("{key}: {error}")))
            .map(|value| value.clamp(min, max))
    })
}

fn parse_i64_env_lookup(
    lookup: &impl Fn(&str) -> Option<String>,
    key: &str,
    default: i64,
    min: i64,
    max: i64,
) -> Result<i64, ConfigError> {
    parse_with_lookup(lookup, key, default, |raw| {
        raw.parse::<i64>()
            .map_err(|error| ConfigError::InvalidCreditPolicyConfig(format!("{key}: {error}")))
            .map(|value| value.clamp(min, max))
    })
}

fn parse_f64_env_lookup(
    lookup: &impl Fn(&str) -> Option<String>,
    key: &str,
    default: f64,
    min: f64,
    max: f64,
) -> Result<f64, ConfigError> {
    parse_with_lookup(lookup, key, default, |raw| {
        raw.parse::<f64>()
            .map_err(|error| ConfigError::InvalidCreditPolicyConfig(format!("{key}: {error}")))
            .map(|value| value.clamp(min, max))
    })
}

fn parse_with_lookup<T>(
    lookup: &impl Fn(&str) -> Option<String>,
    key: &str,
    default: T,
    parser: impl FnOnce(String) -> Result<T, ConfigError>,
) -> Result<T, ConfigError> {
    match lookup(key) {
        Some(raw) => parser(raw),
        None => Ok(default),
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

fn normalize_hydra_fx_asset(value: &str) -> String {
    value.trim().to_ascii_uppercase()
}

fn normalize_hydra_fx_pair(sell_asset: &str, buy_asset: &str) -> String {
    format!(
        "{}->{}",
        normalize_hydra_fx_asset(sell_asset),
        normalize_hydra_fx_asset(buy_asset)
    )
}

fn parse_hydra_fx_pair(raw: &str) -> Result<String, ConfigError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(ConfigError::InvalidHydraFxPolicyConfig(
            "empty pair token".to_string(),
        ));
    }
    let (sell, buy) = if let Some((sell, buy)) = trimmed.split_once("->") {
        (sell, buy)
    } else if let Some((sell, buy)) = trimmed.split_once(':') {
        (sell, buy)
    } else {
        return Err(ConfigError::InvalidHydraFxPolicyConfig(format!(
            "invalid pair format: {trimmed}"
        )));
    };
    let sell_norm = normalize_hydra_fx_asset(sell);
    let buy_norm = normalize_hydra_fx_asset(buy);
    if sell_norm.is_empty() || buy_norm.is_empty() {
        return Err(ConfigError::InvalidHydraFxPolicyConfig(format!(
            "invalid pair with empty asset: {trimmed}"
        )));
    }
    Ok(normalize_hydra_fx_pair(&sell_norm, &buy_norm))
}

fn parse_hydra_fx_policy_from_env(
    lookup: impl Fn(&str) -> Option<String>,
) -> Result<HydraFxPolicyConfig, ConfigError> {
    let defaults = HydraFxPolicyConfig::default();

    let allowed_pairs = parse_with_lookup(
        &lookup,
        "RUNTIME_HYDRA_FX_ALLOWED_PAIRS",
        defaults.allowed_pairs.clone(),
        |raw| {
            let mut parsed = HashSet::new();
            for token in raw.split(',') {
                let pair = parse_hydra_fx_pair(token)?;
                parsed.insert(pair);
            }
            if parsed.is_empty() {
                return Err(ConfigError::InvalidHydraFxPolicyConfig(
                    "RUNTIME_HYDRA_FX_ALLOWED_PAIRS cannot be empty".to_string(),
                ));
            }
            Ok(parsed)
        },
    )?;

    let max_spread_bps = parse_with_lookup(
        &lookup,
        "RUNTIME_HYDRA_FX_MAX_SPREAD_BPS",
        defaults.max_spread_bps,
        |raw| {
            raw.parse::<u32>()
                .map_err(|error| {
                    ConfigError::InvalidHydraFxPolicyConfig(format!(
                        "RUNTIME_HYDRA_FX_MAX_SPREAD_BPS: {error}"
                    ))
                })
                .map(|value| value.clamp(1, 10_000))
        },
    )?;

    let max_fee_bps = parse_with_lookup(
        &lookup,
        "RUNTIME_HYDRA_FX_MAX_FEE_BPS",
        defaults.max_fee_bps,
        |raw| {
            raw.parse::<u32>()
                .map_err(|error| {
                    ConfigError::InvalidHydraFxPolicyConfig(format!(
                        "RUNTIME_HYDRA_FX_MAX_FEE_BPS: {error}"
                    ))
                })
                .map(|value| value.clamp(0, 10_000))
        },
    )?;

    let min_quote_ttl_seconds = parse_with_lookup(
        &lookup,
        "RUNTIME_HYDRA_FX_MIN_QUOTE_TTL_SECONDS",
        defaults.min_quote_ttl_seconds,
        |raw| {
            raw.parse::<u32>()
                .map_err(|error| {
                    ConfigError::InvalidHydraFxPolicyConfig(format!(
                        "RUNTIME_HYDRA_FX_MIN_QUOTE_TTL_SECONDS: {error}"
                    ))
                })
                .map(|value| value.clamp(1, 3600))
        },
    )?;

    let mut max_quote_ttl_seconds = parse_with_lookup(
        &lookup,
        "RUNTIME_HYDRA_FX_MAX_QUOTE_TTL_SECONDS",
        defaults.max_quote_ttl_seconds,
        |raw| {
            raw.parse::<u32>()
                .map_err(|error| {
                    ConfigError::InvalidHydraFxPolicyConfig(format!(
                        "RUNTIME_HYDRA_FX_MAX_QUOTE_TTL_SECONDS: {error}"
                    ))
                })
                .map(|value| value.clamp(1, 86_400))
        },
    )?;
    if max_quote_ttl_seconds < min_quote_ttl_seconds {
        max_quote_ttl_seconds = min_quote_ttl_seconds;
    }

    Ok(HydraFxPolicyConfig {
        allowed_pairs,
        max_spread_bps,
        max_fee_bps,
        min_quote_ttl_seconds,
        max_quote_ttl_seconds,
    })
}

fn parse_bridge_nostr_secret_key(value: &str) -> Result<[u8; 32], ConfigError> {
    let trimmed = value.trim();
    if trimmed.starts_with("nsec1") {
        return nsec_to_private_key(trimmed)
            .map_err(|err| ConfigError::InvalidBridgeNostrSecretKey(err.to_string()));
    }

    let bytes = hex::decode(trimmed)
        .map_err(|err| ConfigError::InvalidBridgeNostrSecretKey(err.to_string()))?;
    if bytes.len() != 32 {
        return Err(ConfigError::InvalidBridgeNostrSecretKey(
            "expected 32-byte hex secret".to_string(),
        ));
    }
    let mut out = [0u8; 32];
    out.copy_from_slice(&bytes);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::{ConfigError, parse_credit_policy_from_env, parse_hydra_fx_policy_from_env};

    #[test]
    fn credit_policy_parser_applies_env_overrides() {
        let values = HashMap::from([
            ("RUNTIME_CREDIT_MAX_SATS_PER_ENVELOPE", "2500"),
            ("RUNTIME_CREDIT_MAX_OUTSTANDING_ENVELOPES_PER_AGENT", "7"),
            ("RUNTIME_CREDIT_MAX_OFFER_TTL_SECONDS", "90"),
            ("RUNTIME_CREDIT_UNDERWRITING_BASE_SATS", "500"),
            ("RUNTIME_CREDIT_UNDERWRITING_K", "12.5"),
            ("RUNTIME_CREDIT_MIN_FEE_BPS", "25"),
            ("RUNTIME_CREDIT_MAX_FEE_BPS", "500"),
            ("RUNTIME_CREDIT_CIRCUIT_BREAKER_MIN_SAMPLE", "9"),
            ("RUNTIME_CREDIT_LOSS_RATE_HALT_THRESHOLD", "0.35"),
            ("RUNTIME_CREDIT_LN_FAILURE_RATE_HALT_THRESHOLD", "0.4"),
            ("RUNTIME_CREDIT_LN_FAILURE_LARGE_SETTLEMENT_CAP_SATS", "750"),
        ]);
        let policy = parse_credit_policy_from_env(|key| values.get(key).map(ToString::to_string))
            .expect("policy parse");
        assert_eq!(policy.max_sats_per_envelope, 2500);
        assert_eq!(policy.max_outstanding_envelopes_per_agent, 7);
        assert_eq!(policy.max_offer_ttl_seconds, 90);
        assert_eq!(policy.underwriting_base_sats, 500);
        assert_eq!(policy.underwriting_k, 12.5);
        assert_eq!(policy.min_fee_bps, 25);
        assert_eq!(policy.max_fee_bps, 500);
        assert_eq!(policy.circuit_breaker_min_sample, 9);
        assert_eq!(policy.loss_rate_halt_threshold, 0.35);
        assert_eq!(policy.ln_failure_rate_halt_threshold, 0.4);
        assert_eq!(policy.ln_failure_large_settlement_cap_sats, 750);
    }

    #[test]
    fn credit_policy_parser_rejects_invalid_env_values() {
        let values = HashMap::from([("RUNTIME_CREDIT_UNDERWRITING_K", "not-a-number")]);
        let error = parse_credit_policy_from_env(|key| values.get(key).map(ToString::to_string))
            .expect_err("invalid value should fail");
        match error {
            ConfigError::InvalidCreditPolicyConfig(message) => {
                assert!(message.contains("RUNTIME_CREDIT_UNDERWRITING_K"));
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn hydra_fx_policy_parser_normalizes_pairs_and_bounds() {
        let values = HashMap::from([
            ("RUNTIME_HYDRA_FX_ALLOWED_PAIRS", "usd:btc_ln, usdt->btc_ln"),
            ("RUNTIME_HYDRA_FX_MAX_SPREAD_BPS", "250"),
            ("RUNTIME_HYDRA_FX_MAX_FEE_BPS", "45"),
            ("RUNTIME_HYDRA_FX_MIN_QUOTE_TTL_SECONDS", "15"),
            ("RUNTIME_HYDRA_FX_MAX_QUOTE_TTL_SECONDS", "45"),
        ]);
        let policy = parse_hydra_fx_policy_from_env(|key| values.get(key).map(ToString::to_string))
            .expect("hydra fx policy parse");
        assert!(policy.allowed_pairs.contains("USD->BTC_LN"));
        assert!(policy.allowed_pairs.contains("USDT->BTC_LN"));
        assert_eq!(policy.max_spread_bps, 250);
        assert_eq!(policy.max_fee_bps, 45);
        assert_eq!(policy.min_quote_ttl_seconds, 15);
        assert_eq!(policy.max_quote_ttl_seconds, 45);
    }

    #[test]
    fn hydra_fx_policy_parser_rejects_invalid_pair_token() {
        let values = HashMap::from([("RUNTIME_HYDRA_FX_ALLOWED_PAIRS", "badtoken")]);
        let error = parse_hydra_fx_policy_from_env(|key| values.get(key).map(ToString::to_string))
            .expect_err("invalid pair token should fail");
        match error {
            ConfigError::InvalidHydraFxPolicyConfig(message) => {
                assert!(message.contains("invalid pair format"));
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }
}
