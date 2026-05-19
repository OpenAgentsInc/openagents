use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::future::Future;
use std::path::{Path, PathBuf};
#[cfg(test)]
use std::pin::Pin;
#[cfg(test)]
use std::sync::OnceLock;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[cfg(test)]
use std::sync::Mutex;

use anyhow::{Context, Result, anyhow, bail};
use bip39::{Language, Mnemonic};
use openagents_provider_substrate::{
    PYLON_PAYMENT_TARGET_VERSION_V0_2, ProviderPaymentTargetRegistration,
    infer_ldk_payment_target_kind, is_ldk_payment_target_kind, ldk_payment_target_capabilities,
    verify_provider_payment_target_registration_signature,
};
use semver::Version;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::economy::AuthorityReceiptContext;
use crate::treasury_provider::{
    LdkChainBackend, LdkNetwork, LdkServerBalances, LdkServerChannel, LdkServerClient,
    LdkTreasuryProvider, LdkTreasuryProviderConfig, TreasuryLightningProvider,
    TreasuryLightningProviderConfig, TreasuryLightningProviderKind, TreasuryProviderFundingRequest,
    TreasuryProviderFundingTarget, TreasuryProviderPayoutRequest,
};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PaymentSummary {
    pub id: String,
    pub direction: String,
    pub status: String,
    pub amount_sats: u64,
    pub fees_sats: u64,
    pub timestamp: u64,
    pub method: String,
    pub description: Option<String>,
    pub invoice: Option<String>,
    pub destination_pubkey: Option<String>,
    pub payment_hash: Option<String>,
    pub htlc_status: Option<String>,
    pub htlc_expiry_epoch_seconds: Option<u64>,
    pub status_detail: Option<String>,
}

const ENV_TREASURY_STATE_PATH: &str = "NEXUS_CONTROL_TREASURY_STATE_PATH";
const ENV_TREASURY_ENABLED: &str = "NEXUS_CONTROL_TREASURY_ENABLED";
const ENV_TREASURY_PAYOUT_SATS_PER_WINDOW: &str = "NEXUS_CONTROL_TREASURY_PAYOUT_SATS_PER_WINDOW";
const ENV_TREASURY_PAYOUT_INTERVAL_SECONDS: &str = "NEXUS_CONTROL_TREASURY_PAYOUT_INTERVAL_SECONDS";
const ENV_TREASURY_REQUIRE_SELLABLE: &str = "NEXUS_CONTROL_TREASURY_REQUIRE_SELLABLE";
const ENV_TREASURY_DAILY_BUDGET_CAP_SATS: &str = "NEXUS_CONTROL_TREASURY_DAILY_BUDGET_CAP_SATS";
const ENV_TREASURY_ACCEPTED_WORK_DEFAULT_PAYOUT_SATS: &str =
    "NEXUS_CONTROL_TREASURY_ACCEPTED_WORK_DEFAULT_PAYOUT_SATS";
const ENV_TREASURY_ACCEPTED_WORK_DAILY_BUDGET_CAP_SATS: &str =
    "NEXUS_CONTROL_TREASURY_ACCEPTED_WORK_DAILY_BUDGET_CAP_SATS";
const ENV_TREASURY_PLACEHOLDER_PAYOUT_MODE: &str = "NEXUS_CONTROL_TREASURY_PLACEHOLDER_PAYOUT_MODE";
const ENV_TREASURY_DEDUPE_PLACEHOLDER_HOSTS: &str =
    "NEXUS_CONTROL_TREASURY_DEDUPE_PLACEHOLDER_HOSTS";
const ENV_TREASURY_AVAILABILITY_MAX_CONCURRENT_SENDS: &str =
    "NEXUS_CONTROL_TREASURY_AVAILABILITY_MAX_CONCURRENT_SENDS";
const ENV_TREASURY_MIN_NEW_ACCRUAL_PYLON_VERSION: &str =
    "NEXUS_CONTROL_TREASURY_MIN_NEW_ACCRUAL_PYLON_VERSION";
const ENV_TREASURY_MIN_NEW_ACCRUAL_STARTED_AT_UNIX_MS: &str =
    "NEXUS_CONTROL_TREASURY_MIN_NEW_ACCRUAL_STARTED_AT_UNIX_MS";
const ENV_TREASURY_WALLET_MNEMONIC_PATH: &str = "NEXUS_CONTROL_TREASURY_WALLET_MNEMONIC_PATH";
const ENV_TREASURY_WALLET_STORAGE_DIR: &str = "NEXUS_CONTROL_TREASURY_WALLET_STORAGE_DIR";
const ENV_TREASURY_WALLET_NETWORK: &str = "NEXUS_CONTROL_TREASURY_WALLET_NETWORK";
const ENV_TREASURY_WALLET_API_KEY_ENV: &str = "NEXUS_CONTROL_TREASURY_WALLET_API_KEY_ENV";
const ENV_TREASURY_WALLET_REAL_TIME_SYNC_ENABLED: &str =
    "NEXUS_CONTROL_TREASURY_WALLET_REAL_TIME_SYNC_ENABLED";
const ENV_TREASURY_WALLET_STATUS_REFRESH_SECONDS: &str =
    "NEXUS_CONTROL_TREASURY_WALLET_STATUS_REFRESH_SECONDS";
const ENV_TREASURY_FUNDING_TARGET_TIMEOUT_MS: &str =
    "NEXUS_CONTROL_TREASURY_FUNDING_TARGET_TIMEOUT_MS";
const ENV_TREASURY_WALLET_RECOVERY_INSPECTION_TIMEOUT_MS: &str =
    "NEXUS_CONTROL_TREASURY_WALLET_RECOVERY_INSPECTION_TIMEOUT_MS";
const ENV_TREASURY_WALLET_RECOVERY_PARALLEL_INSPECTIONS: &str =
    "NEXUS_CONTROL_TREASURY_WALLET_RECOVERY_PARALLEL_INSPECTIONS";
const ENV_TREASURY_WALLET_RECOVERY_SCAN_PAYMENTS: &str =
    "NEXUS_CONTROL_TREASURY_WALLET_RECOVERY_SCAN_PAYMENTS";
const ENV_TREASURY_SIMULATED_WALLET_ENABLED: &str =
    "NEXUS_CONTROL_TREASURY_SIMULATED_WALLET_ENABLED";
const ENV_TREASURY_SIMULATED_WALLET_BALANCE_SATS: &str =
    "NEXUS_CONTROL_TREASURY_SIMULATED_WALLET_BALANCE_SATS";
const ENV_TREASURY_MAX_CONCURRENT_SENDS: &str = "NEXUS_CONTROL_TREASURY_MAX_CONCURRENT_SENDS";
const ENV_TREASURY_RECONCILIATION_HORIZON_SECONDS: &str =
    "NEXUS_CONTROL_TREASURY_RECONCILIATION_HORIZON_SECONDS";
const ENV_TREASURY_POLICY_APPLY_ENV: &str = "NEXUS_CONTROL_TREASURY_POLICY_APPLY_ENV";
const ENV_TREASURY_POLICY_ALLOW_DESTRUCTIVE_ENV_CHANGE: &str =
    "NEXUS_CONTROL_TREASURY_POLICY_ALLOW_DESTRUCTIVE_ENV_CHANGE";
const ENV_TREASURY_POLICY_CHANGE_REASON: &str = "NEXUS_CONTROL_TREASURY_POLICY_CHANGE_REASON";
const ENV_TREASURY_REGISTRATION_CHALLENGE_TTL_SECONDS: &str =
    "NEXUS_CONTROL_TREASURY_REGISTRATION_CHALLENGE_TTL_SECONDS";
const ENV_TREASURY_INTEGRATION_TOKEN: &str = "NEXUS_CONTROL_TREASURY_INTEGRATION_TOKEN";
const ENV_TREASURY_PROVIDER: &str = "NEXUS_TREASURY_PROVIDER";
const ENV_TREASURY_LDK_SERVER_URL: &str = "NEXUS_LDK_SERVER_URL";
const ENV_TREASURY_LDK_API_KEY_PATH: &str = "NEXUS_LDK_API_KEY_PATH";
const ENV_TREASURY_LDK_TLS_CERT_PATH: &str = "NEXUS_LDK_TLS_CERT_PATH";
const ENV_TREASURY_LDK_STORAGE_DIR: &str = "NEXUS_LDK_STORAGE_DIR";
const ENV_TREASURY_LDK_NETWORK: &str = "NEXUS_LDK_NETWORK";
const ENV_TREASURY_LDK_CHAIN_BACKEND: &str = "NEXUS_LDK_CHAIN_BACKEND";
const ENV_TREASURY_LDK_MIN_READY_CHANNEL_COUNT: &str = "NEXUS_LDK_MIN_READY_CHANNEL_COUNT";
const ENV_TREASURY_LDK_MIN_READY_OUTBOUND_CAPACITY_SATS: &str =
    "NEXUS_LDK_MIN_READY_OUTBOUND_CAPACITY_SATS";

const DEFAULT_TREASURY_STATE_PATH: &str = "var/nexus-control/treasury-state.json";
const DEFAULT_TREASURY_ENABLED: bool = false;
const DEFAULT_TREASURY_PAYOUT_SATS_PER_WINDOW: u64 = 0;
const DEFAULT_TREASURY_PAYOUT_INTERVAL_SECONDS: u64 = 600;
const DEFAULT_TREASURY_REQUIRE_SELLABLE: bool = false;
const DEFAULT_TREASURY_DAILY_BUDGET_CAP_SATS: u64 = 21_000;
const DEFAULT_TREASURY_ACCEPTED_WORK_DEFAULT_PAYOUT_SATS: Option<u64> = None;
const DEFAULT_TREASURY_ACCEPTED_WORK_DAILY_BUDGET_CAP_SATS: Option<u64> = None;
const DEFAULT_TREASURY_DEDUPE_PLACEHOLDER_HOSTS: bool = true;
const DEFAULT_TREASURY_MIN_NEW_ACCRUAL_STARTED_AT_UNIX_MS: Option<u64> = None;
const DEFAULT_TREASURY_WALLET_MNEMONIC_PATH: &str = "var/nexus-control/treasury.mnemonic";
const DEFAULT_TREASURY_WALLET_STORAGE_DIR: &str = "var/nexus-control/treasury-wallet";
const DEFAULT_TREASURY_WALLET_NETWORK: &str = "mainnet";
const DEFAULT_TREASURY_WALLET_REAL_TIME_SYNC_ENABLED: bool = false;
const DEFAULT_TREASURY_WALLET_STATUS_REFRESH_SECONDS: u64 = 3;
const DEFAULT_TREASURY_FUNDING_TARGET_TIMEOUT_MS: u64 = 10_000;
const DEFAULT_TREASURY_WALLET_RECOVERY_INSPECTION_TIMEOUT_MS: u64 = 120_000;
const DEFAULT_TREASURY_WALLET_RECOVERY_PARALLEL_INSPECTIONS: bool = false;
const DEFAULT_TREASURY_WALLET_RECOVERY_SCAN_PAYMENTS: bool = false;
const DEFAULT_TREASURY_LDK_STORAGE_DIR: &str = "var/nexus-control/ldk";
const DEFAULT_TREASURY_LDK_MIN_READY_CHANNEL_COUNT: u64 = 2;
const DEFAULT_TREASURY_LDK_MIN_READY_OUTBOUND_CAPACITY_SATS: u64 = 20_000;
const DEFAULT_TREASURY_SIMULATED_WALLET_ENABLED: bool = false;
const DEFAULT_TREASURY_SIMULATED_WALLET_BALANCE_SATS: u64 = 1_000_000;
const DEFAULT_TREASURY_MAX_CONCURRENT_SENDS: usize = 16;
const DEFAULT_TREASURY_AVAILABILITY_MAX_CONCURRENT_SENDS: Option<usize> = None;
const DEFAULT_TREASURY_RECONCILIATION_HORIZON_SECONDS: u64 = 86_400;
const DEFAULT_TREASURY_POLICY_APPLY_ENV: bool = false;
const DEFAULT_TREASURY_POLICY_ALLOW_DESTRUCTIVE_ENV_CHANGE: bool = false;
const DEFAULT_TREASURY_REGISTRATION_CHALLENGE_TTL_SECONDS: u64 = 300;
const TREASURY_RETIRED_UNPAYABLE_PAYOUT_REASON: &str = "retired_unpayable_non_ldk_payout_record";
const TREASURY_UNSUPPORTED_LDK_PAYMENT_TARGET_KIND_REASON: &str =
    "unsupported_ldk_payment_target_kind";
const TREASURY_PUBLIC_STATS_WINDOW_MS: u64 = 86_400_000;
const TREASURY_PAYOUT_TARGET_DOMAIN: &str = "openagents:nexus-treasury-payout-target:v1";
const TREASURY_POLICY_SCHEMA_VERSION: u32 = 3;
const TREASURY_STATE_RETENTION_WINDOW_MS: u64 = 30 * 86_400_000;
const TREASURY_DISPATCH_RESULT_TIMEOUT_MS: u64 = 180_000;
const TREASURY_FAILED_PAYOUT_RETRY_AFTER_MS: u64 = 60_000;
const TREASURY_LDK_CHANNEL_OPEN_RECONCILE_GRACE_MS: u64 = 60_000;
const TREASURY_TARGET_LIMIT: usize = 8_192;
const TREASURY_PAYOUT_LIMIT: usize = 262_144;
const TREASURY_PLACEHOLDER_PAYOUT_RECORD_LIMIT: usize = 1_024;
const TREASURY_PLACEHOLDER_PAYOUT_RECORD_RETENTION_WINDOW_MS: u64 = 86_400_000;
const TREASURY_RECEIVE_LIMIT: usize = 16_384;
const TREASURY_OPERATION_LIMIT: usize = 262_144;
const TREASURY_POLICY_CHANGE_LIMIT: usize = 64;
const TREASURY_STATUS_POLICY_CHANGE_LIMIT: usize = 8;
const TREASURY_STATUS_PAYOUT_TARGET_ROW_LIMIT: usize = 64;
const TREASURY_STATUS_PAYOUT_LEDGER_ROW_LIMIT: usize = 64;
const TREASURY_STATUS_AVAILABILITY_DEBUG_ROW_LIMIT: usize = 128;
const TREASURY_STATUS_LEGACY_AVAILABILITY_ATTENTION_LIMIT: usize = 32;
const TREASURY_IMPOSSIBLE_ZERO_BALANCE_THRESHOLD_SATS: u64 = 1_000;
const TREASURY_CONTINUITY_ALERT_THRESHOLD_MS: u64 = 300_000;
const TREASURY_CONFIRMATION_STALL_ALERT_THRESHOLD_MS: u64 = 15 * 60_000;
const TREASURY_STALE_SNAPSHOT_ALERT_THRESHOLD_MS: u64 = 15_000;
const TREASURY_LOW_LIQUIDITY_PAYOUT_MULTIPLIER: u64 = 3;
const TREASURY_LOW_LIQUIDITY_MIN_SATS: u64 = 1_000;
const TREASURY_FAILED_PAYMENT_ALERT_COUNT: u64 = 3;
const TREASURY_STALE_EVENT_SUBSCRIBER_MS: u64 = 60_000;
const TREASURY_STALE_GOSSIP_ALERT_MS: u64 = 15 * 60_000;
const TREASURY_MAX_CONCURRENT_SENDS_LIMIT: usize = 64;
const TREASURY_MAX_CONCURRENT_ACCEPTED_WORK_SENDS: usize = 4;
const TREASURY_MIN_WALLET_REFRESH_TIMEOUT_MS: u64 = 5_000;
const TREASURY_WALLET_REFRESH_RECENT_PAYMENT_PAGES: usize = 1;
const TREASURY_WALLET_REFRESH_CURSOR_PAYMENT_PAGES: usize = 8;
const TREASURY_WALLET_REFRESH_PAYMENT_PAGE_SIZE: usize = 100;
const TREASURY_WALLET_REFRESH_TRACKED_PAYMENT_LOOKUP_TIMEOUT_MS: u64 = 30_000;
const TREASURY_WALLET_REFRESH_MAX_PAYMENT_PAGES: usize = 8;
const TREASURY_AVAILABILITY_DISPATCH_BACKLOG_GUARD_LIMIT: u64 =
    TREASURY_WALLET_REFRESH_PAYMENT_PAGE_SIZE as u64;
const TREASURY_ORPHAN_SEND_PAYMENT_MATCH_EARLY_SLACK_MS: u64 = 5 * 60_000;
const TREASURY_ORPHAN_SEND_PAYMENT_MATCH_WINDOW_MS: u64 = 30 * 60_000;
const TREASURY_PUBLIC_SNAPSHOT_SOURCE_LOCAL: &str = "nexus_control";
const TREASURY_FUNDING_TARGET_TIMEOUT_PREFIX: &str = "treasury_funding_target_timeout:";
const TREASURY_MIN_WALLET_RECOVERY_INSPECTION_TIMEOUT_MS: u64 = 1_000;
const TREASURY_MAX_WALLET_RECOVERY_INSPECTION_TIMEOUT_MS: u64 = 1_800_000;
const TREASURY_WALLET_RECOVERY_DISCONNECT_TIMEOUT_MS: u64 = 5_000;
const TREASURY_STATE_RECOVERY_DROP_FIELD_SETS: &[&[&str]] = &[
    &["public_snapshot"],
    &["public_snapshot", "active_continuity_alerts"],
    &[
        "public_snapshot",
        "active_continuity_alerts",
        "last_wallet_recovery_report",
    ],
];

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum TreasuryPlaceholderPayoutMode {
    InferenceReady,
    PresenceOnly,
    #[default]
    Disabled,
}

impl TreasuryPlaceholderPayoutMode {
    const fn label(self) -> &'static str {
        match self {
            Self::InferenceReady => "inference_ready",
            Self::PresenceOnly => "presence_only",
            Self::Disabled => "disabled",
        }
    }
}

const fn legacy_treasury_placeholder_payout_mode() -> TreasuryPlaceholderPayoutMode {
    TreasuryPlaceholderPayoutMode::Disabled
}

fn default_accepted_work_policy_snapshot() -> TreasuryAcceptedWorkPolicySnapshot {
    TreasuryAcceptedWorkPolicySnapshot::default()
}

fn default_availability_policy_snapshot() -> TreasuryAvailabilityPolicySnapshot {
    TreasuryAvailabilityPolicySnapshot::default()
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct TreasuryAcceptedWorkPolicySnapshot {
    pub default_payout_sats: u64,
    pub daily_budget_cap_sats: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct TreasuryAvailabilityPolicySnapshot {
    pub payout_sats_per_window: u64,
    pub payout_interval_seconds: u64,
    pub require_sellable: bool,
    pub daily_budget_cap_sats: u64,
    pub max_concurrent_sends: usize,
    pub payout_mode: TreasuryPlaceholderPayoutMode,
    pub dedupe_hosts: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version_floor: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version_floor_started_at_unix_ms: Option<u64>,
    #[serde(default)]
    pub version_gate_active: bool,
}

fn default_treasury_public_snapshot_source() -> String {
    TREASURY_PUBLIC_SNAPSHOT_SOURCE_LOCAL.to_string()
}

fn continuity_alerts_require_persist(
    previous: &[TreasuryContinuityAlert],
    next: &[TreasuryContinuityAlert],
) -> bool {
    if previous.len() != next.len() {
        return true;
    }
    previous.iter().any(|prior| {
        let Some(next_alert) = next
            .iter()
            .find(|candidate| candidate.alert_id == prior.alert_id)
        else {
            return true;
        };
        prior.severity != next_alert.severity
            || prior.reason != next_alert.reason
            || prior.started_at_unix_ms != next_alert.started_at_unix_ms
    })
}

#[derive(Debug, Clone)]
pub struct TreasuryConfig {
    pub enabled: bool,
    pub payout_sats_per_window: u64,
    pub payout_interval_seconds: u64,
    pub require_sellable: bool,
    pub daily_budget_cap_sats: u64,
    pub accepted_work_default_payout_sats: u64,
    pub accepted_work_daily_budget_cap_sats: u64,
    pub placeholder_payout_mode: TreasuryPlaceholderPayoutMode,
    pub dedupe_placeholder_hosts: bool,
    pub availability_max_concurrent_sends: usize,
    pub min_new_accrual_pylon_version: Option<String>,
    pub min_new_accrual_started_at_unix_ms: Option<u64>,
    pub reconciliation_horizon_seconds: u64,
    pub apply_env_policy: bool,
    pub allow_destructive_env_policy_change: bool,
    pub policy_change_reason: Option<String>,
    pub lightning_provider: TreasuryLightningProviderConfig,
    pub state_path: PathBuf,
    pub wallet_mnemonic_path: PathBuf,
    pub wallet_storage_dir: PathBuf,
    pub wallet_network: String,
    pub wallet_api_key_env: Option<String>,
    pub wallet_real_time_sync_enabled: bool,
    pub wallet_status_refresh_seconds: u64,
    pub funding_target_timeout_ms: u64,
    pub wallet_recovery_inspection_timeout_ms: u64,
    pub wallet_recovery_parallel_inspections: bool,
    pub wallet_recovery_scan_payments: bool,
    pub ldk_min_ready_channel_count: u64,
    pub ldk_min_ready_outbound_capacity_sats: u64,
    pub simulated_wallet_enabled: bool,
    pub simulated_wallet_balance_sats: u64,
    pub max_concurrent_sends: usize,
    pub registration_challenge_ttl_seconds: u64,
    pub integration_token: Option<String>,
}

impl TreasuryConfig {
    pub fn from_env() -> Result<Self, String> {
        let enabled = parse_bool_env(ENV_TREASURY_ENABLED, DEFAULT_TREASURY_ENABLED)?;
        let payout_sats_per_window = parse_u64_env(
            ENV_TREASURY_PAYOUT_SATS_PER_WINDOW,
            DEFAULT_TREASURY_PAYOUT_SATS_PER_WINDOW,
        )?;
        let payout_interval_seconds = parse_u64_env(
            ENV_TREASURY_PAYOUT_INTERVAL_SECONDS,
            DEFAULT_TREASURY_PAYOUT_INTERVAL_SECONDS,
        )?;
        if payout_interval_seconds == 0 {
            return Err(format!(
                "{ENV_TREASURY_PAYOUT_INTERVAL_SECONDS} must be greater than zero"
            ));
        }
        let require_sellable = parse_bool_env(
            ENV_TREASURY_REQUIRE_SELLABLE,
            DEFAULT_TREASURY_REQUIRE_SELLABLE,
        )?;
        let daily_budget_cap_sats = parse_u64_env(
            ENV_TREASURY_DAILY_BUDGET_CAP_SATS,
            DEFAULT_TREASURY_DAILY_BUDGET_CAP_SATS,
        )?;
        let accepted_work_default_payout_sats = parse_optional_u64_env(
            ENV_TREASURY_ACCEPTED_WORK_DEFAULT_PAYOUT_SATS,
            DEFAULT_TREASURY_ACCEPTED_WORK_DEFAULT_PAYOUT_SATS,
        )?
        .unwrap_or(payout_sats_per_window);
        let accepted_work_daily_budget_cap_sats = parse_optional_u64_env(
            ENV_TREASURY_ACCEPTED_WORK_DAILY_BUDGET_CAP_SATS,
            DEFAULT_TREASURY_ACCEPTED_WORK_DAILY_BUDGET_CAP_SATS,
        )?
        .unwrap_or(daily_budget_cap_sats);
        let placeholder_payout_mode = parse_placeholder_payout_mode_env(
            ENV_TREASURY_PLACEHOLDER_PAYOUT_MODE,
            TreasuryPlaceholderPayoutMode::Disabled,
        )?;
        let dedupe_placeholder_hosts = parse_bool_env(
            ENV_TREASURY_DEDUPE_PLACEHOLDER_HOSTS,
            DEFAULT_TREASURY_DEDUPE_PLACEHOLDER_HOSTS,
        )?;
        let min_new_accrual_pylon_version =
            read_env_nonempty(ENV_TREASURY_MIN_NEW_ACCRUAL_PYLON_VERSION);
        let min_new_accrual_started_at_unix_ms = parse_optional_u64_env(
            ENV_TREASURY_MIN_NEW_ACCRUAL_STARTED_AT_UNIX_MS,
            DEFAULT_TREASURY_MIN_NEW_ACCRUAL_STARTED_AT_UNIX_MS,
        )?;
        if min_new_accrual_started_at_unix_ms.is_some() && min_new_accrual_pylon_version.is_none() {
            return Err(format!(
                "{ENV_TREASURY_MIN_NEW_ACCRUAL_STARTED_AT_UNIX_MS} requires \
                 {ENV_TREASURY_MIN_NEW_ACCRUAL_PYLON_VERSION}"
            ));
        }
        if let Some(version) = min_new_accrual_pylon_version.as_deref() {
            parse_pylon_client_version(version).map_err(|error| {
                format!("invalid {ENV_TREASURY_MIN_NEW_ACCRUAL_PYLON_VERSION}: {error}")
            })?;
        }
        let wallet_status_refresh_seconds = parse_u64_env(
            ENV_TREASURY_WALLET_STATUS_REFRESH_SECONDS,
            DEFAULT_TREASURY_WALLET_STATUS_REFRESH_SECONDS,
        )?
        .max(1);
        let wallet_real_time_sync_enabled = parse_bool_env(
            ENV_TREASURY_WALLET_REAL_TIME_SYNC_ENABLED,
            DEFAULT_TREASURY_WALLET_REAL_TIME_SYNC_ENABLED,
        )?;
        let funding_target_timeout_ms = parse_u64_env(
            ENV_TREASURY_FUNDING_TARGET_TIMEOUT_MS,
            DEFAULT_TREASURY_FUNDING_TARGET_TIMEOUT_MS,
        )?
        .max(1);
        let wallet_recovery_inspection_timeout_ms = parse_u64_env(
            ENV_TREASURY_WALLET_RECOVERY_INSPECTION_TIMEOUT_MS,
            DEFAULT_TREASURY_WALLET_RECOVERY_INSPECTION_TIMEOUT_MS,
        )?
        .clamp(
            TREASURY_MIN_WALLET_RECOVERY_INSPECTION_TIMEOUT_MS,
            TREASURY_MAX_WALLET_RECOVERY_INSPECTION_TIMEOUT_MS,
        );
        let wallet_recovery_parallel_inspections = parse_bool_env(
            ENV_TREASURY_WALLET_RECOVERY_PARALLEL_INSPECTIONS,
            DEFAULT_TREASURY_WALLET_RECOVERY_PARALLEL_INSPECTIONS,
        )?;
        let wallet_recovery_scan_payments = parse_bool_env(
            ENV_TREASURY_WALLET_RECOVERY_SCAN_PAYMENTS,
            DEFAULT_TREASURY_WALLET_RECOVERY_SCAN_PAYMENTS,
        )?;
        let ldk_min_ready_channel_count = parse_u64_env(
            ENV_TREASURY_LDK_MIN_READY_CHANNEL_COUNT,
            DEFAULT_TREASURY_LDK_MIN_READY_CHANNEL_COUNT,
        )?
        .max(1);
        let ldk_min_ready_outbound_capacity_sats = parse_u64_env(
            ENV_TREASURY_LDK_MIN_READY_OUTBOUND_CAPACITY_SATS,
            DEFAULT_TREASURY_LDK_MIN_READY_OUTBOUND_CAPACITY_SATS,
        )?
        .max(1);
        let simulated_wallet_enabled = parse_bool_env(
            ENV_TREASURY_SIMULATED_WALLET_ENABLED,
            DEFAULT_TREASURY_SIMULATED_WALLET_ENABLED,
        )?;
        let simulated_wallet_balance_sats = parse_u64_env(
            ENV_TREASURY_SIMULATED_WALLET_BALANCE_SATS,
            DEFAULT_TREASURY_SIMULATED_WALLET_BALANCE_SATS,
        )?;
        let max_concurrent_sends = parse_u64_env(
            ENV_TREASURY_MAX_CONCURRENT_SENDS,
            DEFAULT_TREASURY_MAX_CONCURRENT_SENDS as u64,
        )?
        .clamp(1, TREASURY_MAX_CONCURRENT_SENDS_LIMIT as u64)
            as usize;
        let availability_max_concurrent_sends = parse_optional_u64_env(
            ENV_TREASURY_AVAILABILITY_MAX_CONCURRENT_SENDS,
            DEFAULT_TREASURY_AVAILABILITY_MAX_CONCURRENT_SENDS.map(|value| value as u64),
        )?
        .unwrap_or(max_concurrent_sends as u64)
        .clamp(1, TREASURY_MAX_CONCURRENT_SENDS_LIMIT as u64)
            as usize;
        let reconciliation_horizon_seconds = parse_u64_env(
            ENV_TREASURY_RECONCILIATION_HORIZON_SECONDS,
            DEFAULT_TREASURY_RECONCILIATION_HORIZON_SECONDS,
        )?
        .max(1);
        let apply_env_policy = parse_bool_env(
            ENV_TREASURY_POLICY_APPLY_ENV,
            DEFAULT_TREASURY_POLICY_APPLY_ENV,
        )?;
        let allow_destructive_env_policy_change = parse_bool_env(
            ENV_TREASURY_POLICY_ALLOW_DESTRUCTIVE_ENV_CHANGE,
            DEFAULT_TREASURY_POLICY_ALLOW_DESTRUCTIVE_ENV_CHANGE,
        )?;
        let registration_challenge_ttl_seconds = parse_u64_env(
            ENV_TREASURY_REGISTRATION_CHALLENGE_TTL_SECONDS,
            DEFAULT_TREASURY_REGISTRATION_CHALLENGE_TTL_SECONDS,
        )?
        .max(1);
        let lightning_provider = TreasuryLightningProviderConfig::new(
            TreasuryLightningProviderKind::parse(
                read_env_nonempty(ENV_TREASURY_PROVIDER).as_deref(),
            )?,
            LdkTreasuryProviderConfig {
                server_url: read_env_nonempty(ENV_TREASURY_LDK_SERVER_URL),
                api_key_path: read_env_nonempty(ENV_TREASURY_LDK_API_KEY_PATH).map(PathBuf::from),
                tls_cert_path: read_env_nonempty(ENV_TREASURY_LDK_TLS_CERT_PATH).map(PathBuf::from),
                storage_dir: read_path_env(
                    ENV_TREASURY_LDK_STORAGE_DIR,
                    DEFAULT_TREASURY_LDK_STORAGE_DIR,
                ),
                network: LdkNetwork::parse(read_env_nonempty(ENV_TREASURY_LDK_NETWORK).as_deref())?,
                chain_backend: LdkChainBackend::parse(
                    read_env_nonempty(ENV_TREASURY_LDK_CHAIN_BACKEND).as_deref(),
                )?,
            },
        )?;

        Ok(Self {
            enabled,
            payout_sats_per_window,
            payout_interval_seconds,
            require_sellable,
            daily_budget_cap_sats,
            accepted_work_default_payout_sats,
            accepted_work_daily_budget_cap_sats,
            placeholder_payout_mode,
            dedupe_placeholder_hosts,
            availability_max_concurrent_sends,
            min_new_accrual_pylon_version,
            min_new_accrual_started_at_unix_ms,
            reconciliation_horizon_seconds,
            apply_env_policy,
            allow_destructive_env_policy_change,
            policy_change_reason: std::env::var(ENV_TREASURY_POLICY_CHANGE_REASON)
                .ok()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty()),
            lightning_provider,
            state_path: read_path_env(ENV_TREASURY_STATE_PATH, DEFAULT_TREASURY_STATE_PATH),
            wallet_mnemonic_path: read_path_env(
                ENV_TREASURY_WALLET_MNEMONIC_PATH,
                DEFAULT_TREASURY_WALLET_MNEMONIC_PATH,
            ),
            wallet_storage_dir: read_path_env(
                ENV_TREASURY_WALLET_STORAGE_DIR,
                DEFAULT_TREASURY_WALLET_STORAGE_DIR,
            ),
            wallet_network: std::env::var(ENV_TREASURY_WALLET_NETWORK)
                .ok()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| DEFAULT_TREASURY_WALLET_NETWORK.to_string()),
            wallet_api_key_env: std::env::var(ENV_TREASURY_WALLET_API_KEY_ENV)
                .ok()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty()),
            wallet_real_time_sync_enabled,
            wallet_status_refresh_seconds,
            funding_target_timeout_ms,
            wallet_recovery_inspection_timeout_ms,
            wallet_recovery_parallel_inspections,
            wallet_recovery_scan_payments,
            ldk_min_ready_channel_count,
            ldk_min_ready_outbound_capacity_sats,
            simulated_wallet_enabled,
            simulated_wallet_balance_sats,
            max_concurrent_sends,
            registration_challenge_ttl_seconds,
            integration_token: read_env_nonempty(ENV_TREASURY_INTEGRATION_TOKEN),
        })
    }

    pub fn wallet_status_refresh_interval_ms(&self) -> u64 {
        self.wallet_status_refresh_seconds.saturating_mul(1_000)
    }

    pub fn wallet_refresh_timeout_ms(&self) -> u64 {
        TREASURY_MIN_WALLET_REFRESH_TIMEOUT_MS
            .max(self.wallet_status_refresh_seconds.saturating_mul(2_000))
    }

    pub fn wallet_snapshot_stale_after_ms(&self) -> u64 {
        TREASURY_STALE_SNAPSHOT_ALERT_THRESHOLD_MS
            .max(self.wallet_status_refresh_interval_ms().saturating_mul(2))
    }

    pub fn reconciliation_horizon_ms(&self) -> u64 {
        self.reconciliation_horizon_seconds.saturating_mul(1_000)
    }

    pub fn registration_challenge_ttl_ms(&self) -> u64 {
        self.registration_challenge_ttl_seconds
            .saturating_mul(1_000)
    }

    pub fn payout_interval_ms(&self) -> u64 {
        self.payout_interval_seconds.saturating_mul(1_000)
    }

    pub fn dispatch_result_timeout_ms(&self, payout_interval_ms: u64) -> u64 {
        let _ = payout_interval_ms;
        let _ = self.wallet_status_refresh_seconds;
        TREASURY_DISPATCH_RESULT_TIMEOUT_MS
    }

    pub fn max_concurrent_send_operations(&self, plan_count: usize) -> usize {
        plan_count.min(self.max_concurrent_sends).max(1)
    }

    pub fn max_concurrent_send_operations_for_class(
        &self,
        plan_count: usize,
        payout_class: TreasuryPayoutClass,
    ) -> usize {
        let configured = match payout_class {
            TreasuryPayoutClass::PlaceholderLiveness => plan_count
                .min(self.availability_max_concurrent_sends)
                .max(1),
            TreasuryPayoutClass::AcceptedWork | TreasuryPayoutClass::BetaBonus => {
                self.max_concurrent_send_operations(plan_count)
            }
        };
        match payout_class {
            TreasuryPayoutClass::AcceptedWork => {
                configured.min(TREASURY_MAX_CONCURRENT_ACCEPTED_WORK_SENDS)
            }
            TreasuryPayoutClass::PlaceholderLiveness | TreasuryPayoutClass::BetaBonus => configured,
        }
        .max(1)
    }

    pub fn accepted_work_policy_snapshot(&self) -> TreasuryAcceptedWorkPolicySnapshot {
        TreasuryAcceptedWorkPolicySnapshot {
            default_payout_sats: self.accepted_work_default_payout_sats,
            daily_budget_cap_sats: self.accepted_work_daily_budget_cap_sats,
        }
    }

    pub fn availability_policy_snapshot(&self) -> TreasuryAvailabilityPolicySnapshot {
        TreasuryAvailabilityPolicySnapshot {
            payout_sats_per_window: self.payout_sats_per_window,
            payout_interval_seconds: self.payout_interval_seconds,
            require_sellable: self.require_sellable,
            daily_budget_cap_sats: self.daily_budget_cap_sats,
            max_concurrent_sends: self.availability_max_concurrent_sends,
            payout_mode: self.placeholder_payout_mode,
            dedupe_hosts: self.dedupe_placeholder_hosts,
            version_floor: self.min_new_accrual_pylon_version.clone(),
            version_floor_started_at_unix_ms: self.min_new_accrual_started_at_unix_ms,
            version_gate_active: self.min_new_accrual_pylon_version.is_some()
                && self.min_new_accrual_started_at_unix_ms.is_some(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProviderPayoutTargetChallengeRequest {
    pub nostr_pubkey_hex: String,
    pub session_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProviderPayoutTargetChallengeResponse {
    pub authority: String,
    pub nostr_pubkey_hex: String,
    pub session_id: String,
    pub challenge: String,
    pub issued_at_unix_ms: u64,
    pub expires_at_unix_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProviderPayoutTargetRegistrationRequest {
    pub nostr_pubkey_hex: String,
    pub session_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payment_target_kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payment_target: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub payment_target_capabilities: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pylon_payment_target_version: Option<String>,
    pub challenge: String,
    pub challenge_signature_hex: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProviderPayoutTargetRegistrationResponse {
    pub authority: String,
    pub nostr_pubkey_hex: String,
    pub session_id: String,
    pub payment_target_kind: String,
    pub payment_target: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub payment_target_capabilities: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pylon_payment_target_version: Option<String>,
    pub registered_at_unix_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TreasuryFundingTargetRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub amount_sats: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expiry_seconds: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct TreasuryFundingTargetPhaseTimings {
    pub request_received_at_unix_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operation_row_created_at_unix_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ldk_rpc_started_at_unix_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ldk_rpc_completed_at_unix_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub invoice_returned_at_unix_ms: Option<u64>,
}

impl TreasuryFundingTargetPhaseTimings {
    fn insert_rail_metadata(&self, rail_metadata: &mut BTreeMap<String, String>) {
        if self.request_received_at_unix_ms > 0 {
            rail_metadata.insert(
                "phase_request_received_at_unix_ms".to_string(),
                self.request_received_at_unix_ms.to_string(),
            );
        }
        if let Some(value) = self.operation_row_created_at_unix_ms {
            rail_metadata.insert(
                "phase_operation_row_created_at_unix_ms".to_string(),
                value.to_string(),
            );
        }
        if let Some(value) = self.ldk_rpc_started_at_unix_ms {
            rail_metadata.insert(
                "phase_ldk_rpc_started_at_unix_ms".to_string(),
                value.to_string(),
            );
        }
        if let Some(value) = self.ldk_rpc_completed_at_unix_ms {
            rail_metadata.insert(
                "phase_ldk_rpc_completed_at_unix_ms".to_string(),
                value.to_string(),
            );
        }
        if let Some(value) = self.invoice_returned_at_unix_ms {
            rail_metadata.insert(
                "phase_invoice_returned_at_unix_ms".to_string(),
                value.to_string(),
            );
        }
        if let Some(duration_ms) = self.ldk_rpc_duration_ms() {
            rail_metadata.insert(
                "phase_ldk_rpc_duration_ms".to_string(),
                duration_ms.to_string(),
            );
        }
        if let Some(duration_ms) = self.total_duration_ms() {
            rail_metadata.insert(
                "phase_total_duration_ms".to_string(),
                duration_ms.to_string(),
            );
        }
    }

    fn ldk_rpc_duration_ms(&self) -> Option<u64> {
        Some(
            self.ldk_rpc_completed_at_unix_ms?
                .saturating_sub(self.ldk_rpc_started_at_unix_ms?),
        )
    }

    fn total_duration_ms(&self) -> Option<u64> {
        let end = self
            .invoice_returned_at_unix_ms
            .or(self.operation_row_created_at_unix_ms)?;
        Some(end.saturating_sub(self.request_received_at_unix_ms))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TreasuryFundingTargetResponse {
    pub authority: String,
    pub wallet_runtime_status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wallet_runtime_detail: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wallet_hydration_mode: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wallet_payment_scan_mode: Option<String>,
    pub wallet_balance_sats: u64,
    pub wallet_balance_updated_at_unix_ms: u64,
    #[serde(rename = "provider_target")]
    pub provider_target: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub bitcoin_address: String,
    #[serde(
        default,
        rename = "provider_invoice",
        skip_serializing_if = "Option::is_none"
    )]
    pub provider_invoice: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bolt11_invoice: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_payment_id_hash: Option<String>,
    #[serde(default)]
    pub phase_timings: TreasuryFundingTargetPhaseTimings,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct TreasuryWalletPaymentAggregate {
    pub total_payments: u64,
    pub completed_receive_count: u64,
    pub completed_receive_total_sats: u64,
    pub completed_send_count: u64,
    pub completed_send_total_sats: u64,
    pub max_completed_send_sats: u64,
    pub pending_send_count: u64,
    pub pending_send_total_sats: u64,
    pub failed_send_count: u64,
    pub failed_send_total_sats: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct TreasuryWalletUnclaimedDepositAggregate {
    pub count: u64,
    pub total_sats: u64,
    pub with_claim_error_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct TreasuryWalletInspection {
    pub wallet_identity_pubkey: String,
    pub inspected_storage_dir: String,
    #[serde(default)]
    pub inspection_timeout_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime_status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime_detail: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub balance_sats: Option<u64>,
    #[serde(default)]
    pub payment_totals: TreasuryWalletPaymentAggregate,
    #[serde(default)]
    pub unclaimed_deposit_totals: TreasuryWalletUnclaimedDepositAggregate,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct TreasuryWalletRecoveryComparison {
    pub wallet_identity_pubkey_match: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rebuilt_minus_current_balance_sats: Option<i64>,
    pub current_zero_with_receive_history: bool,
    pub major_divergence_detected: bool,
    pub validation_passed: bool,
    pub recommended_action: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct TreasuryWalletRecoveryReportSummary {
    pub generated_at_unix_ms: u64,
    pub report_path: String,
    pub current_storage_dir: String,
    pub rebuilt_storage_dir: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_balance_sats: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rebuilt_balance_sats: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rebuilt_minus_current_balance_sats: Option<i64>,
    pub major_divergence_detected: bool,
    pub validation_passed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TreasuryWalletRecoveryReport {
    pub authority: String,
    pub generated_at_unix_ms: u64,
    pub source_wallet_storage_dir: String,
    pub backup_root_dir: String,
    pub current_storage_backup_dir: String,
    pub rebuilt_storage_dir: String,
    pub report_path: String,
    pub mnemonic_backup_path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state_backup_path: Option<String>,
    pub current_storage: TreasuryWalletInspection,
    pub rebuilt_storage: TreasuryWalletInspection,
    pub comparison: TreasuryWalletRecoveryComparison,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cutover_active_storage_dir: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cutover_rollback_storage_dir: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cutover_completed_at_unix_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TreasuryWalletRecoveryCutoverResponse {
    pub authority: String,
    pub report_path: String,
    pub active_storage_dir: String,
    pub rollback_storage_dir: String,
    pub wallet_storage_runtime_mode: String,
    pub cutover_completed_at_unix_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct TreasuryReasonMetric {
    pub reason: String,
    pub count: u64,
    pub total_sats: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TreasuryContinuityAlert {
    pub alert_id: String,
    pub severity: String,
    pub reason: String,
    pub started_at_unix_ms: u64,
    pub observed_at_unix_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TreasuryDegradedState {
    pub code: String,
    pub severity: String,
    pub public_reason: String,
    pub operator_action: String,
    pub source: String,
    pub observed_at_unix_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub started_at_unix_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metric_value: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub threshold: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
struct TreasuryContinuitySignalSnapshot {
    availability_online_identities_now: u64,
    availability_online_host_clusters_now: u64,
    availability_stipend_eligible_beneficiaries_now: u64,
    eligible_online_payout_targets: u64,
    sellable_pylons_online_now: u64,
    inference_ready_online_payout_targets: u64,
    duplicate_host_placeholder_blocked_online_targets: u64,
    duplicate_payout_target_placeholder_blocked_online_targets: u64,
    missing_payout_target_blocked_online_targets: u64,
    version_floor_blocked_beneficiaries_now: u64,
    readiness_blocked_online_targets: u64,
    latest_eligible_window_started_at_unix_ms: Option<u64>,
    last_dispatch_at_unix_ms: Option<u64>,
    last_confirmed_at_unix_ms: Option<u64>,
    skip_reason_metrics_24h: Vec<TreasuryReasonMetric>,
    fail_reason_metrics_24h: Vec<TreasuryReasonMetric>,
    active_alerts: Vec<TreasuryContinuityAlert>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TreasuryRuntimePolicy {
    pub schema_version: u32,
    pub treasury_enabled: bool,
    pub payout_sats_per_window: u64,
    pub payout_interval_seconds: u64,
    pub require_sellable: bool,
    pub daily_budget_cap_sats: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accepted_work_default_payout_sats: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accepted_work_daily_budget_cap_sats: Option<u64>,
    #[serde(default = "legacy_treasury_placeholder_payout_mode")]
    pub placeholder_payout_mode: TreasuryPlaceholderPayoutMode,
    #[serde(default)]
    pub dedupe_placeholder_hosts: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub availability_max_concurrent_sends: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_new_accrual_pylon_version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_new_accrual_started_at_unix_ms: Option<u64>,
    pub checksum: String,
}

impl TreasuryRuntimePolicy {
    pub fn from_config(config: &TreasuryConfig) -> Self {
        Self::new(
            config.enabled,
            config.payout_sats_per_window,
            config.payout_interval_seconds,
            config.require_sellable,
            config.daily_budget_cap_sats,
            Some(config.accepted_work_default_payout_sats),
            Some(config.accepted_work_daily_budget_cap_sats),
            config.placeholder_payout_mode,
            config.dedupe_placeholder_hosts,
            Some(config.availability_max_concurrent_sends),
            config.min_new_accrual_pylon_version.clone(),
            config.min_new_accrual_started_at_unix_ms,
        )
    }

    pub fn new(
        treasury_enabled: bool,
        payout_sats_per_window: u64,
        payout_interval_seconds: u64,
        require_sellable: bool,
        daily_budget_cap_sats: u64,
        accepted_work_default_payout_sats: Option<u64>,
        accepted_work_daily_budget_cap_sats: Option<u64>,
        placeholder_payout_mode: TreasuryPlaceholderPayoutMode,
        dedupe_placeholder_hosts: bool,
        availability_max_concurrent_sends: Option<usize>,
        min_new_accrual_pylon_version: Option<String>,
        min_new_accrual_started_at_unix_ms: Option<u64>,
    ) -> Self {
        let payload = TreasuryRuntimePolicyChecksumPayload {
            schema_version: TREASURY_POLICY_SCHEMA_VERSION,
            treasury_enabled,
            payout_sats_per_window,
            payout_interval_seconds,
            require_sellable,
            daily_budget_cap_sats,
            accepted_work_default_payout_sats,
            accepted_work_daily_budget_cap_sats,
            placeholder_payout_mode,
            dedupe_placeholder_hosts,
            availability_max_concurrent_sends,
            min_new_accrual_pylon_version: min_new_accrual_pylon_version.clone(),
            min_new_accrual_started_at_unix_ms,
        };
        let checksum = format!(
            "sha256:{}",
            hex::encode(Sha256::digest(
                serde_json::to_vec(&payload).expect("treasury policy checksum payload")
            ))
        );
        Self {
            schema_version: TREASURY_POLICY_SCHEMA_VERSION,
            treasury_enabled,
            payout_sats_per_window,
            payout_interval_seconds,
            require_sellable,
            daily_budget_cap_sats,
            accepted_work_default_payout_sats,
            accepted_work_daily_budget_cap_sats,
            placeholder_payout_mode,
            dedupe_placeholder_hosts,
            availability_max_concurrent_sends,
            min_new_accrual_pylon_version,
            min_new_accrual_started_at_unix_ms,
            checksum,
        }
    }

    pub fn payout_interval_ms(&self) -> u64 {
        self.payout_interval_seconds.saturating_mul(1_000)
    }

    pub fn accepted_work_default_payout_sats(&self) -> u64 {
        self.accepted_work_default_payout_sats
            .unwrap_or(self.payout_sats_per_window)
    }

    pub fn accepted_work_daily_budget_cap_sats(&self) -> u64 {
        self.accepted_work_daily_budget_cap_sats
            .unwrap_or(self.daily_budget_cap_sats)
    }

    pub fn availability_max_concurrent_sends(&self, config: &TreasuryConfig) -> usize {
        self.availability_max_concurrent_sends
            .unwrap_or(config.availability_max_concurrent_sends)
    }

    pub fn accepted_work_policy_snapshot(&self) -> TreasuryAcceptedWorkPolicySnapshot {
        TreasuryAcceptedWorkPolicySnapshot {
            default_payout_sats: self.accepted_work_default_payout_sats(),
            daily_budget_cap_sats: self.accepted_work_daily_budget_cap_sats(),
        }
    }

    pub fn availability_policy_snapshot(
        &self,
        config: &TreasuryConfig,
    ) -> TreasuryAvailabilityPolicySnapshot {
        TreasuryAvailabilityPolicySnapshot {
            payout_sats_per_window: self.payout_sats_per_window,
            payout_interval_seconds: self.payout_interval_seconds,
            require_sellable: self.require_sellable,
            daily_budget_cap_sats: self.daily_budget_cap_sats,
            max_concurrent_sends: self.availability_max_concurrent_sends(config),
            payout_mode: self.placeholder_payout_mode,
            dedupe_hosts: self.dedupe_placeholder_hosts,
            version_floor: self.min_new_accrual_pylon_version.clone(),
            version_floor_started_at_unix_ms: self.min_new_accrual_started_at_unix_ms,
            version_gate_active: self.new_accrual_version_gate_active(),
        }
    }

    pub fn with_resolved_legacy_defaults(mut self, config: &TreasuryConfig) -> Self {
        if self.accepted_work_default_payout_sats.is_none() {
            self.accepted_work_default_payout_sats = Some(self.payout_sats_per_window);
        }
        if self.accepted_work_daily_budget_cap_sats.is_none() {
            self.accepted_work_daily_budget_cap_sats = Some(self.daily_budget_cap_sats);
        }
        if self.availability_max_concurrent_sends.is_none() {
            self.availability_max_concurrent_sends = Some(config.availability_max_concurrent_sends);
        }
        self
    }

    fn daily_budget_cap_sats_for_class(&self, payout_class: TreasuryPayoutClass) -> u64 {
        match payout_class {
            TreasuryPayoutClass::AcceptedWork => self.accepted_work_daily_budget_cap_sats(),
            TreasuryPayoutClass::PlaceholderLiveness | TreasuryPayoutClass::BetaBonus => {
                self.daily_budget_cap_sats
            }
        }
    }

    fn new_accrual_version_gate_active(&self) -> bool {
        self.min_new_accrual_pylon_version.is_some()
            && self.min_new_accrual_started_at_unix_ms.is_some()
    }

    fn new_accrual_version_gate_applies_to_window(&self, window_started_at_unix_ms: u64) -> bool {
        self.min_new_accrual_started_at_unix_ms
            .is_some_and(|cutoff| {
                self.min_new_accrual_pylon_version.is_some() && window_started_at_unix_ms >= cutoff
            })
    }

    fn new_accrual_version_gate_verdict(
        &self,
        client_version: Option<&str>,
        window_started_at_unix_ms: u64,
    ) -> NewAccrualVersionGateVerdict {
        if !self.new_accrual_version_gate_applies_to_window(window_started_at_unix_ms) {
            return NewAccrualVersionGateVerdict::Allowed;
        }
        let Some(required_version) = self.min_new_accrual_pylon_version.as_deref() else {
            return NewAccrualVersionGateVerdict::Allowed;
        };
        let Ok(required_version) = parse_pylon_client_version(required_version) else {
            return NewAccrualVersionGateVerdict::InvalidPolicy;
        };
        let Some(client_version) = client_version else {
            return NewAccrualVersionGateVerdict::MissingClientVersion;
        };
        let Ok(client_version) = parse_pylon_client_version(client_version) else {
            return NewAccrualVersionGateVerdict::InvalidClientVersion;
        };
        if client_version >= required_version {
            NewAccrualVersionGateVerdict::Allowed
        } else {
            NewAccrualVersionGateVerdict::BelowFloor
        }
    }

    fn availability_stipend_base_skip_reason(
        &self,
        identity: &OnlinePylonIdentity,
    ) -> Option<String> {
        match self.placeholder_payout_mode {
            TreasuryPlaceholderPayoutMode::Disabled => {
                return Some("placeholder_payouts_disabled".to_string());
            }
            TreasuryPlaceholderPayoutMode::InferenceReady if !identity.inference_ready => {
                return Some("placeholder_requires_inference_ready".to_string());
            }
            TreasuryPlaceholderPayoutMode::InferenceReady
            | TreasuryPlaceholderPayoutMode::PresenceOnly => {}
        }

        if !identity.availability_stipend_eligible {
            return Some(
                identity
                    .availability_stipend_gate_reason
                    .clone()
                    .unwrap_or_else(|| "availability_stipend_not_eligible".to_string()),
            );
        }

        None
    }

    fn placeholder_payout_classification(&self) -> TreasuryPayoutClassification {
        TreasuryPayoutClassification {
            payout_class: TreasuryPayoutClass::PlaceholderLiveness,
            payout_basis: Some(self.placeholder_payout_mode.label().to_string()),
            ..TreasuryPayoutClassification::default()
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct TreasuryRuntimePolicyChecksumPayload {
    schema_version: u32,
    treasury_enabled: bool,
    payout_sats_per_window: u64,
    payout_interval_seconds: u64,
    require_sellable: bool,
    daily_budget_cap_sats: u64,
    accepted_work_default_payout_sats: Option<u64>,
    accepted_work_daily_budget_cap_sats: Option<u64>,
    placeholder_payout_mode: TreasuryPlaceholderPayoutMode,
    dedupe_placeholder_hosts: bool,
    availability_max_concurrent_sends: Option<usize>,
    min_new_accrual_pylon_version: Option<String>,
    min_new_accrual_started_at_unix_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TreasuryPolicyChangeRecord {
    pub change_id: String,
    pub applied_at_unix_ms: u64,
    pub source: String,
    pub reason: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checksum_before: Option<String>,
    pub checksum_after: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub changed_fields: Vec<String>,
    pub destructive: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TreasuryStatusResponse {
    pub authority: String,
    #[serde(default)]
    pub active_treasury_provider: String,
    #[serde(default)]
    pub active_treasury_rail: String,
    #[serde(default)]
    pub ldk_network: String,
    #[serde(default)]
    pub ldk_chain_backend: String,
    #[serde(default)]
    pub ldk_server_configured: bool,
    #[serde(default)]
    pub ldk_readiness: TreasuryLdkReadinessSnapshot,
    pub treasury_enabled: bool,
    pub payout_sats_per_window: u64,
    pub payout_interval_seconds: u64,
    pub require_sellable: bool,
    pub daily_budget_cap_sats: u64,
    #[serde(default = "default_accepted_work_policy_snapshot")]
    pub accepted_work_policy: TreasuryAcceptedWorkPolicySnapshot,
    #[serde(default = "default_availability_policy_snapshot")]
    pub availability_policy: TreasuryAvailabilityPolicySnapshot,
    pub placeholder_payout_mode: TreasuryPlaceholderPayoutMode,
    pub dedupe_placeholder_hosts: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_new_accrual_pylon_version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_new_accrual_started_at_unix_ms: Option<u64>,
    #[serde(default)]
    pub min_new_accrual_version_gate_active: bool,
    pub registered_payout_identities: u64,
    #[serde(default)]
    pub ldk_payout_target_identities: u64,
    #[serde(default)]
    pub pylon_v0_2_registration_required_identities: u64,
    pub wallet_balance_sats: u64,
    #[serde(default)]
    pub wallet_total_onchain_balance_sats: u64,
    #[serde(default)]
    pub wallet_spendable_onchain_balance_sats: u64,
    #[serde(default)]
    pub wallet_lightning_balance_sats: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wallet_balance_updated_at_unix_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_wallet_sync_at_unix_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_wallet_refresh_attempt_at_unix_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wallet_runtime_status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wallet_last_error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wallet_hydration_mode: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wallet_payment_scan_mode: Option<String>,
    #[serde(default = "default_wallet_storage_runtime_mode")]
    pub wallet_storage_runtime_mode: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wallet_storage_report_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wallet_storage_rollback_dir: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wallet_storage_cutover_at_unix_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wallet_recovery_last_report_generated_at_unix_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wallet_recovery_last_report_validation_passed: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payout_loop_runtime_status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payout_loop_last_error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_payout_reconciliation_at_unix_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payout_loop_last_started_at_unix_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payout_loop_last_completed_at_unix_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub public_snapshot_generated_at_unix_ms: Option<u64>,
    #[serde(default = "default_treasury_public_snapshot_source")]
    pub public_snapshot_source: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub public_snapshot_mode: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub public_snapshot_health_status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snapshot_age_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wallet_sync_lag_ms: Option<u64>,
    #[serde(default)]
    pub backlog_total: u64,
    #[serde(default)]
    pub backlog_retryable: u64,
    #[serde(default)]
    pub pending_confirmation_count: u64,
    #[serde(default)]
    pub tracked_payment_backlog_count: u64,
    #[serde(default)]
    pub legacy_availability_confirmation_attention_count: u64,
    #[serde(default)]
    pub availability_online_identities_now: u64,
    #[serde(default)]
    pub availability_online_host_clusters_now: u64,
    #[serde(default)]
    pub availability_stipend_eligible_beneficiaries_now: u64,
    #[serde(default)]
    pub eligible_online_payout_targets: u64,
    #[serde(default)]
    pub sellable_pylons_online_now: u64,
    #[serde(default)]
    pub inference_ready_online_payout_targets: u64,
    #[serde(default)]
    pub duplicate_host_placeholder_blocked_online_targets: u64,
    #[serde(default)]
    pub duplicate_host_blocked_beneficiaries_now: u64,
    #[serde(default)]
    pub duplicate_payout_target_blocked_beneficiaries_now: u64,
    #[serde(default)]
    pub missing_payout_target_blocked_beneficiaries_now: u64,
    #[serde(default)]
    pub version_floor_blocked_beneficiaries_now: u64,
    #[serde(default)]
    pub readiness_blocked_beneficiaries_now: u64,
    #[serde(default)]
    pub min_new_accrual_version_blocked_online_targets: u64,
    #[serde(default)]
    pub min_new_accrual_unknown_version_online_targets: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_eligible_window_started_at_unix_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_dispatch_at_unix_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_confirmed_payout_at_unix_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub eligible_window_lag_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dispatch_lag_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confirm_lag_ms: Option<u64>,
    #[serde(default)]
    pub payout_loop_health: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub degraded_reason: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub degraded_states: Vec<TreasuryDegradedState>,
    pub policy_schema_version: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub policy_checksum: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub policy_runtime_status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub policy_last_error: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub recent_policy_changes: Vec<TreasuryPolicyChangeRecord>,
    pub payout_sats_paid_total: u64,
    pub payout_sats_paid_24h: u64,
    #[serde(default)]
    pub payout_sats_in_flight_total: u64,
    #[serde(default)]
    pub payout_sats_in_flight_24h: u64,
    #[serde(default)]
    pub accepted_work_payout_sats_paid_total: u64,
    #[serde(default)]
    pub accepted_work_payout_sats_paid_24h: u64,
    #[serde(default)]
    pub accepted_work_payout_sats_in_flight_total: u64,
    #[serde(default)]
    pub accepted_work_payout_sats_in_flight_24h: u64,
    #[serde(default)]
    pub availability_stipend_payout_sats_paid_total: u64,
    #[serde(default)]
    pub availability_stipend_payout_sats_paid_24h: u64,
    #[serde(default)]
    pub availability_stipend_payout_sats_in_flight_total: u64,
    #[serde(default)]
    pub availability_stipend_payout_sats_in_flight_24h: u64,
    #[serde(default)]
    pub placeholder_payout_sats_paid_total: u64,
    #[serde(default)]
    pub placeholder_payout_sats_paid_24h: u64,
    #[serde(default)]
    pub placeholder_payout_sats_in_flight_total: u64,
    #[serde(default)]
    pub placeholder_payout_sats_in_flight_24h: u64,
    #[serde(default)]
    pub beta_bonus_payout_sats_paid_total: u64,
    #[serde(default)]
    pub beta_bonus_payout_sats_paid_24h: u64,
    #[serde(default)]
    pub beta_bonus_payout_sats_in_flight_total: u64,
    #[serde(default)]
    pub beta_bonus_payout_sats_in_flight_24h: u64,
    #[serde(default)]
    pub weak_device_accepted_work_payout_sats_paid_total: u64,
    #[serde(default)]
    pub weak_device_accepted_work_payout_sats_paid_24h: u64,
    #[serde(default)]
    pub strong_lane_accepted_work_payout_sats_paid_total: u64,
    #[serde(default)]
    pub strong_lane_accepted_work_payout_sats_paid_24h: u64,
    pub payouts_dispatched_24h: u64,
    pub payouts_confirmed_24h: u64,
    pub payouts_failed_24h: u64,
    pub payouts_skipped_24h: u64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub skip_reason_metrics_24h: Vec<TreasuryReasonMetric>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fail_reason_metrics_24h: Vec<TreasuryReasonMetric>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub active_continuity_alerts: Vec<TreasuryContinuityAlert>,
    #[serde(default)]
    pub training_payout_ledger_summary: TreasuryTrainingPayoutLedgerSummary,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub payout_target_identities: Vec<TreasuryPayoutTargetIdentityStatus>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub recent_training_payouts: Vec<TreasuryTrainingPayoutLedgerEntry>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub recent_treasury_operations: Vec<TreasuryOperationRecord>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub availability_beneficiary_debug_rows: Vec<TreasuryAvailabilityBeneficiaryDebugRow>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub legacy_availability_confirmation_attention_rows:
        Vec<TreasuryLegacyAvailabilityAttentionRow>,
}

impl TreasuryStatusResponse {
    pub fn public_api_view(mut self) -> Self {
        self.payout_target_identities.clear();
        self.recent_training_payouts.clear();
        self.recent_treasury_operations.clear();
        self.availability_beneficiary_debug_rows.clear();
        self.legacy_availability_confirmation_attention_rows.clear();
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct TreasuryPayoutTargetIdentityStatus {
    pub nostr_pubkey_hex: String,
    pub source_session_id: String,
    #[serde(default)]
    pub payment_target_kind: String,
    #[serde(default)]
    pub payment_target: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub payment_target_capabilities: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pylon_payment_target_version: Option<String>,
    #[serde(default)]
    pub ldk_compatible: bool,
    pub provider_target: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bitcoin_address: Option<String>,
    pub registered_at_unix_ms: u64,
    pub last_verified_at_unix_ms: u64,
    pub payout_record_count: u64,
    pub confirmed_payout_count: u64,
    pub confirmed_payout_sats: u64,
    pub confirmed_accepted_work_payout_sats: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_payout_at_unix_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TreasuryTrainingPayoutLedgerEntry {
    pub payout_key: String,
    pub nostr_pubkey_hex: String,
    pub payout_target: String,
    pub amount_sats: u64,
    pub status: String,
    pub reconciliation_status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payment_id: Option<String>,
    pub window_started_at_unix_ms: u64,
    pub window_ends_at_unix_ms: u64,
    pub created_at_unix_ms: u64,
    pub updated_at_unix_ms: u64,
    pub sellable_at_window_open: bool,
    #[serde(default)]
    pub classification: TreasuryPayoutClassification,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TreasuryLegacyAvailabilityAttentionRow {
    pub payout_key: String,
    pub nostr_pubkey_hex: String,
    pub payout_target: String,
    pub status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payment_id: Option<String>,
    pub window_started_at_unix_ms: u64,
    pub updated_at_unix_ms: u64,
    pub age_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TreasuryTrainingPayoutLedgerSummary {
    pub reconciliation_status: String,
    pub payout_record_count: u64,
    pub pending_payout_count: u64,
    pub confirmed_payout_count: u64,
    pub failed_payout_count: u64,
    pub skipped_payout_count: u64,
    pub attention_payout_count: u64,
    #[serde(default)]
    pub current_ldk_failed_payout_count: u64,
    #[serde(default)]
    pub current_ldk_attention_payout_count: u64,
    #[serde(default)]
    pub retired_historical_payout_count: u64,
    #[serde(default)]
    pub retired_historical_accepted_work_payout_count: u64,
    #[serde(default)]
    pub retired_historical_payout_sats: u64,
    pub missing_payout_target_count: u64,
    pub accepted_work_pending_payout_count: u64,
    pub accepted_work_confirmed_payout_count: u64,
    pub accepted_work_attention_payout_count: u64,
}

impl Default for TreasuryTrainingPayoutLedgerSummary {
    fn default() -> Self {
        Self {
            reconciliation_status: "clean".to_string(),
            payout_record_count: 0,
            pending_payout_count: 0,
            confirmed_payout_count: 0,
            failed_payout_count: 0,
            skipped_payout_count: 0,
            attention_payout_count: 0,
            current_ldk_failed_payout_count: 0,
            current_ldk_attention_payout_count: 0,
            retired_historical_payout_count: 0,
            retired_historical_accepted_work_payout_count: 0,
            retired_historical_payout_sats: 0,
            missing_payout_target_count: 0,
            accepted_work_pending_payout_count: 0,
            accepted_work_confirmed_payout_count: 0,
            accepted_work_attention_payout_count: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct TreasuryPayoutLedgerCleanupReport {
    pub authority: String,
    pub generated_at_unix_ms: u64,
    pub state_path: String,
    pub applied: bool,
    pub changed: bool,
    pub before_summary: TreasuryTrainingPayoutLedgerSummary,
    pub after_summary: TreasuryTrainingPayoutLedgerSummary,
    #[serde(default)]
    pub before_disposition_counts: BTreeMap<String, u64>,
    #[serde(default)]
    pub after_disposition_counts: BTreeMap<String, u64>,
    #[serde(default)]
    pub before_reason_counts: BTreeMap<String, u64>,
    #[serde(default)]
    pub after_reason_counts: BTreeMap<String, u64>,
    #[serde(default)]
    pub records_retired: Vec<TreasuryPayoutLedgerCleanupRetiredRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TreasuryPayoutLedgerCleanupRetiredRecord {
    pub payout_key: String,
    pub previous_status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_reason: Option<String>,
    pub payout_rail: String,
    pub payout_class: String,
    pub amount_sats: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TreasuryPublicSnapshot {
    pub generated_at_unix_ms: u64,
    #[serde(default = "default_treasury_public_snapshot_source")]
    pub source: String,
    pub treasury_enabled: bool,
    pub payout_sats_per_window: u64,
    pub payout_interval_seconds: u64,
    pub require_sellable: bool,
    pub daily_budget_cap_sats: u64,
    #[serde(default = "default_accepted_work_policy_snapshot")]
    pub accepted_work_policy: TreasuryAcceptedWorkPolicySnapshot,
    #[serde(default = "default_availability_policy_snapshot")]
    pub availability_policy: TreasuryAvailabilityPolicySnapshot,
    #[serde(default = "legacy_treasury_placeholder_payout_mode")]
    pub placeholder_payout_mode: TreasuryPlaceholderPayoutMode,
    #[serde(default)]
    pub dedupe_placeholder_hosts: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_new_accrual_pylon_version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_new_accrual_started_at_unix_ms: Option<u64>,
    #[serde(default)]
    pub min_new_accrual_version_gate_active: bool,
    pub registered_payout_identities: u64,
    #[serde(default)]
    pub ldk_payout_target_identities: u64,
    #[serde(default)]
    pub pylon_v0_2_registration_required_identities: u64,
    pub wallet_balance_sats: u64,
    #[serde(default)]
    pub wallet_total_onchain_balance_sats: u64,
    #[serde(default)]
    pub wallet_spendable_onchain_balance_sats: u64,
    #[serde(default)]
    pub wallet_lightning_balance_sats: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wallet_balance_updated_at_unix_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_wallet_sync_at_unix_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_wallet_refresh_attempt_at_unix_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wallet_runtime_status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wallet_last_error: Option<String>,
    #[serde(default = "default_wallet_storage_runtime_mode")]
    pub wallet_storage_runtime_mode: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payout_loop_runtime_status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payout_loop_last_error: Option<String>,
    pub payout_loop_health: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_payout_reconciliation_at_unix_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payout_loop_last_started_at_unix_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payout_loop_last_completed_at_unix_ms: Option<u64>,
    pub payout_sats_paid_total: u64,
    pub payout_sats_paid_24h: u64,
    #[serde(default)]
    pub payout_sats_in_flight_total: u64,
    #[serde(default)]
    pub payout_sats_in_flight_24h: u64,
    #[serde(default)]
    pub accepted_work_payout_sats_paid_total: u64,
    #[serde(default)]
    pub accepted_work_payout_sats_paid_24h: u64,
    #[serde(default)]
    pub accepted_work_payout_sats_in_flight_total: u64,
    #[serde(default)]
    pub accepted_work_payout_sats_in_flight_24h: u64,
    #[serde(default)]
    pub availability_stipend_payout_sats_paid_total: u64,
    #[serde(default)]
    pub availability_stipend_payout_sats_paid_24h: u64,
    #[serde(default)]
    pub availability_stipend_payout_sats_in_flight_total: u64,
    #[serde(default)]
    pub availability_stipend_payout_sats_in_flight_24h: u64,
    #[serde(default)]
    pub placeholder_payout_sats_paid_total: u64,
    #[serde(default)]
    pub placeholder_payout_sats_paid_24h: u64,
    #[serde(default)]
    pub placeholder_payout_sats_in_flight_total: u64,
    #[serde(default)]
    pub placeholder_payout_sats_in_flight_24h: u64,
    #[serde(default)]
    pub beta_bonus_payout_sats_paid_total: u64,
    #[serde(default)]
    pub beta_bonus_payout_sats_paid_24h: u64,
    #[serde(default)]
    pub beta_bonus_payout_sats_in_flight_total: u64,
    #[serde(default)]
    pub beta_bonus_payout_sats_in_flight_24h: u64,
    #[serde(default)]
    pub weak_device_accepted_work_payout_sats_paid_total: u64,
    #[serde(default)]
    pub weak_device_accepted_work_payout_sats_paid_24h: u64,
    #[serde(default)]
    pub strong_lane_accepted_work_payout_sats_paid_total: u64,
    #[serde(default)]
    pub strong_lane_accepted_work_payout_sats_paid_24h: u64,
    pub payouts_dispatched_24h: u64,
    pub payouts_confirmed_24h: u64,
    pub payouts_failed_24h: u64,
    pub payouts_skipped_24h: u64,
    #[serde(default)]
    pub backlog_total: u64,
    #[serde(default)]
    pub backlog_retryable: u64,
    #[serde(default)]
    pub pending_confirmation_count: u64,
    #[serde(default)]
    pub tracked_payment_backlog_count: u64,
    #[serde(default)]
    pub legacy_availability_confirmation_attention_count: u64,
    #[serde(default)]
    pub availability_online_identities_now: u64,
    #[serde(default)]
    pub availability_online_host_clusters_now: u64,
    #[serde(default)]
    pub availability_stipend_eligible_beneficiaries_now: u64,
    pub eligible_online_payout_targets: u64,
    pub sellable_pylons_online_now: u64,
    #[serde(default)]
    pub inference_ready_online_payout_targets: u64,
    #[serde(default)]
    pub duplicate_host_placeholder_blocked_online_targets: u64,
    #[serde(default)]
    pub duplicate_host_blocked_beneficiaries_now: u64,
    #[serde(default)]
    pub duplicate_payout_target_blocked_beneficiaries_now: u64,
    #[serde(default)]
    pub missing_payout_target_blocked_beneficiaries_now: u64,
    #[serde(default)]
    pub version_floor_blocked_beneficiaries_now: u64,
    #[serde(default)]
    pub readiness_blocked_beneficiaries_now: u64,
    #[serde(default)]
    pub min_new_accrual_version_blocked_online_targets: u64,
    #[serde(default)]
    pub min_new_accrual_unknown_version_online_targets: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_eligible_window_started_at_unix_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_dispatch_at_unix_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_confirmed_payout_at_unix_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub skip_reason_metrics_24h: Vec<TreasuryReasonMetric>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fail_reason_metrics_24h: Vec<TreasuryReasonMetric>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub active_continuity_alerts: Vec<TreasuryContinuityAlert>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub degraded_reason: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub degraded_states: Vec<TreasuryDegradedState>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub health_status: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TreasuryCommand {
    Status {
        json: bool,
    },
    FundingTarget {
        amount_sats: Option<u64>,
        description: Option<String>,
        expiry_seconds: Option<u32>,
        json: bool,
    },
    PayoutLedgerCleanup {
        apply: bool,
        report_path: Option<PathBuf>,
        json: bool,
    },
    RecoveryReport {
        work_dir: Option<PathBuf>,
        report_path: Option<PathBuf>,
        json: bool,
    },
    RecoveryCutover {
        report_path: PathBuf,
        json: bool,
    },
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TreasuryPublicStats {
    pub active_treasury_provider: String,
    pub active_treasury_rail: String,
    pub ldk_network: String,
    pub ldk_chain_backend: String,
    pub ldk_server_configured: bool,
    pub ldk_readiness: TreasuryLdkReadinessSnapshot,
    pub treasury_enabled: bool,
    pub payout_sats_per_window: u64,
    pub payout_interval_seconds: u64,
    pub require_sellable: bool,
    pub daily_budget_cap_sats: u64,
    pub accepted_work_policy: TreasuryAcceptedWorkPolicySnapshot,
    pub availability_policy: TreasuryAvailabilityPolicySnapshot,
    pub placeholder_payout_mode: TreasuryPlaceholderPayoutMode,
    pub dedupe_placeholder_hosts: bool,
    pub min_new_accrual_pylon_version: Option<String>,
    pub min_new_accrual_started_at_unix_ms: Option<u64>,
    pub min_new_accrual_version_gate_active: bool,
    pub registered_payout_identities: u64,
    pub ldk_payout_target_identities: u64,
    pub pylon_v0_2_registration_required_identities: u64,
    pub wallet_balance_sats: u64,
    pub wallet_total_onchain_balance_sats: u64,
    pub wallet_spendable_onchain_balance_sats: u64,
    pub wallet_lightning_balance_sats: u64,
    pub wallet_balance_updated_at_unix_ms: Option<u64>,
    pub last_wallet_sync_at_unix_ms: Option<u64>,
    pub last_wallet_refresh_attempt_at_unix_ms: Option<u64>,
    pub wallet_runtime_status: Option<String>,
    pub wallet_last_error: Option<String>,
    pub wallet_hydration_mode: Option<String>,
    pub wallet_payment_scan_mode: Option<String>,
    pub wallet_storage_runtime_mode: String,
    pub payout_loop_runtime_status: Option<String>,
    pub payout_loop_last_error: Option<String>,
    pub last_payout_reconciliation_at_unix_ms: Option<u64>,
    pub payout_loop_last_started_at_unix_ms: Option<u64>,
    pub payout_loop_last_completed_at_unix_ms: Option<u64>,
    pub public_snapshot_generated_at_unix_ms: Option<u64>,
    pub public_snapshot_source: String,
    pub public_snapshot_mode: Option<String>,
    pub public_snapshot_health_status: Option<String>,
    pub snapshot_age_ms: Option<u64>,
    pub wallet_sync_lag_ms: Option<u64>,
    pub backlog_total: u64,
    pub backlog_retryable: u64,
    pub pending_confirmation_count: u64,
    pub tracked_payment_backlog_count: u64,
    pub legacy_availability_confirmation_attention_count: u64,
    pub availability_online_identities_now: u64,
    pub availability_online_host_clusters_now: u64,
    pub availability_stipend_eligible_beneficiaries_now: u64,
    pub eligible_online_payout_targets: u64,
    pub sellable_pylons_online_now: u64,
    pub inference_ready_online_payout_targets: u64,
    pub duplicate_host_placeholder_blocked_online_targets: u64,
    pub duplicate_host_blocked_beneficiaries_now: u64,
    pub duplicate_payout_target_blocked_beneficiaries_now: u64,
    pub missing_payout_target_blocked_beneficiaries_now: u64,
    pub version_floor_blocked_beneficiaries_now: u64,
    pub readiness_blocked_beneficiaries_now: u64,
    pub min_new_accrual_version_blocked_online_targets: u64,
    pub min_new_accrual_unknown_version_online_targets: u64,
    pub latest_eligible_window_started_at_unix_ms: Option<u64>,
    pub last_dispatch_at_unix_ms: Option<u64>,
    pub last_confirmed_payout_at_unix_ms: Option<u64>,
    pub eligible_window_lag_ms: Option<u64>,
    pub dispatch_lag_ms: Option<u64>,
    pub confirm_lag_ms: Option<u64>,
    pub payout_loop_health: String,
    pub degraded_reason: Option<String>,
    pub degraded_states: Vec<TreasuryDegradedState>,
    pub payout_sats_paid_total: u64,
    pub payout_sats_paid_24h: u64,
    pub payout_sats_in_flight_total: u64,
    pub payout_sats_in_flight_24h: u64,
    pub accepted_work_payout_sats_paid_total: u64,
    pub accepted_work_payout_sats_paid_24h: u64,
    pub accepted_work_payout_sats_in_flight_total: u64,
    pub accepted_work_payout_sats_in_flight_24h: u64,
    pub availability_stipend_payout_sats_paid_total: u64,
    pub availability_stipend_payout_sats_paid_24h: u64,
    pub availability_stipend_payout_sats_in_flight_total: u64,
    pub availability_stipend_payout_sats_in_flight_24h: u64,
    pub placeholder_payout_sats_paid_total: u64,
    pub placeholder_payout_sats_paid_24h: u64,
    pub placeholder_payout_sats_in_flight_total: u64,
    pub placeholder_payout_sats_in_flight_24h: u64,
    pub beta_bonus_payout_sats_paid_total: u64,
    pub beta_bonus_payout_sats_paid_24h: u64,
    pub beta_bonus_payout_sats_in_flight_total: u64,
    pub beta_bonus_payout_sats_in_flight_24h: u64,
    pub weak_device_accepted_work_payout_sats_paid_total: u64,
    pub weak_device_accepted_work_payout_sats_paid_24h: u64,
    pub strong_lane_accepted_work_payout_sats_paid_total: u64,
    pub strong_lane_accepted_work_payout_sats_paid_24h: u64,
    pub payouts_dispatched_24h: u64,
    pub payouts_confirmed_24h: u64,
    pub payouts_failed_24h: u64,
    pub payouts_skipped_24h: u64,
    pub skip_reason_metrics_24h: Vec<TreasuryReasonMetric>,
    pub fail_reason_metrics_24h: Vec<TreasuryReasonMetric>,
    pub active_continuity_alerts: Vec<TreasuryContinuityAlert>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OnlinePylonIdentity {
    pub nostr_pubkey_hex: String,
    pub sellable: bool,
    pub client_version: Option<String>,
    pub inference_ready: bool,
    pub host_fingerprint: Option<String>,
    #[serde(default)]
    pub availability_stipend_eligible: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub availability_stipend_gate_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TreasuryAvailabilityBeneficiaryDebugRow {
    pub nostr_pubkey_hex: String,
    pub sellable: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_version: Option<String>,
    pub inference_ready: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host_fingerprint: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payout_target: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub beneficiary_kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub beneficiary_key: Option<String>,
    pub availability_stipend_eligible_now: bool,
    pub verdict_reason: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_window_started_at_unix_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_payout_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_payout_status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_payout_reason: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AvailabilityBeneficiaryKind {
    HostCluster,
    PayoutTarget,
    Identity,
}

impl AvailabilityBeneficiaryKind {
    const fn label(self) -> &'static str {
        match self {
            Self::HostCluster => "host_cluster",
            Self::PayoutTarget => "payout_target",
            Self::Identity => "identity",
        }
    }

    const fn duplicate_skip_reason(self) -> Option<&'static str> {
        match self {
            Self::HostCluster => Some("duplicate_host_placeholder_readiness"),
            Self::PayoutTarget => Some("duplicate_payout_target_placeholder_readiness"),
            Self::Identity => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AvailabilityBeneficiaryProjection {
    kind: AvailabilityBeneficiaryKind,
    key: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AvailabilityIdentityDisposition {
    identity: OnlinePylonIdentity,
    payout_target: Option<RegisteredPayoutTarget>,
    beneficiary: Option<AvailabilityBeneficiaryProjection>,
    current_window_started_at_unix_ms: Option<u64>,
    verdict_reason: Option<String>,
}

impl AvailabilityIdentityDisposition {
    const fn allowed(&self) -> bool {
        self.verdict_reason.is_none()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TreasuryCanonicalPublicSnapshot {
    pub version: String,
    pub source: String,
    pub generated_at_unix_ms: u64,
    pub stale_after_unix_ms: u64,
    pub health_status: String,
    pub mode: String,
    pub drain_active: bool,
    pub payout_sats_paid_total: u64,
    pub payout_sats_paid_24h: u64,
    pub payouts_dispatched_24h: u64,
    pub payouts_confirmed_24h: u64,
    pub payouts_failed_24h: u64,
    pub payouts_skipped_24h: u64,
    pub backlog_total: u64,
    pub backlog_retryable: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wallet_runtime_status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wallet_last_error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wallet_hydration_mode: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wallet_payment_scan_mode: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TreasuryIntegrationPolicySnapshot {
    pub treasury_enabled: bool,
    pub payout_sats_per_window: u64,
    pub payout_interval_seconds: u64,
    pub require_sellable: bool,
    pub daily_budget_cap_sats: u64,
    #[serde(default = "default_accepted_work_policy_snapshot")]
    pub accepted_work_policy: TreasuryAcceptedWorkPolicySnapshot,
    #[serde(default = "default_availability_policy_snapshot")]
    pub availability_policy: TreasuryAvailabilityPolicySnapshot,
    pub placeholder_payout_mode: TreasuryPlaceholderPayoutMode,
    pub dedupe_placeholder_hosts: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_new_accrual_pylon_version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_new_accrual_started_at_unix_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TreasuryIntegrationExportResponse {
    pub authority: String,
    pub generated_at_unix_ms: u64,
    pub policy: TreasuryIntegrationPolicySnapshot,
    pub payout_sats_paid_total_floor: u64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub payout_target_identities: Vec<TreasuryPayoutTargetIdentityStatus>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub online_identities: Vec<OnlinePylonIdentity>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TreasuryIntegrationImportResponse {
    pub authority: String,
    pub public_snapshot_source: String,
    pub public_snapshot_generated_at_unix_ms: u64,
    pub payout_sats_paid_total: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NewAccrualVersionGateVerdict {
    Allowed,
    MissingClientVersion,
    InvalidClientVersion,
    BelowFloor,
    InvalidPolicy,
}

impl NewAccrualVersionGateVerdict {
    const fn skip_reason(self) -> Option<&'static str> {
        match self {
            Self::Allowed => None,
            Self::MissingClientVersion => Some("missing_client_version_for_new_accrual"),
            Self::InvalidClientVersion => Some("invalid_client_version_for_new_accrual"),
            Self::BelowFloor => Some("below_min_new_accrual_version_floor"),
            Self::InvalidPolicy => Some("invalid_min_new_accrual_version_policy"),
        }
    }

    fn skip_reason_is_unknown_version(reason: &str) -> bool {
        matches!(
            reason,
            "missing_client_version_for_new_accrual" | "invalid_client_version_for_new_accrual"
        )
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum TreasuryPayoutClass {
    #[default]
    #[serde(rename = "availability_stipend", alias = "placeholder_liveness")]
    PlaceholderLiveness,
    AcceptedWork,
    BetaBonus,
}

impl TreasuryPayoutClass {
    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::PlaceholderLiveness => "availability_stipend",
            Self::AcceptedWork => "accepted_work",
            Self::BetaBonus => "beta_bonus",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct TreasuryPayoutClassification {
    #[serde(default)]
    pub payout_class: TreasuryPayoutClass,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payout_basis: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub work_class: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub progress_class: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accepted_outcome_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub training_run_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contribution_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assignment_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub share_bps: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub weight_basis: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub weight_value: Option<u64>,
    #[serde(default)]
    pub weak_device_bearing: bool,
    #[serde(default)]
    pub progress_bearing: bool,
}

impl TreasuryPayoutClassification {
    pub(crate) fn effective_payout_class(&self) -> TreasuryPayoutClass {
        if self.payout_class != TreasuryPayoutClass::PlaceholderLiveness {
            return self.payout_class;
        }
        let legacy_accepted_work_markers_present = self.work_class.is_some()
            || self.progress_class.is_some()
            || self.accepted_outcome_id.is_some()
            || self.training_run_id.is_some()
            || self.window_id.is_some()
            || self.contribution_id.is_some()
            || self.assignment_id.is_some()
            || self.share_bps.is_some()
            || self.weight_basis.is_some()
            || self.weight_value.is_some()
            || self.weak_device_bearing
            || self.progress_bearing
            || self
                .payout_basis
                .as_deref()
                .is_some_and(|basis| !payout_basis_is_placeholder_liveness(basis));
        if legacy_accepted_work_markers_present {
            TreasuryPayoutClass::AcceptedWork
        } else {
            TreasuryPayoutClass::PlaceholderLiveness
        }
    }

    fn normalize_legacy_payout_class(&mut self) -> bool {
        let effective = self.effective_payout_class();
        if self.payout_class == effective {
            return false;
        }
        self.payout_class = effective;
        true
    }

    fn accepted_work(&self) -> bool {
        self.effective_payout_class() == TreasuryPayoutClass::AcceptedWork
    }

    fn continuity_alert_relevant(&self, policy: &TreasuryRuntimePolicy) -> bool {
        match self.effective_payout_class() {
            TreasuryPayoutClass::PlaceholderLiveness => {
                policy.placeholder_payout_mode != TreasuryPlaceholderPayoutMode::Disabled
            }
            TreasuryPayoutClass::AcceptedWork | TreasuryPayoutClass::BetaBonus => true,
        }
    }

    fn weak_device_accepted_work(&self) -> bool {
        self.accepted_work() && self.weak_device_bearing
    }

    fn strong_lane_accepted_work(&self) -> bool {
        self.accepted_work() && !self.weak_device_bearing
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RegisteredPayoutTarget {
    pub nostr_pubkey_hex: String,
    pub source_session_id: String,
    #[serde(default)]
    pub payment_target_kind: String,
    #[serde(default)]
    pub payment_target: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub payment_target_capabilities: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pylon_payment_target_version: Option<String>,
    pub provider_target: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bitcoin_address: Option<String>,
    pub registered_at_unix_ms: u64,
    pub last_verified_at_unix_ms: u64,
}

impl RegisteredPayoutTarget {
    fn normalized_payment_target_kind(&self) -> &str {
        let kind = self.payment_target_kind.trim();
        if !kind.is_empty() {
            return kind;
        }
        if !self.provider_target.trim().is_empty() {
            return "provider_target";
        }
        "unknown"
    }

    fn normalized_payment_target(&self) -> &str {
        let target = self.payment_target.trim();
        if !target.is_empty() {
            return target;
        }
        self.provider_target.trim()
    }

    fn is_ldk_compatible(&self) -> bool {
        matches!(
            self.normalized_payment_target_kind(),
            "bolt12_offer" | "bolt11_invoice" | "bip353_name" | "lnurl_pay"
        )
    }

    fn requires_v0_2_registration(&self) -> bool {
        !self.is_ldk_compatible()
    }
}

fn normalized_registration_target_kind(
    request: &ProviderPayoutTargetRegistrationRequest,
) -> Result<String> {
    let explicit = request
        .payment_target_kind
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_ascii_lowercase());
    let kind = match explicit {
        Some(kind) => kind,
        None => request
            .payment_target
            .as_deref()
            .ok_or_else(|| anyhow!("payment_target_missing"))
            .and_then(|value| infer_ldk_payment_target_kind(value).map_err(anyhow::Error::msg))?,
    };
    if is_ldk_payment_target_kind(kind.as_str()) {
        Ok(kind)
    } else if kind == "provider_target" {
        bail!("unsupported_payment_target_kind:provider_target")
    } else {
        bail!("unsupported_payment_target_kind:{kind}")
    }
}

fn normalized_registration_target_value(
    request: &ProviderPayoutTargetRegistrationRequest,
) -> Result<String> {
    let value = request
        .payment_target
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow!("payment_target_missing"))?;
    Ok(value.to_string())
}

fn normalized_payment_target_capabilities(
    request: &ProviderPayoutTargetRegistrationRequest,
) -> Result<Vec<String>> {
    let mut capabilities = request
        .payment_target_capabilities
        .iter()
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    let kind = normalized_registration_target_kind(request)?;
    for capability in ldk_payment_target_capabilities(kind.as_str()).map_err(anyhow::Error::msg)? {
        if !capabilities.iter().any(|value| value == &capability) {
            capabilities.push(capability);
        }
    }
    capabilities.sort();
    capabilities.dedup();
    Ok(capabilities)
}

fn normalized_payment_target_version(request: &ProviderPayoutTargetRegistrationRequest) -> String {
    request
        .pylon_payment_target_version
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(PYLON_PAYMENT_TARGET_VERSION_V0_2)
        .to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TreasuryRegistrationChallenge {
    pub nostr_pubkey_hex: String,
    pub session_id: String,
    pub challenge: String,
    pub issued_at_unix_ms: u64,
    pub expires_at_unix_ms: u64,
    pub consumed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TreasuryFundingReceive {
    pub payment_id: String,
    pub status: String,
    pub amount_sats: u64,
    pub method: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub recorded_at_unix_ms: u64,
    pub updated_at_unix_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TreasuryPayoutRecord {
    pub payout_key: String,
    pub nostr_pubkey_hex: String,
    pub payout_target: String,
    pub amount_sats: u64,
    pub status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payment_id: Option<String>,
    pub window_started_at_unix_ms: u64,
    pub window_ends_at_unix_ms: u64,
    pub created_at_unix_ms: u64,
    pub updated_at_unix_ms: u64,
    pub sellable_at_window_open: bool,
    #[serde(default)]
    pub dispatch_receipt_recorded: bool,
    #[serde(default)]
    pub confirm_receipt_recorded: bool,
    #[serde(default)]
    pub fail_receipt_recorded: bool,
    #[serde(default)]
    pub skip_receipt_recorded: bool,
    #[serde(default)]
    pub counted_in_paid_total: bool,
    #[serde(default)]
    pub classification: TreasuryPayoutClassification,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum TreasuryOperationKind {
    FundingInvoiceCreation,
    OutboundPayoutDispatch,
    PaymentStatusLookup,
    EventProjection,
    ReconciliationPass,
    LightningAdminCommand,
}

impl TreasuryOperationKind {
    const fn as_str(self) -> &'static str {
        match self {
            Self::FundingInvoiceCreation => "funding_invoice_creation",
            Self::OutboundPayoutDispatch => "outbound_payout_dispatch",
            Self::PaymentStatusLookup => "payment_status_lookup",
            Self::EventProjection => "event_projection",
            Self::ReconciliationPass => "reconciliation_pass",
            Self::LightningAdminCommand => "lightning_admin_command",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TreasuryOperationStatus {
    Pending,
    Completed,
    Failed,
    Degraded,
}

impl TreasuryOperationStatus {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Degraded => "degraded",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TreasuryOperationRecord {
    pub operation_id: String,
    pub kind: TreasuryOperationKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
    pub rail: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub rail_metadata: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub amount_msat: Option<u64>,
    pub target_kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub beneficiary: Option<String>,
    pub status: TreasuryOperationStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_payment_id: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub receipt_refs: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub degraded_reason: Option<String>,
    pub created_at_unix_ms: u64,
    pub updated_at_unix_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub terminal_event_state: Option<String>,
}

impl TreasuryOperationRecord {
    pub fn command(&self) -> Option<&str> {
        self.rail_metadata.get("command").map(String::as_str)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TreasuryLdkReadinessSnapshot {
    pub state: String,
    pub registered_payout_target_count: u64,
    pub projected_channel_count: u64,
    pub projected_inbound_capacity_sats: u64,
    pub projected_outbound_capacity_sats: u64,
    #[serde(default)]
    pub min_ready_channel_count: u64,
    #[serde(default)]
    pub min_ready_outbound_capacity_sats: u64,
    pub recent_failed_payment_count_24h: u64,
    pub recent_no_route_count_24h: u64,
    pub recent_insufficient_balance_count_24h: u64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub operator_actions: Vec<String>,
}

impl Default for TreasuryLdkReadinessSnapshot {
    fn default() -> Self {
        Self {
            state: "unknown".to_string(),
            registered_payout_target_count: 0,
            projected_channel_count: 0,
            projected_inbound_capacity_sats: 0,
            projected_outbound_capacity_sats: 0,
            min_ready_channel_count: DEFAULT_TREASURY_LDK_MIN_READY_CHANNEL_COUNT,
            min_ready_outbound_capacity_sats: DEFAULT_TREASURY_LDK_MIN_READY_OUTBOUND_CAPACITY_SATS,
            recent_failed_payment_count_24h: 0,
            recent_no_route_count_24h: 0,
            recent_insufficient_balance_count_24h: 0,
            operator_actions: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct TreasuryLdkChannelReadiness {
    projected_channel_count: u64,
    projected_inbound_capacity_sats: u64,
    projected_outbound_capacity_sats: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TreasuryLdkProviderChannelSnapshot {
    pub channel_id_hash: String,
    pub peer_node_id_hash: String,
    pub status: String,
    pub outbound_capacity_sats: u64,
    pub inbound_capacity_sats: u64,
    pub observed_at_unix_ms: u64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct TreasuryLdkPaymentFailureReadiness {
    recent_failed_payment_count_24h: u64,
    recent_no_route_count_24h: u64,
    recent_insufficient_balance_count_24h: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreasuryQueuedPayoutRequest {
    pub payout_key: String,
    pub nostr_pubkey_hex: String,
    pub amount_sats: u64,
    pub window_started_at_unix_ms: u64,
    pub window_ends_at_unix_ms: u64,
    pub classification: TreasuryPayoutClassification,
    pub queue_block_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TreasuryState {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_policy: Option<TreasuryRuntimePolicy>,
    #[serde(default)]
    pub policy_change_history: Vec<TreasuryPolicyChangeRecord>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub policy_runtime_status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub policy_last_error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub public_snapshot: Option<TreasuryPublicSnapshot>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub canonical_public_snapshot: Option<TreasuryCanonicalPublicSnapshot>,
    #[serde(default)]
    pub payout_targets_by_identity: BTreeMap<String, RegisteredPayoutTarget>,
    #[serde(default)]
    pub payout_records_by_key: BTreeMap<String, TreasuryPayoutRecord>,
    #[serde(skip)]
    payout_key_by_payment_id: BTreeMap<String, String>,
    #[serde(default)]
    pub funding_receives_by_payment_id: BTreeMap<String, TreasuryFundingReceive>,
    #[serde(default)]
    pub treasury_operations_by_id: BTreeMap<String, TreasuryOperationRecord>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub ldk_provider_channels_by_id_hash: BTreeMap<String, TreasuryLdkProviderChannelSnapshot>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wallet_runtime_status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wallet_last_error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wallet_hydration_mode: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wallet_payment_scan_mode: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wallet_storage_runtime_mode: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wallet_storage_report_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wallet_storage_rollback_dir: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wallet_storage_cutover_at_unix_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_wallet_recovery_report: Option<TreasuryWalletRecoveryReportSummary>,
    #[serde(default)]
    pub wallet_balance_sats: u64,
    #[serde(default, skip_serializing_if = "is_zero_u64")]
    pub wallet_total_onchain_balance_sats: u64,
    #[serde(default, skip_serializing_if = "is_zero_u64")]
    pub wallet_spendable_onchain_balance_sats: u64,
    #[serde(default, skip_serializing_if = "is_zero_u64")]
    pub wallet_lightning_balance_sats: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wallet_balance_updated_at_unix_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_wallet_sync_at_unix_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_wallet_refresh_attempt_at_unix_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "is_zero_usize")]
    pub wallet_refresh_history_page_offset: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payout_loop_runtime_status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payout_loop_last_error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_payout_reconciliation_at_unix_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payout_loop_last_started_at_unix_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payout_loop_last_completed_at_unix_ms: Option<u64>,
    #[serde(default)]
    pub availability_online_identities_now: u64,
    #[serde(default)]
    pub availability_online_host_clusters_now: u64,
    #[serde(default)]
    pub availability_stipend_eligible_beneficiaries_now: u64,
    #[serde(default)]
    pub eligible_online_payout_targets: u64,
    #[serde(default)]
    pub sellable_pylons_online_now: u64,
    #[serde(default)]
    pub inference_ready_online_payout_targets: u64,
    #[serde(default)]
    pub duplicate_host_placeholder_blocked_online_targets: u64,
    #[serde(default)]
    pub duplicate_payout_target_placeholder_blocked_online_targets: u64,
    #[serde(default)]
    pub missing_payout_target_blocked_online_targets: u64,
    #[serde(default)]
    pub version_floor_blocked_beneficiaries_now: u64,
    #[serde(default)]
    pub readiness_blocked_online_targets: u64,
    #[serde(default)]
    pub min_new_accrual_version_blocked_online_targets: u64,
    #[serde(default)]
    pub min_new_accrual_unknown_version_online_targets: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_eligible_window_started_at_unix_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_dispatch_at_unix_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_confirmed_payout_at_unix_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub active_continuity_alerts: Vec<TreasuryContinuityAlert>,
    #[serde(default)]
    pub payout_sats_paid_total: u64,
    #[serde(default)]
    pub accepted_work_payout_sats_paid_total: u64,
    #[serde(default)]
    pub placeholder_payout_sats_paid_total: u64,
    #[serde(default)]
    pub beta_bonus_payout_sats_paid_total: u64,
    #[serde(default)]
    pub weak_device_accepted_work_payout_sats_paid_total: u64,
    #[serde(default)]
    pub strong_lane_accepted_work_payout_sats_paid_total: u64,
    #[serde(default)]
    pub next_challenge_nonce: u64,
    #[serde(default)]
    pub registration_challenges_by_key: BTreeMap<String, TreasuryRegistrationChallenge>,
    #[serde(skip)]
    state_path: Option<PathBuf>,
    #[serde(skip)]
    last_persistence_error: Option<String>,
    #[serde(skip)]
    availability_beneficiary_debug_rows: Vec<TreasuryAvailabilityBeneficiaryDebugRow>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct AvailabilityObservabilitySnapshot {
    availability_online_identities_now: u64,
    availability_online_host_clusters_now: u64,
    availability_stipend_eligible_beneficiaries_now: u64,
    eligible_online_payout_targets: u64,
    inference_ready_online_payout_targets: u64,
    duplicate_host_placeholder_blocked_online_targets: u64,
    duplicate_payout_target_placeholder_blocked_online_targets: u64,
    missing_payout_target_blocked_online_targets: u64,
    version_floor_blocked_beneficiaries_now: u64,
    min_new_accrual_unknown_version_online_targets: u64,
    readiness_blocked_online_targets: u64,
    latest_eligible_window_started_at_unix_ms: Option<u64>,
    availability_beneficiary_debug_rows: Vec<TreasuryAvailabilityBeneficiaryDebugRow>,
}

pub(crate) fn treasury_hash(value: &str) -> String {
    format!("sha256:{}", hex::encode(Sha256::digest(value.as_bytes())))
}

fn treasury_short_hash(value: &str) -> String {
    hex::encode(&Sha256::digest(value.as_bytes())[..16])
}

fn treasury_operation_id(kind: TreasuryOperationKind, request_id: &str) -> String {
    format!(
        "treasury-op-{}-{}",
        kind.as_str(),
        treasury_short_hash(request_id)
    )
}

pub fn treasury_admin_operation_request_id(command: &str, idempotency_key: &str) -> String {
    format!("admin:{}:{}", command.trim(), idempotency_key.trim())
}

pub fn treasury_admin_operation_id(command: &str, idempotency_key: &str) -> String {
    treasury_operation_id(
        TreasuryOperationKind::LightningAdminCommand,
        treasury_admin_operation_request_id(command, idempotency_key).as_str(),
    )
}

fn operation_rail_for_provider(provider: TreasuryLightningProviderKind) -> &'static str {
    match provider {
        TreasuryLightningProviderKind::Ldk => "ldk",
    }
}

fn ldk_server_configured(config: &TreasuryConfig) -> bool {
    config
        .lightning_provider
        .ldk
        .server_url
        .as_deref()
        .map(str::trim)
        .is_some_and(|value| !value.is_empty())
}

fn payout_target_kind_for_payment_request(payment_request: &str) -> &'static str {
    let target = payment_request.trim().to_ascii_lowercase();
    if target.starts_with("lno") {
        "bolt12_offer"
    } else if target.starts_with("lnurl") {
        "lnurl_pay"
    } else if target.starts_with("lnbc")
        || target.starts_with("lntb")
        || target.starts_with("lnbcrt")
        || target.starts_with("lntbs")
    {
        "bolt11_invoice"
    } else if target.contains('@') {
        "bip353_name"
    } else {
        "unknown"
    }
}

fn payout_rail_for_payment_request(payment_request: &str) -> &'static str {
    match payout_target_kind_for_payment_request(payment_request) {
        "unknown" => "unknown",
        _ => "ldk",
    }
}

fn payout_record_has_ldk_target(record: &TreasuryPayoutRecord) -> bool {
    payout_rail_for_payment_request(record.payout_target.as_str()) == "ldk"
}

fn payout_record_reason_contains(record: &TreasuryPayoutRecord, needle: &str) -> bool {
    record
        .reason
        .as_deref()
        .is_some_and(|reason| reason.contains(needle))
}

fn payout_record_is_retired_historical(record: &TreasuryPayoutRecord) -> bool {
    record.status == "failed"
        && (record.reason.as_deref() == Some(TREASURY_RETIRED_UNPAYABLE_PAYOUT_REASON)
            || (!payout_record_has_ldk_target(record)
                && payout_record_reason_contains(
                    record,
                    TREASURY_UNSUPPORTED_LDK_PAYMENT_TARGET_KIND_REASON,
                )))
}

fn payout_record_should_be_retired_as_historical(record: &TreasuryPayoutRecord) -> bool {
    matches!(
        record.status.as_str(),
        "queued" | "dispatching" | "dispatched" | "failed"
    ) && !payout_record_has_ldk_target(record)
}

fn payout_record_cleanup_disposition(record: &TreasuryPayoutRecord) -> &'static str {
    match record.status.as_str() {
        "confirmed" => "settled",
        "queued" | "dispatching" | "dispatched" if payout_record_has_ldk_target(record) => {
            "current_ldk_pending"
        }
        "queued" | "dispatching" | "dispatched" => "retire_unpayable_non_ldk",
        "failed" if payout_record_is_retired_historical(record) => "retired_historical",
        "failed" if failed_payout_is_retryable_pending(record) => "current_ldk_retryable",
        "failed" if payout_record_has_ldk_target(record) => "current_ldk_attention",
        "failed" => "retire_unpayable_non_ldk",
        "skipped" if record.reason.as_deref() == Some("missing_payout_target") => {
            "missing_payout_target"
        }
        "skipped" => "skipped",
        _ => "unknown",
    }
}

fn degraded_severity_rank(severity: &str) -> u8 {
    match severity {
        "critical" => 3,
        "warning" => 2,
        "info" => 1,
        _ => 0,
    }
}

fn push_unique_degraded_state(
    states: &mut Vec<TreasuryDegradedState>,
    state: TreasuryDegradedState,
) {
    if states.iter().any(|existing| existing.code == state.code) {
        return;
    }
    states.push(state);
}

fn treasury_reason_indicates_no_route(reason: &str) -> bool {
    let normalized = reason.to_ascii_lowercase();
    normalized.contains("no_route")
        || normalized.contains("no route")
        || normalized.contains("route not found")
}

fn treasury_reason_indicates_insufficient_balance(reason: &str) -> bool {
    let normalized = reason.to_ascii_lowercase();
    normalized.contains("insufficient_balance")
        || normalized.contains("insufficient channel")
        || normalized.contains("insufficient funds")
        || normalized.contains("wallet_balance_insufficient")
        || normalized.contains("not enough balance")
}

fn treasury_reason_indicates_stale_event_stream(reason: &str) -> bool {
    let normalized = reason.to_ascii_lowercase();
    normalized.contains("stale_event_stream")
        || normalized.contains("event stream")
        || normalized.contains("subscriber")
}

fn treasury_reason_indicates_stale_gossip(reason: &str) -> bool {
    let normalized = reason.to_ascii_lowercase();
    (normalized.contains("stale") || normalized.contains("lagged"))
        && (normalized.contains("gossip") || normalized.contains("rgs"))
}

fn payout_dispatch_idempotency_key(payout_key: &str) -> String {
    format!("payout:{payout_key}")
}

fn operation_status_for_payment_status(status: &str) -> TreasuryOperationStatus {
    let lowered = status.trim().to_ascii_lowercase();
    if matches!(
        lowered.as_str(),
        "completed" | "complete" | "confirmed" | "succeeded" | "success"
    ) {
        TreasuryOperationStatus::Completed
    } else if lowered.contains("fail") || lowered.contains("cancel") || lowered.contains("error") {
        TreasuryOperationStatus::Failed
    } else {
        TreasuryOperationStatus::Pending
    }
}

fn operation_amount_msat(amount_sats: u64) -> Option<u64> {
    amount_sats.checked_mul(1_000)
}

fn payout_dispatch_operation_from_record(
    config: &TreasuryConfig,
    record: &TreasuryPayoutRecord,
    now_unix_ms: u64,
) -> TreasuryOperationRecord {
    let mut rail_metadata = BTreeMap::new();
    rail_metadata.insert(
        "provider".to_string(),
        config.lightning_provider.provider.as_str().to_string(),
    );
    rail_metadata.insert(
        "payment_target_kind".to_string(),
        payout_target_kind_for_payment_request(record.payout_target.as_str()).to_string(),
    );
    if !record.payout_target.trim().is_empty() {
        rail_metadata.insert(
            "payment_target_hash".to_string(),
            treasury_hash(record.payout_target.as_str()),
        );
    }
    rail_metadata.insert(
        "idempotency_key".to_string(),
        payout_dispatch_idempotency_key(record.payout_key.as_str()),
    );
    rail_metadata.insert(
        "payout_class".to_string(),
        record
            .classification
            .effective_payout_class()
            .label()
            .to_string(),
    );
    if let Some(training_run_id) = record.classification.training_run_id.as_deref() {
        rail_metadata.insert("training_run_id".to_string(), training_run_id.to_string());
    }

    TreasuryOperationRecord {
        operation_id: treasury_operation_id(
            TreasuryOperationKind::OutboundPayoutDispatch,
            record.payout_key.as_str(),
        ),
        kind: TreasuryOperationKind::OutboundPayoutDispatch,
        request_id: Some(record.payout_key.clone()),
        rail: payout_rail_for_payment_request(record.payout_target.as_str()).to_string(),
        rail_metadata,
        amount_msat: operation_amount_msat(record.amount_sats),
        target_kind: payout_target_kind_for_payment_request(record.payout_target.as_str())
            .to_string(),
        target_hash: (!record.payout_target.trim().is_empty())
            .then(|| treasury_hash(record.payout_target.as_str())),
        beneficiary: Some(record.nostr_pubkey_hex.clone()),
        status: TreasuryOperationStatus::Pending,
        provider_payment_id: record
            .payment_id
            .as_ref()
            .map(|payment_id| treasury_hash(payment_id.as_str())),
        receipt_refs: Vec::new(),
        degraded_reason: record.reason.clone(),
        created_at_unix_ms: record.created_at_unix_ms.min(now_unix_ms),
        updated_at_unix_ms: now_unix_ms,
        terminal_event_state: None,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreasuryReceiptEvent {
    pub receipt_type: &'static str,
    pub context: AuthorityReceiptContext,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreasuryDispatchPlan {
    pub payout_key: String,
    pub payment_request: String,
    pub amount_sats: u64,
    pub classification: TreasuryPayoutClassification,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TreasuryPayoutPreparation {
    pub dispatch_plans: Vec<TreasuryDispatchPlan>,
    pub receipt_events: Vec<TreasuryReceiptEvent>,
    pub reconciliation_degraded_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TreasuryDispatchOutcome {
    Dispatched {
        payout_key: String,
        payment_id: String,
        terminal_event_state: Option<String>,
    },
    Failed {
        payout_key: String,
        reason: String,
    },
}

#[derive(Debug, Clone, Default)]
pub struct TreasuryDispatchBatchResult {
    pub outcomes: Vec<TreasuryDispatchOutcome>,
    pub wallet_snapshot: Option<TreasuryWalletSnapshot>,
    pub wallet_error: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct TreasuryPayoutTotals {
    payout_sats_paid_total: u64,
    accepted_work_payout_sats_paid_total: u64,
    placeholder_payout_sats_paid_total: u64,
    beta_bonus_payout_sats_paid_total: u64,
    weak_device_accepted_work_payout_sats_paid_total: u64,
    strong_lane_accepted_work_payout_sats_paid_total: u64,
}

impl TreasuryPayoutTotals {
    fn add_amount(&mut self, amount_sats: u64, classification: &TreasuryPayoutClassification) {
        self.payout_sats_paid_total = self.payout_sats_paid_total.saturating_add(amount_sats);
        match classification.effective_payout_class() {
            TreasuryPayoutClass::PlaceholderLiveness => {
                self.placeholder_payout_sats_paid_total = self
                    .placeholder_payout_sats_paid_total
                    .saturating_add(amount_sats);
            }
            TreasuryPayoutClass::AcceptedWork => {
                self.accepted_work_payout_sats_paid_total = self
                    .accepted_work_payout_sats_paid_total
                    .saturating_add(amount_sats);
                if classification.weak_device_accepted_work() {
                    self.weak_device_accepted_work_payout_sats_paid_total = self
                        .weak_device_accepted_work_payout_sats_paid_total
                        .saturating_add(amount_sats);
                }
                if classification.strong_lane_accepted_work() {
                    self.strong_lane_accepted_work_payout_sats_paid_total = self
                        .strong_lane_accepted_work_payout_sats_paid_total
                        .saturating_add(amount_sats);
                }
            }
            TreasuryPayoutClass::BetaBonus => {
                self.beta_bonus_payout_sats_paid_total = self
                    .beta_bonus_payout_sats_paid_total
                    .saturating_add(amount_sats);
            }
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct TreasuryCommittedBudgetTotals {
    availability_stipend_sats: u64,
    accepted_work_sats: u64,
    beta_bonus_sats: u64,
}

impl TreasuryCommittedBudgetTotals {
    fn total_for_class(self, payout_class: TreasuryPayoutClass) -> u64 {
        match payout_class {
            TreasuryPayoutClass::PlaceholderLiveness => self.availability_stipend_sats,
            TreasuryPayoutClass::AcceptedWork => self.accepted_work_sats,
            TreasuryPayoutClass::BetaBonus => self.beta_bonus_sats,
        }
    }

    fn add_amount(&mut self, payout_class: TreasuryPayoutClass, amount_sats: u64) {
        match payout_class {
            TreasuryPayoutClass::PlaceholderLiveness => {
                self.availability_stipend_sats =
                    self.availability_stipend_sats.saturating_add(amount_sats);
            }
            TreasuryPayoutClass::AcceptedWork => {
                self.accepted_work_sats = self.accepted_work_sats.saturating_add(amount_sats);
            }
            TreasuryPayoutClass::BetaBonus => {
                self.beta_bonus_sats = self.beta_bonus_sats.saturating_add(amount_sats);
            }
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct TreasuryWalletSnapshot {
    pub runtime_status: String,
    pub runtime_detail: Option<String>,
    pub wallet_hydration_mode: Option<String>,
    pub wallet_payment_scan_mode: Option<String>,
    pub balance_sats: u64,
    pub total_onchain_balance_sats: u64,
    pub spendable_onchain_balance_sats: u64,
    pub lightning_balance_sats: u64,
    pub payments: Vec<PaymentSummary>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TreasuryWalletRefreshPlan {
    tracked_payment_ids: BTreeSet<String>,
    history_scan_page_offset: usize,
    expected_nonzero_balance: bool,
    historical_receive_total_sats: u64,
    payout_sats_paid_total: u64,
}

impl TreasuryWalletRefreshPlan {
    pub fn recent_only() -> Self {
        Self::default()
    }

    fn track_payment_id(&mut self, payment_id: &str) {
        let payment_id = payment_id.trim();
        if payment_id.is_empty() {
            return;
        }
        self.tracked_payment_ids.insert(payment_id.to_string());
    }

    fn tracked_payment_count(&self) -> usize {
        self.tracked_payment_ids.len()
    }

    fn payment_page_budget(&self) -> usize {
        let budget = wallet_refresh_payment_page_budget(self.tracked_payment_count());
        if self.history_scan_page_offset > 0 {
            return budget.max(TREASURY_WALLET_REFRESH_CURSOR_PAYMENT_PAGES);
        }

        budget
    }

    fn expects_funded_balance(&self) -> bool {
        self.expected_nonzero_balance
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TreasuryWalletRefreshProgress {
    history_scan_page_offset: usize,
    history_pages_scanned: usize,
    history_hit_end_of_history: bool,
}

#[derive(Debug, Clone, Default)]
pub struct TreasuryWalletRefreshResult {
    pub snapshot: TreasuryWalletSnapshot,
    pub progress: TreasuryWalletRefreshProgress,
}

#[derive(Debug, Clone)]
pub struct TreasuryFundingMaterial {
    pub provider_target: String,
    pub bitcoin_address: String,
    pub provider_invoice: Option<String>,
    pub bolt11_invoice: Option<String>,
    pub provider_payment_id: Option<String>,
    pub phase_timings: TreasuryFundingTargetPhaseTimings,
    pub wallet_snapshot: TreasuryWalletSnapshot,
}

#[derive(Debug, Clone, Copy, Default)]
struct TreasuryStateSalvagedTotals {
    payout_sats_paid_total: Option<u64>,
    visible_payout_sats_paid_total: Option<u64>,
}

fn json_value_to_u64(value: Option<&serde_json::Value>) -> Option<u64> {
    let value = value?;
    value
        .as_u64()
        .or_else(|| value.as_i64().and_then(|value| u64::try_from(value).ok()))
        .or_else(|| value.as_str().and_then(|value| value.parse::<u64>().ok()))
}

fn is_zero_usize(value: &usize) -> bool {
    *value == 0
}

fn is_zero_u64(value: &u64) -> bool {
    *value == 0
}

fn treasury_state_salvaged_totals_from_payload(payload: &str) -> TreasuryStateSalvagedTotals {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(payload) else {
        return TreasuryStateSalvagedTotals::default();
    };
    let payout_sats_paid_total = json_value_to_u64(value.get("payout_sats_paid_total"));
    let visible_payout_sats_paid_total = value
        .get("public_snapshot")
        .and_then(|snapshot| json_value_to_u64(snapshot.get("payout_sats_paid_total")));
    TreasuryStateSalvagedTotals {
        payout_sats_paid_total,
        visible_payout_sats_paid_total,
    }
}

fn recover_treasury_state_from_value(
    value: serde_json::Value,
) -> Option<(TreasuryState, &'static [&'static str])> {
    let serde_json::Value::Object(object) = value else {
        return None;
    };
    for dropped_fields in TREASURY_STATE_RECOVERY_DROP_FIELD_SETS {
        let mut candidate = object.clone();
        for field in *dropped_fields {
            candidate.remove(*field);
        }
        if let Ok(state) =
            serde_json::from_value::<TreasuryState>(serde_json::Value::Object(candidate))
        {
            return Some((state, dropped_fields));
        }
    }
    None
}

fn recovered_treasury_state_from_payload(
    payload: &str,
    error: &serde_json::Error,
) -> TreasuryState {
    let salvaged_totals = treasury_state_salvaged_totals_from_payload(payload);
    let recovered = serde_json::from_str::<serde_json::Value>(payload)
        .ok()
        .and_then(recover_treasury_state_from_value);
    let (mut state, recovery_detail) = if let Some((mut state, dropped_fields)) = recovered {
        if let Some(payout_sats_paid_total) = salvaged_totals.payout_sats_paid_total {
            state.payout_sats_paid_total = state.payout_sats_paid_total.max(payout_sats_paid_total);
        }
        (
            state,
            format!("recovered_without={}", dropped_fields.join(",")),
        )
    } else {
        let mut state = TreasuryState::default();
        state.payout_sats_paid_total = salvaged_totals
            .visible_payout_sats_paid_total
            .or(salvaged_totals.payout_sats_paid_total)
            .unwrap_or_default();
        state.placeholder_payout_sats_paid_total = state.payout_sats_paid_total;
        (state, "recovered_from=salvaged_totals_only".to_string())
    };
    let detail = format!("treasury_state_deserialize_failed:{error}:{recovery_detail}");
    state.public_snapshot = None;
    state.wallet_runtime_status = Some("error".to_string());
    state.wallet_last_error = Some(detail.clone());
    state.payout_loop_runtime_status = Some("error".to_string());
    state.payout_loop_last_error = Some(detail);
    state
}

fn retryable_failed_payout_is_due(record: &TreasuryPayoutRecord, now_unix_ms: u64) -> bool {
    record.status == "failed"
        && record.payment_id.is_none()
        && !record.payout_target.trim().is_empty()
        && payout_record_has_ldk_target(record)
        && record
            .reason
            .as_deref()
            .is_some_and(failed_payout_reason_is_retryable)
        && now_unix_ms
            >= record
                .updated_at_unix_ms
                .saturating_add(TREASURY_FAILED_PAYOUT_RETRY_AFTER_MS)
}

fn failed_payout_reason_is_retryable(reason: &str) -> bool {
    reason == "dispatch_outcome_timeout"
        || reason == "insufficient_funds"
        || reason == "wallet_balance_insufficient"
        || reason.starts_with("wallet_open_timeout:")
        || reason.starts_with("wallet_send_timeout:")
        || reason.starts_with("wallet_send_retryable:")
}

fn failed_payout_is_retryable_pending(record: &TreasuryPayoutRecord) -> bool {
    record.status == "failed"
        && record.payment_id.is_none()
        && !record.payout_target.trim().is_empty()
        && payout_record_has_ldk_target(record)
        && record
            .reason
            .as_deref()
            .is_some_and(failed_payout_reason_is_retryable)
}

fn wallet_send_failure_is_leaf_selection(reason: &str) -> bool {
    reason.starts_with("wallet_send_retryable:leaf_selection:")
}

fn payout_basis_is_placeholder_liveness(basis: &str) -> bool {
    matches!(
        basis.trim(),
        "inference_ready" | "presence_only" | "disabled"
    )
}

fn availability_reason_is_duplicate_host(reason: &str) -> bool {
    reason == "duplicate_host_placeholder_readiness"
}

fn availability_reason_is_duplicate_payout_target(reason: &str) -> bool {
    reason == "duplicate_payout_target_placeholder_readiness"
}

fn availability_reason_is_missing_payout_target(reason: &str) -> bool {
    reason == "missing_payout_target"
}

fn availability_reason_is_version_floor(reason: &str) -> bool {
    matches!(
        reason,
        "below_min_new_accrual_version_floor"
            | "missing_client_version_for_new_accrual"
            | "invalid_client_version_for_new_accrual"
            | "invalid_min_new_accrual_version_policy"
    )
}

fn placeholder_liveness_record_can_compact(record: &TreasuryPayoutRecord) -> bool {
    if record.classification.effective_payout_class() != TreasuryPayoutClass::PlaceholderLiveness {
        return false;
    }

    match record.status.as_str() {
        "confirmed" => record.counted_in_paid_total,
        "failed" | "skipped" => true,
        "queued" => record.reason.as_deref() == Some("placeholder_payouts_disabled"),
        _ => false,
    }
}

impl TreasuryState {
    fn ldk_payout_target_identity_count(&self) -> u64 {
        self.payout_targets_by_identity
            .values()
            .filter(|target| target.is_ldk_compatible())
            .count() as u64
    }

    pub fn identity_has_registered_ldk_payout_target(&self, nostr_pubkey_hex: &str) -> bool {
        self.payout_targets_by_identity
            .get(nostr_pubkey_hex)
            .is_some_and(|target| {
                !target.normalized_payment_target().is_empty() && target.is_ldk_compatible()
            })
    }

    fn pylon_v0_2_registration_required_identity_count(&self) -> u64 {
        self.payout_targets_by_identity
            .values()
            .filter(|target| target.requires_v0_2_registration())
            .count() as u64
    }

    pub fn reconcile_ldk_channel_operations(
        &mut self,
        provider_channels: &[LdkServerChannel],
        now_unix_ms: u64,
    ) -> bool {
        let channel_status_by_hash = provider_channels
            .iter()
            .map(|channel| {
                (
                    treasury_hash(channel.channel_id.as_str()),
                    channel.status.as_str(),
                )
            })
            .collect::<BTreeMap<_, _>>();
        let mut changed = false;

        for operation in self.treasury_operations_by_id.values_mut() {
            if operation.rail != "ldk"
                || operation.kind != TreasuryOperationKind::LightningAdminCommand
                || operation.status != TreasuryOperationStatus::Pending
            {
                continue;
            }

            let Some(command) = operation.command() else {
                continue;
            };
            match command {
                "treasury.openChannel" => {
                    let age_ms = now_unix_ms.saturating_sub(operation.updated_at_unix_ms);
                    let Some(channel_id_hash) = operation.rail_metadata.get("channel_id_hash")
                    else {
                        continue;
                    };
                    match channel_status_by_hash.get(channel_id_hash).copied() {
                        Some("ready" | "usable") => {
                            operation.status = TreasuryOperationStatus::Completed;
                            operation.terminal_event_state = Some("channel_opened".to_string());
                            operation.updated_at_unix_ms = now_unix_ms;
                            changed = true;
                        }
                        Some(_) => {}
                        None if age_ms >= TREASURY_LDK_CHANNEL_OPEN_RECONCILE_GRACE_MS => {
                            operation.status = TreasuryOperationStatus::Failed;
                            operation.degraded_reason =
                                Some("ldk_channel_not_found_after_open_request".to_string());
                            operation.terminal_event_state =
                                Some("channel_open_failed".to_string());
                            operation.updated_at_unix_ms = now_unix_ms;
                            changed = true;
                        }
                        None => {}
                    }
                }
                "treasury.closeChannel" => {
                    let age_ms = now_unix_ms.saturating_sub(operation.updated_at_unix_ms);
                    let Some(channel_id_hash) = operation.target_hash.as_ref() else {
                        continue;
                    };
                    if age_ms >= TREASURY_LDK_CHANNEL_OPEN_RECONCILE_GRACE_MS
                        && !channel_status_by_hash.contains_key(channel_id_hash)
                    {
                        operation.status = TreasuryOperationStatus::Completed;
                        operation.terminal_event_state = Some("channel_closed".to_string());
                        operation.updated_at_unix_ms = now_unix_ms;
                        changed = true;
                    }
                }
                _ => {}
            }
        }

        if changed {
            self.persist();
        }
        changed
    }

    pub fn reconcile_ldk_provider_channels(
        &mut self,
        provider_channels: &[LdkServerChannel],
        now_unix_ms: u64,
    ) -> bool {
        let next_channels = provider_channels
            .iter()
            .map(|channel| {
                let channel_id_hash = treasury_hash(channel.channel_id.as_str());
                (
                    channel_id_hash.clone(),
                    TreasuryLdkProviderChannelSnapshot {
                        channel_id_hash,
                        peer_node_id_hash: treasury_hash(channel.peer_node_id.as_str()),
                        status: channel.status.clone(),
                        outbound_capacity_sats: channel.outbound_capacity_sats,
                        inbound_capacity_sats: channel.inbound_capacity_sats,
                        observed_at_unix_ms: now_unix_ms,
                    },
                )
            })
            .collect::<BTreeMap<_, _>>();

        if self.ldk_provider_channels_by_id_hash == next_channels {
            return false;
        }

        self.ldk_provider_channels_by_id_hash = next_channels;
        self.persist();
        true
    }

    fn ldk_channel_readiness(&self) -> TreasuryLdkChannelReadiness {
        if !self.ldk_provider_channels_by_id_hash.is_empty() {
            let mut projected_channel_count = 0u64;
            let mut projected_outbound_capacity_sats = 0u64;

            for channel in self.ldk_provider_channels_by_id_hash.values() {
                if !matches!(channel.status.as_str(), "ready" | "usable") {
                    continue;
                }
                if channel.outbound_capacity_sats == 0 {
                    continue;
                }
                projected_channel_count = projected_channel_count.saturating_add(1);
                projected_outbound_capacity_sats =
                    projected_outbound_capacity_sats.saturating_add(channel.outbound_capacity_sats);
            }

            return TreasuryLdkChannelReadiness {
                projected_channel_count,
                projected_inbound_capacity_sats: projected_outbound_capacity_sats,
                projected_outbound_capacity_sats,
            };
        }

        let mut projected_channel_count = 0u64;
        let mut projected_inbound_capacity_msat = 0u64;

        for operation in self.treasury_operations_by_id.values() {
            if operation.rail != "ldk"
                || operation.kind != TreasuryOperationKind::LightningAdminCommand
                || operation.status != TreasuryOperationStatus::Completed
            {
                continue;
            }

            match operation.command() {
                Some("treasury.openChannel") => {
                    projected_channel_count = projected_channel_count.saturating_add(1);
                    if let Some(amount_msat) = operation.amount_msat {
                        projected_inbound_capacity_msat =
                            projected_inbound_capacity_msat.saturating_add(amount_msat);
                    }
                }
                Some("treasury.spliceIn") => {
                    if let Some(amount_msat) = operation.amount_msat {
                        projected_inbound_capacity_msat =
                            projected_inbound_capacity_msat.saturating_add(amount_msat);
                    }
                }
                Some("treasury.spliceOut") => {
                    if let Some(amount_msat) = operation.amount_msat {
                        projected_inbound_capacity_msat =
                            projected_inbound_capacity_msat.saturating_sub(amount_msat);
                    }
                }
                Some("treasury.closeChannel") => {
                    projected_channel_count = projected_channel_count.saturating_sub(1);
                }
                _ => {}
            }
        }

        TreasuryLdkChannelReadiness {
            projected_channel_count,
            projected_inbound_capacity_sats: projected_inbound_capacity_msat / 1_000,
            projected_outbound_capacity_sats: projected_inbound_capacity_msat / 1_000,
        }
    }

    fn ldk_payment_failure_readiness(
        &self,
        now_unix_ms: u64,
    ) -> TreasuryLdkPaymentFailureReadiness {
        let window_started_at_unix_ms = now_unix_ms.saturating_sub(TREASURY_PUBLIC_STATS_WINDOW_MS);
        let mut readiness = TreasuryLdkPaymentFailureReadiness::default();

        for operation in self.treasury_operations_by_id.values() {
            if operation.rail != "ldk"
                || operation.status != TreasuryOperationStatus::Failed
                || operation.updated_at_unix_ms < window_started_at_unix_ms
            {
                continue;
            }

            readiness.recent_failed_payment_count_24h =
                readiness.recent_failed_payment_count_24h.saturating_add(1);
            let reason = operation.degraded_reason.as_deref().unwrap_or_default();
            if treasury_reason_indicates_no_route(reason) {
                readiness.recent_no_route_count_24h =
                    readiness.recent_no_route_count_24h.saturating_add(1);
            }
            if treasury_reason_indicates_insufficient_balance(reason) {
                readiness.recent_insufficient_balance_count_24h = readiness
                    .recent_insufficient_balance_count_24h
                    .saturating_add(1);
            }
        }

        readiness
    }

    fn ldk_readiness_snapshot(
        &self,
        config: &TreasuryConfig,
        now_unix_ms: u64,
        degraded_states: &[TreasuryDegradedState],
    ) -> TreasuryLdkReadinessSnapshot {
        let channel_readiness = self.ldk_channel_readiness();
        let failure_readiness = self.ldk_payment_failure_readiness(now_unix_ms);
        let registered_payout_target_count = self.ldk_payout_target_identity_count();
        let mut operator_actions = Vec::new();

        let state = if config.lightning_provider.provider != TreasuryLightningProviderKind::Ldk {
            operator_actions.push("configure Nexus with NEXUS_TREASURY_PROVIDER=ldk".to_string());
            "misconfigured"
        } else if !ldk_server_configured(config) {
            operator_actions
                .push("configure NEXUS_LDK_SERVER_URL, NEXUS_LDK_API_KEY_PATH, and NEXUS_LDK_TLS_CERT_PATH".to_string());
            "needs_ldk_server"
        } else if self.wallet_balance_sats == 0 {
            operator_actions
                .push("create and pay a Nexus LDK funding invoice before payout smoke".to_string());
            "needs_funding"
        } else if registered_payout_target_count == 0 {
            operator_actions.push(
                "register at least one Pylon v0.2 LDK payout target before payout smoke"
                    .to_string(),
            );
            "needs_payout_targets"
        } else if registered_payout_target_count > 0
            && (channel_readiness.projected_channel_count < config.ldk_min_ready_channel_count
                || channel_readiness.projected_outbound_capacity_sats
                    < config.ldk_min_ready_outbound_capacity_sats)
        {
            operator_actions.push(format!(
                "open or rebalance LDK channels until at least {} usable channel(s) and {} sats outbound capacity are available",
                config.ldk_min_ready_channel_count,
                config.ldk_min_ready_outbound_capacity_sats
            ));
            "needs_channels"
        } else if degraded_states
            .iter()
            .any(|state| state.severity == "critical")
        {
            operator_actions
                .push("resolve critical LDK degraded states before payout smoke".to_string());
            "degraded"
        } else if !degraded_states.is_empty() {
            operator_actions.push("review warning-level LDK degraded states".to_string());
            "attention"
        } else {
            "ready"
        };

        TreasuryLdkReadinessSnapshot {
            state: state.to_string(),
            registered_payout_target_count,
            projected_channel_count: channel_readiness.projected_channel_count,
            projected_inbound_capacity_sats: channel_readiness.projected_inbound_capacity_sats,
            projected_outbound_capacity_sats: channel_readiness.projected_outbound_capacity_sats,
            min_ready_channel_count: config.ldk_min_ready_channel_count,
            min_ready_outbound_capacity_sats: config.ldk_min_ready_outbound_capacity_sats,
            recent_failed_payment_count_24h: failure_readiness.recent_failed_payment_count_24h,
            recent_no_route_count_24h: failure_readiness.recent_no_route_count_24h,
            recent_insufficient_balance_count_24h: failure_readiness
                .recent_insufficient_balance_count_24h,
            operator_actions,
        }
    }

    fn validated_recovery_report_balance(&self, now_unix_ms: u64, max_age_ms: u64) -> Option<u64> {
        let summary = self.last_wallet_recovery_report.as_ref()?;
        if !summary.validation_passed || summary.major_divergence_detected {
            return None;
        }
        if now_unix_ms.saturating_sub(summary.generated_at_unix_ms) >= max_age_ms {
            return None;
        }
        summary.current_balance_sats.filter(|balance| *balance > 0)
    }

    fn wallet_error_covered_by_recovery_report(
        &self,
        detail: &str,
        config: &TreasuryConfig,
        now_unix_ms: u64,
    ) -> bool {
        detail.starts_with("wallet_hydration_zero_balance_after_")
            && self
                .validated_recovery_report_balance(
                    now_unix_ms,
                    config.wallet_snapshot_stale_after_ms(),
                )
                .is_some()
    }

    fn legacy_identity_scoped_availability_scope<'a>(
        record: &'a TreasuryPayoutRecord,
    ) -> Option<&'a str> {
        let (_, scope) = record.payout_key.split_once(':')?;
        if scope.starts_with("availability-beneficiary:")
            || scope.starts_with("availability-identity:")
        {
            return None;
        }
        Some(scope)
    }

    fn legacy_availability_confirmation_attention_record(
        &self,
        record: &TreasuryPayoutRecord,
        config: &TreasuryConfig,
        now_unix_ms: u64,
        policy: &TreasuryRuntimePolicy,
    ) -> bool {
        if record.classification.effective_payout_class()
            != TreasuryPayoutClass::PlaceholderLiveness
        {
            return false;
        }
        if !record.classification.continuity_alert_relevant(policy) {
            return false;
        }
        if !matches!(record.status.as_str(), "dispatching" | "dispatched") {
            return false;
        }
        if record.payment_id.is_none() {
            return false;
        }
        if Self::legacy_identity_scoped_availability_scope(record).is_none() {
            return false;
        }
        now_unix_ms.saturating_sub(record.updated_at_unix_ms)
            >= config
                .reconciliation_horizon_ms()
                .max(TREASURY_CONTINUITY_ALERT_THRESHOLD_MS)
    }

    fn inactive_availability_confirmation_record(
        &self,
        record: &TreasuryPayoutRecord,
        policy: &TreasuryRuntimePolicy,
    ) -> bool {
        policy.placeholder_payout_mode == TreasuryPlaceholderPayoutMode::Disabled
            && record.classification.effective_payout_class()
                == TreasuryPayoutClass::PlaceholderLiveness
            && matches!(record.status.as_str(), "dispatching" | "dispatched")
            && record.payment_id.is_some()
    }

    fn legacy_availability_confirmation_attention_rows(
        &self,
        config: &TreasuryConfig,
        now_unix_ms: u64,
    ) -> Vec<TreasuryLegacyAvailabilityAttentionRow> {
        let policy = self.active_policy(config);
        let mut rows = self
            .payout_records_by_key
            .values()
            .filter(|record| {
                self.legacy_availability_confirmation_attention_record(
                    record,
                    config,
                    now_unix_ms,
                    &policy,
                )
            })
            .map(|record| TreasuryLegacyAvailabilityAttentionRow {
                payout_key: record.payout_key.clone(),
                nostr_pubkey_hex: record.nostr_pubkey_hex.clone(),
                payout_target: record.payout_target.clone(),
                status: record.status.clone(),
                payment_id: record.payment_id.clone(),
                window_started_at_unix_ms: record.window_started_at_unix_ms,
                updated_at_unix_ms: record.updated_at_unix_ms,
                age_ms: now_unix_ms.saturating_sub(record.updated_at_unix_ms),
            })
            .collect::<Vec<_>>();

        rows.sort_by(|left, right| {
            right
                .updated_at_unix_ms
                .cmp(&left.updated_at_unix_ms)
                .then_with(|| left.payout_key.cmp(&right.payout_key))
        });
        rows.truncate(TREASURY_STATUS_LEGACY_AVAILABILITY_ATTENTION_LIMIT);
        rows
    }

    fn availability_existing_payout_key_scope(
        disposition: &AvailabilityIdentityDisposition,
    ) -> String {
        disposition
            .beneficiary
            .as_ref()
            .map(|beneficiary| format!("availability-beneficiary:{}", beneficiary.key))
            .unwrap_or_else(|| {
                format!(
                    "availability-identity:{}",
                    disposition.identity.nostr_pubkey_hex
                )
            })
    }

    fn availability_disposition_payout_key_scope(
        disposition: &AvailabilityIdentityDisposition,
    ) -> String {
        if disposition.allowed() {
            disposition
                .beneficiary
                .as_ref()
                .map(|beneficiary| format!("availability-beneficiary:{}", beneficiary.key))
                .unwrap_or_else(|| {
                    format!(
                        "availability-identity:{}",
                        disposition.identity.nostr_pubkey_hex
                    )
                })
        } else {
            format!(
                "availability-identity:{}",
                disposition.identity.nostr_pubkey_hex
            )
        }
    }

    pub fn new(state_path: PathBuf) -> Self {
        let mut loaded = match fs::read_to_string(state_path.as_path()) {
            Ok(payload) => match serde_json::from_str::<Self>(payload.as_str()) {
                Ok(state) => state,
                Err(error) => recovered_treasury_state_from_payload(payload.as_str(), &error),
            },
            Err(_) => Self::default(),
        };
        let mut changed = false;
        if loaded.next_challenge_nonce == 0 {
            loaded.next_challenge_nonce = 1;
            changed = true;
        }
        changed |= loaded.normalize_legacy_payout_classes();
        changed |= loaded.retire_unpayable_pending_payout_records(now_unix_ms());
        changed |= loaded.backfill_classified_payout_totals();
        loaded.public_snapshot = None;
        changed |= loaded.trim_policy_change_history();
        changed |= loaded.migrate_legacy_payout_records(now_unix_ms());
        changed |= loaded.trim_retention(now_unix_ms());
        loaded.rebuild_payment_index();
        loaded.state_path = Some(state_path);
        if changed {
            loaded.persist();
        }
        loaded
    }

    fn upsert_treasury_operation(&mut self, mut operation: TreasuryOperationRecord) -> bool {
        if let Some(existing) = self
            .treasury_operations_by_id
            .get(operation.operation_id.as_str())
        {
            operation.created_at_unix_ms = existing.created_at_unix_ms;
            for receipt_ref in &existing.receipt_refs {
                if !operation.receipt_refs.contains(receipt_ref) {
                    operation.receipt_refs.push(receipt_ref.clone());
                }
            }
            operation.receipt_refs.sort();
            operation.receipt_refs.dedup();
            if existing == &operation {
                return false;
            }
        }
        self.treasury_operations_by_id
            .insert(operation.operation_id.clone(), operation);
        true
    }

    pub fn attach_receipt_reference_for_request(
        &mut self,
        request_id: &str,
        receipt_id: &str,
        now_unix_ms: u64,
    ) -> bool {
        let mut changed = false;
        for operation in self
            .treasury_operations_by_id
            .values_mut()
            .filter(|operation| {
                operation.request_id.as_deref() == Some(request_id)
                    || operation.operation_id == request_id
            })
        {
            if !operation
                .receipt_refs
                .iter()
                .any(|entry| entry == receipt_id)
            {
                operation.receipt_refs.push(receipt_id.to_string());
                operation.receipt_refs.sort();
                operation.receipt_refs.dedup();
                operation.updated_at_unix_ms = operation.updated_at_unix_ms.max(now_unix_ms);
                changed = true;
            }
        }
        if changed {
            self.persist();
        }
        changed
    }

    pub fn record_event_projection_operation(
        &mut self,
        receipt_type: &str,
        receipt_id: &str,
        related_request_id: Option<&str>,
        now_unix_ms: u64,
    ) -> bool {
        let request_id = format!("event-projection:{receipt_id}");
        let mut rail_metadata = BTreeMap::new();
        rail_metadata.insert("receipt_type".to_string(), receipt_type.to_string());
        if let Some(related_request_id) = related_request_id {
            rail_metadata.insert(
                "related_request_id".to_string(),
                related_request_id.to_string(),
            );
        }
        let operation = TreasuryOperationRecord {
            operation_id: treasury_operation_id(
                TreasuryOperationKind::EventProjection,
                &request_id,
            ),
            kind: TreasuryOperationKind::EventProjection,
            request_id: Some(request_id),
            rail: "receipt_ledger".to_string(),
            rail_metadata,
            amount_msat: None,
            target_kind: "authority_receipt".to_string(),
            target_hash: Some(treasury_hash(receipt_id)),
            beneficiary: None,
            status: TreasuryOperationStatus::Completed,
            provider_payment_id: None,
            receipt_refs: vec![receipt_id.to_string()],
            degraded_reason: None,
            created_at_unix_ms: now_unix_ms,
            updated_at_unix_ms: now_unix_ms,
            terminal_event_state: Some("recorded".to_string()),
        };
        let changed = self.upsert_treasury_operation(operation);
        if changed {
            self.persist();
        }
        changed
    }

    pub fn record_funding_invoice_created_operation(
        &mut self,
        config: &TreasuryConfig,
        request: &TreasuryFundingTargetRequest,
        material: &TreasuryFundingMaterial,
        now_unix_ms: u64,
    ) -> Vec<TreasuryReceiptEvent> {
        let request_id = funding_idempotency_key(request);
        let target = material
            .bolt11_invoice
            .as_deref()
            .unwrap_or(material.provider_target.as_str());
        let target_kind = if material.bolt11_invoice.is_some() {
            "bolt11_invoice"
        } else {
            "provider_target"
        };
        let mut rail_metadata = BTreeMap::new();
        rail_metadata.insert(
            "provider".to_string(),
            config.lightning_provider.provider.as_str().to_string(),
        );
        rail_metadata.insert(
            "ldk_network".to_string(),
            config.lightning_provider.ldk.network.as_str().to_string(),
        );
        rail_metadata.insert(
            "ldk_chain_backend".to_string(),
            config
                .lightning_provider
                .ldk
                .chain_backend
                .as_str()
                .to_string(),
        );
        rail_metadata.insert(
            "has_bolt11_invoice".to_string(),
            material.bolt11_invoice.is_some().to_string(),
        );
        material
            .phase_timings
            .insert_rail_metadata(&mut rail_metadata);
        let target_hash = treasury_hash(target);
        let operation_id =
            treasury_operation_id(TreasuryOperationKind::FundingInvoiceCreation, &request_id);
        let operation = TreasuryOperationRecord {
            operation_id: operation_id.clone(),
            kind: TreasuryOperationKind::FundingInvoiceCreation,
            request_id: Some(request_id.clone()),
            rail: operation_rail_for_provider(config.lightning_provider.provider).to_string(),
            rail_metadata,
            amount_msat: request.amount_sats.and_then(operation_amount_msat),
            target_kind: target_kind.to_string(),
            target_hash: Some(target_hash.clone()),
            beneficiary: None,
            status: TreasuryOperationStatus::Completed,
            provider_payment_id: material
                .provider_payment_id
                .as_ref()
                .map(|payment_id| treasury_hash(payment_id.as_str()))
                .or_else(|| {
                    material
                        .provider_invoice
                        .as_ref()
                        .map(|invoice| treasury_hash(invoice.as_str()))
                })
                .or_else(|| Some(target_hash.clone())),
            receipt_refs: Vec::new(),
            degraded_reason: None,
            created_at_unix_ms: now_unix_ms,
            updated_at_unix_ms: now_unix_ms,
            terminal_event_state: Some("invoice_created".to_string()),
        };
        let changed = self.upsert_treasury_operation(operation);
        if changed {
            self.persist();
        }

        let mut attributes = BTreeMap::new();
        attributes.insert("operation_id".to_string(), operation_id);
        attributes.insert(
            "provider".to_string(),
            config.lightning_provider.provider.as_str().to_string(),
        );
        attributes.insert(
            "rail".to_string(),
            operation_rail_for_provider(config.lightning_provider.provider).to_string(),
        );
        attributes.insert("target_kind".to_string(), target_kind.to_string());
        attributes.insert("target_hash".to_string(), target_hash);
        if let Some(duration_ms) = material.phase_timings.ldk_rpc_duration_ms() {
            attributes.insert(
                "phase_ldk_rpc_duration_ms".to_string(),
                duration_ms.to_string(),
            );
        }
        if let Some(duration_ms) = material.phase_timings.total_duration_ms() {
            attributes.insert(
                "phase_total_duration_ms".to_string(),
                duration_ms.to_string(),
            );
        }
        vec![TreasuryReceiptEvent {
            receipt_type: "treasury.funding_invoice.created",
            context: AuthorityReceiptContext {
                request_id: Some(request_id),
                status: Some(TreasuryOperationStatus::Completed.as_str().to_string()),
                amount_sats: request.amount_sats,
                attributes,
                ..AuthorityReceiptContext::default()
            },
        }]
    }

    fn record_reconciliation_operation(
        &mut self,
        config: &TreasuryConfig,
        now_unix_ms: u64,
        degraded_reason: Option<String>,
    ) -> bool {
        let request_id = format!("reconciliation:{now_unix_ms}");
        let mut rail_metadata = BTreeMap::new();
        rail_metadata.insert(
            "provider".to_string(),
            config.lightning_provider.provider.as_str().to_string(),
        );
        let operation = TreasuryOperationRecord {
            operation_id: treasury_operation_id(
                TreasuryOperationKind::ReconciliationPass,
                &request_id,
            ),
            kind: TreasuryOperationKind::ReconciliationPass,
            request_id: Some(request_id),
            rail: operation_rail_for_provider(config.lightning_provider.provider).to_string(),
            rail_metadata,
            amount_msat: None,
            target_kind: "treasury_state".to_string(),
            target_hash: None,
            beneficiary: None,
            status: if degraded_reason.is_some() {
                TreasuryOperationStatus::Degraded
            } else {
                TreasuryOperationStatus::Completed
            },
            provider_payment_id: None,
            receipt_refs: Vec::new(),
            degraded_reason,
            created_at_unix_ms: now_unix_ms,
            updated_at_unix_ms: now_unix_ms,
            terminal_event_state: Some("projection_checked".to_string()),
        };
        self.upsert_treasury_operation(operation)
    }

    pub fn get_treasury_admin_operation(
        &self,
        command: &str,
        idempotency_key: &str,
    ) -> Option<&TreasuryOperationRecord> {
        let operation_id = treasury_admin_operation_id(command, idempotency_key);
        self.treasury_operations_by_id.get(operation_id.as_str())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn record_treasury_admin_operation(
        &mut self,
        command: &str,
        idempotency_key: &str,
        rail_metadata: BTreeMap<String, String>,
        amount_msat: Option<u64>,
        target_kind: &str,
        target_hash: Option<String>,
        status: TreasuryOperationStatus,
        provider_payment_id: Option<String>,
        degraded_reason: Option<String>,
        terminal_event_state: Option<String>,
        now_unix_ms: u64,
    ) -> TreasuryOperationRecord {
        let request_id = treasury_admin_operation_request_id(command, idempotency_key);
        let operation = TreasuryOperationRecord {
            operation_id: treasury_operation_id(
                TreasuryOperationKind::LightningAdminCommand,
                request_id.as_str(),
            ),
            kind: TreasuryOperationKind::LightningAdminCommand,
            request_id: Some(request_id),
            rail: "ldk".to_string(),
            rail_metadata,
            amount_msat,
            target_kind: target_kind.to_string(),
            target_hash,
            beneficiary: None,
            status,
            provider_payment_id,
            receipt_refs: Vec::new(),
            degraded_reason,
            created_at_unix_ms: now_unix_ms,
            updated_at_unix_ms: now_unix_ms,
            terminal_event_state,
        };
        let _ = self.upsert_treasury_operation(operation.clone());
        self.persist();
        operation
    }

    fn record_payment_status_lookup_operation(
        &mut self,
        payment: &PaymentSummary,
        wallet_hydration_mode: Option<&str>,
        now_unix_ms: u64,
    ) -> bool {
        let _ = wallet_hydration_mode;
        let provider = TreasuryLightningProviderKind::Ldk;
        let request_id = format!("payment-status:{}", payment.id);
        let mut rail_metadata = BTreeMap::new();
        rail_metadata.insert("payment_direction".to_string(), payment.direction.clone());
        rail_metadata.insert("payment_method".to_string(), payment.method.clone());
        let status = operation_status_for_payment_status(payment.status.as_str());
        let operation = TreasuryOperationRecord {
            operation_id: treasury_operation_id(
                TreasuryOperationKind::PaymentStatusLookup,
                &request_id,
            ),
            kind: TreasuryOperationKind::PaymentStatusLookup,
            request_id: Some(request_id),
            rail: operation_rail_for_provider(provider).to_string(),
            rail_metadata,
            amount_msat: operation_amount_msat(payment.amount_sats),
            target_kind: "provider_payment".to_string(),
            target_hash: Some(treasury_hash(payment.id.as_str())),
            beneficiary: None,
            status,
            provider_payment_id: Some(treasury_hash(payment.id.as_str())),
            receipt_refs: Vec::new(),
            degraded_reason: payment.status_detail.clone(),
            created_at_unix_ms: payment.timestamp.saturating_mul(1_000),
            updated_at_unix_ms: now_unix_ms,
            terminal_event_state: matches!(
                status,
                TreasuryOperationStatus::Completed | TreasuryOperationStatus::Failed
            )
            .then(|| payment.status.clone()),
        };
        self.upsert_treasury_operation(operation)
    }

    fn update_payout_operation_status(
        &mut self,
        payout_key: &str,
        status: TreasuryOperationStatus,
        provider_payment_id: Option<String>,
        degraded_reason: Option<String>,
        terminal_event_state: Option<String>,
        now_unix_ms: u64,
    ) -> bool {
        let operation_id =
            treasury_operation_id(TreasuryOperationKind::OutboundPayoutDispatch, payout_key);
        let Some(operation) = self
            .treasury_operations_by_id
            .get_mut(operation_id.as_str())
        else {
            return false;
        };
        let mut changed = false;
        if operation.status != status {
            operation.status = status;
            changed = true;
        }
        if operation.provider_payment_id != provider_payment_id {
            operation.provider_payment_id = provider_payment_id;
            changed = true;
        }
        if operation.degraded_reason != degraded_reason {
            operation.degraded_reason = degraded_reason;
            changed = true;
        }
        if operation.terminal_event_state != terminal_event_state {
            operation.terminal_event_state = terminal_event_state;
            changed = true;
        }
        if changed {
            operation.updated_at_unix_ms = now_unix_ms;
        }
        changed
    }

    fn migrate_legacy_payout_records(&mut self, now_unix_ms: u64) -> bool {
        let mut changed = false;
        let records = self
            .payout_records_by_key
            .values()
            .filter(|record| {
                !record.payout_target.is_empty()
                    || record.payment_id.is_some()
                    || matches!(
                        record.status.as_str(),
                        "dispatching" | "dispatched" | "confirmed" | "failed"
                    )
            })
            .cloned()
            .collect::<Vec<_>>();
        for record in records {
            let status = match record.status.as_str() {
                "confirmed" => TreasuryOperationStatus::Completed,
                "failed" => TreasuryOperationStatus::Failed,
                "skipped" => TreasuryOperationStatus::Degraded,
                _ => TreasuryOperationStatus::Pending,
            };
            let mut rail_metadata = BTreeMap::new();
            rail_metadata.insert("provider".to_string(), "retired_legacy_record".to_string());
            rail_metadata.insert(
                "migrated_from".to_string(),
                "legacy_payout_record".to_string(),
            );
            rail_metadata.insert(
                "payout_class".to_string(),
                record
                    .classification
                    .effective_payout_class()
                    .label()
                    .to_string(),
            );
            let operation = TreasuryOperationRecord {
                operation_id: treasury_operation_id(
                    TreasuryOperationKind::OutboundPayoutDispatch,
                    record.payout_key.as_str(),
                ),
                kind: TreasuryOperationKind::OutboundPayoutDispatch,
                request_id: Some(record.payout_key.clone()),
                rail: "retired_payout_record".to_string(),
                rail_metadata,
                amount_msat: operation_amount_msat(record.amount_sats),
                target_kind: "legacy_payout_target".to_string(),
                target_hash: (!record.payout_target.trim().is_empty())
                    .then(|| treasury_hash(record.payout_target.as_str())),
                beneficiary: Some(record.nostr_pubkey_hex.clone()),
                status,
                provider_payment_id: record
                    .payment_id
                    .as_ref()
                    .map(|id| treasury_hash(id.as_str())),
                receipt_refs: Vec::new(),
                degraded_reason: record.reason.clone(),
                created_at_unix_ms: record.created_at_unix_ms,
                updated_at_unix_ms: record.updated_at_unix_ms.max(now_unix_ms),
                terminal_event_state: matches!(
                    status,
                    TreasuryOperationStatus::Completed | TreasuryOperationStatus::Failed
                )
                .then(|| record.status.clone()),
            };
            changed |= self.upsert_treasury_operation(operation);
        }
        changed
    }

    pub fn apply_paid_total_floor(&mut self, payout_sats_paid_total_floor: u64) -> Option<u64> {
        if payout_sats_paid_total_floor <= self.payout_sats_paid_total {
            return None;
        }
        let previous_total = self.payout_sats_paid_total;
        self.payout_sats_paid_total = payout_sats_paid_total_floor;
        if self.accepted_work_payout_sats_paid_total == 0
            && self.beta_bonus_payout_sats_paid_total == 0
            && self.placeholder_payout_sats_paid_total <= previous_total
        {
            self.placeholder_payout_sats_paid_total = payout_sats_paid_total_floor;
        }
        if let Some(snapshot) = self.public_snapshot.as_mut() {
            snapshot.payout_sats_paid_total = snapshot
                .payout_sats_paid_total
                .max(payout_sats_paid_total_floor);
            if snapshot.accepted_work_payout_sats_paid_total == 0
                && snapshot.beta_bonus_payout_sats_paid_total == 0
            {
                snapshot.placeholder_payout_sats_paid_total = snapshot
                    .placeholder_payout_sats_paid_total
                    .max(payout_sats_paid_total_floor);
            }
        }
        self.persist();
        Some(previous_total)
    }

    fn backfill_classified_payout_totals(&mut self) -> bool {
        let computed = self.computed_classified_paid_totals_from_records();
        let current_zero = self.accepted_work_payout_sats_paid_total == 0
            && self.beta_bonus_payout_sats_paid_total == 0
            && self.placeholder_payout_sats_paid_total == 0
            && self.weak_device_accepted_work_payout_sats_paid_total == 0
            && self.strong_lane_accepted_work_payout_sats_paid_total == 0;
        let legacy_placeholder_only = self.payout_sats_paid_total > 0
            && self.accepted_work_payout_sats_paid_total == 0
            && self.beta_bonus_payout_sats_paid_total == 0
            && self.weak_device_accepted_work_payout_sats_paid_total == 0
            && self.strong_lane_accepted_work_payout_sats_paid_total == 0
            && self.placeholder_payout_sats_paid_total == self.payout_sats_paid_total;
        let target_total = self
            .payout_sats_paid_total
            .max(computed.payout_sats_paid_total);
        let mut changed = false;
        if self.payout_sats_paid_total != target_total {
            self.payout_sats_paid_total = target_total;
            changed = true;
        }

        if !(current_zero || legacy_placeholder_only) {
            if self.weak_device_accepted_work_payout_sats_paid_total
                < computed.weak_device_accepted_work_payout_sats_paid_total
            {
                self.weak_device_accepted_work_payout_sats_paid_total =
                    computed.weak_device_accepted_work_payout_sats_paid_total;
                changed = true;
            }
            if self.strong_lane_accepted_work_payout_sats_paid_total
                < computed.strong_lane_accepted_work_payout_sats_paid_total
            {
                self.strong_lane_accepted_work_payout_sats_paid_total =
                    computed.strong_lane_accepted_work_payout_sats_paid_total;
                changed = true;
            }
            return changed;
        }

        let mut next = computed;
        if target_total > next.payout_sats_paid_total {
            let residual = target_total.saturating_sub(next.payout_sats_paid_total);
            next.payout_sats_paid_total = target_total;
            next.placeholder_payout_sats_paid_total = next
                .placeholder_payout_sats_paid_total
                .saturating_add(residual);
        }

        if self.accepted_work_payout_sats_paid_total != next.accepted_work_payout_sats_paid_total {
            self.accepted_work_payout_sats_paid_total = next.accepted_work_payout_sats_paid_total;
            changed = true;
        }
        if self.placeholder_payout_sats_paid_total != next.placeholder_payout_sats_paid_total {
            self.placeholder_payout_sats_paid_total = next.placeholder_payout_sats_paid_total;
            changed = true;
        }
        if self.beta_bonus_payout_sats_paid_total != next.beta_bonus_payout_sats_paid_total {
            self.beta_bonus_payout_sats_paid_total = next.beta_bonus_payout_sats_paid_total;
            changed = true;
        }
        if self.weak_device_accepted_work_payout_sats_paid_total
            != next.weak_device_accepted_work_payout_sats_paid_total
        {
            self.weak_device_accepted_work_payout_sats_paid_total =
                next.weak_device_accepted_work_payout_sats_paid_total;
            changed = true;
        }
        if self.strong_lane_accepted_work_payout_sats_paid_total
            != next.strong_lane_accepted_work_payout_sats_paid_total
        {
            self.strong_lane_accepted_work_payout_sats_paid_total =
                next.strong_lane_accepted_work_payout_sats_paid_total;
            changed = true;
        }
        changed
    }

    fn computed_classified_paid_totals_from_records(&self) -> TreasuryPayoutTotals {
        let mut totals = TreasuryPayoutTotals::default();
        for record in self
            .payout_records_by_key
            .values()
            .filter(|record| record.counted_in_paid_total && record.status == "confirmed")
        {
            totals.add_amount(record.amount_sats, &record.classification);
        }
        totals
    }

    fn normalize_legacy_payout_classes(&mut self) -> bool {
        let mut changed = false;
        for record in self.payout_records_by_key.values_mut() {
            changed |= record.classification.normalize_legacy_payout_class();
        }
        changed
    }

    fn retire_unpayable_pending_payout_records(&mut self, now_unix_ms: u64) -> bool {
        let payout_keys = self
            .payout_records_by_key
            .values()
            .filter(|record| matches!(record.status.as_str(), "dispatching" | "dispatched"))
            .filter(|record| !payout_record_has_ldk_target(record))
            .map(|record| record.payout_key.clone())
            .collect::<Vec<_>>();
        self.retire_unpayable_payout_keys(payout_keys, now_unix_ms)
            .0
    }

    fn retire_unpayable_historical_payout_records(
        &mut self,
        now_unix_ms: u64,
    ) -> (bool, Vec<TreasuryPayoutLedgerCleanupRetiredRecord>) {
        let payout_keys = self
            .payout_records_by_key
            .values()
            .filter(|record| payout_record_should_be_retired_as_historical(record))
            .map(|record| record.payout_key.clone())
            .collect::<Vec<_>>();
        self.retire_unpayable_payout_keys(payout_keys, now_unix_ms)
    }

    fn retire_unpayable_payout_keys(
        &mut self,
        payout_keys: Vec<String>,
        now_unix_ms: u64,
    ) -> (bool, Vec<TreasuryPayoutLedgerCleanupRetiredRecord>) {
        let reason = TREASURY_RETIRED_UNPAYABLE_PAYOUT_REASON.to_string();
        let mut changed = false;
        let mut retired_records = Vec::new();
        for payout_key in payout_keys {
            let (provider_payment_id, retired_record) = {
                let Some(record) = self.payout_records_by_key.get_mut(payout_key.as_str()) else {
                    continue;
                };
                let retired_record = TreasuryPayoutLedgerCleanupRetiredRecord {
                    payout_key: record.payout_key.clone(),
                    previous_status: record.status.clone(),
                    previous_reason: record.reason.clone(),
                    payout_rail: payout_rail_for_payment_request(record.payout_target.as_str())
                        .to_string(),
                    payout_class: record
                        .classification
                        .effective_payout_class()
                        .label()
                        .to_string(),
                    amount_sats: record.amount_sats,
                };
                let mut record_changed = false;
                let provider_payment_id = record.payment_id.as_deref().map(treasury_hash);
                if record.status != "failed" {
                    record.status = "failed".to_string();
                    record_changed = true;
                }
                if record.reason.as_deref() != Some(reason.as_str()) {
                    record.reason = Some(reason.clone());
                    record_changed = true;
                }
                if !record.fail_receipt_recorded {
                    record.fail_receipt_recorded = true;
                    record_changed = true;
                }
                if record.counted_in_paid_total {
                    record.counted_in_paid_total = false;
                    record_changed = true;
                }
                if record_changed {
                    record.updated_at_unix_ms = now_unix_ms;
                }
                changed |= record_changed;
                (provider_payment_id, retired_record)
            };
            retired_records.push(retired_record);
            changed |= self.update_payout_operation_status(
                payout_key.as_str(),
                TreasuryOperationStatus::Failed,
                provider_payment_id,
                Some(reason.clone()),
                Some("failed".to_string()),
                now_unix_ms,
            );
        }
        (changed, retired_records)
    }

    fn cumulative_payout_totals(&self) -> TreasuryPayoutTotals {
        TreasuryPayoutTotals {
            payout_sats_paid_total: self.payout_sats_paid_total,
            accepted_work_payout_sats_paid_total: self.accepted_work_payout_sats_paid_total,
            placeholder_payout_sats_paid_total: self.placeholder_payout_sats_paid_total,
            beta_bonus_payout_sats_paid_total: self.beta_bonus_payout_sats_paid_total,
            weak_device_accepted_work_payout_sats_paid_total: self
                .weak_device_accepted_work_payout_sats_paid_total,
            strong_lane_accepted_work_payout_sats_paid_total: self
                .strong_lane_accepted_work_payout_sats_paid_total,
        }
    }

    pub fn queue_payout_requests(
        &mut self,
        config: &TreasuryConfig,
        requests: &[TreasuryQueuedPayoutRequest],
        now_unix_ms: u64,
    ) {
        let mut inserted = false;
        for request in requests {
            if self
                .payout_records_by_key
                .contains_key(request.payout_key.as_str())
            {
                continue;
            }
            inserted = true;
            let skipped = request.queue_block_reason.is_some();
            self.payout_records_by_key.insert(
                request.payout_key.clone(),
                TreasuryPayoutRecord {
                    payout_key: request.payout_key.clone(),
                    nostr_pubkey_hex: request.nostr_pubkey_hex.clone(),
                    payout_target: String::new(),
                    amount_sats: request.amount_sats,
                    status: if skipped {
                        "skipped".to_string()
                    } else {
                        "queued".to_string()
                    },
                    reason: request.queue_block_reason.clone(),
                    payment_id: None,
                    window_started_at_unix_ms: request.window_started_at_unix_ms,
                    window_ends_at_unix_ms: request.window_ends_at_unix_ms,
                    created_at_unix_ms: now_unix_ms,
                    updated_at_unix_ms: now_unix_ms,
                    sellable_at_window_open: true,
                    dispatch_receipt_recorded: false,
                    confirm_receipt_recorded: false,
                    fail_receipt_recorded: false,
                    skip_receipt_recorded: skipped,
                    counted_in_paid_total: false,
                    classification: request.classification.clone(),
                },
            );
        }
        if inserted {
            self.refresh_public_snapshot(config, now_unix_ms);
        } else {
            self.refresh_public_snapshot_in_memory(config, now_unix_ms);
        }
    }

    pub fn initialize_runtime_policy(
        &mut self,
        config: &TreasuryConfig,
        now_unix_ms: u64,
    ) -> Vec<TreasuryReceiptEvent> {
        let requested_policy = TreasuryRuntimePolicy::from_config(config);
        let Some(active_policy) = self.active_policy.clone() else {
            let bootstrap_record = build_treasury_policy_change_record(
                None,
                &requested_policy,
                "bootstrap_env",
                "bootstrap_env",
                now_unix_ms,
            );
            self.active_policy = Some(requested_policy.clone());
            self.policy_change_history.push(bootstrap_record);
            self.trim_policy_change_history();
            self.policy_runtime_status = Some("bootstrapped".to_string());
            self.policy_last_error = None;
            self.refresh_public_snapshot(config, now_unix_ms);
            return Vec::new();
        };

        if active_policy == requested_policy {
            self.policy_runtime_status = Some("persisted".to_string());
            self.policy_last_error = None;
            self.refresh_public_snapshot(config, now_unix_ms);
            return Vec::new();
        }

        if !config.apply_env_policy {
            self.policy_runtime_status = Some("persisted".to_string());
            self.policy_last_error = None;
            self.refresh_public_snapshot(config, now_unix_ms);
            return Vec::new();
        }

        let changed_fields = treasury_policy_changed_fields(&active_policy, &requested_policy);
        let destructive = treasury_policy_change_is_destructive(&active_policy, &requested_policy);
        if destructive && !config.allow_destructive_env_policy_change {
            self.policy_runtime_status = Some("blocked".to_string());
            self.policy_last_error =
                Some("destructive_policy_change_requires_explicit_override".to_string());
            self.refresh_public_snapshot(config, now_unix_ms);
            return vec![treasury_policy_change_blocked_receipt(
                &active_policy,
                &requested_policy,
                changed_fields.as_slice(),
                now_unix_ms,
            )];
        }

        let reason = config
            .policy_change_reason
            .clone()
            .unwrap_or_else(|| "env_override".to_string());
        let change_record = build_treasury_policy_change_record(
            Some(&active_policy),
            &requested_policy,
            "env_apply",
            reason.as_str(),
            now_unix_ms,
        );
        self.active_policy = Some(requested_policy.clone());
        self.policy_change_history.push(change_record.clone());
        self.trim_policy_change_history();
        self.policy_runtime_status = Some("updated".to_string());
        self.policy_last_error = None;
        self.refresh_public_snapshot(config, now_unix_ms);
        vec![treasury_policy_change_receipt(&change_record)]
    }

    fn active_policy(&self, config: &TreasuryConfig) -> TreasuryRuntimePolicy {
        self.active_policy
            .clone()
            .map(|policy| policy.with_resolved_legacy_defaults(config))
            .unwrap_or_else(|| TreasuryRuntimePolicy::from_config(config))
    }

    pub fn treasury_enabled(&self, config: &TreasuryConfig) -> bool {
        self.active_policy(config).treasury_enabled
    }

    pub fn wallet_refresh_due(&self, config: &TreasuryConfig, now_unix_ms: u64) -> bool {
        if self.wallet_refresh_requires_leaf_selection_recovery() {
            return true;
        }
        self.last_wallet_sync_at_unix_ms
            .max(self.last_wallet_refresh_attempt_at_unix_ms)
            .is_none_or(|last_refresh| {
                now_unix_ms.saturating_sub(last_refresh)
                    >= config.wallet_status_refresh_interval_ms()
            })
    }

    pub fn due_wallet_refresh_requires_reconciliation(&self) -> bool {
        self.payout_records_by_key.values().any(|record| {
            record.status == "dispatched"
                && !record.counted_in_paid_total
                && record.payment_id.is_some()
        }) || self.payout_records_by_key.values().any(|record| {
            record.status == "queued"
                && record.reason.as_deref() == Some("wallet_balance_insufficient")
        }) || self.payout_records_by_key.values().any(|record| {
            record.status == "failed"
                && record.payment_id.is_none()
                && record.reason.as_deref() == Some("wallet_balance_insufficient")
        }) || self.payout_records_by_key.values().any(|record| {
            record.status == "failed"
                && record.payment_id.is_none()
                && record
                    .reason
                    .as_deref()
                    .is_some_and(wallet_send_failure_is_leaf_selection)
        })
    }

    fn wallet_refresh_requires_leaf_selection_recovery(&self) -> bool {
        self.wallet_last_error
            .as_deref()
            .is_some_and(wallet_send_failure_is_leaf_selection)
            && self.due_wallet_refresh_requires_reconciliation()
    }

    fn wallet_dispatch_suppression_reason(&self) -> Option<String> {
        if self.wallet_refresh_requires_leaf_selection_recovery() {
            return Some("wallet_spendability_blocked:leaf_selection".to_string());
        }
        None
    }

    fn availability_beneficiary_projection(
        policy: &TreasuryRuntimePolicy,
        identity: &OnlinePylonIdentity,
        payout_target: &RegisteredPayoutTarget,
    ) -> AvailabilityBeneficiaryProjection {
        if policy.dedupe_placeholder_hosts {
            if let Some(host_fingerprint) = identity.host_fingerprint.as_ref() {
                return AvailabilityBeneficiaryProjection {
                    kind: AvailabilityBeneficiaryKind::HostCluster,
                    key: format!("host:{host_fingerprint}"),
                };
            }
        }
        if !payout_target.normalized_payment_target().is_empty() {
            return AvailabilityBeneficiaryProjection {
                kind: AvailabilityBeneficiaryKind::PayoutTarget,
                key: format!(
                    "payout_target:{}:{}",
                    payout_target.normalized_payment_target_kind(),
                    payout_target.normalized_payment_target()
                ),
            };
        }
        AvailabilityBeneficiaryProjection {
            kind: AvailabilityBeneficiaryKind::Identity,
            key: format!("identity:{}", identity.nostr_pubkey_hex),
        }
    }

    fn availability_identity_dispositions(
        &self,
        policy: &TreasuryRuntimePolicy,
        online_identities: &[OnlinePylonIdentity],
        now_unix_ms: u64,
    ) -> Vec<AvailabilityIdentityDisposition> {
        let payout_interval_ms = policy.payout_interval_ms();
        let mut dispositions = online_identities
            .iter()
            .cloned()
            .map(|identity| AvailabilityIdentityDisposition {
                identity,
                payout_target: None,
                beneficiary: None,
                current_window_started_at_unix_ms: None,
                verdict_reason: None,
            })
            .collect::<Vec<_>>();
        let mut candidate_indexes_by_primary_key = BTreeMap::<String, Vec<usize>>::new();

        for (index, disposition) in dispositions.iter_mut().enumerate() {
            let identity = &disposition.identity;
            if policy.require_sellable && !identity.sellable {
                disposition.verdict_reason = Some("requires_sellable_supply".to_string());
                continue;
            }

            let Some(target) = self
                .payout_targets_by_identity
                .get(identity.nostr_pubkey_hex.as_str())
                .cloned()
            else {
                disposition.verdict_reason = Some("missing_payout_target".to_string());
                continue;
            };
            if target.normalized_payment_target().is_empty() {
                disposition.verdict_reason = Some("missing_payout_target".to_string());
                continue;
            }
            if !target.is_ldk_compatible() {
                disposition.verdict_reason = Some("payout_target_requires_ldk_v0_2".to_string());
                continue;
            }
            disposition.payout_target = Some(target.clone());

            if let Some(reason) = policy.availability_stipend_base_skip_reason(identity) {
                disposition.verdict_reason = Some(reason);
                continue;
            }

            let beneficiary = Self::availability_beneficiary_projection(policy, identity, &target);
            let current_window_started_at_unix_ms = payout_window_started_at_for_identity(
                now_unix_ms,
                payout_interval_ms,
                beneficiary.key.as_str(),
            );
            disposition.current_window_started_at_unix_ms = Some(current_window_started_at_unix_ms);
            disposition.beneficiary = Some(beneficiary.clone());

            candidate_indexes_by_primary_key
                .entry(beneficiary.key)
                .or_default()
                .push(index);
        }

        let mut primary_winner_indexes = Vec::new();
        for candidate_indexes in candidate_indexes_by_primary_key.values() {
            let winner_index = candidate_indexes
                .iter()
                .copied()
                .min_by(|lhs, rhs| {
                    dispositions[*lhs]
                        .identity
                        .nostr_pubkey_hex
                        .cmp(&dispositions[*rhs].identity.nostr_pubkey_hex)
                })
                .expect("availability beneficiary candidate");
            primary_winner_indexes.push(winner_index);

            if let Some(reason) = dispositions[winner_index]
                .beneficiary
                .as_ref()
                .and_then(|beneficiary| beneficiary.kind.duplicate_skip_reason())
                .map(str::to_string)
            {
                for candidate_index in candidate_indexes {
                    if *candidate_index != winner_index {
                        dispositions[*candidate_index].verdict_reason = Some(reason.clone());
                    }
                }
            }
        }

        let mut seen_payout_targets = BTreeMap::<String, usize>::new();
        for winner_index in primary_winner_indexes {
            let Some(target) = dispositions[winner_index].payout_target.as_ref() else {
                continue;
            };
            if seen_payout_targets
                .insert(
                    format!(
                        "{}:{}",
                        target.normalized_payment_target_kind(),
                        target.normalized_payment_target()
                    ),
                    winner_index,
                )
                .is_some()
            {
                dispositions[winner_index].verdict_reason =
                    Some("duplicate_payout_target_placeholder_readiness".to_string());
            }
        }

        for disposition in dispositions.iter_mut() {
            if disposition.verdict_reason.is_some() {
                continue;
            }
            if self
                .availability_oldest_unsettled_stipend_payout_key(disposition)
                .is_some()
            {
                disposition.verdict_reason =
                    Some("beneficiary_unsettled_stipend_backpressure".to_string());
            }
        }

        dispositions
    }

    fn availability_oldest_unsettled_stipend_payout_key(
        &self,
        disposition: &AvailabilityIdentityDisposition,
    ) -> Option<String> {
        let current_window_started_at_unix_ms = disposition.current_window_started_at_unix_ms?;
        let payout_key_scope = Self::availability_existing_payout_key_scope(disposition);
        self.payout_records_by_key
            .values()
            .filter(|record| {
                record.classification.effective_payout_class()
                    == TreasuryPayoutClass::PlaceholderLiveness
                    && matches!(record.status.as_str(), "dispatching" | "dispatched")
                    && record.window_started_at_unix_ms < current_window_started_at_unix_ms
                    && record
                        .payout_key
                        .split_once(':')
                        .is_some_and(|(_, scope)| scope == payout_key_scope)
            })
            .min_by_key(|record| record.window_started_at_unix_ms)
            .map(|record| record.payout_key.clone())
    }

    fn availability_current_payout_key(
        &self,
        disposition: &AvailabilityIdentityDisposition,
    ) -> Option<String> {
        let current_window_started_at_unix_ms = disposition.current_window_started_at_unix_ms?;
        let payout_key_scope = Self::availability_disposition_payout_key_scope(disposition);
        let current_payout_key =
            payout_window_key(current_window_started_at_unix_ms, payout_key_scope.as_str());
        if self
            .payout_records_by_key
            .contains_key(current_payout_key.as_str())
        {
            return Some(current_payout_key);
        }
        self.availability_oldest_unsettled_stipend_payout_key(disposition)
    }

    fn availability_observability_snapshot(
        &self,
        policy: &TreasuryRuntimePolicy,
        online_identities: &[OnlinePylonIdentity],
        now_unix_ms: u64,
    ) -> AvailabilityObservabilitySnapshot {
        let dispositions =
            self.availability_identity_dispositions(policy, online_identities, now_unix_ms);
        let mut host_clusters = BTreeSet::new();
        let mut snapshot = AvailabilityObservabilitySnapshot {
            availability_online_identities_now: online_identities.len() as u64,
            ..AvailabilityObservabilitySnapshot::default()
        };

        for identity in online_identities {
            host_clusters.insert(
                identity
                    .host_fingerprint
                    .as_ref()
                    .map(|fingerprint| format!("host:{fingerprint}"))
                    .unwrap_or_else(|| format!("identity:{}", identity.nostr_pubkey_hex)),
            );
        }
        snapshot.availability_online_host_clusters_now = host_clusters.len() as u64;

        let mut rows = dispositions
            .into_iter()
            .map(|disposition| {
                if disposition.identity.inference_ready {
                    snapshot.inference_ready_online_payout_targets = snapshot
                        .inference_ready_online_payout_targets
                        .saturating_add(1);
                }

                let version_gate_reason = disposition.current_window_started_at_unix_ms.and_then(
                    |window_started_at_unix_ms| {
                        policy
                            .new_accrual_version_gate_verdict(
                                disposition.identity.client_version.as_deref(),
                                window_started_at_unix_ms,
                            )
                            .skip_reason()
                            .map(str::to_string)
                    },
                );
                let final_verdict_reason = disposition
                    .verdict_reason
                    .clone()
                    .or(version_gate_reason.clone())
                    .unwrap_or_else(|| "eligible".to_string());
                let availability_stipend_eligible_now = final_verdict_reason == "eligible";

                if availability_stipend_eligible_now {
                    snapshot.availability_stipend_eligible_beneficiaries_now = snapshot
                        .availability_stipend_eligible_beneficiaries_now
                        .saturating_add(1);
                    snapshot.eligible_online_payout_targets =
                        snapshot.eligible_online_payout_targets.saturating_add(1);
                    if let Some(current_window_started_at_unix_ms) =
                        disposition.current_window_started_at_unix_ms
                    {
                        snapshot.latest_eligible_window_started_at_unix_ms =
                            Some(match snapshot.latest_eligible_window_started_at_unix_ms {
                                Some(existing) => existing.max(current_window_started_at_unix_ms),
                                None => current_window_started_at_unix_ms,
                            });
                    }
                } else if availability_reason_is_duplicate_host(final_verdict_reason.as_str()) {
                    snapshot.duplicate_host_placeholder_blocked_online_targets = snapshot
                        .duplicate_host_placeholder_blocked_online_targets
                        .saturating_add(1);
                } else if availability_reason_is_duplicate_payout_target(
                    final_verdict_reason.as_str(),
                ) {
                    snapshot.duplicate_payout_target_placeholder_blocked_online_targets = snapshot
                        .duplicate_payout_target_placeholder_blocked_online_targets
                        .saturating_add(1);
                } else if availability_reason_is_missing_payout_target(
                    final_verdict_reason.as_str(),
                ) {
                    snapshot.missing_payout_target_blocked_online_targets = snapshot
                        .missing_payout_target_blocked_online_targets
                        .saturating_add(1);
                } else if availability_reason_is_version_floor(final_verdict_reason.as_str()) {
                    snapshot.version_floor_blocked_beneficiaries_now = snapshot
                        .version_floor_blocked_beneficiaries_now
                        .saturating_add(1);
                    if NewAccrualVersionGateVerdict::skip_reason_is_unknown_version(
                        final_verdict_reason.as_str(),
                    ) {
                        snapshot.min_new_accrual_unknown_version_online_targets = snapshot
                            .min_new_accrual_unknown_version_online_targets
                            .saturating_add(1);
                    }
                } else {
                    snapshot.readiness_blocked_online_targets =
                        snapshot.readiness_blocked_online_targets.saturating_add(1);
                }

                let current_payout_key = self.availability_current_payout_key(&disposition);
                let current_payout_record = current_payout_key
                    .as_ref()
                    .and_then(|key| self.payout_records_by_key.get(key));
                TreasuryAvailabilityBeneficiaryDebugRow {
                    nostr_pubkey_hex: disposition.identity.nostr_pubkey_hex.clone(),
                    sellable: disposition.identity.sellable,
                    client_version: disposition.identity.client_version.clone(),
                    inference_ready: disposition.identity.inference_ready,
                    host_fingerprint: disposition.identity.host_fingerprint.clone(),
                    payout_target: disposition
                        .payout_target
                        .as_ref()
                        .map(|target| target.normalized_payment_target().to_string()),
                    beneficiary_kind: disposition
                        .beneficiary
                        .as_ref()
                        .map(|beneficiary| beneficiary.kind.label().to_string()),
                    beneficiary_key: disposition
                        .beneficiary
                        .as_ref()
                        .map(|beneficiary| beneficiary.key.clone()),
                    availability_stipend_eligible_now,
                    verdict_reason: final_verdict_reason,
                    current_window_started_at_unix_ms: disposition
                        .current_window_started_at_unix_ms,
                    current_payout_key,
                    current_payout_status: current_payout_record
                        .map(|record| record.status.clone()),
                    current_payout_reason: current_payout_record
                        .and_then(|record| record.reason.clone()),
                }
            })
            .collect::<Vec<_>>();

        rows.sort_by(|left, right| {
            left.availability_stipend_eligible_now
                .cmp(&right.availability_stipend_eligible_now)
                .then_with(|| left.verdict_reason.cmp(&right.verdict_reason))
                .then_with(|| left.nostr_pubkey_hex.cmp(&right.nostr_pubkey_hex))
        });
        rows.truncate(TREASURY_STATUS_AVAILABILITY_DEBUG_ROW_LIMIT);
        snapshot.availability_beneficiary_debug_rows = rows;
        snapshot
    }

    fn apply_availability_observability_snapshot(
        &mut self,
        observability: AvailabilityObservabilitySnapshot,
    ) {
        self.availability_online_identities_now = observability.availability_online_identities_now;
        self.availability_online_host_clusters_now =
            observability.availability_online_host_clusters_now;
        self.availability_stipend_eligible_beneficiaries_now =
            observability.availability_stipend_eligible_beneficiaries_now;
        self.eligible_online_payout_targets = observability.eligible_online_payout_targets;
        self.inference_ready_online_payout_targets =
            observability.inference_ready_online_payout_targets;
        self.duplicate_host_placeholder_blocked_online_targets =
            observability.duplicate_host_placeholder_blocked_online_targets;
        self.duplicate_payout_target_placeholder_blocked_online_targets =
            observability.duplicate_payout_target_placeholder_blocked_online_targets;
        self.missing_payout_target_blocked_online_targets =
            observability.missing_payout_target_blocked_online_targets;
        self.version_floor_blocked_beneficiaries_now =
            observability.version_floor_blocked_beneficiaries_now;
        self.readiness_blocked_online_targets = observability.readiness_blocked_online_targets;
        self.min_new_accrual_version_blocked_online_targets =
            observability.version_floor_blocked_beneficiaries_now;
        self.min_new_accrual_unknown_version_online_targets =
            observability.min_new_accrual_unknown_version_online_targets;
        self.latest_eligible_window_started_at_unix_ms =
            observability.latest_eligible_window_started_at_unix_ms;
        self.availability_beneficiary_debug_rows =
            observability.availability_beneficiary_debug_rows;
    }

    fn payout_loop_health(&self, config: &TreasuryConfig) -> String {
        if !self.treasury_enabled(config) {
            return "disabled".to_string();
        }
        if self
            .active_continuity_alerts
            .iter()
            .any(|alert| alert.severity == "critical")
        {
            return "degraded".to_string();
        }
        if self
            .active_continuity_alerts
            .iter()
            .any(|alert| alert.severity == "warning")
        {
            return "warning".to_string();
        }
        self.payout_loop_runtime_status
            .clone()
            .unwrap_or_else(|| "unknown".to_string())
    }

    fn wallet_storage_runtime_mode(&self) -> String {
        self.wallet_storage_runtime_mode
            .clone()
            .unwrap_or_else(default_wallet_storage_runtime_mode)
    }

    fn completed_funding_receive_total_sats(&self) -> u64 {
        self.funding_receives_by_payment_id
            .values()
            .filter(|receive| receive.status.eq_ignore_ascii_case("completed"))
            .fold(0u64, |total, receive| {
                total.saturating_add(receive.amount_sats)
            })
    }

    pub fn observe_payout_eligibility(
        &mut self,
        config: &TreasuryConfig,
        online_identities: &[OnlinePylonIdentity],
        now_unix_ms: u64,
    ) {
        let policy = self.active_policy(config);
        self.sellable_pylons_online_now = online_identities
            .iter()
            .filter(|identity| identity.sellable)
            .count() as u64;
        self.availability_online_identities_now = online_identities.len() as u64;
        self.availability_online_host_clusters_now = 0;
        self.availability_stipend_eligible_beneficiaries_now = 0;
        self.eligible_online_payout_targets = 0;
        self.inference_ready_online_payout_targets = 0;
        self.duplicate_host_placeholder_blocked_online_targets = 0;
        self.duplicate_payout_target_placeholder_blocked_online_targets = 0;
        self.missing_payout_target_blocked_online_targets = 0;
        self.version_floor_blocked_beneficiaries_now = 0;
        self.readiness_blocked_online_targets = 0;
        self.min_new_accrual_version_blocked_online_targets = 0;
        self.min_new_accrual_unknown_version_online_targets = 0;
        self.availability_beneficiary_debug_rows.clear();
        self.latest_eligible_window_started_at_unix_ms = None;
        if !policy.treasury_enabled || policy.payout_interval_seconds == 0 {
            return;
        }
        let observability =
            self.availability_observability_snapshot(&policy, online_identities, now_unix_ms);
        self.apply_availability_observability_snapshot(observability);
    }

    fn reason_metrics_24h(
        &self,
        now_unix_ms: u64,
    ) -> (Vec<TreasuryReasonMetric>, Vec<TreasuryReasonMetric>) {
        let cutoff = now_unix_ms.saturating_sub(TREASURY_PUBLIC_STATS_WINDOW_MS);
        let mut skip_metrics = BTreeMap::<String, TreasuryReasonMetric>::new();
        let mut fail_metrics = BTreeMap::<String, TreasuryReasonMetric>::new();
        for record in self.payout_records_by_key.values() {
            if record.updated_at_unix_ms < cutoff {
                continue;
            }
            let reason = record
                .reason
                .clone()
                .unwrap_or_else(|| "unspecified".to_string());
            match record.status.as_str() {
                "skipped" => {
                    let metric =
                        skip_metrics
                            .entry(reason.clone())
                            .or_insert(TreasuryReasonMetric {
                                reason,
                                count: 0,
                                total_sats: 0,
                            });
                    metric.count = metric.count.saturating_add(1);
                    metric.total_sats = metric.total_sats.saturating_add(record.amount_sats);
                }
                "failed" => {
                    let metric =
                        fail_metrics
                            .entry(reason.clone())
                            .or_insert(TreasuryReasonMetric {
                                reason,
                                count: 0,
                                total_sats: 0,
                            });
                    metric.count = metric.count.saturating_add(1);
                    metric.total_sats = metric.total_sats.saturating_add(record.amount_sats);
                }
                _ => {}
            }
        }
        (
            skip_metrics.into_values().collect(),
            fail_metrics.into_values().collect(),
        )
    }

    fn has_recent_skip_reason_since(&self, reason: &str, cutoff_unix_ms: u64) -> bool {
        self.payout_records_by_key.values().any(|record| {
            record.status == "skipped"
                && record.reason.as_deref() == Some(reason)
                && (record.window_started_at_unix_ms >= cutoff_unix_ms
                    || record.updated_at_unix_ms >= cutoff_unix_ms)
        })
    }

    fn continuity_signal_snapshot(
        &self,
        config: &TreasuryConfig,
        now_unix_ms: u64,
    ) -> TreasuryContinuitySignalSnapshot {
        let (skip_reason_metrics_24h, fail_reason_metrics_24h) =
            self.reason_metrics_24h(now_unix_ms);
        let mut active_alerts = Vec::new();
        let latest_eligible_window_started_at_unix_ms =
            self.latest_eligible_window_started_at_unix_ms;
        let policy = self.active_policy(config);
        let oldest_dispatch_pending_at_unix_ms = self
            .oldest_continuity_relevant_pending_payout_updated_at_unix_ms(
                &["queued", "dispatching"],
                config,
                now_unix_ms,
                &policy,
            );
        let oldest_confirmation_pending_at_unix_ms = self
            .oldest_continuity_relevant_pending_payout_updated_at_unix_ms(
                &["dispatched"],
                config,
                now_unix_ms,
                &policy,
            );

        if policy.treasury_enabled {
            if oldest_dispatch_pending_at_unix_ms.is_some_and(|pending_since_unix_ms| {
                now_unix_ms.saturating_sub(pending_since_unix_ms)
                    >= TREASURY_CONTINUITY_ALERT_THRESHOLD_MS
            }) {
                active_alerts.push(TreasuryContinuityAlert {
                    alert_id: "dispatch_stalled".to_string(),
                    severity: "critical".to_string(),
                    reason: "pending_payouts_not_dispatching".to_string(),
                    started_at_unix_ms: oldest_dispatch_pending_at_unix_ms.unwrap_or(now_unix_ms),
                    observed_at_unix_ms: now_unix_ms,
                });
            }

            if oldest_confirmation_pending_at_unix_ms.is_some_and(|pending_since_unix_ms| {
                now_unix_ms.saturating_sub(pending_since_unix_ms)
                    >= TREASURY_CONFIRMATION_STALL_ALERT_THRESHOLD_MS
            }) {
                active_alerts.push(TreasuryContinuityAlert {
                    alert_id: "confirmations_stalled".to_string(),
                    severity: "critical".to_string(),
                    reason: "pending_payouts_not_confirming".to_string(),
                    started_at_unix_ms: oldest_confirmation_pending_at_unix_ms
                        .unwrap_or(now_unix_ms),
                    observed_at_unix_ms: now_unix_ms,
                });
            }

            if self.eligible_online_payout_targets > 0
                && latest_eligible_window_started_at_unix_ms.is_some_and(|window_started_at| {
                    now_unix_ms.saturating_sub(window_started_at)
                        <= TREASURY_CONTINUITY_ALERT_THRESHOLD_MS
                        && self.has_recent_skip_reason_since(
                            "daily_budget_cap_reached",
                            window_started_at.saturating_sub(policy.payout_interval_ms()),
                        )
                })
            {
                active_alerts.push(TreasuryContinuityAlert {
                    alert_id: "budget_cap_exhausted".to_string(),
                    severity: "critical".to_string(),
                    reason: "daily_budget_cap_reached".to_string(),
                    started_at_unix_ms: latest_eligible_window_started_at_unix_ms
                        .unwrap_or(now_unix_ms),
                    observed_at_unix_ms: now_unix_ms,
                });
            }
        }

        if matches!(self.policy_runtime_status.as_deref(), Some("blocked")) {
            active_alerts.push(TreasuryContinuityAlert {
                alert_id: "policy_runtime_blocked".to_string(),
                severity: "critical".to_string(),
                reason: self
                    .policy_last_error
                    .clone()
                    .unwrap_or_else(|| "policy_blocked".to_string()),
                started_at_unix_ms: self
                    .payout_loop_last_completed_at_unix_ms
                    .unwrap_or(now_unix_ms),
                observed_at_unix_ms: now_unix_ms,
            });
        }

        let snapshot_age_ms = self
            .public_snapshot
            .as_ref()
            .map(|snapshot| now_unix_ms.saturating_sub(snapshot.generated_at_unix_ms));
        let wallet_sync_lag_ms = self
            .last_wallet_sync_at_unix_ms
            .map(|last_sync| now_unix_ms.saturating_sub(last_sync));

        let stale_after_ms = config.wallet_snapshot_stale_after_ms();
        let snapshot_stale = snapshot_age_ms.is_some_and(|lag| lag >= stale_after_ms);
        let wallet_sync_stale = wallet_sync_lag_ms.is_some_and(|lag| lag >= stale_after_ms);
        let wallet_sync_stale_requires_action =
            wallet_sync_stale && self.due_wallet_refresh_requires_reconciliation();
        if snapshot_stale || wallet_sync_stale_requires_action {
            active_alerts.push(TreasuryContinuityAlert {
                alert_id: "snapshot_stale".to_string(),
                severity: "warning".to_string(),
                reason: if snapshot_stale {
                    "treasury_snapshot_stale".to_string()
                } else {
                    "wallet_sync_stale_with_pending_reconciliation".to_string()
                },
                started_at_unix_ms: self
                    .public_snapshot
                    .as_ref()
                    .map(|snapshot| snapshot.generated_at_unix_ms)
                    .unwrap_or(now_unix_ms),
                observed_at_unix_ms: now_unix_ms,
            });
        }

        TreasuryContinuitySignalSnapshot {
            availability_online_identities_now: self.availability_online_identities_now,
            availability_online_host_clusters_now: self.availability_online_host_clusters_now,
            availability_stipend_eligible_beneficiaries_now: self
                .availability_stipend_eligible_beneficiaries_now,
            eligible_online_payout_targets: self.eligible_online_payout_targets,
            sellable_pylons_online_now: self.sellable_pylons_online_now,
            inference_ready_online_payout_targets: self.inference_ready_online_payout_targets,
            duplicate_host_placeholder_blocked_online_targets: self
                .duplicate_host_placeholder_blocked_online_targets,
            duplicate_payout_target_placeholder_blocked_online_targets: self
                .duplicate_payout_target_placeholder_blocked_online_targets,
            missing_payout_target_blocked_online_targets: self
                .missing_payout_target_blocked_online_targets,
            version_floor_blocked_beneficiaries_now: self.version_floor_blocked_beneficiaries_now,
            readiness_blocked_online_targets: self.readiness_blocked_online_targets,
            latest_eligible_window_started_at_unix_ms,
            last_dispatch_at_unix_ms: self.last_dispatch_at_unix_ms,
            last_confirmed_at_unix_ms: self.last_confirmed_payout_at_unix_ms,
            skip_reason_metrics_24h,
            fail_reason_metrics_24h,
            active_alerts,
        }
    }

    pub fn sync_continuity_alerts(
        &mut self,
        config: &TreasuryConfig,
        now_unix_ms: u64,
    ) -> Vec<TreasuryReceiptEvent> {
        let desired = self.continuity_signal_snapshot(config, now_unix_ms);
        let previous = self.active_continuity_alerts.clone();
        let mut receipts = Vec::new();
        let mut next_alerts = Vec::new();

        for alert in desired.active_alerts {
            if let Some(existing) = previous.iter().find(|row| row.alert_id == alert.alert_id) {
                next_alerts.push(TreasuryContinuityAlert {
                    alert_id: alert.alert_id.clone(),
                    severity: alert.severity.clone(),
                    reason: alert.reason.clone(),
                    started_at_unix_ms: existing.started_at_unix_ms,
                    observed_at_unix_ms: now_unix_ms,
                });
            } else {
                receipts.push(treasury_alert_raised_receipt(&alert));
                next_alerts.push(alert);
            }
        }

        for alert in previous {
            if next_alerts.iter().any(|row| row.alert_id == alert.alert_id) {
                continue;
            }
            receipts.push(treasury_alert_cleared_receipt(&alert, now_unix_ms));
        }

        let persist_needed = continuity_alerts_require_persist(
            self.active_continuity_alerts.as_slice(),
            next_alerts.as_slice(),
        );
        self.active_continuity_alerts = next_alerts;
        if persist_needed {
            self.persist();
        }
        receipts
    }

    fn oldest_continuity_relevant_pending_payout_updated_at_unix_ms(
        &self,
        statuses: &[&str],
        config: &TreasuryConfig,
        now_unix_ms: u64,
        policy: &TreasuryRuntimePolicy,
    ) -> Option<u64> {
        self.payout_records_by_key
            .values()
            .filter(|record| statuses.contains(&record.status.as_str()))
            .filter(|record| record.classification.continuity_alert_relevant(policy))
            .filter(|record| {
                !self.presence_stipend_backlog_superseded_by_newer_progress(record, statuses)
            })
            .filter(|record| {
                !self.legacy_availability_confirmation_attention_record(
                    record,
                    config,
                    now_unix_ms,
                    policy,
                )
            })
            .map(|record| record.updated_at_unix_ms)
            .min()
    }

    fn presence_stipend_backlog_superseded_by_newer_progress(
        &self,
        record: &TreasuryPayoutRecord,
        statuses: &[&str],
    ) -> bool {
        if record.classification.effective_payout_class()
            != TreasuryPayoutClass::PlaceholderLiveness
        {
            return false;
        }

        let newer_progress_at_unix_ms = if statuses.contains(&"dispatched") {
            self.last_confirmed_payout_at_unix_ms
        } else {
            self.last_dispatch_at_unix_ms
        };

        newer_progress_at_unix_ms
            .is_some_and(|progress_at| progress_at >= record.updated_at_unix_ms)
    }

    fn impossible_zero_balance_with_receive_history(&self) -> bool {
        self.wallet_balance_sats == 0
            && self.completed_funding_receive_total_sats()
                > self
                    .payout_sats_paid_total
                    .saturating_add(TREASURY_IMPOSSIBLE_ZERO_BALANCE_THRESHOLD_SATS)
    }

    fn latest_wallet_activity_at_unix_ms(&self) -> Option<u64> {
        let validated_recovery_activity_at =
            self.last_wallet_recovery_report
                .as_ref()
                .and_then(|summary| {
                    (summary.validation_passed
                        && !summary.major_divergence_detected
                        && summary
                            .current_balance_sats
                            .is_some_and(|balance| balance > 0))
                    .then_some(summary.generated_at_unix_ms)
                });
        [
            self.last_wallet_sync_at_unix_ms,
            self.wallet_balance_updated_at_unix_ms,
            self.last_dispatch_at_unix_ms,
            self.last_confirmed_payout_at_unix_ms,
            validated_recovery_activity_at,
        ]
        .into_iter()
        .flatten()
        .max()
    }

    fn backlog_counts(&self) -> (u64, u64) {
        let mut backlog_total = 0u64;
        let mut backlog_retryable = 0u64;
        for record in self.payout_records_by_key.values() {
            if !matches!(record.status.as_str(), "confirmed" | "skipped") {
                backlog_total = backlog_total.saturating_add(1);
            }
            if record.payment_id.is_none()
                && !record.payout_target.trim().is_empty()
                && payout_record_has_ldk_target(record)
                && (record.status == "dispatching"
                    || (record.status == "failed"
                        && record
                            .reason
                            .as_deref()
                            .is_some_and(failed_payout_reason_is_retryable)))
            {
                backlog_retryable = backlog_retryable.saturating_add(1);
            }
        }
        (backlog_total, backlog_retryable)
    }

    fn confirmation_visibility_counts(
        &self,
        config: &TreasuryConfig,
        now_unix_ms: u64,
    ) -> (u64, u64, u64) {
        let mut pending_confirmation_count = 0u64;
        let mut tracked_payment_backlog_count = 0u64;
        let mut legacy_availability_confirmation_attention_count = 0u64;
        let policy = self.active_policy(config);
        for record in self.payout_records_by_key.values() {
            if self.inactive_availability_confirmation_record(record, &policy) {
                legacy_availability_confirmation_attention_count =
                    legacy_availability_confirmation_attention_count.saturating_add(1);
                continue;
            }
            if self.legacy_availability_confirmation_attention_record(
                record,
                config,
                now_unix_ms,
                &policy,
            ) {
                legacy_availability_confirmation_attention_count =
                    legacy_availability_confirmation_attention_count.saturating_add(1);
                continue;
            }
            if record.payment_id.is_none() {
                continue;
            }
            if record.status != "confirmed" {
                tracked_payment_backlog_count = tracked_payment_backlog_count.saturating_add(1);
            }
            if record.status == "dispatched" {
                pending_confirmation_count = pending_confirmation_count.saturating_add(1);
            }
        }
        (
            pending_confirmation_count,
            tracked_payment_backlog_count,
            legacy_availability_confirmation_attention_count,
        )
    }

    fn availability_dispatch_suppression_reason(
        &self,
        config: &TreasuryConfig,
        now_unix_ms: u64,
    ) -> Option<String> {
        let (pending_confirmation_count, _, _) =
            self.confirmation_visibility_counts(config, now_unix_ms);
        if pending_confirmation_count >= TREASURY_AVAILABILITY_DISPATCH_BACKLOG_GUARD_LIMIT {
            return Some("pending_confirmation_backlog_guard_active".to_string());
        }
        None
    }

    fn active_canonical_public_snapshot(
        &self,
        now_unix_ms: u64,
    ) -> Option<&TreasuryCanonicalPublicSnapshot> {
        self.canonical_public_snapshot
            .as_ref()
            .filter(|snapshot| snapshot.stale_after_unix_ms >= now_unix_ms)
    }

    fn wallet_runtime_view(
        &self,
        config: &TreasuryConfig,
        now_unix_ms: u64,
    ) -> (Option<String>, Option<String>) {
        let recent_wallet_activity = self
            .latest_wallet_activity_at_unix_ms()
            .map(|last_activity| now_unix_ms.saturating_sub(last_activity))
            .is_some_and(|lag_ms| lag_ms < config.wallet_snapshot_stale_after_ms());
        let timeout_only_error = matches!(self.wallet_runtime_status.as_deref(), Some("error"))
            && self
                .wallet_last_error
                .as_deref()
                .is_some_and(|detail| detail.starts_with("wallet_refresh_timeout:"));
        if timeout_only_error && recent_wallet_activity {
            return (Some("connected".to_string()), None);
        }
        let funding_target_timeout = matches!(self.wallet_runtime_status.as_deref(), Some("error"))
            && self
                .wallet_last_error
                .as_deref()
                .is_some_and(|detail| detail.starts_with(TREASURY_FUNDING_TARGET_TIMEOUT_PREFIX));
        if funding_target_timeout {
            if self.wallet_balance_updated_at_unix_ms.is_some()
                || self.wallet_balance_sats > 0
                || self
                    .last_wallet_recovery_report
                    .as_ref()
                    .is_some_and(|summary| {
                        summary.validation_passed && !summary.major_divergence_detected
                    })
            {
                return (Some("connected".to_string()), None);
            }
            return (None, None);
        }
        if matches!(self.wallet_runtime_status.as_deref(), Some("error"))
            && self.wallet_last_error.as_deref().is_some_and(|detail| {
                self.wallet_error_covered_by_recovery_report(detail, config, now_unix_ms)
            })
        {
            return (Some("connected".to_string()), None);
        }
        (
            self.wallet_runtime_status.clone(),
            self.wallet_last_error.clone(),
        )
    }

    fn degraded_reason(&self, config: &TreasuryConfig, now_unix_ms: u64) -> Option<String> {
        let (wallet_runtime_status, wallet_last_error) =
            self.wallet_runtime_view(config, now_unix_ms);
        let active_continuity_alerts = self.continuity_signal_snapshot(config, now_unix_ms);
        if matches!(self.policy_runtime_status.as_deref(), Some("blocked")) {
            return self
                .policy_last_error
                .clone()
                .or_else(|| Some("policy_blocked".to_string()));
        }
        if matches!(
            self.payout_loop_runtime_status.as_deref(),
            Some("error" | "degraded")
        ) {
            return self
                .payout_loop_last_error
                .clone()
                .or_else(|| Some("payout_loop_unhealthy".to_string()));
        }
        if let Some(alert) = active_continuity_alerts
            .active_alerts
            .iter()
            .find(|alert| alert.severity == "critical")
        {
            return Some(format!("continuity_alert:{}", alert.alert_id));
        }
        if matches!(wallet_runtime_status.as_deref(), Some("error")) {
            return wallet_last_error.or_else(|| Some("wallet_error".to_string()));
        }
        if self.impossible_zero_balance_with_receive_history() {
            return Some(format!(
                "wallet_balance_zero_with_receive_history:{}:{}",
                self.completed_funding_receive_total_sats(),
                self.payout_sats_paid_total
            ));
        }
        if self.wallet_storage_runtime_mode() == "original"
            && self
                .last_wallet_recovery_report
                .as_ref()
                .is_some_and(|summary| {
                    summary.validation_passed
                        && summary.major_divergence_detected
                        && summary
                            .rebuilt_minus_current_balance_sats
                            .map_or(true, |delta| delta > 0)
                })
        {
            let delta = self
                .last_wallet_recovery_report
                .as_ref()
                .and_then(|summary| summary.rebuilt_minus_current_balance_sats)
                .unwrap_or_default();
            return Some(format!("wallet_storage_diverges_from_rebuild:{delta}"));
        }
        if self.treasury_enabled(config) {
            let Some(last_wallet_activity_at_unix_ms) = self.latest_wallet_activity_at_unix_ms()
            else {
                return Some("wallet_unsynced".to_string());
            };
            let lag_ms = now_unix_ms.saturating_sub(last_wallet_activity_at_unix_ms);
            if lag_ms >= config.wallet_snapshot_stale_after_ms()
                && self.due_wallet_refresh_requires_reconciliation()
            {
                return Some(format!("wallet_snapshot_stale:{lag_ms}"));
            }
        }
        None
    }

    pub fn degraded_states(
        &self,
        config: &TreasuryConfig,
        now_unix_ms: u64,
    ) -> Vec<TreasuryDegradedState> {
        let mut states = Vec::new();
        let policy = self.active_policy(config);
        let continuity = self.continuity_signal_snapshot(config, now_unix_ms);

        for alert in continuity.active_alerts {
            states.push(TreasuryDegradedState {
                code: format!("continuity_{}", alert.alert_id),
                severity: alert.severity,
                public_reason: alert.reason,
                operator_action: "inspect treasury continuity and payout backlog".to_string(),
                source: "continuity_alert".to_string(),
                observed_at_unix_ms: now_unix_ms,
                started_at_unix_ms: Some(alert.started_at_unix_ms),
                metric_value: None,
                threshold: None,
            });
        }

        if policy.treasury_enabled {
            let payout_floor = policy
                .accepted_work_default_payout_sats()
                .max(policy.payout_sats_per_window);
            if payout_floor > 0 {
                let threshold = payout_floor
                    .saturating_mul(TREASURY_LOW_LIQUIDITY_PAYOUT_MULTIPLIER)
                    .max(TREASURY_LOW_LIQUIDITY_MIN_SATS);
                if self.wallet_balance_sats < threshold
                    && (self.backlog_counts().0 > 0
                        || self.eligible_online_payout_targets > 0
                        || !self.payout_targets_by_identity.is_empty())
                {
                    states.push(TreasuryDegradedState {
                        code: "low_outbound_liquidity".to_string(),
                        severity: "warning".to_string(),
                        public_reason: "outbound liquidity is below the configured payout reserve"
                            .to_string(),
                        operator_action:
                            "fund the LDK wallet or rebalance channels before dispatching payouts"
                                .to_string(),
                        source: "treasury_balance_threshold".to_string(),
                        observed_at_unix_ms: now_unix_ms,
                        started_at_unix_ms: self.wallet_balance_updated_at_unix_ms,
                        metric_value: Some(self.wallet_balance_sats),
                        threshold: Some(threshold),
                    });
                }
            }

            let ldk_target_count = self
                .payout_targets_by_identity
                .values()
                .filter(|target| target.is_ldk_compatible())
                .count() as u64;
            let channel_readiness = self.ldk_channel_readiness();
            if config.lightning_provider.provider == TreasuryLightningProviderKind::Ldk
                && ldk_target_count > 0
                && channel_readiness.projected_inbound_capacity_sats == 0
            {
                states.push(TreasuryDegradedState {
                    code: "low_inbound_liquidity".to_string(),
                    severity: "warning".to_string(),
                    public_reason: "no live LDK payout channel capacity is available for registered payout targets"
                        .to_string(),
                    operator_action: "open or rebalance an LDK channel and verify usable outbound capacity to registered Pylons"
                        .to_string(),
                    source: "ldk_provider_channel_snapshot".to_string(),
                    observed_at_unix_ms: now_unix_ms,
                    started_at_unix_ms: self.last_wallet_sync_at_unix_ms,
                    metric_value: Some(channel_readiness.projected_inbound_capacity_sats),
                    threshold: Some(1),
                });
            }

            match self.latest_wallet_activity_at_unix_ms() {
                Some(last_activity_at) => {
                    let lag_ms = now_unix_ms.saturating_sub(last_activity_at);
                    let threshold = config.wallet_snapshot_stale_after_ms();
                    if lag_ms >= threshold {
                        states.push(TreasuryDegradedState {
                            code: "stale_wallet_sync".to_string(),
                            severity: "warning".to_string(),
                            public_reason: "treasury wallet sync is stale".to_string(),
                            operator_action:
                                "refresh the LDK wallet state and rerun payment reconciliation"
                                    .to_string(),
                            source: "wallet_sync_threshold".to_string(),
                            observed_at_unix_ms: now_unix_ms,
                            started_at_unix_ms: Some(last_activity_at),
                            metric_value: Some(lag_ms),
                            threshold: Some(threshold),
                        });
                    }
                }
                None => {
                    states.push(TreasuryDegradedState {
                        code: "stale_wallet_sync".to_string(),
                        severity: "critical".to_string(),
                        public_reason: "treasury wallet has not completed a sync".to_string(),
                        operator_action: "start the LDK node and complete the first wallet sync"
                            .to_string(),
                        source: "wallet_sync_threshold".to_string(),
                        observed_at_unix_ms: now_unix_ms,
                        started_at_unix_ms: None,
                        metric_value: None,
                        threshold: Some(config.wallet_snapshot_stale_after_ms()),
                    });
                }
            }
        }

        let failure_readiness = self.ldk_payment_failure_readiness(now_unix_ms);
        if failure_readiness.recent_failed_payment_count_24h >= TREASURY_FAILED_PAYMENT_ALERT_COUNT
        {
            states.push(TreasuryDegradedState {
                code: "rising_failed_payment_count".to_string(),
                severity: "warning".to_string(),
                public_reason: "LDK payment failures are above the alert threshold".to_string(),
                operator_action: "inspect recent failed payment operations and route liquidity"
                    .to_string(),
                source: "payment_failure_threshold".to_string(),
                observed_at_unix_ms: now_unix_ms,
                started_at_unix_ms: None,
                metric_value: Some(failure_readiness.recent_failed_payment_count_24h),
                threshold: Some(TREASURY_FAILED_PAYMENT_ALERT_COUNT),
            });
        }

        for operation in self.treasury_operations_by_id.values() {
            if operation.rail != "ldk" {
                continue;
            }
            let reason = operation.degraded_reason.as_deref().unwrap_or_default();
            let terminal = operation
                .terminal_event_state
                .as_deref()
                .unwrap_or_default();
            if treasury_reason_indicates_no_route(reason) {
                push_unique_degraded_state(
                    &mut states,
                    TreasuryDegradedState {
                        code: "no_route".to_string(),
                        severity: "critical".to_string(),
                        public_reason: "LDK could not find a payment route".to_string(),
                        operator_action:
                            "inspect channels, route hints, peer connectivity, and outbound liquidity"
                                .to_string(),
                        source: "ldk_payment_error".to_string(),
                        observed_at_unix_ms: now_unix_ms,
                        started_at_unix_ms: Some(operation.updated_at_unix_ms),
                        metric_value: None,
                        threshold: None,
                    },
                );
            }
            if treasury_reason_indicates_insufficient_balance(reason) {
                push_unique_degraded_state(
                    &mut states,
                    TreasuryDegradedState {
                        code: "insufficient_channel_balance".to_string(),
                        severity: "critical".to_string(),
                        public_reason: "LDK reported insufficient channel or wallet balance"
                            .to_string(),
                        operator_action:
                            "fund the LDK wallet, open capacity, or rebalance before retrying"
                                .to_string(),
                        source: "ldk_payment_error".to_string(),
                        observed_at_unix_ms: now_unix_ms,
                        started_at_unix_ms: Some(operation.updated_at_unix_ms),
                        metric_value: operation.amount_msat.map(|msat| msat / 1_000),
                        threshold: None,
                    },
                );
            }
            if treasury_reason_indicates_stale_event_stream(reason)
                || terminal == "event_stream_disconnected"
            {
                let age_ms = now_unix_ms.saturating_sub(operation.updated_at_unix_ms);
                if age_ms >= TREASURY_STALE_EVENT_SUBSCRIBER_MS {
                    push_unique_degraded_state(
                        &mut states,
                        TreasuryDegradedState {
                            code: "stale_event_subscriber".to_string(),
                            severity: "warning".to_string(),
                            public_reason: "LDK event subscriber is stale or disconnected"
                                .to_string(),
                            operator_action:
                                "restart the event subscriber and replay payment event projection"
                                    .to_string(),
                            source: "ldk_event_projection".to_string(),
                            observed_at_unix_ms: now_unix_ms,
                            started_at_unix_ms: Some(operation.updated_at_unix_ms),
                            metric_value: Some(age_ms),
                            threshold: Some(TREASURY_STALE_EVENT_SUBSCRIBER_MS),
                        },
                    );
                }
            }
            if treasury_reason_indicates_stale_gossip(reason) {
                let age_ms = now_unix_ms.saturating_sub(operation.updated_at_unix_ms);
                if age_ms >= TREASURY_STALE_GOSSIP_ALERT_MS {
                    push_unique_degraded_state(
                        &mut states,
                        TreasuryDegradedState {
                            code: "stale_gossip".to_string(),
                            severity: "warning".to_string(),
                            public_reason: "LDK gossip or RGS data appears stale".to_string(),
                            operator_action:
                                "refresh rapid gossip sync and verify channel graph freshness"
                                    .to_string(),
                            source: "ldk_gossip_projection".to_string(),
                            observed_at_unix_ms: now_unix_ms,
                            started_at_unix_ms: Some(operation.updated_at_unix_ms),
                            metric_value: Some(age_ms),
                            threshold: Some(TREASURY_STALE_GOSSIP_ALERT_MS),
                        },
                    );
                }
            }
        }

        states.sort_by(|left, right| {
            degraded_severity_rank(right.severity.as_str())
                .cmp(&degraded_severity_rank(left.severity.as_str()))
                .then_with(|| left.code.cmp(&right.code))
        });
        states
    }

    fn build_public_snapshot(
        &self,
        config: &TreasuryConfig,
        now_unix_ms: u64,
    ) -> TreasuryPublicSnapshot {
        let (wallet_runtime_status, wallet_last_error) =
            self.wallet_runtime_view(config, now_unix_ms);
        let continuity = self.continuity_signal_snapshot(config, now_unix_ms);
        let policy = self.active_policy(config);
        let window_started_at_unix_ms = now_unix_ms.saturating_sub(TREASURY_PUBLIC_STATS_WINDOW_MS);
        let cumulative_totals = self.cumulative_payout_totals();
        let mut confirmed_24h_totals = TreasuryPayoutTotals::default();
        let mut in_flight_totals = TreasuryPayoutTotals::default();
        let mut in_flight_24h_totals = TreasuryPayoutTotals::default();
        let mut payouts_dispatched_24h = 0u64;
        let mut payouts_confirmed_24h = 0u64;
        let mut payouts_failed_24h = 0u64;
        let mut payouts_skipped_24h = 0u64;
        let (backlog_total, backlog_retryable) = self.backlog_counts();
        let (
            pending_confirmation_count,
            tracked_payment_backlog_count,
            legacy_availability_confirmation_attention_count,
        ) = self.confirmation_visibility_counts(config, now_unix_ms);

        for record in self.payout_records_by_key.values() {
            if record.status == "dispatched" && !record.counted_in_paid_total {
                in_flight_totals.add_amount(record.amount_sats, &record.classification);
                if record.updated_at_unix_ms >= window_started_at_unix_ms {
                    in_flight_24h_totals.add_amount(record.amount_sats, &record.classification);
                }
            }
            if record.updated_at_unix_ms < window_started_at_unix_ms {
                continue;
            }
            match record.status.as_str() {
                "dispatched" => {
                    payouts_dispatched_24h = payouts_dispatched_24h.saturating_add(1);
                }
                "confirmed" => {
                    payouts_confirmed_24h = payouts_confirmed_24h.saturating_add(1);
                    confirmed_24h_totals.add_amount(record.amount_sats, &record.classification);
                }
                "failed" => {
                    payouts_failed_24h = payouts_failed_24h.saturating_add(1);
                }
                "skipped" => {
                    payouts_skipped_24h = payouts_skipped_24h.saturating_add(1);
                }
                _ => {}
            }
        }

        let mut snapshot = TreasuryPublicSnapshot {
            generated_at_unix_ms: now_unix_ms,
            source: default_treasury_public_snapshot_source(),
            treasury_enabled: policy.treasury_enabled,
            payout_sats_per_window: policy.payout_sats_per_window,
            payout_interval_seconds: policy.payout_interval_seconds,
            require_sellable: policy.require_sellable,
            daily_budget_cap_sats: policy.daily_budget_cap_sats,
            accepted_work_policy: policy.accepted_work_policy_snapshot(),
            availability_policy: policy.availability_policy_snapshot(config),
            placeholder_payout_mode: policy.placeholder_payout_mode,
            dedupe_placeholder_hosts: policy.dedupe_placeholder_hosts,
            min_new_accrual_pylon_version: policy.min_new_accrual_pylon_version.clone(),
            min_new_accrual_started_at_unix_ms: policy.min_new_accrual_started_at_unix_ms,
            min_new_accrual_version_gate_active: policy.new_accrual_version_gate_active(),
            registered_payout_identities: self.payout_targets_by_identity.len() as u64,
            ldk_payout_target_identities: self.ldk_payout_target_identity_count(),
            pylon_v0_2_registration_required_identities: self
                .pylon_v0_2_registration_required_identity_count(),
            wallet_balance_sats: self.wallet_balance_sats,
            wallet_total_onchain_balance_sats: self.wallet_total_onchain_balance_sats,
            wallet_spendable_onchain_balance_sats: self.wallet_spendable_onchain_balance_sats,
            wallet_lightning_balance_sats: self.wallet_lightning_balance_sats,
            wallet_balance_updated_at_unix_ms: self.wallet_balance_updated_at_unix_ms,
            last_wallet_sync_at_unix_ms: self.last_wallet_sync_at_unix_ms,
            last_wallet_refresh_attempt_at_unix_ms: self.last_wallet_refresh_attempt_at_unix_ms,
            wallet_runtime_status,
            wallet_last_error,
            wallet_storage_runtime_mode: self.wallet_storage_runtime_mode(),
            payout_loop_runtime_status: self.payout_loop_runtime_status.clone(),
            payout_loop_last_error: self.payout_loop_last_error.clone(),
            payout_loop_health: self.payout_loop_health(config),
            last_payout_reconciliation_at_unix_ms: self.last_payout_reconciliation_at_unix_ms,
            payout_loop_last_started_at_unix_ms: self.payout_loop_last_started_at_unix_ms,
            payout_loop_last_completed_at_unix_ms: self.payout_loop_last_completed_at_unix_ms,
            payout_sats_paid_total: cumulative_totals.payout_sats_paid_total,
            payout_sats_paid_24h: confirmed_24h_totals.payout_sats_paid_total,
            payout_sats_in_flight_total: in_flight_totals.payout_sats_paid_total,
            payout_sats_in_flight_24h: in_flight_24h_totals.payout_sats_paid_total,
            accepted_work_payout_sats_paid_total: cumulative_totals
                .accepted_work_payout_sats_paid_total,
            accepted_work_payout_sats_paid_24h: confirmed_24h_totals
                .accepted_work_payout_sats_paid_total,
            accepted_work_payout_sats_in_flight_total: in_flight_totals
                .accepted_work_payout_sats_paid_total,
            accepted_work_payout_sats_in_flight_24h: in_flight_24h_totals
                .accepted_work_payout_sats_paid_total,
            availability_stipend_payout_sats_paid_total: cumulative_totals
                .placeholder_payout_sats_paid_total,
            availability_stipend_payout_sats_paid_24h: confirmed_24h_totals
                .placeholder_payout_sats_paid_total,
            availability_stipend_payout_sats_in_flight_total: in_flight_totals
                .placeholder_payout_sats_paid_total,
            availability_stipend_payout_sats_in_flight_24h: in_flight_24h_totals
                .placeholder_payout_sats_paid_total,
            placeholder_payout_sats_paid_total: cumulative_totals
                .placeholder_payout_sats_paid_total,
            placeholder_payout_sats_paid_24h: confirmed_24h_totals
                .placeholder_payout_sats_paid_total,
            placeholder_payout_sats_in_flight_total: in_flight_totals
                .placeholder_payout_sats_paid_total,
            placeholder_payout_sats_in_flight_24h: in_flight_24h_totals
                .placeholder_payout_sats_paid_total,
            beta_bonus_payout_sats_paid_total: cumulative_totals.beta_bonus_payout_sats_paid_total,
            beta_bonus_payout_sats_paid_24h: confirmed_24h_totals.beta_bonus_payout_sats_paid_total,
            beta_bonus_payout_sats_in_flight_total: in_flight_totals
                .beta_bonus_payout_sats_paid_total,
            beta_bonus_payout_sats_in_flight_24h: in_flight_24h_totals
                .beta_bonus_payout_sats_paid_total,
            weak_device_accepted_work_payout_sats_paid_total: cumulative_totals
                .weak_device_accepted_work_payout_sats_paid_total,
            weak_device_accepted_work_payout_sats_paid_24h: confirmed_24h_totals
                .weak_device_accepted_work_payout_sats_paid_total,
            strong_lane_accepted_work_payout_sats_paid_total: cumulative_totals
                .strong_lane_accepted_work_payout_sats_paid_total,
            strong_lane_accepted_work_payout_sats_paid_24h: confirmed_24h_totals
                .strong_lane_accepted_work_payout_sats_paid_total,
            payouts_dispatched_24h,
            payouts_confirmed_24h,
            payouts_failed_24h,
            payouts_skipped_24h,
            backlog_total,
            backlog_retryable,
            pending_confirmation_count,
            tracked_payment_backlog_count,
            legacy_availability_confirmation_attention_count,
            availability_online_identities_now: continuity.availability_online_identities_now,
            availability_online_host_clusters_now: continuity.availability_online_host_clusters_now,
            availability_stipend_eligible_beneficiaries_now: continuity
                .availability_stipend_eligible_beneficiaries_now,
            eligible_online_payout_targets: continuity.eligible_online_payout_targets,
            sellable_pylons_online_now: continuity.sellable_pylons_online_now,
            inference_ready_online_payout_targets: continuity.inference_ready_online_payout_targets,
            duplicate_host_placeholder_blocked_online_targets: continuity
                .duplicate_host_placeholder_blocked_online_targets,
            duplicate_host_blocked_beneficiaries_now: continuity
                .duplicate_host_placeholder_blocked_online_targets,
            duplicate_payout_target_blocked_beneficiaries_now: continuity
                .duplicate_payout_target_placeholder_blocked_online_targets,
            missing_payout_target_blocked_beneficiaries_now: continuity
                .missing_payout_target_blocked_online_targets,
            version_floor_blocked_beneficiaries_now: continuity
                .version_floor_blocked_beneficiaries_now,
            readiness_blocked_beneficiaries_now: continuity.readiness_blocked_online_targets,
            min_new_accrual_version_blocked_online_targets: self
                .min_new_accrual_version_blocked_online_targets,
            min_new_accrual_unknown_version_online_targets: self
                .min_new_accrual_unknown_version_online_targets,
            latest_eligible_window_started_at_unix_ms: continuity
                .latest_eligible_window_started_at_unix_ms,
            last_dispatch_at_unix_ms: continuity.last_dispatch_at_unix_ms,
            last_confirmed_payout_at_unix_ms: continuity.last_confirmed_at_unix_ms,
            skip_reason_metrics_24h: continuity.skip_reason_metrics_24h,
            fail_reason_metrics_24h: continuity.fail_reason_metrics_24h,
            active_continuity_alerts: self.active_continuity_alerts.clone(),
            degraded_reason: self.degraded_reason(config, now_unix_ms),
            degraded_states: self.degraded_states(config, now_unix_ms),
            mode: None,
            health_status: None,
        };

        if let Some(canonical) = self.active_canonical_public_snapshot(now_unix_ms) {
            snapshot.generated_at_unix_ms = canonical.generated_at_unix_ms;
            snapshot.source = canonical.source.clone();
            snapshot.wallet_runtime_status = canonical
                .wallet_runtime_status
                .clone()
                .or(snapshot.wallet_runtime_status);
            snapshot.wallet_last_error = canonical
                .wallet_last_error
                .clone()
                .or(snapshot.wallet_last_error);
            snapshot.payout_loop_health = if canonical.health_status == "healthy" {
                "healthy".to_string()
            } else {
                "degraded".to_string()
            };
            snapshot.payout_sats_paid_total = canonical.payout_sats_paid_total;
            snapshot.payout_sats_paid_24h = canonical.payout_sats_paid_24h;
            snapshot.payout_sats_in_flight_total = 0;
            snapshot.payout_sats_in_flight_24h = 0;
            snapshot.accepted_work_payout_sats_in_flight_total = 0;
            snapshot.accepted_work_payout_sats_in_flight_24h = 0;
            snapshot.availability_stipend_payout_sats_in_flight_total = 0;
            snapshot.availability_stipend_payout_sats_in_flight_24h = 0;
            snapshot.placeholder_payout_sats_in_flight_total = 0;
            snapshot.placeholder_payout_sats_in_flight_24h = 0;
            snapshot.beta_bonus_payout_sats_in_flight_total = 0;
            snapshot.beta_bonus_payout_sats_in_flight_24h = 0;
            snapshot.payouts_dispatched_24h = canonical.payouts_dispatched_24h;
            snapshot.payouts_confirmed_24h = canonical.payouts_confirmed_24h;
            snapshot.payouts_failed_24h = canonical.payouts_failed_24h;
            snapshot.payouts_skipped_24h = canonical.payouts_skipped_24h;
            snapshot.backlog_total = canonical.backlog_total;
            snapshot.backlog_retryable = canonical.backlog_retryable;
            snapshot.pending_confirmation_count = 0;
            snapshot.tracked_payment_backlog_count = 0;
            snapshot.legacy_availability_confirmation_attention_count = 0;
            snapshot.active_continuity_alerts = Vec::new();
            snapshot.degraded_reason = if canonical.health_status == "healthy" {
                None
            } else {
                canonical
                    .wallet_last_error
                    .clone()
                    .or_else(|| Some("treasury_service_degraded".to_string()))
            };
            snapshot.mode = Some(canonical.mode.clone());
            snapshot.health_status = Some(canonical.health_status.clone());
        }

        snapshot
    }

    pub fn refresh_public_snapshot_in_memory(&mut self, config: &TreasuryConfig, now_unix_ms: u64) {
        self.public_snapshot = Some(self.build_public_snapshot(config, now_unix_ms));
    }

    pub fn refresh_public_snapshot(&mut self, config: &TreasuryConfig, now_unix_ms: u64) {
        self.refresh_public_snapshot_in_memory(config, now_unix_ms);
        self.persist();
    }

    pub fn public_stats(&self, config: &TreasuryConfig, now_unix_ms: u64) -> TreasuryPublicStats {
        let snapshot = self.build_public_snapshot(config, now_unix_ms);
        let continuity = self.continuity_signal_snapshot(config, now_unix_ms);
        let (wallet_runtime_status, wallet_last_error) =
            self.wallet_runtime_view(config, now_unix_ms);
        let canonical = self.active_canonical_public_snapshot(now_unix_ms);
        let wallet_runtime_status = canonical
            .and_then(|snapshot| snapshot.wallet_runtime_status.clone())
            .or(wallet_runtime_status);
        let wallet_last_error = canonical
            .and_then(|snapshot| snapshot.wallet_last_error.clone())
            .or(wallet_last_error);
        let wallet_hydration_mode = canonical
            .and_then(|snapshot| snapshot.wallet_hydration_mode.clone())
            .or_else(|| self.wallet_hydration_mode.clone());
        let wallet_payment_scan_mode = canonical
            .and_then(|snapshot| snapshot.wallet_payment_scan_mode.clone())
            .or_else(|| self.wallet_payment_scan_mode.clone());
        let wallet_sync_lag_ms = self
            .latest_wallet_activity_at_unix_ms()
            .map(|last_activity| now_unix_ms.saturating_sub(last_activity));
        let use_local_continuity_alerts = snapshot.source == TREASURY_PUBLIC_SNAPSHOT_SOURCE_LOCAL;
        let payout_loop_health = if use_local_continuity_alerts {
            self.payout_loop_health(config)
        } else {
            snapshot.payout_loop_health.clone()
        };
        let degraded_reason = if use_local_continuity_alerts {
            self.degraded_reason(config, now_unix_ms)
        } else {
            snapshot.degraded_reason.clone()
        };
        let degraded_states = if use_local_continuity_alerts {
            self.degraded_states(config, now_unix_ms)
        } else {
            snapshot.degraded_states.clone()
        };
        let ldk_readiness =
            self.ldk_readiness_snapshot(config, now_unix_ms, degraded_states.as_slice());

        TreasuryPublicStats {
            active_treasury_provider: config.lightning_provider.provider.as_str().to_string(),
            active_treasury_rail: operation_rail_for_provider(config.lightning_provider.provider)
                .to_string(),
            ldk_network: config.lightning_provider.ldk.network.as_str().to_string(),
            ldk_chain_backend: config
                .lightning_provider
                .ldk
                .chain_backend
                .as_str()
                .to_string(),
            ldk_server_configured: ldk_server_configured(config),
            ldk_readiness,
            treasury_enabled: snapshot.treasury_enabled,
            payout_sats_per_window: snapshot.payout_sats_per_window,
            payout_interval_seconds: snapshot.payout_interval_seconds,
            require_sellable: snapshot.require_sellable,
            daily_budget_cap_sats: snapshot.daily_budget_cap_sats,
            accepted_work_policy: snapshot.accepted_work_policy,
            availability_policy: snapshot.availability_policy,
            placeholder_payout_mode: snapshot.placeholder_payout_mode,
            dedupe_placeholder_hosts: snapshot.dedupe_placeholder_hosts,
            min_new_accrual_pylon_version: snapshot.min_new_accrual_pylon_version,
            min_new_accrual_started_at_unix_ms: snapshot.min_new_accrual_started_at_unix_ms,
            min_new_accrual_version_gate_active: snapshot.min_new_accrual_version_gate_active,
            registered_payout_identities: snapshot.registered_payout_identities,
            ldk_payout_target_identities: snapshot.ldk_payout_target_identities,
            pylon_v0_2_registration_required_identities: snapshot
                .pylon_v0_2_registration_required_identities,
            wallet_balance_sats: snapshot.wallet_balance_sats,
            wallet_total_onchain_balance_sats: snapshot.wallet_total_onchain_balance_sats,
            wallet_spendable_onchain_balance_sats: snapshot.wallet_spendable_onchain_balance_sats,
            wallet_lightning_balance_sats: snapshot.wallet_lightning_balance_sats,
            wallet_balance_updated_at_unix_ms: snapshot.wallet_balance_updated_at_unix_ms,
            last_wallet_sync_at_unix_ms: snapshot.last_wallet_sync_at_unix_ms,
            last_wallet_refresh_attempt_at_unix_ms: snapshot.last_wallet_refresh_attempt_at_unix_ms,
            wallet_runtime_status,
            wallet_last_error,
            wallet_hydration_mode,
            wallet_payment_scan_mode,
            wallet_storage_runtime_mode: snapshot.wallet_storage_runtime_mode,
            payout_loop_runtime_status: snapshot.payout_loop_runtime_status,
            payout_loop_last_error: snapshot.payout_loop_last_error,
            last_payout_reconciliation_at_unix_ms: snapshot.last_payout_reconciliation_at_unix_ms,
            payout_loop_last_started_at_unix_ms: snapshot.payout_loop_last_started_at_unix_ms,
            payout_loop_last_completed_at_unix_ms: snapshot.payout_loop_last_completed_at_unix_ms,
            public_snapshot_generated_at_unix_ms: Some(snapshot.generated_at_unix_ms),
            public_snapshot_source: snapshot.source,
            public_snapshot_mode: snapshot.mode,
            public_snapshot_health_status: snapshot.health_status,
            snapshot_age_ms: Some(now_unix_ms.saturating_sub(snapshot.generated_at_unix_ms)),
            wallet_sync_lag_ms,
            backlog_total: snapshot.backlog_total,
            backlog_retryable: snapshot.backlog_retryable,
            pending_confirmation_count: snapshot.pending_confirmation_count,
            tracked_payment_backlog_count: snapshot.tracked_payment_backlog_count,
            legacy_availability_confirmation_attention_count: snapshot
                .legacy_availability_confirmation_attention_count,
            availability_online_identities_now: snapshot.availability_online_identities_now,
            availability_online_host_clusters_now: snapshot.availability_online_host_clusters_now,
            availability_stipend_eligible_beneficiaries_now: snapshot
                .availability_stipend_eligible_beneficiaries_now,
            eligible_online_payout_targets: snapshot.eligible_online_payout_targets,
            sellable_pylons_online_now: snapshot.sellable_pylons_online_now,
            inference_ready_online_payout_targets: snapshot.inference_ready_online_payout_targets,
            duplicate_host_placeholder_blocked_online_targets: snapshot
                .duplicate_host_placeholder_blocked_online_targets,
            duplicate_host_blocked_beneficiaries_now: snapshot
                .duplicate_host_blocked_beneficiaries_now,
            duplicate_payout_target_blocked_beneficiaries_now: snapshot
                .duplicate_payout_target_blocked_beneficiaries_now,
            missing_payout_target_blocked_beneficiaries_now: snapshot
                .missing_payout_target_blocked_beneficiaries_now,
            version_floor_blocked_beneficiaries_now: snapshot
                .version_floor_blocked_beneficiaries_now,
            readiness_blocked_beneficiaries_now: snapshot.readiness_blocked_beneficiaries_now,
            min_new_accrual_version_blocked_online_targets: snapshot
                .min_new_accrual_version_blocked_online_targets,
            min_new_accrual_unknown_version_online_targets: snapshot
                .min_new_accrual_unknown_version_online_targets,
            latest_eligible_window_started_at_unix_ms: snapshot
                .latest_eligible_window_started_at_unix_ms,
            last_dispatch_at_unix_ms: snapshot.last_dispatch_at_unix_ms,
            last_confirmed_payout_at_unix_ms: snapshot.last_confirmed_payout_at_unix_ms,
            eligible_window_lag_ms: lag_since(
                now_unix_ms,
                snapshot.latest_eligible_window_started_at_unix_ms,
            ),
            dispatch_lag_ms: lag_since(now_unix_ms, snapshot.last_dispatch_at_unix_ms),
            confirm_lag_ms: lag_since(now_unix_ms, snapshot.last_confirmed_payout_at_unix_ms),
            payout_loop_health,
            degraded_reason,
            payout_sats_paid_total: snapshot.payout_sats_paid_total,
            payout_sats_paid_24h: snapshot.payout_sats_paid_24h,
            payout_sats_in_flight_total: snapshot.payout_sats_in_flight_total,
            payout_sats_in_flight_24h: snapshot.payout_sats_in_flight_24h,
            accepted_work_payout_sats_paid_total: snapshot.accepted_work_payout_sats_paid_total,
            accepted_work_payout_sats_paid_24h: snapshot.accepted_work_payout_sats_paid_24h,
            accepted_work_payout_sats_in_flight_total: snapshot
                .accepted_work_payout_sats_in_flight_total,
            accepted_work_payout_sats_in_flight_24h: snapshot
                .accepted_work_payout_sats_in_flight_24h,
            availability_stipend_payout_sats_paid_total: snapshot
                .availability_stipend_payout_sats_paid_total,
            availability_stipend_payout_sats_paid_24h: snapshot
                .availability_stipend_payout_sats_paid_24h,
            availability_stipend_payout_sats_in_flight_total: snapshot
                .availability_stipend_payout_sats_in_flight_total,
            availability_stipend_payout_sats_in_flight_24h: snapshot
                .availability_stipend_payout_sats_in_flight_24h,
            placeholder_payout_sats_paid_total: snapshot.placeholder_payout_sats_paid_total,
            placeholder_payout_sats_paid_24h: snapshot.placeholder_payout_sats_paid_24h,
            placeholder_payout_sats_in_flight_total: snapshot
                .placeholder_payout_sats_in_flight_total,
            placeholder_payout_sats_in_flight_24h: snapshot.placeholder_payout_sats_in_flight_24h,
            beta_bonus_payout_sats_paid_total: snapshot.beta_bonus_payout_sats_paid_total,
            beta_bonus_payout_sats_paid_24h: snapshot.beta_bonus_payout_sats_paid_24h,
            beta_bonus_payout_sats_in_flight_total: snapshot.beta_bonus_payout_sats_in_flight_total,
            beta_bonus_payout_sats_in_flight_24h: snapshot.beta_bonus_payout_sats_in_flight_24h,
            weak_device_accepted_work_payout_sats_paid_total: snapshot
                .weak_device_accepted_work_payout_sats_paid_total,
            weak_device_accepted_work_payout_sats_paid_24h: snapshot
                .weak_device_accepted_work_payout_sats_paid_24h,
            strong_lane_accepted_work_payout_sats_paid_total: snapshot
                .strong_lane_accepted_work_payout_sats_paid_total,
            strong_lane_accepted_work_payout_sats_paid_24h: snapshot
                .strong_lane_accepted_work_payout_sats_paid_24h,
            payouts_dispatched_24h: snapshot.payouts_dispatched_24h,
            payouts_confirmed_24h: snapshot.payouts_confirmed_24h,
            payouts_failed_24h: snapshot.payouts_failed_24h,
            payouts_skipped_24h: snapshot.payouts_skipped_24h,
            skip_reason_metrics_24h: snapshot.skip_reason_metrics_24h,
            fail_reason_metrics_24h: snapshot.fail_reason_metrics_24h,
            active_continuity_alerts: if use_local_continuity_alerts {
                continuity.active_alerts
            } else {
                Vec::new()
            },
            degraded_states,
        }
    }

    pub(crate) fn payout_target_identity_rows(&self) -> Vec<TreasuryPayoutTargetIdentityStatus> {
        #[derive(Default)]
        struct PayoutTargetAggregate {
            payout_record_count: u64,
            confirmed_payout_count: u64,
            confirmed_payout_sats: u64,
            confirmed_accepted_work_payout_sats: u64,
            last_payout_at_unix_ms: Option<u64>,
        }

        let mut aggregates: BTreeMap<&str, PayoutTargetAggregate> = BTreeMap::new();
        for record in self.payout_records_by_key.values() {
            let aggregate = aggregates
                .entry(record.nostr_pubkey_hex.as_str())
                .or_default();
            aggregate.payout_record_count = aggregate.payout_record_count.saturating_add(1);
            aggregate.last_payout_at_unix_ms = Some(
                aggregate
                    .last_payout_at_unix_ms
                    .map(|current| current.max(record.updated_at_unix_ms))
                    .unwrap_or(record.updated_at_unix_ms),
            );

            if record.status == "confirmed" {
                aggregate.confirmed_payout_count =
                    aggregate.confirmed_payout_count.saturating_add(1);
                aggregate.confirmed_payout_sats = aggregate
                    .confirmed_payout_sats
                    .saturating_add(record.amount_sats);
                if record.classification.accepted_work() {
                    aggregate.confirmed_accepted_work_payout_sats = aggregate
                        .confirmed_accepted_work_payout_sats
                        .saturating_add(record.amount_sats);
                }
            }
        }

        let mut rows = self
            .payout_targets_by_identity
            .values()
            .map(|target| {
                let aggregate = aggregates.get(target.nostr_pubkey_hex.as_str());
                TreasuryPayoutTargetIdentityStatus {
                    nostr_pubkey_hex: target.nostr_pubkey_hex.clone(),
                    source_session_id: target.source_session_id.clone(),
                    payment_target_kind: target.normalized_payment_target_kind().to_string(),
                    payment_target: target.normalized_payment_target().to_string(),
                    payment_target_capabilities: target.payment_target_capabilities.clone(),
                    pylon_payment_target_version: target.pylon_payment_target_version.clone(),
                    ldk_compatible: target.is_ldk_compatible(),
                    provider_target: target.provider_target.clone(),
                    bitcoin_address: target.bitcoin_address.clone(),
                    registered_at_unix_ms: target.registered_at_unix_ms,
                    last_verified_at_unix_ms: target.last_verified_at_unix_ms,
                    payout_record_count: aggregate
                        .map(|aggregate| aggregate.payout_record_count)
                        .unwrap_or_default(),
                    confirmed_payout_count: aggregate
                        .map(|aggregate| aggregate.confirmed_payout_count)
                        .unwrap_or_default(),
                    confirmed_payout_sats: aggregate
                        .map(|aggregate| aggregate.confirmed_payout_sats)
                        .unwrap_or_default(),
                    confirmed_accepted_work_payout_sats: aggregate
                        .map(|aggregate| aggregate.confirmed_accepted_work_payout_sats)
                        .unwrap_or_default(),
                    last_payout_at_unix_ms: aggregate
                        .and_then(|aggregate| aggregate.last_payout_at_unix_ms),
                }
            })
            .collect::<Vec<_>>();

        rows.sort_by(|left, right| {
            right
                .last_verified_at_unix_ms
                .cmp(&left.last_verified_at_unix_ms)
                .then_with(|| right.registered_at_unix_ms.cmp(&left.registered_at_unix_ms))
                .then_with(|| left.nostr_pubkey_hex.cmp(&right.nostr_pubkey_hex))
        });
        rows.truncate(TREASURY_STATUS_PAYOUT_TARGET_ROW_LIMIT);
        rows
    }

    pub(crate) fn training_payout_ledger_summary(&self) -> TreasuryTrainingPayoutLedgerSummary {
        let mut summary = TreasuryTrainingPayoutLedgerSummary::default();

        for record in self.payout_records_by_key.values() {
            summary.payout_record_count = summary.payout_record_count.saturating_add(1);

            match record.status.as_str() {
                "confirmed" => {
                    summary.confirmed_payout_count =
                        summary.confirmed_payout_count.saturating_add(1);
                    if record.classification.accepted_work() {
                        summary.accepted_work_confirmed_payout_count = summary
                            .accepted_work_confirmed_payout_count
                            .saturating_add(1);
                    }
                }
                "queued" | "dispatching" | "dispatched" => {
                    summary.pending_payout_count = summary.pending_payout_count.saturating_add(1);
                    if record.classification.accepted_work() {
                        summary.accepted_work_pending_payout_count =
                            summary.accepted_work_pending_payout_count.saturating_add(1);
                    }
                }
                "failed" => {
                    if failed_payout_is_retryable_pending(record) {
                        summary.pending_payout_count =
                            summary.pending_payout_count.saturating_add(1);
                        if record.classification.accepted_work() {
                            summary.accepted_work_pending_payout_count =
                                summary.accepted_work_pending_payout_count.saturating_add(1);
                        }
                    } else if payout_record_is_retired_historical(record) {
                        summary.retired_historical_payout_count =
                            summary.retired_historical_payout_count.saturating_add(1);
                        summary.retired_historical_payout_sats = summary
                            .retired_historical_payout_sats
                            .saturating_add(record.amount_sats);
                        if record.classification.accepted_work() {
                            summary.retired_historical_accepted_work_payout_count = summary
                                .retired_historical_accepted_work_payout_count
                                .saturating_add(1);
                        }
                    } else {
                        summary.failed_payout_count = summary.failed_payout_count.saturating_add(1);
                        summary.attention_payout_count =
                            summary.attention_payout_count.saturating_add(1);
                        if payout_record_has_ldk_target(record) {
                            summary.current_ldk_failed_payout_count =
                                summary.current_ldk_failed_payout_count.saturating_add(1);
                            summary.current_ldk_attention_payout_count =
                                summary.current_ldk_attention_payout_count.saturating_add(1);
                        }
                        if record.classification.accepted_work() {
                            summary.accepted_work_attention_payout_count = summary
                                .accepted_work_attention_payout_count
                                .saturating_add(1);
                        }
                    }
                }
                "skipped" => {
                    summary.skipped_payout_count = summary.skipped_payout_count.saturating_add(1);
                    if record.reason.as_deref() == Some("missing_payout_target") {
                        summary.missing_payout_target_count =
                            summary.missing_payout_target_count.saturating_add(1);
                    }
                    if record.classification.accepted_work() {
                        summary.accepted_work_attention_payout_count = summary
                            .accepted_work_attention_payout_count
                            .saturating_add(1);
                    }
                }
                _ => {}
            }
        }

        summary.reconciliation_status = if summary.accepted_work_attention_payout_count > 0
            || summary.missing_payout_target_count > 0
        {
            "attention_required".to_string()
        } else if summary.accepted_work_pending_payout_count > 0 {
            "pending".to_string()
        } else {
            "clean".to_string()
        };

        summary
    }

    fn payout_ledger_cleanup_disposition_counts(&self) -> BTreeMap<String, u64> {
        let mut counts = BTreeMap::new();
        for record in self.payout_records_by_key.values() {
            let disposition = payout_record_cleanup_disposition(record).to_string();
            *counts.entry(disposition).or_insert(0) += 1;
        }
        counts
    }

    fn payout_ledger_cleanup_reason_counts(&self) -> BTreeMap<String, u64> {
        let mut counts = BTreeMap::new();
        for record in self.payout_records_by_key.values() {
            let reason = record.reason.clone().unwrap_or_else(|| "none".to_string());
            *counts.entry(reason).or_insert(0) += 1;
        }
        counts
    }

    fn payout_ledger_cleanup_report(
        &mut self,
        apply: bool,
        now_unix_ms: u64,
    ) -> TreasuryPayoutLedgerCleanupReport {
        let before_summary = self.training_payout_ledger_summary();
        let before_disposition_counts = self.payout_ledger_cleanup_disposition_counts();
        let before_reason_counts = self.payout_ledger_cleanup_reason_counts();

        let (changed, records_retired) = if apply {
            self.retire_unpayable_historical_payout_records(now_unix_ms)
        } else {
            let records = self
                .payout_records_by_key
                .values()
                .filter(|record| payout_record_should_be_retired_as_historical(record))
                .map(|record| TreasuryPayoutLedgerCleanupRetiredRecord {
                    payout_key: record.payout_key.clone(),
                    previous_status: record.status.clone(),
                    previous_reason: record.reason.clone(),
                    payout_rail: payout_rail_for_payment_request(record.payout_target.as_str())
                        .to_string(),
                    payout_class: record
                        .classification
                        .effective_payout_class()
                        .label()
                        .to_string(),
                    amount_sats: record.amount_sats,
                })
                .collect::<Vec<_>>();
            (false, records)
        };

        if apply && changed {
            self.persist();
        }

        let after_summary = self.training_payout_ledger_summary();
        let after_disposition_counts = self.payout_ledger_cleanup_disposition_counts();
        let after_reason_counts = self.payout_ledger_cleanup_reason_counts();

        TreasuryPayoutLedgerCleanupReport {
            authority: "openagents-hosted-nexus".to_string(),
            generated_at_unix_ms: now_unix_ms,
            state_path: self
                .state_path
                .as_ref()
                .map(|path| path.display().to_string())
                .unwrap_or_else(|| "<memory>".to_string()),
            applied: apply,
            changed,
            before_summary,
            after_summary,
            before_disposition_counts,
            after_disposition_counts,
            before_reason_counts,
            after_reason_counts,
            records_retired,
        }
    }

    fn recent_training_payouts(&self) -> Vec<TreasuryTrainingPayoutLedgerEntry> {
        let mut rows = self
            .payout_records_by_key
            .values()
            .map(|record| TreasuryTrainingPayoutLedgerEntry {
                payout_key: record.payout_key.clone(),
                nostr_pubkey_hex: record.nostr_pubkey_hex.clone(),
                payout_target: record.payout_target.clone(),
                amount_sats: record.amount_sats,
                status: record.status.clone(),
                reconciliation_status: treasury_payout_reconciliation_status(record).to_string(),
                reason: record.reason.clone(),
                payment_id: record.payment_id.clone(),
                window_started_at_unix_ms: record.window_started_at_unix_ms,
                window_ends_at_unix_ms: record.window_ends_at_unix_ms,
                created_at_unix_ms: record.created_at_unix_ms,
                updated_at_unix_ms: record.updated_at_unix_ms,
                sellable_at_window_open: record.sellable_at_window_open,
                classification: record.classification.clone(),
            })
            .collect::<Vec<_>>();

        rows.sort_by(|left, right| {
            right
                .updated_at_unix_ms
                .cmp(&left.updated_at_unix_ms)
                .then_with(|| right.created_at_unix_ms.cmp(&left.created_at_unix_ms))
                .then_with(|| left.payout_key.cmp(&right.payout_key))
        });
        rows.truncate(TREASURY_STATUS_PAYOUT_LEDGER_ROW_LIMIT);
        rows
    }

    pub(crate) fn training_payout_ledger_entries_for_run(
        &self,
        training_run_id: &str,
    ) -> Vec<TreasuryTrainingPayoutLedgerEntry> {
        let mut rows = self
            .payout_records_by_key
            .values()
            .filter(|record| {
                record.classification.training_run_id.as_deref() == Some(training_run_id)
            })
            .map(|record| TreasuryTrainingPayoutLedgerEntry {
                payout_key: record.payout_key.clone(),
                nostr_pubkey_hex: record.nostr_pubkey_hex.clone(),
                payout_target: record.payout_target.clone(),
                amount_sats: record.amount_sats,
                status: record.status.clone(),
                reconciliation_status: treasury_payout_reconciliation_status(record).to_string(),
                reason: record.reason.clone(),
                payment_id: record.payment_id.clone(),
                window_started_at_unix_ms: record.window_started_at_unix_ms,
                window_ends_at_unix_ms: record.window_ends_at_unix_ms,
                created_at_unix_ms: record.created_at_unix_ms,
                updated_at_unix_ms: record.updated_at_unix_ms,
                sellable_at_window_open: record.sellable_at_window_open,
                classification: record.classification.clone(),
            })
            .collect::<Vec<_>>();

        rows.sort_by(|left, right| {
            right
                .updated_at_unix_ms
                .cmp(&left.updated_at_unix_ms)
                .then_with(|| right.created_at_unix_ms.cmp(&left.created_at_unix_ms))
                .then_with(|| left.payout_key.cmp(&right.payout_key))
        });
        rows
    }

    fn recent_treasury_operations(&self) -> Vec<TreasuryOperationRecord> {
        let mut operations = self
            .treasury_operations_by_id
            .values()
            .cloned()
            .collect::<Vec<_>>();
        operations.sort_by(|left, right| {
            right
                .updated_at_unix_ms
                .cmp(&left.updated_at_unix_ms)
                .then_with(|| right.created_at_unix_ms.cmp(&left.created_at_unix_ms))
                .then_with(|| left.operation_id.cmp(&right.operation_id))
        });
        operations.truncate(64);
        operations
    }

    pub fn status_response(
        &self,
        config: &TreasuryConfig,
        now_unix_ms: u64,
    ) -> TreasuryStatusResponse {
        let stats = self.public_stats(config, now_unix_ms);
        let policy = self.active_policy(config);
        let training_payout_ledger_summary = self.training_payout_ledger_summary();
        let payout_target_identities = self.payout_target_identity_rows();
        let recent_training_payouts = self.recent_training_payouts();
        let recent_treasury_operations = self.recent_treasury_operations();
        let legacy_availability_confirmation_attention_rows =
            self.legacy_availability_confirmation_attention_rows(config, now_unix_ms);
        TreasuryStatusResponse {
            authority: "openagents-hosted-nexus".to_string(),
            active_treasury_provider: stats.active_treasury_provider,
            active_treasury_rail: stats.active_treasury_rail,
            ldk_network: stats.ldk_network,
            ldk_chain_backend: stats.ldk_chain_backend,
            ldk_server_configured: stats.ldk_server_configured,
            ldk_readiness: stats.ldk_readiness,
            treasury_enabled: stats.treasury_enabled,
            payout_sats_per_window: stats.payout_sats_per_window,
            payout_interval_seconds: stats.payout_interval_seconds,
            require_sellable: stats.require_sellable,
            daily_budget_cap_sats: stats.daily_budget_cap_sats,
            accepted_work_policy: stats.accepted_work_policy,
            availability_policy: stats.availability_policy,
            placeholder_payout_mode: stats.placeholder_payout_mode,
            dedupe_placeholder_hosts: stats.dedupe_placeholder_hosts,
            min_new_accrual_pylon_version: stats.min_new_accrual_pylon_version,
            min_new_accrual_started_at_unix_ms: stats.min_new_accrual_started_at_unix_ms,
            min_new_accrual_version_gate_active: stats.min_new_accrual_version_gate_active,
            registered_payout_identities: stats.registered_payout_identities,
            ldk_payout_target_identities: stats.ldk_payout_target_identities,
            pylon_v0_2_registration_required_identities: stats
                .pylon_v0_2_registration_required_identities,
            wallet_balance_sats: stats.wallet_balance_sats,
            wallet_total_onchain_balance_sats: stats.wallet_total_onchain_balance_sats,
            wallet_spendable_onchain_balance_sats: stats.wallet_spendable_onchain_balance_sats,
            wallet_lightning_balance_sats: stats.wallet_lightning_balance_sats,
            wallet_balance_updated_at_unix_ms: stats.wallet_balance_updated_at_unix_ms,
            last_wallet_sync_at_unix_ms: stats.last_wallet_sync_at_unix_ms,
            last_wallet_refresh_attempt_at_unix_ms: stats.last_wallet_refresh_attempt_at_unix_ms,
            wallet_runtime_status: stats.wallet_runtime_status,
            wallet_last_error: stats.wallet_last_error,
            wallet_hydration_mode: stats.wallet_hydration_mode,
            wallet_payment_scan_mode: stats.wallet_payment_scan_mode,
            wallet_storage_runtime_mode: stats.wallet_storage_runtime_mode,
            wallet_storage_report_path: self.wallet_storage_report_path.clone(),
            wallet_storage_rollback_dir: self.wallet_storage_rollback_dir.clone(),
            wallet_storage_cutover_at_unix_ms: self.wallet_storage_cutover_at_unix_ms,
            wallet_recovery_last_report_generated_at_unix_ms: self
                .last_wallet_recovery_report
                .as_ref()
                .map(|summary| summary.generated_at_unix_ms),
            wallet_recovery_last_report_validation_passed: self
                .last_wallet_recovery_report
                .as_ref()
                .map(|summary| summary.validation_passed),
            payout_loop_runtime_status: stats.payout_loop_runtime_status,
            payout_loop_last_error: stats.payout_loop_last_error,
            last_payout_reconciliation_at_unix_ms: stats.last_payout_reconciliation_at_unix_ms,
            payout_loop_last_started_at_unix_ms: stats.payout_loop_last_started_at_unix_ms,
            payout_loop_last_completed_at_unix_ms: stats.payout_loop_last_completed_at_unix_ms,
            public_snapshot_generated_at_unix_ms: stats.public_snapshot_generated_at_unix_ms,
            public_snapshot_source: stats.public_snapshot_source,
            public_snapshot_mode: stats.public_snapshot_mode,
            public_snapshot_health_status: stats.public_snapshot_health_status,
            snapshot_age_ms: stats.snapshot_age_ms,
            wallet_sync_lag_ms: stats.wallet_sync_lag_ms,
            backlog_total: stats.backlog_total,
            backlog_retryable: stats.backlog_retryable,
            pending_confirmation_count: stats.pending_confirmation_count,
            tracked_payment_backlog_count: stats.tracked_payment_backlog_count,
            legacy_availability_confirmation_attention_count: stats
                .legacy_availability_confirmation_attention_count,
            availability_online_identities_now: stats.availability_online_identities_now,
            availability_online_host_clusters_now: stats.availability_online_host_clusters_now,
            availability_stipend_eligible_beneficiaries_now: stats
                .availability_stipend_eligible_beneficiaries_now,
            eligible_online_payout_targets: stats.eligible_online_payout_targets,
            sellable_pylons_online_now: stats.sellable_pylons_online_now,
            inference_ready_online_payout_targets: stats.inference_ready_online_payout_targets,
            duplicate_host_placeholder_blocked_online_targets: stats
                .duplicate_host_placeholder_blocked_online_targets,
            duplicate_host_blocked_beneficiaries_now: stats
                .duplicate_host_blocked_beneficiaries_now,
            duplicate_payout_target_blocked_beneficiaries_now: stats
                .duplicate_payout_target_blocked_beneficiaries_now,
            missing_payout_target_blocked_beneficiaries_now: stats
                .missing_payout_target_blocked_beneficiaries_now,
            version_floor_blocked_beneficiaries_now: stats.version_floor_blocked_beneficiaries_now,
            readiness_blocked_beneficiaries_now: stats.readiness_blocked_beneficiaries_now,
            min_new_accrual_version_blocked_online_targets: stats
                .min_new_accrual_version_blocked_online_targets,
            min_new_accrual_unknown_version_online_targets: stats
                .min_new_accrual_unknown_version_online_targets,
            latest_eligible_window_started_at_unix_ms: stats
                .latest_eligible_window_started_at_unix_ms,
            last_dispatch_at_unix_ms: stats.last_dispatch_at_unix_ms,
            last_confirmed_payout_at_unix_ms: stats.last_confirmed_payout_at_unix_ms,
            eligible_window_lag_ms: stats.eligible_window_lag_ms,
            dispatch_lag_ms: stats.dispatch_lag_ms,
            confirm_lag_ms: stats.confirm_lag_ms,
            payout_loop_health: stats.payout_loop_health,
            degraded_reason: stats.degraded_reason,
            degraded_states: stats.degraded_states,
            policy_schema_version: policy.schema_version,
            policy_checksum: Some(policy.checksum.clone()),
            policy_runtime_status: self.policy_runtime_status.clone(),
            policy_last_error: self.policy_last_error.clone(),
            recent_policy_changes: self
                .policy_change_history
                .iter()
                .rev()
                .take(TREASURY_STATUS_POLICY_CHANGE_LIMIT)
                .cloned()
                .collect(),
            payout_sats_paid_total: stats.payout_sats_paid_total,
            payout_sats_paid_24h: stats.payout_sats_paid_24h,
            payout_sats_in_flight_total: stats.payout_sats_in_flight_total,
            payout_sats_in_flight_24h: stats.payout_sats_in_flight_24h,
            accepted_work_payout_sats_paid_total: stats.accepted_work_payout_sats_paid_total,
            accepted_work_payout_sats_paid_24h: stats.accepted_work_payout_sats_paid_24h,
            accepted_work_payout_sats_in_flight_total: stats
                .accepted_work_payout_sats_in_flight_total,
            accepted_work_payout_sats_in_flight_24h: stats.accepted_work_payout_sats_in_flight_24h,
            availability_stipend_payout_sats_paid_total: stats
                .availability_stipend_payout_sats_paid_total,
            availability_stipend_payout_sats_paid_24h: stats
                .availability_stipend_payout_sats_paid_24h,
            availability_stipend_payout_sats_in_flight_total: stats
                .availability_stipend_payout_sats_in_flight_total,
            availability_stipend_payout_sats_in_flight_24h: stats
                .availability_stipend_payout_sats_in_flight_24h,
            placeholder_payout_sats_paid_total: stats.placeholder_payout_sats_paid_total,
            placeholder_payout_sats_paid_24h: stats.placeholder_payout_sats_paid_24h,
            placeholder_payout_sats_in_flight_total: stats.placeholder_payout_sats_in_flight_total,
            placeholder_payout_sats_in_flight_24h: stats.placeholder_payout_sats_in_flight_24h,
            beta_bonus_payout_sats_paid_total: stats.beta_bonus_payout_sats_paid_total,
            beta_bonus_payout_sats_paid_24h: stats.beta_bonus_payout_sats_paid_24h,
            beta_bonus_payout_sats_in_flight_total: stats.beta_bonus_payout_sats_in_flight_total,
            beta_bonus_payout_sats_in_flight_24h: stats.beta_bonus_payout_sats_in_flight_24h,
            weak_device_accepted_work_payout_sats_paid_total: stats
                .weak_device_accepted_work_payout_sats_paid_total,
            weak_device_accepted_work_payout_sats_paid_24h: stats
                .weak_device_accepted_work_payout_sats_paid_24h,
            strong_lane_accepted_work_payout_sats_paid_total: stats
                .strong_lane_accepted_work_payout_sats_paid_total,
            strong_lane_accepted_work_payout_sats_paid_24h: stats
                .strong_lane_accepted_work_payout_sats_paid_24h,
            payouts_dispatched_24h: stats.payouts_dispatched_24h,
            payouts_confirmed_24h: stats.payouts_confirmed_24h,
            payouts_failed_24h: stats.payouts_failed_24h,
            payouts_skipped_24h: stats.payouts_skipped_24h,
            skip_reason_metrics_24h: stats.skip_reason_metrics_24h,
            fail_reason_metrics_24h: stats.fail_reason_metrics_24h,
            active_continuity_alerts: stats.active_continuity_alerts,
            training_payout_ledger_summary,
            payout_target_identities,
            recent_training_payouts,
            recent_treasury_operations,
            availability_beneficiary_debug_rows: self.availability_beneficiary_debug_rows.clone(),
            legacy_availability_confirmation_attention_rows,
        }
    }

    pub fn integration_policy_snapshot(
        &self,
        config: &TreasuryConfig,
    ) -> TreasuryIntegrationPolicySnapshot {
        let policy = self.active_policy(config);
        TreasuryIntegrationPolicySnapshot {
            treasury_enabled: policy.treasury_enabled,
            payout_sats_per_window: policy.payout_sats_per_window,
            payout_interval_seconds: policy.payout_interval_seconds,
            require_sellable: policy.require_sellable,
            daily_budget_cap_sats: policy.daily_budget_cap_sats,
            accepted_work_policy: policy.accepted_work_policy_snapshot(),
            availability_policy: policy.availability_policy_snapshot(config),
            placeholder_payout_mode: policy.placeholder_payout_mode,
            dedupe_placeholder_hosts: policy.dedupe_placeholder_hosts,
            min_new_accrual_pylon_version: policy.min_new_accrual_pylon_version,
            min_new_accrual_started_at_unix_ms: policy.min_new_accrual_started_at_unix_ms,
        }
    }

    pub fn import_canonical_public_snapshot(
        &mut self,
        config: &TreasuryConfig,
        mut snapshot: TreasuryCanonicalPublicSnapshot,
        now_unix_ms: u64,
    ) -> TreasuryIntegrationImportResponse {
        if let Some(existing) = self.canonical_public_snapshot.as_ref() {
            if snapshot.generated_at_unix_ms < existing.generated_at_unix_ms
                && snapshot.payout_sats_paid_total <= existing.payout_sats_paid_total
            {
                snapshot = existing.clone();
            }
        }
        self.apply_paid_total_floor(snapshot.payout_sats_paid_total);
        self.canonical_public_snapshot = Some(snapshot.clone());
        self.refresh_public_snapshot(config, now_unix_ms);
        TreasuryIntegrationImportResponse {
            authority: "openagents-hosted-nexus".to_string(),
            public_snapshot_source: snapshot.source,
            public_snapshot_generated_at_unix_ms: snapshot.generated_at_unix_ms,
            payout_sats_paid_total: snapshot.payout_sats_paid_total,
        }
    }

    pub fn record_wallet_error(&mut self, detail: impl Into<String>) {
        let detail = detail.into();
        if self.wallet_runtime_status.as_deref() == Some("error")
            && self.wallet_last_error.as_deref() == Some(detail.as_str())
        {
            return;
        }
        self.wallet_runtime_status = Some("error".to_string());
        self.wallet_last_error = Some(detail);
        self.persist();
    }

    pub fn record_wallet_refresh_error(&mut self, detail: impl Into<String>, now_unix_ms: u64) {
        let detail = detail.into();
        let status_changed = self.wallet_runtime_status.as_deref() != Some("error")
            || self.wallet_last_error.as_deref() != Some(detail.as_str());
        let had_recorded_attempt = self.last_wallet_refresh_attempt_at_unix_ms.is_some();
        self.wallet_runtime_status = Some("error".to_string());
        self.wallet_last_error = Some(detail);
        self.last_wallet_refresh_attempt_at_unix_ms = Some(
            self.last_wallet_refresh_attempt_at_unix_ms
                .unwrap_or(now_unix_ms)
                .max(now_unix_ms),
        );
        if status_changed || !had_recorded_attempt {
            self.persist();
        }
    }

    pub fn note_wallet_recovery_report(&mut self, report: &TreasuryWalletRecoveryReport) {
        self.wallet_storage_report_path = Some(report.report_path.clone());
        self.last_wallet_recovery_report = Some(TreasuryWalletRecoveryReportSummary {
            generated_at_unix_ms: report.generated_at_unix_ms,
            report_path: report.report_path.clone(),
            current_storage_dir: report.current_storage_backup_dir.clone(),
            rebuilt_storage_dir: report.rebuilt_storage_dir.clone(),
            current_balance_sats: report.current_storage.balance_sats,
            rebuilt_balance_sats: report.rebuilt_storage.balance_sats,
            rebuilt_minus_current_balance_sats: report
                .comparison
                .rebuilt_minus_current_balance_sats,
            major_divergence_detected: report.comparison.major_divergence_detected,
            validation_passed: report.comparison.validation_passed,
        });
        if report.comparison.validation_passed
            && !report.comparison.major_divergence_detected
            && matches!(
                report.current_storage.runtime_status.as_deref(),
                Some("synced" | "connected")
            )
            && report
                .current_storage
                .balance_sats
                .is_some_and(|balance| balance > 0)
        {
            let balance = report.current_storage.balance_sats.unwrap_or_default();
            self.wallet_runtime_status = Some("connected".to_string());
            self.wallet_last_error = None;
            self.wallet_balance_sats = balance;
            self.wallet_balance_updated_at_unix_ms = Some(
                self.wallet_balance_updated_at_unix_ms
                    .unwrap_or(report.generated_at_unix_ms)
                    .max(report.generated_at_unix_ms),
            );
            self.last_wallet_sync_at_unix_ms = Some(
                self.last_wallet_sync_at_unix_ms
                    .unwrap_or(report.generated_at_unix_ms)
                    .max(report.generated_at_unix_ms),
            );
        }
    }

    pub fn note_wallet_activity(&mut self, now_unix_ms: u64) {
        self.wallet_runtime_status = Some("connected".to_string());
        self.wallet_last_error = None;
        self.last_wallet_sync_at_unix_ms = Some(
            self.last_wallet_sync_at_unix_ms
                .unwrap_or(now_unix_ms)
                .max(now_unix_ms),
        );
    }

    pub fn wallet_refresh_plan(&self) -> TreasuryWalletRefreshPlan {
        let mut plan = TreasuryWalletRefreshPlan::recent_only();
        plan.history_scan_page_offset = self.wallet_refresh_history_page_offset;
        let completed_receive_total_sats = self.completed_funding_receive_total_sats();
        if completed_receive_total_sats
            > self
                .payout_sats_paid_total
                .saturating_add(TREASURY_IMPOSSIBLE_ZERO_BALANCE_THRESHOLD_SATS)
        {
            plan.expected_nonzero_balance = true;
            plan.historical_receive_total_sats = completed_receive_total_sats;
            plan.payout_sats_paid_total = self.payout_sats_paid_total;
        }
        for record in self.payout_records_by_key.values() {
            if record.status != "dispatched" || record.counted_in_paid_total {
                continue;
            }
            if let Some(payment_id) = record.payment_id.as_deref() {
                plan.track_payment_id(payment_id);
            }
        }
        plan
    }

    pub fn note_wallet_refresh_progress(
        &mut self,
        plan: &TreasuryWalletRefreshPlan,
        progress: &TreasuryWalletRefreshProgress,
    ) {
        let has_unconfirmed_dispatched_payouts =
            self.payout_records_by_key.values().any(|record| {
                record.status == "dispatched"
                    && !record.counted_in_paid_total
                    && record.payment_id.is_some()
            });
        if !has_unconfirmed_dispatched_payouts {
            self.wallet_refresh_history_page_offset = 0;
            return;
        }

        let history_scan_page_offset = plan
            .history_scan_page_offset
            .max(TREASURY_WALLET_REFRESH_RECENT_PAYMENT_PAGES);
        if progress.history_pages_scanned == 0 || progress.history_hit_end_of_history {
            self.wallet_refresh_history_page_offset = TREASURY_WALLET_REFRESH_RECENT_PAYMENT_PAGES;
            return;
        }

        self.wallet_refresh_history_page_offset =
            history_scan_page_offset.saturating_add(progress.history_pages_scanned);
    }

    pub fn note_wallet_recovery_cutover(
        &mut self,
        response: &TreasuryWalletRecoveryCutoverResponse,
    ) {
        self.wallet_storage_runtime_mode = Some(response.wallet_storage_runtime_mode.clone());
        self.wallet_storage_report_path = Some(response.report_path.clone());
        self.wallet_storage_rollback_dir = Some(response.rollback_storage_dir.clone());
        self.wallet_storage_cutover_at_unix_ms = Some(response.cutover_completed_at_unix_ms);
    }

    pub fn note_payout_loop_started(&mut self, now_unix_ms: u64) {
        self.payout_loop_runtime_status = Some("running".to_string());
        self.payout_loop_last_started_at_unix_ms = Some(now_unix_ms);
        self.last_payout_reconciliation_at_unix_ms
            .get_or_insert(now_unix_ms);
    }

    pub fn note_payout_loop_completed(
        &mut self,
        now_unix_ms: u64,
        reconciliation_degraded_reason: Option<String>,
    ) {
        self.payout_loop_last_completed_at_unix_ms = Some(now_unix_ms);
        if let Some(reason) = reconciliation_degraded_reason {
            self.payout_loop_runtime_status = Some("degraded".to_string());
            self.payout_loop_last_error = Some(reason);
        } else {
            self.payout_loop_runtime_status = Some("idle".to_string());
            self.payout_loop_last_error = None;
            self.last_payout_reconciliation_at_unix_ms = Some(
                self.last_payout_reconciliation_at_unix_ms
                    .unwrap_or(now_unix_ms)
                    .max(now_unix_ms),
            );
        }
    }

    pub fn note_payout_loop_error(&mut self, now_unix_ms: u64, detail: impl Into<String>) {
        self.payout_loop_runtime_status = Some("error".to_string());
        self.payout_loop_last_error = Some(detail.into());
        self.payout_loop_last_completed_at_unix_ms = Some(now_unix_ms);
    }

    pub fn dispatch_cycle_due(
        &self,
        config: &TreasuryConfig,
        now_unix_ms: u64,
        idle_interval_ms: u64,
    ) -> bool {
        let policy = self.active_policy(config);
        if !policy.treasury_enabled {
            return false;
        }
        let last_cycle_at = self
            .payout_loop_last_completed_at_unix_ms
            .or(self.payout_loop_last_started_at_unix_ms)
            .or(self.last_payout_reconciliation_at_unix_ms);
        if self
            .payout_records_by_key
            .values()
            .any(|record| record.status == "queued")
        {
            return true;
        }
        let retryable_failed_work_due = self
            .payout_records_by_key
            .values()
            .any(|record| retryable_failed_payout_is_due(record, now_unix_ms));
        if retryable_failed_work_due {
            return last_cycle_at
                .map(|last_cycle_at| now_unix_ms.saturating_sub(last_cycle_at) >= idle_interval_ms)
                .unwrap_or(true);
        }
        if policy.payout_interval_seconds == 0 {
            return false;
        }
        let interval_ms = policy.payout_interval_ms().max(idle_interval_ms);
        last_cycle_at
            .map(|last_cycle_at| now_unix_ms.saturating_sub(last_cycle_at) >= interval_ms)
            .unwrap_or(true)
    }

    pub fn issue_registration_challenge(
        &mut self,
        config: &TreasuryConfig,
        nostr_pubkey_hex: &str,
        session_id: &str,
        now_unix_ms: u64,
    ) -> ProviderPayoutTargetChallengeResponse {
        self.prune_challenges(now_unix_ms);
        let challenge = TreasuryRegistrationChallenge {
            nostr_pubkey_hex: nostr_pubkey_hex.to_string(),
            session_id: session_id.to_string(),
            challenge: format!(
                "{}:{nostr_pubkey_hex}:{session_id}:{now_unix_ms}:{}",
                TREASURY_PAYOUT_TARGET_DOMAIN, self.next_challenge_nonce
            ),
            issued_at_unix_ms: now_unix_ms,
            expires_at_unix_ms: now_unix_ms.saturating_add(config.registration_challenge_ttl_ms()),
            consumed: false,
        };
        self.next_challenge_nonce = self.next_challenge_nonce.saturating_add(1);
        self.registration_challenges_by_key.insert(
            registration_challenge_key(nostr_pubkey_hex, session_id),
            challenge.clone(),
        );
        ProviderPayoutTargetChallengeResponse {
            authority: "openagents-hosted-nexus".to_string(),
            nostr_pubkey_hex: nostr_pubkey_hex.to_string(),
            session_id: session_id.to_string(),
            challenge: challenge.challenge,
            issued_at_unix_ms: challenge.issued_at_unix_ms,
            expires_at_unix_ms: challenge.expires_at_unix_ms,
        }
    }

    pub fn register_payout_target(
        &mut self,
        request: &ProviderPayoutTargetRegistrationRequest,
        now_unix_ms: u64,
    ) -> Result<(
        ProviderPayoutTargetRegistrationResponse,
        Vec<TreasuryReceiptEvent>,
    )> {
        let challenge_key = registration_challenge_key(
            request.nostr_pubkey_hex.as_str(),
            request.session_id.as_str(),
        );
        let Some(challenge) = self.registration_challenges_by_key.get(&challenge_key) else {
            bail!("provider_payout_target_challenge_missing");
        };
        if challenge.consumed {
            bail!("provider_payout_target_challenge_consumed");
        }
        if now_unix_ms > challenge.expires_at_unix_ms {
            bail!("provider_payout_target_challenge_expired");
        }
        if challenge.challenge != request.challenge {
            bail!("provider_payout_target_challenge_mismatch");
        }
        let target_kind = normalized_registration_target_kind(request)?;
        let target_value = normalized_registration_target_value(request)?;
        let capabilities = normalized_payment_target_capabilities(request)?;
        let version = normalized_payment_target_version(request);
        let capability_refs = capabilities.iter().map(String::as_str).collect::<Vec<_>>();
        verify_provider_payment_target_registration_signature(
            request.nostr_pubkey_hex.as_str(),
            request.session_id.as_str(),
            request.challenge.as_str(),
            ProviderPaymentTargetRegistration {
                target_kind: target_kind.as_str(),
                target_value: target_value.as_str(),
                capabilities: capability_refs.as_slice(),
                version: version.as_str(),
            },
            request.challenge_signature_hex.as_str(),
        )
        .map_err(anyhow::Error::msg)?;
        self.registration_challenges_by_key.remove(&challenge_key);

        if let Some(existing) = self
            .payout_targets_by_identity
            .get_mut(request.nostr_pubkey_hex.as_str())
            .filter(|existing| {
                existing.normalized_payment_target_kind() == target_kind
                    && existing.normalized_payment_target() == target_value
                    && existing.payment_target_capabilities == capabilities
                    && existing.pylon_payment_target_version.as_deref() == Some(version.as_str())
                    && existing.provider_target.trim().is_empty()
                    && existing.bitcoin_address.is_none()
            })
        {
            existing.source_session_id = request.session_id.clone();
            existing.last_verified_at_unix_ms = now_unix_ms;
            return Ok((
                ProviderPayoutTargetRegistrationResponse {
                    authority: "openagents-hosted-nexus".to_string(),
                    nostr_pubkey_hex: request.nostr_pubkey_hex.clone(),
                    session_id: request.session_id.clone(),
                    payment_target_kind: target_kind,
                    payment_target: target_value,
                    payment_target_capabilities: capabilities,
                    pylon_payment_target_version: Some(version),
                    registered_at_unix_ms: existing.registered_at_unix_ms,
                },
                Vec::new(),
            ));
        }

        let target = RegisteredPayoutTarget {
            nostr_pubkey_hex: request.nostr_pubkey_hex.clone(),
            source_session_id: request.session_id.clone(),
            payment_target_kind: target_kind.clone(),
            payment_target: target_value.clone(),
            payment_target_capabilities: capabilities.clone(),
            pylon_payment_target_version: Some(version.clone()),
            provider_target: String::new(),
            bitcoin_address: None,
            registered_at_unix_ms: now_unix_ms,
            last_verified_at_unix_ms: now_unix_ms,
        };
        self.payout_targets_by_identity
            .insert(request.nostr_pubkey_hex.clone(), target.clone());
        self.trim_retention(now_unix_ms);
        self.persist();

        let mut attributes = BTreeMap::new();
        attributes.insert(
            "nostr_pubkey_hex".to_string(),
            request.nostr_pubkey_hex.clone(),
        );
        attributes.insert("payment_target_kind".to_string(), target_kind.clone());
        attributes.insert(
            "payment_target".to_string(),
            truncate_target(target_value.as_str()),
        );
        attributes.insert(
            "ldk_compatible".to_string(),
            target.is_ldk_compatible().to_string(),
        );
        attributes.insert("pylon_payment_target_version".to_string(), version.clone());

        Ok((
            ProviderPayoutTargetRegistrationResponse {
                authority: "openagents-hosted-nexus".to_string(),
                nostr_pubkey_hex: request.nostr_pubkey_hex.clone(),
                session_id: request.session_id.clone(),
                payment_target_kind: target_kind,
                payment_target: target_value,
                payment_target_capabilities: capabilities,
                pylon_payment_target_version: Some(version),
                registered_at_unix_ms: now_unix_ms,
            },
            vec![TreasuryReceiptEvent {
                receipt_type: "treasury.payout_target.registered",
                context: AuthorityReceiptContext {
                    session_id: Some(request.session_id.clone()),
                    request_id: Some(challenge_key),
                    status: Some("registered".to_string()),
                    attributes,
                    ..AuthorityReceiptContext::default()
                },
            }],
        ))
    }

    fn claim_queued_payouts_for_dispatch(
        &mut self,
        config: &TreasuryConfig,
        policy: &TreasuryRuntimePolicy,
        now_unix_ms: u64,
        reserved_wallet_sats: &mut u64,
        committed_daily_budget_totals: &mut TreasuryCommittedBudgetTotals,
    ) -> (Vec<TreasuryDispatchPlan>, bool) {
        let mut queued = self
            .payout_records_by_key
            .values()
            .filter(|record| {
                record.status == "queued" || retryable_failed_payout_is_due(record, now_unix_ms)
            })
            .map(|record| {
                (
                    if record.classification.accepted_work() {
                        0u8
                    } else {
                        1u8
                    },
                    record.amount_sats,
                    record.updated_at_unix_ms,
                    record.created_at_unix_ms,
                    record.payout_key.clone(),
                )
            })
            .collect::<Vec<_>>();
        queued.sort();

        let mut dispatch_plans = Vec::new();
        let mut operation_records = Vec::new();
        let mut changed = false;
        let mut accepted_work_claimed = 0usize;
        let mut availability_claimed = 0usize;
        let mut beta_bonus_claimed = 0usize;
        for (_, _, _, _, payout_key) in queued {
            let Some(record) = self.payout_records_by_key.get_mut(payout_key.as_str()) else {
                continue;
            };
            if policy.placeholder_payout_mode == TreasuryPlaceholderPayoutMode::Disabled
                && !record.classification.accepted_work()
            {
                if record.status != "skipped"
                    || record.reason.as_deref() != Some("placeholder_payouts_disabled")
                {
                    record.status = "skipped".to_string();
                    record.reason = Some("placeholder_payouts_disabled".to_string());
                    record.updated_at_unix_ms = now_unix_ms;
                    record.skip_receipt_recorded = true;
                    changed = true;
                }
                continue;
            }
            let Some(target) = self
                .payout_targets_by_identity
                .get(record.nostr_pubkey_hex.as_str())
                .cloned()
            else {
                if record.status != "skipped"
                    || record.reason.as_deref() != Some("missing_payout_target")
                {
                    record.status = "skipped".to_string();
                    record.reason = Some("missing_payout_target".to_string());
                    record.updated_at_unix_ms = now_unix_ms;
                    record.skip_receipt_recorded = true;
                    changed = true;
                }
                continue;
            };
            let payout_class = record.classification.effective_payout_class();
            let claim_limit =
                config.max_concurrent_send_operations_for_class(usize::MAX, payout_class);
            let claimed_for_class = match payout_class {
                TreasuryPayoutClass::AcceptedWork => &mut accepted_work_claimed,
                TreasuryPayoutClass::PlaceholderLiveness => &mut availability_claimed,
                TreasuryPayoutClass::BetaBonus => &mut beta_bonus_claimed,
            };
            if *claimed_for_class >= claim_limit {
                continue;
            }
            if config.lightning_provider.provider != TreasuryLightningProviderKind::Ldk
                && record.amount_sats
                    > self
                        .wallet_balance_sats
                        .saturating_sub(*reserved_wallet_sats)
            {
                if record.reason.as_deref() != Some("wallet_balance_insufficient") {
                    record.reason = Some("wallet_balance_insufficient".to_string());
                    record.updated_at_unix_ms = now_unix_ms;
                    changed = true;
                }
                continue;
            }
            let daily_budget_cap_sats = policy.daily_budget_cap_sats_for_class(payout_class);
            if daily_budget_cap_sats > 0
                && committed_daily_budget_totals
                    .total_for_class(payout_class)
                    .saturating_add(record.amount_sats)
                    > daily_budget_cap_sats
            {
                if record.reason.as_deref() != Some("daily_budget_cap_reached") {
                    record.reason = Some("daily_budget_cap_reached".to_string());
                    record.updated_at_unix_ms = now_unix_ms;
                    changed = true;
                }
                continue;
            }
            *reserved_wallet_sats = reserved_wallet_sats.saturating_add(record.amount_sats);
            committed_daily_budget_totals.add_amount(payout_class, record.amount_sats);
            *claimed_for_class = claimed_for_class.saturating_add(1);
            record.payout_target = target.normalized_payment_target().to_string();
            record.status = "dispatching".to_string();
            record.reason = None;
            record.updated_at_unix_ms = now_unix_ms;
            changed = true;
            operation_records.push(payout_dispatch_operation_from_record(
                config,
                record,
                now_unix_ms,
            ));
            dispatch_plans.push(TreasuryDispatchPlan {
                payout_key,
                payment_request: target.normalized_payment_target().to_string(),
                amount_sats: record.amount_sats,
                classification: record.classification.clone(),
            });
        }
        for operation in operation_records {
            changed |= self.upsert_treasury_operation(operation);
        }
        (dispatch_plans, changed)
    }

    pub fn prepare_due_payouts(
        &mut self,
        config: &TreasuryConfig,
        online_identities: &[OnlinePylonIdentity],
        now_unix_ms: u64,
    ) -> TreasuryPayoutPreparation {
        let mut changed = self.normalize_legacy_payout_classes();
        changed |= self.retire_unpayable_pending_payout_records(now_unix_ms);
        changed |= self.trim_retention(now_unix_ms);
        let (mut receipt_events, stale_changed) = self.expire_stale_dispatches(config, now_unix_ms);
        changed |= stale_changed;
        let policy = self.active_policy(config);
        if !policy.treasury_enabled || policy.payout_interval_seconds == 0 {
            self.observe_payout_eligibility(config, online_identities, now_unix_ms);
            self.refresh_public_snapshot_in_memory(config, now_unix_ms);
            return TreasuryPayoutPreparation {
                dispatch_plans: Vec::new(),
                receipt_events,
                reconciliation_degraded_reason: None,
            };
        }

        let wallet_dispatch_suppression_reason = self.wallet_dispatch_suppression_reason();
        if wallet_dispatch_suppression_reason.is_some() {
            self.observe_payout_eligibility(config, online_identities, now_unix_ms);
            if changed {
                self.refresh_public_snapshot(config, now_unix_ms);
            } else {
                self.refresh_public_snapshot_in_memory(config, now_unix_ms);
            }
            return TreasuryPayoutPreparation {
                dispatch_plans: Vec::new(),
                receipt_events,
                reconciliation_degraded_reason: wallet_dispatch_suppression_reason,
            };
        }

        let mut reserved_wallet_sats = self.reserved_wallet_outstanding_sats();
        let mut committed_daily_budget_totals =
            self.committed_daily_budget_sats_last_24h(now_unix_ms);
        let (mut dispatch_plans, queued_claim_changed) = self.claim_queued_payouts_for_dispatch(
            config,
            &policy,
            now_unix_ms,
            &mut reserved_wallet_sats,
            &mut committed_daily_budget_totals,
        );
        changed |= queued_claim_changed;
        if online_identities.is_empty() {
            self.observe_payout_eligibility(config, online_identities, now_unix_ms);
            if changed {
                self.refresh_public_snapshot(config, now_unix_ms);
            } else {
                self.refresh_public_snapshot_in_memory(config, now_unix_ms);
            }
            return TreasuryPayoutPreparation {
                dispatch_plans,
                receipt_events,
                reconciliation_degraded_reason: None,
            };
        }
        if policy.payout_sats_per_window == 0 {
            self.observe_payout_eligibility(config, online_identities, now_unix_ms);
            if changed {
                self.refresh_public_snapshot(config, now_unix_ms);
            } else {
                self.refresh_public_snapshot_in_memory(config, now_unix_ms);
            }
            return TreasuryPayoutPreparation {
                dispatch_plans,
                receipt_events,
                reconciliation_degraded_reason: None,
            };
        }
        if policy.placeholder_payout_mode == TreasuryPlaceholderPayoutMode::Disabled {
            self.observe_payout_eligibility(config, online_identities, now_unix_ms);
            if changed {
                self.refresh_public_snapshot(config, now_unix_ms);
            } else {
                self.refresh_public_snapshot_in_memory(config, now_unix_ms);
            }
            return TreasuryPayoutPreparation {
                dispatch_plans,
                receipt_events,
                reconciliation_degraded_reason: None,
            };
        }

        let payout_interval_ms = policy.payout_interval_ms();
        let (reconciliation_started_at_unix_ms, reconciliation_degraded_reason) =
            self.payout_reconciliation_started_at(config, now_unix_ms);
        changed |= self.record_reconciliation_operation(
            config,
            now_unix_ms,
            reconciliation_degraded_reason.clone(),
        );
        let availability_dispatch_suppression_reason =
            self.availability_dispatch_suppression_reason(config, now_unix_ms);
        let placeholder_classification = policy.placeholder_payout_classification();
        let dispositions =
            self.availability_identity_dispositions(&policy, online_identities, now_unix_ms);

        if availability_dispatch_suppression_reason.is_some() {
            let observability =
                self.availability_observability_snapshot(&policy, online_identities, now_unix_ms);
            self.apply_availability_observability_snapshot(observability);
            if changed {
                self.refresh_public_snapshot(config, now_unix_ms);
            } else {
                self.refresh_public_snapshot_in_memory(config, now_unix_ms);
            }
            return TreasuryPayoutPreparation {
                dispatch_plans,
                receipt_events,
                reconciliation_degraded_reason,
            };
        }

        for disposition in dispositions {
            let identity = &disposition.identity;
            let transient_unsettled_backpressure = disposition.verdict_reason.as_deref()
                == Some("beneficiary_unsettled_stipend_backpressure");
            let beneficiary_phase_key = disposition
                .beneficiary
                .as_ref()
                .map(|beneficiary| beneficiary.key.as_str())
                .unwrap_or(identity.nostr_pubkey_hex.as_str());
            let current_window_started_at_unix_ms = disposition
                .current_window_started_at_unix_ms
                .unwrap_or_else(|| {
                    payout_window_started_at_for_identity(
                        now_unix_ms,
                        payout_interval_ms,
                        beneficiary_phase_key,
                    )
                });
            let mut window_started_at_unix_ms = payout_window_started_at_for_identity(
                reconciliation_started_at_unix_ms,
                payout_interval_ms,
                beneficiary_phase_key,
            );
            loop {
                let window_ends_at_unix_ms =
                    window_started_at_unix_ms.saturating_add(payout_interval_ms);
                let payout_key_scope = if disposition.allowed() {
                    format!("availability-beneficiary:{beneficiary_phase_key}")
                } else {
                    format!("availability-identity:{}", identity.nostr_pubkey_hex)
                };
                let payout_key =
                    payout_window_key(window_started_at_unix_ms, payout_key_scope.as_str());
                let payout_target = disposition
                    .payout_target
                    .as_ref()
                    .map(|target| target.normalized_payment_target().to_string())
                    .unwrap_or_default();

                if !self.payout_records_by_key.contains_key(&payout_key) {
                    if transient_unsettled_backpressure {
                        break;
                    }
                    if let Some(reason) = disposition.verdict_reason.clone() {
                        let record = TreasuryPayoutRecord {
                            payout_key: payout_key.clone(),
                            nostr_pubkey_hex: identity.nostr_pubkey_hex.clone(),
                            payout_target,
                            amount_sats: policy.payout_sats_per_window,
                            status: "skipped".to_string(),
                            reason: Some(reason),
                            payment_id: None,
                            window_started_at_unix_ms,
                            window_ends_at_unix_ms,
                            created_at_unix_ms: now_unix_ms,
                            updated_at_unix_ms: now_unix_ms,
                            sellable_at_window_open: identity.sellable,
                            dispatch_receipt_recorded: false,
                            confirm_receipt_recorded: false,
                            fail_receipt_recorded: false,
                            skip_receipt_recorded: true,
                            counted_in_paid_total: false,
                            classification: placeholder_classification.clone(),
                        };
                        self.payout_records_by_key
                            .insert(payout_key, record.clone());
                        receipt_events.push(skipped_payout_receipt(&record));
                        changed = true;
                        if window_started_at_unix_ms >= current_window_started_at_unix_ms {
                            break;
                        }
                        window_started_at_unix_ms =
                            window_started_at_unix_ms.saturating_add(payout_interval_ms);
                        continue;
                    }

                    if let Some(reason) = policy
                        .new_accrual_version_gate_verdict(
                            identity.client_version.as_deref(),
                            window_started_at_unix_ms,
                        )
                        .skip_reason()
                    {
                        let record = TreasuryPayoutRecord {
                            payout_key: payout_key.clone(),
                            nostr_pubkey_hex: identity.nostr_pubkey_hex.clone(),
                            payout_target: payout_target.clone(),
                            amount_sats: policy.payout_sats_per_window,
                            status: "skipped".to_string(),
                            reason: Some(reason.to_string()),
                            payment_id: None,
                            window_started_at_unix_ms,
                            window_ends_at_unix_ms,
                            created_at_unix_ms: now_unix_ms,
                            updated_at_unix_ms: now_unix_ms,
                            sellable_at_window_open: identity.sellable,
                            dispatch_receipt_recorded: false,
                            confirm_receipt_recorded: false,
                            fail_receipt_recorded: false,
                            skip_receipt_recorded: true,
                            counted_in_paid_total: false,
                            classification: placeholder_classification.clone(),
                        };
                        self.payout_records_by_key
                            .insert(payout_key, record.clone());
                        receipt_events.push(skipped_payout_receipt(&record));
                        changed = true;
                        if window_started_at_unix_ms >= current_window_started_at_unix_ms {
                            break;
                        }
                        window_started_at_unix_ms =
                            window_started_at_unix_ms.saturating_add(payout_interval_ms);
                        continue;
                    }

                    let payout_class = placeholder_classification.effective_payout_class();
                    let daily_budget_cap_sats =
                        policy.daily_budget_cap_sats_for_class(payout_class);
                    if daily_budget_cap_sats > 0
                        && committed_daily_budget_totals
                            .total_for_class(payout_class)
                            .saturating_add(policy.payout_sats_per_window)
                            > daily_budget_cap_sats
                    {
                        let record = TreasuryPayoutRecord {
                            payout_key: payout_key.clone(),
                            nostr_pubkey_hex: identity.nostr_pubkey_hex.clone(),
                            payout_target: payout_target.clone(),
                            amount_sats: policy.payout_sats_per_window,
                            status: "skipped".to_string(),
                            reason: Some("daily_budget_cap_reached".to_string()),
                            payment_id: None,
                            window_started_at_unix_ms,
                            window_ends_at_unix_ms,
                            created_at_unix_ms: now_unix_ms,
                            updated_at_unix_ms: now_unix_ms,
                            sellable_at_window_open: identity.sellable,
                            dispatch_receipt_recorded: false,
                            confirm_receipt_recorded: false,
                            fail_receipt_recorded: false,
                            skip_receipt_recorded: true,
                            counted_in_paid_total: false,
                            classification: placeholder_classification.clone(),
                        };
                        self.payout_records_by_key
                            .insert(payout_key, record.clone());
                        receipt_events.push(skipped_payout_receipt(&record));
                        changed = true;
                        if window_started_at_unix_ms >= current_window_started_at_unix_ms {
                            break;
                        }
                        window_started_at_unix_ms =
                            window_started_at_unix_ms.saturating_add(payout_interval_ms);
                        continue;
                    }

                    committed_daily_budget_totals
                        .add_amount(payout_class, policy.payout_sats_per_window);
                    let record = TreasuryPayoutRecord {
                        payout_key: payout_key.clone(),
                        nostr_pubkey_hex: identity.nostr_pubkey_hex.clone(),
                        payout_target: payout_target.clone(),
                        amount_sats: policy.payout_sats_per_window,
                        status: "dispatching".to_string(),
                        reason: None,
                        payment_id: None,
                        window_started_at_unix_ms,
                        window_ends_at_unix_ms,
                        created_at_unix_ms: now_unix_ms,
                        updated_at_unix_ms: now_unix_ms,
                        sellable_at_window_open: identity.sellable,
                        dispatch_receipt_recorded: false,
                        confirm_receipt_recorded: false,
                        fail_receipt_recorded: false,
                        skip_receipt_recorded: false,
                        counted_in_paid_total: false,
                        classification: placeholder_classification.clone(),
                    };
                    let operation =
                        payout_dispatch_operation_from_record(config, &record, now_unix_ms);
                    self.payout_records_by_key
                        .insert(payout_key.clone(), record);
                    changed = true;
                    changed |= self.upsert_treasury_operation(operation);
                    dispatch_plans.push(TreasuryDispatchPlan {
                        payout_key,
                        payment_request: payout_target,
                        amount_sats: policy.payout_sats_per_window,
                        classification: placeholder_classification.clone(),
                    });
                }

                if window_started_at_unix_ms >= current_window_started_at_unix_ms {
                    break;
                }
                window_started_at_unix_ms =
                    window_started_at_unix_ms.saturating_add(payout_interval_ms);
            }
        }

        let observability =
            self.availability_observability_snapshot(&policy, online_identities, now_unix_ms);
        self.apply_availability_observability_snapshot(observability);

        if changed {
            self.refresh_public_snapshot(config, now_unix_ms);
        } else {
            self.refresh_public_snapshot_in_memory(config, now_unix_ms);
        }
        TreasuryPayoutPreparation {
            dispatch_plans,
            receipt_events,
            reconciliation_degraded_reason,
        }
    }

    pub fn apply_dispatch_outcome(
        &mut self,
        outcome: TreasuryDispatchOutcome,
        now_unix_ms: u64,
    ) -> Vec<TreasuryReceiptEvent> {
        let mut receipt_events = Vec::new();
        match outcome {
            TreasuryDispatchOutcome::Dispatched {
                payout_key,
                payment_id,
                terminal_event_state,
            } => {
                let terminal_event_state = terminal_event_state
                    .filter(|state| !state.trim().is_empty())
                    .unwrap_or_else(|| "dispatched".to_string());
                self.note_wallet_activity(now_unix_ms);
                self.last_dispatch_at_unix_ms = Some(
                    self.last_dispatch_at_unix_ms
                        .unwrap_or(now_unix_ms)
                        .max(now_unix_ms),
                );
                if let Some(record) = self.payout_records_by_key.get_mut(&payout_key) {
                    record.status = "dispatched".to_string();
                    record.reason = None;
                    record.payment_id = Some(payment_id.clone());
                    record.updated_at_unix_ms = now_unix_ms;
                    if !record.dispatch_receipt_recorded {
                        record.dispatch_receipt_recorded = true;
                        receipt_events.push(dispatched_payout_receipt(
                            record,
                            payment_id.as_str(),
                            terminal_event_state.as_str(),
                        ));
                    }
                }
                self.update_payout_operation_status(
                    payout_key.as_str(),
                    TreasuryOperationStatus::Pending,
                    Some(treasury_hash(payment_id.as_str())),
                    None,
                    Some(terminal_event_state),
                    now_unix_ms,
                );
                self.payout_key_by_payment_id.insert(payment_id, payout_key);
            }
            TreasuryDispatchOutcome::Failed { payout_key, reason } => {
                if let Some(record) = self.payout_records_by_key.get_mut(&payout_key) {
                    record.status = "failed".to_string();
                    record.reason = Some(reason.clone());
                    record.updated_at_unix_ms = now_unix_ms;
                    if !record.fail_receipt_recorded {
                        record.fail_receipt_recorded = true;
                        receipt_events.push(failed_payout_receipt(record));
                    }
                }
                self.update_payout_operation_status(
                    payout_key.as_str(),
                    TreasuryOperationStatus::Failed,
                    None,
                    Some(reason.clone()),
                    Some("failed".to_string()),
                    now_unix_ms,
                );
                if wallet_send_failure_is_leaf_selection(reason.as_str()) {
                    self.record_wallet_refresh_error(reason, now_unix_ms);
                }
            }
        }
        self.persist();
        receipt_events
    }

    pub fn apply_wallet_snapshot(
        &mut self,
        snapshot: &TreasuryWalletSnapshot,
        now_unix_ms: u64,
    ) -> Vec<TreasuryReceiptEvent> {
        let mut persist_needed = self.normalize_legacy_payout_classes();
        let next_wallet_runtime_status = Some(snapshot.runtime_status.clone());
        let next_wallet_last_error = snapshot.runtime_detail.clone();
        let next_wallet_hydration_mode = snapshot.wallet_hydration_mode.clone();
        let next_wallet_payment_scan_mode = snapshot.wallet_payment_scan_mode.clone();
        let next_wallet_balance_updated_at_unix_ms = Some(now_unix_ms);
        let next_last_wallet_sync_at_unix_ms = Some(now_unix_ms);
        let next_last_wallet_refresh_attempt_at_unix_ms = Some(now_unix_ms);
        if self.wallet_runtime_status != next_wallet_runtime_status
            || self.wallet_last_error != next_wallet_last_error
            || self.wallet_hydration_mode != next_wallet_hydration_mode
            || self.wallet_payment_scan_mode != next_wallet_payment_scan_mode
            || self.wallet_balance_sats != snapshot.balance_sats
            || self.wallet_total_onchain_balance_sats != snapshot.total_onchain_balance_sats
            || self.wallet_spendable_onchain_balance_sats != snapshot.spendable_onchain_balance_sats
            || self.wallet_lightning_balance_sats != snapshot.lightning_balance_sats
            || self.wallet_balance_updated_at_unix_ms != next_wallet_balance_updated_at_unix_ms
            || self.last_wallet_sync_at_unix_ms != next_last_wallet_sync_at_unix_ms
            || self.last_wallet_refresh_attempt_at_unix_ms
                != next_last_wallet_refresh_attempt_at_unix_ms
        {
            persist_needed = true;
        }
        self.wallet_runtime_status = next_wallet_runtime_status;
        self.wallet_last_error = next_wallet_last_error;
        self.wallet_hydration_mode = next_wallet_hydration_mode;
        self.wallet_payment_scan_mode = next_wallet_payment_scan_mode;
        self.wallet_balance_sats = snapshot.balance_sats;
        self.wallet_total_onchain_balance_sats = snapshot.total_onchain_balance_sats;
        self.wallet_spendable_onchain_balance_sats = snapshot.spendable_onchain_balance_sats;
        self.wallet_lightning_balance_sats = snapshot.lightning_balance_sats;
        self.wallet_balance_updated_at_unix_ms = next_wallet_balance_updated_at_unix_ms;
        self.last_wallet_sync_at_unix_ms = next_last_wallet_sync_at_unix_ms;
        self.last_wallet_refresh_attempt_at_unix_ms = next_last_wallet_refresh_attempt_at_unix_ms;

        let mut receipt_events = Vec::new();
        let mut last_confirmed_payout_at_unix_ms = self.last_confirmed_payout_at_unix_ms;
        let mut orphan_recovery_payout_keys = self.orphan_payment_recovery_keys();
        let mut payments = snapshot.payments.clone();
        payments.sort_by_key(|payment| payment.timestamp);
        for payment in &payments {
            persist_needed |= self.record_payment_status_lookup_operation(
                payment,
                snapshot.wallet_hydration_mode.as_deref(),
                now_unix_ms,
            );
            if payment.direction.eq_ignore_ascii_case("receive") {
                let payment_updated_at_unix_ms = payment.timestamp.saturating_mul(1_000);
                if let Some(existing) = self
                    .funding_receives_by_payment_id
                    .get_mut(payment.id.as_str())
                {
                    if existing.status != payment.status
                        || existing.amount_sats != payment.amount_sats
                        || existing.method != payment.method
                        || existing.description != payment.description
                        || existing.updated_at_unix_ms != payment_updated_at_unix_ms
                    {
                        existing.status = payment.status.clone();
                        existing.amount_sats = payment.amount_sats;
                        existing.method = payment.method.clone();
                        existing.description = payment.description.clone();
                        existing.updated_at_unix_ms = payment_updated_at_unix_ms;
                        persist_needed = true;
                    }
                } else {
                    self.funding_receives_by_payment_id.insert(
                        payment.id.clone(),
                        TreasuryFundingReceive {
                            payment_id: payment.id.clone(),
                            status: payment.status.clone(),
                            amount_sats: payment.amount_sats,
                            method: payment.method.clone(),
                            description: payment.description.clone(),
                            recorded_at_unix_ms: payment_updated_at_unix_ms,
                            updated_at_unix_ms: payment_updated_at_unix_ms,
                        },
                    );
                    persist_needed = true;
                }
            }

            if !payment.direction.eq_ignore_ascii_case("send") {
                continue;
            }
            let known_payout_key = self.payout_key_for_payment_id(payment.id.as_str());
            let recovered_orphan = known_payout_key.is_none();
            let Some(payout_key) = known_payout_key.or_else(|| {
                self.bind_orphan_send_payment(payment, &mut orphan_recovery_payout_keys)
            }) else {
                continue;
            };
            let payment_updated_at_unix_ms = payment.timestamp.saturating_mul(1_000);
            if recovered_orphan {
                let next_last_dispatch_at_unix_ms = Some(
                    self.last_dispatch_at_unix_ms
                        .unwrap_or(payment_updated_at_unix_ms)
                        .max(payment_updated_at_unix_ms),
                );
                if self.last_dispatch_at_unix_ms != next_last_dispatch_at_unix_ms {
                    self.last_dispatch_at_unix_ms = next_last_dispatch_at_unix_ms;
                    persist_needed = true;
                }
                tracing::info!(
                    payment_id = payment.id.as_str(),
                    payout_key = payout_key.as_str(),
                    payment_status = payment.status.as_str(),
                    amount_sats = payment.amount_sats,
                    "treasury recovered orphan provider payment from wallet history",
                );
            }
            let Some(record) = self.payout_records_by_key.get_mut(&payout_key) else {
                if self
                    .payout_key_by_payment_id
                    .remove(payment.id.as_str())
                    .is_some()
                {
                    persist_needed = true;
                }
                continue;
            };
            let payout_operation_update: (
                TreasuryOperationStatus,
                Option<String>,
                Option<String>,
                Option<String>,
            );
            if recovered_orphan && !record.dispatch_receipt_recorded {
                record.dispatch_receipt_recorded = true;
                persist_needed = true;
                receipt_events.push(dispatched_payout_receipt(
                    record,
                    payment.id.as_str(),
                    payment.status.as_str(),
                ));
            }
            if record.updated_at_unix_ms != payment_updated_at_unix_ms {
                record.updated_at_unix_ms = payment_updated_at_unix_ms;
                persist_needed = true;
            }
            if wallet_payment_is_confirmed(payment) {
                let confirmed_at_unix_ms = payment_updated_at_unix_ms;
                let next_last_confirmed_payout_at_unix_ms = Some(
                    last_confirmed_payout_at_unix_ms
                        .unwrap_or(confirmed_at_unix_ms)
                        .max(confirmed_at_unix_ms),
                );
                if last_confirmed_payout_at_unix_ms != next_last_confirmed_payout_at_unix_ms {
                    last_confirmed_payout_at_unix_ms = next_last_confirmed_payout_at_unix_ms;
                    persist_needed = true;
                }
                if record.status != "confirmed" {
                    record.status = "confirmed".to_string();
                    persist_needed = true;
                }
                if record.reason.is_some() {
                    record.reason = None;
                    persist_needed = true;
                }
                if !record.confirm_receipt_recorded {
                    record.confirm_receipt_recorded = true;
                    persist_needed = true;
                    receipt_events.push(confirmed_payout_receipt(record, payment.id.as_str()));
                }
                if !record.counted_in_paid_total {
                    record.counted_in_paid_total = true;
                    persist_needed = true;
                    self.payout_sats_paid_total = self
                        .payout_sats_paid_total
                        .saturating_add(record.amount_sats);
                    match record.classification.effective_payout_class() {
                        TreasuryPayoutClass::PlaceholderLiveness => {
                            self.placeholder_payout_sats_paid_total = self
                                .placeholder_payout_sats_paid_total
                                .saturating_add(record.amount_sats);
                        }
                        TreasuryPayoutClass::AcceptedWork => {
                            self.accepted_work_payout_sats_paid_total = self
                                .accepted_work_payout_sats_paid_total
                                .saturating_add(record.amount_sats);
                            if record.classification.weak_device_accepted_work() {
                                self.weak_device_accepted_work_payout_sats_paid_total = self
                                    .weak_device_accepted_work_payout_sats_paid_total
                                    .saturating_add(record.amount_sats);
                            }
                            if record.classification.strong_lane_accepted_work() {
                                self.strong_lane_accepted_work_payout_sats_paid_total = self
                                    .strong_lane_accepted_work_payout_sats_paid_total
                                    .saturating_add(record.amount_sats);
                            }
                        }
                        TreasuryPayoutClass::BetaBonus => {
                            self.beta_bonus_payout_sats_paid_total = self
                                .beta_bonus_payout_sats_paid_total
                                .saturating_add(record.amount_sats);
                        }
                    }
                }
                payout_operation_update = (
                    TreasuryOperationStatus::Completed,
                    Some(treasury_hash(payment.id.as_str())),
                    None,
                    Some(payment.status.clone()),
                );
            } else if wallet_payment_is_failed(payment) {
                let next_reason = payment
                    .status_detail
                    .clone()
                    .or_else(|| Some(payment.status.clone()));
                if record.status != "failed" {
                    record.status = "failed".to_string();
                    persist_needed = true;
                }
                if record.reason != next_reason {
                    record.reason = next_reason;
                    persist_needed = true;
                }
                if !record.fail_receipt_recorded {
                    record.fail_receipt_recorded = true;
                    persist_needed = true;
                    receipt_events.push(failed_payout_receipt(record));
                }
                payout_operation_update = (
                    TreasuryOperationStatus::Failed,
                    Some(treasury_hash(payment.id.as_str())),
                    record.reason.clone(),
                    Some(payment.status.clone()),
                );
            } else {
                if record.status != "dispatched" {
                    record.status = "dispatched".to_string();
                    persist_needed = true;
                }
                if record.reason.is_some() {
                    record.reason = None;
                    persist_needed = true;
                }
                payout_operation_update = (
                    TreasuryOperationStatus::Pending,
                    Some(treasury_hash(payment.id.as_str())),
                    None,
                    Some(payment.status.clone()),
                );
            }
            let (status, provider_payment_id, degraded_reason, terminal_event_state) =
                payout_operation_update;
            persist_needed |= self.update_payout_operation_status(
                payout_key.as_str(),
                status,
                provider_payment_id,
                degraded_reason,
                terminal_event_state,
                now_unix_ms,
            );
        }
        if self.last_confirmed_payout_at_unix_ms != last_confirmed_payout_at_unix_ms {
            self.last_confirmed_payout_at_unix_ms = last_confirmed_payout_at_unix_ms;
            persist_needed = true;
        }

        persist_needed |= self.trim_retention(now_unix_ms);
        if persist_needed {
            self.persist();
        }
        receipt_events
    }

    fn orphan_payment_recovery_keys(&self) -> Vec<String> {
        let mut payout_keys = self
            .payout_records_by_key
            .values()
            .filter(|record| record_can_recover_orphan_send_payment(record))
            .map(|record| record.payout_key.clone())
            .collect::<Vec<_>>();
        payout_keys.sort_by(|left_key, right_key| {
            let left = self
                .payout_records_by_key
                .get(left_key)
                .expect("orphan recovery payout key");
            let right = self
                .payout_records_by_key
                .get(right_key)
                .expect("orphan recovery payout key");
            left.created_at_unix_ms
                .cmp(&right.created_at_unix_ms)
                .then_with(|| left.updated_at_unix_ms.cmp(&right.updated_at_unix_ms))
                .then_with(|| left.payout_key.cmp(&right.payout_key))
        });
        payout_keys
    }

    fn bind_orphan_send_payment(
        &mut self,
        payment: &PaymentSummary,
        orphan_recovery_payout_keys: &mut Vec<String>,
    ) -> Option<String> {
        let payment_timestamp_ms = payment.timestamp.saturating_mul(1_000);
        let match_index = orphan_recovery_payout_keys.iter().position(|payout_key| {
            let Some(record) = self.payout_records_by_key.get(payout_key) else {
                return false;
            };
            record_matches_orphan_send_payment(record, payment, payment_timestamp_ms)
        })?;
        let payout_key = orphan_recovery_payout_keys.remove(match_index);
        let record = self.payout_records_by_key.get_mut(&payout_key)?;
        record.payment_id = Some(payment.id.clone());
        record.status = "dispatched".to_string();
        record.reason = None;
        record.updated_at_unix_ms = payment_timestamp_ms;
        self.payout_key_by_payment_id
            .insert(payment.id.clone(), payout_key.clone());
        Some(payout_key)
    }

    pub fn last_persistence_error(&self) -> Option<String> {
        self.last_persistence_error.clone()
    }

    fn reserved_wallet_outstanding_sats(&self) -> u64 {
        self.payout_records_by_key
            .values()
            .filter(|record| record.status == "dispatching")
            .fold(0u64, |total, record| {
                total.saturating_add(record.amount_sats)
            })
    }

    fn committed_daily_budget_sats_last_24h(
        &self,
        now_unix_ms: u64,
    ) -> TreasuryCommittedBudgetTotals {
        let cutoff = now_unix_ms.saturating_sub(TREASURY_PUBLIC_STATS_WINDOW_MS);
        self.payout_records_by_key
            .values()
            .filter(|record| {
                record.updated_at_unix_ms >= cutoff
                    && matches!(
                        record.status.as_str(),
                        "dispatching" | "dispatched" | "confirmed"
                    )
            })
            .fold(
                TreasuryCommittedBudgetTotals::default(),
                |mut totals, record| {
                    totals.add_amount(
                        record.classification.effective_payout_class(),
                        record.amount_sats,
                    );
                    totals
                },
            )
    }

    fn expire_stale_dispatches(
        &mut self,
        config: &TreasuryConfig,
        now_unix_ms: u64,
    ) -> (Vec<TreasuryReceiptEvent>, bool) {
        let timeout_ms =
            config.dispatch_result_timeout_ms(self.active_policy(config).payout_interval_ms());
        let mut receipt_events = Vec::new();
        let mut changed = false;
        for record in self.payout_records_by_key.values_mut() {
            if record.status != "dispatching" || record.payment_id.is_some() {
                continue;
            }
            if now_unix_ms <= record.updated_at_unix_ms.saturating_add(timeout_ms) {
                continue;
            }
            record.status = "failed".to_string();
            record.reason = Some("dispatch_outcome_timeout".to_string());
            record.updated_at_unix_ms = now_unix_ms;
            if !record.fail_receipt_recorded {
                record.fail_receipt_recorded = true;
                receipt_events.push(failed_payout_receipt(record));
            }
            changed = true;
        }
        (receipt_events, changed)
    }

    fn payout_reconciliation_started_at(
        &self,
        config: &TreasuryConfig,
        now_unix_ms: u64,
    ) -> (u64, Option<String>) {
        let Some(last_reconciliation_at_unix_ms) = self.last_payout_reconciliation_at_unix_ms
        else {
            return (now_unix_ms, None);
        };

        let mut reconciliation_started_at_unix_ms = last_reconciliation_at_unix_ms.min(now_unix_ms);
        let oldest_recoverable_at_unix_ms =
            now_unix_ms.saturating_sub(config.reconciliation_horizon_ms());
        if reconciliation_started_at_unix_ms < oldest_recoverable_at_unix_ms {
            reconciliation_started_at_unix_ms = oldest_recoverable_at_unix_ms;
            return (
                reconciliation_started_at_unix_ms,
                Some(format!(
                    "reconciliation_horizon_exceeded:{last_reconciliation_at_unix_ms}"
                )),
            );
        }

        (reconciliation_started_at_unix_ms, None)
    }

    fn trim_policy_change_history(&mut self) -> bool {
        if self.policy_change_history.len() <= TREASURY_POLICY_CHANGE_LIMIT {
            return false;
        }
        let overflow = self
            .policy_change_history
            .len()
            .saturating_sub(TREASURY_POLICY_CHANGE_LIMIT);
        self.policy_change_history.drain(0..overflow);
        true
    }

    fn prune_challenges(&mut self, now_unix_ms: u64) -> bool {
        let before = self.registration_challenges_by_key.len();
        self.registration_challenges_by_key.retain(|_, challenge| {
            !challenge.consumed && now_unix_ms <= challenge.expires_at_unix_ms
        });
        self.registration_challenges_by_key.len() != before
    }

    fn trim_retention(&mut self, now_unix_ms: u64) -> bool {
        let mut changed = false;
        if self.next_challenge_nonce == 0 {
            self.next_challenge_nonce = 1;
            changed = true;
        }
        changed |= self.trim_policy_change_history();
        if self.payout_targets_by_identity.len() > TREASURY_TARGET_LIMIT {
            let overflow = self
                .payout_targets_by_identity
                .len()
                .saturating_sub(TREASURY_TARGET_LIMIT);
            let victims = self
                .payout_targets_by_identity
                .keys()
                .take(overflow)
                .cloned()
                .collect::<Vec<_>>();
            for victim in victims {
                self.payout_targets_by_identity.remove(&victim);
            }
            changed = true;
        }
        if self.payout_records_by_key.len() > TREASURY_PAYOUT_LIMIT {
            let mut records = self
                .payout_records_by_key
                .values()
                .cloned()
                .collect::<Vec<_>>();
            records.sort_by_key(|record| record.updated_at_unix_ms);
            let overflow = records.len().saturating_sub(TREASURY_PAYOUT_LIMIT);
            for record in records.into_iter().take(overflow) {
                self.payout_records_by_key.remove(&record.payout_key);
            }
            changed = true;
        }
        if self.funding_receives_by_payment_id.len() > TREASURY_RECEIVE_LIMIT {
            let mut receives = self
                .funding_receives_by_payment_id
                .values()
                .cloned()
                .collect::<Vec<_>>();
            receives.sort_by_key(|receive| receive.updated_at_unix_ms);
            let overflow = receives.len().saturating_sub(TREASURY_RECEIVE_LIMIT);
            for receive in receives.into_iter().take(overflow) {
                self.funding_receives_by_payment_id
                    .remove(&receive.payment_id);
            }
            changed = true;
        }
        let placeholder_oldest_allowed =
            now_unix_ms.saturating_sub(TREASURY_PLACEHOLDER_PAYOUT_RECORD_RETENTION_WINDOW_MS);
        let placeholder_records_before = self.payout_records_by_key.len();
        self.payout_records_by_key.retain(|_, record| {
            !placeholder_liveness_record_can_compact(record)
                || record.updated_at_unix_ms >= placeholder_oldest_allowed
        });
        changed |= self.payout_records_by_key.len() != placeholder_records_before;
        let mut compactable_placeholder_records = self
            .payout_records_by_key
            .values()
            .filter(|record| placeholder_liveness_record_can_compact(record))
            .map(|record| (record.updated_at_unix_ms, record.payout_key.clone()))
            .collect::<Vec<_>>();
        if compactable_placeholder_records.len() > TREASURY_PLACEHOLDER_PAYOUT_RECORD_LIMIT {
            compactable_placeholder_records.sort();
            let overflow = compactable_placeholder_records
                .len()
                .saturating_sub(TREASURY_PLACEHOLDER_PAYOUT_RECORD_LIMIT);
            for (_, payout_key) in compactable_placeholder_records.into_iter().take(overflow) {
                self.payout_records_by_key.remove(payout_key.as_str());
            }
            changed = true;
        }

        let oldest_allowed = now_unix_ms.saturating_sub(TREASURY_STATE_RETENTION_WINDOW_MS);
        let payout_records_before = self.payout_records_by_key.len();
        self.payout_records_by_key
            .retain(|_, record| record.updated_at_unix_ms >= oldest_allowed);
        changed |= self.payout_records_by_key.len() != payout_records_before;
        let funding_receives_before = self.funding_receives_by_payment_id.len();
        self.funding_receives_by_payment_id
            .retain(|_, receive| receive.updated_at_unix_ms >= oldest_allowed);
        changed |= self.funding_receives_by_payment_id.len() != funding_receives_before;
        let operations_before = self.treasury_operations_by_id.len();
        self.treasury_operations_by_id
            .retain(|_, operation| operation.updated_at_unix_ms >= oldest_allowed);
        changed |= self.treasury_operations_by_id.len() != operations_before;
        if self.treasury_operations_by_id.len() > TREASURY_OPERATION_LIMIT {
            let mut operations = self
                .treasury_operations_by_id
                .values()
                .cloned()
                .collect::<Vec<_>>();
            operations.sort_by_key(|operation| operation.updated_at_unix_ms);
            let overflow = operations.len().saturating_sub(TREASURY_OPERATION_LIMIT);
            for operation in operations.into_iter().take(overflow) {
                self.treasury_operations_by_id
                    .remove(operation.operation_id.as_str());
            }
            changed = true;
        }
        changed |= self.prune_challenges(now_unix_ms);
        self.rebuild_payment_index();
        changed
    }

    fn rebuild_payment_index(&mut self) {
        self.payout_key_by_payment_id = self
            .payout_records_by_key
            .iter()
            .filter_map(|(payout_key, record)| {
                record
                    .payment_id
                    .as_ref()
                    .map(|payment_id| (payment_id.clone(), payout_key.clone()))
            })
            .collect();
    }

    fn payout_key_for_payment_id(&mut self, payment_id: &str) -> Option<String> {
        if let Some(payout_key) = self.payout_key_by_payment_id.get(payment_id) {
            return Some(payout_key.clone());
        }

        let payout_key = self
            .payout_records_by_key
            .iter()
            .find_map(|(payout_key, record)| {
                (record.payment_id.as_deref() == Some(payment_id)).then(|| payout_key.clone())
            })?;

        self.payout_key_by_payment_id
            .insert(payment_id.to_string(), payout_key.clone());

        Some(payout_key)
    }

    fn persist(&mut self) {
        let Some(state_path) = self.state_path.clone() else {
            return;
        };
        if let Some(parent) = state_path.parent()
            && let Err(error) = fs::create_dir_all(parent)
        {
            self.last_persistence_error = Some(format!(
                "failed to create treasury state dir {}: {error}",
                parent.display()
            ));
            return;
        }
        match serde_json::to_string_pretty(self) {
            Ok(payload) => {
                let tmp_path = state_path.with_extension("tmp");
                if let Err(error) = fs::write(tmp_path.as_path(), format!("{payload}\n")) {
                    self.last_persistence_error = Some(format!(
                        "failed to write treasury state temp {}: {error}",
                        tmp_path.display()
                    ));
                    return;
                }
                if let Err(error) = fs::rename(tmp_path.as_path(), state_path.as_path()) {
                    self.last_persistence_error = Some(format!(
                        "failed to replace treasury state {} with {}: {error}",
                        state_path.display(),
                        tmp_path.display()
                    ));
                } else {
                    self.last_persistence_error = None;
                }
            }
            Err(error) => {
                self.last_persistence_error =
                    Some(format!("failed to serialize treasury state: {error}"));
            }
        }
    }
}

fn simulated_wallet_snapshot(
    config: &TreasuryConfig,
    payments: Vec<PaymentSummary>,
) -> TreasuryWalletSnapshot {
    TreasuryWalletSnapshot {
        runtime_status: "connected".to_string(),
        runtime_detail: None,
        wallet_hydration_mode: Some("simulated_proof_wallet".to_string()),
        wallet_payment_scan_mode: Some("simulated".to_string()),
        balance_sats: config.simulated_wallet_balance_sats,
        total_onchain_balance_sats: config.simulated_wallet_balance_sats,
        spendable_onchain_balance_sats: config.simulated_wallet_balance_sats,
        lightning_balance_sats: config.simulated_wallet_balance_sats,
        payments,
    }
}

fn simulated_payment_id(plan: &TreasuryDispatchPlan) -> String {
    let digest = Sha256::digest(format!(
        "{}\n{}\n{}",
        plan.payout_key, plan.payment_request, plan.amount_sats
    ));
    format!("simulated:{}", hex::encode(digest))
}

fn simulated_payment_summary(plan: &TreasuryDispatchPlan, payment_id: &str) -> PaymentSummary {
    PaymentSummary {
        id: payment_id.to_string(),
        direction: "send".to_string(),
        status: "completed".to_string(),
        amount_sats: plan.amount_sats,
        fees_sats: 0,
        timestamp: now_unix_ms() / 1_000,
        method: "ldk-simulated".to_string(),
        description: Some(format!("simulated payout for {}", plan.payout_key)),
        invoice: Some(plan.payment_request.clone()),
        destination_pubkey: None,
        payment_hash: None,
        htlc_status: None,
        htlc_expiry_epoch_seconds: None,
        status_detail: None,
    }
}

fn simulated_funding_target(
    config: &TreasuryConfig,
    request: TreasuryFundingTargetRequest,
) -> TreasuryFundingMaterial {
    let amount = request.amount_sats.unwrap_or_default();
    let now = now_unix_ms();
    TreasuryFundingMaterial {
        provider_target: "provider:simulated-treasury-proof-wallet".to_string(),
        bitcoin_address: "bcrt1qsimulatedtreasuryproofwallet".to_string(),
        provider_invoice: (amount > 0)
            .then(|| format!("providerinvoice{amount}simulatedproofwallet")),
        bolt11_invoice: (amount > 0).then(|| format!("lnbc{amount}simulatedproofwallet")),
        provider_payment_id: None,
        phase_timings: TreasuryFundingTargetPhaseTimings {
            request_received_at_unix_ms: now,
            invoice_returned_at_unix_ms: Some(now),
            ..TreasuryFundingTargetPhaseTimings::default()
        },
        wallet_snapshot: simulated_wallet_snapshot(config, Vec::new()),
    }
}

fn dispatch_with_simulated_wallet(
    config: &TreasuryConfig,
    plans: &[TreasuryDispatchPlan],
) -> TreasuryDispatchBatchResult {
    let mut outcomes = Vec::with_capacity(plans.len());
    let mut payments = Vec::with_capacity(plans.len());
    for plan in plans {
        let payment_id = simulated_payment_id(plan);
        payments.push(simulated_payment_summary(plan, payment_id.as_str()));
        outcomes.push(TreasuryDispatchOutcome::Dispatched {
            payout_key: plan.payout_key.clone(),
            payment_id,
            terminal_event_state: Some("completed".to_string()),
        });
    }
    TreasuryDispatchBatchResult {
        outcomes,
        wallet_snapshot: Some(simulated_wallet_snapshot(config, payments)),
        wallet_error: None,
    }
}

fn funding_idempotency_key(request: &TreasuryFundingTargetRequest) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"nexus:ldk-funding-target:v1");
    hasher.update(request.amount_sats.unwrap_or_default().to_be_bytes());
    hasher.update(
        request
            .description
            .as_deref()
            .unwrap_or_default()
            .as_bytes(),
    );
    hasher.update(request.expiry_seconds.unwrap_or_default().to_be_bytes());
    format!("funding:{}", hex::encode(&hasher.finalize()[..16]))
}

fn ldk_wallet_snapshot(
    _config: &TreasuryConfig,
    balances: LdkServerBalances,
    payments: Vec<PaymentSummary>,
) -> TreasuryWalletSnapshot {
    TreasuryWalletSnapshot {
        runtime_status: "connected".to_string(),
        runtime_detail: None,
        wallet_hydration_mode: Some("ldk_provider_scaffold".to_string()),
        wallet_payment_scan_mode: Some("ldk_provider_boundary".to_string()),
        balance_sats: balances.usable_sats,
        total_onchain_balance_sats: balances.total_onchain_sats,
        spendable_onchain_balance_sats: balances.spendable_onchain_sats,
        lightning_balance_sats: balances.lightning_sats,
        payments,
    }
}

async fn load_ldk_server_balances(config: &TreasuryConfig) -> Result<LdkServerBalances> {
    let client = LdkServerClient::from_provider_config(&config.lightning_provider.ldk)
        .map_err(|error| anyhow!("ldk_wallet_snapshot_client_failed:{error}"))?;
    client
        .get_balances()
        .await
        .map_err(|error| anyhow!("ldk_wallet_snapshot_balance_failed:{error}"))
}

fn funding_material_from_provider_target(
    config: &TreasuryConfig,
    target: TreasuryProviderFundingTarget,
    balances: LdkServerBalances,
    phase_timings: TreasuryFundingTargetPhaseTimings,
) -> TreasuryFundingMaterial {
    TreasuryFundingMaterial {
        provider_target: target.provider_target,
        bitcoin_address: target.bitcoin_address,
        provider_invoice: None,
        bolt11_invoice: target.bolt11_invoice,
        provider_payment_id: target.provider_invoice,
        phase_timings,
        wallet_snapshot: ldk_wallet_snapshot(config, balances, Vec::new()),
    }
}

async fn create_ldk_provider_funding_target(
    config: &TreasuryConfig,
    request: TreasuryFundingTargetRequest,
) -> Result<TreasuryFundingMaterial> {
    let provider = LdkTreasuryProvider::new(config.lightning_provider.ldk.clone());
    let request_received_at_unix_ms = now_unix_ms();
    let idempotency_key = funding_idempotency_key(&request);
    let ldk_rpc_started_at_unix_ms = now_unix_ms();
    let target = provider
        .create_funding_target(TreasuryProviderFundingRequest {
            amount_sats: request.amount_sats,
            description: request.description,
            expiry_seconds: request.expiry_seconds,
            idempotency_key,
        })
        .await
        .map_err(|error| anyhow!(error.normalized_reason()))?;
    let ldk_rpc_completed_at_unix_ms = now_unix_ms();
    let balances = load_ldk_server_balances(config).await?;
    let invoice_returned_at_unix_ms = now_unix_ms();
    Ok(funding_material_from_provider_target(
        config,
        target,
        balances,
        TreasuryFundingTargetPhaseTimings {
            request_received_at_unix_ms,
            ldk_rpc_started_at_unix_ms: Some(ldk_rpc_started_at_unix_ms),
            ldk_rpc_completed_at_unix_ms: Some(ldk_rpc_completed_at_unix_ms),
            invoice_returned_at_unix_ms: Some(invoice_returned_at_unix_ms),
            ..TreasuryFundingTargetPhaseTimings::default()
        },
    ))
}

async fn dispatch_with_ldk_provider(
    config: &TreasuryConfig,
    plans: &[TreasuryDispatchPlan],
) -> TreasuryDispatchBatchResult {
    let provider = LdkTreasuryProvider::new(config.lightning_provider.ldk.clone());
    let mut outcomes = Vec::with_capacity(plans.len());
    let mut payments = Vec::new();
    let dispatch_timestamp_seconds = now_unix_ms() / 1_000;
    for plan in plans {
        match provider
            .dispatch_payout(TreasuryProviderPayoutRequest {
                payout_key: plan.payout_key.clone(),
                payment_target_kind: payout_target_kind_for_payment_request(
                    plan.payment_request.as_str(),
                )
                .to_string(),
                payment_request: plan.payment_request.clone(),
                amount_sats: plan.amount_sats,
                idempotency_key: payout_dispatch_idempotency_key(plan.payout_key.as_str()),
            })
            .await
        {
            Ok(receipt) => {
                payments.push(PaymentSummary {
                    id: receipt.payment_id.clone(),
                    direction: "send".to_string(),
                    status: "completed".to_string(),
                    amount_sats: plan.amount_sats,
                    fees_sats: 0,
                    timestamp: dispatch_timestamp_seconds,
                    method: "ldk".to_string(),
                    description: None,
                    invoice: Some(plan.payment_request.clone()),
                    destination_pubkey: None,
                    payment_hash: None,
                    htlc_status: None,
                    htlc_expiry_epoch_seconds: None,
                    status_detail: None,
                });
                outcomes.push(TreasuryDispatchOutcome::Dispatched {
                    payout_key: plan.payout_key.clone(),
                    payment_id: receipt.payment_id,
                    terminal_event_state: receipt.terminal_event_state,
                });
            }
            Err(error) => outcomes.push(TreasuryDispatchOutcome::Failed {
                payout_key: plan.payout_key.clone(),
                reason: error.normalized_reason(),
            }),
        }
    }
    let (wallet_snapshot, wallet_error) = match load_ldk_server_balances(config).await {
        Ok(balances) => (Some(ldk_wallet_snapshot(config, balances, payments)), None),
        Err(error) => (None, Some(error.to_string())),
    };
    TreasuryDispatchBatchResult {
        outcomes,
        wallet_snapshot,
        wallet_error,
    }
}

pub async fn create_live_funding_target(
    config: &TreasuryConfig,
    request: TreasuryFundingTargetRequest,
) -> Result<TreasuryFundingMaterial> {
    if config.simulated_wallet_enabled {
        return Ok(simulated_funding_target(config, request));
    }
    #[cfg(test)]
    {
        let hook = test_wallet_funding_hook()
            .lock()
            .expect("treasury funding hook")
            .clone();
        if let Some(hook) = hook {
            return hook(request).await;
        }
    }
    create_ldk_provider_funding_target(config, request).await
}

pub async fn load_live_wallet_snapshot(
    config: &TreasuryConfig,
    create_if_missing: bool,
) -> Result<TreasuryWalletSnapshot> {
    load_live_wallet_refresh_result_with_plan(
        config,
        create_if_missing,
        TreasuryWalletRefreshPlan::recent_only(),
    )
    .await
    .map(|result| result.snapshot)
}

pub async fn load_live_wallet_refresh_result_with_plan(
    config: &TreasuryConfig,
    create_if_missing: bool,
    _refresh_plan: TreasuryWalletRefreshPlan,
) -> Result<TreasuryWalletRefreshResult> {
    let _ = create_if_missing;
    if config.simulated_wallet_enabled {
        return Ok(TreasuryWalletRefreshResult {
            snapshot: simulated_wallet_snapshot(config, Vec::new()),
            progress: TreasuryWalletRefreshProgress::default(),
        });
    }

    #[cfg(test)]
    if let Some(hook) = test_wallet_snapshot_hook()
        .lock()
        .expect("treasury snapshot hook")
        .as_ref()
    {
        return hook().map(|snapshot| TreasuryWalletRefreshResult {
            snapshot,
            progress: TreasuryWalletRefreshProgress::default(),
        });
    }

    let balances = load_ldk_server_balances(config).await?;

    Ok(TreasuryWalletRefreshResult {
        snapshot: ldk_wallet_snapshot(config, balances, Vec::new()),
        progress: TreasuryWalletRefreshProgress::default(),
    })
}

pub async fn load_live_wallet_snapshot_with_plan(
    config: &TreasuryConfig,
    create_if_missing: bool,
    refresh_plan: TreasuryWalletRefreshPlan,
) -> Result<TreasuryWalletSnapshot> {
    load_live_wallet_refresh_result_with_plan(config, create_if_missing, refresh_plan)
        .await
        .map(|result| result.snapshot)
}

pub async fn dispatch_live_payouts(
    config: &TreasuryConfig,
    plans: &[TreasuryDispatchPlan],
) -> TreasuryDispatchBatchResult {
    if plans.is_empty() {
        return TreasuryDispatchBatchResult::default();
    }

    if config.simulated_wallet_enabled {
        return dispatch_with_simulated_wallet(config, plans);
    }
    #[cfg(test)]
    {
        let send_hook = test_wallet_send_hook()
            .lock()
            .expect("treasury send hook")
            .clone();
        let snapshot_hook = test_wallet_snapshot_hook()
            .lock()
            .expect("treasury snapshot hook")
            .clone();
        if send_hook.is_some() || snapshot_hook.is_some() {
            return dispatch_with_test_hooks(config, plans);
        }
    }
    dispatch_with_ldk_provider(config, plans).await
}

fn classify_wallet_send_failure(reason: &str) -> String {
    let normalized = reason.trim();
    let lowered = normalized.to_ascii_lowercase();
    let failure_class = if lowered.contains("operation was canceled")
        || lowered.contains("status: cancelled")
        || lowered.contains("status: canceled")
    {
        Some("cancelled_transport")
    } else if lowered.contains("treeserviceerror(insufficientfunds)")
        || lowered.contains("tree service error")
        || lowered.contains("insufficient funds")
    {
        Some("leaf_selection")
    } else if lowered.contains("service connection error")
        || lowered.contains("connection error")
        || lowered.contains("transport error")
    {
        Some("transport")
    } else {
        None
    };

    match failure_class {
        Some(classification) => format!("wallet_send_retryable:{classification}:{normalized}"),
        None => format!("wallet_send_failed:unknown:{normalized}"),
    }
}

async fn dispatch_outcome_from_send_future<F, E>(
    plan: TreasuryDispatchPlan,
    send_timeout_ms: u64,
    send_future: F,
) -> TreasuryDispatchOutcome
where
    F: Future<Output = std::result::Result<String, E>>,
    E: std::fmt::Display,
{
    match tokio::time::timeout(Duration::from_millis(send_timeout_ms), send_future).await {
        Ok(Ok(payment_id)) => TreasuryDispatchOutcome::Dispatched {
            payout_key: plan.payout_key,
            payment_id,
            terminal_event_state: Some("submitted".to_string()),
        },
        Ok(Err(error)) => TreasuryDispatchOutcome::Failed {
            payout_key: plan.payout_key,
            reason: classify_wallet_send_failure(error.to_string().as_str()),
        },
        Err(_) => TreasuryDispatchOutcome::Failed {
            payout_key: plan.payout_key,
            reason: format!("wallet_send_timeout:{send_timeout_ms}"),
        },
    }
}

pub fn parse_treasury_command(args: &[String]) -> Result<TreasuryCommand> {
    match args.get(2).map(String::as_str) {
        None => Ok(TreasuryCommand::Status { json: false }),
        Some("status") => Ok(TreasuryCommand::Status {
            json: parse_json_only(args, 3, "treasury status")?,
        }),
        Some("funding-target") => {
            let mut amount_sats = None;
            let mut description = None;
            let mut expiry_seconds = None;
            let mut json = false;
            let mut index = 3usize;
            while index < args.len() {
                match args[index].as_str() {
                    "--amount-sats" => {
                        index += 1;
                        let raw = args
                            .get(index)
                            .ok_or_else(|| anyhow!("missing value for --amount-sats"))?;
                        let parsed = raw
                            .parse::<u64>()
                            .with_context(|| format!("invalid treasury amount_sats: {raw}"))?;
                        if parsed == 0 {
                            bail!("treasury amount_sats must be greater than 0");
                        }
                        amount_sats = Some(parsed);
                        index += 1;
                    }
                    "--description" => {
                        index += 1;
                        description = Some(
                            args.get(index)
                                .ok_or_else(|| anyhow!("missing value for --description"))?
                                .trim()
                                .to_string(),
                        );
                        index += 1;
                    }
                    "--expiry-seconds" => {
                        index += 1;
                        let raw = args
                            .get(index)
                            .ok_or_else(|| anyhow!("missing value for --expiry-seconds"))?;
                        let parsed = raw
                            .parse::<u32>()
                            .with_context(|| format!("invalid treasury expiry_seconds: {raw}"))?;
                        if parsed == 0 {
                            bail!("treasury expiry_seconds must be greater than 0");
                        }
                        expiry_seconds = Some(parsed);
                        index += 1;
                    }
                    "--json" => {
                        json = true;
                        index += 1;
                    }
                    other => bail!("unexpected argument for treasury funding-target: {other}"),
                }
            }
            Ok(TreasuryCommand::FundingTarget {
                amount_sats,
                description: description.filter(|value| !value.is_empty()),
                expiry_seconds,
                json,
            })
        }
        Some("payout-ledger-cleanup") => {
            let mut apply = false;
            let mut report_path = None;
            let mut json = false;
            let mut index = 3usize;
            while index < args.len() {
                match args[index].as_str() {
                    "--apply" => {
                        apply = true;
                        index += 1;
                    }
                    "--report-path" => {
                        index += 1;
                        let raw = args
                            .get(index)
                            .ok_or_else(|| anyhow!("missing value for --report-path"))?;
                        report_path = Some(PathBuf::from(raw));
                        index += 1;
                    }
                    "--json" => {
                        json = true;
                        index += 1;
                    }
                    other => {
                        bail!("unexpected argument for treasury payout-ledger-cleanup: {other}")
                    }
                }
            }
            Ok(TreasuryCommand::PayoutLedgerCleanup {
                apply,
                report_path,
                json,
            })
        }
        Some("recovery-report") => {
            let mut work_dir = None;
            let mut report_path = None;
            let mut json = false;
            let mut index = 3usize;
            while index < args.len() {
                match args[index].as_str() {
                    "--work-dir" => {
                        index += 1;
                        let raw = args
                            .get(index)
                            .ok_or_else(|| anyhow!("missing value for --work-dir"))?;
                        work_dir = Some(PathBuf::from(raw));
                        index += 1;
                    }
                    "--report-path" => {
                        index += 1;
                        let raw = args
                            .get(index)
                            .ok_or_else(|| anyhow!("missing value for --report-path"))?;
                        report_path = Some(PathBuf::from(raw));
                        index += 1;
                    }
                    "--json" => {
                        json = true;
                        index += 1;
                    }
                    other => bail!("unexpected argument for treasury recovery-report: {other}"),
                }
            }
            Ok(TreasuryCommand::RecoveryReport {
                work_dir,
                report_path,
                json,
            })
        }
        Some("recovery-cutover") => {
            let mut report_path = None;
            let mut json = false;
            let mut index = 3usize;
            while index < args.len() {
                match args[index].as_str() {
                    "--report-path" => {
                        index += 1;
                        let raw = args
                            .get(index)
                            .ok_or_else(|| anyhow!("missing value for --report-path"))?;
                        report_path = Some(PathBuf::from(raw));
                        index += 1;
                    }
                    "--json" => {
                        json = true;
                        index += 1;
                    }
                    other => bail!("unexpected argument for treasury recovery-cutover: {other}"),
                }
            }
            Ok(TreasuryCommand::RecoveryCutover {
                report_path: report_path
                    .ok_or_else(|| anyhow!("treasury recovery-cutover requires --report-path"))?,
                json,
            })
        }
        Some(other) => bail!("unsupported treasury command: {other}"),
    }
}

pub async fn run_treasury_command(
    config: &TreasuryConfig,
    command: &TreasuryCommand,
) -> Result<String> {
    match command {
        TreasuryCommand::Status { json } => {
            let snapshot = load_live_wallet_snapshot(config, true).await?;
            let mut state = TreasuryState::new(config.state_path.clone());
            let now_unix_ms = now_unix_ms();
            state.initialize_runtime_policy(config, now_unix_ms);
            state.apply_wallet_snapshot(&snapshot, now_unix_ms);
            state.sync_continuity_alerts(config, now_unix_ms);
            state.refresh_public_snapshot(config, now_unix_ms);
            let response = state.status_response(config, now_unix_ms);
            if *json {
                return Ok(serde_json::to_string_pretty(&response)?);
            }
            Ok(render_treasury_status_response(&response))
        }
        TreasuryCommand::FundingTarget {
            amount_sats,
            description,
            expiry_seconds,
            json,
        } => {
            let mut material = create_live_funding_target(
                config,
                TreasuryFundingTargetRequest {
                    amount_sats: *amount_sats,
                    description: description.clone(),
                    expiry_seconds: *expiry_seconds,
                },
            )
            .await?;
            let now_unix_ms = now_unix_ms();
            material
                .phase_timings
                .invoice_returned_at_unix_ms
                .get_or_insert(now_unix_ms);
            let response = TreasuryFundingTargetResponse {
                authority: "openagents-hosted-nexus".to_string(),
                wallet_runtime_status: material.wallet_snapshot.runtime_status,
                wallet_runtime_detail: material.wallet_snapshot.runtime_detail,
                wallet_hydration_mode: material.wallet_snapshot.wallet_hydration_mode,
                wallet_payment_scan_mode: material.wallet_snapshot.wallet_payment_scan_mode,
                wallet_balance_sats: material.wallet_snapshot.balance_sats,
                wallet_balance_updated_at_unix_ms: now_unix_ms,
                provider_target: material.provider_target,
                bitcoin_address: material.bitcoin_address,
                provider_invoice: material.provider_invoice,
                bolt11_invoice: material.bolt11_invoice,
                provider_payment_id_hash: material
                    .provider_payment_id
                    .as_deref()
                    .map(treasury_hash),
                phase_timings: material.phase_timings,
            };
            if *json {
                return Ok(serde_json::to_string_pretty(&response)?);
            }
            Ok(render_treasury_funding_target_response(&response))
        }
        TreasuryCommand::PayoutLedgerCleanup {
            apply,
            report_path,
            json,
        } => {
            let mut state = TreasuryState::new(config.state_path.clone());
            let now_unix_ms = now_unix_ms();
            let report = state.payout_ledger_cleanup_report(*apply, now_unix_ms);
            if let Some(report_path) = report_path {
                write_json_file(report_path.as_path(), &report)?;
            }
            if *json {
                return Ok(serde_json::to_string_pretty(&report)?);
            }
            Ok(render_treasury_payout_ledger_cleanup_report(&report))
        }
        TreasuryCommand::RecoveryReport {
            work_dir,
            report_path,
            json,
        } => {
            let report = generate_treasury_wallet_recovery_report(
                config,
                work_dir.as_deref(),
                report_path.as_deref(),
            )
            .await?;
            if *json {
                return Ok(serde_json::to_string_pretty(&report)?);
            }
            Ok(render_treasury_wallet_recovery_report(&report))
        }
        TreasuryCommand::RecoveryCutover { report_path, json } => {
            let response = apply_treasury_wallet_recovery_cutover(config, report_path.as_path())?;
            if *json {
                return Ok(serde_json::to_string_pretty(&response)?);
            }
            Ok(render_treasury_wallet_recovery_cutover_response(&response))
        }
    }
}

fn default_wallet_storage_runtime_mode() -> String {
    "original".to_string()
}

fn default_treasury_wallet_recovery_work_dir(config: &TreasuryConfig, now_unix_ms: u64) -> PathBuf {
    config
        .state_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(format!("treasury-wallet-recovery-{now_unix_ms}"))
}

fn ensure_directory_empty(path: &Path, label: &str) -> Result<()> {
    if path.exists() {
        if path.is_file() {
            bail!("{label} must be a directory: {}", path.display());
        }
        let mut entries = fs::read_dir(path)
            .with_context(|| format!("failed to read {label} {}", path.display()))?;
        if entries.next().transpose()?.is_some() {
            bail!(
                "{label} already exists and is not empty: {}",
                path.display()
            );
        }
    } else {
        fs::create_dir_all(path)
            .with_context(|| format!("failed to create {label} {}", path.display()))?;
    }
    Ok(())
}

fn copy_file_preserving_permissions(source: &Path, destination: &Path) -> Result<()> {
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create dir {}", parent.display()))?;
    }
    fs::copy(source, destination).with_context(|| {
        format!(
            "failed to copy file {} -> {}",
            source.display(),
            destination.display()
        )
    })?;
    let permissions = fs::metadata(source)
        .with_context(|| format!("failed to stat {}", source.display()))?
        .permissions();
    fs::set_permissions(destination, permissions)
        .with_context(|| format!("failed to set permissions on {}", destination.display()))?;
    Ok(())
}

fn copy_directory_tree(source: &Path, destination: &Path) -> Result<()> {
    if !source.exists() {
        bail!("source path does not exist: {}", source.display());
    }
    if source.is_file() {
        return copy_file_preserving_permissions(source, destination);
    }
    fs::create_dir_all(destination)
        .with_context(|| format!("failed to create dir {}", destination.display()))?;
    for entry in fs::read_dir(source)
        .with_context(|| format!("failed to read source dir {}", source.display()))?
    {
        let entry = entry.with_context(|| format!("failed to read {}", source.display()))?;
        let entry_type = entry
            .file_type()
            .with_context(|| format!("failed to inspect {}", entry.path().display()))?;
        let target_path = destination.join(entry.file_name());
        if entry_type.is_dir() {
            copy_directory_tree(entry.path().as_path(), target_path.as_path())?;
        } else if entry_type.is_file() {
            copy_file_preserving_permissions(entry.path().as_path(), target_path.as_path())?;
        } else {
            bail!(
                "unsupported file type in treasury wallet backup: {}",
                entry.path().display()
            );
        }
    }
    Ok(())
}

fn write_json_file<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create dir {}", parent.display()))?;
    }
    let payload = serde_json::to_string_pretty(value)?;
    fs::write(path, format!("{payload}\n"))
        .with_context(|| format!("failed to write {}", path.display()))?;
    Ok(())
}

fn write_secret_file(path: &Path, contents: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create dir {}", parent.display()))?;
    }
    fs::write(path, contents).with_context(|| format!("failed to write {}", path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))
            .with_context(|| format!("failed to chmod {}", path.display()))?;
    }
    Ok(())
}

fn aggregate_payment_summaries(payments: &[PaymentSummary]) -> TreasuryWalletPaymentAggregate {
    let mut aggregate = TreasuryWalletPaymentAggregate {
        total_payments: payments.len() as u64,
        ..TreasuryWalletPaymentAggregate::default()
    };
    for payment in payments {
        let direction = payment.direction.as_str();
        let status = payment.status.as_str();
        if direction.eq_ignore_ascii_case("receive") && status.eq_ignore_ascii_case("completed") {
            aggregate.completed_receive_count = aggregate.completed_receive_count.saturating_add(1);
            aggregate.completed_receive_total_sats = aggregate
                .completed_receive_total_sats
                .saturating_add(payment.amount_sats);
        }
        if direction.eq_ignore_ascii_case("send") {
            if status.eq_ignore_ascii_case("completed") {
                aggregate.completed_send_count = aggregate.completed_send_count.saturating_add(1);
                aggregate.completed_send_total_sats = aggregate
                    .completed_send_total_sats
                    .saturating_add(payment.amount_sats);
                aggregate.max_completed_send_sats =
                    aggregate.max_completed_send_sats.max(payment.amount_sats);
            } else if status.eq_ignore_ascii_case("pending") {
                aggregate.pending_send_count = aggregate.pending_send_count.saturating_add(1);
                aggregate.pending_send_total_sats = aggregate
                    .pending_send_total_sats
                    .saturating_add(payment.amount_sats);
            } else if status.eq_ignore_ascii_case("failed") {
                aggregate.failed_send_count = aggregate.failed_send_count.saturating_add(1);
                aggregate.failed_send_total_sats = aggregate
                    .failed_send_total_sats
                    .saturating_add(payment.amount_sats);
            }
        }
    }
    aggregate
}

fn signed_balance_delta(newer: u64, older: u64) -> Option<i64> {
    if newer >= older {
        i64::try_from(newer.saturating_sub(older)).ok()
    } else {
        i64::try_from(older.saturating_sub(newer))
            .ok()
            .map(|delta| -delta)
    }
}

fn lag_since(now_unix_ms: u64, timestamp_unix_ms: Option<u64>) -> Option<u64> {
    timestamp_unix_ms.map(|timestamp| now_unix_ms.saturating_sub(timestamp))
}

fn build_treasury_wallet_recovery_comparison(
    current_storage: &TreasuryWalletInspection,
    rebuilt_storage: &TreasuryWalletInspection,
) -> TreasuryWalletRecoveryComparison {
    let wallet_identity_pubkey_match = !current_storage.wallet_identity_pubkey.is_empty()
        && current_storage.wallet_identity_pubkey == rebuilt_storage.wallet_identity_pubkey;
    let rebuilt_minus_current_balance_sats =
        match (rebuilt_storage.balance_sats, current_storage.balance_sats) {
            (Some(rebuilt), Some(current)) => signed_balance_delta(rebuilt, current),
            _ => None,
        };
    let current_zero_with_receive_history = current_storage.balance_sats == Some(0)
        && current_storage.payment_totals.completed_receive_total_sats
            > current_storage
                .payment_totals
                .completed_send_total_sats
                .saturating_add(TREASURY_IMPOSSIBLE_ZERO_BALANCE_THRESHOLD_SATS);
    let major_divergence_detected =
        match (current_storage.balance_sats, rebuilt_storage.balance_sats) {
            (Some(current), Some(rebuilt)) => {
                let delta = current.abs_diff(rebuilt);
                let larger_balance = current.max(rebuilt);
                current_zero_with_receive_history
                    || delta >= TREASURY_IMPOSSIBLE_ZERO_BALANCE_THRESHOLD_SATS
                    || (larger_balance > 0 && delta.saturating_mul(100) >= larger_balance * 10)
            }
            _ => current_zero_with_receive_history,
        };
    let inspection_outputs_usable =
        current_storage.balance_sats.is_some() && rebuilt_storage.balance_sats.is_some();
    let validation_passed = current_storage.error.is_none()
        && rebuilt_storage.error.is_none()
        && wallet_identity_pubkey_match
        && inspection_outputs_usable;
    let fully_synced = current_storage.runtime_status.as_deref() == Some("synced")
        && rebuilt_storage.runtime_status.as_deref() == Some("synced");
    let rebuilt_is_balance_regression = matches!(
        (current_storage.balance_sats, rebuilt_storage.balance_sats),
        (Some(current), Some(rebuilt)) if rebuilt < current
    );
    let rebuilt_has_recovery_evidence = matches!(
        (current_storage.balance_sats, rebuilt_storage.balance_sats),
        (Some(current), Some(rebuilt)) if rebuilt > current
    ) || (current_zero_with_receive_history
        && rebuilt_storage.balance_sats.unwrap_or_default()
            > TREASURY_IMPOSSIBLE_ZERO_BALANCE_THRESHOLD_SATS);
    let recommended_action = if !validation_passed {
        "inspect_errors".to_string()
    } else if major_divergence_detected
        && rebuilt_has_recovery_evidence
        && !rebuilt_is_balance_regression
        && fully_synced
    {
        "cutover_rebuilt_storage_after_service_stop".to_string()
    } else if major_divergence_detected && !fully_synced {
        "retry_live_sync_before_cutover".to_string()
    } else if major_divergence_detected {
        "inspect_divergence_before_cutover".to_string()
    } else if !fully_synced {
        "no_cutover_needed_sync_timeout_cached".to_string()
    } else {
        "no_cutover_needed".to_string()
    };
    TreasuryWalletRecoveryComparison {
        wallet_identity_pubkey_match,
        rebuilt_minus_current_balance_sats,
        current_zero_with_receive_history,
        major_divergence_detected,
        validation_passed,
        recommended_action,
    }
}

async fn inspect_treasury_wallet_storage(
    _config: &TreasuryConfig,
    _mnemonic: &str,
    storage_dir: &Path,
) -> TreasuryWalletInspection {
    TreasuryWalletInspection {
        inspected_storage_dir: storage_dir.display().to_string(),
        runtime_status: Some("unsupported".to_string()),
        runtime_detail: Some(
            "Legacy wallet storage inspection is not part of the LDK-only Nexus runtime"
                .to_string(),
        ),
        error: Some("legacy_wallet_inspection_removed_from_active_runtime".to_string()),
        ..TreasuryWalletInspection::default()
    }
}

async fn generate_treasury_wallet_recovery_report(
    config: &TreasuryConfig,
    work_dir_override: Option<&Path>,
    report_path_override: Option<&Path>,
) -> Result<TreasuryWalletRecoveryReport> {
    let now_unix_ms = now_unix_ms();
    let work_dir = work_dir_override
        .map(PathBuf::from)
        .or_else(|| report_path_override.and_then(|path| path.parent().map(PathBuf::from)))
        .unwrap_or_else(|| default_treasury_wallet_recovery_work_dir(config, now_unix_ms));
    ensure_directory_empty(work_dir.as_path(), "treasury recovery work dir")?;

    let backup_root_dir = work_dir.join("backup");
    let current_storage_backup_dir = backup_root_dir.join("current-storage");
    let rebuilt_storage_dir = work_dir.join("rebuilt-storage");
    let report_path = report_path_override
        .map(PathBuf::from)
        .unwrap_or_else(|| work_dir.join("recovery-report.json"));
    let mnemonic_backup_path = backup_root_dir.join("treasury.mnemonic");
    let state_backup_path = if config.state_path.exists() {
        Some(backup_root_dir.join("treasury-state.json"))
    } else {
        None
    };

    ensure_directory_empty(backup_root_dir.as_path(), "treasury recovery backup dir")?;
    ensure_directory_empty(
        rebuilt_storage_dir.as_path(),
        "treasury recovery rebuilt storage dir",
    )?;
    copy_directory_tree(
        config.wallet_storage_dir.as_path(),
        current_storage_backup_dir.as_path(),
    )?;

    let mnemonic = ensure_wallet_mnemonic(config.wallet_mnemonic_path.as_path(), false)?;
    write_secret_file(
        mnemonic_backup_path.as_path(),
        format!("{mnemonic}\n").as_str(),
    )?;
    if let Some(state_backup_path) = state_backup_path.as_ref() {
        copy_file_preserving_permissions(config.state_path.as_path(), state_backup_path.as_path())?;
    }

    let (current_storage, rebuilt_storage) = if config.wallet_recovery_parallel_inspections {
        let current_storage = inspect_treasury_wallet_storage(
            config,
            mnemonic.as_str(),
            current_storage_backup_dir.as_path(),
        );
        let rebuilt_storage = inspect_treasury_wallet_storage(
            config,
            mnemonic.as_str(),
            rebuilt_storage_dir.as_path(),
        );
        tokio::join!(current_storage, rebuilt_storage)
    } else {
        let current_storage = inspect_treasury_wallet_storage(
            config,
            mnemonic.as_str(),
            current_storage_backup_dir.as_path(),
        )
        .await;
        let rebuilt_storage = inspect_treasury_wallet_storage(
            config,
            mnemonic.as_str(),
            rebuilt_storage_dir.as_path(),
        )
        .await;
        (current_storage, rebuilt_storage)
    };
    let comparison = build_treasury_wallet_recovery_comparison(&current_storage, &rebuilt_storage);

    let report = TreasuryWalletRecoveryReport {
        authority: "openagents-hosted-nexus".to_string(),
        generated_at_unix_ms: now_unix_ms,
        source_wallet_storage_dir: config.wallet_storage_dir.display().to_string(),
        backup_root_dir: backup_root_dir.display().to_string(),
        current_storage_backup_dir: current_storage_backup_dir.display().to_string(),
        rebuilt_storage_dir: rebuilt_storage_dir.display().to_string(),
        report_path: report_path.display().to_string(),
        mnemonic_backup_path: mnemonic_backup_path.display().to_string(),
        state_backup_path: state_backup_path
            .as_ref()
            .map(|path| path.display().to_string()),
        current_storage,
        rebuilt_storage,
        comparison,
        cutover_active_storage_dir: None,
        cutover_rollback_storage_dir: None,
        cutover_completed_at_unix_ms: None,
    };
    write_json_file(report_path.as_path(), &report)?;

    let mut state = TreasuryState::new(config.state_path.clone());
    state.note_wallet_recovery_report(&report);
    state.refresh_public_snapshot(config, now_unix_ms);

    Ok(report)
}

fn load_treasury_wallet_recovery_report(path: &Path) -> Result<TreasuryWalletRecoveryReport> {
    let payload =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    serde_json::from_str::<TreasuryWalletRecoveryReport>(payload.as_str())
        .with_context(|| format!("failed to parse {}", path.display()))
}

fn recovery_cutover_rollback_dir(active_storage_dir: &Path, now_unix_ms: u64) -> PathBuf {
    let file_name = active_storage_dir
        .file_name()
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_else(|| "treasury-wallet".to_string());
    active_storage_dir
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(format!("{file_name}-rollback-{now_unix_ms}"))
}

fn apply_treasury_wallet_recovery_cutover(
    config: &TreasuryConfig,
    report_path: &Path,
) -> Result<TreasuryWalletRecoveryCutoverResponse> {
    let mut report = load_treasury_wallet_recovery_report(report_path)?;
    if report.source_wallet_storage_dir != config.wallet_storage_dir.display().to_string() {
        bail!(
            "recovery report source storage {} does not match configured live storage {}",
            report.source_wallet_storage_dir,
            config.wallet_storage_dir.display()
        );
    }
    if !report.comparison.validation_passed {
        bail!("recovery report did not pass validation");
    }
    if !report.comparison.wallet_identity_pubkey_match {
        bail!("recovery report wallet identity does not match");
    }
    if report.comparison.recommended_action != "cutover_rebuilt_storage_after_service_stop" {
        bail!(
            "recovery report does not recommend cutover: {}",
            report.comparison.recommended_action
        );
    }

    let rebuilt_storage_dir = PathBuf::from(report.rebuilt_storage_dir.as_str());
    if !rebuilt_storage_dir.exists() {
        bail!(
            "rebuilt storage dir missing: {}",
            rebuilt_storage_dir.display()
        );
    }

    let now_unix_ms = now_unix_ms();
    let rollback_storage_dir =
        recovery_cutover_rollback_dir(config.wallet_storage_dir.as_path(), now_unix_ms);
    if config.wallet_storage_dir.exists() {
        fs::rename(
            config.wallet_storage_dir.as_path(),
            rollback_storage_dir.as_path(),
        )
        .with_context(|| {
            format!(
                "failed to preserve rollback wallet storage {} -> {}",
                config.wallet_storage_dir.display(),
                rollback_storage_dir.display()
            )
        })?;
    } else {
        bail!(
            "configured live wallet storage dir is missing: {}",
            config.wallet_storage_dir.display()
        );
    }

    if let Err(error) = fs::rename(
        rebuilt_storage_dir.as_path(),
        config.wallet_storage_dir.as_path(),
    ) {
        let restore_result = fs::rename(
            rollback_storage_dir.as_path(),
            config.wallet_storage_dir.as_path(),
        );
        if let Err(restore_error) = restore_result {
            bail!(
                "failed to activate rebuilt storage: {error}; failed to restore rollback: {restore_error}"
            );
        }
        bail!("failed to activate rebuilt storage: {error}");
    }

    let response = TreasuryWalletRecoveryCutoverResponse {
        authority: "openagents-hosted-nexus".to_string(),
        report_path: report_path.display().to_string(),
        active_storage_dir: config.wallet_storage_dir.display().to_string(),
        rollback_storage_dir: rollback_storage_dir.display().to_string(),
        wallet_storage_runtime_mode: "rebuilt".to_string(),
        cutover_completed_at_unix_ms: now_unix_ms,
    };
    report.cutover_active_storage_dir = Some(response.active_storage_dir.clone());
    report.cutover_rollback_storage_dir = Some(response.rollback_storage_dir.clone());
    report.cutover_completed_at_unix_ms = Some(now_unix_ms);
    write_json_file(report_path, &report)?;

    let mut state = TreasuryState::new(config.state_path.clone());
    state.note_wallet_recovery_report(&report);
    state.note_wallet_recovery_cutover(&response);
    if let Some(balance_sats) = report.rebuilt_storage.balance_sats {
        state.wallet_balance_sats = balance_sats;
        state.wallet_balance_updated_at_unix_ms = Some(now_unix_ms);
        state.last_wallet_sync_at_unix_ms = Some(now_unix_ms);
    }
    state.wallet_runtime_status = report.rebuilt_storage.runtime_status.clone();
    state.wallet_last_error = report.rebuilt_storage.error.clone();
    state.refresh_public_snapshot(config, now_unix_ms);

    Ok(response)
}

fn treasury_policy_changed_fields(
    before: &TreasuryRuntimePolicy,
    after: &TreasuryRuntimePolicy,
) -> Vec<String> {
    let mut changed_fields = Vec::new();
    if before.treasury_enabled != after.treasury_enabled {
        changed_fields.push("treasury_enabled".to_string());
    }
    if before.payout_sats_per_window != after.payout_sats_per_window {
        changed_fields.push("payout_sats_per_window".to_string());
    }
    if before.payout_interval_seconds != after.payout_interval_seconds {
        changed_fields.push("payout_interval_seconds".to_string());
    }
    if before.require_sellable != after.require_sellable {
        changed_fields.push("require_sellable".to_string());
    }
    if before.daily_budget_cap_sats != after.daily_budget_cap_sats {
        changed_fields.push("daily_budget_cap_sats".to_string());
    }
    if before.accepted_work_default_payout_sats() != after.accepted_work_default_payout_sats() {
        changed_fields.push("accepted_work_default_payout_sats".to_string());
    }
    if before.accepted_work_daily_budget_cap_sats() != after.accepted_work_daily_budget_cap_sats() {
        changed_fields.push("accepted_work_daily_budget_cap_sats".to_string());
    }
    if before.placeholder_payout_mode != after.placeholder_payout_mode {
        changed_fields.push("placeholder_payout_mode".to_string());
    }
    if before.dedupe_placeholder_hosts != after.dedupe_placeholder_hosts {
        changed_fields.push("dedupe_placeholder_hosts".to_string());
    }
    if before.availability_max_concurrent_sends != after.availability_max_concurrent_sends {
        changed_fields.push("availability_max_concurrent_sends".to_string());
    }
    if before.min_new_accrual_pylon_version != after.min_new_accrual_pylon_version {
        changed_fields.push("min_new_accrual_pylon_version".to_string());
    }
    if before.min_new_accrual_started_at_unix_ms != after.min_new_accrual_started_at_unix_ms {
        changed_fields.push("min_new_accrual_started_at_unix_ms".to_string());
    }
    changed_fields
}

fn treasury_policy_change_is_destructive(
    before: &TreasuryRuntimePolicy,
    after: &TreasuryRuntimePolicy,
) -> bool {
    (before.treasury_enabled && !after.treasury_enabled)
        || after.payout_sats_per_window < before.payout_sats_per_window
        || after.daily_budget_cap_sats < before.daily_budget_cap_sats
        || after.accepted_work_default_payout_sats() < before.accepted_work_default_payout_sats()
        || after.accepted_work_daily_budget_cap_sats()
            < before.accepted_work_daily_budget_cap_sats()
        || after.payout_interval_seconds > before.payout_interval_seconds
        || (!before.require_sellable && after.require_sellable)
        || treasury_policy_placeholder_lane_is_more_restrictive(before, after)
        || treasury_policy_gate_is_more_restrictive(before, after)
}

fn build_treasury_policy_change_record(
    before: Option<&TreasuryRuntimePolicy>,
    after: &TreasuryRuntimePolicy,
    source: &str,
    reason: &str,
    now_unix_ms: u64,
) -> TreasuryPolicyChangeRecord {
    let changed_fields = before
        .map(|before| treasury_policy_changed_fields(before, after))
        .unwrap_or_else(|| {
            vec![
                "treasury_enabled".to_string(),
                "payout_sats_per_window".to_string(),
                "payout_interval_seconds".to_string(),
                "require_sellable".to_string(),
                "daily_budget_cap_sats".to_string(),
                "placeholder_payout_mode".to_string(),
                "dedupe_placeholder_hosts".to_string(),
                "min_new_accrual_pylon_version".to_string(),
                "min_new_accrual_started_at_unix_ms".to_string(),
            ]
        });
    let destructive = before
        .map(|before| treasury_policy_change_is_destructive(before, after))
        .unwrap_or(false);
    let checksum_after_suffix = after.checksum.trim_start_matches("sha256:");
    TreasuryPolicyChangeRecord {
        change_id: format!("treasury-policy-{now_unix_ms}-{checksum_after_suffix}"),
        applied_at_unix_ms: now_unix_ms,
        source: source.to_string(),
        reason: reason.to_string(),
        checksum_before: before.map(|policy| policy.checksum.clone()),
        checksum_after: after.checksum.clone(),
        changed_fields,
        destructive,
    }
}

fn treasury_policy_change_receipt(
    change_record: &TreasuryPolicyChangeRecord,
) -> TreasuryReceiptEvent {
    let mut attributes = BTreeMap::new();
    attributes.insert("source".to_string(), change_record.source.clone());
    attributes.insert("reason".to_string(), change_record.reason.clone());
    attributes.insert(
        "checksum_after".to_string(),
        change_record.checksum_after.clone(),
    );
    attributes.insert(
        "destructive".to_string(),
        change_record.destructive.to_string(),
    );
    if let Some(checksum_before) = change_record.checksum_before.as_ref() {
        attributes.insert("checksum_before".to_string(), checksum_before.clone());
    }
    if !change_record.changed_fields.is_empty() {
        attributes.insert(
            "changed_fields".to_string(),
            change_record.changed_fields.join(","),
        );
    }
    TreasuryReceiptEvent {
        receipt_type: "treasury.policy.changed",
        context: AuthorityReceiptContext {
            request_id: Some(change_record.change_id.clone()),
            status: Some("applied".to_string()),
            attributes,
            ..AuthorityReceiptContext::default()
        },
    }
}

fn treasury_policy_change_blocked_receipt(
    before: &TreasuryRuntimePolicy,
    after: &TreasuryRuntimePolicy,
    changed_fields: &[String],
    now_unix_ms: u64,
) -> TreasuryReceiptEvent {
    let mut attributes = BTreeMap::new();
    attributes.insert("checksum_before".to_string(), before.checksum.clone());
    attributes.insert("checksum_after".to_string(), after.checksum.clone());
    attributes.insert("changed_fields".to_string(), changed_fields.join(","));
    TreasuryReceiptEvent {
        receipt_type: "treasury.policy.change_blocked",
        context: AuthorityReceiptContext {
            request_id: Some(format!(
                "treasury-policy-blocked-{now_unix_ms}-{}",
                after.checksum.trim_start_matches("sha256:")
            )),
            status: Some("blocked".to_string()),
            attributes,
            ..AuthorityReceiptContext::default()
        },
    }
}

pub const fn treasury_usage() -> &'static str {
    "treasury [status [--json] | funding-target [--amount-sats <n>] [--description <text>] [--expiry-seconds <n>] [--json] | payout-ledger-cleanup [--apply] [--report-path <path>] [--json] | recovery-report [--work-dir <path>] [--report-path <path>] [--json] | recovery-cutover --report-path <path> [--json]]"
}

fn parse_json_only(args: &[String], start_index: usize, label: &str) -> Result<bool> {
    let mut json = false;
    let mut index = start_index;
    while index < args.len() {
        match args[index].as_str() {
            "--json" => {
                json = true;
                index += 1;
            }
            other => bail!("unexpected argument for {label}: {other}"),
        }
    }
    Ok(json)
}

fn treasury_payout_reconciliation_status(record: &TreasuryPayoutRecord) -> &'static str {
    match record.status.as_str() {
        "confirmed" => "settled",
        "queued" => "pending_dispatch",
        "dispatching" | "dispatched" => "pending_confirmation",
        "failed" if failed_payout_is_retryable_pending(record) => "pending_retry",
        "failed" if payout_record_is_retired_historical(record) => "retired_historical",
        "failed" => "attention_required",
        "skipped" => {
            if record.reason.as_deref() == Some("missing_payout_target") {
                "missing_payout_target"
            } else {
                "skipped"
            }
        }
        _ => "unknown",
    }
}

fn render_treasury_status_response(response: &TreasuryStatusResponse) -> String {
    let mut lines = vec![
        format!("treasury_enabled: {}", response.treasury_enabled),
        format!("wallet_balance_sats: {}", response.wallet_balance_sats),
        format!("ldk_readiness: {}", response.ldk_readiness.state),
        format!(
            "ldk_projected_channel_count: {}",
            response.ldk_readiness.projected_channel_count
        ),
        format!(
            "ldk_projected_inbound_capacity_sats: {}",
            response.ldk_readiness.projected_inbound_capacity_sats
        ),
        format!(
            "ldk_projected_outbound_capacity_sats: {}",
            response.ldk_readiness.projected_outbound_capacity_sats
        ),
        format!(
            "wallet_storage_runtime_mode: {}",
            response.wallet_storage_runtime_mode
        ),
        format!(
            "min_new_accrual_version_gate_active: {}",
            response.min_new_accrual_version_gate_active
        ),
        format!(
            "placeholder_payout_mode: {}",
            response.placeholder_payout_mode.label()
        ),
        format!(
            "dedupe_placeholder_hosts: {}",
            response.dedupe_placeholder_hosts
        ),
        format!(
            "payout_sats_paid_total: {}",
            response.payout_sats_paid_total
        ),
        format!("payout_sats_paid_24h: {}", response.payout_sats_paid_24h),
        format!(
            "accepted_work_payout_sats_paid_total: {}",
            response.accepted_work_payout_sats_paid_total
        ),
        format!(
            "accepted_work_payout_sats_paid_24h: {}",
            response.accepted_work_payout_sats_paid_24h
        ),
        format!(
            "availability_stipend_payout_sats_paid_total: {}",
            response.availability_stipend_payout_sats_paid_total
        ),
        format!(
            "availability_stipend_payout_sats_paid_24h: {}",
            response.availability_stipend_payout_sats_paid_24h
        ),
        format!(
            "placeholder_payout_sats_paid_total: {}",
            response.placeholder_payout_sats_paid_total
        ),
        format!(
            "placeholder_payout_sats_paid_24h: {}",
            response.placeholder_payout_sats_paid_24h
        ),
        format!(
            "beta_bonus_payout_sats_paid_total: {}",
            response.beta_bonus_payout_sats_paid_total
        ),
        format!(
            "beta_bonus_payout_sats_paid_24h: {}",
            response.beta_bonus_payout_sats_paid_24h
        ),
        format!(
            "weak_device_accepted_work_payout_sats_paid_total: {}",
            response.weak_device_accepted_work_payout_sats_paid_total
        ),
        format!(
            "weak_device_accepted_work_payout_sats_paid_24h: {}",
            response.weak_device_accepted_work_payout_sats_paid_24h
        ),
        format!(
            "strong_lane_accepted_work_payout_sats_paid_total: {}",
            response.strong_lane_accepted_work_payout_sats_paid_total
        ),
        format!(
            "strong_lane_accepted_work_payout_sats_paid_24h: {}",
            response.strong_lane_accepted_work_payout_sats_paid_24h
        ),
        format!(
            "registered_payout_identities: {}",
            response.registered_payout_identities
        ),
        format!(
            "ldk_payout_target_identities: {}",
            response.ldk_payout_target_identities
        ),
        format!(
            "pylon_v0_2_registration_required_identities: {}",
            response.pylon_v0_2_registration_required_identities
        ),
        format!(
            "availability_online_identities_now: {}",
            response.availability_online_identities_now
        ),
        format!(
            "availability_online_host_clusters_now: {}",
            response.availability_online_host_clusters_now
        ),
        format!(
            "availability_stipend_eligible_beneficiaries_now: {}",
            response.availability_stipend_eligible_beneficiaries_now
        ),
        format!(
            "min_new_accrual_version_blocked_online_targets: {}",
            response.min_new_accrual_version_blocked_online_targets
        ),
        format!(
            "min_new_accrual_unknown_version_online_targets: {}",
            response.min_new_accrual_unknown_version_online_targets
        ),
        format!(
            "inference_ready_online_payout_targets: {}",
            response.inference_ready_online_payout_targets
        ),
        format!(
            "duplicate_host_placeholder_blocked_online_targets: {}",
            response.duplicate_host_placeholder_blocked_online_targets
        ),
        format!(
            "duplicate_host_blocked_beneficiaries_now: {}",
            response.duplicate_host_blocked_beneficiaries_now
        ),
        format!(
            "duplicate_payout_target_blocked_beneficiaries_now: {}",
            response.duplicate_payout_target_blocked_beneficiaries_now
        ),
        format!(
            "missing_payout_target_blocked_beneficiaries_now: {}",
            response.missing_payout_target_blocked_beneficiaries_now
        ),
        format!(
            "version_floor_blocked_beneficiaries_now: {}",
            response.version_floor_blocked_beneficiaries_now
        ),
        format!(
            "readiness_blocked_beneficiaries_now: {}",
            response.readiness_blocked_beneficiaries_now
        ),
        format!(
            "training_payout_reconciliation_status: {}",
            response
                .training_payout_ledger_summary
                .reconciliation_status
        ),
        format!(
            "training_payout_record_count: {}",
            response.training_payout_ledger_summary.payout_record_count
        ),
        format!(
            "training_pending_payout_count: {}",
            response.training_payout_ledger_summary.pending_payout_count
        ),
        format!(
            "training_attention_payout_count: {}",
            response
                .training_payout_ledger_summary
                .attention_payout_count
        ),
        format!(
            "current_ldk_attention_payout_count: {}",
            response
                .training_payout_ledger_summary
                .current_ldk_attention_payout_count
        ),
        format!(
            "retired_historical_payout_count: {}",
            response
                .training_payout_ledger_summary
                .retired_historical_payout_count
        ),
        format!(
            "accepted_work_pending_payout_count: {}",
            response
                .training_payout_ledger_summary
                .accepted_work_pending_payout_count
        ),
        format!(
            "accepted_work_attention_payout_count: {}",
            response
                .training_payout_ledger_summary
                .accepted_work_attention_payout_count
        ),
        format!(
            "payout_target_identity_rows: {}",
            response.payout_target_identities.len()
        ),
        format!(
            "recent_training_payout_rows: {}",
            response.recent_training_payouts.len()
        ),
        format!(
            "availability_beneficiary_debug_rows: {}",
            response.availability_beneficiary_debug_rows.len()
        ),
    ];
    if let Some(status) = response.wallet_runtime_status.as_deref() {
        lines.push(format!("wallet_runtime_status: {status}"));
    }
    if let Some(version) = response.min_new_accrual_pylon_version.as_deref() {
        lines.push(format!("min_new_accrual_pylon_version: {version}"));
    }
    if let Some(cutoff) = response.min_new_accrual_started_at_unix_ms {
        lines.push(format!("min_new_accrual_started_at_unix_ms: {cutoff}"));
    }
    if let Some(error) = response.wallet_last_error.as_deref() {
        lines.push(format!("wallet_last_error: {error}"));
    }
    if let Some(mode) = response.wallet_hydration_mode.as_deref() {
        lines.push(format!("wallet_hydration_mode: {mode}"));
    }
    if let Some(mode) = response.wallet_payment_scan_mode.as_deref() {
        lines.push(format!("wallet_payment_scan_mode: {mode}"));
    }
    if let Some(report_path) = response.wallet_storage_report_path.as_deref() {
        lines.push(format!("wallet_storage_report_path: {report_path}"));
    }
    if let Some(rollback_dir) = response.wallet_storage_rollback_dir.as_deref() {
        lines.push(format!("wallet_storage_rollback_dir: {rollback_dir}"));
    }
    if let Some(cutover_at_unix_ms) = response.wallet_storage_cutover_at_unix_ms {
        lines.push(format!(
            "wallet_storage_cutover_at_unix_ms: {cutover_at_unix_ms}"
        ));
    }
    if let Some(generated_at_unix_ms) = response.wallet_recovery_last_report_generated_at_unix_ms {
        lines.push(format!(
            "wallet_recovery_last_report_generated_at_unix_ms: {generated_at_unix_ms}"
        ));
    }
    if let Some(validation_passed) = response.wallet_recovery_last_report_validation_passed {
        lines.push(format!(
            "wallet_recovery_last_report_validation_passed: {validation_passed}"
        ));
    }
    if let Some(status) = response.payout_loop_runtime_status.as_deref() {
        lines.push(format!("payout_loop_runtime_status: {status}"));
    }
    lines.push(format!(
        "payout_loop_health: {}",
        response.payout_loop_health
    ));
    if let Some(error) = response.payout_loop_last_error.as_deref() {
        lines.push(format!("payout_loop_last_error: {error}"));
    }
    if let Some(last_reconciliation_at_unix_ms) = response.last_payout_reconciliation_at_unix_ms {
        lines.push(format!(
            "last_payout_reconciliation_at_unix_ms: {last_reconciliation_at_unix_ms}"
        ));
    }
    if let Some(snapshot_generated_at_unix_ms) = response.public_snapshot_generated_at_unix_ms {
        lines.push(format!(
            "public_snapshot_generated_at_unix_ms: {snapshot_generated_at_unix_ms}"
        ));
    }
    lines.push(format!(
        "public_snapshot_source: {}",
        response.public_snapshot_source
    ));
    if let Some(mode) = response.public_snapshot_mode.as_deref() {
        lines.push(format!("public_snapshot_mode: {mode}"));
    }
    if let Some(status) = response.public_snapshot_health_status.as_deref() {
        lines.push(format!("public_snapshot_health_status: {status}"));
    }
    if let Some(snapshot_age_ms) = response.snapshot_age_ms {
        lines.push(format!("snapshot_age_ms: {snapshot_age_ms}"));
    }
    if let Some(last_wallet_sync_at_unix_ms) = response.last_wallet_sync_at_unix_ms {
        lines.push(format!(
            "last_wallet_sync_at_unix_ms: {last_wallet_sync_at_unix_ms}"
        ));
    }
    if let Some(last_wallet_refresh_attempt_at_unix_ms) =
        response.last_wallet_refresh_attempt_at_unix_ms
    {
        lines.push(format!(
            "last_wallet_refresh_attempt_at_unix_ms: {last_wallet_refresh_attempt_at_unix_ms}"
        ));
    }
    if let Some(wallet_sync_lag_ms) = response.wallet_sync_lag_ms {
        lines.push(format!("wallet_sync_lag_ms: {wallet_sync_lag_ms}"));
    }
    lines.push(format!("backlog_total: {}", response.backlog_total));
    lines.push(format!("backlog_retryable: {}", response.backlog_retryable));
    lines.push(format!(
        "pending_confirmation_count: {}",
        response.pending_confirmation_count
    ));
    lines.push(format!(
        "tracked_payment_backlog_count: {}",
        response.tracked_payment_backlog_count
    ));
    lines.push(format!(
        "eligible_online_payout_targets: {}",
        response.eligible_online_payout_targets
    ));
    lines.push(format!(
        "sellable_pylons_online_now: {}",
        response.sellable_pylons_online_now
    ));
    if let Some(window_started_at_unix_ms) = response.latest_eligible_window_started_at_unix_ms {
        lines.push(format!(
            "latest_eligible_window_started_at_unix_ms: {window_started_at_unix_ms}"
        ));
    }
    if let Some(last_dispatch_at_unix_ms) = response.last_dispatch_at_unix_ms {
        lines.push(format!(
            "last_dispatch_at_unix_ms: {last_dispatch_at_unix_ms}"
        ));
    }
    if let Some(last_confirmed_payout_at_unix_ms) = response.last_confirmed_payout_at_unix_ms {
        lines.push(format!(
            "last_confirmed_payout_at_unix_ms: {last_confirmed_payout_at_unix_ms}"
        ));
    }
    if let Some(eligible_window_lag_ms) = response.eligible_window_lag_ms {
        lines.push(format!("eligible_window_lag_ms: {eligible_window_lag_ms}"));
    }
    if let Some(dispatch_lag_ms) = response.dispatch_lag_ms {
        lines.push(format!("dispatch_lag_ms: {dispatch_lag_ms}"));
    }
    if let Some(confirm_lag_ms) = response.confirm_lag_ms {
        lines.push(format!("confirm_lag_ms: {confirm_lag_ms}"));
    }
    if !response.skip_reason_metrics_24h.is_empty() {
        lines.push(format!(
            "skip_reason_metrics_24h: {}",
            serde_json::to_string(&response.skip_reason_metrics_24h)
                .unwrap_or_else(|_| "[]".to_string())
        ));
    }
    if !response.fail_reason_metrics_24h.is_empty() {
        lines.push(format!(
            "fail_reason_metrics_24h: {}",
            serde_json::to_string(&response.fail_reason_metrics_24h)
                .unwrap_or_else(|_| "[]".to_string())
        ));
    }
    if !response.active_continuity_alerts.is_empty() {
        lines.push(format!(
            "active_continuity_alerts: {}",
            serde_json::to_string(&response.active_continuity_alerts)
                .unwrap_or_else(|_| "[]".to_string())
        ));
    }
    if !response.availability_beneficiary_debug_rows.is_empty() {
        lines.push(format!(
            "availability_beneficiary_debug_rows_json: {}",
            serde_json::to_string(&response.availability_beneficiary_debug_rows)
                .unwrap_or_else(|_| "[]".to_string())
        ));
    }
    if let Some(reason) = response.degraded_reason.as_deref() {
        lines.push(format!("degraded_reason: {reason}"));
    }
    if !response.degraded_states.is_empty() {
        lines.push(format!(
            "degraded_states: {}",
            serde_json::to_string(&response.degraded_states).unwrap_or_else(|_| "[]".to_string())
        ));
    }
    if let Some(policy_checksum) = response.policy_checksum.as_deref() {
        lines.push(format!("policy_checksum: {policy_checksum}"));
    }
    if let Some(status) = response.policy_runtime_status.as_deref() {
        lines.push(format!("policy_runtime_status: {status}"));
    }
    if let Some(error) = response.policy_last_error.as_deref() {
        lines.push(format!("policy_last_error: {error}"));
    }
    lines.join("\n")
}

fn render_treasury_funding_target_response(response: &TreasuryFundingTargetResponse) -> String {
    let mut lines = vec![
        format!("wallet_runtime_status: {}", response.wallet_runtime_status),
        format!("wallet_balance_sats: {}", response.wallet_balance_sats),
        format!("provider_target: {}", response.provider_target),
    ];
    if !response.bitcoin_address.trim().is_empty() {
        lines.push(format!("bitcoin_address: {}", response.bitcoin_address));
    }
    if let Some(invoice) = response.provider_invoice.as_deref() {
        lines.push(format!("provider_invoice: {invoice}"));
    }
    if let Some(invoice) = response.bolt11_invoice.as_deref() {
        lines.push(format!("bolt11_invoice: {invoice}"));
    }
    if let Some(hash) = response.provider_payment_id_hash.as_deref() {
        lines.push(format!("provider_payment_id_hash: {hash}"));
    }
    if let Some(duration_ms) = response.phase_timings.ldk_rpc_duration_ms() {
        lines.push(format!("ldk_rpc_duration_ms: {duration_ms}"));
    }
    if let Some(duration_ms) = response.phase_timings.total_duration_ms() {
        lines.push(format!("funding_target_total_duration_ms: {duration_ms}"));
    }
    if let Some(detail) = response.wallet_runtime_detail.as_deref() {
        lines.push(format!("wallet_runtime_detail: {detail}"));
    }
    if let Some(mode) = response.wallet_hydration_mode.as_deref() {
        lines.push(format!("wallet_hydration_mode: {mode}"));
    }
    if let Some(mode) = response.wallet_payment_scan_mode.as_deref() {
        lines.push(format!("wallet_payment_scan_mode: {mode}"));
    }
    lines.join("\n")
}

fn render_treasury_payout_ledger_cleanup_report(
    report: &TreasuryPayoutLedgerCleanupReport,
) -> String {
    let mut lines = vec![
        format!("state_path: {}", report.state_path),
        format!("applied: {}", report.applied),
        format!("changed: {}", report.changed),
        format!(
            "before_reconciliation_status: {}",
            report.before_summary.reconciliation_status
        ),
        format!(
            "after_reconciliation_status: {}",
            report.after_summary.reconciliation_status
        ),
        format!(
            "before_accepted_work_pending_payout_count: {}",
            report.before_summary.accepted_work_pending_payout_count
        ),
        format!(
            "after_accepted_work_pending_payout_count: {}",
            report.after_summary.accepted_work_pending_payout_count
        ),
        format!(
            "before_accepted_work_attention_payout_count: {}",
            report.before_summary.accepted_work_attention_payout_count
        ),
        format!(
            "after_accepted_work_attention_payout_count: {}",
            report.after_summary.accepted_work_attention_payout_count
        ),
        format!(
            "retired_historical_payout_count: {}",
            report.after_summary.retired_historical_payout_count
        ),
        format!(
            "retired_historical_accepted_work_payout_count: {}",
            report
                .after_summary
                .retired_historical_accepted_work_payout_count
        ),
        format!("records_retired: {}", report.records_retired.len()),
    ];
    if !report.after_disposition_counts.is_empty() {
        lines.push("after_disposition_counts:".to_string());
        for (disposition, count) in &report.after_disposition_counts {
            lines.push(format!("  {disposition}: {count}"));
        }
    }
    lines.join("\n")
}

fn render_treasury_wallet_recovery_report(report: &TreasuryWalletRecoveryReport) -> String {
    let mut lines = vec![
        format!("report_path: {}", report.report_path),
        format!(
            "source_wallet_storage_dir: {}",
            report.source_wallet_storage_dir
        ),
        format!(
            "current_storage_backup_dir: {}",
            report.current_storage_backup_dir
        ),
        format!("rebuilt_storage_dir: {}", report.rebuilt_storage_dir),
        format!(
            "wallet_identity_pubkey_match: {}",
            report.comparison.wallet_identity_pubkey_match
        ),
        format!("validation_passed: {}", report.comparison.validation_passed),
        format!(
            "major_divergence_detected: {}",
            report.comparison.major_divergence_detected
        ),
        format!(
            "recommended_action: {}",
            report.comparison.recommended_action
        ),
    ];
    if let Some(current_balance_sats) = report.current_storage.balance_sats {
        lines.push(format!(
            "current_storage_balance_sats: {current_balance_sats}"
        ));
    }
    if let Some(rebuilt_balance_sats) = report.rebuilt_storage.balance_sats {
        lines.push(format!(
            "rebuilt_storage_balance_sats: {rebuilt_balance_sats}"
        ));
    }
    if let Some(delta_sats) = report.comparison.rebuilt_minus_current_balance_sats {
        lines.push(format!("rebuilt_minus_current_balance_sats: {delta_sats}"));
    }
    if let Some(error) = report.current_storage.error.as_deref() {
        lines.push(format!("current_storage_error: {error}"));
    }
    if let Some(error) = report.rebuilt_storage.error.as_deref() {
        lines.push(format!("rebuilt_storage_error: {error}"));
    }
    lines.join("\n")
}

fn render_treasury_wallet_recovery_cutover_response(
    response: &TreasuryWalletRecoveryCutoverResponse,
) -> String {
    [
        format!("report_path: {}", response.report_path),
        format!("active_storage_dir: {}", response.active_storage_dir),
        format!("rollback_storage_dir: {}", response.rollback_storage_dir),
        format!(
            "wallet_storage_runtime_mode: {}",
            response.wallet_storage_runtime_mode
        ),
        format!(
            "cutover_completed_at_unix_ms: {}",
            response.cutover_completed_at_unix_ms
        ),
    ]
    .join("\n")
}

fn dispatched_payout_receipt(
    record: &TreasuryPayoutRecord,
    payment_id: &str,
    terminal_event_state: &str,
) -> TreasuryReceiptEvent {
    let mut attributes = payout_receipt_attributes(record);
    attributes.insert("payment_id".to_string(), payment_id.to_string());
    attributes.insert(
        "provider_payment_id_hash".to_string(),
        treasury_hash(payment_id),
    );
    attributes.insert(
        "terminal_event_state".to_string(),
        terminal_event_state.to_string(),
    );
    TreasuryReceiptEvent {
        receipt_type: "treasury.payout.dispatched",
        context: AuthorityReceiptContext {
            request_id: Some(record.payout_key.clone()),
            status: Some("dispatched".to_string()),
            amount_sats: Some(record.amount_sats),
            attributes,
            ..AuthorityReceiptContext::default()
        },
    }
}

fn confirmed_payout_receipt(
    record: &TreasuryPayoutRecord,
    payment_id: &str,
) -> TreasuryReceiptEvent {
    let mut attributes = payout_receipt_attributes(record);
    attributes.insert("payment_id".to_string(), payment_id.to_string());
    attributes.insert(
        "provider_payment_id_hash".to_string(),
        treasury_hash(payment_id),
    );
    attributes.insert("terminal_event_state".to_string(), "confirmed".to_string());
    TreasuryReceiptEvent {
        receipt_type: "treasury.payout.confirmed",
        context: AuthorityReceiptContext {
            request_id: Some(record.payout_key.clone()),
            status: Some("confirmed".to_string()),
            amount_sats: Some(record.amount_sats),
            attributes,
            ..AuthorityReceiptContext::default()
        },
    }
}

fn failed_payout_receipt(record: &TreasuryPayoutRecord) -> TreasuryReceiptEvent {
    TreasuryReceiptEvent {
        receipt_type: "treasury.payout.failed",
        context: AuthorityReceiptContext {
            request_id: Some(record.payout_key.clone()),
            status: Some("failed".to_string()),
            reason: record.reason.clone(),
            amount_sats: Some(record.amount_sats),
            attributes: payout_receipt_attributes(record),
            ..AuthorityReceiptContext::default()
        },
    }
}

fn skipped_payout_receipt(record: &TreasuryPayoutRecord) -> TreasuryReceiptEvent {
    TreasuryReceiptEvent {
        receipt_type: "treasury.payout.skipped",
        context: AuthorityReceiptContext {
            request_id: Some(record.payout_key.clone()),
            status: Some("skipped".to_string()),
            reason: record.reason.clone(),
            amount_sats: Some(record.amount_sats),
            attributes: payout_receipt_attributes(record),
            ..AuthorityReceiptContext::default()
        },
    }
}

fn treasury_alert_raised_receipt(alert: &TreasuryContinuityAlert) -> TreasuryReceiptEvent {
    let mut attributes = BTreeMap::new();
    attributes.insert("alert_id".to_string(), alert.alert_id.clone());
    attributes.insert("severity".to_string(), alert.severity.clone());
    TreasuryReceiptEvent {
        receipt_type: "treasury.alert.raised",
        context: AuthorityReceiptContext {
            request_id: Some(format!("treasury-alert-{}", alert.alert_id)),
            status: Some("active".to_string()),
            reason: Some(alert.reason.clone()),
            attributes,
            ..AuthorityReceiptContext::default()
        },
    }
}

fn treasury_alert_cleared_receipt(
    alert: &TreasuryContinuityAlert,
    cleared_at_unix_ms: u64,
) -> TreasuryReceiptEvent {
    let mut attributes = BTreeMap::new();
    attributes.insert("alert_id".to_string(), alert.alert_id.clone());
    attributes.insert("severity".to_string(), alert.severity.clone());
    attributes.insert(
        "cleared_at_unix_ms".to_string(),
        cleared_at_unix_ms.to_string(),
    );
    TreasuryReceiptEvent {
        receipt_type: "treasury.alert.cleared",
        context: AuthorityReceiptContext {
            request_id: Some(format!("treasury-alert-{}", alert.alert_id)),
            status: Some("cleared".to_string()),
            reason: Some(alert.reason.clone()),
            attributes,
            ..AuthorityReceiptContext::default()
        },
    }
}

fn payout_receipt_attributes(record: &TreasuryPayoutRecord) -> BTreeMap<String, String> {
    let mut attributes = BTreeMap::new();
    attributes.insert(
        "nostr_pubkey_hex".to_string(),
        record.nostr_pubkey_hex.clone(),
    );
    attributes.insert(
        "window_started_at_unix_ms".to_string(),
        record.window_started_at_unix_ms.to_string(),
    );
    attributes.insert(
        "window_ends_at_unix_ms".to_string(),
        record.window_ends_at_unix_ms.to_string(),
    );
    attributes.insert(
        "payout_class".to_string(),
        record
            .classification
            .effective_payout_class()
            .label()
            .to_string(),
    );
    if !record.payout_target.is_empty() {
        attributes.insert(
            "payout_target".to_string(),
            truncate_target(record.payout_target.as_str()),
        );
        attributes.insert(
            "payout_target_kind".to_string(),
            payout_target_kind_for_payment_request(record.payout_target.as_str()).to_string(),
        );
        attributes.insert(
            "payout_target_hash".to_string(),
            treasury_hash(record.payout_target.as_str()),
        );
        attributes.insert(
            "payout_rail".to_string(),
            payout_rail_for_payment_request(record.payout_target.as_str()).to_string(),
        );
    }
    attributes.insert(
        "payout_idempotency_key".to_string(),
        payout_dispatch_idempotency_key(record.payout_key.as_str()),
    );
    if let Some(reason) = record.reason.as_deref() {
        attributes.insert("degraded_reason".to_string(), reason.to_string());
    }
    if let Some(payout_basis) = record.classification.payout_basis.as_deref() {
        attributes.insert("payout_basis".to_string(), payout_basis.to_owned());
    }
    if let Some(work_class) = record.classification.work_class.as_deref() {
        attributes.insert("work_class".to_string(), work_class.to_owned());
    }
    if let Some(progress_class) = record.classification.progress_class.as_deref() {
        attributes.insert("progress_class".to_string(), progress_class.to_owned());
    }
    if let Some(accepted_outcome_id) = record.classification.accepted_outcome_id.as_deref() {
        attributes.insert(
            "accepted_outcome_id".to_string(),
            accepted_outcome_id.to_owned(),
        );
    }
    if let Some(training_run_id) = record.classification.training_run_id.as_deref() {
        attributes.insert("training_run_id".to_string(), training_run_id.to_owned());
    }
    if let Some(window_id) = record.classification.window_id.as_deref() {
        attributes.insert("window_id".to_string(), window_id.to_owned());
    }
    if let Some(contribution_id) = record.classification.contribution_id.as_deref() {
        attributes.insert("contribution_id".to_string(), contribution_id.to_owned());
    }
    if let Some(assignment_id) = record.classification.assignment_id.as_deref() {
        attributes.insert("assignment_id".to_string(), assignment_id.to_owned());
    }
    if let Some(share_bps) = record.classification.share_bps {
        attributes.insert("share_bps".to_string(), share_bps.to_string());
    }
    if let Some(weight_basis) = record.classification.weight_basis.as_deref() {
        attributes.insert("weight_basis".to_string(), weight_basis.to_string());
    }
    if let Some(weight_value) = record.classification.weight_value {
        attributes.insert("weight_value".to_string(), weight_value.to_string());
    }
    attributes.insert(
        "weak_device_bearing".to_string(),
        record.classification.weak_device_bearing.to_string(),
    );
    attributes.insert(
        "progress_bearing".to_string(),
        record.classification.progress_bearing.to_string(),
    );
    attributes
}

fn registration_challenge_key(nostr_pubkey_hex: &str, session_id: &str) -> String {
    format!("{nostr_pubkey_hex}:{session_id}")
}

fn payout_window_key(window_started_at_unix_ms: u64, nostr_pubkey_hex: &str) -> String {
    format!("{window_started_at_unix_ms}:{nostr_pubkey_hex}")
}

fn payout_window_started_at(now_unix_ms: u64, interval_ms: u64) -> u64 {
    if interval_ms == 0 {
        return now_unix_ms;
    }
    now_unix_ms.saturating_sub(now_unix_ms % interval_ms)
}

fn payout_window_started_at_for_identity(
    now_unix_ms: u64,
    interval_ms: u64,
    nostr_pubkey_hex: &str,
) -> u64 {
    if interval_ms == 0 {
        return now_unix_ms;
    }
    let phase_offset_ms = payout_phase_offset_ms(nostr_pubkey_hex, interval_ms);
    payout_window_started_at(now_unix_ms.saturating_sub(phase_offset_ms), interval_ms)
        .saturating_add(phase_offset_ms)
}

fn payout_phase_offset_ms(nostr_pubkey_hex: &str, interval_ms: u64) -> u64 {
    if interval_ms <= 1 {
        return 0;
    }
    let mut hash = 0xcbf29ce484222325u64;
    for byte in nostr_pubkey_hex.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash % interval_ms
}

fn wallet_payment_is_confirmed(payment: &PaymentSummary) -> bool {
    matches!(
        payment.status.to_ascii_lowercase().as_str(),
        "completed" | "complete" | "succeeded"
    )
}

fn wallet_payment_is_failed(payment: &PaymentSummary) -> bool {
    payment.status.eq_ignore_ascii_case("failed")
        || payment
            .status_detail
            .as_deref()
            .is_some_and(|detail| detail.to_ascii_lowercase().contains("failed"))
}

fn record_can_recover_orphan_send_payment(record: &TreasuryPayoutRecord) -> bool {
    if record.payment_id.is_some() || record.counted_in_paid_total {
        return false;
    }

    match record.status.as_str() {
        "dispatching" => true,
        "failed" => record
            .reason
            .as_deref()
            .is_some_and(failed_payout_reason_is_retryable),
        _ => false,
    }
}

fn record_matches_orphan_send_payment(
    record: &TreasuryPayoutRecord,
    payment: &PaymentSummary,
    payment_timestamp_ms: u64,
) -> bool {
    if !record_can_recover_orphan_send_payment(record)
        || !payment.direction.eq_ignore_ascii_case("send")
        || payment.amount_sats != record.amount_sats
    {
        return false;
    }

    let earliest_match_at_unix_ms = record
        .created_at_unix_ms
        .saturating_sub(TREASURY_ORPHAN_SEND_PAYMENT_MATCH_EARLY_SLACK_MS);
    let latest_match_at_unix_ms = record
        .updated_at_unix_ms
        .max(record.created_at_unix_ms)
        .saturating_add(TREASURY_ORPHAN_SEND_PAYMENT_MATCH_WINDOW_MS);

    payment_timestamp_ms >= earliest_match_at_unix_ms
        && payment_timestamp_ms <= latest_match_at_unix_ms
}

fn truncate_target(value: &str) -> String {
    if value.len() <= 24 {
        return value.to_string();
    }
    format!(
        "{}...{}",
        &value[..12],
        &value[value.len().saturating_sub(8)..]
    )
}

fn wallet_refresh_payment_page_budget(tracked_payment_count: usize) -> usize {
    if tracked_payment_count == 0 {
        return 1;
    }

    let tracked_pages = (tracked_payment_count
        .saturating_add(TREASURY_WALLET_REFRESH_PAYMENT_PAGE_SIZE - 1))
        / TREASURY_WALLET_REFRESH_PAYMENT_PAGE_SIZE;

    tracked_pages
        .saturating_add(1)
        .clamp(1, TREASURY_WALLET_REFRESH_MAX_PAYMENT_PAGES)
}

fn track_wallet_refresh_payment(
    payments: &mut Vec<PaymentSummary>,
    seen_payment_ids: &mut BTreeSet<String>,
    unresolved_payment_ids: &mut BTreeSet<String>,
    payment: PaymentSummary,
) -> bool {
    unresolved_payment_ids.remove(payment.id.as_str());
    if !seen_payment_ids.insert(payment.id.clone()) {
        return false;
    }
    payments.push(payment);
    true
}

fn wallet_refresh_page_offsets(plan: &TreasuryWalletRefreshPlan) -> Vec<usize> {
    let mut page_offsets = Vec::with_capacity(plan.payment_page_budget());
    page_offsets.extend(0..TREASURY_WALLET_REFRESH_RECENT_PAYMENT_PAGES);

    if plan.tracked_payment_count() == 0 {
        return page_offsets;
    }

    let history_scan_page_offset = plan
        .history_scan_page_offset
        .max(TREASURY_WALLET_REFRESH_RECENT_PAYMENT_PAGES);
    let history_page_budget = plan
        .payment_page_budget()
        .saturating_sub(TREASURY_WALLET_REFRESH_RECENT_PAYMENT_PAGES);
    page_offsets.extend(
        history_scan_page_offset..history_scan_page_offset.saturating_add(history_page_budget),
    );
    page_offsets
}

fn wallet_payment_scan_mode(plan: &TreasuryWalletRefreshPlan) -> &'static str {
    if plan.history_scan_page_offset > 0 || plan.tracked_payment_count() > 0 {
        "recent_plus_backfill"
    } else {
        "recent_only"
    }
}

fn validate_wallet_hydration_balance(
    plan: &TreasuryWalletRefreshPlan,
    balance_sats: u64,
    hydration_mode: &str,
) -> Result<()> {
    if balance_sats > 0 || !plan.expects_funded_balance() {
        return Ok(());
    }

    bail!(
        "wallet_hydration_zero_balance_after_{}:{}:{}",
        hydration_mode,
        plan.historical_receive_total_sats,
        plan.payout_sats_paid_total
    );
}

fn ensure_wallet_mnemonic(path: &Path, create_if_missing: bool) -> Result<String> {
    if path.exists() {
        let mnemonic = fs::read_to_string(path)
            .with_context(|| format!("failed to read treasury mnemonic {}", path.display()))?;
        let trimmed = mnemonic.trim();
        if trimmed.is_empty() {
            bail!("treasury mnemonic is empty at {}", path.display());
        }
        return Ok(trimmed.to_string());
    }
    if !create_if_missing {
        bail!("treasury wallet mnemonic missing at {}", path.display());
    }
    let entropy: [u8; 16] = rand::random();
    let mnemonic = Mnemonic::from_entropy_in(Language::English, &entropy)
        .context("failed to generate treasury mnemonic")?
        .to_string();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| {
            format!(
                "failed to create treasury mnemonic dir {}",
                parent.display()
            )
        })?;
    }
    fs::write(path, format!("{mnemonic}\n"))
        .with_context(|| format!("failed to write treasury mnemonic {}", path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600)).with_context(|| {
            format!(
                "failed to set treasury mnemonic permissions {}",
                path.display()
            )
        })?;
    }
    Ok(mnemonic)
}

fn read_env_nonempty(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn parse_bool_env(name: &str, default: bool) -> Result<bool, String> {
    match std::env::var(name) {
        Ok(value) => match value.trim().to_ascii_lowercase().as_str() {
            "" => Ok(default),
            "1" | "true" | "yes" | "on" => Ok(true),
            "0" | "false" | "no" | "off" => Ok(false),
            other => Err(format!("invalid {name}: expected bool, got '{other}'")),
        },
        Err(_) => Ok(default),
    }
}

fn parse_u64_env(name: &str, default: u64) -> Result<u64, String> {
    match std::env::var(name) {
        Ok(value) => value
            .trim()
            .parse::<u64>()
            .map_err(|error| format!("invalid {name}: {error}")),
        Err(_) => Ok(default),
    }
}

fn parse_optional_u64_env(name: &str, default: Option<u64>) -> Result<Option<u64>, String> {
    match std::env::var(name) {
        Ok(value) => {
            let value = value.trim();
            if value.is_empty() {
                Ok(default)
            } else {
                value
                    .parse::<u64>()
                    .map(Some)
                    .map_err(|error| format!("invalid {name}: {error}"))
            }
        }
        Err(_) => Ok(default),
    }
}

fn parse_placeholder_payout_mode_env(
    name: &str,
    default: TreasuryPlaceholderPayoutMode,
) -> Result<TreasuryPlaceholderPayoutMode, String> {
    match std::env::var(name) {
        Ok(value) => match value.trim().to_ascii_lowercase().as_str() {
            "" => Ok(default),
            "presence_only" => Ok(TreasuryPlaceholderPayoutMode::PresenceOnly),
            "inference_ready" | "readiness" => Ok(TreasuryPlaceholderPayoutMode::InferenceReady),
            "disabled" => Ok(TreasuryPlaceholderPayoutMode::Disabled),
            other => Err(format!(
                "invalid {name}: expected presence_only, inference_ready, or disabled, got '{other}'"
            )),
        },
        Err(_) => Ok(default),
    }
}

fn read_path_env(name: &str, default: &str) -> PathBuf {
    std::env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(default))
}

pub(crate) fn parse_pylon_client_version(raw: &str) -> Result<Version, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("empty version".to_string());
    }
    let lowered = trimmed.to_ascii_lowercase();
    let normalized = if let Some((_, suffix)) = trimmed.rsplit_once('@') {
        suffix
    } else if lowered.starts_with("pylon-v") {
        &trimmed["pylon-v".len()..]
    } else if lowered.starts_with("pylon/") {
        &trimmed["pylon/".len()..]
    } else if lowered.starts_with("pylon-") {
        &trimmed["pylon-".len()..]
    } else if lowered.starts_with('v') {
        &trimmed[1..]
    } else {
        trimmed
    };
    Version::parse(normalized.trim()).map_err(|error| error.to_string())
}

fn treasury_policy_gate_is_more_restrictive(
    before: &TreasuryRuntimePolicy,
    after: &TreasuryRuntimePolicy,
) -> bool {
    let before_active = before.new_accrual_version_gate_active();
    let after_active = after.new_accrual_version_gate_active();
    if !before_active {
        return after_active;
    }
    if !after_active {
        return false;
    }

    if before
        .min_new_accrual_started_at_unix_ms
        .zip(after.min_new_accrual_started_at_unix_ms)
        .is_some_and(|(before_cutoff, after_cutoff)| after_cutoff < before_cutoff)
    {
        return true;
    }

    match (
        before
            .min_new_accrual_pylon_version
            .as_deref()
            .map(parse_pylon_client_version),
        after
            .min_new_accrual_pylon_version
            .as_deref()
            .map(parse_pylon_client_version),
    ) {
        (Some(Ok(before_version)), Some(Ok(after_version))) => after_version > before_version,
        _ => before.min_new_accrual_pylon_version != after.min_new_accrual_pylon_version,
    }
}

fn treasury_policy_placeholder_lane_is_more_restrictive(
    before: &TreasuryRuntimePolicy,
    after: &TreasuryRuntimePolicy,
) -> bool {
    let placeholder_rank = |mode: TreasuryPlaceholderPayoutMode| match mode {
        TreasuryPlaceholderPayoutMode::PresenceOnly => 0u8,
        TreasuryPlaceholderPayoutMode::InferenceReady => 1u8,
        TreasuryPlaceholderPayoutMode::Disabled => 2u8,
    };

    placeholder_rank(after.placeholder_payout_mode)
        > placeholder_rank(before.placeholder_payout_mode)
        || (!before.dedupe_placeholder_hosts && after.dedupe_placeholder_hosts)
}

fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
type TestWalletSnapshotHook =
    std::sync::Arc<dyn Fn() -> Result<TreasuryWalletSnapshot> + Send + Sync>;
#[cfg(test)]
type TestWalletFundingHook = std::sync::Arc<
    dyn Fn(
            TreasuryFundingTargetRequest,
        ) -> Pin<Box<dyn Future<Output = Result<TreasuryFundingMaterial>> + Send>>
        + Send
        + Sync,
>;
#[cfg(test)]
type TestWalletSendHook = std::sync::Arc<dyn Fn(String, u64) -> Result<String> + Send + Sync>;

#[cfg(test)]
fn test_wallet_snapshot_hook() -> &'static Mutex<Option<TestWalletSnapshotHook>> {
    static HOOK: OnceLock<Mutex<Option<TestWalletSnapshotHook>>> = OnceLock::new();
    HOOK.get_or_init(|| Mutex::new(None))
}

#[cfg(test)]
fn test_wallet_funding_hook() -> &'static Mutex<Option<TestWalletFundingHook>> {
    static HOOK: OnceLock<Mutex<Option<TestWalletFundingHook>>> = OnceLock::new();
    HOOK.get_or_init(|| Mutex::new(None))
}

#[cfg(test)]
fn test_wallet_send_hook() -> &'static Mutex<Option<TestWalletSendHook>> {
    static HOOK: OnceLock<Mutex<Option<TestWalletSendHook>>> = OnceLock::new();
    HOOK.get_or_init(|| Mutex::new(None))
}

#[cfg(test)]
pub(crate) fn treasury_test_hook_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

#[cfg(test)]
fn dispatch_with_test_hooks(
    config: &TreasuryConfig,
    plans: &[TreasuryDispatchPlan],
) -> TreasuryDispatchBatchResult {
    let send_hook = test_wallet_send_hook()
        .lock()
        .expect("treasury send hook")
        .clone();
    let snapshot_hook = test_wallet_snapshot_hook()
        .lock()
        .expect("treasury snapshot hook")
        .clone();
    let mut outcomes = Vec::with_capacity(plans.len());
    for payout_class in [
        TreasuryPayoutClass::AcceptedWork,
        TreasuryPayoutClass::PlaceholderLiveness,
        TreasuryPayoutClass::BetaBonus,
    ] {
        let _parallelism = config.max_concurrent_send_operations_for_class(
            plans
                .iter()
                .filter(|plan| plan.classification.payout_class == payout_class)
                .count(),
            payout_class,
        );
        for plan in plans
            .iter()
            .filter(|plan| plan.classification.payout_class == payout_class)
        {
            match send_hook
                .as_ref()
                .ok_or_else(|| anyhow!("missing treasury send hook"))
                .and_then(|hook| hook(plan.payment_request.clone(), plan.amount_sats))
            {
                Ok(payment_id) => outcomes.push(TreasuryDispatchOutcome::Dispatched {
                    payout_key: plan.payout_key.clone(),
                    payment_id,
                    terminal_event_state: Some("submitted".to_string()),
                }),
                Err(error) => outcomes.push(TreasuryDispatchOutcome::Failed {
                    payout_key: plan.payout_key.clone(),
                    reason: classify_wallet_send_failure(error.to_string().as_str()),
                }),
            }
        }
    }
    let (wallet_snapshot, wallet_error) = match snapshot_hook {
        Some(hook) => match hook() {
            Ok(snapshot) => (Some(snapshot), None),
            Err(error) => (None, Some(error.to_string())),
        },
        None => (None, None),
    };
    TreasuryDispatchBatchResult {
        outcomes,
        wallet_snapshot,
        wallet_error,
    }
}

#[cfg(test)]
pub(crate) fn set_test_wallet_snapshot_hook(hook: Option<TestWalletSnapshotHook>) {
    *test_wallet_snapshot_hook()
        .lock()
        .expect("treasury snapshot hook") = hook;
}

#[cfg(test)]
pub(crate) fn set_test_wallet_funding_hook(hook: Option<TestWalletFundingHook>) {
    *test_wallet_funding_hook()
        .lock()
        .expect("treasury funding hook") = hook;
}

#[cfg(test)]
pub(crate) fn set_test_wallet_send_hook(hook: Option<TestWalletSendHook>) {
    *test_wallet_send_hook().lock().expect("treasury send hook") = hook;
}

#[cfg(test)]
mod tests {
    use super::{
        OnlinePylonIdentity, PaymentSummary, TREASURY_FAILED_PAYOUT_RETRY_AFTER_MS,
        TREASURY_IMPOSSIBLE_ZERO_BALANCE_THRESHOLD_SATS,
        TREASURY_WALLET_REFRESH_CURSOR_PAYMENT_PAGES, TREASURY_WALLET_REFRESH_MAX_PAYMENT_PAGES,
        TREASURY_WALLET_REFRESH_PAYMENT_PAGE_SIZE, TREASURY_WALLET_REFRESH_RECENT_PAYMENT_PAGES,
        TreasuryConfig, TreasuryDispatchOutcome, TreasuryFundingMaterial, TreasuryFundingReceive,
        TreasuryFundingTargetPhaseTimings, TreasuryFundingTargetRequest,
        TreasuryLightningProviderConfig, TreasuryLightningProviderKind, TreasuryOperationKind,
        TreasuryOperationStatus, TreasuryPayoutClass, TreasuryPayoutClassification,
        TreasuryPayoutRecord, TreasuryPlaceholderPayoutMode, TreasuryPublicStats,
        TreasuryQueuedPayoutRequest, TreasuryState, TreasuryWalletInspection,
        TreasuryWalletPaymentAggregate, TreasuryWalletRecoveryComparison,
        TreasuryWalletRecoveryReport, TreasuryWalletRefreshPlan, TreasuryWalletRefreshProgress,
        TreasuryWalletSnapshot, apply_treasury_wallet_recovery_cutover,
        build_treasury_wallet_recovery_comparison, create_live_funding_target,
        dispatch_live_payouts, parse_treasury_command, payout_phase_offset_ms, payout_window_key,
        payout_window_started_at, payout_window_started_at_for_identity, run_treasury_command,
        set_test_wallet_funding_hook, set_test_wallet_send_hook, set_test_wallet_snapshot_hook,
        track_wallet_refresh_payment, treasury_test_hook_lock, validate_wallet_hydration_balance,
        wallet_refresh_page_offsets, wallet_refresh_payment_page_budget, write_json_file,
    };
    use crate::treasury_provider::{
        LdkChainBackend, LdkNetwork, LdkServerBalances, LdkServerChannel,
        LdkTreasuryProviderConfig, TreasuryProviderFundingTarget,
    };
    use openagents_provider_substrate::{
        ProviderPaymentTargetRegistration, sign_provider_payment_target_registration,
        verify_provider_payment_target_registration_signature,
    };
    use std::collections::{BTreeMap, BTreeSet};
    use std::fs;
    use std::path::PathBuf;
    use std::sync::Arc;

    fn test_treasury_config() -> TreasuryConfig {
        TreasuryConfig {
            enabled: true,
            payout_sats_per_window: 120,
            payout_interval_seconds: 60,
            require_sellable: false,
            daily_budget_cap_sats: 1_000,
            accepted_work_default_payout_sats: 120,
            accepted_work_daily_budget_cap_sats: 1_000,
            placeholder_payout_mode: TreasuryPlaceholderPayoutMode::InferenceReady,
            dedupe_placeholder_hosts: true,
            availability_max_concurrent_sends: 16,
            min_new_accrual_pylon_version: None,
            min_new_accrual_started_at_unix_ms: None,
            reconciliation_horizon_seconds: 300,
            apply_env_policy: false,
            allow_destructive_env_policy_change: false,
            policy_change_reason: None,
            lightning_provider: TreasuryLightningProviderConfig::new(
                TreasuryLightningProviderKind::Ldk,
                LdkTreasuryProviderConfig {
                    server_url: None,
                    api_key_path: None,
                    tls_cert_path: None,
                    storage_dir: PathBuf::from("/tmp/test-nexus-treasury-ldk"),
                    network: LdkNetwork::Regtest,
                    chain_backend: LdkChainBackend::Bitcoind,
                },
            )
            .expect("ldk provider config"),
            state_path: PathBuf::from("/tmp/test-nexus-treasury-state.json"),
            wallet_mnemonic_path: PathBuf::from("/tmp/test-nexus-treasury.mnemonic"),
            wallet_storage_dir: PathBuf::from("/tmp/test-nexus-treasury-wallet"),
            wallet_network: "regtest".to_string(),
            wallet_api_key_env: None,
            wallet_real_time_sync_enabled: false,
            wallet_status_refresh_seconds: 30,
            funding_target_timeout_ms: 10_000,
            wallet_recovery_inspection_timeout_ms: 120_000,
            wallet_recovery_parallel_inspections: false,
            wallet_recovery_scan_payments: false,
            ldk_min_ready_channel_count: 2,
            ldk_min_ready_outbound_capacity_sats: 20_000,
            simulated_wallet_enabled: false,
            simulated_wallet_balance_sats: 1_000_000,
            max_concurrent_sends: 16,
            registration_challenge_ttl_seconds: 300,
            integration_token: None,
        }
    }

    #[test]
    fn treasury_config_defaults_to_ldk_provider() {
        let config = test_treasury_config();
        assert_eq!(
            config.lightning_provider.provider,
            TreasuryLightningProviderKind::Ldk
        );
        assert_eq!(config.lightning_provider.ldk.network, LdkNetwork::Regtest);
    }

    #[tokio::test]
    async fn default_ldk_provider_creates_local_funding_target_without_retired_provider() {
        let funding = create_live_funding_target(
            &test_treasury_config(),
            TreasuryFundingTargetRequest {
                amount_sats: Some(210),
                description: Some("fund treasury".to_string()),
                expiry_seconds: Some(60),
            },
        )
        .await
        .expect("ldk funding target should build");
        assert!(
            funding
                .provider_target
                .starts_with("ldk://server/regtest/bitcoind/")
        );
        assert_eq!(funding.provider_invoice, None);
        assert!(funding.provider_payment_id.is_some());
        assert!(
            funding
                .bolt11_invoice
                .as_deref()
                .is_some_and(|invoice| { invoice.starts_with("lnbcrt210") })
        );
        assert!(funding.phase_timings.request_received_at_unix_ms > 0);
        assert!(funding.phase_timings.ldk_rpc_started_at_unix_ms.is_some());
        assert!(funding.phase_timings.ldk_rpc_completed_at_unix_ms.is_some());
        assert!(funding.phase_timings.invoice_returned_at_unix_ms.is_some());
        assert!(funding.phase_timings.ldk_rpc_duration_ms().is_some());
        assert_eq!(
            funding.wallet_snapshot.wallet_hydration_mode.as_deref(),
            Some("ldk_provider_scaffold")
        );
    }

    #[test]
    fn ldk_funding_material_uses_live_balances_not_invoice_target_balance() {
        let config = test_treasury_config();
        let material = super::funding_material_from_provider_target(
            &config,
            TreasuryProviderFundingTarget {
                provider_target: "ldk://server/regtest/bitcoind/test".to_string(),
                bitcoin_address: String::new(),
                bolt11_invoice: Some("lnbcrt210test".to_string()),
                provider_invoice: Some("ldk-payment-id".to_string()),
                balance_sats: 0,
            },
            LdkServerBalances {
                total_onchain_sats: 5_000,
                spendable_onchain_sats: 4_000,
                lightning_sats: 1_000,
                usable_sats: 4_750,
            },
            TreasuryFundingTargetPhaseTimings::default(),
        );

        assert_eq!(material.wallet_snapshot.balance_sats, 4_750);
        assert_eq!(material.wallet_snapshot.total_onchain_balance_sats, 5_000);
        assert_eq!(
            material.wallet_snapshot.spendable_onchain_balance_sats,
            4_000
        );
        assert_eq!(material.wallet_snapshot.lightning_balance_sats, 1_000);
        assert_eq!(material.wallet_snapshot.runtime_detail, None);
    }

    #[tokio::test]
    async fn default_ldk_provider_dispatches_with_stable_idempotency_key() {
        let config = test_treasury_config();
        let plans = [super::TreasuryDispatchPlan {
            payout_key: "window-a:pubkey-a".to_string(),
            payment_request: "lnbcrt120receiver".to_string(),
            amount_sats: 120,
            classification: TreasuryPayoutClassification::default(),
        }];
        let first = dispatch_live_payouts(&config, &plans).await;
        let second = dispatch_live_payouts(&config, &plans).await;
        assert_eq!(first.outcomes, second.outcomes);
        assert!(matches!(
            first.outcomes.first(),
            Some(TreasuryDispatchOutcome::Dispatched { payment_id, .. })
                if payment_id.starts_with("ldk-local-payment-")
        ));
    }

    fn test_online_identity(nostr_pubkey_hex: &str) -> OnlinePylonIdentity {
        OnlinePylonIdentity {
            nostr_pubkey_hex: nostr_pubkey_hex.to_string(),
            sellable: true,
            client_version: None,
            inference_ready: true,
            host_fingerprint: None,
            availability_stipend_eligible: true,
            availability_stipend_gate_reason: None,
        }
    }

    fn availability_beneficiary_scope_key(key: &str) -> String {
        format!("availability-beneficiary:{key}")
    }

    fn availability_identity_scope_key(nostr_pubkey_hex: &str) -> String {
        format!("availability-identity:{nostr_pubkey_hex}")
    }

    fn unique_treasury_state_path(label: &str) -> PathBuf {
        PathBuf::from(format!(
            "/tmp/test-nexus-treasury-{label}-{}.json",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ))
    }

    fn unique_temp_dir(label: &str) -> PathBuf {
        PathBuf::from(format!(
            "/tmp/test-nexus-treasury-{label}-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ))
    }

    fn test_payout_record(payout_key: &str, status: &str) -> TreasuryPayoutRecord {
        TreasuryPayoutRecord {
            payout_key: payout_key.to_string(),
            nostr_pubkey_hex: "pubkey-a".to_string(),
            payout_target: "provider:alice-secret-target".to_string(),
            amount_sats: 120,
            status: status.to_string(),
            reason: None,
            payment_id: Some("legacy-payment-secret".to_string()),
            window_started_at_unix_ms: 1_700_000_000_000,
            window_ends_at_unix_ms: 1_700_000_060_000,
            created_at_unix_ms: 1_700_000_000_000,
            updated_at_unix_ms: 1_700_000_001_000,
            sellable_at_window_open: true,
            dispatch_receipt_recorded: true,
            confirm_receipt_recorded: status == "confirmed",
            fail_receipt_recorded: status == "failed",
            skip_receipt_recorded: status == "skipped",
            counted_in_paid_total: status == "confirmed",
            classification: TreasuryPayoutClassification::default(),
        }
    }

    #[test]
    fn state_load_retires_non_ldk_dispatched_records() {
        let path = unique_treasury_state_path("retire-non-ldk-dispatched");
        let mut state = TreasuryState::default();
        state.state_path = Some(path.clone());
        let now = super::now_unix_ms();
        let stale_key = "accepted_work:stale-non-ldk:pubkey-a";
        let ldk_key = "accepted_work:ldk-target:pubkey-a";
        let mut stale = test_payout_record(stale_key, "dispatched");
        stale.payout_target = String::new();
        stale.counted_in_paid_total = true;
        stale.window_started_at_unix_ms = now.saturating_sub(60_000);
        stale.window_ends_at_unix_ms = now.saturating_add(60_000);
        stale.created_at_unix_ms = now.saturating_sub(30_000);
        stale.updated_at_unix_ms = now.saturating_sub(10_000);
        let mut ldk = test_payout_record(ldk_key, "dispatched");
        ldk.payout_target = "lno1pylonalice".to_string();
        ldk.window_started_at_unix_ms = now.saturating_sub(60_000);
        ldk.window_ends_at_unix_ms = now.saturating_add(60_000);
        ldk.created_at_unix_ms = now.saturating_sub(30_000);
        ldk.updated_at_unix_ms = now.saturating_sub(10_000);
        state
            .payout_records_by_key
            .insert(stale_key.to_string(), stale);
        state.payout_records_by_key.insert(ldk_key.to_string(), ldk);
        state.persist();

        let loaded = TreasuryState::new(path.clone());
        let retired = loaded
            .payout_records_by_key
            .get(stale_key)
            .expect("retired stale payout");
        assert_eq!(retired.status, "failed");
        assert_eq!(
            retired.reason.as_deref(),
            Some("retired_unpayable_non_ldk_payout_record")
        );
        assert!(!retired.counted_in_paid_total);
        assert!(retired.fail_receipt_recorded);
        assert_eq!(
            loaded
                .payout_records_by_key
                .get(ldk_key)
                .expect("ldk payout remains pending")
                .status,
            "dispatched"
        );

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn failed_non_ldk_records_are_historical_cleanup_not_current_ldk_attention() {
        let mut state = TreasuryState::default();
        let payout_key = "accepted_work:legacy-unsupported-target:pubkey-a";
        let mut record = test_payout_record(payout_key, "failed");
        record.payout_target = "provider:old-target".to_string();
        record.payment_id = None;
        record.reason = Some("insufficient_funds".to_string());
        record.classification = TreasuryPayoutClassification {
            payout_class: TreasuryPayoutClass::AcceptedWork,
            payout_basis: Some("validator_verdict".to_string()),
            ..TreasuryPayoutClassification::default()
        };
        state
            .payout_records_by_key
            .insert(payout_key.to_string(), record);

        let before = state.training_payout_ledger_summary();
        assert_eq!(before.accepted_work_pending_payout_count, 0);
        assert_eq!(before.current_ldk_attention_payout_count, 0);
        assert_eq!(before.accepted_work_attention_payout_count, 1);

        let report = state.payout_ledger_cleanup_report(true, 1_800_000);

        assert!(report.changed);
        assert_eq!(report.records_retired.len(), 1);
        assert_eq!(report.after_summary.reconciliation_status.as_str(), "clean");
        assert_eq!(report.after_summary.accepted_work_pending_payout_count, 0);
        assert_eq!(report.after_summary.accepted_work_attention_payout_count, 0);
        assert_eq!(report.after_summary.retired_historical_payout_count, 1);
        assert_eq!(
            report
                .after_summary
                .retired_historical_accepted_work_payout_count,
            1
        );

        let retired = state
            .payout_records_by_key
            .get(payout_key)
            .expect("retired payout");
        assert_eq!(
            retired.reason.as_deref(),
            Some("retired_unpayable_non_ldk_payout_record")
        );
        assert_eq!(
            super::treasury_payout_reconciliation_status(retired),
            "retired_historical"
        );
    }

    #[tokio::test]
    async fn payout_ledger_cleanup_command_writes_before_after_report() {
        let path = unique_treasury_state_path("payout-ledger-cleanup-state");
        let report_path = unique_treasury_state_path("payout-ledger-cleanup-report");
        let mut config = test_treasury_config();
        config.state_path = path.clone();

        let payout_key = "accepted_work:cleanup-command:pubkey-a";
        let now = super::now_unix_ms();
        let mut state = TreasuryState::default();
        state.state_path = Some(path.clone());
        let mut record = test_payout_record(payout_key, "failed");
        record.payout_target = "provider:cleanup-command".to_string();
        record.payment_id = None;
        record.reason = Some("insufficient_funds".to_string());
        record.window_started_at_unix_ms = now.saturating_sub(60_000);
        record.window_ends_at_unix_ms = now.saturating_add(60_000);
        record.created_at_unix_ms = now.saturating_sub(30_000);
        record.updated_at_unix_ms = now.saturating_sub(10_000);
        record.classification = TreasuryPayoutClassification {
            payout_class: TreasuryPayoutClass::AcceptedWork,
            payout_basis: Some("validator_verdict".to_string()),
            ..TreasuryPayoutClassification::default()
        };
        state
            .payout_records_by_key
            .insert(payout_key.to_string(), record);
        state.persist();

        let command = parse_treasury_command(&[
            "nexus-control".to_string(),
            "treasury".to_string(),
            "payout-ledger-cleanup".to_string(),
            "--apply".to_string(),
            "--report-path".to_string(),
            report_path.display().to_string(),
            "--json".to_string(),
        ])
        .expect("parse cleanup");
        let output = run_treasury_command(&config, &command)
            .await
            .expect("cleanup command");
        let report: super::TreasuryPayoutLedgerCleanupReport =
            serde_json::from_str(output.as_str()).expect("json report");

        assert!(report.applied);
        assert!(report.changed);
        assert_eq!(report.records_retired.len(), 1);
        assert_eq!(
            report.before_summary.accepted_work_attention_payout_count,
            1
        );
        assert_eq!(report.after_summary.accepted_work_attention_payout_count, 0);
        assert_eq!(report.after_summary.retired_historical_payout_count, 1);
        assert!(report_path.exists());

        let loaded = TreasuryState::new(path.clone());
        let retired = loaded
            .payout_records_by_key
            .get(payout_key)
            .expect("retired payout");
        assert_eq!(
            retired.reason.as_deref(),
            Some("retired_unpayable_non_ldk_payout_record")
        );

        let _ = std::fs::remove_file(path);
        let _ = std::fs::remove_file(report_path);
    }

    #[test]
    fn ldk_dispatch_receipt_records_target_rail_idempotency_and_terminal_state() {
        let mut state = TreasuryState::default();
        let payout_key = "accepted_work:closeout-001:contrib-001:pubkey-a";
        let mut record = test_payout_record(payout_key, "dispatching");
        record.payout_target = "lno1pylonalice".to_string();
        record.payment_id = None;
        record.dispatch_receipt_recorded = false;
        state
            .payout_records_by_key
            .insert(payout_key.to_string(), record);

        let receipts = state.apply_dispatch_outcome(
            TreasuryDispatchOutcome::Dispatched {
                payout_key: payout_key.to_string(),
                payment_id: "ldk-payment-001".to_string(),
                terminal_event_state: Some("completed".to_string()),
            },
            123,
        );

        assert_eq!(receipts.len(), 1);
        let attributes = &receipts[0].context.attributes;
        assert_eq!(
            attributes.get("payout_target_kind").map(String::as_str),
            Some("bolt12_offer")
        );
        assert_eq!(
            attributes.get("payout_rail").map(String::as_str),
            Some("ldk")
        );
        assert_eq!(
            attributes.get("payout_idempotency_key").map(String::as_str),
            Some("payout:accepted_work:closeout-001:contrib-001:pubkey-a")
        );
        assert_eq!(
            attributes.get("terminal_event_state").map(String::as_str),
            Some("completed")
        );
        assert!(
            attributes
                .get("payout_target_hash")
                .is_some_and(|hash| hash.starts_with("sha256:"))
        );
        assert!(
            attributes
                .get("provider_payment_id_hash")
                .is_some_and(|hash| hash.starts_with("sha256:"))
        );
    }

    #[test]
    fn legacy_provider_payout_records_migrate_to_operation_rows() {
        let path = unique_treasury_state_path("legacy-provider-operations");
        let payout_key = "window.legacy:pubkey-a";
        let mut seed = TreasuryState::default();
        seed.payout_records_by_key.insert(
            payout_key.to_string(),
            test_payout_record(payout_key, "confirmed"),
        );
        std::fs::write(
            path.as_path(),
            serde_json::to_string_pretty(&seed).expect("serialize seed"),
        )
        .expect("write treasury state");

        let state = TreasuryState::new(path.clone());
        let operation_id =
            super::treasury_operation_id(TreasuryOperationKind::OutboundPayoutDispatch, payout_key);
        let operation = state
            .treasury_operations_by_id
            .get(operation_id.as_str())
            .expect("migrated legacy provider operation");

        assert_eq!(operation.rail, "retired_payout_record");
        assert_eq!(operation.status, TreasuryOperationStatus::Completed);
        assert_eq!(
            operation
                .rail_metadata
                .get("migrated_from")
                .map(String::as_str),
            Some("legacy_payout_record")
        );
        assert!(
            operation
                .target_hash
                .as_deref()
                .is_some_and(|hash| hash.starts_with("sha256:"))
        );
        assert!(
            operation
                .provider_payment_id
                .as_deref()
                .is_some_and(|hash| hash.starts_with("sha256:"))
        );
        let serialized = serde_json::to_string(operation).expect("serialize operation");
        assert!(!serialized.contains("provider:alice-secret-target"));
        assert!(!serialized.contains("legacy-payment-secret"));

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn funding_invoice_operation_is_idempotent_and_secret_free() {
        let config = test_treasury_config();
        let mut state = TreasuryState::default();
        let request = TreasuryFundingTargetRequest {
            amount_sats: Some(21),
            description: Some("test funding".to_string()),
            expiry_seconds: Some(300),
        };
        let material = TreasuryFundingMaterial {
            provider_target: "ldk:provider-target-secret".to_string(),
            bitcoin_address: "bcrt1qfundingtarget".to_string(),
            provider_invoice: Some("provider-invoice-secret".to_string()),
            bolt11_invoice: Some("lnbc21secretinvoice".to_string()),
            provider_payment_id: Some("ldk-payment-id-secret".to_string()),
            phase_timings: TreasuryFundingTargetPhaseTimings {
                request_received_at_unix_ms: 90,
                operation_row_created_at_unix_ms: Some(100),
                ldk_rpc_started_at_unix_ms: Some(91),
                ldk_rpc_completed_at_unix_ms: Some(99),
                invoice_returned_at_unix_ms: Some(100),
            },
            wallet_snapshot: TreasuryWalletSnapshot {
                runtime_status: "connected".to_string(),
                runtime_detail: None,
                wallet_hydration_mode: Some("ldk_provider_scaffold".to_string()),
                wallet_payment_scan_mode: Some("ldk_provider_boundary".to_string()),
                balance_sats: 100,
                total_onchain_balance_sats: 100,
                spendable_onchain_balance_sats: 100,
                lightning_balance_sats: 0,
                payments: Vec::new(),
            },
        };

        let first_events =
            state.record_funding_invoice_created_operation(&config, &request, &material, 100);
        let second_events =
            state.record_funding_invoice_created_operation(&config, &request, &material, 200);
        let operation_id = super::treasury_operation_id(
            TreasuryOperationKind::FundingInvoiceCreation,
            super::funding_idempotency_key(&request).as_str(),
        );
        let operation = state
            .treasury_operations_by_id
            .get(operation_id.as_str())
            .expect("funding operation");

        assert_eq!(state.treasury_operations_by_id.len(), 1);
        assert_eq!(operation.status, TreasuryOperationStatus::Completed);
        assert_eq!(operation.amount_msat, Some(21_000));
        assert_eq!(first_events.len(), 1);
        assert_eq!(second_events.len(), 1);
        let serialized_operation = serde_json::to_string(operation).expect("operation json");
        let serialized_event = serde_json::to_string(&first_events[0].context).expect("event json");
        for secret in [
            "ldk:provider-target-secret",
            "provider-invoice-secret",
            "lnbc21secretinvoice",
            "ldk-payment-id-secret",
        ] {
            assert!(!serialized_operation.contains(secret));
            assert!(!serialized_event.contains(secret));
        }
        assert_eq!(
            operation
                .rail_metadata
                .get("phase_ldk_rpc_duration_ms")
                .map(String::as_str),
            Some("8")
        );
        assert_eq!(
            operation
                .rail_metadata
                .get("phase_total_duration_ms")
                .map(String::as_str),
            Some("10")
        );
    }

    #[test]
    fn ldk_readiness_reports_channel_capacity_and_payment_failures() {
        let mut config = test_treasury_config();
        config.lightning_provider.ldk.server_url = Some("https://ldk.internal:3536".to_string());
        let now_unix_ms = 2_000_000u64;
        let mut state = TreasuryState::default();
        state.wallet_balance_sats = 50_000;
        state.wallet_balance_updated_at_unix_ms = Some(now_unix_ms);
        state.last_wallet_sync_at_unix_ms = Some(now_unix_ms);

        let needs_payout_targets = state.status_response(&config, now_unix_ms);
        assert_eq!(
            needs_payout_targets.ldk_readiness.state,
            "needs_payout_targets"
        );
        assert_eq!(
            needs_payout_targets
                .ldk_readiness
                .registered_payout_target_count,
            0
        );
        assert_eq!(
            needs_payout_targets.ldk_readiness.projected_channel_count,
            0
        );

        state.payout_targets_by_identity.insert(
            "pubkey-ldk".to_string(),
            super::RegisteredPayoutTarget {
                nostr_pubkey_hex: "pubkey-ldk".to_string(),
                source_session_id: "session-ldk".to_string(),
                payment_target_kind: "bolt12_offer".to_string(),
                payment_target: "lno1validtesttarget".to_string(),
                payment_target_capabilities: vec!["ldk_payment_target_v0_2".to_string()],
                pylon_payment_target_version: Some("pylon-payment-target/v0.2".to_string()),
                provider_target: String::new(),
                bitcoin_address: None,
                registered_at_unix_ms: now_unix_ms,
                last_verified_at_unix_ms: now_unix_ms,
            },
        );

        let needs_channels = state.status_response(&config, now_unix_ms);
        assert_eq!(needs_channels.ldk_readiness.state, "needs_channels");
        assert_eq!(needs_channels.ldk_readiness.projected_channel_count, 0);
        assert_eq!(
            needs_channels.ldk_readiness.projected_inbound_capacity_sats,
            0
        );

        let mut channel_metadata = BTreeMap::new();
        channel_metadata.insert("command".to_string(), "treasury.openChannel".to_string());
        let mut open_operation = super::TreasuryOperationRecord {
            operation_id: "op-open-channel".to_string(),
            kind: TreasuryOperationKind::LightningAdminCommand,
            request_id: Some("admin:treasury.openChannel:readiness".to_string()),
            rail: "ldk".to_string(),
            rail_metadata: channel_metadata,
            amount_msat: Some(21_000_000),
            target_kind: "channel_peer".to_string(),
            target_hash: Some(super::treasury_hash("02peer")),
            beneficiary: None,
            status: TreasuryOperationStatus::Pending,
            provider_payment_id: None,
            receipt_refs: Vec::new(),
            degraded_reason: None,
            created_at_unix_ms: now_unix_ms,
            updated_at_unix_ms: now_unix_ms,
            terminal_event_state: Some("channel_open_requested".to_string()),
        };
        state
            .treasury_operations_by_id
            .insert("op-open-channel".to_string(), open_operation.clone());

        let still_needs_channels = state.status_response(&config, now_unix_ms);
        assert_eq!(still_needs_channels.ldk_readiness.state, "needs_channels");
        assert_eq!(
            still_needs_channels.ldk_readiness.projected_channel_count,
            0
        );

        open_operation.status = TreasuryOperationStatus::Completed;
        open_operation.terminal_event_state = Some("channel_opened".to_string());
        state
            .treasury_operations_by_id
            .insert("op-open-channel".to_string(), open_operation);

        let one_channel = state.status_response(&config, now_unix_ms);
        assert_eq!(one_channel.ldk_readiness.state, "needs_channels");
        assert_eq!(one_channel.ldk_readiness.projected_channel_count, 1);
        assert_eq!(
            one_channel.ldk_readiness.projected_outbound_capacity_sats,
            21_000
        );

        let mut second_metadata = BTreeMap::new();
        second_metadata.insert("command".to_string(), "treasury.openChannel".to_string());
        state.treasury_operations_by_id.insert(
            "op-open-channel-2".to_string(),
            super::TreasuryOperationRecord {
                operation_id: "op-open-channel-2".to_string(),
                kind: TreasuryOperationKind::LightningAdminCommand,
                request_id: Some("admin:treasury.openChannel:readiness-2".to_string()),
                rail: "ldk".to_string(),
                rail_metadata: second_metadata,
                amount_msat: Some(21_000_000),
                target_kind: "channel_peer".to_string(),
                target_hash: Some(super::treasury_hash("02peer2")),
                beneficiary: None,
                status: TreasuryOperationStatus::Completed,
                provider_payment_id: None,
                receipt_refs: Vec::new(),
                degraded_reason: None,
                created_at_unix_ms: now_unix_ms,
                updated_at_unix_ms: now_unix_ms,
                terminal_event_state: Some("channel_opened".to_string()),
            },
        );

        let ready = state.status_response(&config, now_unix_ms);
        assert_eq!(ready.ldk_readiness.state, "ready");
        assert_eq!(ready.ldk_readiness.projected_channel_count, 2);
        assert_eq!(ready.ldk_readiness.projected_inbound_capacity_sats, 42_000);
        assert_eq!(ready.ldk_readiness.projected_outbound_capacity_sats, 42_000);
        assert_eq!(ready.ldk_readiness.min_ready_channel_count, 2);
        assert_eq!(ready.ldk_readiness.min_ready_outbound_capacity_sats, 20_000);

        let mut pay_metadata = BTreeMap::new();
        pay_metadata.insert("command".to_string(), "treasury.payInvoice".to_string());
        state.treasury_operations_by_id.insert(
            "op-pay-failed".to_string(),
            super::TreasuryOperationRecord {
                operation_id: "op-pay-failed".to_string(),
                kind: TreasuryOperationKind::LightningAdminCommand,
                request_id: Some("admin:treasury.payInvoice:readiness".to_string()),
                rail: "ldk".to_string(),
                rail_metadata: pay_metadata,
                amount_msat: Some(5_000),
                target_kind: "bolt11_invoice".to_string(),
                target_hash: Some(super::treasury_hash("lnbcfailed")),
                beneficiary: None,
                status: TreasuryOperationStatus::Failed,
                provider_payment_id: None,
                receipt_refs: Vec::new(),
                degraded_reason: Some(
                    "no_route: insufficient channel balance for payment".to_string(),
                ),
                created_at_unix_ms: now_unix_ms,
                updated_at_unix_ms: now_unix_ms,
                terminal_event_state: Some("payment_failed".to_string()),
            },
        );

        let degraded = state.status_response(&config, now_unix_ms);
        assert_eq!(degraded.ldk_readiness.state, "degraded");
        assert_eq!(degraded.ldk_readiness.recent_failed_payment_count_24h, 1);
        assert_eq!(degraded.ldk_readiness.recent_no_route_count_24h, 1);
        assert_eq!(
            degraded.ldk_readiness.recent_insufficient_balance_count_24h,
            1
        );
    }

    #[test]
    fn ldk_readiness_uses_live_provider_channels_before_operation_history() {
        let mut config = test_treasury_config();
        config.lightning_provider.ldk.server_url = Some("https://ldk.internal:3536".to_string());
        let now_unix_ms = 2_000_000u64;
        let mut state = TreasuryState::default();
        state.wallet_balance_sats = 50_000;
        state.wallet_balance_updated_at_unix_ms = Some(now_unix_ms);
        state.last_wallet_sync_at_unix_ms = Some(now_unix_ms);
        state.payout_targets_by_identity.insert(
            "pubkey-ldk".to_string(),
            super::RegisteredPayoutTarget {
                nostr_pubkey_hex: "pubkey-ldk".to_string(),
                source_session_id: "session-ldk".to_string(),
                payment_target_kind: "bolt12_offer".to_string(),
                payment_target: "lno1validtesttarget".to_string(),
                payment_target_capabilities: vec!["ldk_payment_target_v0_2".to_string()],
                pylon_payment_target_version: Some("pylon-payment-target/v0.2".to_string()),
                provider_target: String::new(),
                bitcoin_address: None,
                registered_at_unix_ms: now_unix_ms,
                last_verified_at_unix_ms: now_unix_ms,
            },
        );
        state.treasury_operations_by_id.insert(
            "op-open-channel-failed".to_string(),
            super::TreasuryOperationRecord {
                operation_id: "op-open-channel-failed".to_string(),
                kind: TreasuryOperationKind::LightningAdminCommand,
                request_id: Some("admin:treasury.openChannel:old".to_string()),
                rail: "ldk".to_string(),
                rail_metadata: BTreeMap::from([
                    ("command".to_string(), "treasury.openChannel".to_string()),
                    (
                        "channel_id_hash".to_string(),
                        super::treasury_hash("old-channel-id"),
                    ),
                ]),
                amount_msat: Some(99_000_000),
                target_kind: "channel_peer".to_string(),
                target_hash: Some(super::treasury_hash("02oldpeer")),
                beneficiary: None,
                status: TreasuryOperationStatus::Failed,
                provider_payment_id: None,
                receipt_refs: Vec::new(),
                degraded_reason: Some("ldk_channel_not_found_after_open_request".to_string()),
                created_at_unix_ms: now_unix_ms,
                updated_at_unix_ms: now_unix_ms,
                terminal_event_state: Some("channel_open_failed".to_string()),
            },
        );

        assert!(state.reconcile_ldk_provider_channels(
            &[LdkServerChannel {
                channel_id: "live-channel-id".to_string(),
                peer_node_id: "02pylonpeer".to_string(),
                status: "usable".to_string(),
                outbound_capacity_sats: 2_000,
                inbound_capacity_sats: 0,
            }],
            now_unix_ms
        ));

        let status = state.status_response(&config, now_unix_ms);
        assert_eq!(status.ldk_readiness.state, "needs_channels");
        assert_eq!(status.ldk_readiness.projected_channel_count, 1);
        assert_eq!(status.ldk_readiness.projected_inbound_capacity_sats, 2_000);
        assert_eq!(status.ldk_readiness.projected_outbound_capacity_sats, 2_000);
        assert!(
            !status
                .degraded_states
                .iter()
                .any(|state| state.code == "low_inbound_liquidity")
        );
    }

    #[test]
    fn ldk_channel_reconciliation_fails_disappeared_pending_open() {
        let now_unix_ms = 2_000_000u64;
        let mut state = TreasuryState::default();
        let channel_id = "ldk-channel-rejected";
        state.treasury_operations_by_id.insert(
            "op-open-channel".to_string(),
            super::TreasuryOperationRecord {
                operation_id: "op-open-channel".to_string(),
                kind: TreasuryOperationKind::LightningAdminCommand,
                request_id: Some("admin:treasury.openChannel:rejected".to_string()),
                rail: "ldk".to_string(),
                rail_metadata: BTreeMap::from([
                    ("command".to_string(), "treasury.openChannel".to_string()),
                    (
                        "channel_id_hash".to_string(),
                        super::treasury_hash(channel_id),
                    ),
                ]),
                amount_msat: Some(3_000_000),
                target_kind: "channel_peer".to_string(),
                target_hash: Some(super::treasury_hash("02peer")),
                beneficiary: None,
                status: TreasuryOperationStatus::Pending,
                provider_payment_id: None,
                receipt_refs: Vec::new(),
                degraded_reason: None,
                created_at_unix_ms: now_unix_ms,
                updated_at_unix_ms: now_unix_ms,
                terminal_event_state: Some("channel_open_requested".to_string()),
            },
        );

        assert!(state.reconcile_ldk_channel_operations(
            &[],
            now_unix_ms + super::TREASURY_LDK_CHANNEL_OPEN_RECONCILE_GRACE_MS + 1
        ));

        let operation = state
            .treasury_operations_by_id
            .get("op-open-channel")
            .expect("operation should remain");
        assert_eq!(operation.status, TreasuryOperationStatus::Failed);
        assert_eq!(
            operation.degraded_reason.as_deref(),
            Some("ldk_channel_not_found_after_open_request")
        );
        let readiness = state.ldk_channel_readiness();
        assert_eq!(readiness.projected_channel_count, 0);
        assert_eq!(readiness.projected_inbound_capacity_sats, 0);
    }

    #[test]
    fn payout_operation_projection_replays_same_read_model() {
        let config = test_treasury_config();
        let mut state = TreasuryState::default();
        let payout_key = "window.replay:pubkey-a";
        let record = test_payout_record(payout_key, "dispatching");
        let operation = super::payout_dispatch_operation_from_record(&config, &record, 100);

        assert!(state.upsert_treasury_operation(operation.clone()));
        assert!(!state.upsert_treasury_operation(operation));
        assert!(state.update_payout_operation_status(
            payout_key,
            TreasuryOperationStatus::Completed,
            Some(super::treasury_hash("payment-complete")),
            None,
            Some("completed".to_string()),
            200,
        ));

        let serialized = serde_json::to_string(&state).expect("serialize state");
        let replayed: TreasuryState = serde_json::from_str(serialized.as_str()).expect("replay");
        assert_eq!(
            replayed.treasury_operations_by_id,
            state.treasury_operations_by_id
        );
    }

    #[test]
    fn receipt_projection_attaches_receipt_refs_and_event_operation() {
        let config = test_treasury_config();
        let mut state = TreasuryState::default();
        let payout_key = "window.receipt:pubkey-a";
        let record = test_payout_record(payout_key, "dispatching");
        let operation = super::payout_dispatch_operation_from_record(&config, &record, 100);
        state.upsert_treasury_operation(operation);

        assert!(state.attach_receipt_reference_for_request(payout_key, "receipt-1", 110));
        assert!(!state.attach_receipt_reference_for_request(payout_key, "receipt-1", 120));
        assert!(state.record_event_projection_operation(
            "treasury.payout.dispatched",
            "receipt-1",
            Some(payout_key),
            130,
        ));

        let payout_operation_id =
            super::treasury_operation_id(TreasuryOperationKind::OutboundPayoutDispatch, payout_key);
        let payout_operation = state
            .treasury_operations_by_id
            .get(payout_operation_id.as_str())
            .expect("payout operation");
        assert_eq!(payout_operation.receipt_refs, vec!["receipt-1".to_string()]);

        let event_operation = state
            .treasury_operations_by_id
            .values()
            .find(|operation| operation.kind == TreasuryOperationKind::EventProjection)
            .expect("event projection operation");
        assert_eq!(event_operation.receipt_refs, vec!["receipt-1".to_string()]);
        assert_eq!(event_operation.status, TreasuryOperationStatus::Completed);
    }

    #[test]
    fn payout_target_signature_round_trip_is_valid() {
        let private_key_hex = "1111111111111111111111111111111111111111111111111111111111111111";
        let nostr_pubkey_hex = "4f355bdcb7cc0af728ef3cceb9615d90684bb5b2ca5f859ab0f0b704075871aa";
        let ldk_target = ProviderPaymentTargetRegistration {
            target_kind: "bolt12_offer",
            target_value: "lno1pylonalice",
            capabilities: &[
                "ldk_payment_target_v0_2",
                "bolt12_offer",
                "bolt11_invoice_request",
                "durable_payout_target",
            ],
            version: "pylon-payment-target/v0.2",
        };
        let ldk_signature = sign_provider_payment_target_registration(
            private_key_hex,
            nostr_pubkey_hex,
            "session-a",
            "challenge-a",
            ldk_target.clone(),
        )
        .expect("LDK payment target signature should build");
        verify_provider_payment_target_registration_signature(
            nostr_pubkey_hex,
            "session-a",
            "challenge-a",
            ldk_target,
            ldk_signature.as_str(),
        )
        .expect("LDK payment target signature should verify");
    }

    #[test]
    fn retired_provider_targets_are_not_ldk_compatible() {
        let retired_provider = super::RegisteredPayoutTarget {
            nostr_pubkey_hex: "pubkey-a".to_string(),
            source_session_id: "session-a".to_string(),
            payment_target_kind: "provider_target".to_string(),
            payment_target: "provider:alice".to_string(),
            payment_target_capabilities: Vec::new(),
            pylon_payment_target_version: None,
            provider_target: "provider:alice".to_string(),
            bitcoin_address: None,
            registered_at_unix_ms: 1,
            last_verified_at_unix_ms: 1,
        };
        let ldk_target = super::RegisteredPayoutTarget {
            nostr_pubkey_hex: "pubkey-b".to_string(),
            source_session_id: "session-b".to_string(),
            payment_target_kind: "bolt12_offer".to_string(),
            payment_target: "lno1pylonbob".to_string(),
            payment_target_capabilities: vec!["ldk_payment_target_v0_2".to_string()],
            pylon_payment_target_version: Some("pylon-payment-target/v0.2".to_string()),
            provider_target: String::new(),
            bitcoin_address: None,
            registered_at_unix_ms: 1,
            last_verified_at_unix_ms: 1,
        };

        assert!(!retired_provider.is_ldk_compatible());
        assert!(ldk_target.is_ldk_compatible());
    }

    #[test]
    fn payout_target_challenge_issue_does_not_rewrite_treasury_state() {
        let path = unique_treasury_state_path("challenge-noop");
        std::fs::write(path.as_path(), "sentinel\n").expect("write sentinel");

        let mut config = test_treasury_config();
        config.state_path = path.clone();
        let mut state = TreasuryState::default();
        state.next_challenge_nonce = 1;
        state.state_path = Some(path.clone());

        let challenge = state.issue_registration_challenge(&config, "pubkey-a", "session-a", 1_000);

        assert!(challenge.challenge.contains("pubkey-a:session-a:1000"));
        assert_eq!(
            std::fs::read_to_string(path.as_path()).expect("read sentinel"),
            "sentinel\n"
        );

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn idempotent_payout_target_registration_does_not_rewrite_treasury_state() {
        let path = unique_treasury_state_path("register-noop");
        std::fs::write(path.as_path(), "sentinel\n").expect("write sentinel");

        let private_key_hex = "1111111111111111111111111111111111111111111111111111111111111111";
        let nostr_pubkey_hex = "4f355bdcb7cc0af728ef3cceb9615d90684bb5b2ca5f859ab0f0b704075871aa";
        let mut config = test_treasury_config();
        config.state_path = path.clone();
        let mut state = TreasuryState::default();
        state.next_challenge_nonce = 1;
        state.state_path = Some(path.clone());
        state.payout_targets_by_identity.insert(
            nostr_pubkey_hex.to_string(),
            super::RegisteredPayoutTarget {
                nostr_pubkey_hex: nostr_pubkey_hex.to_string(),
                source_session_id: "session-old".to_string(),
                payment_target_kind: "bolt12_offer".to_string(),
                payment_target: "lno1pylonalice".to_string(),
                payment_target_capabilities: vec![
                    "bolt11_invoice_request".to_string(),
                    "bolt12_offer".to_string(),
                    "durable_payout_target".to_string(),
                    "ldk_payment_target_v0_2".to_string(),
                ],
                pylon_payment_target_version: Some("pylon-payment-target/v0.2".to_string()),
                provider_target: String::new(),
                bitcoin_address: None,
                registered_at_unix_ms: 500,
                last_verified_at_unix_ms: 500,
            },
        );

        let challenge =
            state.issue_registration_challenge(&config, nostr_pubkey_hex, "session-a", 1_000);
        let capabilities = vec![
            "bolt11_invoice_request".to_string(),
            "bolt12_offer".to_string(),
            "durable_payout_target".to_string(),
            "ldk_payment_target_v0_2".to_string(),
        ];
        let capability_refs = capabilities.iter().map(String::as_str).collect::<Vec<_>>();
        let signature = sign_provider_payment_target_registration(
            private_key_hex,
            nostr_pubkey_hex,
            "session-a",
            challenge.challenge.as_str(),
            ProviderPaymentTargetRegistration {
                target_kind: "bolt12_offer",
                target_value: "lno1pylonalice",
                capabilities: capability_refs.as_slice(),
                version: "pylon-payment-target/v0.2",
            },
        )
        .expect("signature should build");
        let (response, receipt_events) = state
            .register_payout_target(
                &super::ProviderPayoutTargetRegistrationRequest {
                    nostr_pubkey_hex: nostr_pubkey_hex.to_string(),
                    session_id: "session-a".to_string(),
                    payment_target_kind: Some("bolt12_offer".to_string()),
                    payment_target: Some("lno1pylonalice".to_string()),
                    payment_target_capabilities: capabilities,
                    pylon_payment_target_version: Some("pylon-payment-target/v0.2".to_string()),
                    challenge: challenge.challenge,
                    challenge_signature_hex: signature,
                },
                2_000,
            )
            .expect("idempotent registration");

        assert!(receipt_events.is_empty());
        assert_eq!(response.registered_at_unix_ms, 500);
        let target = state
            .payout_targets_by_identity
            .get(nostr_pubkey_hex)
            .expect("payout target");
        assert_eq!(target.source_session_id, "session-a");
        assert_eq!(target.last_verified_at_unix_ms, 2_000);
        assert_eq!(
            std::fs::read_to_string(path.as_path()).expect("read sentinel"),
            "sentinel\n"
        );

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn parse_treasury_command_supports_recovery_commands() {
        let recovery_report = parse_treasury_command(&[
            "nexus-control".to_string(),
            "treasury".to_string(),
            "recovery-report".to_string(),
            "--work-dir".to_string(),
            "/tmp/recovery-work".to_string(),
            "--report-path".to_string(),
            "/tmp/recovery-work/report.json".to_string(),
            "--json".to_string(),
        ])
        .expect("recovery-report should parse");
        assert!(matches!(
            recovery_report,
            super::TreasuryCommand::RecoveryReport {
                work_dir: Some(_),
                report_path: Some(_),
                json: true,
            }
        ));

        let recovery_cutover = parse_treasury_command(&[
            "nexus-control".to_string(),
            "treasury".to_string(),
            "recovery-cutover".to_string(),
            "--report-path".to_string(),
            "/tmp/recovery-work/report.json".to_string(),
        ])
        .expect("recovery-cutover should parse");
        assert!(matches!(
            recovery_cutover,
            super::TreasuryCommand::RecoveryCutover { .. }
        ));
    }

    #[test]
    fn runtime_policy_bootstraps_and_preserves_persisted_policy() {
        let mut config = test_treasury_config();
        config.state_path = unique_treasury_state_path("bootstrap");
        let mut state = TreasuryState::new(config.state_path.clone());

        let receipts = state.initialize_runtime_policy(&config, 100);
        assert!(receipts.is_empty());
        assert_eq!(state.policy_runtime_status.as_deref(), Some("bootstrapped"));
        assert_eq!(state.policy_change_history.len(), 1);
        let bootstrapped_policy = state
            .active_policy
            .clone()
            .expect("bootstrapped treasury policy");

        let mut drift_config = config.clone();
        drift_config.enabled = false;
        drift_config.payout_sats_per_window = 0;
        drift_config.daily_budget_cap_sats = 1;
        state.initialize_runtime_policy(&drift_config, 200);

        assert_eq!(
            state
                .active_policy
                .as_ref()
                .map(|policy| policy.checksum.as_str()),
            Some(bootstrapped_policy.checksum.as_str())
        );
        assert_eq!(state.policy_change_history.len(), 1);
    }

    #[test]
    fn runtime_policy_missing_placeholder_mode_defaults_disabled() {
        let policy: super::TreasuryRuntimePolicy = serde_json::from_value(serde_json::json!({
            "schema_version": 1,
            "treasury_enabled": true,
            "payout_sats_per_window": 25,
            "payout_interval_seconds": 600,
            "require_sellable": false,
            "daily_budget_cap_sats": 1_000_000,
            "checksum": "sha256:test"
        }))
        .expect("legacy policy should deserialize");

        assert_eq!(
            policy.placeholder_payout_mode,
            TreasuryPlaceholderPayoutMode::Disabled
        );
        assert_eq!(
            TreasuryPlaceholderPayoutMode::default(),
            TreasuryPlaceholderPayoutMode::Disabled
        );
    }

    #[test]
    fn runtime_policy_legacy_values_flow_into_split_policy_snapshots() {
        let mut config = test_treasury_config();
        config.accepted_work_default_payout_sats = 240;
        config.accepted_work_daily_budget_cap_sats = 9_999;
        config.availability_max_concurrent_sends = 3;

        let policy: super::TreasuryRuntimePolicy = serde_json::from_value(serde_json::json!({
            "schema_version": 1,
            "treasury_enabled": true,
            "payout_sats_per_window": 25,
            "payout_interval_seconds": 600,
            "require_sellable": true,
            "daily_budget_cap_sats": 1_000_000,
            "placeholder_payout_mode": "inference_ready",
            "dedupe_placeholder_hosts": true,
            "checksum": "sha256:test"
        }))
        .expect("legacy policy should deserialize");
        let resolved = policy.with_resolved_legacy_defaults(&config);

        assert_eq!(resolved.accepted_work_default_payout_sats(), 25);
        assert_eq!(resolved.accepted_work_daily_budget_cap_sats(), 1_000_000);
        assert_eq!(
            resolved.accepted_work_policy_snapshot(),
            super::TreasuryAcceptedWorkPolicySnapshot {
                default_payout_sats: 25,
                daily_budget_cap_sats: 1_000_000,
            }
        );
        assert_eq!(
            resolved.availability_policy_snapshot(&config),
            super::TreasuryAvailabilityPolicySnapshot {
                payout_sats_per_window: 25,
                payout_interval_seconds: 600,
                require_sellable: true,
                daily_budget_cap_sats: 1_000_000,
                max_concurrent_sends: 3,
                payout_mode: TreasuryPlaceholderPayoutMode::InferenceReady,
                dedupe_hosts: true,
                version_floor: None,
                version_floor_started_at_unix_ms: None,
                version_gate_active: false,
            }
        );
    }

    #[test]
    fn status_response_surfaces_split_accepted_and_availability_policy() {
        let mut config = test_treasury_config();
        config.accepted_work_default_payout_sats = 240;
        config.accepted_work_daily_budget_cap_sats = 5_000;
        config.payout_sats_per_window = 25;
        config.payout_interval_seconds = 600;
        config.daily_budget_cap_sats = 1_000_000;
        config.availability_max_concurrent_sends = 7;

        let mut state = TreasuryState::new(unique_treasury_state_path("split-status"));
        state.initialize_runtime_policy(&config, 100);

        let status = state.status_response(&config, 200);

        assert_eq!(status.payout_sats_per_window, 25);
        assert_eq!(status.accepted_work_policy.default_payout_sats, 240);
        assert_eq!(status.accepted_work_policy.daily_budget_cap_sats, 5_000);
        assert_eq!(status.availability_policy.payout_sats_per_window, 25);
        assert_eq!(status.availability_policy.payout_interval_seconds, 600);
        assert_eq!(status.availability_policy.max_concurrent_sends, 7);
        assert_eq!(
            status.availability_policy.payout_mode,
            TreasuryPlaceholderPayoutMode::InferenceReady
        );
    }

    #[test]
    fn degraded_states_surface_low_liquidity_and_stale_sync() {
        let mut config = test_treasury_config();
        config.accepted_work_default_payout_sats = 2_000;
        config.payout_sats_per_window = 500;
        config.wallet_status_refresh_seconds = 1;
        let now_unix_ms = 20_000u64;
        let mut state = TreasuryState::new(unique_treasury_state_path("degraded-low-liquidity"));
        state.initialize_runtime_policy(&config, 100);
        state.wallet_balance_sats = 500;
        state.wallet_balance_updated_at_unix_ms = Some(1_000);
        state.last_wallet_sync_at_unix_ms = Some(1_000);
        state.payout_targets_by_identity.insert(
            "pubkey-ldk".to_string(),
            super::RegisteredPayoutTarget {
                nostr_pubkey_hex: "pubkey-ldk".to_string(),
                source_session_id: "session-ldk".to_string(),
                payment_target_kind: "bolt11_invoice".to_string(),
                payment_target: "lnbcrt1test-target".to_string(),
                payment_target_capabilities: vec!["ldk_payment_target_v0_2".to_string()],
                pylon_payment_target_version: Some("pylon-v0.2.0".to_string()),
                provider_target: String::new(),
                bitcoin_address: None,
                registered_at_unix_ms: 1_000,
                last_verified_at_unix_ms: 1_000,
            },
        );

        let status = state.status_response(&config, now_unix_ms);
        let codes = status
            .degraded_states
            .iter()
            .map(|state| state.code.as_str())
            .collect::<BTreeSet<_>>();

        assert!(codes.contains("low_outbound_liquidity"));
        assert!(codes.contains("low_inbound_liquidity"));
        assert!(codes.contains("stale_wallet_sync"));
    }

    #[test]
    fn degraded_states_map_ldk_route_and_balance_failures() {
        let config = test_treasury_config();
        let now_unix_ms = 50_000u64;
        let mut state = TreasuryState::new(unique_treasury_state_path("degraded-route-fixture"));
        state.initialize_runtime_policy(&config, 100);
        state.wallet_balance_sats = 10_000;
        state.wallet_balance_updated_at_unix_ms = Some(now_unix_ms);
        state.last_wallet_sync_at_unix_ms = Some(now_unix_ms);
        let metadata = BTreeMap::from([("command".to_string(), "treasury.payInvoice".to_string())]);
        state.record_treasury_admin_operation(
            "treasury.payInvoice",
            "no-route-fixture",
            metadata.clone(),
            Some(12_000),
            "bolt11_invoice",
            Some("target-hash-a".to_string()),
            TreasuryOperationStatus::Failed,
            None,
            Some("ldk_server_client_error:no_route:route not found".to_string()),
            Some("payment_failed".to_string()),
            now_unix_ms.saturating_sub(100),
        );
        state.record_treasury_admin_operation(
            "treasury.payInvoice",
            "insufficient-fixture",
            metadata,
            Some(21_000),
            "bolt11_invoice",
            Some("target-hash-b".to_string()),
            TreasuryOperationStatus::Failed,
            None,
            Some("ldk_server_client_error:insufficient_balance:not enough balance".to_string()),
            Some("payment_failed".to_string()),
            now_unix_ms.saturating_sub(100),
        );

        let status = state.status_response(&config, now_unix_ms);
        let codes = status
            .degraded_states
            .iter()
            .map(|state| state.code.as_str())
            .collect::<BTreeSet<_>>();

        assert!(codes.contains("no_route"));
        assert!(codes.contains("insufficient_channel_balance"));
    }

    #[test]
    fn runtime_policy_applies_explicit_safe_env_change() {
        let mut config = test_treasury_config();
        config.state_path = unique_treasury_state_path("safe-change");
        let mut state = TreasuryState::new(config.state_path.clone());
        state.initialize_runtime_policy(&config, 100);

        let mut updated_config = config.clone();
        updated_config.apply_env_policy = true;
        updated_config.daily_budget_cap_sats = 2_000;
        updated_config.policy_change_reason = Some("raise_daily_budget".to_string());
        let receipts = state.initialize_runtime_policy(&updated_config, 200);

        assert_eq!(receipts.len(), 1);
        assert_eq!(receipts[0].receipt_type, "treasury.policy.changed");
        assert_eq!(
            state
                .active_policy
                .as_ref()
                .map(|policy| policy.daily_budget_cap_sats),
            Some(2_000)
        );
        let status = state.status_response(&updated_config, 200);
        assert_eq!(status.policy_runtime_status.as_deref(), Some("updated"));
        assert_eq!(status.recent_policy_changes.len(), 2);
        assert_eq!(status.recent_policy_changes[0].reason, "raise_daily_budget");
    }

    #[test]
    fn runtime_policy_blocks_destructive_change_without_override() {
        let mut config = test_treasury_config();
        config.state_path = unique_treasury_state_path("blocked-change");
        let mut state = TreasuryState::new(config.state_path.clone());
        state.initialize_runtime_policy(&config, 100);
        let original_checksum = state
            .active_policy
            .as_ref()
            .map(|policy| policy.checksum.clone())
            .expect("bootstrapped policy checksum");

        let mut destructive_config = config.clone();
        destructive_config.apply_env_policy = true;
        destructive_config.daily_budget_cap_sats = 60;
        destructive_config.policy_change_reason = Some("lower_daily_budget".to_string());
        let receipts = state.initialize_runtime_policy(&destructive_config, 200);

        assert_eq!(receipts.len(), 1);
        assert_eq!(receipts[0].receipt_type, "treasury.policy.change_blocked");
        assert_eq!(state.policy_runtime_status.as_deref(), Some("blocked"));
        assert_eq!(
            state
                .active_policy
                .as_ref()
                .map(|policy| policy.checksum.as_str()),
            Some(original_checksum.as_str())
        );
        assert_eq!(state.policy_change_history.len(), 1);
    }

    #[test]
    fn recovery_comparison_detects_zero_balance_divergence() {
        let comparison = build_treasury_wallet_recovery_comparison(
            &TreasuryWalletInspection {
                wallet_identity_pubkey: "identity".to_string(),
                inspected_storage_dir: "/tmp/current".to_string(),
                runtime_status: Some("synced".to_string()),
                balance_sats: Some(0),
                payment_totals: TreasuryWalletPaymentAggregate {
                    completed_receive_total_sats: 100_000,
                    completed_send_total_sats: 20_000,
                    ..TreasuryWalletPaymentAggregate::default()
                },
                ..TreasuryWalletInspection::default()
            },
            &TreasuryWalletInspection {
                wallet_identity_pubkey: "identity".to_string(),
                inspected_storage_dir: "/tmp/rebuilt".to_string(),
                runtime_status: Some("synced".to_string()),
                balance_sats: Some(80_000),
                ..TreasuryWalletInspection::default()
            },
        );

        assert!(comparison.wallet_identity_pubkey_match);
        assert!(comparison.current_zero_with_receive_history);
        assert!(comparison.major_divergence_detected);
        assert!(comparison.validation_passed);
        assert_eq!(
            comparison.recommended_action,
            "cutover_rebuilt_storage_after_service_stop"
        );
    }

    #[test]
    fn recovery_comparison_blocks_cutover_when_rebuilt_regresses_current() {
        let comparison = build_treasury_wallet_recovery_comparison(
            &TreasuryWalletInspection {
                wallet_identity_pubkey: "identity".to_string(),
                inspected_storage_dir: "/tmp/current".to_string(),
                runtime_status: Some("synced".to_string()),
                balance_sats: Some(1_500),
                payment_totals: TreasuryWalletPaymentAggregate {
                    total_payments: 4,
                    completed_receive_total_sats: 10_000,
                    completed_send_total_sats: 8_500,
                    ..TreasuryWalletPaymentAggregate::default()
                },
                ..TreasuryWalletInspection::default()
            },
            &TreasuryWalletInspection {
                wallet_identity_pubkey: "identity".to_string(),
                inspected_storage_dir: "/tmp/rebuilt".to_string(),
                runtime_status: Some("synced".to_string()),
                balance_sats: Some(0),
                payment_totals: TreasuryWalletPaymentAggregate::default(),
                ..TreasuryWalletInspection::default()
            },
        );

        assert!(comparison.wallet_identity_pubkey_match);
        assert!(comparison.major_divergence_detected);
        assert!(comparison.validation_passed);
        assert_eq!(
            comparison.recommended_action,
            "inspect_divergence_before_cutover"
        );
    }

    #[test]
    fn recovery_comparison_accepts_cached_timeout_only_as_non_cutover_report() {
        let comparison = build_treasury_wallet_recovery_comparison(
            &TreasuryWalletInspection {
                wallet_identity_pubkey: "identity".to_string(),
                inspected_storage_dir: "/tmp/current".to_string(),
                runtime_status: Some("cached_after_sync_timeout".to_string()),
                runtime_detail: Some("live sync timed out; cached balance only".to_string()),
                balance_sats: Some(80),
                ..TreasuryWalletInspection::default()
            },
            &TreasuryWalletInspection {
                wallet_identity_pubkey: "identity".to_string(),
                inspected_storage_dir: "/tmp/rebuilt".to_string(),
                runtime_status: Some("cached_after_sync_timeout".to_string()),
                runtime_detail: Some("live sync timed out; cached balance only".to_string()),
                balance_sats: Some(80),
                ..TreasuryWalletInspection::default()
            },
        );

        assert!(comparison.wallet_identity_pubkey_match);
        assert!(comparison.validation_passed);
        assert!(!comparison.major_divergence_detected);
        assert_eq!(
            comparison.recommended_action,
            "no_cutover_needed_sync_timeout_cached"
        );
    }

    #[test]
    fn recovery_comparison_never_recommends_cutover_from_cached_timeout_divergence() {
        let comparison = build_treasury_wallet_recovery_comparison(
            &TreasuryWalletInspection {
                wallet_identity_pubkey: "identity".to_string(),
                inspected_storage_dir: "/tmp/current".to_string(),
                runtime_status: Some("cached_after_sync_timeout".to_string()),
                balance_sats: Some(0),
                payment_totals: TreasuryWalletPaymentAggregate {
                    completed_receive_total_sats: 100_000,
                    completed_send_total_sats: 20_000,
                    ..TreasuryWalletPaymentAggregate::default()
                },
                ..TreasuryWalletInspection::default()
            },
            &TreasuryWalletInspection {
                wallet_identity_pubkey: "identity".to_string(),
                inspected_storage_dir: "/tmp/rebuilt".to_string(),
                runtime_status: Some("cached_after_sync_timeout".to_string()),
                balance_sats: Some(80_000),
                ..TreasuryWalletInspection::default()
            },
        );

        assert!(comparison.wallet_identity_pubkey_match);
        assert!(comparison.validation_passed);
        assert!(comparison.major_divergence_detected);
        assert_eq!(
            comparison.recommended_action,
            "retry_live_sync_before_cutover"
        );
    }

    #[test]
    fn payout_preparation_uses_persisted_policy_instead_of_env_drift() {
        let mut state = TreasuryState::default();
        let config = test_treasury_config();
        state.initialize_runtime_policy(&config, 100);
        state.payout_targets_by_identity.insert(
            "pubkey-a".to_string(),
            super::RegisteredPayoutTarget {
                nostr_pubkey_hex: "pubkey-a".to_string(),
                source_session_id: "session-a".to_string(),
                payment_target_kind: String::new(),
                payment_target: String::new(),
                payment_target_capabilities: Vec::new(),
                pylon_payment_target_version: None,
                provider_target: "provider:alice".to_string(),
                bitcoin_address: None,
                registered_at_unix_ms: 10,
                last_verified_at_unix_ms: 10,
            },
        );

        let mut drift_config = config.clone();
        drift_config.enabled = false;
        drift_config.payout_sats_per_window = 0;
        state.initialize_runtime_policy(&drift_config, 200);

        let now_unix_ms = super::now_unix_ms();
        let prepared = state.prepare_due_payouts(
            &drift_config,
            &[test_online_identity("pubkey-a")],
            now_unix_ms,
        );

        assert_eq!(prepared.dispatch_plans.len(), 1);
        let stats = state.public_stats(&drift_config, now_unix_ms);
        assert!(stats.treasury_enabled);
        assert_eq!(stats.payout_sats_per_window, config.payout_sats_per_window);
    }

    #[test]
    fn payout_preparation_is_identity_deduped_and_idempotent() {
        let mut state = TreasuryState::default();
        let config = test_treasury_config();
        state.payout_targets_by_identity.insert(
            "pubkey-a".to_string(),
            super::RegisteredPayoutTarget {
                nostr_pubkey_hex: "pubkey-a".to_string(),
                source_session_id: "session-a".to_string(),
                payment_target_kind: String::new(),
                payment_target: String::new(),
                payment_target_capabilities: Vec::new(),
                pylon_payment_target_version: None,
                provider_target: "provider:alice".to_string(),
                bitcoin_address: None,
                registered_at_unix_ms: 10,
                last_verified_at_unix_ms: 10,
            },
        );

        let online = vec![test_online_identity("pubkey-a")];
        let now_unix_ms = super::now_unix_ms();
        let prepared = state.prepare_due_payouts(&config, &online, now_unix_ms);
        assert_eq!(prepared.dispatch_plans.len(), 1);
        assert!(prepared.receipt_events.is_empty());

        let prepared_again = state.prepare_due_payouts(&config, &online, now_unix_ms);
        assert!(prepared_again.dispatch_plans.is_empty());
        assert!(prepared_again.receipt_events.is_empty());
    }

    #[test]
    fn no_op_payout_preparation_does_not_rewrite_treasury_state() {
        let path = unique_treasury_state_path("noop-persist");
        std::fs::write(path.as_path(), "sentinel\n").expect("write sentinel");

        let mut state = TreasuryState::default();
        state.next_challenge_nonce = 1;
        state.state_path = Some(path.clone());

        let config = test_treasury_config();
        let prepared = state.prepare_due_payouts(&config, &[], super::now_unix_ms());

        assert!(prepared.dispatch_plans.is_empty());
        assert!(prepared.receipt_events.is_empty());
        assert_eq!(
            std::fs::read_to_string(path.as_path()).expect("read sentinel"),
            "sentinel\n"
        );

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn duplicate_queued_payout_request_does_not_rewrite_treasury_state() {
        let path = unique_treasury_state_path("duplicate-queue-noop");
        std::fs::write(path.as_path(), "sentinel\n").expect("write sentinel");

        let mut state = TreasuryState::default();
        state.next_challenge_nonce = 1;
        state.state_path = Some(path.clone());
        let payout_key = "window.existing:pubkey-a".to_string();
        state.payout_records_by_key.insert(
            payout_key.clone(),
            TreasuryPayoutRecord {
                payout_key: payout_key.clone(),
                nostr_pubkey_hex: "pubkey-a".to_string(),
                payout_target: "provider:alice".to_string(),
                amount_sats: 25,
                status: "queued".to_string(),
                reason: None,
                payment_id: None,
                window_started_at_unix_ms: 1_000,
                window_ends_at_unix_ms: 2_000,
                created_at_unix_ms: 1_000,
                updated_at_unix_ms: 1_000,
                sellable_at_window_open: true,
                dispatch_receipt_recorded: false,
                confirm_receipt_recorded: false,
                fail_receipt_recorded: false,
                skip_receipt_recorded: false,
                counted_in_paid_total: false,
                classification: TreasuryPayoutClassification {
                    payout_class: TreasuryPayoutClass::AcceptedWork,
                    ..TreasuryPayoutClassification::default()
                },
            },
        );

        state.queue_payout_requests(
            &test_treasury_config(),
            &[TreasuryQueuedPayoutRequest {
                payout_key,
                nostr_pubkey_hex: "pubkey-a".to_string(),
                amount_sats: 25,
                window_started_at_unix_ms: 1_000,
                window_ends_at_unix_ms: 2_000,
                classification: TreasuryPayoutClassification {
                    payout_class: TreasuryPayoutClass::AcceptedWork,
                    ..TreasuryPayoutClassification::default()
                },
                queue_block_reason: None,
            }],
            3_000,
        );

        assert_eq!(
            std::fs::read_to_string(path.as_path()).expect("read sentinel"),
            "sentinel\n"
        );

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn wallet_snapshot_without_ledger_changes_persists_wallet_status() {
        let path = unique_treasury_state_path("wallet-refresh-noop");
        let mut state = TreasuryState::default();
        state.next_challenge_nonce = 1;
        state.wallet_runtime_status = Some("error".to_string());
        state.wallet_last_error = Some("treasury_isolated_dispatch_timeout:70000".to_string());
        state.wallet_balance_sats = 12;
        state.wallet_balance_updated_at_unix_ms = Some(1_776_027_000_000u64);
        state.last_wallet_refresh_attempt_at_unix_ms = Some(1_776_027_000_000u64);
        state.state_path = Some(path.clone());
        state.persist();

        let before = std::fs::read_to_string(path.as_path()).expect("read persisted state");
        let now_unix_ms = 1_776_028_000_000u64;
        let receipts = state.apply_wallet_snapshot(
            &TreasuryWalletSnapshot {
                runtime_status: "connected".to_string(),
                runtime_detail: None,
                wallet_hydration_mode: Some("sync_wallet_then_cached_balance".to_string()),
                wallet_payment_scan_mode: Some("recent_only".to_string()),
                balance_sats: 321,
                total_onchain_balance_sats: 321,
                spendable_onchain_balance_sats: 321,
                lightning_balance_sats: 0,
                payments: Vec::new(),
            },
            now_unix_ms,
        );

        assert!(receipts.is_empty());
        let after = std::fs::read_to_string(path.as_path()).expect("read persisted state");
        assert_ne!(after, before);

        let persisted = TreasuryState::new(path.clone());
        assert_eq!(
            persisted.wallet_runtime_status.as_deref(),
            Some("connected")
        );
        assert_eq!(persisted.wallet_last_error, None);
        assert_eq!(
            persisted.wallet_hydration_mode.as_deref(),
            Some("sync_wallet_then_cached_balance")
        );
        assert_eq!(
            persisted.wallet_payment_scan_mode.as_deref(),
            Some("recent_only")
        );
        assert_eq!(persisted.wallet_balance_sats, 321);
        assert_eq!(
            persisted.wallet_balance_updated_at_unix_ms,
            Some(now_unix_ms)
        );
        assert_eq!(persisted.last_wallet_sync_at_unix_ms, Some(now_unix_ms));
        assert_eq!(
            persisted.last_wallet_refresh_attempt_at_unix_ms,
            Some(now_unix_ms)
        );

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn placeholder_liveness_records_compact_without_dropping_homework_records() {
        let mut state = TreasuryState::default();
        state.next_challenge_nonce = 1;
        let now_unix_ms = super::now_unix_ms();
        for index in 0..(super::TREASURY_PLACEHOLDER_PAYOUT_RECORD_LIMIT + 10) {
            let payout_key = format!("placeholder.{index:04}:pubkey-{index:04}");
            let updated_at_unix_ms = now_unix_ms.saturating_sub(index as u64);
            state.payout_records_by_key.insert(
                payout_key.clone(),
                TreasuryPayoutRecord {
                    payout_key,
                    nostr_pubkey_hex: format!("pubkey-{index:04}"),
                    payout_target: format!("provider:{index:04}"),
                    amount_sats: 1,
                    status: "confirmed".to_string(),
                    reason: None,
                    payment_id: Some(format!("payment-placeholder-{index:04}")),
                    window_started_at_unix_ms: updated_at_unix_ms,
                    window_ends_at_unix_ms: updated_at_unix_ms.saturating_add(1),
                    created_at_unix_ms: updated_at_unix_ms,
                    updated_at_unix_ms,
                    sellable_at_window_open: true,
                    dispatch_receipt_recorded: true,
                    confirm_receipt_recorded: true,
                    fail_receipt_recorded: false,
                    skip_receipt_recorded: false,
                    counted_in_paid_total: true,
                    classification: TreasuryPayoutClassification::default(),
                },
            );
        }
        state.payout_records_by_key.insert(
            "homework.accepted:pubkey-homework".to_string(),
            TreasuryPayoutRecord {
                payout_key: "homework.accepted:pubkey-homework".to_string(),
                nostr_pubkey_hex: "pubkey-homework".to_string(),
                payout_target: "provider:homework".to_string(),
                amount_sats: 25,
                status: "confirmed".to_string(),
                reason: None,
                payment_id: Some("payment-homework".to_string()),
                window_started_at_unix_ms: now_unix_ms,
                window_ends_at_unix_ms: now_unix_ms.saturating_add(1),
                created_at_unix_ms: now_unix_ms,
                updated_at_unix_ms: now_unix_ms,
                sellable_at_window_open: true,
                dispatch_receipt_recorded: true,
                confirm_receipt_recorded: true,
                fail_receipt_recorded: false,
                skip_receipt_recorded: false,
                counted_in_paid_total: true,
                classification: TreasuryPayoutClassification {
                    payout_class: TreasuryPayoutClass::AcceptedWork,
                    training_run_id: Some("run.cs336.a1.demo".to_string()),
                    ..TreasuryPayoutClassification::default()
                },
            },
        );

        assert!(state.trim_retention(now_unix_ms));

        let placeholder_count = state
            .payout_records_by_key
            .values()
            .filter(|record| {
                record.classification.payout_class == TreasuryPayoutClass::PlaceholderLiveness
            })
            .count();
        assert_eq!(
            placeholder_count,
            super::TREASURY_PLACEHOLDER_PAYOUT_RECORD_LIMIT
        );
        assert!(
            state
                .payout_records_by_key
                .contains_key("homework.accepted:pubkey-homework")
        );
    }

    #[test]
    fn stale_treasury_state_is_pruned_and_rewritten_on_load() {
        let path = unique_treasury_state_path("load-compact");
        let mut state = TreasuryState::default();
        state.next_challenge_nonce = 1;
        let stale_updated_at_unix_ms =
            super::now_unix_ms().saturating_sub(super::TREASURY_STATE_RETENTION_WINDOW_MS + 1_000);

        for index in 0..5 {
            let payout_key = format!("{index:08}:pubkey-{index:08}");
            state.payout_records_by_key.insert(
                payout_key.clone(),
                TreasuryPayoutRecord {
                    payout_key,
                    nostr_pubkey_hex: format!("pubkey-{index:08}"),
                    payout_target: format!("provider:{index:08}"),
                    amount_sats: 1,
                    status: "confirmed".to_string(),
                    reason: None,
                    payment_id: Some(format!("payment-{index:08}")),
                    window_started_at_unix_ms: stale_updated_at_unix_ms,
                    window_ends_at_unix_ms: stale_updated_at_unix_ms + 1,
                    created_at_unix_ms: stale_updated_at_unix_ms,
                    updated_at_unix_ms: stale_updated_at_unix_ms,
                    sellable_at_window_open: true,
                    dispatch_receipt_recorded: true,
                    confirm_receipt_recorded: true,
                    fail_receipt_recorded: false,
                    skip_receipt_recorded: false,
                    counted_in_paid_total: true,
                    classification: TreasuryPayoutClassification::default(),
                },
            );
        }

        std::fs::write(
            path.as_path(),
            serde_json::to_string(&state).expect("serialize oversized state"),
        )
        .expect("write oversized state");

        let loaded = TreasuryState::new(path.clone());
        assert!(loaded.payout_records_by_key.is_empty());

        let rewritten: TreasuryState = serde_json::from_str(
            std::fs::read_to_string(path.as_path())
                .expect("read compacted state")
                .as_str(),
        )
        .expect("parse compacted state");
        assert!(rewritten.payout_records_by_key.is_empty());

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn loading_legacy_accepted_work_records_backfills_class_and_totals() {
        let path = unique_treasury_state_path("legacy-accepted-work-class");
        let now_unix_ms = super::now_unix_ms();
        let payout_key = "legacy.accepted.pubkey-a".to_string();
        let mut state = TreasuryState::default();
        state.next_challenge_nonce = 1;
        state.payout_sats_paid_total = 25;
        state.placeholder_payout_sats_paid_total = 25;
        state.payout_records_by_key.insert(
            payout_key.clone(),
            TreasuryPayoutRecord {
                payout_key: payout_key.clone(),
                nostr_pubkey_hex: "pubkey-a".to_string(),
                payout_target: "provider:alice".to_string(),
                amount_sats: 25,
                status: "confirmed".to_string(),
                reason: None,
                payment_id: Some("payment-legacy-accepted".to_string()),
                window_started_at_unix_ms: now_unix_ms.saturating_sub(60_000),
                window_ends_at_unix_ms: now_unix_ms.saturating_sub(1),
                created_at_unix_ms: now_unix_ms.saturating_sub(60_000),
                updated_at_unix_ms: now_unix_ms.saturating_sub(10_000),
                sellable_at_window_open: true,
                dispatch_receipt_recorded: true,
                confirm_receipt_recorded: true,
                fail_receipt_recorded: false,
                skip_receipt_recorded: false,
                counted_in_paid_total: true,
                classification: TreasuryPayoutClassification {
                    payout_basis: Some("homework_acceptance".to_string()),
                    accepted_outcome_id: Some("accepted.legacy.one".to_string()),
                    training_run_id: Some("run.cs336.a1.legacy".to_string()),
                    assignment_id: Some("assign-legacy".to_string()),
                    ..TreasuryPayoutClassification::default()
                },
            },
        );

        let mut payload = serde_json::to_value(&state).expect("serialize treasury state");
        payload
            .pointer_mut("/payout_records_by_key/legacy.accepted.pubkey-a/classification")
            .and_then(serde_json::Value::as_object_mut)
            .expect("classification object")
            .remove("payout_class");
        std::fs::write(
            path.as_path(),
            serde_json::to_string(&payload).expect("serialize legacy payload"),
        )
        .expect("write legacy payload");

        let loaded = TreasuryState::new(path.clone());
        let record = loaded
            .payout_records_by_key
            .get(&payout_key)
            .expect("legacy accepted-work payout record");
        assert_eq!(
            record.classification.payout_class,
            TreasuryPayoutClass::AcceptedWork
        );
        assert_eq!(loaded.payout_sats_paid_total, 25);
        assert_eq!(loaded.accepted_work_payout_sats_paid_total, 25);
        assert_eq!(loaded.placeholder_payout_sats_paid_total, 0);

        let rewritten: TreasuryState = serde_json::from_str(
            std::fs::read_to_string(path.as_path())
                .expect("read rewritten legacy state")
                .as_str(),
        )
        .expect("parse rewritten legacy state");
        let rewritten_record = rewritten
            .payout_records_by_key
            .get(&payout_key)
            .expect("rewritten legacy payout record");
        assert_eq!(
            rewritten_record.classification.payout_class,
            TreasuryPayoutClass::AcceptedWork
        );
        assert_eq!(rewritten.accepted_work_payout_sats_paid_total, 25);
        assert_eq!(rewritten.placeholder_payout_sats_paid_total, 0);

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn payout_preparation_keeps_pre_cutoff_backlog_but_blocks_new_accrual_below_floor() {
        let mut state = TreasuryState::default();
        let mut config = test_treasury_config();
        config.min_new_accrual_pylon_version = Some("pylon-v0.1.1-rc1".to_string());
        state.payout_targets_by_identity.insert(
            "pubkey-a".to_string(),
            super::RegisteredPayoutTarget {
                nostr_pubkey_hex: "pubkey-a".to_string(),
                source_session_id: "session-a".to_string(),
                payment_target_kind: String::new(),
                payment_target: String::new(),
                payment_target_capabilities: Vec::new(),
                pylon_payment_target_version: None,
                provider_target: "provider:alice".to_string(),
                bitcoin_address: None,
                registered_at_unix_ms: 10,
                last_verified_at_unix_ms: 10,
            },
        );

        let now_unix_ms = 1_800_000;
        let payout_interval_ms = config.payout_interval_ms();
        let current_window_started_at_unix_ms = payout_window_started_at_for_identity(
            now_unix_ms,
            payout_interval_ms,
            "payout_target:provider:alice",
        );
        config.min_new_accrual_started_at_unix_ms = Some(current_window_started_at_unix_ms);
        state.last_payout_reconciliation_at_unix_ms =
            Some(current_window_started_at_unix_ms.saturating_sub(payout_interval_ms));

        let prepared = state.prepare_due_payouts(
            &config,
            &[OnlinePylonIdentity {
                client_version: Some("0.0.1-rc12".to_string()),
                ..test_online_identity("pubkey-a")
            }],
            now_unix_ms,
        );

        assert_eq!(prepared.dispatch_plans.len(), 1);
        assert_eq!(
            prepared.dispatch_plans[0].payout_key,
            payout_window_key(
                payout_window_started_at_for_identity(
                    current_window_started_at_unix_ms.saturating_sub(payout_interval_ms),
                    payout_interval_ms,
                    "payout_target:provider:alice",
                ),
                availability_beneficiary_scope_key("payout_target:provider:alice").as_str()
            )
        );

        let blocked_record = state
            .payout_records_by_key
            .get(
                payout_window_key(
                    current_window_started_at_unix_ms,
                    availability_beneficiary_scope_key("payout_target:provider:alice").as_str(),
                )
                .as_str(),
            )
            .expect("post-cutoff payout record");
        assert_eq!(blocked_record.status, "skipped");
        assert_eq!(
            blocked_record.reason.as_deref(),
            Some("below_min_new_accrual_version_floor")
        );
    }

    #[test]
    fn observe_payout_eligibility_surfaces_version_floor_blocks() {
        let mut state = TreasuryState::default();
        let mut config = test_treasury_config();
        config.min_new_accrual_pylon_version = Some("pylon-v0.1.1-rc1".to_string());
        state.payout_targets_by_identity.insert(
            "pubkey-a".to_string(),
            super::RegisteredPayoutTarget {
                nostr_pubkey_hex: "pubkey-a".to_string(),
                source_session_id: "session-a".to_string(),
                payment_target_kind: String::new(),
                payment_target: String::new(),
                payment_target_capabilities: Vec::new(),
                pylon_payment_target_version: None,
                provider_target: "provider:alice".to_string(),
                bitcoin_address: None,
                registered_at_unix_ms: 10,
                last_verified_at_unix_ms: 10,
            },
        );

        let now_unix_ms = 1_800_000;
        config.min_new_accrual_started_at_unix_ms = Some(payout_window_started_at_for_identity(
            now_unix_ms,
            config.payout_interval_ms(),
            "payout_target:provider:alice",
        ));
        state.observe_payout_eligibility(&config, &[test_online_identity("pubkey-a")], now_unix_ms);

        let stats = state.public_stats(&config, now_unix_ms);
        assert!(stats.min_new_accrual_version_gate_active);
        assert_eq!(
            stats.min_new_accrual_pylon_version.as_deref(),
            Some("pylon-v0.1.1-rc1")
        );
        assert_eq!(stats.eligible_online_payout_targets, 0);
        assert_eq!(stats.availability_online_identities_now, 1);
        assert_eq!(stats.availability_stipend_eligible_beneficiaries_now, 0);
        assert_eq!(stats.version_floor_blocked_beneficiaries_now, 1);
        assert_eq!(stats.min_new_accrual_version_blocked_online_targets, 1);
        assert_eq!(stats.min_new_accrual_unknown_version_online_targets, 1);
    }

    #[test]
    fn availability_stipend_payout_class_serializes_canonically_and_reads_legacy_payloads() {
        let classification = TreasuryPayoutClassification {
            payout_class: TreasuryPayoutClass::PlaceholderLiveness,
            ..TreasuryPayoutClassification::default()
        };
        let payload = serde_json::to_value(&classification).expect("serialize classification");
        assert_eq!(
            payload
                .get("payout_class")
                .and_then(serde_json::Value::as_str),
            Some("availability_stipend")
        );

        let legacy_payload = serde_json::json!({
            "payout_class": "placeholder_liveness"
        });
        let legacy: TreasuryPayoutClassification =
            serde_json::from_value(legacy_payload).expect("deserialize legacy classification");
        assert_eq!(
            legacy.payout_class,
            TreasuryPayoutClass::PlaceholderLiveness
        );
    }

    #[test]
    fn treasury_status_surfaces_availability_beneficiary_observability() {
        let mut state = TreasuryState::default();
        let mut config = test_treasury_config();
        config.min_new_accrual_pylon_version = Some("pylon-v0.1.10".to_string());

        for (pubkey, payment_target) in [
            ("pubkey-a", "lno1shared"),
            ("pubkey-b", "lno1pubkeyb"),
            ("pubkey-c", "lno1shared"),
            ("pubkey-e", "lno1pubkeye"),
            ("pubkey-f", "lno1pubkeyf"),
        ] {
            state.payout_targets_by_identity.insert(
                pubkey.to_string(),
                super::RegisteredPayoutTarget {
                    nostr_pubkey_hex: pubkey.to_string(),
                    source_session_id: format!("session-{pubkey}"),
                    payment_target_kind: "bolt12_offer".to_string(),
                    payment_target: payment_target.to_string(),
                    payment_target_capabilities: vec![
                        "ldk_payment_target_v0_2".to_string(),
                        "bolt12_offer".to_string(),
                    ],
                    pylon_payment_target_version: Some("pylon-payment-target/v0.2".to_string()),
                    provider_target: String::new(),
                    bitcoin_address: None,
                    registered_at_unix_ms: 10,
                    last_verified_at_unix_ms: 10,
                },
            );
        }

        let now_unix_ms = 1_800_000;
        config.min_new_accrual_started_at_unix_ms = Some(0);

        let online = vec![
            OnlinePylonIdentity {
                client_version: Some("pylon-v0.1.10".to_string()),
                host_fingerprint: Some("host-1".to_string()),
                ..test_online_identity("pubkey-a")
            },
            OnlinePylonIdentity {
                client_version: Some("pylon-v0.1.10".to_string()),
                host_fingerprint: Some("host-1".to_string()),
                ..test_online_identity("pubkey-b")
            },
            OnlinePylonIdentity {
                client_version: Some("pylon-v0.1.10".to_string()),
                host_fingerprint: Some("host-2".to_string()),
                ..test_online_identity("pubkey-c")
            },
            OnlinePylonIdentity {
                host_fingerprint: Some("host-3".to_string()),
                ..test_online_identity("pubkey-d")
            },
            OnlinePylonIdentity {
                client_version: Some("pylon-v0.1.10".to_string()),
                host_fingerprint: Some("host-4".to_string()),
                availability_stipend_eligible: false,
                availability_stipend_gate_reason: Some("missing_worker_role".to_string()),
                ..test_online_identity("pubkey-e")
            },
            OnlinePylonIdentity {
                client_version: None,
                host_fingerprint: Some("host-5".to_string()),
                ..test_online_identity("pubkey-f")
            },
        ];

        state.observe_payout_eligibility(&config, &online, now_unix_ms);
        let prepared = state.prepare_due_payouts(&config, &online, now_unix_ms);
        assert_eq!(prepared.dispatch_plans.len(), 1);

        let status = state.status_response(&config, now_unix_ms);
        assert_eq!(status.availability_online_identities_now, 6);
        assert_eq!(status.availability_online_host_clusters_now, 5);
        assert_eq!(status.availability_stipend_eligible_beneficiaries_now, 1);
        assert_eq!(status.eligible_online_payout_targets, 1);
        assert_eq!(status.duplicate_host_blocked_beneficiaries_now, 1);
        assert_eq!(status.duplicate_payout_target_blocked_beneficiaries_now, 1);
        assert_eq!(status.missing_payout_target_blocked_beneficiaries_now, 1);
        assert_eq!(status.version_floor_blocked_beneficiaries_now, 1);
        assert_eq!(status.readiness_blocked_beneficiaries_now, 1);
        assert_eq!(status.min_new_accrual_unknown_version_online_targets, 1);
        assert_eq!(status.availability_beneficiary_debug_rows.len(), 6);
        assert_eq!(status.availability_stipend_payout_sats_paid_total, 0);
        assert_eq!(status.availability_stipend_payout_sats_paid_24h, 0);

        let row_a = status
            .availability_beneficiary_debug_rows
            .iter()
            .find(|row| row.nostr_pubkey_hex == "pubkey-a")
            .expect("eligible row");
        assert!(row_a.availability_stipend_eligible_now);
        assert_eq!(row_a.verdict_reason, "eligible");
        assert_eq!(row_a.current_payout_status.as_deref(), Some("dispatching"));

        let row_b = status
            .availability_beneficiary_debug_rows
            .iter()
            .find(|row| row.nostr_pubkey_hex == "pubkey-b")
            .expect("duplicate host row");
        assert_eq!(row_b.verdict_reason, "duplicate_host_placeholder_readiness");
        assert_eq!(row_b.current_payout_status.as_deref(), Some("skipped"));

        let row_c = status
            .availability_beneficiary_debug_rows
            .iter()
            .find(|row| row.nostr_pubkey_hex == "pubkey-c")
            .expect("duplicate payout target row");
        assert_eq!(
            row_c.verdict_reason,
            "duplicate_payout_target_placeholder_readiness"
        );
        assert_eq!(row_c.current_payout_status.as_deref(), Some("skipped"));

        let row_d = status
            .availability_beneficiary_debug_rows
            .iter()
            .find(|row| row.nostr_pubkey_hex == "pubkey-d")
            .expect("missing payout target row");
        assert_eq!(row_d.verdict_reason, "missing_payout_target");
        assert!(row_d.payout_target.is_none());

        let row_e = status
            .availability_beneficiary_debug_rows
            .iter()
            .find(|row| row.nostr_pubkey_hex == "pubkey-e")
            .expect("readiness row");
        assert_eq!(row_e.verdict_reason, "missing_worker_role");

        let row_f = status
            .availability_beneficiary_debug_rows
            .iter()
            .find(|row| row.nostr_pubkey_hex == "pubkey-f")
            .expect("version floor row");
        assert_eq!(
            row_f.verdict_reason,
            "missing_client_version_for_new_accrual"
        );
    }

    #[test]
    fn placeholder_payout_mode_inference_ready_skips_presence_only_nodes() {
        let mut state = TreasuryState::default();
        let config = test_treasury_config();
        state.payout_targets_by_identity.insert(
            "pubkey-a".to_string(),
            super::RegisteredPayoutTarget {
                nostr_pubkey_hex: "pubkey-a".to_string(),
                source_session_id: "session-a".to_string(),
                payment_target_kind: String::new(),
                payment_target: String::new(),
                payment_target_capabilities: Vec::new(),
                pylon_payment_target_version: None,
                provider_target: "provider:alice".to_string(),
                bitcoin_address: None,
                registered_at_unix_ms: 10,
                last_verified_at_unix_ms: 10,
            },
        );

        let now_unix_ms = 1_800_000;
        let prepared = state.prepare_due_payouts(
            &config,
            &[OnlinePylonIdentity {
                client_version: Some("pylon-v0.1.1-rc1".to_string()),
                inference_ready: false,
                ..test_online_identity("pubkey-a")
            }],
            now_unix_ms,
        );

        assert!(prepared.dispatch_plans.is_empty());
        let payout_key = payout_window_key(
            payout_window_started_at_for_identity(
                now_unix_ms,
                config.payout_interval_ms(),
                "pubkey-a",
            ),
            availability_identity_scope_key("pubkey-a").as_str(),
        );
        let record = state
            .payout_records_by_key
            .get(payout_key.as_str())
            .expect("placeholder skip");
        assert_eq!(
            record.reason.as_deref(),
            Some("placeholder_requires_inference_ready")
        );
        assert_eq!(
            record.classification.payout_basis.as_deref(),
            Some("inference_ready")
        );
    }

    #[test]
    fn placeholder_payouts_disabled_only_dispatches_queued_accepted_work() {
        let mut state = TreasuryState::default();
        let mut config = test_treasury_config();
        config.placeholder_payout_mode = TreasuryPlaceholderPayoutMode::Disabled;
        state.payout_targets_by_identity.insert(
            "pubkey-a".to_string(),
            super::RegisteredPayoutTarget {
                nostr_pubkey_hex: "pubkey-a".to_string(),
                source_session_id: "session-a".to_string(),
                payment_target_kind: String::new(),
                payment_target: String::new(),
                payment_target_capabilities: Vec::new(),
                pylon_payment_target_version: None,
                provider_target: "provider:alice".to_string(),
                bitcoin_address: None,
                registered_at_unix_ms: 10,
                last_verified_at_unix_ms: 10,
            },
        );

        let now_unix_ms = super::now_unix_ms();
        state.wallet_balance_sats = 1_000_000;
        let online = vec![OnlinePylonIdentity {
            client_version: Some("pylon-v0.1.1-rc1".to_string()),
            ..test_online_identity("pubkey-a")
        }];

        state.observe_payout_eligibility(&config, &online, now_unix_ms);
        let stats = state.public_stats(&config, now_unix_ms);
        assert_eq!(
            stats.placeholder_payout_mode,
            TreasuryPlaceholderPayoutMode::Disabled
        );
        assert_eq!(stats.inference_ready_online_payout_targets, 1);
        assert_eq!(stats.eligible_online_payout_targets, 0);

        let prepared = state.prepare_due_payouts(&config, &online, now_unix_ms);
        assert!(prepared.dispatch_plans.is_empty());
        assert!(prepared.receipt_events.is_empty());
        assert!(state.payout_records_by_key.is_empty());

        state.queue_payout_requests(
            &config,
            &[TreasuryQueuedPayoutRequest {
                payout_key: "accepted-work:cs336-a1:pubkey-a".to_string(),
                nostr_pubkey_hex: "pubkey-a".to_string(),
                amount_sats: 25,
                window_started_at_unix_ms: now_unix_ms,
                window_ends_at_unix_ms: now_unix_ms.saturating_add(1),
                classification: TreasuryPayoutClassification {
                    payout_class: TreasuryPayoutClass::AcceptedWork,
                    payout_basis: Some("homework_acceptance".to_string()),
                    assignment_id: Some("cs336-a1".to_string()),
                    ..TreasuryPayoutClassification::default()
                },
                queue_block_reason: None,
            }],
            now_unix_ms,
        );

        let prepared = state.prepare_due_payouts(&config, &online, now_unix_ms.saturating_add(1));
        assert_eq!(prepared.dispatch_plans.len(), 1);
        assert_eq!(prepared.dispatch_plans[0].amount_sats, 25);
        assert_eq!(prepared.dispatch_plans[0].payment_request, "provider:alice");
        let accepted_work = state
            .payout_records_by_key
            .get("accepted-work:cs336-a1:pubkey-a")
            .expect("accepted-work payout record");
        assert_eq!(accepted_work.status, "dispatching");
        assert!(accepted_work.classification.accepted_work());
        assert_eq!(state.payout_records_by_key.len(), 1);
    }

    #[test]
    fn accepted_work_wallet_reservation_ignores_confirmed_and_dispatched_spend() {
        let mut state = TreasuryState::default();
        let mut config = test_treasury_config();
        config.placeholder_payout_mode = TreasuryPlaceholderPayoutMode::Disabled;
        config.daily_budget_cap_sats = 1_000_000;
        state.wallet_balance_sats = 3;
        state.payout_targets_by_identity.insert(
            "pubkey-a".to_string(),
            super::RegisteredPayoutTarget {
                nostr_pubkey_hex: "pubkey-a".to_string(),
                source_session_id: "session-a".to_string(),
                payment_target_kind: String::new(),
                payment_target: String::new(),
                payment_target_capabilities: Vec::new(),
                pylon_payment_target_version: None,
                provider_target: "provider:alice".to_string(),
                bitcoin_address: None,
                registered_at_unix_ms: 10,
                last_verified_at_unix_ms: 10,
            },
        );

        let now_unix_ms = super::now_unix_ms();
        state.payout_records_by_key.insert(
            "accepted-work:already-confirmed".to_string(),
            TreasuryPayoutRecord {
                payout_key: "accepted-work:already-confirmed".to_string(),
                nostr_pubkey_hex: "pubkey-a".to_string(),
                payout_target: "provider:alice".to_string(),
                amount_sats: 10,
                status: "confirmed".to_string(),
                reason: None,
                payment_id: Some("payment-already-confirmed".to_string()),
                window_started_at_unix_ms: now_unix_ms.saturating_sub(1_000),
                window_ends_at_unix_ms: now_unix_ms.saturating_sub(999),
                created_at_unix_ms: now_unix_ms.saturating_sub(1_000),
                updated_at_unix_ms: now_unix_ms.saturating_sub(1_000),
                sellable_at_window_open: true,
                dispatch_receipt_recorded: true,
                confirm_receipt_recorded: true,
                fail_receipt_recorded: false,
                skip_receipt_recorded: false,
                counted_in_paid_total: true,
                classification: TreasuryPayoutClassification {
                    payout_class: TreasuryPayoutClass::AcceptedWork,
                    accepted_outcome_id: Some("accepted.already-confirmed".to_string()),
                    ..TreasuryPayoutClassification::default()
                },
            },
        );
        state.payout_records_by_key.insert(
            "placeholder:already-dispatched".to_string(),
            TreasuryPayoutRecord {
                payout_key: "placeholder:already-dispatched".to_string(),
                nostr_pubkey_hex: "pubkey-placeholder".to_string(),
                payout_target: "provider:placeholder".to_string(),
                amount_sats: 48,
                status: "dispatched".to_string(),
                reason: None,
                payment_id: Some("payment-already-dispatched".to_string()),
                window_started_at_unix_ms: now_unix_ms.saturating_sub(500),
                window_ends_at_unix_ms: now_unix_ms.saturating_sub(499),
                created_at_unix_ms: now_unix_ms.saturating_sub(500),
                updated_at_unix_ms: now_unix_ms.saturating_sub(500),
                sellable_at_window_open: true,
                dispatch_receipt_recorded: true,
                confirm_receipt_recorded: false,
                fail_receipt_recorded: false,
                skip_receipt_recorded: false,
                counted_in_paid_total: false,
                classification: TreasuryPayoutClassification::default(),
            },
        );
        state.queue_payout_requests(
            &config,
            &[TreasuryQueuedPayoutRequest {
                payout_key: "accepted-work:new-one-sat".to_string(),
                nostr_pubkey_hex: "pubkey-a".to_string(),
                amount_sats: 1,
                window_started_at_unix_ms: now_unix_ms,
                window_ends_at_unix_ms: now_unix_ms.saturating_add(1),
                classification: TreasuryPayoutClassification {
                    payout_class: TreasuryPayoutClass::AcceptedWork,
                    accepted_outcome_id: Some("accepted.new-one-sat".to_string()),
                    ..TreasuryPayoutClassification::default()
                },
                queue_block_reason: None,
            }],
            now_unix_ms,
        );

        let prepared = state.prepare_due_payouts(&config, &[], now_unix_ms.saturating_add(1));

        assert_eq!(prepared.dispatch_plans.len(), 1);
        assert_eq!(
            prepared.dispatch_plans[0].payout_key,
            "accepted-work:new-one-sat"
        );
        assert_eq!(prepared.dispatch_plans[0].amount_sats, 1);
        assert_eq!(prepared.dispatch_plans[0].payment_request, "provider:alice");
        assert_eq!(
            state
                .payout_records_by_key
                .get("accepted-work:new-one-sat")
                .map(|record| (record.status.as_str(), record.reason.as_deref())),
            Some(("dispatching", None))
        );
    }

    #[test]
    fn placeholder_payouts_dedupe_same_host_clients() {
        let mut state = TreasuryState::default();
        let config = test_treasury_config();
        for (nostr_pubkey_hex, payment_target) in [
            ("pubkey-a", "lno1pylonplaceholderalice"),
            ("pubkey-b", "lno1pylonplaceholderbob"),
        ] {
            state.payout_targets_by_identity.insert(
                nostr_pubkey_hex.to_string(),
                super::RegisteredPayoutTarget {
                    nostr_pubkey_hex: nostr_pubkey_hex.to_string(),
                    source_session_id: format!("session-{nostr_pubkey_hex}"),
                    payment_target_kind: "bolt12_offer".to_string(),
                    payment_target: payment_target.to_string(),
                    payment_target_capabilities: vec![
                        "bolt12_offer".to_string(),
                        "ldk_payment_target_v0_2".to_string(),
                    ],
                    pylon_payment_target_version: Some("pylon-payment-target/v0.2".to_string()),
                    provider_target: String::new(),
                    bitcoin_address: None,
                    registered_at_unix_ms: 10,
                    last_verified_at_unix_ms: 10,
                },
            );
        }

        let now_unix_ms = 1_800_000;
        let online = vec![
            OnlinePylonIdentity {
                client_version: Some("pylon-v0.1.1-rc1".to_string()),
                host_fingerprint: Some("sha256:host-alpha".to_string()),
                ..test_online_identity("pubkey-a")
            },
            OnlinePylonIdentity {
                client_version: Some("pylon-v0.1.1-rc1".to_string()),
                host_fingerprint: Some("sha256:host-alpha".to_string()),
                ..test_online_identity("pubkey-b")
            },
        ];
        state.observe_payout_eligibility(&config, &online, now_unix_ms);
        let stats = state.public_stats(&config, now_unix_ms);
        assert_eq!(stats.inference_ready_online_payout_targets, 2);
        assert_eq!(stats.eligible_online_payout_targets, 1);
        assert_eq!(stats.duplicate_host_placeholder_blocked_online_targets, 1);

        let prepared = state.prepare_due_payouts(&config, &online, now_unix_ms);
        assert_eq!(prepared.dispatch_plans.len(), 1);
        let duplicate_key = payout_window_key(
            payout_window_started_at_for_identity(
                now_unix_ms,
                config.payout_interval_ms(),
                "host:sha256:host-alpha",
            ),
            availability_identity_scope_key("pubkey-b").as_str(),
        );
        let duplicate_record = state
            .payout_records_by_key
            .get(duplicate_key.as_str())
            .expect("duplicate placeholder skip");
        assert_eq!(
            duplicate_record.reason.as_deref(),
            Some("duplicate_host_placeholder_readiness")
        );
        assert_eq!(
            duplicate_record.classification.payout_basis.as_deref(),
            Some("inference_ready")
        );
    }

    #[test]
    fn legacy_provider_payout_targets_require_pylon_v0_2_registration() {
        let mut state = TreasuryState::default();
        let config = test_treasury_config();
        state.payout_targets_by_identity.insert(
            "pubkey-a".to_string(),
            super::RegisteredPayoutTarget {
                nostr_pubkey_hex: "pubkey-a".to_string(),
                source_session_id: "session-pubkey-a".to_string(),
                payment_target_kind: String::new(),
                payment_target: String::new(),
                payment_target_capabilities: Vec::new(),
                pylon_payment_target_version: None,
                provider_target: "provider:alice".to_string(),
                bitcoin_address: None,
                registered_at_unix_ms: 10,
                last_verified_at_unix_ms: 10,
            },
        );

        let now_unix_ms = 1_800_000;
        state.observe_payout_eligibility(
            &config,
            &[OnlinePylonIdentity {
                client_version: Some("pylon-v0.1.13".to_string()),
                ..test_online_identity("pubkey-a")
            }],
            now_unix_ms,
        );

        let stats = state.public_stats(&config, now_unix_ms);
        assert_eq!(stats.registered_payout_identities, 1);
        assert_eq!(stats.ldk_payout_target_identities, 0);
        assert_eq!(stats.pylon_v0_2_registration_required_identities, 1);
        assert_eq!(stats.eligible_online_payout_targets, 0);
        assert_eq!(stats.inference_ready_online_payout_targets, 1);

        let prepared = state.prepare_due_payouts(
            &config,
            &[OnlinePylonIdentity {
                client_version: Some("pylon-v0.1.13".to_string()),
                ..test_online_identity("pubkey-a")
            }],
            now_unix_ms,
        );
        assert!(prepared.dispatch_plans.is_empty());
    }

    #[test]
    fn placeholder_payouts_dedupe_shared_payout_targets_across_hosts() {
        let mut state = TreasuryState::default();
        let config = test_treasury_config();
        for nostr_pubkey_hex in ["pubkey-a", "pubkey-b"] {
            state.payout_targets_by_identity.insert(
                nostr_pubkey_hex.to_string(),
                super::RegisteredPayoutTarget {
                    nostr_pubkey_hex: nostr_pubkey_hex.to_string(),
                    source_session_id: format!("session-{nostr_pubkey_hex}"),
                    payment_target_kind: "bolt12_offer".to_string(),
                    payment_target: "lno1sharedpylon".to_string(),
                    payment_target_capabilities: vec![
                        "bolt12_offer".to_string(),
                        "ldk_payment_target_v0_2".to_string(),
                    ],
                    pylon_payment_target_version: Some("pylon-payment-target/v0.2".to_string()),
                    provider_target: String::new(),
                    bitcoin_address: None,
                    registered_at_unix_ms: 10,
                    last_verified_at_unix_ms: 10,
                },
            );
        }

        let now_unix_ms = 1_800_000;
        let online = vec![
            OnlinePylonIdentity {
                client_version: Some("pylon-v0.1.13".to_string()),
                host_fingerprint: Some("sha256:host-alpha".to_string()),
                ..test_online_identity("pubkey-a")
            },
            OnlinePylonIdentity {
                client_version: Some("pylon-v0.1.13".to_string()),
                host_fingerprint: Some("sha256:host-beta".to_string()),
                ..test_online_identity("pubkey-b")
            },
        ];

        let prepared = state.prepare_due_payouts(&config, &online, now_unix_ms);
        assert_eq!(prepared.dispatch_plans.len(), 1);

        let duplicate_key = payout_window_key(
            payout_window_started_at_for_identity(
                now_unix_ms,
                config.payout_interval_ms(),
                "host:sha256:host-beta",
            ),
            availability_identity_scope_key("pubkey-b").as_str(),
        );
        let duplicate_record = state
            .payout_records_by_key
            .get(duplicate_key.as_str())
            .expect("duplicate payout-target skip");
        assert_eq!(
            duplicate_record.reason.as_deref(),
            Some("duplicate_payout_target_placeholder_readiness")
        );
    }

    #[test]
    fn placeholder_payouts_allow_same_host_when_host_dedupe_disabled() {
        let mut state = TreasuryState::default();
        let mut config = test_treasury_config();
        config.dedupe_placeholder_hosts = false;
        for (nostr_pubkey_hex, payment_target) in [
            ("pubkey-a", "lno1pylonplaceholderalice"),
            ("pubkey-b", "lno1pylonplaceholderbob"),
        ] {
            state.payout_targets_by_identity.insert(
                nostr_pubkey_hex.to_string(),
                super::RegisteredPayoutTarget {
                    nostr_pubkey_hex: nostr_pubkey_hex.to_string(),
                    source_session_id: format!("session-{nostr_pubkey_hex}"),
                    payment_target_kind: "bolt12_offer".to_string(),
                    payment_target: payment_target.to_string(),
                    payment_target_capabilities: vec![
                        "bolt12_offer".to_string(),
                        "ldk_payment_target_v0_2".to_string(),
                    ],
                    pylon_payment_target_version: Some("pylon-payment-target/v0.2".to_string()),
                    provider_target: String::new(),
                    bitcoin_address: None,
                    registered_at_unix_ms: 10,
                    last_verified_at_unix_ms: 10,
                },
            );
        }

        let now_unix_ms = 1_800_000;
        let online = vec![
            OnlinePylonIdentity {
                client_version: Some("pylon-v0.1.13".to_string()),
                host_fingerprint: Some("sha256:host-alpha".to_string()),
                ..test_online_identity("pubkey-a")
            },
            OnlinePylonIdentity {
                client_version: Some("pylon-v0.1.13".to_string()),
                host_fingerprint: Some("sha256:host-alpha".to_string()),
                ..test_online_identity("pubkey-b")
            },
        ];

        state.observe_payout_eligibility(&config, &online, now_unix_ms);
        let stats = state.public_stats(&config, now_unix_ms);
        assert_eq!(stats.eligible_online_payout_targets, 2);
        assert_eq!(stats.duplicate_host_placeholder_blocked_online_targets, 0);

        let prepared = state.prepare_due_payouts(&config, &online, now_unix_ms);
        assert_eq!(prepared.dispatch_plans.len(), 2);
        assert!(state.payout_records_by_key.values().all(|record| {
            record.reason.as_deref() != Some("duplicate_host_placeholder_readiness")
        }));
    }

    #[test]
    fn availability_stipend_gate_reason_blocks_ineligible_identity() {
        let mut state = TreasuryState::default();
        let config = test_treasury_config();
        state.payout_targets_by_identity.insert(
            "pubkey-a".to_string(),
            super::RegisteredPayoutTarget {
                nostr_pubkey_hex: "pubkey-a".to_string(),
                source_session_id: "session-a".to_string(),
                payment_target_kind: String::new(),
                payment_target: String::new(),
                payment_target_capabilities: Vec::new(),
                pylon_payment_target_version: None,
                provider_target: "provider:alice".to_string(),
                bitcoin_address: None,
                registered_at_unix_ms: 10,
                last_verified_at_unix_ms: 10,
            },
        );

        let now_unix_ms = 1_800_000;
        let online = vec![OnlinePylonIdentity {
            availability_stipend_eligible: false,
            availability_stipend_gate_reason: Some("missing_worker_role".to_string()),
            ..test_online_identity("pubkey-a")
        }];
        state.observe_payout_eligibility(&config, &online, now_unix_ms);
        let prepared = state.prepare_due_payouts(&config, &online, now_unix_ms);

        assert!(prepared.dispatch_plans.is_empty());
        assert_eq!(
            state
                .public_stats(&config, now_unix_ms)
                .eligible_online_payout_targets,
            0
        );

        let payout_key = payout_window_key(
            payout_window_started_at_for_identity(
                now_unix_ms,
                config.payout_interval_ms(),
                "pubkey-a",
            ),
            availability_identity_scope_key("pubkey-a").as_str(),
        );
        let record = state
            .payout_records_by_key
            .get(payout_key.as_str())
            .expect("availability gate skip");
        assert_eq!(record.reason.as_deref(), Some("missing_worker_role"));
    }

    #[test]
    fn payout_windows_are_staggered_per_beneficiary() {
        let interval_ms = test_treasury_config().payout_interval_ms();
        let beneficiary_a = "payout_target:provider:alice";
        let beneficiary_b = "payout_target:provider:bob";
        let phase_a = payout_phase_offset_ms(beneficiary_a, interval_ms);
        let phase_b = payout_phase_offset_ms(beneficiary_b, interval_ms);

        assert_ne!(phase_a, phase_b);

        let now_unix_ms = 1_800_000;
        let window_a =
            payout_window_started_at_for_identity(now_unix_ms, interval_ms, beneficiary_a);
        let window_b =
            payout_window_started_at_for_identity(now_unix_ms, interval_ms, beneficiary_b);

        assert_ne!(window_a, window_b);
        assert_eq!(window_a % interval_ms, phase_a);
        assert_eq!(window_b % interval_ms, phase_b);
    }

    #[test]
    fn payout_preparation_uses_beneficiary_phased_windows() {
        let mut state = TreasuryState::default();
        let config = test_treasury_config();
        for (nostr_pubkey_hex, provider_target) in
            [("pubkey-a", "provider:alice"), ("pubkey-b", "provider:bob")]
        {
            state.payout_targets_by_identity.insert(
                nostr_pubkey_hex.to_string(),
                super::RegisteredPayoutTarget {
                    nostr_pubkey_hex: nostr_pubkey_hex.to_string(),
                    source_session_id: format!("session-{nostr_pubkey_hex}"),
                    payment_target_kind: String::new(),
                    payment_target: String::new(),
                    payment_target_capabilities: Vec::new(),
                    pylon_payment_target_version: None,
                    provider_target: provider_target.to_string(),
                    bitcoin_address: None,
                    registered_at_unix_ms: 10,
                    last_verified_at_unix_ms: 10,
                },
            );
        }

        let online = vec![
            test_online_identity("pubkey-a"),
            test_online_identity("pubkey-b"),
        ];
        let now_unix_ms = 1_800_000;
        let prepared = state.prepare_due_payouts(&config, &online, now_unix_ms);

        assert!(prepared.receipt_events.is_empty());
        assert_eq!(prepared.dispatch_plans.len(), 2);
        assert_ne!(
            prepared.dispatch_plans[0].payout_key,
            prepared.dispatch_plans[1].payout_key
        );

        let expected_window_a = payout_window_started_at_for_identity(
            now_unix_ms,
            config.payout_interval_ms(),
            "payout_target:provider:alice",
        );
        let expected_window_b = payout_window_started_at_for_identity(
            now_unix_ms,
            config.payout_interval_ms(),
            "payout_target:provider:bob",
        );
        assert!(prepared.dispatch_plans.iter().any(|plan| {
            plan.payout_key
                == payout_window_key(
                    expected_window_a,
                    availability_beneficiary_scope_key("payout_target:provider:alice").as_str(),
                )
        }));
        assert!(prepared.dispatch_plans.iter().any(|plan| {
            plan.payout_key
                == payout_window_key(
                    expected_window_b,
                    availability_beneficiary_scope_key("payout_target:provider:bob").as_str(),
                )
        }));
    }

    #[test]
    fn payout_preparation_reconciles_missed_windows_within_horizon() {
        let mut state = TreasuryState::default();
        let config = test_treasury_config();
        state.payout_targets_by_identity.insert(
            "pubkey-a".to_string(),
            super::RegisteredPayoutTarget {
                nostr_pubkey_hex: "pubkey-a".to_string(),
                source_session_id: "session-a".to_string(),
                payment_target_kind: String::new(),
                payment_target: String::new(),
                payment_target_capabilities: Vec::new(),
                pylon_payment_target_version: None,
                provider_target: "provider:alice".to_string(),
                bitcoin_address: None,
                registered_at_unix_ms: 10,
                last_verified_at_unix_ms: 10,
            },
        );

        let interval_ms = config.payout_interval_ms();
        let now_unix_ms = 1_800_000;
        state.last_payout_reconciliation_at_unix_ms = Some(now_unix_ms - (interval_ms * 3));

        let prepared =
            state.prepare_due_payouts(&config, &[test_online_identity("pubkey-a")], now_unix_ms);

        assert_eq!(prepared.dispatch_plans.len(), 4);
        assert!(prepared.reconciliation_degraded_reason.is_none());
    }

    #[test]
    fn payout_preparation_clamps_reconciliation_to_horizon() {
        let mut state = TreasuryState::default();
        let mut config = test_treasury_config();
        config.reconciliation_horizon_seconds = 120;
        state.payout_targets_by_identity.insert(
            "pubkey-a".to_string(),
            super::RegisteredPayoutTarget {
                nostr_pubkey_hex: "pubkey-a".to_string(),
                source_session_id: "session-a".to_string(),
                payment_target_kind: String::new(),
                payment_target: String::new(),
                payment_target_capabilities: Vec::new(),
                pylon_payment_target_version: None,
                provider_target: "provider:alice".to_string(),
                bitcoin_address: None,
                registered_at_unix_ms: 10,
                last_verified_at_unix_ms: 10,
            },
        );

        let interval_ms = config.payout_interval_ms();
        let now_unix_ms = 1_800_000;
        state.last_payout_reconciliation_at_unix_ms = Some(now_unix_ms - (interval_ms * 10));

        let prepared =
            state.prepare_due_payouts(&config, &[test_online_identity("pubkey-a")], now_unix_ms);

        assert_eq!(prepared.dispatch_plans.len(), 3);
        assert_eq!(
            prepared.reconciliation_degraded_reason.as_deref(),
            Some("reconciliation_horizon_exceeded:1200000")
        );
    }

    #[test]
    fn payout_target_identity_rows_aggregate_payout_records_once() {
        let mut state = TreasuryState::default();

        for target_index in 0..80 {
            let pubkey = format!("pubkey-{target_index:03}");
            state.payout_targets_by_identity.insert(
                pubkey.clone(),
                super::RegisteredPayoutTarget {
                    nostr_pubkey_hex: pubkey.clone(),
                    source_session_id: format!("session-{target_index:03}"),
                    payment_target_kind: String::new(),
                    payment_target: String::new(),
                    payment_target_capabilities: Vec::new(),
                    pylon_payment_target_version: None,
                    provider_target: format!("provider:{target_index:03}"),
                    bitcoin_address: None,
                    registered_at_unix_ms: 1_000u64.saturating_sub(target_index),
                    last_verified_at_unix_ms: 10_000u64.saturating_sub(target_index),
                },
            );

            for record_index in 0..20 {
                let payout_key = format!("{pubkey}:record-{record_index:02}");
                state.payout_records_by_key.insert(
                    payout_key.clone(),
                    TreasuryPayoutRecord {
                        payout_key,
                        nostr_pubkey_hex: pubkey.clone(),
                        payout_target: format!("provider:{target_index:03}"),
                        amount_sats: 10 + record_index,
                        status: if record_index % 2 == 0 {
                            "confirmed".to_string()
                        } else {
                            "failed".to_string()
                        },
                        reason: None,
                        payment_id: Some(format!("payment-{target_index:03}-{record_index:02}")),
                        window_started_at_unix_ms: 20_000 + record_index,
                        window_ends_at_unix_ms: 21_000 + record_index,
                        created_at_unix_ms: 20_000 + record_index,
                        updated_at_unix_ms: 20_000 + record_index,
                        sellable_at_window_open: true,
                        dispatch_receipt_recorded: true,
                        confirm_receipt_recorded: record_index % 2 == 0,
                        fail_receipt_recorded: record_index % 2 != 0,
                        skip_receipt_recorded: false,
                        counted_in_paid_total: record_index % 2 == 0,
                        classification: if record_index % 4 == 0 {
                            TreasuryPayoutClassification {
                                payout_class: TreasuryPayoutClass::AcceptedWork,
                                payout_basis: Some("validator_verdict".to_string()),
                                work_class: Some("validation_replay".to_string()),
                                progress_class: Some("model_progress".to_string()),
                                accepted_outcome_id: Some(format!(
                                    "accepted-{target_index:03}-{record_index:02}"
                                )),
                                training_run_id: Some("run.aggregate.test".to_string()),
                                window_id: Some(format!("window-{record_index:02}")),
                                contribution_id: Some(format!(
                                    "contrib-{target_index:03}-{record_index:02}"
                                )),
                                assignment_id: Some(format!(
                                    "assignment-{target_index:03}-{record_index:02}"
                                )),
                                share_bps: Some(10_000),
                                weight_basis: Some("tokens".to_string()),
                                weight_value: Some(1_024),
                                weak_device_bearing: false,
                                progress_bearing: true,
                            }
                        } else {
                            TreasuryPayoutClassification::default()
                        },
                    },
                );
            }
        }

        state.payout_records_by_key.insert(
            "unregistered-record".to_string(),
            TreasuryPayoutRecord {
                payout_key: "unregistered-record".to_string(),
                nostr_pubkey_hex: "pubkey-unregistered".to_string(),
                payout_target: "provider:unregistered".to_string(),
                amount_sats: 999,
                status: "confirmed".to_string(),
                reason: None,
                payment_id: Some("payment-unregistered".to_string()),
                window_started_at_unix_ms: 30_000,
                window_ends_at_unix_ms: 31_000,
                created_at_unix_ms: 30_000,
                updated_at_unix_ms: 30_000,
                sellable_at_window_open: true,
                dispatch_receipt_recorded: true,
                confirm_receipt_recorded: true,
                fail_receipt_recorded: false,
                skip_receipt_recorded: false,
                counted_in_paid_total: true,
                classification: TreasuryPayoutClassification::default(),
            },
        );

        let rows = state.payout_target_identity_rows();

        assert_eq!(rows.len(), super::TREASURY_STATUS_PAYOUT_TARGET_ROW_LIMIT);
        assert_eq!(rows[0].nostr_pubkey_hex, "pubkey-000");
        assert_eq!(rows[0].payout_record_count, 20);
        assert_eq!(rows[0].confirmed_payout_count, 10);
        assert_eq!(rows[0].confirmed_payout_sats, 190);
        assert_eq!(rows[0].confirmed_accepted_work_payout_sats, 90);
        assert_eq!(rows[0].last_payout_at_unix_ms, Some(20_019));
        assert!(
            !rows
                .iter()
                .any(|row| row.nostr_pubkey_hex == "pubkey-unregistered")
        );
    }

    #[test]
    fn wallet_snapshot_updates_receives_and_confirmed_payout_totals() {
        let mut state = TreasuryState::default();
        let config = test_treasury_config();
        let now_unix_ms = super::now_unix_ms();
        let window_started_at_unix_ms =
            payout_window_started_at(now_unix_ms, config.payout_interval_ms());
        let payout_key = format!("{window_started_at_unix_ms}:pubkey-a");
        state.payout_records_by_key.insert(
            payout_key.clone(),
            super::TreasuryPayoutRecord {
                payout_key: payout_key.clone(),
                nostr_pubkey_hex: "pubkey-a".to_string(),
                payout_target: "provider:alice".to_string(),
                amount_sats: 120,
                status: "dispatched".to_string(),
                reason: None,
                payment_id: Some("payment-send-001".to_string()),
                window_started_at_unix_ms,
                window_ends_at_unix_ms: window_started_at_unix_ms + config.payout_interval_ms(),
                created_at_unix_ms: now_unix_ms,
                updated_at_unix_ms: now_unix_ms,
                sellable_at_window_open: true,
                dispatch_receipt_recorded: true,
                confirm_receipt_recorded: false,
                fail_receipt_recorded: false,
                skip_receipt_recorded: false,
                counted_in_paid_total: false,
                classification: TreasuryPayoutClassification::default(),
            },
        );

        let receipts = state.apply_wallet_snapshot(
            &TreasuryWalletSnapshot {
                runtime_status: "connected".to_string(),
                runtime_detail: None,
                wallet_hydration_mode: None,
                wallet_payment_scan_mode: None,
                balance_sats: 880,
                total_onchain_balance_sats: 880,
                spendable_onchain_balance_sats: 880,
                lightning_balance_sats: 0,
                payments: vec![
                    PaymentSummary {
                        id: "payment-receive-001".to_string(),
                        direction: "receive".to_string(),
                        status: "completed".to_string(),
                        amount_sats: 500,
                        fees_sats: 0,
                        timestamp: now_unix_ms.saturating_div(1_000).saturating_sub(1),
                        method: "provider".to_string(),
                        description: Some("fund".to_string()),
                        invoice: None,
                        destination_pubkey: None,
                        payment_hash: None,
                        htlc_status: None,
                        htlc_expiry_epoch_seconds: None,
                        status_detail: None,
                    },
                    PaymentSummary {
                        id: "payment-send-001".to_string(),
                        direction: "send".to_string(),
                        status: "completed".to_string(),
                        amount_sats: 120,
                        fees_sats: 1,
                        timestamp: now_unix_ms.saturating_div(1_000),
                        method: "provider".to_string(),
                        description: None,
                        invoice: Some("provider:alice".to_string()),
                        destination_pubkey: None,
                        payment_hash: None,
                        htlc_status: None,
                        htlc_expiry_epoch_seconds: None,
                        status_detail: None,
                    },
                ],
            },
            now_unix_ms,
        );

        assert_eq!(state.wallet_balance_sats, 880);
        assert_eq!(state.payout_sats_paid_total, 120);
        assert_eq!(state.funding_receives_by_payment_id.len(), 1);
        assert_eq!(receipts.len(), 1);
        let stats: TreasuryPublicStats = state.public_stats(&config, now_unix_ms);
        assert_eq!(stats.payout_sats_paid_total, 120);
        assert_eq!(stats.payout_sats_paid_24h, 120);
        assert_eq!(stats.payouts_confirmed_24h, 1);
    }

    #[test]
    fn wallet_snapshot_recovers_timed_out_send_without_payment_id() {
        let mut state = TreasuryState::default();
        let config = test_treasury_config();
        let now_unix_ms = 1_776_028_000_000u64;
        let created_at_unix_ms = now_unix_ms.saturating_sub(180_000);
        let payout_key = "window-a:pubkey-a".to_string();
        state.payout_records_by_key.insert(
            payout_key.clone(),
            TreasuryPayoutRecord {
                payout_key: payout_key.clone(),
                nostr_pubkey_hex: "pubkey-a".to_string(),
                payout_target: "provider:alice".to_string(),
                amount_sats: 50,
                status: "failed".to_string(),
                reason: Some("wallet_send_timeout:60000".to_string()),
                payment_id: None,
                window_started_at_unix_ms: created_at_unix_ms,
                window_ends_at_unix_ms: created_at_unix_ms.saturating_add(60_000),
                created_at_unix_ms,
                updated_at_unix_ms: created_at_unix_ms.saturating_add(60_000),
                sellable_at_window_open: true,
                dispatch_receipt_recorded: false,
                confirm_receipt_recorded: false,
                fail_receipt_recorded: true,
                skip_receipt_recorded: false,
                counted_in_paid_total: false,
                classification: TreasuryPayoutClassification {
                    payout_class: TreasuryPayoutClass::AcceptedWork,
                    payout_basis: Some("homework_acceptance".to_string()),
                    training_run_id: Some("run.cs336.a1.demo".to_string()),
                    assignment_id: Some("assign-timeout-recovery".to_string()),
                    ..TreasuryPayoutClassification::default()
                },
            },
        );

        let payment_timestamp = created_at_unix_ms
            .saturating_add(120_000)
            .saturating_div(1_000);
        let receipts = state.apply_wallet_snapshot(
            &TreasuryWalletSnapshot {
                runtime_status: "connected".to_string(),
                runtime_detail: None,
                wallet_hydration_mode: None,
                wallet_payment_scan_mode: None,
                balance_sats: 830,
                total_onchain_balance_sats: 830,
                spendable_onchain_balance_sats: 830,
                lightning_balance_sats: 0,
                payments: vec![PaymentSummary {
                    id: "payment-send-recovered".to_string(),
                    direction: "send".to_string(),
                    status: "completed".to_string(),
                    amount_sats: 50,
                    fees_sats: 0,
                    timestamp: payment_timestamp,
                    method: "provider".to_string(),
                    description: None,
                    invoice: None,
                    destination_pubkey: None,
                    payment_hash: None,
                    htlc_status: None,
                    htlc_expiry_epoch_seconds: None,
                    status_detail: None,
                }],
            },
            now_unix_ms,
        );

        let record = state
            .payout_records_by_key
            .get(&payout_key)
            .expect("recovered payout record");
        assert_eq!(record.status, "confirmed");
        assert_eq!(record.payment_id.as_deref(), Some("payment-send-recovered"));
        assert!(record.dispatch_receipt_recorded);
        assert!(record.confirm_receipt_recorded);
        assert!(record.counted_in_paid_total);
        assert_eq!(state.payout_sats_paid_total, 50);
        assert_eq!(
            state.last_dispatch_at_unix_ms,
            Some(payment_timestamp.saturating_mul(1_000))
        );
        assert_eq!(
            state.last_confirmed_payout_at_unix_ms,
            Some(payment_timestamp.saturating_mul(1_000))
        );
        assert_eq!(receipts.len(), 2);
        assert_eq!(receipts[0].receipt_type, "treasury.payout.dispatched");
        assert_eq!(receipts[1].receipt_type, "treasury.payout.confirmed");

        let stats: TreasuryPublicStats = state.public_stats(&config, now_unix_ms);
        assert_eq!(stats.payout_sats_paid_total, 50);
        assert_eq!(stats.payouts_confirmed_24h, 1);
    }

    #[test]
    fn wallet_snapshot_does_not_recover_send_outside_match_window() {
        let mut state = TreasuryState::default();
        let now_unix_ms = 1_776_028_000_000u64;
        let created_at_unix_ms = now_unix_ms.saturating_sub(180_000);
        let payout_key = "window-a:pubkey-a".to_string();
        state.payout_records_by_key.insert(
            payout_key.clone(),
            TreasuryPayoutRecord {
                payout_key: payout_key.clone(),
                nostr_pubkey_hex: "pubkey-a".to_string(),
                payout_target: "provider:alice".to_string(),
                amount_sats: 50,
                status: "failed".to_string(),
                reason: Some("wallet_send_timeout:60000".to_string()),
                payment_id: None,
                window_started_at_unix_ms: created_at_unix_ms,
                window_ends_at_unix_ms: created_at_unix_ms.saturating_add(60_000),
                created_at_unix_ms,
                updated_at_unix_ms: created_at_unix_ms.saturating_add(60_000),
                sellable_at_window_open: true,
                dispatch_receipt_recorded: false,
                confirm_receipt_recorded: false,
                fail_receipt_recorded: true,
                skip_receipt_recorded: false,
                counted_in_paid_total: false,
                classification: TreasuryPayoutClassification {
                    payout_class: TreasuryPayoutClass::AcceptedWork,
                    payout_basis: Some("homework_acceptance".to_string()),
                    training_run_id: Some("run.cs336.a1.demo".to_string()),
                    assignment_id: Some("assign-timeout-unmatched".to_string()),
                    ..TreasuryPayoutClassification::default()
                },
            },
        );

        let payment_timestamp = created_at_unix_ms
            .saturating_add(super::TREASURY_ORPHAN_SEND_PAYMENT_MATCH_WINDOW_MS)
            .saturating_add(120_000)
            .saturating_div(1_000);
        let receipts = state.apply_wallet_snapshot(
            &TreasuryWalletSnapshot {
                runtime_status: "connected".to_string(),
                runtime_detail: None,
                wallet_hydration_mode: None,
                wallet_payment_scan_mode: None,
                balance_sats: 830,
                total_onchain_balance_sats: 830,
                spendable_onchain_balance_sats: 830,
                lightning_balance_sats: 0,
                payments: vec![PaymentSummary {
                    id: "payment-send-unmatched".to_string(),
                    direction: "send".to_string(),
                    status: "completed".to_string(),
                    amount_sats: 50,
                    fees_sats: 0,
                    timestamp: payment_timestamp,
                    method: "provider".to_string(),
                    description: None,
                    invoice: None,
                    destination_pubkey: None,
                    payment_hash: None,
                    htlc_status: None,
                    htlc_expiry_epoch_seconds: None,
                    status_detail: None,
                }],
            },
            now_unix_ms,
        );

        let record = state
            .payout_records_by_key
            .get(&payout_key)
            .expect("timeout payout record");
        assert_eq!(record.status, "failed");
        assert!(record.payment_id.is_none());
        assert_eq!(state.payout_sats_paid_total, 0);
        assert!(receipts.is_empty());
    }

    #[test]
    fn public_stats_keep_unconfirmed_dispatched_sats_out_of_paid_totals() {
        let mut state = TreasuryState::default();
        let config = test_treasury_config();
        let now_unix_ms = super::now_unix_ms();
        let window_started_at_unix_ms =
            payout_window_started_at(now_unix_ms, config.payout_interval_ms());
        let payout_key = format!("{window_started_at_unix_ms}:pubkey-b");
        state.payout_sats_paid_total = 120;
        state.payout_records_by_key.insert(
            payout_key.clone(),
            super::TreasuryPayoutRecord {
                payout_key,
                nostr_pubkey_hex: "pubkey-b".to_string(),
                payout_target: "provider:bob".to_string(),
                amount_sats: 2,
                status: "dispatched".to_string(),
                reason: None,
                payment_id: Some("payment-send-002".to_string()),
                window_started_at_unix_ms,
                window_ends_at_unix_ms: window_started_at_unix_ms + config.payout_interval_ms(),
                created_at_unix_ms: now_unix_ms,
                updated_at_unix_ms: now_unix_ms,
                sellable_at_window_open: true,
                dispatch_receipt_recorded: true,
                confirm_receipt_recorded: false,
                fail_receipt_recorded: false,
                skip_receipt_recorded: false,
                counted_in_paid_total: false,
                classification: TreasuryPayoutClassification::default(),
            },
        );

        let stats: TreasuryPublicStats = state.public_stats(&config, now_unix_ms);
        assert_eq!(stats.payout_sats_paid_total, 120);
        assert_eq!(stats.payout_sats_paid_24h, 0);
        assert_eq!(stats.payout_sats_in_flight_total, 2);
        assert_eq!(stats.payout_sats_in_flight_24h, 2);
        assert_eq!(stats.payouts_dispatched_24h, 1);
        assert_eq!(stats.payouts_confirmed_24h, 0);
    }

    #[test]
    fn queued_accepted_work_payouts_dispatch_without_online_presence_and_split_public_totals() {
        let mut state = TreasuryState::default();
        let mut config = test_treasury_config();
        config.enabled = true;
        config.payout_sats_per_window = 120;
        let now_unix_ms = super::now_unix_ms();

        state.payout_targets_by_identity.insert(
            "pubkey-replay".to_string(),
            super::RegisteredPayoutTarget {
                nostr_pubkey_hex: "pubkey-replay".to_string(),
                source_session_id: "session-replay".to_string(),
                payment_target_kind: String::new(),
                payment_target: String::new(),
                payment_target_capabilities: Vec::new(),
                pylon_payment_target_version: None,
                provider_target: "provider:replay".to_string(),
                bitcoin_address: None,
                registered_at_unix_ms: now_unix_ms.saturating_sub(10),
                last_verified_at_unix_ms: now_unix_ms.saturating_sub(10),
            },
        );
        state.queue_payout_requests(
            &config,
            &[TreasuryQueuedPayoutRequest {
                payout_key: "accepted_work:closeout-001:contrib-001:pubkey-replay".to_string(),
                nostr_pubkey_hex: "pubkey-replay".to_string(),
                amount_sats: 120,
                window_started_at_unix_ms: now_unix_ms,
                window_ends_at_unix_ms: now_unix_ms,
                classification: TreasuryPayoutClassification {
                    payout_class: TreasuryPayoutClass::AcceptedWork,
                    payout_basis: Some("validator_verdict".to_string()),
                    work_class: Some("validation_replay".to_string()),
                    progress_class: Some("participation_only".to_string()),
                    accepted_outcome_id: Some(
                        "accepted.training_window.window.weak.0001".to_string(),
                    ),
                    training_run_id: Some("run.weak.validation".to_string()),
                    window_id: Some("window.weak.0001".to_string()),
                    contribution_id: Some("contrib-001".to_string()),
                    assignment_id: Some("assign-001".to_string()),
                    share_bps: Some(10_000),
                    weight_basis: None,
                    weight_value: None,
                    weak_device_bearing: true,
                    progress_bearing: false,
                },
                queue_block_reason: None,
            }],
            now_unix_ms,
        );

        let prepared = state.prepare_due_payouts(&config, &[], now_unix_ms.saturating_add(1));
        assert_eq!(prepared.dispatch_plans.len(), 1);
        assert_eq!(
            prepared.dispatch_plans[0].payment_request,
            "provider:replay"
        );
        assert_eq!(prepared.dispatch_plans[0].amount_sats, 120);
        assert_eq!(
            state
                .payout_records_by_key
                .get("accepted_work:closeout-001:contrib-001:pubkey-replay")
                .map(|record| record.status.as_str()),
            Some("dispatching")
        );

        let dispatch_receipts = state.apply_dispatch_outcome(
            TreasuryDispatchOutcome::Dispatched {
                payout_key: "accepted_work:closeout-001:contrib-001:pubkey-replay".to_string(),
                payment_id: "payment-replay-001".to_string(),
                terminal_event_state: Some("completed".to_string()),
            },
            now_unix_ms.saturating_add(2),
        );
        assert_eq!(dispatch_receipts.len(), 1);
        state.refresh_public_snapshot(&config, now_unix_ms.saturating_add(2));
        let stats = state.public_stats(&config, now_unix_ms.saturating_add(2));
        assert_eq!(stats.payout_sats_paid_total, 120);
        assert_eq!(stats.accepted_work_payout_sats_paid_total, 120);
        assert_eq!(stats.weak_device_accepted_work_payout_sats_paid_total, 120);
        assert_eq!(stats.placeholder_payout_sats_paid_total, 0);
        assert_eq!(stats.beta_bonus_payout_sats_paid_total, 0);
        assert_eq!(stats.payouts_dispatched_24h, 1);
        let status = state.status_response(&config, now_unix_ms.saturating_add(2));
        assert_eq!(
            status.training_payout_ledger_summary.reconciliation_status,
            "pending"
        );
        assert_eq!(
            status
                .training_payout_ledger_summary
                .accepted_work_pending_payout_count,
            1
        );
        assert_eq!(status.payout_target_identities.len(), 1);
        assert_eq!(
            status.payout_target_identities[0].nostr_pubkey_hex,
            "pubkey-replay"
        );
        assert_eq!(status.payout_target_identities[0].payout_record_count, 1);
        assert_eq!(status.payout_target_identities[0].confirmed_payout_count, 0);
        assert_eq!(status.recent_training_payouts.len(), 1);
        assert_eq!(
            status.recent_training_payouts[0].reconciliation_status,
            "pending_confirmation"
        );
        assert_eq!(
            status.recent_training_payouts[0]
                .classification
                .training_run_id
                .as_deref(),
            Some("run.weak.validation")
        );
        assert_eq!(
            dispatch_receipts[0]
                .context
                .attributes
                .get("accepted_outcome_id")
                .map(String::as_str),
            Some("accepted.training_window.window.weak.0001")
        );
        assert_eq!(
            dispatch_receipts[0]
                .context
                .attributes
                .get("payout_class")
                .map(String::as_str),
            Some("accepted_work")
        );
    }

    #[test]
    fn queued_accepted_work_payouts_dispatch_when_placeholder_budget_is_zero() {
        let mut state = TreasuryState::default();
        let mut config = test_treasury_config();
        config.enabled = true;
        config.payout_sats_per_window = 0;
        let now_unix_ms = super::now_unix_ms();

        state.payout_targets_by_identity.insert(
            "pubkey-homework".to_string(),
            super::RegisteredPayoutTarget {
                nostr_pubkey_hex: "pubkey-homework".to_string(),
                source_session_id: "session-homework".to_string(),
                payment_target_kind: String::new(),
                payment_target: String::new(),
                payment_target_capabilities: Vec::new(),
                pylon_payment_target_version: None,
                provider_target: "provider:homework".to_string(),
                bitcoin_address: None,
                registered_at_unix_ms: now_unix_ms.saturating_sub(10),
                last_verified_at_unix_ms: now_unix_ms.saturating_sub(10),
            },
        );
        state.queue_payout_requests(
            &config,
            &[TreasuryQueuedPayoutRequest {
                payout_key: "accepted_work:closeout-002:contrib-002:pubkey-homework".to_string(),
                nostr_pubkey_hex: "pubkey-homework".to_string(),
                amount_sats: 55,
                window_started_at_unix_ms: now_unix_ms,
                window_ends_at_unix_ms: now_unix_ms,
                classification: TreasuryPayoutClassification {
                    payout_class: TreasuryPayoutClass::AcceptedWork,
                    payout_basis: Some("aggregation_weight".to_string()),
                    work_class: Some("small_model_local_training".to_string()),
                    progress_class: Some("model_update".to_string()),
                    accepted_outcome_id: Some(
                        "accepted.training_window.window.homework.0001".to_string(),
                    ),
                    training_run_id: Some("run.homework".to_string()),
                    window_id: Some("window.homework.0001".to_string()),
                    contribution_id: Some("contrib-002".to_string()),
                    assignment_id: Some("assign-002".to_string()),
                    share_bps: Some(10_000),
                    weight_basis: Some("tokens".to_string()),
                    weight_value: Some(131_072),
                    weak_device_bearing: false,
                    progress_bearing: true,
                },
                queue_block_reason: None,
            }],
            now_unix_ms,
        );

        let prepared = state.prepare_due_payouts(&config, &[], now_unix_ms.saturating_add(1));
        assert_eq!(prepared.dispatch_plans.len(), 1);
        assert_eq!(
            prepared.dispatch_plans[0].payment_request,
            "provider:homework"
        );
        assert_eq!(prepared.dispatch_plans[0].amount_sats, 55);
        assert_eq!(
            state
                .payout_records_by_key
                .get("accepted_work:closeout-002:contrib-002:pubkey-homework")
                .map(|record| record.status.as_str()),
            Some("dispatching")
        );
    }

    #[test]
    fn availability_budget_exhaustion_does_not_block_accepted_work_dispatch() {
        let mut state = TreasuryState::default();
        let mut config = test_treasury_config();
        config.enabled = true;
        config.payout_sats_per_window = 25;
        config.daily_budget_cap_sats = 25;
        config.accepted_work_daily_budget_cap_sats = 1_000;
        state.wallet_balance_sats = 1_000_000;
        let now_unix_ms = super::now_unix_ms();

        state.payout_records_by_key.insert(
            "placeholder:already-paid".to_string(),
            TreasuryPayoutRecord {
                payout_key: "placeholder:already-paid".to_string(),
                nostr_pubkey_hex: "pubkey-placeholder".to_string(),
                payout_target: "provider:placeholder".to_string(),
                amount_sats: 25,
                status: "confirmed".to_string(),
                reason: None,
                payment_id: Some("payment-placeholder".to_string()),
                window_started_at_unix_ms: now_unix_ms.saturating_sub(60_000),
                window_ends_at_unix_ms: now_unix_ms.saturating_sub(30_000),
                created_at_unix_ms: now_unix_ms.saturating_sub(60_000),
                updated_at_unix_ms: now_unix_ms.saturating_sub(30_000),
                sellable_at_window_open: true,
                dispatch_receipt_recorded: true,
                confirm_receipt_recorded: true,
                fail_receipt_recorded: false,
                skip_receipt_recorded: false,
                counted_in_paid_total: true,
                classification: TreasuryPayoutClassification {
                    payout_class: TreasuryPayoutClass::PlaceholderLiveness,
                    payout_basis: Some("presence_only".to_string()),
                    ..TreasuryPayoutClassification::default()
                },
            },
        );
        state.payout_targets_by_identity.insert(
            "pubkey-homework".to_string(),
            super::RegisteredPayoutTarget {
                nostr_pubkey_hex: "pubkey-homework".to_string(),
                source_session_id: "session-homework".to_string(),
                payment_target_kind: String::new(),
                payment_target: String::new(),
                payment_target_capabilities: Vec::new(),
                pylon_payment_target_version: None,
                provider_target: "provider:homework".to_string(),
                bitcoin_address: None,
                registered_at_unix_ms: now_unix_ms.saturating_sub(10),
                last_verified_at_unix_ms: now_unix_ms.saturating_sub(10),
            },
        );
        state.queue_payout_requests(
            &config,
            &[TreasuryQueuedPayoutRequest {
                payout_key: "accepted_work:closeout-003:contrib-003:pubkey-homework".to_string(),
                nostr_pubkey_hex: "pubkey-homework".to_string(),
                amount_sats: 55,
                window_started_at_unix_ms: now_unix_ms,
                window_ends_at_unix_ms: now_unix_ms,
                classification: TreasuryPayoutClassification {
                    payout_class: TreasuryPayoutClass::AcceptedWork,
                    payout_basis: Some("aggregation_weight".to_string()),
                    work_class: Some("small_model_local_training".to_string()),
                    progress_class: Some("model_update".to_string()),
                    accepted_outcome_id: Some(
                        "accepted.training_window.window.homework.0002".to_string(),
                    ),
                    training_run_id: Some("run.homework".to_string()),
                    window_id: Some("window.homework.0002".to_string()),
                    contribution_id: Some("contrib-003".to_string()),
                    assignment_id: Some("assign-003".to_string()),
                    share_bps: Some(10_000),
                    weight_basis: Some("tokens".to_string()),
                    weight_value: Some(131_072),
                    weak_device_bearing: false,
                    progress_bearing: true,
                },
                queue_block_reason: None,
            }],
            now_unix_ms,
        );

        let prepared = state.prepare_due_payouts(&config, &[], now_unix_ms.saturating_add(1));
        assert_eq!(prepared.dispatch_plans.len(), 1);
        assert_eq!(
            prepared.dispatch_plans[0].payout_key,
            "accepted_work:closeout-003:contrib-003:pubkey-homework"
        );
        assert_eq!(
            prepared.dispatch_plans[0].payment_request,
            "provider:homework"
        );
        assert_eq!(prepared.dispatch_plans[0].amount_sats, 55);
    }

    #[test]
    fn degraded_reason_flags_zero_balance_with_receive_history() {
        let mut state = TreasuryState::default();
        state.wallet_balance_sats = 0;
        state.funding_receives_by_payment_id.insert(
            "payment-receive-001".to_string(),
            super::TreasuryFundingReceive {
                payment_id: "payment-receive-001".to_string(),
                status: "completed".to_string(),
                amount_sats: 100_000,
                method: "provider".to_string(),
                description: Some("fund treasury".to_string()),
                recorded_at_unix_ms: 100,
                updated_at_unix_ms: 100,
            },
        );
        state.payout_sats_paid_total = 2_000;
        let stats = state.public_stats(&test_treasury_config(), 200);
        assert_eq!(
            stats.degraded_reason.as_deref(),
            Some("wallet_balance_zero_with_receive_history:100000:2000")
        );
    }

    #[test]
    fn negative_wallet_recovery_delta_does_not_degrade_live_storage() {
        let mut state = TreasuryState::default();
        let config = test_treasury_config();
        let now_unix_ms = 1_000_000;
        state.wallet_runtime_status = Some("connected".to_string());
        state.wallet_balance_sats = 116_825;
        state.wallet_balance_updated_at_unix_ms = Some(now_unix_ms);
        state.last_wallet_sync_at_unix_ms = Some(now_unix_ms);
        state.last_wallet_recovery_report = Some(super::TreasuryWalletRecoveryReportSummary {
            generated_at_unix_ms: now_unix_ms,
            report_path: "/tmp/recovery-report.json".to_string(),
            current_storage_dir: "/tmp/current".to_string(),
            rebuilt_storage_dir: "/tmp/rebuilt".to_string(),
            current_balance_sats: Some(116_825),
            rebuilt_balance_sats: Some(0),
            rebuilt_minus_current_balance_sats: Some(-116_825),
            major_divergence_detected: true,
            validation_passed: true,
        });

        let stats = state.public_stats(&config, now_unix_ms);
        assert_eq!(stats.degraded_reason, None);

        state
            .last_wallet_recovery_report
            .as_mut()
            .expect("recovery summary")
            .rebuilt_minus_current_balance_sats = Some(1);
        let stats = state.public_stats(&config, now_unix_ms);
        assert_eq!(
            stats.degraded_reason.as_deref(),
            Some("wallet_storage_diverges_from_rebuild:1")
        );
    }

    #[test]
    fn validated_wallet_recovery_report_covers_zero_hydration_error() {
        let mut state = TreasuryState::default();
        let config = test_treasury_config();
        let now_unix_ms = 1_000_000;
        state.wallet_runtime_status = Some("error".to_string());
        state.wallet_last_error = Some(
            "wallet_hydration_zero_balance_after_sync_wallet_then_cached_balance:2500:100"
                .to_string(),
        );
        state.wallet_balance_sats = 91_475;
        state.last_wallet_recovery_report = Some(super::TreasuryWalletRecoveryReportSummary {
            generated_at_unix_ms: now_unix_ms,
            report_path: "/tmp/recovery-report.json".to_string(),
            current_storage_dir: "/tmp/current".to_string(),
            rebuilt_storage_dir: "/tmp/rebuilt".to_string(),
            current_balance_sats: Some(91_475),
            rebuilt_balance_sats: Some(91_475),
            rebuilt_minus_current_balance_sats: Some(0),
            major_divergence_detected: false,
            validation_passed: true,
        });

        let stats = state.public_stats(&config, now_unix_ms);

        assert_eq!(stats.wallet_runtime_status.as_deref(), Some("connected"));
        assert_eq!(stats.wallet_last_error, None);
        assert_eq!(stats.degraded_reason, None);
        assert_eq!(stats.wallet_sync_lag_ms, Some(0));
    }

    #[test]
    fn wallet_recovery_report_persists_connected_snapshot_when_current_storage_is_synced() {
        let mut state = TreasuryState::default();
        state.wallet_runtime_status = Some("error".to_string());
        state.wallet_last_error = Some("wallet_hydration_zero_balance_after_cached".to_string());

        let report = TreasuryWalletRecoveryReport {
            authority: "openagents-hosted-nexus".to_string(),
            generated_at_unix_ms: 1_000_000,
            source_wallet_storage_dir: "/tmp/current".to_string(),
            backup_root_dir: "/tmp/backup".to_string(),
            current_storage_backup_dir: "/tmp/backup/current-storage".to_string(),
            rebuilt_storage_dir: "/tmp/rebuilt".to_string(),
            report_path: "/tmp/recovery-report.json".to_string(),
            mnemonic_backup_path: "/tmp/backup/treasury.mnemonic".to_string(),
            state_backup_path: None,
            current_storage: TreasuryWalletInspection {
                wallet_identity_pubkey: "identity".to_string(),
                inspected_storage_dir: "/tmp/backup/current-storage".to_string(),
                runtime_status: Some("synced".to_string()),
                balance_sats: Some(91_475),
                ..TreasuryWalletInspection::default()
            },
            rebuilt_storage: TreasuryWalletInspection {
                wallet_identity_pubkey: "identity".to_string(),
                inspected_storage_dir: "/tmp/rebuilt".to_string(),
                runtime_status: Some("cached_after_sync_timeout".to_string()),
                balance_sats: Some(91_475),
                ..TreasuryWalletInspection::default()
            },
            comparison: TreasuryWalletRecoveryComparison {
                wallet_identity_pubkey_match: true,
                rebuilt_minus_current_balance_sats: Some(0),
                current_zero_with_receive_history: false,
                major_divergence_detected: false,
                validation_passed: true,
                recommended_action: "no_cutover_needed_sync_timeout_cached".to_string(),
            },
            cutover_active_storage_dir: None,
            cutover_rollback_storage_dir: None,
            cutover_completed_at_unix_ms: None,
        };

        state.note_wallet_recovery_report(&report);

        assert_eq!(state.wallet_runtime_status.as_deref(), Some("connected"));
        assert_eq!(state.wallet_last_error, None);
        assert_eq!(state.wallet_balance_sats, 91_475);
        assert_eq!(state.wallet_balance_updated_at_unix_ms, Some(1_000_000));
        assert_eq!(state.last_wallet_sync_at_unix_ms, Some(1_000_000));
    }

    #[test]
    fn wallet_snapshot_stale_respects_refresh_budget() {
        let mut state = TreasuryState::default();
        let mut config = test_treasury_config();
        config.wallet_status_refresh_seconds = 30;
        let now_unix_ms = 1_000_000;

        state.apply_wallet_snapshot(
            &TreasuryWalletSnapshot {
                runtime_status: "connected".to_string(),
                runtime_detail: None,
                wallet_hydration_mode: None,
                wallet_payment_scan_mode: None,
                balance_sats: 500,
                total_onchain_balance_sats: 500,
                spendable_onchain_balance_sats: 500,
                lightning_balance_sats: 0,
                payments: Vec::new(),
            },
            now_unix_ms,
        );

        let healthy_stats = state.public_stats(&config, now_unix_ms.saturating_add(30_000));
        assert_eq!(healthy_stats.degraded_reason, None);

        let idle_stats = state.public_stats(&config, now_unix_ms.saturating_add(60_001));
        assert_eq!(idle_stats.degraded_reason, None);
        assert_eq!(idle_stats.payout_loop_health, "unknown");

        let payout_key = "accepted-work:balance-blocked".to_string();
        state.payout_records_by_key.insert(
            payout_key.clone(),
            TreasuryPayoutRecord {
                payout_key,
                nostr_pubkey_hex: "pubkey-balance-blocked".to_string(),
                payout_target: "provider:balance-blocked".to_string(),
                amount_sats: 25,
                status: "queued".to_string(),
                reason: Some("wallet_balance_insufficient".to_string()),
                payment_id: None,
                window_started_at_unix_ms: now_unix_ms,
                window_ends_at_unix_ms: now_unix_ms.saturating_add(1),
                created_at_unix_ms: now_unix_ms,
                updated_at_unix_ms: now_unix_ms,
                sellable_at_window_open: true,
                dispatch_receipt_recorded: false,
                confirm_receipt_recorded: false,
                fail_receipt_recorded: false,
                skip_receipt_recorded: false,
                counted_in_paid_total: false,
                classification: TreasuryPayoutClassification {
                    payout_class: TreasuryPayoutClass::AcceptedWork,
                    payout_basis: Some("homework_acceptance".to_string()),
                    ..TreasuryPayoutClassification::default()
                },
            },
        );

        let warning_stats = state.public_stats(&config, now_unix_ms.saturating_add(60_001));
        assert_eq!(
            warning_stats.degraded_reason.as_deref(),
            Some("wallet_snapshot_stale:60001")
        );
    }

    #[test]
    fn wallet_refresh_failed_attempt_backs_off_due_check_without_faking_sync() {
        let mut state = TreasuryState::default();
        let mut config = test_treasury_config();
        config.wallet_status_refresh_seconds = 30;
        let now_unix_ms = 1_000_000;

        assert!(state.wallet_refresh_due(&config, now_unix_ms));

        state.record_wallet_refresh_error("not_enough_funds", now_unix_ms);

        assert_eq!(state.last_wallet_sync_at_unix_ms, None);
        assert_eq!(
            state.last_wallet_refresh_attempt_at_unix_ms,
            Some(now_unix_ms)
        );
        assert!(!state.wallet_refresh_due(&config, now_unix_ms + 29_999));
        assert!(state.wallet_refresh_due(&config, now_unix_ms + 30_000));
    }

    #[test]
    fn wallet_refresh_reconciles_balance_blocked_queued_payouts() {
        let mut state = TreasuryState::default();
        let now_unix_ms = 1_000_000;
        let payout_key = "accepted-work:balance-blocked".to_string();

        assert!(!state.due_wallet_refresh_requires_reconciliation());

        state.payout_records_by_key.insert(
            payout_key.clone(),
            TreasuryPayoutRecord {
                payout_key: payout_key.clone(),
                nostr_pubkey_hex: "pubkey-balance-blocked".to_string(),
                payout_target: "provider:balance-blocked".to_string(),
                amount_sats: 25,
                status: "queued".to_string(),
                reason: Some("wallet_balance_insufficient".to_string()),
                payment_id: None,
                window_started_at_unix_ms: now_unix_ms,
                window_ends_at_unix_ms: now_unix_ms.saturating_add(1),
                created_at_unix_ms: now_unix_ms,
                updated_at_unix_ms: now_unix_ms,
                sellable_at_window_open: true,
                dispatch_receipt_recorded: false,
                confirm_receipt_recorded: false,
                fail_receipt_recorded: false,
                skip_receipt_recorded: false,
                counted_in_paid_total: false,
                classification: TreasuryPayoutClassification {
                    payout_class: TreasuryPayoutClass::AcceptedWork,
                    payout_basis: Some("homework_acceptance".to_string()),
                    ..TreasuryPayoutClassification::default()
                },
            },
        );

        assert!(state.due_wallet_refresh_requires_reconciliation());

        let record = state
            .payout_records_by_key
            .get_mut(payout_key.as_str())
            .expect("queued payout");
        record.status = "failed".to_string();
        record.reason = Some("wallet_balance_insufficient".to_string());

        assert!(state.due_wallet_refresh_requires_reconciliation());

        state
            .payout_records_by_key
            .get_mut(payout_key.as_str())
            .expect("queued payout")
            .reason = Some("missing_payout_target".to_string());

        assert!(!state.due_wallet_refresh_requires_reconciliation());
    }

    #[test]
    fn wallet_refresh_reconciles_leaf_selection_failed_payouts_immediately() {
        let mut state = TreasuryState::default();
        let mut config = test_treasury_config();
        config.wallet_status_refresh_seconds = 300;
        let now_unix_ms = 1_000_000;
        let payout_key = "accepted-work:leaf-selection".to_string();

        state.payout_records_by_key.insert(
            payout_key,
            TreasuryPayoutRecord {
                payout_key: "accepted-work:leaf-selection".to_string(),
                nostr_pubkey_hex: "pubkey-leaf-selection".to_string(),
                payout_target: "provider:leaf-selection".to_string(),
                amount_sats: 25,
                status: "failed".to_string(),
                reason: Some(
                    "wallet_send_retryable:leaf_selection:TreeServiceError(InsufficientFunds)"
                        .to_string(),
                ),
                payment_id: None,
                window_started_at_unix_ms: now_unix_ms,
                window_ends_at_unix_ms: now_unix_ms.saturating_add(1),
                created_at_unix_ms: now_unix_ms,
                updated_at_unix_ms: now_unix_ms,
                sellable_at_window_open: true,
                dispatch_receipt_recorded: true,
                confirm_receipt_recorded: false,
                fail_receipt_recorded: true,
                skip_receipt_recorded: false,
                counted_in_paid_total: false,
                classification: TreasuryPayoutClassification {
                    payout_class: TreasuryPayoutClass::AcceptedWork,
                    payout_basis: Some("homework_acceptance".to_string()),
                    ..TreasuryPayoutClassification::default()
                },
            },
        );
        state.record_wallet_refresh_error(
            "wallet_send_retryable:leaf_selection:TreeServiceError(InsufficientFunds)",
            now_unix_ms,
        );

        assert!(state.due_wallet_refresh_requires_reconciliation());
        assert!(state.wallet_refresh_due(&config, now_unix_ms));
        assert!(state.wallet_refresh_due(&config, now_unix_ms + 1));
    }

    #[test]
    fn prepare_due_payouts_suppresses_dispatch_while_leaf_selection_recovery_is_active() {
        let mut state = TreasuryState::default();
        let mut config = test_treasury_config();
        config.enabled = true;
        config.placeholder_payout_mode = TreasuryPlaceholderPayoutMode::PresenceOnly;
        let now_unix_ms = 1_000_000;
        let payout_key = "accepted-work:queued-leaf-selection".to_string();

        state.payout_records_by_key.insert(
            payout_key.clone(),
            TreasuryPayoutRecord {
                payout_key: payout_key.clone(),
                nostr_pubkey_hex: "pubkey-queued".to_string(),
                payout_target: "provider:queued".to_string(),
                amount_sats: 25,
                status: "failed".to_string(),
                reason: Some(
                    "wallet_send_retryable:leaf_selection:TreeServiceError(InsufficientFunds)"
                        .to_string(),
                ),
                payment_id: None,
                window_started_at_unix_ms: now_unix_ms,
                window_ends_at_unix_ms: now_unix_ms.saturating_add(1),
                created_at_unix_ms: now_unix_ms,
                updated_at_unix_ms: now_unix_ms,
                sellable_at_window_open: true,
                dispatch_receipt_recorded: true,
                confirm_receipt_recorded: false,
                fail_receipt_recorded: true,
                skip_receipt_recorded: false,
                counted_in_paid_total: false,
                classification: TreasuryPayoutClassification {
                    payout_class: TreasuryPayoutClass::AcceptedWork,
                    payout_basis: Some("homework_acceptance".to_string()),
                    ..TreasuryPayoutClassification::default()
                },
            },
        );
        state.wallet_runtime_status = Some("error".to_string());
        state.wallet_last_error = Some(
            "wallet_send_retryable:leaf_selection:TreeServiceError(InsufficientFunds)".to_string(),
        );

        let prepared = state.prepare_due_payouts(&config, &[], now_unix_ms);

        assert!(prepared.dispatch_plans.is_empty());
        assert_eq!(
            prepared.reconciliation_degraded_reason.as_deref(),
            Some("wallet_spendability_blocked:leaf_selection")
        );
        assert_eq!(
            state
                .payout_records_by_key
                .get(payout_key.as_str())
                .expect("queued payout remains")
                .status,
            "failed"
        );
    }

    #[test]
    fn apply_dispatch_outcome_records_leaf_selection_as_wallet_refresh_error() {
        let mut state = TreasuryState::default();
        let now_unix_ms = 1_000_000;
        let payout_key = "accepted-work:dispatch-fail".to_string();

        state.payout_records_by_key.insert(
            payout_key.clone(),
            TreasuryPayoutRecord {
                payout_key: payout_key.clone(),
                nostr_pubkey_hex: "pubkey-dispatch-fail".to_string(),
                payout_target: "provider:dispatch-fail".to_string(),
                amount_sats: 25,
                status: "dispatching".to_string(),
                reason: None,
                payment_id: None,
                window_started_at_unix_ms: now_unix_ms,
                window_ends_at_unix_ms: now_unix_ms.saturating_add(1),
                created_at_unix_ms: now_unix_ms,
                updated_at_unix_ms: now_unix_ms,
                sellable_at_window_open: true,
                dispatch_receipt_recorded: false,
                confirm_receipt_recorded: false,
                fail_receipt_recorded: false,
                skip_receipt_recorded: false,
                counted_in_paid_total: false,
                classification: TreasuryPayoutClassification {
                    payout_class: TreasuryPayoutClass::AcceptedWork,
                    payout_basis: Some("homework_acceptance".to_string()),
                    ..TreasuryPayoutClassification::default()
                },
            },
        );

        state.apply_dispatch_outcome(
            TreasuryDispatchOutcome::Failed {
                payout_key,
                reason: "wallet_send_retryable:leaf_selection:TreeServiceError(InsufficientFunds)"
                    .to_string(),
            },
            now_unix_ms,
        );

        assert_eq!(state.wallet_runtime_status.as_deref(), Some("error"));
        assert_eq!(
            state.wallet_last_error.as_deref(),
            Some("wallet_send_retryable:leaf_selection:TreeServiceError(InsufficientFunds)")
        );
        assert_eq!(
            state.last_wallet_refresh_attempt_at_unix_ms,
            Some(now_unix_ms)
        );
    }

    #[test]
    fn repeated_wallet_refresh_error_updates_backoff_without_rewriting_state() {
        let path = unique_treasury_state_path("wallet-refresh-error-noop");
        let mut state = TreasuryState::default();
        state.state_path = Some(path.clone());

        state.record_wallet_refresh_error("not_enough_funds", 1_000_000);
        let persisted_first = std::fs::read_to_string(path.as_path()).expect("read first persist");

        state.record_wallet_refresh_error("not_enough_funds", 1_030_000);

        assert_eq!(
            state.last_wallet_refresh_attempt_at_unix_ms,
            Some(1_030_000)
        );
        assert_eq!(
            std::fs::read_to_string(path.as_path()).expect("read repeated failure persist"),
            persisted_first
        );

        state.record_wallet_refresh_error("wallet_refresh_timeout:60000", 1_060_000);
        let persisted_changed =
            std::fs::read_to_string(path.as_path()).expect("read changed failure persist");

        assert_ne!(persisted_changed, persisted_first);
        assert!(persisted_changed.contains("wallet_refresh_timeout:60000"));

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn dispatched_payout_refreshes_wallet_activity_and_clears_stale_reason() {
        let mut state = TreasuryState::default();
        let mut config = test_treasury_config();
        config.wallet_status_refresh_seconds = 30;

        state.wallet_runtime_status = Some("error".to_string());
        state.wallet_last_error = Some("wallet_refresh_timeout:60000".to_string());
        state.last_wallet_sync_at_unix_ms = Some(1_000);
        state.payout_records_by_key.insert(
            "window-a:pubkey-a".to_string(),
            TreasuryPayoutRecord {
                payout_key: "window-a:pubkey-a".to_string(),
                nostr_pubkey_hex: "pubkey-a".to_string(),
                payout_target: "provider:alice".to_string(),
                amount_sats: 2,
                status: "dispatching".to_string(),
                reason: None,
                payment_id: None,
                window_started_at_unix_ms: 1_000,
                window_ends_at_unix_ms: 2_000,
                created_at_unix_ms: 1_000,
                updated_at_unix_ms: 1_000,
                sellable_at_window_open: true,
                dispatch_receipt_recorded: false,
                confirm_receipt_recorded: false,
                fail_receipt_recorded: false,
                skip_receipt_recorded: false,
                counted_in_paid_total: false,
                classification: TreasuryPayoutClassification::default(),
            },
        );

        let now_unix_ms = 100_000;
        let before_stats = state.public_stats(&config, now_unix_ms);
        assert_eq!(
            before_stats.degraded_reason.as_deref(),
            Some("wallet_refresh_timeout:60000")
        );

        let receipts = state.apply_dispatch_outcome(
            TreasuryDispatchOutcome::Dispatched {
                payout_key: "window-a:pubkey-a".to_string(),
                payment_id: "payment-send-001".to_string(),
                terminal_event_state: Some("completed".to_string()),
            },
            now_unix_ms,
        );

        assert_eq!(receipts.len(), 1);
        assert_eq!(state.wallet_runtime_status.as_deref(), Some("connected"));
        assert_eq!(state.wallet_last_error, None);
        assert_eq!(state.last_wallet_sync_at_unix_ms, Some(now_unix_ms));

        let after_stats = state.public_stats(&config, now_unix_ms);
        assert_eq!(after_stats.degraded_reason, None);
    }

    #[test]
    fn recent_dispatch_activity_prevents_wallet_stale_alerts() {
        let mut state = TreasuryState::default();
        let mut config = test_treasury_config();
        config.wallet_status_refresh_seconds = 30;

        state.wallet_runtime_status = Some("connected".to_string());
        state.last_wallet_sync_at_unix_ms = Some(1_000);
        state.last_dispatch_at_unix_ms = Some(95_000);
        state.last_confirmed_payout_at_unix_ms = Some(94_000);

        let stats = state.public_stats(&config, 100_000);
        assert_eq!(stats.wallet_sync_lag_ms, Some(5_000));
        assert_eq!(stats.degraded_reason, None);
    }

    #[test]
    fn recent_dispatch_activity_suppresses_wallet_refresh_timeout_surface() {
        let mut state = TreasuryState::default();
        let mut config = test_treasury_config();
        config.wallet_status_refresh_seconds = 30;

        state.wallet_runtime_status = Some("error".to_string());
        state.wallet_last_error = Some("wallet_refresh_timeout:60000".to_string());
        state.last_wallet_sync_at_unix_ms = Some(1_000);
        state.last_dispatch_at_unix_ms = Some(95_000);
        state.last_confirmed_payout_at_unix_ms = Some(94_000);

        let stats = state.public_stats(&config, 100_000);
        assert_eq!(stats.wallet_runtime_status.as_deref(), Some("connected"));
        assert_eq!(stats.wallet_last_error, None);
        assert_eq!(stats.wallet_sync_lag_ms, Some(5_000));
        assert_eq!(stats.degraded_reason, None);
    }

    #[test]
    fn funding_target_timeout_does_not_poison_cached_wallet_status() {
        let mut state = TreasuryState::default();
        let config = test_treasury_config();
        state.wallet_runtime_status = Some("error".to_string());
        state.wallet_last_error = Some("treasury_funding_target_timeout:10000".to_string());
        state.wallet_balance_sats = 80;
        state.wallet_balance_updated_at_unix_ms = Some(1_000);

        let stats = state.public_stats(&config, 100_000);
        assert_eq!(stats.wallet_runtime_status.as_deref(), Some("connected"));
        assert_eq!(stats.wallet_last_error, None);
        assert_eq!(stats.wallet_balance_sats, 80);
        assert_eq!(stats.degraded_reason, None);
    }

    #[test]
    fn queued_accepted_work_payout_holds_insert_skipped_records() {
        let config = test_treasury_config();
        let mut state = TreasuryState::default();
        let now_unix_ms = super::now_unix_ms();

        state.queue_payout_requests(
            &config,
            &[TreasuryQueuedPayoutRequest {
                payout_key: "accepted_work:closeout-002:contrib-002:pubkey-hold".to_string(),
                nostr_pubkey_hex: "pubkey-hold".to_string(),
                amount_sats: 240,
                window_started_at_unix_ms: now_unix_ms,
                window_ends_at_unix_ms: now_unix_ms,
                classification: TreasuryPayoutClassification {
                    payout_class: TreasuryPayoutClass::AcceptedWork,
                    payout_basis: Some("aggregation_weight".to_string()),
                    work_class: Some("full_island_local_update_training".to_string()),
                    progress_class: Some("model_progress".to_string()),
                    accepted_outcome_id: Some(
                        "accepted.training_window.window.strong.hold.0001".to_string(),
                    ),
                    training_run_id: Some("run.strong.hold".to_string()),
                    window_id: Some("window.strong.hold.0001".to_string()),
                    contribution_id: Some("contrib-002".to_string()),
                    assignment_id: Some("assign-002".to_string()),
                    share_bps: Some(10_000),
                    weight_basis: Some("tokens".to_string()),
                    weight_value: Some(131_072),
                    weak_device_bearing: false,
                    progress_bearing: true,
                },
                queue_block_reason: Some(
                    "training_payout_hold_recent_non_useful_contributions".to_string(),
                ),
            }],
            now_unix_ms,
        );

        let record = state
            .payout_records_by_key
            .get("accepted_work:closeout-002:contrib-002:pubkey-hold")
            .expect("held payout record");
        assert_eq!(record.status, "skipped");
        assert_eq!(
            record.reason.as_deref(),
            Some("training_payout_hold_recent_non_useful_contributions")
        );
        assert!(record.skip_receipt_recorded);

        let prepared = state.prepare_due_payouts(&config, &[], now_unix_ms.saturating_add(1));
        assert!(prepared.dispatch_plans.is_empty());

        state.refresh_public_snapshot(&config, now_unix_ms.saturating_add(1));
        let stats = state.public_stats(&config, now_unix_ms.saturating_add(1));
        assert_eq!(stats.payout_sats_paid_total, 0);
        assert_eq!(stats.accepted_work_payout_sats_paid_total, 0);
        assert_eq!(stats.payouts_skipped_24h, 1);
        assert_eq!(stats.skip_reason_metrics_24h.len(), 1);
        assert_eq!(
            stats.skip_reason_metrics_24h[0].reason,
            "training_payout_hold_recent_non_useful_contributions"
        );
        let status = state.status_response(&config, now_unix_ms.saturating_add(1));
        assert_eq!(
            status.training_payout_ledger_summary.skipped_payout_count,
            1
        );
        assert_eq!(
            status
                .training_payout_ledger_summary
                .accepted_work_attention_payout_count,
            1
        );
        assert_eq!(
            status.training_payout_ledger_summary.reconciliation_status,
            "attention_required"
        );
    }

    #[test]
    fn failed_accepted_work_retry_claim_requires_ldk_target_and_placeholder_disable() {
        let mut config = test_treasury_config();
        config.placeholder_payout_mode = TreasuryPlaceholderPayoutMode::Disabled;
        config.daily_budget_cap_sats = 1_000_000;
        let mut state = TreasuryState::default();
        state.wallet_balance_sats = 4;
        let now_unix_ms = super::now_unix_ms();
        let retry_due_updated_at = now_unix_ms
            .saturating_sub(super::TREASURY_FAILED_PAYOUT_RETRY_AFTER_MS)
            .saturating_sub(1);

        for (pubkey, target_kind, target) in [
            ("pubkey-one", "bolt12_offer", "lno1pylonone"),
            ("pubkey-old", "", "provider:old"),
            ("pubkey-placeholder", "", "provider:placeholder"),
        ] {
            state.payout_targets_by_identity.insert(
                pubkey.to_string(),
                super::RegisteredPayoutTarget {
                    nostr_pubkey_hex: pubkey.to_string(),
                    source_session_id: format!("session-{pubkey}"),
                    payment_target_kind: target_kind.to_string(),
                    payment_target: if target_kind == "bolt12_offer" {
                        target.to_string()
                    } else {
                        String::new()
                    },
                    payment_target_capabilities: Vec::new(),
                    pylon_payment_target_version: None,
                    provider_target: if target_kind == "bolt12_offer" {
                        String::new()
                    } else {
                        target.to_string()
                    },
                    bitcoin_address: None,
                    registered_at_unix_ms: 10,
                    last_verified_at_unix_ms: 10,
                },
            );
        }

        state.payout_records_by_key.insert(
            "accepted-work:one".to_string(),
            TreasuryPayoutRecord {
                payout_key: "accepted-work:one".to_string(),
                nostr_pubkey_hex: "pubkey-one".to_string(),
                payout_target: "lno1pylonone".to_string(),
                amount_sats: 1,
                status: "failed".to_string(),
                reason: Some("wallet_send_timeout:60000".to_string()),
                payment_id: None,
                window_started_at_unix_ms: 100,
                window_ends_at_unix_ms: 200,
                created_at_unix_ms: retry_due_updated_at,
                updated_at_unix_ms: retry_due_updated_at,
                sellable_at_window_open: true,
                dispatch_receipt_recorded: false,
                confirm_receipt_recorded: false,
                fail_receipt_recorded: true,
                skip_receipt_recorded: false,
                counted_in_paid_total: false,
                classification: TreasuryPayoutClassification {
                    payout_class: TreasuryPayoutClass::AcceptedWork,
                    accepted_outcome_id: Some("accepted.one".to_string()),
                    ..TreasuryPayoutClassification::default()
                },
            },
        );
        state.payout_records_by_key.insert(
            "accepted-work:old".to_string(),
            TreasuryPayoutRecord {
                payout_key: "accepted-work:old".to_string(),
                nostr_pubkey_hex: "pubkey-old".to_string(),
                payout_target: "provider:old".to_string(),
                amount_sats: 25,
                status: "failed".to_string(),
                reason: Some("insufficient_funds".to_string()),
                payment_id: None,
                window_started_at_unix_ms: 100,
                window_ends_at_unix_ms: 200,
                created_at_unix_ms: retry_due_updated_at.saturating_sub(50),
                updated_at_unix_ms: retry_due_updated_at,
                sellable_at_window_open: true,
                dispatch_receipt_recorded: false,
                confirm_receipt_recorded: false,
                fail_receipt_recorded: true,
                skip_receipt_recorded: false,
                counted_in_paid_total: false,
                classification: TreasuryPayoutClassification {
                    payout_class: TreasuryPayoutClass::AcceptedWork,
                    accepted_outcome_id: Some("accepted.old".to_string()),
                    ..TreasuryPayoutClassification::default()
                },
            },
        );
        state.payout_records_by_key.insert(
            "accepted-work:balance-recovered".to_string(),
            TreasuryPayoutRecord {
                payout_key: "accepted-work:balance-recovered".to_string(),
                nostr_pubkey_hex: "pubkey-one".to_string(),
                payout_target: "lno1pylonone".to_string(),
                amount_sats: 1,
                status: "failed".to_string(),
                reason: Some("wallet_balance_insufficient".to_string()),
                payment_id: None,
                window_started_at_unix_ms: 100,
                window_ends_at_unix_ms: 200,
                created_at_unix_ms: retry_due_updated_at.saturating_sub(75),
                updated_at_unix_ms: retry_due_updated_at,
                sellable_at_window_open: true,
                dispatch_receipt_recorded: false,
                confirm_receipt_recorded: false,
                fail_receipt_recorded: true,
                skip_receipt_recorded: false,
                counted_in_paid_total: false,
                classification: TreasuryPayoutClassification {
                    payout_class: TreasuryPayoutClass::AcceptedWork,
                    accepted_outcome_id: Some("accepted.balance-recovered".to_string()),
                    ..TreasuryPayoutClassification::default()
                },
            },
        );
        state.payout_records_by_key.insert(
            "placeholder:old".to_string(),
            TreasuryPayoutRecord {
                payout_key: "placeholder:old".to_string(),
                nostr_pubkey_hex: "pubkey-placeholder".to_string(),
                payout_target: "provider:placeholder".to_string(),
                amount_sats: 1,
                status: "queued".to_string(),
                reason: None,
                payment_id: None,
                window_started_at_unix_ms: 100,
                window_ends_at_unix_ms: 200,
                created_at_unix_ms: retry_due_updated_at.saturating_sub(100),
                updated_at_unix_ms: retry_due_updated_at,
                sellable_at_window_open: true,
                dispatch_receipt_recorded: false,
                confirm_receipt_recorded: false,
                fail_receipt_recorded: false,
                skip_receipt_recorded: false,
                counted_in_paid_total: false,
                classification: TreasuryPayoutClassification::default(),
            },
        );

        assert!(state.treasury_enabled(&config));
        assert!(super::retryable_failed_payout_is_due(
            state
                .payout_records_by_key
                .get("accepted-work:one")
                .expect("one-sat failed accepted-work record"),
            now_unix_ms
        ));

        let prepared = state.prepare_due_payouts(&config, &[], now_unix_ms);

        assert_eq!(prepared.dispatch_plans.len(), 2);
        let dispatch_keys = prepared
            .dispatch_plans
            .iter()
            .map(|plan| plan.payout_key.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            dispatch_keys,
            vec!["accepted-work:balance-recovered", "accepted-work:one"]
        );
        assert!(
            prepared
                .dispatch_plans
                .iter()
                .all(|plan| plan.amount_sats == 1 && plan.payment_request == "lno1pylonone")
        );
        assert_eq!(
            state
                .payout_records_by_key
                .get("accepted-work:one")
                .map(|record| (record.status.as_str(), record.reason.as_deref())),
            Some(("dispatching", None))
        );
        assert_eq!(
            state
                .payout_records_by_key
                .get("accepted-work:balance-recovered")
                .map(|record| (record.status.as_str(), record.reason.as_deref())),
            Some(("dispatching", None))
        );
        assert_eq!(
            state
                .payout_records_by_key
                .get("accepted-work:old")
                .map(|record| (record.status.as_str(), record.reason.as_deref())),
            Some(("failed", Some("insufficient_funds")))
        );
        assert_eq!(
            state
                .payout_records_by_key
                .get("placeholder:old")
                .map(|record| (record.status.as_str(), record.reason.as_deref())),
            Some(("skipped", Some("placeholder_payouts_disabled")))
        );
    }

    #[test]
    fn retryable_failed_accepted_work_counts_as_pending_in_training_summary() {
        let mut state = TreasuryState::default();
        state.payout_records_by_key.insert(
            "accepted-work:retryable".to_string(),
            TreasuryPayoutRecord {
                payout_key: "accepted-work:retryable".to_string(),
                nostr_pubkey_hex: "pubkey-retryable".to_string(),
                payout_target: "lno1pylonretryable".to_string(),
                amount_sats: 25,
                status: "failed".to_string(),
                reason: Some("wallet_send_timeout:60000".to_string()),
                payment_id: None,
                window_started_at_unix_ms: 100,
                window_ends_at_unix_ms: 200,
                created_at_unix_ms: 100,
                updated_at_unix_ms: 200,
                sellable_at_window_open: true,
                dispatch_receipt_recorded: false,
                confirm_receipt_recorded: false,
                fail_receipt_recorded: true,
                skip_receipt_recorded: false,
                counted_in_paid_total: false,
                classification: TreasuryPayoutClassification {
                    payout_class: TreasuryPayoutClass::AcceptedWork,
                    accepted_outcome_id: Some("accepted.retryable".to_string()),
                    ..TreasuryPayoutClassification::default()
                },
            },
        );

        let summary = state.training_payout_ledger_summary();
        assert_eq!(summary.pending_payout_count, 1);
        assert_eq!(summary.accepted_work_pending_payout_count, 1);
        assert_eq!(summary.failed_payout_count, 0);
        assert_eq!(summary.accepted_work_attention_payout_count, 0);
        assert_eq!(summary.reconciliation_status, "pending");
    }

    #[test]
    fn retryable_failed_availability_payout_counts_as_pending_not_attention() {
        let mut state = TreasuryState::default();
        state.payout_records_by_key.insert(
            "availability:retryable".to_string(),
            TreasuryPayoutRecord {
                payout_key: "availability:retryable".to_string(),
                nostr_pubkey_hex: "pubkey-retryable".to_string(),
                payout_target: "lno1pylonretryable".to_string(),
                amount_sats: 25,
                status: "failed".to_string(),
                reason: Some(
                    "wallet_send_retryable:isolated_runtime:treasury_isolated_dispatch_timeout:70000"
                        .to_string(),
                ),
                payment_id: None,
                window_started_at_unix_ms: 100,
                window_ends_at_unix_ms: 200,
                created_at_unix_ms: 100,
                updated_at_unix_ms: 200,
                sellable_at_window_open: true,
                dispatch_receipt_recorded: false,
                confirm_receipt_recorded: false,
                fail_receipt_recorded: true,
                skip_receipt_recorded: false,
                counted_in_paid_total: false,
                classification: TreasuryPayoutClassification::default(),
            },
        );

        let summary = state.training_payout_ledger_summary();
        assert_eq!(summary.pending_payout_count, 1);
        assert_eq!(summary.accepted_work_pending_payout_count, 0);
        assert_eq!(summary.failed_payout_count, 0);
        assert_eq!(summary.attention_payout_count, 0);
        assert_eq!(summary.accepted_work_attention_payout_count, 0);
        assert_eq!(summary.reconciliation_status, "clean");
        assert_eq!(
            super::treasury_payout_reconciliation_status(
                state
                    .payout_records_by_key
                    .get("availability:retryable")
                    .expect("retryable availability record")
            ),
            "pending_retry"
        );
    }

    #[test]
    fn retryable_failed_availability_dispatch_is_capped_per_cycle() {
        let mut state = TreasuryState::default();
        state.wallet_balance_sats = 1_000;
        let mut config = test_treasury_config();
        config.availability_max_concurrent_sends = 2;
        let now_unix_ms = 100 + TREASURY_FAILED_PAYOUT_RETRY_AFTER_MS + 1;

        for index in 0..3 {
            let pubkey = format!("pubkey-{index}");
            state.payout_targets_by_identity.insert(
                pubkey.clone(),
                super::RegisteredPayoutTarget {
                    nostr_pubkey_hex: pubkey.clone(),
                    source_session_id: format!("session-{index}"),
                    payment_target_kind: "bolt12_offer".to_string(),
                    payment_target: format!("lno1pylontarget{index}"),
                    payment_target_capabilities: Vec::new(),
                    pylon_payment_target_version: None,
                    provider_target: String::new(),
                    bitcoin_address: None,
                    registered_at_unix_ms: 10,
                    last_verified_at_unix_ms: 10,
                },
            );
            state.payout_records_by_key.insert(
                format!("availability:retryable:{index}"),
                TreasuryPayoutRecord {
                    payout_key: format!("availability:retryable:{index}"),
                    nostr_pubkey_hex: pubkey,
                    payout_target: format!("lno1pylontarget{index}"),
                    amount_sats: 25,
                    status: "failed".to_string(),
                    reason: Some("wallet_send_timeout:60000".to_string()),
                    payment_id: None,
                    window_started_at_unix_ms: 50,
                    window_ends_at_unix_ms: 60,
                    created_at_unix_ms: 100,
                    updated_at_unix_ms: 100,
                    sellable_at_window_open: true,
                    dispatch_receipt_recorded: false,
                    confirm_receipt_recorded: false,
                    fail_receipt_recorded: true,
                    skip_receipt_recorded: false,
                    counted_in_paid_total: false,
                    classification: TreasuryPayoutClassification::default(),
                },
            );
        }

        let prepared = state.prepare_due_payouts(&config, &[], now_unix_ms);

        assert_eq!(prepared.dispatch_plans.len(), 2);
        assert_eq!(
            state
                .payout_records_by_key
                .values()
                .filter(|record| record.status == "dispatching")
                .count(),
            2
        );
        assert_eq!(
            state
                .payout_records_by_key
                .values()
                .filter(|record| record.status == "failed")
                .count(),
            1
        );
    }

    #[test]
    fn reason_metrics_break_out_skip_and_fail_reasons() {
        let mut state = TreasuryState::default();
        state.payout_records_by_key.insert(
            "skip-a".to_string(),
            super::TreasuryPayoutRecord {
                payout_key: "skip-a".to_string(),
                nostr_pubkey_hex: "pubkey-a".to_string(),
                payout_target: "provider:alice".to_string(),
                amount_sats: 120,
                status: "skipped".to_string(),
                reason: Some("daily_budget_cap_reached".to_string()),
                payment_id: None,
                window_started_at_unix_ms: 100,
                window_ends_at_unix_ms: 200,
                created_at_unix_ms: 200,
                updated_at_unix_ms: 200,
                sellable_at_window_open: true,
                dispatch_receipt_recorded: false,
                confirm_receipt_recorded: false,
                fail_receipt_recorded: false,
                skip_receipt_recorded: true,
                counted_in_paid_total: false,
                classification: TreasuryPayoutClassification::default(),
            },
        );
        state.payout_records_by_key.insert(
            "skip-b".to_string(),
            super::TreasuryPayoutRecord {
                payout_key: "skip-b".to_string(),
                nostr_pubkey_hex: "pubkey-b".to_string(),
                payout_target: String::new(),
                amount_sats: 120,
                status: "skipped".to_string(),
                reason: Some("missing_payout_target".to_string()),
                payment_id: None,
                window_started_at_unix_ms: 100,
                window_ends_at_unix_ms: 200,
                created_at_unix_ms: 210,
                updated_at_unix_ms: 210,
                sellable_at_window_open: true,
                dispatch_receipt_recorded: false,
                confirm_receipt_recorded: false,
                fail_receipt_recorded: false,
                skip_receipt_recorded: true,
                counted_in_paid_total: false,
                classification: TreasuryPayoutClassification::default(),
            },
        );
        state.payout_records_by_key.insert(
            "fail-a".to_string(),
            super::TreasuryPayoutRecord {
                payout_key: "fail-a".to_string(),
                nostr_pubkey_hex: "pubkey-a".to_string(),
                payout_target: "provider:alice".to_string(),
                amount_sats: 120,
                status: "failed".to_string(),
                reason: Some("wallet_send_failed".to_string()),
                payment_id: Some("payment-1".to_string()),
                window_started_at_unix_ms: 100,
                window_ends_at_unix_ms: 200,
                created_at_unix_ms: 220,
                updated_at_unix_ms: 220,
                sellable_at_window_open: true,
                dispatch_receipt_recorded: true,
                confirm_receipt_recorded: false,
                fail_receipt_recorded: true,
                skip_receipt_recorded: false,
                counted_in_paid_total: false,
                classification: TreasuryPayoutClassification::default(),
            },
        );

        let (skip_metrics, fail_metrics) = state.reason_metrics_24h(1_000);
        assert_eq!(skip_metrics.len(), 2);
        assert!(skip_metrics.iter().any(|metric| {
            metric.reason == "daily_budget_cap_reached"
                && metric.count == 1
                && metric.total_sats == 120
        }));
        assert!(skip_metrics.iter().any(|metric| {
            metric.reason == "missing_payout_target"
                && metric.count == 1
                && metric.total_sats == 120
        }));
        assert_eq!(fail_metrics.len(), 1);
        assert_eq!(fail_metrics[0].reason, "wallet_send_failed");
        assert_eq!(fail_metrics[0].count, 1);
        assert_eq!(fail_metrics[0].total_sats, 120);
    }

    #[test]
    fn continuity_alerts_raise_and_clear_for_stalled_windows() {
        let mut state = TreasuryState::default();
        let config = test_treasury_config();
        let eligible_at_unix_ms = 1_800_000;
        state.eligible_online_payout_targets = 1;
        state.sellable_pylons_online_now = 1;
        state.latest_eligible_window_started_at_unix_ms = Some(eligible_at_unix_ms);
        state.payout_records_by_key.insert(
            "pending-a".to_string(),
            super::TreasuryPayoutRecord {
                payout_key: "pending-a".to_string(),
                nostr_pubkey_hex: "pubkey-a".to_string(),
                payout_target: "provider:alice".to_string(),
                amount_sats: 2,
                status: "dispatching".to_string(),
                reason: None,
                payment_id: None,
                window_started_at_unix_ms: eligible_at_unix_ms,
                window_ends_at_unix_ms: eligible_at_unix_ms.saturating_add(20_000),
                created_at_unix_ms: eligible_at_unix_ms,
                updated_at_unix_ms: eligible_at_unix_ms,
                sellable_at_window_open: true,
                dispatch_receipt_recorded: false,
                confirm_receipt_recorded: false,
                fail_receipt_recorded: false,
                skip_receipt_recorded: false,
                counted_in_paid_total: false,
                classification: TreasuryPayoutClassification::default(),
            },
        );

        let alert_at_unix_ms =
            eligible_at_unix_ms + super::TREASURY_CONTINUITY_ALERT_THRESHOLD_MS + 60_000;
        let raised = state.sync_continuity_alerts(&config, alert_at_unix_ms);
        assert_eq!(raised.len(), 1);
        assert!(
            raised
                .iter()
                .any(|event| event.receipt_type == "treasury.alert.raised")
        );
        assert_eq!(state.active_continuity_alerts.len(), 1);

        if let Some(record) = state.payout_records_by_key.get_mut("pending-a") {
            record.status = "confirmed".to_string();
            record.updated_at_unix_ms = alert_at_unix_ms;
        }
        state.last_dispatch_at_unix_ms = Some(alert_at_unix_ms);
        state.last_confirmed_payout_at_unix_ms = Some(alert_at_unix_ms);
        let cleared = state.sync_continuity_alerts(&config, alert_at_unix_ms + 1);
        assert_eq!(cleared.len(), 1);
        assert!(
            cleared
                .iter()
                .all(|event| event.receipt_type == "treasury.alert.cleared")
        );
        assert!(state.active_continuity_alerts.is_empty());
    }

    #[test]
    fn continuity_alert_refresh_without_structural_change_does_not_rewrite_state() {
        let path = unique_treasury_state_path("continuity-noop");
        let mut state = TreasuryState::default();
        state.next_challenge_nonce = 1;
        state.state_path = Some(path.clone());
        let config = test_treasury_config();
        let eligible_at_unix_ms = 1_800_000;
        state.eligible_online_payout_targets = 1;
        state.sellable_pylons_online_now = 1;
        state.latest_eligible_window_started_at_unix_ms = Some(eligible_at_unix_ms);
        state.payout_records_by_key.insert(
            "pending-a".to_string(),
            super::TreasuryPayoutRecord {
                payout_key: "pending-a".to_string(),
                nostr_pubkey_hex: "pubkey-a".to_string(),
                payout_target: "provider:alice".to_string(),
                amount_sats: 2,
                status: "dispatching".to_string(),
                reason: None,
                payment_id: None,
                window_started_at_unix_ms: eligible_at_unix_ms,
                window_ends_at_unix_ms: eligible_at_unix_ms.saturating_add(20_000),
                created_at_unix_ms: eligible_at_unix_ms,
                updated_at_unix_ms: eligible_at_unix_ms,
                sellable_at_window_open: true,
                dispatch_receipt_recorded: false,
                confirm_receipt_recorded: false,
                fail_receipt_recorded: false,
                skip_receipt_recorded: false,
                counted_in_paid_total: false,
                classification: TreasuryPayoutClassification::default(),
            },
        );

        let alert_at_unix_ms =
            eligible_at_unix_ms + super::TREASURY_CONTINUITY_ALERT_THRESHOLD_MS + 60_000;
        let raised = state.sync_continuity_alerts(&config, alert_at_unix_ms);
        assert_eq!(raised.len(), 1);

        let before = std::fs::read_to_string(path.as_path()).expect("read persisted state");
        let refreshed = state.sync_continuity_alerts(&config, alert_at_unix_ms.saturating_add(1));
        assert!(refreshed.is_empty());
        assert_eq!(
            std::fs::read_to_string(path.as_path()).expect("read persisted state"),
            before
        );

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn continuity_alerts_detect_backlog_even_when_latest_window_is_recent() {
        let mut state = TreasuryState::default();
        let config = test_treasury_config();
        let now_unix_ms = 2_000_000u64;

        state.eligible_online_payout_targets = 120;
        state.sellable_pylons_online_now = 120;
        state.latest_eligible_window_started_at_unix_ms = Some(now_unix_ms.saturating_sub(1_000));
        state.last_dispatch_at_unix_ms = Some(
            now_unix_ms.saturating_sub(super::TREASURY_CONTINUITY_ALERT_THRESHOLD_MS + 60_000),
        );
        state.last_confirmed_payout_at_unix_ms = Some(
            now_unix_ms
                .saturating_sub(super::TREASURY_CONFIRMATION_STALL_ALERT_THRESHOLD_MS + 60_000),
        );
        state.payout_records_by_key.insert(
            "dispatch-backlog".to_string(),
            super::TreasuryPayoutRecord {
                payout_key: "dispatch-backlog".to_string(),
                nostr_pubkey_hex: "pubkey-a".to_string(),
                payout_target: "provider:alice".to_string(),
                amount_sats: 2,
                status: "dispatching".to_string(),
                reason: None,
                payment_id: None,
                window_started_at_unix_ms: now_unix_ms.saturating_sub(60_000),
                window_ends_at_unix_ms: now_unix_ms.saturating_sub(40_000),
                created_at_unix_ms: now_unix_ms
                    .saturating_sub(super::TREASURY_CONTINUITY_ALERT_THRESHOLD_MS + 10_000),
                updated_at_unix_ms: now_unix_ms
                    .saturating_sub(super::TREASURY_CONTINUITY_ALERT_THRESHOLD_MS + 10_000),
                sellable_at_window_open: true,
                dispatch_receipt_recorded: false,
                confirm_receipt_recorded: false,
                fail_receipt_recorded: false,
                skip_receipt_recorded: false,
                counted_in_paid_total: false,
                classification: TreasuryPayoutClassification::default(),
            },
        );
        state.payout_records_by_key.insert(
            "confirmation-backlog".to_string(),
            super::TreasuryPayoutRecord {
                payout_key: "confirmation-backlog".to_string(),
                nostr_pubkey_hex: "pubkey-b".to_string(),
                payout_target: "provider:bob".to_string(),
                amount_sats: 2,
                status: "dispatched".to_string(),
                reason: None,
                payment_id: Some("payment-b".to_string()),
                window_started_at_unix_ms: now_unix_ms.saturating_sub(120_000),
                window_ends_at_unix_ms: now_unix_ms.saturating_sub(100_000),
                created_at_unix_ms: now_unix_ms
                    .saturating_sub(super::TREASURY_CONFIRMATION_STALL_ALERT_THRESHOLD_MS + 10_000),
                updated_at_unix_ms: now_unix_ms
                    .saturating_sub(super::TREASURY_CONFIRMATION_STALL_ALERT_THRESHOLD_MS + 10_000),
                sellable_at_window_open: true,
                dispatch_receipt_recorded: true,
                confirm_receipt_recorded: false,
                fail_receipt_recorded: false,
                skip_receipt_recorded: false,
                counted_in_paid_total: false,
                classification: TreasuryPayoutClassification::default(),
            },
        );

        let stats = state.public_stats(&config, now_unix_ms);
        assert_eq!(
            stats.degraded_reason.as_deref(),
            Some("continuity_alert:dispatch_stalled")
        );
        assert!(
            stats.active_continuity_alerts.iter().any(|alert| {
                alert.alert_id == "dispatch_stalled"
                    && alert.reason == "pending_payouts_not_dispatching"
            }),
            "dispatch backlog should raise a critical continuity alert"
        );
        assert!(
            stats.active_continuity_alerts.iter().any(|alert| {
                alert.alert_id == "confirmations_stalled"
                    && alert.reason == "pending_payouts_not_confirming"
            }),
            "confirmation backlog should raise a critical continuity alert"
        );
    }

    #[test]
    fn disabled_placeholder_payouts_do_not_raise_stalled_alerts_for_legacy_liveness_records() {
        let mut state = TreasuryState::default();
        let mut config = test_treasury_config();
        config.placeholder_payout_mode = TreasuryPlaceholderPayoutMode::Disabled;
        let now_unix_ms = 2_000_000u64;
        state.wallet_runtime_status = Some("connected".to_string());
        state.wallet_balance_sats = 10_000;
        state.wallet_balance_updated_at_unix_ms = Some(now_unix_ms);
        state.last_wallet_sync_at_unix_ms = Some(now_unix_ms);

        state.payout_records_by_key.insert(
            "legacy-placeholder-dispatched".to_string(),
            super::TreasuryPayoutRecord {
                payout_key: "legacy-placeholder-dispatched".to_string(),
                nostr_pubkey_hex: "pubkey-a".to_string(),
                payout_target: "provider:alice".to_string(),
                amount_sats: 2,
                status: "dispatched".to_string(),
                reason: None,
                payment_id: Some("legacy-payment-a".to_string()),
                window_started_at_unix_ms: now_unix_ms.saturating_sub(120_000),
                window_ends_at_unix_ms: now_unix_ms.saturating_sub(60_000),
                created_at_unix_ms: now_unix_ms
                    .saturating_sub(super::TREASURY_CONFIRMATION_STALL_ALERT_THRESHOLD_MS + 10_000),
                updated_at_unix_ms: now_unix_ms
                    .saturating_sub(super::TREASURY_CONFIRMATION_STALL_ALERT_THRESHOLD_MS + 10_000),
                sellable_at_window_open: true,
                dispatch_receipt_recorded: true,
                confirm_receipt_recorded: false,
                fail_receipt_recorded: false,
                skip_receipt_recorded: false,
                counted_in_paid_total: false,
                classification: TreasuryPayoutClassification::default(),
            },
        );

        let receipts = state.sync_continuity_alerts(&config, now_unix_ms);
        assert!(receipts.is_empty());
        assert!(state.active_continuity_alerts.is_empty());
    }

    #[test]
    fn recent_dispatched_payments_do_not_degrade_before_confirmation_stall_threshold() {
        let mut state = TreasuryState::default();
        let config = test_treasury_config();
        let now_unix_ms = 2_000_000u64;
        let normal_slow_confirmation_age_ms =
            super::TREASURY_CONTINUITY_ALERT_THRESHOLD_MS + 60_000;
        state.payout_records_by_key.insert(
            "normal-slow-dispatched".to_string(),
            super::TreasuryPayoutRecord {
                payout_key: "normal-slow-dispatched".to_string(),
                nostr_pubkey_hex: "pubkey-a".to_string(),
                payout_target: "provider:alice".to_string(),
                amount_sats: 25,
                status: "dispatched".to_string(),
                reason: None,
                payment_id: Some("normal-slow-payment".to_string()),
                window_started_at_unix_ms: now_unix_ms.saturating_sub(120_000),
                window_ends_at_unix_ms: now_unix_ms.saturating_sub(60_000),
                created_at_unix_ms: now_unix_ms.saturating_sub(normal_slow_confirmation_age_ms),
                updated_at_unix_ms: now_unix_ms.saturating_sub(normal_slow_confirmation_age_ms),
                sellable_at_window_open: true,
                dispatch_receipt_recorded: true,
                confirm_receipt_recorded: false,
                fail_receipt_recorded: false,
                skip_receipt_recorded: false,
                counted_in_paid_total: false,
                classification: TreasuryPayoutClassification {
                    payout_class: TreasuryPayoutClass::AcceptedWork,
                    accepted_outcome_id: Some("outcome-a".to_string()),
                    ..TreasuryPayoutClassification::default()
                },
            },
        );

        let stats = state.public_stats(&config, now_unix_ms);

        assert_ne!(
            stats.degraded_reason.as_deref(),
            Some("continuity_alert:confirmations_stalled")
        );
        assert!(
            stats
                .active_continuity_alerts
                .iter()
                .all(|alert| alert.alert_id != "confirmations_stalled"),
            "freshly dispatched payments should not degrade Nexus while wallet confirmations are still within the normal window"
        );
    }

    #[test]
    fn disabled_placeholder_payouts_still_raise_stalled_alerts_for_accepted_work() {
        let mut state = TreasuryState::default();
        let mut config = test_treasury_config();
        config.placeholder_payout_mode = TreasuryPlaceholderPayoutMode::Disabled;
        let now_unix_ms = 2_000_000u64;

        state.payout_records_by_key.insert(
            "accepted-work-dispatched".to_string(),
            super::TreasuryPayoutRecord {
                payout_key: "accepted-work-dispatched".to_string(),
                nostr_pubkey_hex: "pubkey-a".to_string(),
                payout_target: "provider:alice".to_string(),
                amount_sats: 2,
                status: "dispatched".to_string(),
                reason: None,
                payment_id: Some("accepted-payment-a".to_string()),
                window_started_at_unix_ms: now_unix_ms.saturating_sub(120_000),
                window_ends_at_unix_ms: now_unix_ms.saturating_sub(60_000),
                created_at_unix_ms: now_unix_ms
                    .saturating_sub(super::TREASURY_CONFIRMATION_STALL_ALERT_THRESHOLD_MS + 10_000),
                updated_at_unix_ms: now_unix_ms
                    .saturating_sub(super::TREASURY_CONFIRMATION_STALL_ALERT_THRESHOLD_MS + 10_000),
                sellable_at_window_open: true,
                dispatch_receipt_recorded: true,
                confirm_receipt_recorded: false,
                fail_receipt_recorded: false,
                skip_receipt_recorded: false,
                counted_in_paid_total: false,
                classification: TreasuryPayoutClassification {
                    payout_class: TreasuryPayoutClass::AcceptedWork,
                    accepted_outcome_id: Some("outcome-a".to_string()),
                    ..TreasuryPayoutClassification::default()
                },
            },
        );

        let receipts = state.sync_continuity_alerts(&config, now_unix_ms);
        assert_eq!(receipts.len(), 1);
        assert!(
            state.active_continuity_alerts.iter().any(|alert| {
                alert.alert_id == "confirmations_stalled"
                    && alert.reason == "pending_payouts_not_confirming"
            }),
            "accepted-work backlog must still raise a critical continuity alert"
        );
    }

    #[test]
    fn disabled_placeholder_payouts_hide_availability_confirmations_from_backlog() {
        let mut state = TreasuryState::default();
        let mut config = test_treasury_config();
        config.placeholder_payout_mode = TreasuryPlaceholderPayoutMode::Disabled;
        let now_unix_ms = 2_000_000u64;
        state.wallet_runtime_status = Some("connected".to_string());
        state.wallet_balance_sats = 10_000;
        state.wallet_balance_updated_at_unix_ms = Some(now_unix_ms);
        state.last_wallet_sync_at_unix_ms = Some(now_unix_ms);

        state.payout_records_by_key.insert(
            "1770000:legacy-identity-pubkey".to_string(),
            super::TreasuryPayoutRecord {
                payout_key: "1770000:legacy-identity-pubkey".to_string(),
                nostr_pubkey_hex: "legacy-identity-pubkey".to_string(),
                payout_target: "provider:legacy".to_string(),
                amount_sats: 2,
                status: "dispatched".to_string(),
                reason: None,
                payment_id: Some("legacy-presence-payment".to_string()),
                window_started_at_unix_ms: now_unix_ms.saturating_sub(120_000),
                window_ends_at_unix_ms: now_unix_ms.saturating_sub(60_000),
                created_at_unix_ms: now_unix_ms
                    .saturating_sub(super::TREASURY_CONFIRMATION_STALL_ALERT_THRESHOLD_MS + 10_000),
                updated_at_unix_ms: now_unix_ms
                    .saturating_sub(super::TREASURY_CONFIRMATION_STALL_ALERT_THRESHOLD_MS + 10_000),
                sellable_at_window_open: true,
                dispatch_receipt_recorded: true,
                confirm_receipt_recorded: false,
                fail_receipt_recorded: false,
                skip_receipt_recorded: false,
                counted_in_paid_total: false,
                classification: TreasuryPayoutClassification::default(),
            },
        );

        state.payout_records_by_key.insert(
            "1770001:availability-beneficiary:host:sha256:newer-stipend".to_string(),
            super::TreasuryPayoutRecord {
                payout_key: "1770001:availability-beneficiary:host:sha256:newer-stipend"
                    .to_string(),
                nostr_pubkey_hex: "newer-presence-pubkey".to_string(),
                payout_target: "provider:newer-presence".to_string(),
                amount_sats: 25,
                status: "dispatched".to_string(),
                reason: None,
                payment_id: Some("newer-presence-payment".to_string()),
                window_started_at_unix_ms: now_unix_ms.saturating_sub(120_000),
                window_ends_at_unix_ms: now_unix_ms.saturating_sub(60_000),
                created_at_unix_ms: now_unix_ms
                    .saturating_sub(super::TREASURY_CONFIRMATION_STALL_ALERT_THRESHOLD_MS + 10_000),
                updated_at_unix_ms: now_unix_ms
                    .saturating_sub(super::TREASURY_CONFIRMATION_STALL_ALERT_THRESHOLD_MS + 10_000),
                sellable_at_window_open: true,
                dispatch_receipt_recorded: true,
                confirm_receipt_recorded: false,
                fail_receipt_recorded: false,
                skip_receipt_recorded: false,
                counted_in_paid_total: false,
                classification: TreasuryPayoutClassification {
                    payout_basis: Some("presence_only".to_string()),
                    ..TreasuryPayoutClassification::default()
                },
            },
        );

        let stats = state.public_stats(&config, now_unix_ms);

        assert_eq!(stats.pending_confirmation_count, 0);
        assert_eq!(stats.tracked_payment_backlog_count, 0);
        assert_eq!(stats.legacy_availability_confirmation_attention_count, 2);
        assert_eq!(stats.degraded_reason, None);
    }

    #[test]
    fn newer_stipend_confirmation_progress_clears_stale_presence_stall() {
        let mut state = TreasuryState::default();
        let config = test_treasury_config();
        let now_unix_ms = 2_000_000u64;
        let stale_updated_at = now_unix_ms
            .saturating_sub(super::TREASURY_CONFIRMATION_STALL_ALERT_THRESHOLD_MS + 60_000);

        state.payout_records_by_key.insert(
            "1770000:availability-beneficiary:host:sha256:stale".to_string(),
            super::TreasuryPayoutRecord {
                payout_key: "1770000:availability-beneficiary:host:sha256:stale".to_string(),
                nostr_pubkey_hex: "pubkey-a".to_string(),
                payout_target: "provider:alice".to_string(),
                amount_sats: 25,
                status: "dispatched".to_string(),
                reason: None,
                payment_id: Some("stale-presence-payment".to_string()),
                window_started_at_unix_ms: stale_updated_at.saturating_sub(60_000),
                window_ends_at_unix_ms: stale_updated_at,
                created_at_unix_ms: stale_updated_at,
                updated_at_unix_ms: stale_updated_at,
                sellable_at_window_open: true,
                dispatch_receipt_recorded: true,
                confirm_receipt_recorded: false,
                fail_receipt_recorded: false,
                skip_receipt_recorded: false,
                counted_in_paid_total: false,
                classification: TreasuryPayoutClassification {
                    payout_basis: Some("presence_only".to_string()),
                    ..TreasuryPayoutClassification::default()
                },
            },
        );

        let stale_stats = state.public_stats(&config, now_unix_ms);
        assert_eq!(
            stale_stats.degraded_reason.as_deref(),
            Some("continuity_alert:confirmations_stalled")
        );

        state.last_confirmed_payout_at_unix_ms = Some(now_unix_ms.saturating_sub(1_000));
        let recovered_stats = state.public_stats(&config, now_unix_ms);
        assert_ne!(
            recovered_stats.degraded_reason.as_deref(),
            Some("continuity_alert:confirmations_stalled")
        );
        assert!(
            recovered_stats
                .active_continuity_alerts
                .iter()
                .all(|alert| alert.alert_id != "confirmations_stalled"),
            "presence-only stale confirmations should not keep Nexus degraded after newer confirmations"
        );
    }

    #[test]
    fn newer_stipend_progress_does_not_clear_stale_accepted_work_stall() {
        let mut state = TreasuryState::default();
        let config = test_treasury_config();
        let now_unix_ms = 2_000_000u64;
        let stale_updated_at = now_unix_ms
            .saturating_sub(super::TREASURY_CONFIRMATION_STALL_ALERT_THRESHOLD_MS + 60_000);

        state.last_confirmed_payout_at_unix_ms = Some(now_unix_ms.saturating_sub(1_000));
        state.payout_records_by_key.insert(
            "accepted-work-stale".to_string(),
            super::TreasuryPayoutRecord {
                payout_key: "accepted-work-stale".to_string(),
                nostr_pubkey_hex: "pubkey-a".to_string(),
                payout_target: "provider:alice".to_string(),
                amount_sats: 25,
                status: "dispatched".to_string(),
                reason: None,
                payment_id: Some("accepted-work-payment".to_string()),
                window_started_at_unix_ms: stale_updated_at.saturating_sub(60_000),
                window_ends_at_unix_ms: stale_updated_at,
                created_at_unix_ms: stale_updated_at,
                updated_at_unix_ms: stale_updated_at,
                sellable_at_window_open: true,
                dispatch_receipt_recorded: true,
                confirm_receipt_recorded: false,
                fail_receipt_recorded: false,
                skip_receipt_recorded: false,
                counted_in_paid_total: false,
                classification: TreasuryPayoutClassification {
                    payout_class: TreasuryPayoutClass::AcceptedWork,
                    accepted_outcome_id: Some("outcome-a".to_string()),
                    ..TreasuryPayoutClassification::default()
                },
            },
        );

        let stats = state.public_stats(&config, now_unix_ms);
        assert_eq!(
            stats.degraded_reason.as_deref(),
            Some("continuity_alert:confirmations_stalled")
        );
    }

    #[test]
    fn budget_cap_alert_ignores_stale_historical_skips() {
        let mut state = TreasuryState::default();
        let config = test_treasury_config();
        state.payout_targets_by_identity.insert(
            "pubkey-a".to_string(),
            super::RegisteredPayoutTarget {
                nostr_pubkey_hex: "pubkey-a".to_string(),
                source_session_id: "session-a".to_string(),
                payment_target_kind: String::new(),
                payment_target: String::new(),
                payment_target_capabilities: Vec::new(),
                pylon_payment_target_version: None,
                provider_target: "provider:alice".to_string(),
                bitcoin_address: None,
                registered_at_unix_ms: 10,
                last_verified_at_unix_ms: 10,
            },
        );
        state.payout_records_by_key.insert(
            "stale-budget-skip".to_string(),
            super::TreasuryPayoutRecord {
                payout_key: "stale-budget-skip".to_string(),
                nostr_pubkey_hex: "pubkey-a".to_string(),
                payout_target: "provider:alice".to_string(),
                amount_sats: 2,
                status: "skipped".to_string(),
                reason: Some("daily_budget_cap_reached".to_string()),
                payment_id: None,
                window_started_at_unix_ms: 1_000,
                window_ends_at_unix_ms: 21_000,
                created_at_unix_ms: 21_000,
                updated_at_unix_ms: 21_000,
                sellable_at_window_open: true,
                dispatch_receipt_recorded: false,
                confirm_receipt_recorded: false,
                fail_receipt_recorded: false,
                skip_receipt_recorded: true,
                counted_in_paid_total: false,
                classification: TreasuryPayoutClassification::default(),
            },
        );

        let eligible_at_unix_ms = 1_800_000;
        state.observe_payout_eligibility(
            &config,
            &[OnlinePylonIdentity {
                inference_ready: false,
                ..test_online_identity("pubkey-a")
            }],
            eligible_at_unix_ms,
        );

        let stats = state.public_stats(&config, eligible_at_unix_ms + 1);
        assert!(
            !stats
                .active_continuity_alerts
                .iter()
                .any(|alert| alert.alert_id == "budget_cap_exhausted")
        );
    }

    #[test]
    fn payout_loop_start_sets_reconciliation_anchor_immediately() {
        let mut state = TreasuryState::default();
        state.note_payout_loop_started(1_234_567);
        assert_eq!(state.payout_loop_runtime_status.as_deref(), Some("running"));
        assert_eq!(state.payout_loop_last_started_at_unix_ms, Some(1_234_567));
        assert_eq!(state.last_payout_reconciliation_at_unix_ms, Some(1_234_567));

        state.note_payout_loop_completed(1_345_678, None);
        assert_eq!(state.last_payout_reconciliation_at_unix_ms, Some(1_345_678));
        assert_eq!(state.payout_loop_last_completed_at_unix_ms, Some(1_345_678));
    }

    #[test]
    fn dispatch_cycle_due_throttles_idle_availability_scan_but_honors_queued_work() {
        let config = test_treasury_config();
        let mut state = TreasuryState::default();
        let completed_at_unix_ms = 1_345_678;
        let idle_interval_ms = 300_000;
        state.note_payout_loop_completed(completed_at_unix_ms, None);

        assert!(!state.dispatch_cycle_due(
            &config,
            completed_at_unix_ms.saturating_add(config.payout_interval_ms()),
            idle_interval_ms,
        ));
        assert!(state.dispatch_cycle_due(
            &config,
            completed_at_unix_ms.saturating_add(idle_interval_ms),
            idle_interval_ms,
        ));

        state.payout_records_by_key.insert(
            "accepted-work:one".to_string(),
            TreasuryPayoutRecord {
                payout_key: "accepted-work:one".to_string(),
                nostr_pubkey_hex: "pubkey-a".to_string(),
                payout_target: "provider:alice".to_string(),
                amount_sats: 25,
                status: "queued".to_string(),
                reason: None,
                payment_id: None,
                window_started_at_unix_ms: 1_000,
                window_ends_at_unix_ms: 2_000,
                created_at_unix_ms: completed_at_unix_ms,
                updated_at_unix_ms: completed_at_unix_ms,
                sellable_at_window_open: true,
                dispatch_receipt_recorded: false,
                confirm_receipt_recorded: false,
                fail_receipt_recorded: false,
                skip_receipt_recorded: false,
                counted_in_paid_total: false,
                classification: TreasuryPayoutClassification {
                    payout_class: TreasuryPayoutClass::AcceptedWork,
                    accepted_outcome_id: Some("accepted.one".to_string()),
                    ..TreasuryPayoutClassification::default()
                },
            },
        );
        assert!(state.dispatch_cycle_due(
            &config,
            completed_at_unix_ms.saturating_add(1),
            idle_interval_ms,
        ));

        let record = state
            .payout_records_by_key
            .get_mut("accepted-work:one")
            .expect("queued payout record");
        record.status = "failed".to_string();
        record.reason = Some("wallet_send_retryable:leaf_selection:test".to_string());
        record.updated_at_unix_ms =
            completed_at_unix_ms.saturating_sub(super::TREASURY_FAILED_PAYOUT_RETRY_AFTER_MS);
        assert!(!state.dispatch_cycle_due(
            &config,
            completed_at_unix_ms.saturating_add(1),
            idle_interval_ms,
        ));
        assert!(state.dispatch_cycle_due(
            &config,
            completed_at_unix_ms.saturating_add(idle_interval_ms),
            idle_interval_ms,
        ));
    }

    #[test]
    fn availability_dispatch_is_suppressed_during_confirmation_stall() {
        let mut state = TreasuryState::default();
        let mut config = test_treasury_config();
        config.enabled = true;
        config.payout_sats_per_window = 25;
        let now_unix_ms = 2_000_000u64;
        let payout_interval_ms = config.payout_interval_ms();

        state.last_payout_reconciliation_at_unix_ms =
            Some(now_unix_ms.saturating_sub(payout_interval_ms));
        state.payout_targets_by_identity.insert(
            "pubkey-a".to_string(),
            super::RegisteredPayoutTarget {
                nostr_pubkey_hex: "pubkey-a".to_string(),
                source_session_id: "session-a".to_string(),
                payment_target_kind: String::new(),
                payment_target: String::new(),
                payment_target_capabilities: Vec::new(),
                pylon_payment_target_version: None,
                provider_target: "provider:alice".to_string(),
                bitcoin_address: None,
                registered_at_unix_ms: now_unix_ms.saturating_sub(10),
                last_verified_at_unix_ms: now_unix_ms.saturating_sub(10),
            },
        );
        state.payout_records_by_key.insert(
            "stalled-dispatch".to_string(),
            TreasuryPayoutRecord {
                payout_key: "1770000:availability-beneficiary:host:sha256:stalled".to_string(),
                nostr_pubkey_hex: "pubkey-stalled".to_string(),
                payout_target: "provider:stalled".to_string(),
                amount_sats: 25,
                status: "dispatched".to_string(),
                reason: None,
                payment_id: Some("payment-stalled".to_string()),
                window_started_at_unix_ms: now_unix_ms.saturating_sub(payout_interval_ms * 2),
                window_ends_at_unix_ms: now_unix_ms.saturating_sub(payout_interval_ms),
                created_at_unix_ms: now_unix_ms
                    .saturating_sub(super::TREASURY_CONFIRMATION_STALL_ALERT_THRESHOLD_MS + 60_000),
                updated_at_unix_ms: now_unix_ms
                    .saturating_sub(super::TREASURY_CONFIRMATION_STALL_ALERT_THRESHOLD_MS + 60_000),
                sellable_at_window_open: true,
                dispatch_receipt_recorded: true,
                confirm_receipt_recorded: false,
                fail_receipt_recorded: false,
                skip_receipt_recorded: false,
                counted_in_paid_total: false,
                classification: TreasuryPayoutClassification::default(),
            },
        );
        assert_eq!(
            state.availability_dispatch_suppression_reason(&config, now_unix_ms),
            None
        );

        let prepared =
            state.prepare_due_payouts(&config, &[test_online_identity("pubkey-a")], now_unix_ms);

        assert!(
            !prepared.dispatch_plans.is_empty(),
            "dispatch should continue during confirmation stalls: {:?}",
            prepared.dispatch_plans
        );
    }

    #[test]
    fn legacy_identity_scoped_availability_rows_become_attention_without_stalling_current_rail() {
        let mut state = TreasuryState::default();
        let config = test_treasury_config();
        let now_unix_ms = 2_000_000u64;

        state.last_wallet_sync_at_unix_ms = Some(now_unix_ms);
        state.wallet_balance_updated_at_unix_ms = Some(now_unix_ms);
        state.last_payout_reconciliation_at_unix_ms = Some(now_unix_ms);

        state.payout_records_by_key.insert(
            "1770000:pubkey-a".to_string(),
            TreasuryPayoutRecord {
                payout_key: "1770000:pubkey-a".to_string(),
                nostr_pubkey_hex: "pubkey-a".to_string(),
                payout_target: "provider:alice".to_string(),
                amount_sats: 25,
                status: "dispatched".to_string(),
                reason: None,
                payment_id: Some("legacy-payment-a".to_string()),
                window_started_at_unix_ms: now_unix_ms.saturating_sub(120_000),
                window_ends_at_unix_ms: now_unix_ms.saturating_sub(60_000),
                created_at_unix_ms: now_unix_ms
                    .saturating_sub(super::TREASURY_CONTINUITY_ALERT_THRESHOLD_MS + 60_000),
                updated_at_unix_ms: now_unix_ms
                    .saturating_sub(super::TREASURY_CONTINUITY_ALERT_THRESHOLD_MS + 60_000),
                sellable_at_window_open: true,
                dispatch_receipt_recorded: true,
                confirm_receipt_recorded: false,
                fail_receipt_recorded: false,
                skip_receipt_recorded: false,
                counted_in_paid_total: false,
                classification: TreasuryPayoutClassification::default(),
            },
        );

        assert_eq!(
            state.availability_dispatch_suppression_reason(&config, now_unix_ms),
            None
        );

        let stats = state.public_stats(&config, now_unix_ms);
        assert_ne!(
            stats.degraded_reason.as_deref(),
            Some("continuity_alert:confirmations_stalled")
        );
        assert_eq!(stats.pending_confirmation_count, 0);
        assert_eq!(stats.tracked_payment_backlog_count, 0);
        assert_eq!(stats.legacy_availability_confirmation_attention_count, 1);

        let status = state.status_response(&config, now_unix_ms);
        assert_eq!(status.pending_confirmation_count, 0);
        assert_eq!(status.tracked_payment_backlog_count, 0);
        assert_eq!(status.legacy_availability_confirmation_attention_count, 1);
        assert_eq!(
            status.legacy_availability_confirmation_attention_rows.len(),
            1
        );
        assert_eq!(
            status.legacy_availability_confirmation_attention_rows[0].payout_key,
            "1770000:pubkey-a"
        );
    }

    #[test]
    fn unresolved_stipend_backpressure_blocks_only_that_beneficiary() {
        let mut state = TreasuryState::default();
        let config = test_treasury_config();
        let now_unix_ms = 1_800_000u64;
        let payout_interval_ms = config.payout_interval_ms();
        for (nostr_pubkey_hex, provider_target) in
            [("pubkey-a", "provider:alice"), ("pubkey-b", "provider:bob")]
        {
            state.payout_targets_by_identity.insert(
                nostr_pubkey_hex.to_string(),
                super::RegisteredPayoutTarget {
                    nostr_pubkey_hex: nostr_pubkey_hex.to_string(),
                    source_session_id: format!("session-{nostr_pubkey_hex}"),
                    payment_target_kind: String::new(),
                    payment_target: String::new(),
                    payment_target_capabilities: Vec::new(),
                    pylon_payment_target_version: None,
                    provider_target: provider_target.to_string(),
                    bitcoin_address: None,
                    registered_at_unix_ms: 10,
                    last_verified_at_unix_ms: 10,
                },
            );
        }

        let beneficiary_key_a = "host:sha256:host-a";
        let current_window_a = payout_window_started_at_for_identity(
            now_unix_ms,
            payout_interval_ms,
            beneficiary_key_a,
        );
        let older_window_a = current_window_a.saturating_sub(payout_interval_ms);
        state.payout_records_by_key.insert(
            payout_window_key(
                older_window_a,
                availability_beneficiary_scope_key(beneficiary_key_a).as_str(),
            ),
            TreasuryPayoutRecord {
                payout_key: payout_window_key(
                    older_window_a,
                    availability_beneficiary_scope_key(beneficiary_key_a).as_str(),
                ),
                nostr_pubkey_hex: "pubkey-a".to_string(),
                payout_target: "provider:alice".to_string(),
                amount_sats: 25,
                status: "dispatched".to_string(),
                reason: None,
                payment_id: Some("payment-older-a".to_string()),
                window_started_at_unix_ms: older_window_a,
                window_ends_at_unix_ms: older_window_a.saturating_add(payout_interval_ms),
                created_at_unix_ms: older_window_a,
                updated_at_unix_ms: now_unix_ms.saturating_sub(1_000),
                sellable_at_window_open: true,
                dispatch_receipt_recorded: true,
                confirm_receipt_recorded: false,
                fail_receipt_recorded: false,
                skip_receipt_recorded: false,
                counted_in_paid_total: false,
                classification: TreasuryPayoutClassification {
                    payout_class: TreasuryPayoutClass::PlaceholderLiveness,
                    ..TreasuryPayoutClassification::default()
                },
            },
        );

        let online = vec![
            OnlinePylonIdentity {
                client_version: Some("pylon-v0.1.13".to_string()),
                host_fingerprint: Some("sha256:host-a".to_string()),
                ..test_online_identity("pubkey-a")
            },
            OnlinePylonIdentity {
                client_version: Some("pylon-v0.1.13".to_string()),
                host_fingerprint: Some("sha256:host-b".to_string()),
                ..test_online_identity("pubkey-b")
            },
        ];

        let prepared = state.prepare_due_payouts(&config, &online, now_unix_ms);
        assert_eq!(prepared.dispatch_plans.len(), 1);
        assert_eq!(prepared.dispatch_plans[0].payment_request, "provider:bob");

        let status = state.status_response(&config, now_unix_ms);
        let row_a = status
            .availability_beneficiary_debug_rows
            .iter()
            .find(|row| row.nostr_pubkey_hex == "pubkey-a")
            .expect("row a");
        assert!(!row_a.availability_stipend_eligible_now);
        assert_eq!(
            row_a.verdict_reason,
            "beneficiary_unsettled_stipend_backpressure"
        );
        assert_eq!(row_a.current_payout_status.as_deref(), Some("dispatched"));

        let row_b = status
            .availability_beneficiary_debug_rows
            .iter()
            .find(|row| row.nostr_pubkey_hex == "pubkey-b")
            .expect("row b");
        assert!(row_b.availability_stipend_eligible_now);
        assert_eq!(row_b.verdict_reason, "eligible");
    }

    #[test]
    fn beneficiary_backpressure_clears_after_old_stipend_confirms() {
        let mut state = TreasuryState::default();
        let config = test_treasury_config();
        let now_unix_ms = 1_800_000u64;
        let payout_interval_ms = config.payout_interval_ms();
        state.payout_targets_by_identity.insert(
            "pubkey-a".to_string(),
            super::RegisteredPayoutTarget {
                nostr_pubkey_hex: "pubkey-a".to_string(),
                source_session_id: "session-a".to_string(),
                payment_target_kind: String::new(),
                payment_target: String::new(),
                payment_target_capabilities: Vec::new(),
                pylon_payment_target_version: None,
                provider_target: "provider:alice".to_string(),
                bitcoin_address: None,
                registered_at_unix_ms: 10,
                last_verified_at_unix_ms: 10,
            },
        );

        let beneficiary_key_a = "host:sha256:host-a";
        let current_window_a = payout_window_started_at_for_identity(
            now_unix_ms,
            payout_interval_ms,
            beneficiary_key_a,
        );
        let older_window_a = current_window_a.saturating_sub(payout_interval_ms);
        state.payout_records_by_key.insert(
            payout_window_key(
                older_window_a,
                availability_beneficiary_scope_key(beneficiary_key_a).as_str(),
            ),
            TreasuryPayoutRecord {
                payout_key: payout_window_key(
                    older_window_a,
                    availability_beneficiary_scope_key(beneficiary_key_a).as_str(),
                ),
                nostr_pubkey_hex: "pubkey-a".to_string(),
                payout_target: "provider:alice".to_string(),
                amount_sats: 25,
                status: "confirmed".to_string(),
                reason: None,
                payment_id: Some("payment-older-a".to_string()),
                window_started_at_unix_ms: older_window_a,
                window_ends_at_unix_ms: older_window_a.saturating_add(payout_interval_ms),
                created_at_unix_ms: older_window_a,
                updated_at_unix_ms: now_unix_ms.saturating_sub(1_000),
                sellable_at_window_open: true,
                dispatch_receipt_recorded: true,
                confirm_receipt_recorded: true,
                fail_receipt_recorded: false,
                skip_receipt_recorded: false,
                counted_in_paid_total: true,
                classification: TreasuryPayoutClassification {
                    payout_class: TreasuryPayoutClass::PlaceholderLiveness,
                    ..TreasuryPayoutClassification::default()
                },
            },
        );

        let prepared = state.prepare_due_payouts(
            &config,
            &[OnlinePylonIdentity {
                client_version: Some("pylon-v0.1.13".to_string()),
                host_fingerprint: Some("sha256:host-a".to_string()),
                ..test_online_identity("pubkey-a")
            }],
            now_unix_ms,
        );

        assert_eq!(prepared.dispatch_plans.len(), 1);
        assert_eq!(prepared.dispatch_plans[0].payment_request, "provider:alice");
    }

    #[test]
    fn recovery_cutover_swaps_storage_and_updates_state() {
        let work_dir = unique_temp_dir("cutover");
        let current_storage_dir = work_dir.join("current-wallet");
        let rebuilt_storage_dir = work_dir.join("rebuilt-wallet");
        let state_path = work_dir.join("treasury-state.json");
        let report_path = work_dir.join("recovery-report.json");
        fs::create_dir_all(current_storage_dir.as_path()).expect("current dir");
        fs::create_dir_all(rebuilt_storage_dir.as_path()).expect("rebuilt dir");
        fs::write(current_storage_dir.join("storage.sql"), "current").expect("current wallet");
        fs::write(rebuilt_storage_dir.join("storage.sql"), "rebuilt").expect("rebuilt wallet");

        let mut config = test_treasury_config();
        config.state_path = state_path.clone();
        config.wallet_storage_dir = current_storage_dir.clone();

        let report = TreasuryWalletRecoveryReport {
            authority: "openagents-hosted-nexus".to_string(),
            generated_at_unix_ms: 100,
            source_wallet_storage_dir: current_storage_dir.display().to_string(),
            backup_root_dir: work_dir.join("backup").display().to_string(),
            current_storage_backup_dir: work_dir
                .join("backup/current-storage")
                .display()
                .to_string(),
            rebuilt_storage_dir: rebuilt_storage_dir.display().to_string(),
            report_path: report_path.display().to_string(),
            mnemonic_backup_path: work_dir
                .join("backup/treasury.mnemonic")
                .display()
                .to_string(),
            state_backup_path: None,
            current_storage: TreasuryWalletInspection {
                wallet_identity_pubkey: "identity".to_string(),
                inspected_storage_dir: work_dir
                    .join("backup/current-storage")
                    .display()
                    .to_string(),
                balance_sats: Some(0),
                ..TreasuryWalletInspection::default()
            },
            rebuilt_storage: TreasuryWalletInspection {
                wallet_identity_pubkey: "identity".to_string(),
                inspected_storage_dir: rebuilt_storage_dir.display().to_string(),
                runtime_status: Some("connected".to_string()),
                balance_sats: Some(80_000),
                ..TreasuryWalletInspection::default()
            },
            comparison: TreasuryWalletRecoveryComparison {
                wallet_identity_pubkey_match: true,
                rebuilt_minus_current_balance_sats: Some(80_000),
                current_zero_with_receive_history: true,
                major_divergence_detected: true,
                validation_passed: true,
                recommended_action: "cutover_rebuilt_storage_after_service_stop".to_string(),
            },
            cutover_active_storage_dir: None,
            cutover_rollback_storage_dir: None,
            cutover_completed_at_unix_ms: None,
        };
        write_json_file(report_path.as_path(), &report).expect("report file");

        let response = apply_treasury_wallet_recovery_cutover(&config, report_path.as_path())
            .expect("cutover");
        assert_eq!(response.wallet_storage_runtime_mode, "rebuilt");
        assert!(config.wallet_storage_dir.join("storage.sql").exists());
        assert_eq!(
            fs::read_to_string(config.wallet_storage_dir.join("storage.sql")).expect("active"),
            "rebuilt"
        );
        assert!(
            PathBuf::from(response.rollback_storage_dir.as_str())
                .join("storage.sql")
                .exists()
        );
        assert_eq!(
            fs::read_to_string(
                PathBuf::from(response.rollback_storage_dir.as_str()).join("storage.sql")
            )
            .expect("rollback"),
            "current"
        );

        let state = TreasuryState::new(state_path.clone());
        assert_eq!(
            state.wallet_storage_runtime_mode.as_deref(),
            Some("rebuilt")
        );
        assert_eq!(state.wallet_balance_sats, 80_000);
        assert_eq!(
            state
                .last_wallet_recovery_report
                .as_ref()
                .map(|summary| summary.validation_passed),
            Some(true)
        );

        let updated_report = fs::read_to_string(report_path.as_path()).expect("updated report");
        let parsed_report: TreasuryWalletRecoveryReport =
            serde_json::from_str(updated_report.as_str()).expect("report json");
        assert_eq!(
            parsed_report.cutover_active_storage_dir.as_deref(),
            Some(config.wallet_storage_dir.display().to_string().as_str())
        );
        assert!(parsed_report.cutover_completed_at_unix_ms.is_some());
    }

    #[test]
    fn recovery_cutover_requires_explicit_cutover_recommendation() {
        let work_dir = unique_temp_dir("cutover-rejected");
        let current_storage_dir = work_dir.join("current-wallet");
        let rebuilt_storage_dir = work_dir.join("rebuilt-wallet");
        let report_path = work_dir.join("recovery-report.json");
        fs::create_dir_all(current_storage_dir.as_path()).expect("current dir");
        fs::create_dir_all(rebuilt_storage_dir.as_path()).expect("rebuilt dir");
        fs::write(current_storage_dir.join("storage.sql"), "current").expect("current wallet");
        fs::write(rebuilt_storage_dir.join("storage.sql"), "rebuilt").expect("rebuilt wallet");

        let mut config = test_treasury_config();
        config.wallet_storage_dir = current_storage_dir.clone();

        let report = TreasuryWalletRecoveryReport {
            authority: "openagents-hosted-nexus".to_string(),
            generated_at_unix_ms: 100,
            source_wallet_storage_dir: current_storage_dir.display().to_string(),
            backup_root_dir: work_dir.join("backup").display().to_string(),
            current_storage_backup_dir: work_dir
                .join("backup/current-storage")
                .display()
                .to_string(),
            rebuilt_storage_dir: rebuilt_storage_dir.display().to_string(),
            report_path: report_path.display().to_string(),
            mnemonic_backup_path: work_dir
                .join("backup/treasury.mnemonic")
                .display()
                .to_string(),
            state_backup_path: None,
            current_storage: TreasuryWalletInspection {
                wallet_identity_pubkey: "identity".to_string(),
                inspected_storage_dir: work_dir
                    .join("backup/current-storage")
                    .display()
                    .to_string(),
                runtime_status: Some("cached_after_sync_timeout".to_string()),
                balance_sats: Some(80),
                ..TreasuryWalletInspection::default()
            },
            rebuilt_storage: TreasuryWalletInspection {
                wallet_identity_pubkey: "identity".to_string(),
                inspected_storage_dir: rebuilt_storage_dir.display().to_string(),
                runtime_status: Some("cached_after_sync_timeout".to_string()),
                balance_sats: Some(80),
                ..TreasuryWalletInspection::default()
            },
            comparison: TreasuryWalletRecoveryComparison {
                wallet_identity_pubkey_match: true,
                rebuilt_minus_current_balance_sats: Some(0),
                current_zero_with_receive_history: false,
                major_divergence_detected: false,
                validation_passed: true,
                recommended_action: "no_cutover_needed_sync_timeout_cached".to_string(),
            },
            cutover_active_storage_dir: None,
            cutover_rollback_storage_dir: None,
            cutover_completed_at_unix_ms: None,
        };
        write_json_file(report_path.as_path(), &report).expect("report file");

        let error = apply_treasury_wallet_recovery_cutover(&config, report_path.as_path())
            .expect_err("cached timeout report must not cut over");
        assert!(
            error
                .to_string()
                .contains("does not recommend cutover: no_cutover_needed_sync_timeout_cached")
        );
        assert_eq!(
            fs::read_to_string(current_storage_dir.join("storage.sql")).expect("current"),
            "current"
        );
        assert_eq!(
            fs::read_to_string(rebuilt_storage_dir.join("storage.sql")).expect("rebuilt"),
            "rebuilt"
        );
    }

    #[test]
    fn treasury_state_load_preserves_paid_total_when_deserialize_fails() {
        let work_dir = unique_temp_dir("state-recovery");
        fs::create_dir_all(work_dir.as_path()).expect("work dir");
        let state_path = work_dir.join("treasury-state.json");
        fs::write(
            state_path.as_path(),
            r#"{
  "payout_sats_paid_total": 139813,
  "public_snapshot": {
    "generated_at_unix_ms": 1775887507000,
    "payout_sats_paid_total": 139821
  },
  "next_challenge_nonce": "oops"
}
"#,
        )
        .expect("state file");

        let config = test_treasury_config();
        let state = TreasuryState::new(state_path);
        let stats = state.public_stats(&config, 1_775_887_507_001);

        assert_eq!(state.payout_sats_paid_total, 139821);
        assert_eq!(stats.payout_sats_paid_total, 139821);
        assert_eq!(state.wallet_runtime_status.as_deref(), Some("error"));
        assert!(
            state
                .wallet_last_error
                .as_deref()
                .is_some_and(|detail| detail.contains("treasury_state_deserialize_failed"))
        );
        assert_eq!(state.payout_loop_runtime_status.as_deref(), Some("error"));
        assert_eq!(state.next_challenge_nonce, 1);
        assert!(state.public_snapshot.is_none());
    }

    #[test]
    fn stale_dispatching_records_stop_blocking_budget() {
        let mut state = TreasuryState::default();
        let mut config = test_treasury_config();
        config.daily_budget_cap_sats = config.payout_sats_per_window;

        state.payout_targets_by_identity.insert(
            "pubkey-a".to_string(),
            super::RegisteredPayoutTarget {
                nostr_pubkey_hex: "pubkey-a".to_string(),
                source_session_id: "session-a".to_string(),
                payment_target_kind: String::new(),
                payment_target: String::new(),
                payment_target_capabilities: Vec::new(),
                pylon_payment_target_version: None,
                provider_target: "provider:alice".to_string(),
                bitcoin_address: None,
                registered_at_unix_ms: 10,
                last_verified_at_unix_ms: 10,
            },
        );

        let now_unix_ms = super::now_unix_ms();
        let stale_window_started_at_unix_ms = payout_window_started_at(
            now_unix_ms.saturating_sub(
                config.dispatch_result_timeout_ms(config.payout_interval_ms()) + 5_000,
            ),
            config.payout_interval_ms(),
        );
        let stale_payout_key = format!("{stale_window_started_at_unix_ms}:pubkey-stale");
        state.payout_records_by_key.insert(
            stale_payout_key.clone(),
            super::TreasuryPayoutRecord {
                payout_key: stale_payout_key.clone(),
                nostr_pubkey_hex: "pubkey-stale".to_string(),
                payout_target: "provider:stale".to_string(),
                amount_sats: config.payout_sats_per_window,
                status: "dispatching".to_string(),
                reason: None,
                payment_id: None,
                window_started_at_unix_ms: stale_window_started_at_unix_ms,
                window_ends_at_unix_ms: stale_window_started_at_unix_ms
                    + config.payout_interval_ms(),
                created_at_unix_ms: stale_window_started_at_unix_ms,
                updated_at_unix_ms: now_unix_ms.saturating_sub(
                    config.dispatch_result_timeout_ms(config.payout_interval_ms()) + 1,
                ),
                sellable_at_window_open: true,
                dispatch_receipt_recorded: false,
                confirm_receipt_recorded: false,
                fail_receipt_recorded: false,
                skip_receipt_recorded: false,
                counted_in_paid_total: false,
                classification: TreasuryPayoutClassification::default(),
            },
        );

        let prepared =
            state.prepare_due_payouts(&config, &[test_online_identity("pubkey-a")], now_unix_ms);

        assert_eq!(prepared.dispatch_plans.len(), 1);
        assert_eq!(prepared.receipt_events.len(), 1);
        assert_eq!(
            prepared.receipt_events[0].receipt_type,
            "treasury.payout.failed"
        );
        assert_eq!(
            state
                .payout_records_by_key
                .get(stale_payout_key.as_str())
                .and_then(|record| record.reason.as_deref()),
            Some("dispatch_outcome_timeout")
        );
    }

    #[test]
    fn dispatch_result_timeout_stays_bounded_when_payout_interval_increases() {
        let mut config = test_treasury_config();
        config.payout_interval_seconds = 600;
        config.wallet_status_refresh_seconds = 30;

        assert_eq!(
            config.dispatch_result_timeout_ms(config.payout_interval_ms()),
            60_000
        );

        config.wallet_status_refresh_seconds = 300;

        assert_eq!(
            config.dispatch_result_timeout_ms(config.payout_interval_ms()),
            60_000
        );
    }

    #[tokio::test]
    async fn funding_and_dispatch_hooks_cover_happy_path() {
        let _lock = treasury_test_hook_lock().lock().expect("guard");
        let config = test_treasury_config();

        set_test_wallet_funding_hook(Some(Arc::new(|request| {
            Box::pin(async move {
                assert_eq!(request.amount_sats, Some(210));
                Ok(TreasuryFundingMaterial {
                    provider_target: "ldk://server/regtest/bitcoind/test".to_string(),
                    bitcoin_address: String::new(),
                    provider_invoice: None,
                    bolt11_invoice: Some("lnbc210fund".to_string()),
                    provider_payment_id: Some("ldk-funding-target-test".to_string()),
                    phase_timings: TreasuryFundingTargetPhaseTimings::default(),
                    wallet_snapshot: TreasuryWalletSnapshot {
                        runtime_status: "connected".to_string(),
                        runtime_detail: None,
                        wallet_hydration_mode: None,
                        wallet_payment_scan_mode: None,
                        balance_sats: 500,
                        total_onchain_balance_sats: 500,
                        spendable_onchain_balance_sats: 500,
                        lightning_balance_sats: 0,
                        payments: Vec::new(),
                    },
                })
            })
        })));
        let funding = create_live_funding_target(
            &config,
            TreasuryFundingTargetRequest {
                amount_sats: Some(210),
                description: Some("fund treasury".to_string()),
                expiry_seconds: Some(60),
            },
        )
        .await
        .expect("funding target should build");
        assert_eq!(
            funding.provider_target,
            "ldk://server/regtest/bitcoind/test"
        );
        assert_eq!(funding.bolt11_invoice.as_deref(), Some("lnbc210fund"));
        set_test_wallet_funding_hook(None);

        set_test_wallet_send_hook(Some(Arc::new(|target, amount_sats| {
            assert_eq!(target, "lnbcrt120alice");
            assert_eq!(amount_sats, 120);
            Ok("payment-send-001".to_string())
        })));
        set_test_wallet_snapshot_hook(Some(Arc::new(|| {
            Ok(TreasuryWalletSnapshot {
                runtime_status: "connected".to_string(),
                runtime_detail: None,
                wallet_hydration_mode: None,
                wallet_payment_scan_mode: None,
                balance_sats: 380,
                total_onchain_balance_sats: 380,
                spendable_onchain_balance_sats: 380,
                lightning_balance_sats: 0,
                payments: vec![PaymentSummary {
                    id: "payment-send-001".to_string(),
                    direction: "send".to_string(),
                    status: "completed".to_string(),
                    amount_sats: 120,
                    fees_sats: 0,
                    timestamp: 300,
                    method: "ldk".to_string(),
                    description: None,
                    invoice: Some("lnbcrt120alice".to_string()),
                    destination_pubkey: None,
                    payment_hash: None,
                    htlc_status: None,
                    htlc_expiry_epoch_seconds: None,
                    status_detail: None,
                }],
            })
        })));

        let batch = dispatch_live_payouts(
            &config,
            &[super::TreasuryDispatchPlan {
                payout_key: "window-a:pubkey-a".to_string(),
                payment_request: "lnbcrt120alice".to_string(),
                amount_sats: 120,
                classification: TreasuryPayoutClassification::default(),
            }],
        )
        .await;
        assert_eq!(batch.outcomes.len(), 1);
        assert!(matches!(
            batch.outcomes[0],
            TreasuryDispatchOutcome::Dispatched { .. }
        ));
        assert_eq!(
            batch.wallet_snapshot.as_ref().map(|row| row.balance_sats),
            Some(380)
        );

        set_test_wallet_send_hook(None);
        set_test_wallet_snapshot_hook(None);
    }

    #[tokio::test]
    async fn simulated_wallet_covers_local_proof_funding_and_dispatch() {
        let mut config = test_treasury_config();
        config.simulated_wallet_enabled = true;
        config.simulated_wallet_balance_sats = 42_000;

        let funding = create_live_funding_target(
            &config,
            TreasuryFundingTargetRequest {
                amount_sats: Some(210),
                description: Some("fund simulated treasury".to_string()),
                expiry_seconds: Some(60),
            },
        )
        .await
        .expect("simulated funding target");
        assert_eq!(
            funding.provider_target,
            "provider:simulated-treasury-proof-wallet"
        );
        assert_eq!(
            funding.bolt11_invoice.as_deref(),
            Some("lnbc210simulatedproofwallet")
        );
        assert_eq!(funding.wallet_snapshot.runtime_status, "connected");
        assert_eq!(funding.wallet_snapshot.balance_sats, 42_000);

        let batch = dispatch_live_payouts(
            &config,
            &[super::TreasuryDispatchPlan {
                payout_key: "window-a:pubkey-a".to_string(),
                payment_request: "provider:alice".to_string(),
                amount_sats: 120,
                classification: TreasuryPayoutClassification::default(),
            }],
        )
        .await;
        assert_eq!(batch.outcomes.len(), 1);
        assert!(matches!(
            batch.outcomes[0],
            TreasuryDispatchOutcome::Dispatched { .. }
        ));
        let snapshot = batch.wallet_snapshot.expect("simulated wallet snapshot");
        assert_eq!(snapshot.runtime_status, "connected");
        assert_eq!(
            snapshot.wallet_hydration_mode.as_deref(),
            Some("simulated_proof_wallet")
        );
        assert_eq!(snapshot.payments.len(), 1);
        assert_eq!(snapshot.payments[0].direction, "send");
        assert_eq!(snapshot.payments[0].status, "completed");
        assert_eq!(snapshot.payments[0].amount_sats, 120);
    }

    #[tokio::test]
    async fn dispatch_send_timeout_returns_failed_outcome() {
        let outcome = super::dispatch_outcome_from_send_future(
            super::TreasuryDispatchPlan {
                payout_key: "window-a:pubkey-a".to_string(),
                payment_request: "provider:alice".to_string(),
                amount_sats: 120,
                classification: TreasuryPayoutClassification::default(),
            },
            5,
            async {
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                Ok::<_, anyhow::Error>("payment-send-001".to_string())
            },
        )
        .await;

        assert!(matches!(
            outcome,
            TreasuryDispatchOutcome::Failed { ref reason, .. }
                if reason == "wallet_send_timeout:5"
        ));
    }

    #[test]
    fn wallet_open_timeout_remains_retryable_for_accepted_work() {
        assert!(super::failed_payout_reason_is_retryable(
            "wallet_open_timeout:60000"
        ));
    }

    #[test]
    fn max_concurrent_send_operations_clamps_to_configured_limit() {
        let mut config = test_treasury_config();
        config.max_concurrent_sends = 16;

        assert_eq!(config.max_concurrent_send_operations(0), 1);
        assert_eq!(config.max_concurrent_send_operations(1), 1);
        assert_eq!(config.max_concurrent_send_operations(4), 4);
        assert_eq!(config.max_concurrent_send_operations(16), 16);
        assert_eq!(config.max_concurrent_send_operations(128), 16);
        assert_eq!(
            config.max_concurrent_send_operations_for_class(16, TreasuryPayoutClass::AcceptedWork),
            super::TREASURY_MAX_CONCURRENT_ACCEPTED_WORK_SENDS
        );
        assert_eq!(
            config.max_concurrent_send_operations_for_class(
                128,
                TreasuryPayoutClass::PlaceholderLiveness
            ),
            16
        );
    }

    #[test]
    fn wallet_send_failure_classification_marks_retryable_transport_failures() {
        assert_eq!(
            super::classify_wallet_send_failure(
                "ProviderSdkError: Service error: service connection error: Connection error: status: Cancelled, message: \"operation was canceled\""
            ),
            "wallet_send_retryable:cancelled_transport:ProviderSdkError: Service error: service connection error: Connection error: status: Cancelled, message: \"operation was canceled\""
        );
        assert_eq!(
            super::classify_wallet_send_failure("TreeServiceError(InsufficientFunds)"),
            "wallet_send_retryable:leaf_selection:TreeServiceError(InsufficientFunds)"
        );
        assert_eq!(
            super::classify_wallet_send_failure("some permanent failure"),
            "wallet_send_failed:unknown:some permanent failure"
        );
    }

    #[test]
    fn retryable_failed_backlog_counts_exclude_non_retryable_and_non_ldk_failures() {
        let mut state = TreasuryState::default();
        state.payout_records_by_key.insert(
            "retryable".to_string(),
            TreasuryPayoutRecord {
                payout_key: "retryable".to_string(),
                nostr_pubkey_hex: "pubkey-a".to_string(),
                payout_target: "lno1pylonalice".to_string(),
                amount_sats: 25,
                status: "failed".to_string(),
                reason: Some("wallet_send_retryable:transport:boom".to_string()),
                payment_id: None,
                window_started_at_unix_ms: 1_000,
                window_ends_at_unix_ms: 2_000,
                created_at_unix_ms: 1_000,
                updated_at_unix_ms: 1_000,
                sellable_at_window_open: true,
                dispatch_receipt_recorded: false,
                confirm_receipt_recorded: false,
                fail_receipt_recorded: true,
                skip_receipt_recorded: false,
                counted_in_paid_total: false,
                classification: TreasuryPayoutClassification::default(),
            },
        );
        state.payout_records_by_key.insert(
            "non-retryable".to_string(),
            TreasuryPayoutRecord {
                payout_key: "non-retryable".to_string(),
                nostr_pubkey_hex: "pubkey-b".to_string(),
                payout_target: "provider:bob".to_string(),
                amount_sats: 25,
                status: "failed".to_string(),
                reason: Some("wallet_send_failed:unknown:nope".to_string()),
                payment_id: None,
                window_started_at_unix_ms: 1_000,
                window_ends_at_unix_ms: 2_000,
                created_at_unix_ms: 1_000,
                updated_at_unix_ms: 1_000,
                sellable_at_window_open: true,
                dispatch_receipt_recorded: false,
                confirm_receipt_recorded: false,
                fail_receipt_recorded: true,
                skip_receipt_recorded: false,
                counted_in_paid_total: false,
                classification: TreasuryPayoutClassification::default(),
            },
        );
        state.payout_records_by_key.insert(
            "historical-non-ldk".to_string(),
            TreasuryPayoutRecord {
                payout_key: "historical-non-ldk".to_string(),
                nostr_pubkey_hex: "pubkey-c".to_string(),
                payout_target: "provider:carol".to_string(),
                amount_sats: 25,
                status: "failed".to_string(),
                reason: Some("wallet_send_retryable:transport:historical".to_string()),
                payment_id: None,
                window_started_at_unix_ms: 1_000,
                window_ends_at_unix_ms: 2_000,
                created_at_unix_ms: 1_000,
                updated_at_unix_ms: 1_000,
                sellable_at_window_open: true,
                dispatch_receipt_recorded: false,
                confirm_receipt_recorded: false,
                fail_receipt_recorded: true,
                skip_receipt_recorded: false,
                counted_in_paid_total: false,
                classification: TreasuryPayoutClassification::default(),
            },
        );

        let (backlog_total, backlog_retryable) = state.backlog_counts();
        assert_eq!(backlog_total, 3);
        assert_eq!(backlog_retryable, 1);
    }

    #[test]
    fn wallet_refresh_plan_tracks_only_unconfirmed_dispatched_payment_ids() {
        let mut state = TreasuryState::new(PathBuf::from("var/test-treasury-state.json"));
        state.payout_records_by_key.insert(
            "window-a:pubkey-a".to_string(),
            TreasuryPayoutRecord {
                payout_key: "window-a:pubkey-a".to_string(),
                nostr_pubkey_hex: "pubkey-a".to_string(),
                payout_target: "provider:alice".to_string(),
                amount_sats: 2,
                status: "dispatched".to_string(),
                reason: None,
                payment_id: Some("pay-confirm-me".to_string()),
                window_started_at_unix_ms: 100,
                window_ends_at_unix_ms: 200,
                created_at_unix_ms: 100,
                updated_at_unix_ms: 200,
                sellable_at_window_open: true,
                dispatch_receipt_recorded: true,
                confirm_receipt_recorded: false,
                fail_receipt_recorded: false,
                skip_receipt_recorded: false,
                counted_in_paid_total: false,
                classification: TreasuryPayoutClassification::default(),
            },
        );
        state.payout_records_by_key.insert(
            "window-b:pubkey-b".to_string(),
            TreasuryPayoutRecord {
                payout_key: "window-b:pubkey-b".to_string(),
                nostr_pubkey_hex: "pubkey-b".to_string(),
                payout_target: "provider:bob".to_string(),
                amount_sats: 2,
                status: "confirmed".to_string(),
                reason: None,
                payment_id: Some("pay-already-confirmed".to_string()),
                window_started_at_unix_ms: 300,
                window_ends_at_unix_ms: 400,
                created_at_unix_ms: 300,
                updated_at_unix_ms: 400,
                sellable_at_window_open: true,
                dispatch_receipt_recorded: true,
                confirm_receipt_recorded: true,
                fail_receipt_recorded: false,
                skip_receipt_recorded: false,
                counted_in_paid_total: true,
                classification: TreasuryPayoutClassification::default(),
            },
        );
        state.payout_records_by_key.insert(
            "window-c:pubkey-c".to_string(),
            TreasuryPayoutRecord {
                payout_key: "window-c:pubkey-c".to_string(),
                nostr_pubkey_hex: "pubkey-c".to_string(),
                payout_target: "provider:carol".to_string(),
                amount_sats: 2,
                status: "dispatching".to_string(),
                reason: None,
                payment_id: None,
                window_started_at_unix_ms: 500,
                window_ends_at_unix_ms: 600,
                created_at_unix_ms: 500,
                updated_at_unix_ms: 600,
                sellable_at_window_open: true,
                dispatch_receipt_recorded: false,
                confirm_receipt_recorded: false,
                fail_receipt_recorded: false,
                skip_receipt_recorded: false,
                counted_in_paid_total: false,
                classification: TreasuryPayoutClassification::default(),
            },
        );

        let plan = state.wallet_refresh_plan();

        assert_eq!(plan.tracked_payment_count(), 1);
        assert!(plan.tracked_payment_ids.contains("pay-confirm-me"));
        assert!(!plan.tracked_payment_ids.contains("pay-already-confirmed"));
        assert_eq!(plan.history_scan_page_offset, 0);
    }

    #[test]
    fn wallet_refresh_payment_page_budget_stays_bounded() {
        assert_eq!(wallet_refresh_payment_page_budget(0), 1);
        assert_eq!(wallet_refresh_payment_page_budget(1), 2);
        assert_eq!(wallet_refresh_payment_page_budget(100), 2);
        assert_eq!(wallet_refresh_payment_page_budget(101), 3);
        assert_eq!(
            wallet_refresh_payment_page_budget(
                TREASURY_WALLET_REFRESH_PAYMENT_PAGE_SIZE
                    * TREASURY_WALLET_REFRESH_MAX_PAYMENT_PAGES
            ),
            TREASURY_WALLET_REFRESH_MAX_PAYMENT_PAGES
        );
        assert_eq!(
            wallet_refresh_payment_page_budget(10_000),
            TREASURY_WALLET_REFRESH_MAX_PAYMENT_PAGES
        );
    }

    #[test]
    fn wallet_refresh_page_offsets_follow_the_history_cursor() {
        let mut plan = TreasuryWalletRefreshPlan::recent_only();
        plan.track_payment_id("pay-confirm-me");
        plan.history_scan_page_offset = 8;

        let page_offsets = wallet_refresh_page_offsets(&plan);

        assert_eq!(page_offsets[0], 0);
        assert_eq!(page_offsets[1], 8);
        assert_eq!(
            page_offsets.len(),
            TREASURY_WALLET_REFRESH_CURSOR_PAYMENT_PAGES
        );
    }

    #[test]
    fn track_wallet_refresh_payment_deduplicates_and_clears_tracked_ids() {
        let mut payments = Vec::new();
        let mut seen_payment_ids = BTreeSet::new();
        let mut unresolved_payment_ids = BTreeSet::from([
            "pay-confirm-me".to_string(),
            "pay-still-pending".to_string(),
        ]);
        let payment = PaymentSummary {
            id: "pay-confirm-me".to_string(),
            direction: "send".to_string(),
            status: "completed".to_string(),
            amount_sats: 25,
            timestamp: 1_777_000_000,
            ..PaymentSummary::default()
        };

        assert!(track_wallet_refresh_payment(
            &mut payments,
            &mut seen_payment_ids,
            &mut unresolved_payment_ids,
            payment.clone(),
        ));
        assert_eq!(payments.len(), 1);
        assert!(!unresolved_payment_ids.contains("pay-confirm-me"));

        assert!(!track_wallet_refresh_payment(
            &mut payments,
            &mut seen_payment_ids,
            &mut unresolved_payment_ids,
            payment,
        ));
        assert_eq!(payments.len(), 1);
        assert!(unresolved_payment_ids.contains("pay-still-pending"));
    }

    #[test]
    fn wallet_refresh_plan_marks_funded_history_as_nonzero_expected() {
        let mut state = TreasuryState::default();
        state.payout_sats_paid_total = 100;
        state.funding_receives_by_payment_id.insert(
            "receive-001".to_string(),
            TreasuryFundingReceive {
                payment_id: "receive-001".to_string(),
                status: "completed".to_string(),
                amount_sats: TREASURY_IMPOSSIBLE_ZERO_BALANCE_THRESHOLD_SATS + 1_500,
                method: "provider".to_string(),
                description: Some("fund treasury".to_string()),
                recorded_at_unix_ms: 10,
                updated_at_unix_ms: 10,
            },
        );

        let plan = state.wallet_refresh_plan();

        assert!(plan.expects_funded_balance());
        assert_eq!(
            plan.historical_receive_total_sats,
            TREASURY_IMPOSSIBLE_ZERO_BALANCE_THRESHOLD_SATS + 1_500
        );
        assert_eq!(plan.payout_sats_paid_total, 100);
    }

    #[test]
    fn validate_wallet_hydration_balance_rejects_zero_when_funded_history_exists() {
        let mut plan = TreasuryWalletRefreshPlan::recent_only();
        plan.expected_nonzero_balance = true;
        plan.historical_receive_total_sats = 2_500;
        plan.payout_sats_paid_total = 100;

        let error = validate_wallet_hydration_balance(&plan, 0, "sync_wallet_then_cached_balance")
            .expect_err("zero balance should fail when funded history exists")
            .to_string();

        assert!(error.contains(
            "wallet_hydration_zero_balance_after_sync_wallet_then_cached_balance:2500:100"
        ));
        assert!(
            validate_wallet_hydration_balance(&plan, 1, "sync_wallet_then_cached_balance").is_ok()
        );
    }

    #[test]
    fn wallet_refresh_progress_advances_history_cursor_while_backlog_remains() {
        let mut state = TreasuryState::default();
        state.payout_records_by_key.insert(
            "window-a:pubkey-a".to_string(),
            TreasuryPayoutRecord {
                payout_key: "window-a:pubkey-a".to_string(),
                nostr_pubkey_hex: "pubkey-a".to_string(),
                payout_target: "provider:alice".to_string(),
                amount_sats: 2,
                status: "dispatched".to_string(),
                reason: None,
                payment_id: Some("pay-confirm-me".to_string()),
                window_started_at_unix_ms: 100,
                window_ends_at_unix_ms: 200,
                created_at_unix_ms: 100,
                updated_at_unix_ms: 200,
                sellable_at_window_open: true,
                dispatch_receipt_recorded: true,
                confirm_receipt_recorded: false,
                fail_receipt_recorded: false,
                skip_receipt_recorded: false,
                counted_in_paid_total: false,
                classification: TreasuryPayoutClassification::default(),
            },
        );

        let plan = state.wallet_refresh_plan();
        state.note_wallet_refresh_progress(
            &plan,
            &TreasuryWalletRefreshProgress {
                history_scan_page_offset: 1,
                history_pages_scanned: 7,
                history_hit_end_of_history: false,
            },
        );

        assert_eq!(state.wallet_refresh_history_page_offset, 8);
    }

    #[test]
    fn wallet_refresh_progress_resets_history_cursor_when_backlog_clears() {
        let mut state = TreasuryState::default();
        state.wallet_refresh_history_page_offset = 12;
        let plan = state.wallet_refresh_plan();

        state.note_wallet_refresh_progress(
            &plan,
            &TreasuryWalletRefreshProgress {
                history_scan_page_offset: 12,
                history_pages_scanned: 4,
                history_hit_end_of_history: false,
            },
        );

        assert_eq!(state.wallet_refresh_history_page_offset, 0);
    }

    #[test]
    fn wallet_refresh_progress_restarts_history_scan_after_reaching_the_end() {
        let mut state = TreasuryState::default();
        state.payout_records_by_key.insert(
            "window-a:pubkey-a".to_string(),
            TreasuryPayoutRecord {
                payout_key: "window-a:pubkey-a".to_string(),
                nostr_pubkey_hex: "pubkey-a".to_string(),
                payout_target: "provider:alice".to_string(),
                amount_sats: 2,
                status: "dispatched".to_string(),
                reason: None,
                payment_id: Some("pay-confirm-me".to_string()),
                window_started_at_unix_ms: 100,
                window_ends_at_unix_ms: 200,
                created_at_unix_ms: 100,
                updated_at_unix_ms: 200,
                sellable_at_window_open: true,
                dispatch_receipt_recorded: true,
                confirm_receipt_recorded: false,
                fail_receipt_recorded: false,
                skip_receipt_recorded: false,
                counted_in_paid_total: false,
                classification: TreasuryPayoutClassification::default(),
            },
        );
        state.wallet_refresh_history_page_offset = 24;
        let plan = state.wallet_refresh_plan();

        state.note_wallet_refresh_progress(
            &plan,
            &TreasuryWalletRefreshProgress {
                history_scan_page_offset: 24,
                history_pages_scanned: 3,
                history_hit_end_of_history: true,
            },
        );

        assert_eq!(
            state.wallet_refresh_history_page_offset,
            TREASURY_WALLET_REFRESH_RECENT_PAYMENT_PAGES
        );
    }
}
