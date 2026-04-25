use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::future::Future;
use std::path::{Path, PathBuf};
#[cfg(test)]
use std::pin::Pin;
use std::sync::{Arc, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[cfg(test)]
use std::sync::Mutex;

use anyhow::{Context, Result, anyhow, bail};
use bip39::{Language, Mnemonic};
use futures::stream::{self, StreamExt};
use openagents_provider_substrate::verify_provider_payout_target_registration_signature;
use openagents_spark::{
    DepositClaimFeePolicy, Network as SparkNetwork, PaymentSummary, SparkSigner, SparkWallet,
    WalletConfig,
};
use semver::Version;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::sync::Mutex as AsyncMutex;

use crate::economy::AuthorityReceiptContext;

const DEFAULT_OPENAGENTS_SPARK_API_KEY: &str = "MIIBfjCCATCgAwIBAgIHPYzgGw0A+zAFBgMrZXAwEDEOMAwGA1UEAxMFQnJlZXowHhcNMjQxMTI0MjIxOTMzWhcNMzQxMTIyMjIxOTMzWjA3MRkwFwYDVQQKExBPcGVuQWdlbnRzLCBJbmMuMRowGAYDVQQDExFDaHJpc3RvcGhlciBEYXZpZDAqMAUGAytlcAMhANCD9cvfIDwcoiDKKYdT9BunHLS2/OuKzV8NS0SzqV13o4GBMH8wDgYDVR0PAQH/BAQDAgWgMAwGA1UdEwEB/wQCMAAwHQYDVR0OBBYEFNo5o+5ea0sNMlW/75VgGJCv2AcJMB8GA1UdIwQYMBaAFN6q1pJW843ndJIW/Ey2ILJrKJhrMB8GA1UdEQQYMBaBFGNocmlzQG9wZW5hZ2VudHMuY29tMAUGAytlcANBABvQIfNsop0kGIk0bgO/2kPum5B5lv6pYaSBXz73G1RV+eZj/wuW88lNQoGwVER+rA9+kWWTaR/dpdi8AFwjxw0=";

const ENV_TREASURY_STATE_PATH: &str = "NEXUS_CONTROL_TREASURY_STATE_PATH";
const ENV_TREASURY_ENABLED: &str = "NEXUS_CONTROL_TREASURY_ENABLED";
const ENV_TREASURY_PAYOUT_SATS_PER_WINDOW: &str = "NEXUS_CONTROL_TREASURY_PAYOUT_SATS_PER_WINDOW";
const ENV_TREASURY_PAYOUT_INTERVAL_SECONDS: &str = "NEXUS_CONTROL_TREASURY_PAYOUT_INTERVAL_SECONDS";
const ENV_TREASURY_REQUIRE_SELLABLE: &str = "NEXUS_CONTROL_TREASURY_REQUIRE_SELLABLE";
const ENV_TREASURY_DAILY_BUDGET_CAP_SATS: &str = "NEXUS_CONTROL_TREASURY_DAILY_BUDGET_CAP_SATS";
const ENV_TREASURY_PLACEHOLDER_PAYOUT_MODE: &str = "NEXUS_CONTROL_TREASURY_PLACEHOLDER_PAYOUT_MODE";
const ENV_TREASURY_DEDUPE_PLACEHOLDER_HOSTS: &str =
    "NEXUS_CONTROL_TREASURY_DEDUPE_PLACEHOLDER_HOSTS";
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

const DEFAULT_TREASURY_STATE_PATH: &str = "var/nexus-control/treasury-state.json";
const DEFAULT_TREASURY_ENABLED: bool = false;
const DEFAULT_TREASURY_PAYOUT_SATS_PER_WINDOW: u64 = 0;
const DEFAULT_TREASURY_PAYOUT_INTERVAL_SECONDS: u64 = 3_600;
const DEFAULT_TREASURY_REQUIRE_SELLABLE: bool = false;
const DEFAULT_TREASURY_DAILY_BUDGET_CAP_SATS: u64 = 21_000;
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
const DEFAULT_TREASURY_SIMULATED_WALLET_ENABLED: bool = false;
const DEFAULT_TREASURY_SIMULATED_WALLET_BALANCE_SATS: u64 = 1_000_000;
const DEFAULT_TREASURY_MAX_CONCURRENT_SENDS: usize = 16;
const DEFAULT_TREASURY_RECONCILIATION_HORIZON_SECONDS: u64 = 86_400;
const DEFAULT_TREASURY_POLICY_APPLY_ENV: bool = false;
const DEFAULT_TREASURY_POLICY_ALLOW_DESTRUCTIVE_ENV_CHANGE: bool = false;
const DEFAULT_TREASURY_REGISTRATION_CHALLENGE_TTL_SECONDS: u64 = 300;
const TREASURY_PUBLIC_STATS_WINDOW_MS: u64 = 86_400_000;
const TREASURY_PAYOUT_TARGET_DOMAIN: &str = "openagents:nexus-treasury-payout-target:v1";
const TREASURY_POLICY_SCHEMA_VERSION: u32 = 3;
const TREASURY_STATE_RETENTION_WINDOW_MS: u64 = 30 * 86_400_000;
const TREASURY_DISPATCH_RESULT_TIMEOUT_MS: u64 = 60_000;
const TREASURY_FAILED_ACCEPTED_WORK_RETRY_AFTER_MS: u64 = TREASURY_DISPATCH_RESULT_TIMEOUT_MS;
const TREASURY_TARGET_LIMIT: usize = 8_192;
const TREASURY_PAYOUT_LIMIT: usize = 262_144;
const TREASURY_PLACEHOLDER_PAYOUT_RECORD_LIMIT: usize = 1_024;
const TREASURY_PLACEHOLDER_PAYOUT_RECORD_RETENTION_WINDOW_MS: u64 = 86_400_000;
const TREASURY_RECEIVE_LIMIT: usize = 16_384;
const TREASURY_POLICY_CHANGE_LIMIT: usize = 64;
const TREASURY_STATUS_POLICY_CHANGE_LIMIT: usize = 8;
const TREASURY_STATUS_PAYOUT_TARGET_ROW_LIMIT: usize = 64;
const TREASURY_STATUS_PAYOUT_LEDGER_ROW_LIMIT: usize = 64;
const TREASURY_IMPOSSIBLE_ZERO_BALANCE_THRESHOLD_SATS: u64 = 1_000;
const TREASURY_CONTINUITY_ALERT_THRESHOLD_MS: u64 = 300_000;
const TREASURY_STALE_SNAPSHOT_ALERT_THRESHOLD_MS: u64 = 15_000;
const TREASURY_MAX_CONCURRENT_SENDS_LIMIT: usize = 64;
const TREASURY_MIN_WALLET_REFRESH_TIMEOUT_MS: u64 = 5_000;
const TREASURY_WALLET_REFRESH_SYNC_TIMEOUT_MS: u64 = 20_000;
const TREASURY_WALLET_REFRESH_RECENT_PAYMENT_PAGES: usize = 1;
const TREASURY_WALLET_REFRESH_CURSOR_PAYMENT_PAGES: usize = 8;
const TREASURY_WALLET_REFRESH_PAYMENT_PAGE_SIZE: usize = 100;
const TREASURY_WALLET_REFRESH_MAX_PAYMENT_PAGES: usize = 8;
const TREASURY_ORPHAN_SEND_PAYMENT_MATCH_EARLY_SLACK_MS: u64 = 5 * 60_000;
const TREASURY_ORPHAN_SEND_PAYMENT_MATCH_WINDOW_MS: u64 = 30 * 60_000;
const TREASURY_PUBLIC_SNAPSHOT_SOURCE_LOCAL: &str = "nexus_control";
const TREASURY_FUNDING_TARGET_TIMEOUT_PREFIX: &str = "treasury_funding_target_timeout:";
const TREASURY_MIN_WALLET_RECOVERY_INSPECTION_TIMEOUT_MS: u64 = 1_000;
const TREASURY_MAX_WALLET_RECOVERY_INSPECTION_TIMEOUT_MS: u64 = 1_800_000;
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
    pub placeholder_payout_mode: TreasuryPlaceholderPayoutMode,
    pub dedupe_placeholder_hosts: bool,
    pub min_new_accrual_pylon_version: Option<String>,
    pub min_new_accrual_started_at_unix_ms: Option<u64>,
    pub reconciliation_horizon_seconds: u64,
    pub apply_env_policy: bool,
    pub allow_destructive_env_policy_change: bool,
    pub policy_change_reason: Option<String>,
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

        Ok(Self {
            enabled,
            payout_sats_per_window,
            payout_interval_seconds,
            require_sellable,
            daily_budget_cap_sats,
            placeholder_payout_mode,
            dedupe_placeholder_hosts,
            min_new_accrual_pylon_version,
            min_new_accrual_started_at_unix_ms,
            reconciliation_horizon_seconds,
            apply_env_policy,
            allow_destructive_env_policy_change,
            policy_change_reason: std::env::var(ENV_TREASURY_POLICY_CHANGE_REASON)
                .ok()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty()),
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
        TREASURY_DISPATCH_RESULT_TIMEOUT_MS
            .max(self.wallet_status_refresh_seconds.saturating_mul(2_000))
    }

    pub fn max_concurrent_send_operations(&self, plan_count: usize) -> usize {
        plan_count.min(self.max_concurrent_sends).max(1)
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
    pub spark_address: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bitcoin_address: Option<String>,
    pub challenge: String,
    pub challenge_signature_hex: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProviderPayoutTargetRegistrationResponse {
    pub authority: String,
    pub nostr_pubkey_hex: String,
    pub session_id: String,
    pub spark_address: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bitcoin_address: Option<String>,
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
    pub spark_address: String,
    pub bitcoin_address: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bolt11_invoice: Option<String>,
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

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
struct TreasuryContinuitySignalSnapshot {
    eligible_online_payout_targets: u64,
    sellable_pylons_online_now: u64,
    inference_ready_online_payout_targets: u64,
    duplicate_host_placeholder_blocked_online_targets: u64,
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
    #[serde(default = "legacy_treasury_placeholder_payout_mode")]
    pub placeholder_payout_mode: TreasuryPlaceholderPayoutMode,
    #[serde(default)]
    pub dedupe_placeholder_hosts: bool,
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
            config.placeholder_payout_mode,
            config.dedupe_placeholder_hosts,
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
        placeholder_payout_mode: TreasuryPlaceholderPayoutMode,
        dedupe_placeholder_hosts: bool,
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
            placeholder_payout_mode,
            dedupe_placeholder_hosts,
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
            placeholder_payout_mode,
            dedupe_placeholder_hosts,
            min_new_accrual_pylon_version,
            min_new_accrual_started_at_unix_ms,
            checksum,
        }
    }

    pub fn payout_interval_ms(&self) -> u64 {
        self.payout_interval_seconds.saturating_mul(1_000)
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

    fn placeholder_payout_verdict(
        &self,
        identity: &OnlinePylonIdentity,
        seen_host_fingerprints: &mut BTreeSet<String>,
    ) -> PlaceholderPayoutEligibilityVerdict {
        match self.placeholder_payout_mode {
            TreasuryPlaceholderPayoutMode::Disabled => {
                return PlaceholderPayoutEligibilityVerdict::Disabled;
            }
            TreasuryPlaceholderPayoutMode::InferenceReady if !identity.inference_ready => {
                return PlaceholderPayoutEligibilityVerdict::RequiresInferenceReady;
            }
            TreasuryPlaceholderPayoutMode::InferenceReady
            | TreasuryPlaceholderPayoutMode::PresenceOnly => {}
        }

        if self.dedupe_placeholder_hosts
            && let Some(host_fingerprint) = identity.host_fingerprint.clone()
            && !seen_host_fingerprints.insert(host_fingerprint)
        {
            return PlaceholderPayoutEligibilityVerdict::DuplicateHost;
        }

        PlaceholderPayoutEligibilityVerdict::Allowed
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
    placeholder_payout_mode: TreasuryPlaceholderPayoutMode,
    dedupe_placeholder_hosts: bool,
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
    pub treasury_enabled: bool,
    pub payout_sats_per_window: u64,
    pub payout_interval_seconds: u64,
    pub require_sellable: bool,
    pub daily_budget_cap_sats: u64,
    pub placeholder_payout_mode: TreasuryPlaceholderPayoutMode,
    pub dedupe_placeholder_hosts: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_new_accrual_pylon_version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_new_accrual_started_at_unix_ms: Option<u64>,
    #[serde(default)]
    pub min_new_accrual_version_gate_active: bool,
    pub registered_payout_identities: u64,
    pub wallet_balance_sats: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wallet_balance_updated_at_unix_ms: Option<u64>,
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
    pub eligible_online_payout_targets: u64,
    #[serde(default)]
    pub sellable_pylons_online_now: u64,
    #[serde(default)]
    pub inference_ready_online_payout_targets: u64,
    #[serde(default)]
    pub duplicate_host_placeholder_blocked_online_targets: u64,
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
    pub accepted_work_payout_sats_paid_total: u64,
    #[serde(default)]
    pub accepted_work_payout_sats_paid_24h: u64,
    #[serde(default)]
    pub placeholder_payout_sats_paid_total: u64,
    #[serde(default)]
    pub placeholder_payout_sats_paid_24h: u64,
    #[serde(default)]
    pub beta_bonus_payout_sats_paid_total: u64,
    #[serde(default)]
    pub beta_bonus_payout_sats_paid_24h: u64,
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
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct TreasuryPayoutTargetIdentityStatus {
    pub nostr_pubkey_hex: String,
    pub source_session_id: String,
    pub spark_address: String,
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
pub struct TreasuryTrainingPayoutLedgerSummary {
    pub reconciliation_status: String,
    pub payout_record_count: u64,
    pub pending_payout_count: u64,
    pub confirmed_payout_count: u64,
    pub failed_payout_count: u64,
    pub skipped_payout_count: u64,
    pub attention_payout_count: u64,
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
            missing_payout_target_count: 0,
            accepted_work_pending_payout_count: 0,
            accepted_work_confirmed_payout_count: 0,
            accepted_work_attention_payout_count: 0,
        }
    }
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
    pub wallet_balance_sats: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wallet_balance_updated_at_unix_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_wallet_sync_at_unix_ms: Option<u64>,
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
    pub accepted_work_payout_sats_paid_total: u64,
    #[serde(default)]
    pub accepted_work_payout_sats_paid_24h: u64,
    #[serde(default)]
    pub placeholder_payout_sats_paid_total: u64,
    #[serde(default)]
    pub placeholder_payout_sats_paid_24h: u64,
    #[serde(default)]
    pub beta_bonus_payout_sats_paid_total: u64,
    #[serde(default)]
    pub beta_bonus_payout_sats_paid_24h: u64,
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
    pub eligible_online_payout_targets: u64,
    pub sellable_pylons_online_now: u64,
    #[serde(default)]
    pub inference_ready_online_payout_targets: u64,
    #[serde(default)]
    pub duplicate_host_placeholder_blocked_online_targets: u64,
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
    pub treasury_enabled: bool,
    pub payout_sats_per_window: u64,
    pub payout_interval_seconds: u64,
    pub require_sellable: bool,
    pub daily_budget_cap_sats: u64,
    pub placeholder_payout_mode: TreasuryPlaceholderPayoutMode,
    pub dedupe_placeholder_hosts: bool,
    pub min_new_accrual_pylon_version: Option<String>,
    pub min_new_accrual_started_at_unix_ms: Option<u64>,
    pub min_new_accrual_version_gate_active: bool,
    pub registered_payout_identities: u64,
    pub wallet_balance_sats: u64,
    pub wallet_balance_updated_at_unix_ms: Option<u64>,
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
    pub eligible_online_payout_targets: u64,
    pub sellable_pylons_online_now: u64,
    pub inference_ready_online_payout_targets: u64,
    pub duplicate_host_placeholder_blocked_online_targets: u64,
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
    pub payout_sats_paid_total: u64,
    pub payout_sats_paid_24h: u64,
    pub accepted_work_payout_sats_paid_total: u64,
    pub accepted_work_payout_sats_paid_24h: u64,
    pub placeholder_payout_sats_paid_total: u64,
    pub placeholder_payout_sats_paid_24h: u64,
    pub beta_bonus_payout_sats_paid_total: u64,
    pub beta_bonus_payout_sats_paid_24h: u64,
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

    const fn counts_as_unknown_version(self) -> bool {
        matches!(
            self,
            Self::MissingClientVersion | Self::InvalidClientVersion
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PlaceholderPayoutEligibilityVerdict {
    Allowed,
    Disabled,
    RequiresInferenceReady,
    DuplicateHost,
}

impl PlaceholderPayoutEligibilityVerdict {
    const fn skip_reason(self) -> Option<&'static str> {
        match self {
            Self::Allowed => None,
            Self::Disabled => Some("placeholder_payouts_disabled"),
            Self::RequiresInferenceReady => Some("placeholder_requires_inference_ready"),
            Self::DuplicateHost => Some("duplicate_host_placeholder_readiness"),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum TreasuryPayoutClass {
    #[default]
    PlaceholderLiveness,
    AcceptedWork,
    BetaBonus,
}

impl TreasuryPayoutClass {
    const fn label(self) -> &'static str {
        match self {
            Self::PlaceholderLiveness => "placeholder_liveness",
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
    fn accepted_work(&self) -> bool {
        self.payout_class == TreasuryPayoutClass::AcceptedWork
    }

    fn continuity_alert_relevant(&self, policy: &TreasuryRuntimePolicy) -> bool {
        match self.payout_class {
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
    pub spark_address: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bitcoin_address: Option<String>,
    pub registered_at_unix_ms: u64,
    pub last_verified_at_unix_ms: u64,
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
    pub eligible_online_payout_targets: u64,
    #[serde(default)]
    pub sellable_pylons_online_now: u64,
    #[serde(default)]
    pub inference_ready_online_payout_targets: u64,
    #[serde(default)]
    pub duplicate_host_placeholder_blocked_online_targets: u64,
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
        match classification.payout_class {
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

#[derive(Debug, Clone, Default)]
pub struct TreasuryWalletSnapshot {
    pub runtime_status: String,
    pub runtime_detail: Option<String>,
    pub wallet_hydration_mode: Option<String>,
    pub wallet_payment_scan_mode: Option<String>,
    pub balance_sats: u64,
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
    pub spark_address: String,
    pub bitcoin_address: String,
    pub bolt11_invoice: Option<String>,
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

fn retryable_failed_accepted_work_payout_is_due(
    record: &TreasuryPayoutRecord,
    now_unix_ms: u64,
) -> bool {
    record.status == "failed"
        && record.payment_id.is_none()
        && record.classification.accepted_work()
        && !record.payout_target.trim().is_empty()
        && now_unix_ms
            >= record
                .updated_at_unix_ms
                .saturating_add(TREASURY_FAILED_ACCEPTED_WORK_RETRY_AFTER_MS)
}

fn placeholder_liveness_record_can_compact(record: &TreasuryPayoutRecord) -> bool {
    if record.classification.payout_class != TreasuryPayoutClass::PlaceholderLiveness {
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
        changed |= loaded.backfill_classified_payout_totals();
        loaded.public_snapshot = None;
        changed |= loaded.trim_policy_change_history();
        changed |= loaded.trim_retention();
        loaded.rebuild_payment_index();
        loaded.state_path = Some(state_path);
        if changed {
            loaded.persist();
        }
        loaded
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
        if self.payout_sats_paid_total == 0 {
            return false;
        }
        if self.accepted_work_payout_sats_paid_total > 0
            || self.beta_bonus_payout_sats_paid_total > 0
            || self.placeholder_payout_sats_paid_total > 0
        {
            return false;
        }
        self.placeholder_payout_sats_paid_total = self.payout_sats_paid_total;
        true
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
            .unwrap_or_else(|| TreasuryRuntimePolicy::from_config(config))
    }

    pub fn treasury_enabled(&self, config: &TreasuryConfig) -> bool {
        self.active_policy(config).treasury_enabled
    }

    pub fn wallet_refresh_due(&self, config: &TreasuryConfig, now_unix_ms: u64) -> bool {
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
        })
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
        self.eligible_online_payout_targets = 0;
        self.inference_ready_online_payout_targets = 0;
        self.duplicate_host_placeholder_blocked_online_targets = 0;
        self.min_new_accrual_version_blocked_online_targets = 0;
        self.min_new_accrual_unknown_version_online_targets = 0;
        if !policy.treasury_enabled || policy.payout_interval_seconds == 0 {
            return;
        }

        let payout_interval_ms = policy.payout_interval_ms();
        let mut latest_eligible_window_started_at_unix_ms: Option<u64> = None;
        let mut seen_placeholder_host_fingerprints = BTreeSet::new();
        for identity in online_identities {
            if policy.require_sellable && !identity.sellable {
                continue;
            }
            if !self
                .payout_targets_by_identity
                .contains_key(identity.nostr_pubkey_hex.as_str())
            {
                continue;
            }
            if identity.inference_ready {
                self.inference_ready_online_payout_targets =
                    self.inference_ready_online_payout_targets.saturating_add(1);
            }
            let placeholder_verdict = policy
                .placeholder_payout_verdict(identity, &mut seen_placeholder_host_fingerprints);
            if placeholder_verdict == PlaceholderPayoutEligibilityVerdict::DuplicateHost {
                self.duplicate_host_placeholder_blocked_online_targets = self
                    .duplicate_host_placeholder_blocked_online_targets
                    .saturating_add(1);
            }
            if placeholder_verdict != PlaceholderPayoutEligibilityVerdict::Allowed {
                continue;
            }
            let window_started_at_unix_ms = payout_window_started_at_for_identity(
                now_unix_ms,
                payout_interval_ms,
                identity.nostr_pubkey_hex.as_str(),
            );
            let gate_verdict = policy.new_accrual_version_gate_verdict(
                identity.client_version.as_deref(),
                window_started_at_unix_ms,
            );
            if gate_verdict != NewAccrualVersionGateVerdict::Allowed {
                self.min_new_accrual_version_blocked_online_targets = self
                    .min_new_accrual_version_blocked_online_targets
                    .saturating_add(1);
                if gate_verdict.counts_as_unknown_version() {
                    self.min_new_accrual_unknown_version_online_targets = self
                        .min_new_accrual_unknown_version_online_targets
                        .saturating_add(1);
                }
                continue;
            }
            self.eligible_online_payout_targets =
                self.eligible_online_payout_targets.saturating_add(1);
            latest_eligible_window_started_at_unix_ms =
                Some(match latest_eligible_window_started_at_unix_ms {
                    Some(existing) => existing.max(window_started_at_unix_ms),
                    None => window_started_at_unix_ms,
                });
        }

        if let Some(window_started_at_unix_ms) = latest_eligible_window_started_at_unix_ms {
            self.latest_eligible_window_started_at_unix_ms = Some(
                self.latest_eligible_window_started_at_unix_ms
                    .unwrap_or(window_started_at_unix_ms)
                    .max(window_started_at_unix_ms),
            );
        }
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
                &policy,
            );
        let oldest_confirmation_pending_at_unix_ms = self
            .oldest_continuity_relevant_pending_payout_updated_at_unix_ms(
                &["queued", "dispatching", "dispatched"],
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
                    >= TREASURY_CONTINUITY_ALERT_THRESHOLD_MS
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
            eligible_online_payout_targets: self.eligible_online_payout_targets,
            sellable_pylons_online_now: self.sellable_pylons_online_now,
            inference_ready_online_payout_targets: self.inference_ready_online_payout_targets,
            duplicate_host_placeholder_blocked_online_targets: self
                .duplicate_host_placeholder_blocked_online_targets,
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
        policy: &TreasuryRuntimePolicy,
    ) -> Option<u64> {
        self.payout_records_by_key
            .values()
            .filter(|record| statuses.contains(&record.status.as_str()))
            .filter(|record| record.classification.continuity_alert_relevant(policy))
            .map(|record| record.updated_at_unix_ms)
            .min()
    }

    fn impossible_zero_balance_with_receive_history(&self) -> bool {
        self.wallet_balance_sats == 0
            && self.completed_funding_receive_total_sats()
                > self
                    .payout_sats_paid_total
                    .saturating_add(TREASURY_IMPOSSIBLE_ZERO_BALANCE_THRESHOLD_SATS)
    }

    fn latest_wallet_activity_at_unix_ms(&self) -> Option<u64> {
        [
            self.last_wallet_sync_at_unix_ms,
            self.wallet_balance_updated_at_unix_ms,
            self.last_dispatch_at_unix_ms,
            self.last_confirmed_payout_at_unix_ms,
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
                && matches!(record.status.as_str(), "failed" | "dispatching")
            {
                backlog_retryable = backlog_retryable.saturating_add(1);
            }
        }
        (backlog_total, backlog_retryable)
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
                    summary.validation_passed && summary.major_divergence_detected
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
        let mut unconfirmed_visible_totals = TreasuryPayoutTotals::default();
        let mut unconfirmed_visible_24h_totals = TreasuryPayoutTotals::default();
        let mut payouts_dispatched_24h = 0u64;
        let mut payouts_confirmed_24h = 0u64;
        let mut payouts_failed_24h = 0u64;
        let mut payouts_skipped_24h = 0u64;
        let (backlog_total, backlog_retryable) = self.backlog_counts();

        for record in self.payout_records_by_key.values() {
            if record.status == "dispatched" && !record.counted_in_paid_total {
                unconfirmed_visible_totals.add_amount(record.amount_sats, &record.classification);
                if record.updated_at_unix_ms >= window_started_at_unix_ms {
                    unconfirmed_visible_24h_totals
                        .add_amount(record.amount_sats, &record.classification);
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
            placeholder_payout_mode: policy.placeholder_payout_mode,
            dedupe_placeholder_hosts: policy.dedupe_placeholder_hosts,
            min_new_accrual_pylon_version: policy.min_new_accrual_pylon_version.clone(),
            min_new_accrual_started_at_unix_ms: policy.min_new_accrual_started_at_unix_ms,
            min_new_accrual_version_gate_active: policy.new_accrual_version_gate_active(),
            registered_payout_identities: self.payout_targets_by_identity.len() as u64,
            wallet_balance_sats: self.wallet_balance_sats,
            wallet_balance_updated_at_unix_ms: self.wallet_balance_updated_at_unix_ms,
            last_wallet_sync_at_unix_ms: self.last_wallet_sync_at_unix_ms,
            wallet_runtime_status,
            wallet_last_error,
            wallet_storage_runtime_mode: self.wallet_storage_runtime_mode(),
            payout_loop_runtime_status: self.payout_loop_runtime_status.clone(),
            payout_loop_last_error: self.payout_loop_last_error.clone(),
            payout_loop_health: self.payout_loop_health(config),
            last_payout_reconciliation_at_unix_ms: self.last_payout_reconciliation_at_unix_ms,
            payout_loop_last_started_at_unix_ms: self.payout_loop_last_started_at_unix_ms,
            payout_loop_last_completed_at_unix_ms: self.payout_loop_last_completed_at_unix_ms,
            payout_sats_paid_total: cumulative_totals
                .payout_sats_paid_total
                .saturating_add(unconfirmed_visible_totals.payout_sats_paid_total),
            payout_sats_paid_24h: confirmed_24h_totals
                .payout_sats_paid_total
                .saturating_add(unconfirmed_visible_24h_totals.payout_sats_paid_total),
            accepted_work_payout_sats_paid_total: cumulative_totals
                .accepted_work_payout_sats_paid_total
                .saturating_add(unconfirmed_visible_totals.accepted_work_payout_sats_paid_total),
            accepted_work_payout_sats_paid_24h: confirmed_24h_totals
                .accepted_work_payout_sats_paid_total
                .saturating_add(
                    unconfirmed_visible_24h_totals.accepted_work_payout_sats_paid_total,
                ),
            placeholder_payout_sats_paid_total: cumulative_totals
                .placeholder_payout_sats_paid_total
                .saturating_add(unconfirmed_visible_totals.placeholder_payout_sats_paid_total),
            placeholder_payout_sats_paid_24h: confirmed_24h_totals
                .placeholder_payout_sats_paid_total
                .saturating_add(unconfirmed_visible_24h_totals.placeholder_payout_sats_paid_total),
            beta_bonus_payout_sats_paid_total: cumulative_totals
                .beta_bonus_payout_sats_paid_total
                .saturating_add(unconfirmed_visible_totals.beta_bonus_payout_sats_paid_total),
            beta_bonus_payout_sats_paid_24h: confirmed_24h_totals
                .beta_bonus_payout_sats_paid_total
                .saturating_add(unconfirmed_visible_24h_totals.beta_bonus_payout_sats_paid_total),
            weak_device_accepted_work_payout_sats_paid_total: cumulative_totals
                .weak_device_accepted_work_payout_sats_paid_total
                .saturating_add(
                    unconfirmed_visible_totals.weak_device_accepted_work_payout_sats_paid_total,
                ),
            weak_device_accepted_work_payout_sats_paid_24h: confirmed_24h_totals
                .weak_device_accepted_work_payout_sats_paid_total
                .saturating_add(
                    unconfirmed_visible_24h_totals.weak_device_accepted_work_payout_sats_paid_total,
                ),
            strong_lane_accepted_work_payout_sats_paid_total: cumulative_totals
                .strong_lane_accepted_work_payout_sats_paid_total
                .saturating_add(
                    unconfirmed_visible_totals.strong_lane_accepted_work_payout_sats_paid_total,
                ),
            strong_lane_accepted_work_payout_sats_paid_24h: confirmed_24h_totals
                .strong_lane_accepted_work_payout_sats_paid_total
                .saturating_add(
                    unconfirmed_visible_24h_totals.strong_lane_accepted_work_payout_sats_paid_total,
                ),
            payouts_dispatched_24h,
            payouts_confirmed_24h,
            payouts_failed_24h,
            payouts_skipped_24h,
            backlog_total,
            backlog_retryable,
            eligible_online_payout_targets: continuity.eligible_online_payout_targets,
            sellable_pylons_online_now: continuity.sellable_pylons_online_now,
            inference_ready_online_payout_targets: continuity.inference_ready_online_payout_targets,
            duplicate_host_placeholder_blocked_online_targets: continuity
                .duplicate_host_placeholder_blocked_online_targets,
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
            snapshot.payouts_dispatched_24h = canonical.payouts_dispatched_24h;
            snapshot.payouts_confirmed_24h = canonical.payouts_confirmed_24h;
            snapshot.payouts_failed_24h = canonical.payouts_failed_24h;
            snapshot.payouts_skipped_24h = canonical.payouts_skipped_24h;
            snapshot.backlog_total = canonical.backlog_total;
            snapshot.backlog_retryable = canonical.backlog_retryable;
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
        let snapshot = self
            .public_snapshot
            .clone()
            .unwrap_or_else(|| self.build_public_snapshot(config, now_unix_ms));
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

        TreasuryPublicStats {
            treasury_enabled: snapshot.treasury_enabled,
            payout_sats_per_window: snapshot.payout_sats_per_window,
            payout_interval_seconds: snapshot.payout_interval_seconds,
            require_sellable: snapshot.require_sellable,
            daily_budget_cap_sats: snapshot.daily_budget_cap_sats,
            placeholder_payout_mode: snapshot.placeholder_payout_mode,
            dedupe_placeholder_hosts: snapshot.dedupe_placeholder_hosts,
            min_new_accrual_pylon_version: snapshot.min_new_accrual_pylon_version,
            min_new_accrual_started_at_unix_ms: snapshot.min_new_accrual_started_at_unix_ms,
            min_new_accrual_version_gate_active: snapshot.min_new_accrual_version_gate_active,
            registered_payout_identities: snapshot.registered_payout_identities,
            wallet_balance_sats: snapshot.wallet_balance_sats,
            wallet_balance_updated_at_unix_ms: snapshot.wallet_balance_updated_at_unix_ms,
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
            eligible_online_payout_targets: snapshot.eligible_online_payout_targets,
            sellable_pylons_online_now: snapshot.sellable_pylons_online_now,
            inference_ready_online_payout_targets: snapshot.inference_ready_online_payout_targets,
            duplicate_host_placeholder_blocked_online_targets: snapshot
                .duplicate_host_placeholder_blocked_online_targets,
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
            accepted_work_payout_sats_paid_total: snapshot.accepted_work_payout_sats_paid_total,
            accepted_work_payout_sats_paid_24h: snapshot.accepted_work_payout_sats_paid_24h,
            placeholder_payout_sats_paid_total: snapshot.placeholder_payout_sats_paid_total,
            placeholder_payout_sats_paid_24h: snapshot.placeholder_payout_sats_paid_24h,
            beta_bonus_payout_sats_paid_total: snapshot.beta_bonus_payout_sats_paid_total,
            beta_bonus_payout_sats_paid_24h: snapshot.beta_bonus_payout_sats_paid_24h,
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
                    spark_address: target.spark_address.clone(),
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
                    summary.failed_payout_count = summary.failed_payout_count.saturating_add(1);
                    summary.attention_payout_count =
                        summary.attention_payout_count.saturating_add(1);
                    if record.classification.accepted_work() {
                        summary.accepted_work_attention_payout_count = summary
                            .accepted_work_attention_payout_count
                            .saturating_add(1);
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
        TreasuryStatusResponse {
            authority: "openagents-hosted-nexus".to_string(),
            treasury_enabled: stats.treasury_enabled,
            payout_sats_per_window: stats.payout_sats_per_window,
            payout_interval_seconds: stats.payout_interval_seconds,
            require_sellable: stats.require_sellable,
            daily_budget_cap_sats: stats.daily_budget_cap_sats,
            placeholder_payout_mode: stats.placeholder_payout_mode,
            dedupe_placeholder_hosts: stats.dedupe_placeholder_hosts,
            min_new_accrual_pylon_version: stats.min_new_accrual_pylon_version,
            min_new_accrual_started_at_unix_ms: stats.min_new_accrual_started_at_unix_ms,
            min_new_accrual_version_gate_active: stats.min_new_accrual_version_gate_active,
            registered_payout_identities: stats.registered_payout_identities,
            wallet_balance_sats: stats.wallet_balance_sats,
            wallet_balance_updated_at_unix_ms: stats.wallet_balance_updated_at_unix_ms,
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
            eligible_online_payout_targets: stats.eligible_online_payout_targets,
            sellable_pylons_online_now: stats.sellable_pylons_online_now,
            inference_ready_online_payout_targets: stats.inference_ready_online_payout_targets,
            duplicate_host_placeholder_blocked_online_targets: stats
                .duplicate_host_placeholder_blocked_online_targets,
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
            accepted_work_payout_sats_paid_total: stats.accepted_work_payout_sats_paid_total,
            accepted_work_payout_sats_paid_24h: stats.accepted_work_payout_sats_paid_24h,
            placeholder_payout_sats_paid_total: stats.placeholder_payout_sats_paid_total,
            placeholder_payout_sats_paid_24h: stats.placeholder_payout_sats_paid_24h,
            beta_bonus_payout_sats_paid_total: stats.beta_bonus_payout_sats_paid_total,
            beta_bonus_payout_sats_paid_24h: stats.beta_bonus_payout_sats_paid_24h,
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
        }
    }

    pub fn note_payout_loop_error(&mut self, now_unix_ms: u64, detail: impl Into<String>) {
        self.payout_loop_runtime_status = Some("error".to_string());
        self.payout_loop_last_error = Some(detail.into());
        self.payout_loop_last_completed_at_unix_ms = Some(now_unix_ms);
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
        verify_payout_target_registration_signature(
            request.nostr_pubkey_hex.as_str(),
            request.session_id.as_str(),
            request.challenge.as_str(),
            request.spark_address.as_str(),
            request.challenge_signature_hex.as_str(),
        )
        .map_err(anyhow::Error::msg)?;
        self.registration_challenges_by_key.remove(&challenge_key);

        if let Some(existing) = self
            .payout_targets_by_identity
            .get_mut(request.nostr_pubkey_hex.as_str())
            .filter(|existing| {
                existing.spark_address == request.spark_address
                    && existing.bitcoin_address == request.bitcoin_address
            })
        {
            existing.source_session_id = request.session_id.clone();
            existing.last_verified_at_unix_ms = now_unix_ms;
            return Ok((
                ProviderPayoutTargetRegistrationResponse {
                    authority: "openagents-hosted-nexus".to_string(),
                    nostr_pubkey_hex: request.nostr_pubkey_hex.clone(),
                    session_id: request.session_id.clone(),
                    spark_address: request.spark_address.clone(),
                    bitcoin_address: request.bitcoin_address.clone(),
                    registered_at_unix_ms: existing.registered_at_unix_ms,
                },
                Vec::new(),
            ));
        }

        let target = RegisteredPayoutTarget {
            nostr_pubkey_hex: request.nostr_pubkey_hex.clone(),
            source_session_id: request.session_id.clone(),
            spark_address: request.spark_address.clone(),
            bitcoin_address: request.bitcoin_address.clone(),
            registered_at_unix_ms: now_unix_ms,
            last_verified_at_unix_ms: now_unix_ms,
        };
        self.payout_targets_by_identity
            .insert(request.nostr_pubkey_hex.clone(), target.clone());
        self.trim_retention();
        self.persist();

        let mut attributes = BTreeMap::new();
        attributes.insert(
            "nostr_pubkey_hex".to_string(),
            request.nostr_pubkey_hex.clone(),
        );
        attributes.insert(
            "spark_address".to_string(),
            truncate_target(request.spark_address.as_str()),
        );
        if let Some(bitcoin_address) = request.bitcoin_address.as_deref() {
            attributes.insert(
                "bitcoin_address".to_string(),
                truncate_target(bitcoin_address),
            );
        }

        Ok((
            ProviderPayoutTargetRegistrationResponse {
                authority: "openagents-hosted-nexus".to_string(),
                nostr_pubkey_hex: request.nostr_pubkey_hex.clone(),
                session_id: request.session_id.clone(),
                spark_address: request.spark_address.clone(),
                bitcoin_address: request.bitcoin_address.clone(),
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
        policy: &TreasuryRuntimePolicy,
        now_unix_ms: u64,
        reserved_wallet_sats: &mut u64,
        committed_daily_budget_sats: &mut u64,
    ) -> (Vec<TreasuryDispatchPlan>, bool) {
        let mut queued = self
            .payout_records_by_key
            .values()
            .filter(|record| {
                record.status == "queued"
                    || retryable_failed_accepted_work_payout_is_due(record, now_unix_ms)
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
        let mut changed = false;
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
            if record.amount_sats
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
            if policy.daily_budget_cap_sats > 0
                && committed_daily_budget_sats.saturating_add(record.amount_sats)
                    > policy.daily_budget_cap_sats
            {
                if record.reason.as_deref() != Some("daily_budget_cap_reached") {
                    record.reason = Some("daily_budget_cap_reached".to_string());
                    record.updated_at_unix_ms = now_unix_ms;
                    changed = true;
                }
                continue;
            }
            *reserved_wallet_sats = reserved_wallet_sats.saturating_add(record.amount_sats);
            *committed_daily_budget_sats =
                committed_daily_budget_sats.saturating_add(record.amount_sats);
            record.payout_target = target.spark_address.clone();
            record.status = "dispatching".to_string();
            record.reason = None;
            record.updated_at_unix_ms = now_unix_ms;
            changed = true;
            dispatch_plans.push(TreasuryDispatchPlan {
                payout_key,
                payment_request: target.spark_address,
                amount_sats: record.amount_sats,
            });
        }
        (dispatch_plans, changed)
    }

    pub fn prepare_due_payouts(
        &mut self,
        config: &TreasuryConfig,
        online_identities: &[OnlinePylonIdentity],
        now_unix_ms: u64,
    ) -> TreasuryPayoutPreparation {
        let mut changed = self.trim_retention();
        let (mut receipt_events, stale_changed) = self.expire_stale_dispatches(config, now_unix_ms);
        changed |= stale_changed;
        let policy = self.active_policy(config);
        if !policy.treasury_enabled || policy.payout_interval_seconds == 0 {
            self.refresh_public_snapshot_in_memory(config, now_unix_ms);
            return TreasuryPayoutPreparation {
                dispatch_plans: Vec::new(),
                receipt_events,
                reconciliation_degraded_reason: None,
            };
        }

        let mut reserved_wallet_sats = self.reserved_wallet_outstanding_sats();
        let mut committed_daily_budget_sats =
            self.committed_daily_budget_sats_last_24h(now_unix_ms);
        let (mut dispatch_plans, queued_claim_changed) = self.claim_queued_payouts_for_dispatch(
            &policy,
            now_unix_ms,
            &mut reserved_wallet_sats,
            &mut committed_daily_budget_sats,
        );
        changed |= queued_claim_changed;
        if online_identities.is_empty() {
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
        let placeholder_classification = policy.placeholder_payout_classification();
        let mut seen_placeholder_host_fingerprints = BTreeSet::new();

        for identity in online_identities {
            let placeholder_verdict = policy
                .placeholder_payout_verdict(identity, &mut seen_placeholder_host_fingerprints);
            let current_window_started_at_unix_ms = payout_window_started_at_for_identity(
                now_unix_ms,
                payout_interval_ms,
                identity.nostr_pubkey_hex.as_str(),
            );
            let mut window_started_at_unix_ms = payout_window_started_at_for_identity(
                reconciliation_started_at_unix_ms,
                payout_interval_ms,
                identity.nostr_pubkey_hex.as_str(),
            );
            loop {
                let window_ends_at_unix_ms =
                    window_started_at_unix_ms.saturating_add(payout_interval_ms);
                let payout_key = payout_window_key(
                    window_started_at_unix_ms,
                    identity.nostr_pubkey_hex.as_str(),
                );

                if !self.payout_records_by_key.contains_key(&payout_key) {
                    let Some(target) = self
                        .payout_targets_by_identity
                        .get(identity.nostr_pubkey_hex.as_str())
                        .cloned()
                    else {
                        let record = TreasuryPayoutRecord {
                            payout_key: payout_key.clone(),
                            nostr_pubkey_hex: identity.nostr_pubkey_hex.clone(),
                            payout_target: String::new(),
                            amount_sats: policy.payout_sats_per_window,
                            status: "skipped".to_string(),
                            reason: Some("missing_payout_target".to_string()),
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
                    };

                    if policy.require_sellable && !identity.sellable {
                        let record = TreasuryPayoutRecord {
                            payout_key: payout_key.clone(),
                            nostr_pubkey_hex: identity.nostr_pubkey_hex.clone(),
                            payout_target: target.spark_address.clone(),
                            amount_sats: policy.payout_sats_per_window,
                            status: "skipped".to_string(),
                            reason: Some("requires_sellable_supply".to_string()),
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

                    if let Some(reason) = placeholder_verdict.skip_reason() {
                        let record = TreasuryPayoutRecord {
                            payout_key: payout_key.clone(),
                            nostr_pubkey_hex: identity.nostr_pubkey_hex.clone(),
                            payout_target: target.spark_address.clone(),
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

                    let gate_verdict = policy.new_accrual_version_gate_verdict(
                        identity.client_version.as_deref(),
                        window_started_at_unix_ms,
                    );
                    if let Some(reason) = gate_verdict.skip_reason() {
                        let record = TreasuryPayoutRecord {
                            payout_key: payout_key.clone(),
                            nostr_pubkey_hex: identity.nostr_pubkey_hex.clone(),
                            payout_target: target.spark_address.clone(),
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

                    if policy.daily_budget_cap_sats > 0
                        && committed_daily_budget_sats.saturating_add(policy.payout_sats_per_window)
                            > policy.daily_budget_cap_sats
                    {
                        let record = TreasuryPayoutRecord {
                            payout_key: payout_key.clone(),
                            nostr_pubkey_hex: identity.nostr_pubkey_hex.clone(),
                            payout_target: target.spark_address.clone(),
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

                    committed_daily_budget_sats =
                        committed_daily_budget_sats.saturating_add(policy.payout_sats_per_window);
                    self.payout_records_by_key.insert(
                        payout_key.clone(),
                        TreasuryPayoutRecord {
                            payout_key: payout_key.clone(),
                            nostr_pubkey_hex: identity.nostr_pubkey_hex.clone(),
                            payout_target: target.spark_address.clone(),
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
                        },
                    );
                    dispatch_plans.push(TreasuryDispatchPlan {
                        payout_key,
                        payment_request: target.spark_address,
                        amount_sats: policy.payout_sats_per_window,
                    });
                    changed = true;
                }

                if window_started_at_unix_ms >= current_window_started_at_unix_ms {
                    break;
                }
                window_started_at_unix_ms =
                    window_started_at_unix_ms.saturating_add(payout_interval_ms);
            }
        }

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
            } => {
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
                        receipt_events.push(dispatched_payout_receipt(record, payment_id.as_str()));
                    }
                }
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
        self.wallet_runtime_status = Some(snapshot.runtime_status.clone());
        self.wallet_last_error = snapshot.runtime_detail.clone();
        self.wallet_hydration_mode = snapshot.wallet_hydration_mode.clone();
        self.wallet_payment_scan_mode = snapshot.wallet_payment_scan_mode.clone();
        self.wallet_balance_sats = snapshot.balance_sats;
        self.wallet_balance_updated_at_unix_ms = Some(now_unix_ms);
        self.last_wallet_sync_at_unix_ms = Some(now_unix_ms);
        self.last_wallet_refresh_attempt_at_unix_ms = Some(now_unix_ms);

        let mut receipt_events = Vec::new();
        let mut persist_needed = false;
        let mut last_confirmed_payout_at_unix_ms = self.last_confirmed_payout_at_unix_ms;
        let mut orphan_recovery_payout_keys = self.orphan_payment_recovery_keys();
        let mut payments = snapshot.payments.clone();
        payments.sort_by_key(|payment| payment.timestamp);
        for payment in &payments {
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
                    "treasury recovered orphan Spark send payment from wallet history",
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
            if recovered_orphan && !record.dispatch_receipt_recorded {
                record.dispatch_receipt_recorded = true;
                persist_needed = true;
                receipt_events.push(dispatched_payout_receipt(record, payment.id.as_str()));
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
                    match record.classification.payout_class {
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
            } else {
                if record.status != "dispatched" {
                    record.status = "dispatched".to_string();
                    persist_needed = true;
                }
                if record.reason.is_some() {
                    record.reason = None;
                    persist_needed = true;
                }
            }
        }
        if self.last_confirmed_payout_at_unix_ms != last_confirmed_payout_at_unix_ms {
            self.last_confirmed_payout_at_unix_ms = last_confirmed_payout_at_unix_ms;
            persist_needed = true;
        }

        persist_needed |= self.trim_retention();
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

    fn committed_daily_budget_sats_last_24h(&self, now_unix_ms: u64) -> u64 {
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
            .fold(0u64, |total, record| {
                total.saturating_add(record.amount_sats)
            })
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

    fn trim_retention(&mut self) -> bool {
        let mut changed = false;
        let now_unix_ms = now_unix_ms();
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
        method: "spark-simulated".to_string(),
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
    TreasuryFundingMaterial {
        spark_address: "spark:simulated-treasury-proof-wallet".to_string(),
        bitcoin_address: "bcrt1qsimulatedtreasuryproofwallet".to_string(),
        bolt11_invoice: (amount > 0).then(|| format!("lnbc{amount}simulatedproofwallet")),
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
        });
    }
    TreasuryDispatchBatchResult {
        outcomes,
        wallet_snapshot: Some(simulated_wallet_snapshot(config, payments)),
        wallet_error: None,
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

    with_live_wallet(config, true, |wallet| async move {
        let spark_address = wallet
            .get_spark_address()
            .await
            .context("failed to create treasury Spark receive address")?;
        let bitcoin_address = wallet
            .get_bitcoin_address()
            .await
            .context("failed to create treasury Bitcoin receive address")?;
        let bolt11_invoice = match request.amount_sats {
            Some(amount_sats) if amount_sats > 0 => Some(
                wallet
                    .create_bolt11_invoice(
                        amount_sats,
                        request.description.clone(),
                        request.expiry_seconds,
                    )
                    .await
                    .context("failed to create treasury Bolt11 invoice")?,
            ),
            Some(_) => bail!("treasury funding amount must be greater than 0"),
            None => None,
        };
        let wallet_snapshot =
            wallet_snapshot_from_wallet_for_funding_target(wallet.as_ref()).await?;
        Ok(TreasuryFundingMaterial {
            spark_address,
            bitcoin_address,
            bolt11_invoice,
            wallet_snapshot,
        })
    })
    .await
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
    refresh_plan: TreasuryWalletRefreshPlan,
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

    with_live_wallet(config, create_if_missing, move |wallet| async move {
        wallet_snapshot_from_wallet_with_plan_result(wallet.as_ref(), &refresh_plan).await
    })
    .await
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
            return dispatch_with_test_hooks(plans);
        }
    }

    let operation_lock = live_wallet_operation_lock();
    let _operation_guard = operation_lock.lock().await;

    let wallet = match open_wallet(config, false).await {
        Ok(wallet) => wallet,
        Err(error) => {
            return TreasuryDispatchBatchResult {
                outcomes: plans
                    .iter()
                    .map(|plan| TreasuryDispatchOutcome::Failed {
                        payout_key: plan.payout_key.clone(),
                        reason: error.to_string(),
                    })
                    .collect(),
                wallet_snapshot: None,
                wallet_error: Some(error.to_string()),
            };
        }
    };

    let send_timeout_ms = config.dispatch_result_timeout_ms(config.payout_interval_ms());

    // Keep the wallet-operation lock held for a bounded window even when the
    // upstream Spark send path stalls or many Pylons become due together.
    let max_concurrent_sends = config.max_concurrent_send_operations(plans.len());
    let mut indexed_outcomes = stream::iter(plans.iter().cloned().enumerate())
        .map(|(index, plan)| {
            let wallet = wallet.clone();
            async move {
                let outcome =
                    dispatch_outcome_from_send_future(plan.clone(), send_timeout_ms, async move {
                        wallet
                            .send_payment_simple(
                                plan.payment_request.as_str(),
                                Some(plan.amount_sats),
                            )
                            .await
                    })
                    .await;
                (index, outcome)
            }
        })
        .buffer_unordered(max_concurrent_sends)
        .collect::<Vec<_>>()
        .await;
    indexed_outcomes.sort_by_key(|(index, _)| *index);
    let outcomes = indexed_outcomes
        .into_iter()
        .map(|(_, outcome)| outcome)
        .collect();
    disconnect_live_wallet(wallet).await;

    TreasuryDispatchBatchResult {
        outcomes,
        // The dedicated wallet refresh loop reconciles confirms and balance.
        // Keeping the full wallet scan out of the dispatch path preserves the
        // intended payout cadence even when many Pylons are online.
        wallet_snapshot: None,
        wallet_error: None,
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
        },
        Ok(Err(error)) => TreasuryDispatchOutcome::Failed {
            payout_key: plan.payout_key,
            reason: error.to_string(),
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
            let material = create_live_funding_target(
                config,
                TreasuryFundingTargetRequest {
                    amount_sats: *amount_sats,
                    description: description.clone(),
                    expiry_seconds: *expiry_seconds,
                },
            )
            .await?;
            let response = TreasuryFundingTargetResponse {
                authority: "openagents-hosted-nexus".to_string(),
                wallet_runtime_status: material.wallet_snapshot.runtime_status,
                wallet_runtime_detail: material.wallet_snapshot.runtime_detail,
                wallet_hydration_mode: material.wallet_snapshot.wallet_hydration_mode,
                wallet_payment_scan_mode: material.wallet_snapshot.wallet_payment_scan_mode,
                wallet_balance_sats: material.wallet_snapshot.balance_sats,
                wallet_balance_updated_at_unix_ms: now_unix_ms(),
                spark_address: material.spark_address,
                bitcoin_address: material.bitcoin_address,
                bolt11_invoice: material.bolt11_invoice,
            };
            if *json {
                return Ok(serde_json::to_string_pretty(&response)?);
            }
            Ok(render_treasury_funding_target_response(&response))
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

fn aggregate_unclaimed_deposits(
    deposits: &[openagents_spark::UnclaimedDeposit],
) -> TreasuryWalletUnclaimedDepositAggregate {
    let mut aggregate = TreasuryWalletUnclaimedDepositAggregate::default();
    for deposit in deposits {
        aggregate.count = aggregate.count.saturating_add(1);
        aggregate.total_sats = aggregate.total_sats.saturating_add(deposit.amount_sats);
        if deposit.claim_error.is_some() {
            aggregate.with_claim_error_count = aggregate.with_claim_error_count.saturating_add(1);
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

async fn try_cached_balance_after_inspection_sync_failure(
    wallet: &SparkWallet,
    inspection: &mut TreasuryWalletInspection,
    inspection_timeout_ms: u64,
    sync_failure: String,
) -> bool {
    let cached_timeout_ms = inspection_timeout_ms
        .min(TREASURY_MIN_WALLET_RECOVERY_INSPECTION_TIMEOUT_MS)
        .max(1);
    match tokio::time::timeout(
        Duration::from_millis(cached_timeout_ms),
        wallet.get_balance_cached(),
    )
    .await
    {
        Ok(Ok(balance)) => {
            inspection.runtime_status = Some("cached_after_sync_timeout".to_string());
            inspection.runtime_detail = Some(format!(
                "{sync_failure}; using cached local balance only. This validates identity/balance comparison for report-only recovery but is not cutover-safe."
            ));
            inspection.balance_sats = Some(balance.total_sats());
            true
        }
        Ok(Err(error)) => {
            inspection.runtime_status = Some("error".to_string());
            inspection.error = Some(format!(
                "{sync_failure}; cached treasury Spark balance also failed: {error}"
            ));
            false
        }
        Err(_) => {
            inspection.runtime_status = Some("timeout".to_string());
            inspection.error = Some(format!(
                "{sync_failure}; cached treasury Spark balance also timed out after {cached_timeout_ms} ms"
            ));
            false
        }
    }
}

async fn inspect_treasury_wallet_storage(
    config: &TreasuryConfig,
    mnemonic: &str,
    storage_dir: &Path,
) -> TreasuryWalletInspection {
    let inspection_timeout_ms = config.wallet_recovery_inspection_timeout_ms.clamp(
        TREASURY_MIN_WALLET_RECOVERY_INSPECTION_TIMEOUT_MS,
        TREASURY_MAX_WALLET_RECOVERY_INSPECTION_TIMEOUT_MS,
    );
    let mut inspection = TreasuryWalletInspection {
        inspected_storage_dir: storage_dir.display().to_string(),
        inspection_timeout_ms,
        ..TreasuryWalletInspection::default()
    };
    let signer = match SparkSigner::from_mnemonic(mnemonic, "") {
        Ok(signer) => {
            inspection.wallet_identity_pubkey = signer.public_key_hex();
            signer
        }
        Err(error) => {
            inspection.error = Some(format!("failed to derive treasury Spark signer: {error}"));
            return inspection;
        }
    };
    let wallet_config = match treasury_wallet_config(config, storage_dir.to_path_buf()) {
        Ok(wallet_config) => wallet_config,
        Err(error) => {
            inspection.error = Some(error.to_string());
            return inspection;
        }
    };
    let wallet = match SparkWallet::new(signer, wallet_config).await {
        Ok(wallet) => wallet,
        Err(error) => {
            inspection.error = Some(format!(
                "failed to initialize treasury Spark wallet: {error}"
            ));
            return inspection;
        }
    };

    inspection.runtime_status = Some("syncing".to_string());
    inspection.runtime_detail =
        Some("treasury wallet inspection is syncing isolated storage".to_string());

    match tokio::time::timeout(
        Duration::from_millis(inspection_timeout_ms),
        wallet.get_balance(),
    )
    .await
    {
        Ok(Ok(balance)) => {
            inspection.runtime_status = Some("synced".to_string());
            inspection.runtime_detail = Some(
                "treasury wallet inspection completed live sync on isolated storage".to_string(),
            );
            inspection.balance_sats = Some(balance.total_sats());
        }
        Ok(Err(error)) => {
            let sync_failure = format!("failed to fetch synced treasury Spark balance: {error}");
            if try_cached_balance_after_inspection_sync_failure(
                &wallet,
                &mut inspection,
                inspection_timeout_ms,
                sync_failure,
            )
            .await
            {
                let _ = wallet.disconnect().await;
                return inspection;
            }
            let _ = wallet.disconnect().await;
            return inspection;
        }
        Err(_) => {
            let sync_failure = format!(
                "timed out after {inspection_timeout_ms} ms while syncing treasury Spark wallet"
            );
            if try_cached_balance_after_inspection_sync_failure(
                &wallet,
                &mut inspection,
                inspection_timeout_ms,
                sync_failure,
            )
            .await
            {
                let _ = wallet.disconnect().await;
                return inspection;
            }
            let _ = wallet.disconnect().await;
            return inspection;
        }
    }

    match tokio::time::timeout(
        Duration::from_millis(inspection_timeout_ms),
        wallet.list_all_payments(),
    )
    .await
    {
        Ok(Ok(payments)) => {
            inspection.payment_totals = aggregate_payment_summaries(&payments);
        }
        Ok(Err(error)) => {
            inspection.runtime_status = Some("error".to_string());
            inspection.error = Some(format!("failed to list treasury Spark payments: {error}"));
            let _ = wallet.disconnect().await;
            return inspection;
        }
        Err(_) => {
            inspection.runtime_status = Some("timeout".to_string());
            inspection.error = Some(format!(
                "timed out after {inspection_timeout_ms} ms while listing treasury Spark payments"
            ));
            let _ = wallet.disconnect().await;
            return inspection;
        }
    }

    match tokio::time::timeout(
        Duration::from_millis(inspection_timeout_ms),
        wallet.list_unclaimed_deposits(),
    )
    .await
    {
        Ok(Ok(deposits)) => {
            inspection.unclaimed_deposit_totals = aggregate_unclaimed_deposits(&deposits);
        }
        Ok(Err(error)) => {
            inspection.runtime_status = Some("error".to_string());
            inspection.error = Some(format!(
                "failed to list treasury Spark unclaimed deposits: {error}"
            ));
            let _ = wallet.disconnect().await;
            return inspection;
        }
        Err(_) => {
            inspection.runtime_status = Some("timeout".to_string());
            inspection.error = Some(format!(
                "timed out after {inspection_timeout_ms} ms while listing treasury Spark unclaimed deposits"
            ));
            let _ = wallet.disconnect().await;
            return inspection;
        }
    }

    let _ = wallet.disconnect().await;
    inspection
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
    if before.placeholder_payout_mode != after.placeholder_payout_mode {
        changed_fields.push("placeholder_payout_mode".to_string());
    }
    if before.dedupe_placeholder_hosts != after.dedupe_placeholder_hosts {
        changed_fields.push("dedupe_placeholder_hosts".to_string());
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
    "treasury [status [--json] | funding-target [--amount-sats <n>] [--description <text>] [--expiry-seconds <n>] [--json] | recovery-report [--work-dir <path>] [--report-path <path>] [--json] | recovery-cutover --report-path <path> [--json]]"
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
    if let Some(wallet_sync_lag_ms) = response.wallet_sync_lag_ms {
        lines.push(format!("wallet_sync_lag_ms: {wallet_sync_lag_ms}"));
    }
    lines.push(format!("backlog_total: {}", response.backlog_total));
    lines.push(format!("backlog_retryable: {}", response.backlog_retryable));
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
    if let Some(reason) = response.degraded_reason.as_deref() {
        lines.push(format!("degraded_reason: {reason}"));
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
        format!("spark_address: {}", response.spark_address),
        format!("bitcoin_address: {}", response.bitcoin_address),
    ];
    if let Some(invoice) = response.bolt11_invoice.as_deref() {
        lines.push(format!("bolt11_invoice: {invoice}"));
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

pub fn verify_payout_target_registration_signature(
    nostr_pubkey_hex: &str,
    session_id: &str,
    challenge: &str,
    spark_address: &str,
    signature_hex: &str,
) -> Result<()> {
    verify_provider_payout_target_registration_signature(
        nostr_pubkey_hex,
        session_id,
        challenge,
        spark_address,
        signature_hex,
    )
    .map_err(anyhow::Error::msg)
}

fn dispatched_payout_receipt(
    record: &TreasuryPayoutRecord,
    payment_id: &str,
) -> TreasuryReceiptEvent {
    let mut attributes = payout_receipt_attributes(record);
    attributes.insert("payment_id".to_string(), payment_id.to_string());
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
        record.classification.payout_class.label().to_string(),
    );
    if !record.payout_target.is_empty() {
        attributes.insert(
            "payout_target".to_string(),
            truncate_target(record.payout_target.as_str()),
        );
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
        "failed" => record.reason.as_deref().is_some_and(|reason| {
            reason == "dispatch_outcome_timeout" || reason.starts_with("wallet_send_timeout:")
        }),
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

const LIVE_WALLET_DISCONNECT_TIMEOUT_MS: u64 = 5_000;

fn live_wallet_operation_lock() -> &'static AsyncMutex<()> {
    static LOCK: OnceLock<AsyncMutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| AsyncMutex::new(()))
}

async fn with_live_wallet<F, Fut, T>(
    config: &TreasuryConfig,
    create_if_missing: bool,
    operation: F,
) -> Result<T>
where
    F: FnOnce(Arc<SparkWallet>) -> Fut,
    Fut: std::future::Future<Output = Result<T>>,
{
    let operation_lock = live_wallet_operation_lock();
    let _operation_guard = operation_lock.lock().await;
    let wallet = open_wallet(config, create_if_missing).await?;
    let result = operation(wallet.clone()).await;
    disconnect_live_wallet(wallet).await;
    result
}

async fn open_wallet(config: &TreasuryConfig, create_if_missing: bool) -> Result<Arc<SparkWallet>> {
    Ok(Arc::new(
        open_wallet_uncached(config, create_if_missing).await?,
    ))
}

async fn disconnect_live_wallet(wallet: Arc<SparkWallet>) {
    // The bounded treasury runtime does not need to retain a long-lived Spark
    // session between refresh/send cycles. Avoid a shared wallet cache here:
    // outer timeout guards can cancel wallet futures before this disconnect
    // path runs, and a globally cached Arc would let the next operation reuse a
    // stuck SDK handle.
    match tokio::time::timeout(
        Duration::from_millis(LIVE_WALLET_DISCONNECT_TIMEOUT_MS),
        wallet.disconnect(),
    )
    .await
    {
        Ok(Ok(())) => {}
        Ok(Err(error)) => {
            tracing::warn!("treasury live wallet disconnect failed: {error}");
        }
        Err(_) => {
            tracing::warn!(
                "treasury live wallet disconnect timed out after {} ms",
                LIVE_WALLET_DISCONNECT_TIMEOUT_MS
            );
        }
    }
}

async fn open_wallet_uncached(
    config: &TreasuryConfig,
    create_if_missing: bool,
) -> Result<SparkWallet> {
    ensure_rustls_crypto_provider()?;
    let mnemonic =
        ensure_wallet_mnemonic(config.wallet_mnemonic_path.as_path(), create_if_missing)?;
    fs::create_dir_all(config.wallet_storage_dir.as_path()).with_context(|| {
        format!(
            "failed to create treasury wallet storage dir {}",
            config.wallet_storage_dir.display()
        )
    })?;
    let signer = SparkSigner::from_mnemonic(mnemonic.as_str(), "")
        .map_err(|error| anyhow!("failed to derive treasury Spark signer: {error}"))?;
    SparkWallet::new(
        signer,
        treasury_wallet_config(config, config.wallet_storage_dir.clone())?,
    )
    .await
    .context("failed to initialize treasury Spark wallet")
}

fn treasury_wallet_config(config: &TreasuryConfig, storage_dir: PathBuf) -> Result<WalletConfig> {
    Ok(WalletConfig {
        network: parse_wallet_network(config.wallet_network.as_str())?,
        api_key: resolve_wallet_api_key(config.wallet_api_key_env.as_deref()),
        storage_dir,
        deposit_claim_fee_policy: DepositClaimFeePolicy::Auto,
        background_processing: false,
        real_time_sync_enabled: config.wallet_real_time_sync_enabled,
    })
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

async fn wallet_refresh_payments(
    wallet: &SparkWallet,
    plan: &TreasuryWalletRefreshPlan,
) -> Result<WalletRefreshPaymentsResult> {
    let mut payments = Vec::new();
    let mut unresolved_payment_ids = plan.tracked_payment_ids.clone();
    let page_offsets = wallet_refresh_page_offsets(plan);
    let page_size = TREASURY_WALLET_REFRESH_PAYMENT_PAGE_SIZE as u32;
    let mut scanned_pages = 0usize;
    let mut progress = TreasuryWalletRefreshProgress {
        history_scan_page_offset: plan
            .history_scan_page_offset
            .max(TREASURY_WALLET_REFRESH_RECENT_PAYMENT_PAGES),
        ..TreasuryWalletRefreshProgress::default()
    };

    for page_offset in page_offsets {
        let offset = (page_offset * TREASURY_WALLET_REFRESH_PAYMENT_PAGE_SIZE) as u32;
        let mut page = wallet
            .list_payments(Some(page_size), Some(offset))
            .await
            .context("failed to list treasury Spark payments")?;
        if page.is_empty() {
            progress.history_hit_end_of_history = true;
            break;
        }

        scanned_pages = scanned_pages.saturating_add(1);
        if page_offset >= TREASURY_WALLET_REFRESH_RECENT_PAYMENT_PAGES {
            progress.history_pages_scanned = progress.history_pages_scanned.saturating_add(1);
        }
        for payment in &page {
            unresolved_payment_ids.remove(payment.id.as_str());
        }

        let page_len = page.len();
        payments.append(&mut page);

        if page_len < TREASURY_WALLET_REFRESH_PAYMENT_PAGE_SIZE || unresolved_payment_ids.is_empty()
        {
            if page_len < TREASURY_WALLET_REFRESH_PAYMENT_PAGE_SIZE {
                progress.history_hit_end_of_history = true;
            }
            break;
        }
    }

    if !unresolved_payment_ids.is_empty() {
        tracing::warn!(
            tracked_payment_count = plan.tracked_payment_count(),
            remaining_payment_count = unresolved_payment_ids.len(),
            scanned_pages,
            page_budget = plan.payment_page_budget(),
            "treasury wallet refresh bounded payment scan left unresolved payouts for a later cycle",
        );
    }

    Ok(WalletRefreshPaymentsResult { payments, progress })
}

#[derive(Debug, Clone, Default)]
struct WalletRefreshPaymentsResult {
    payments: Vec<PaymentSummary>,
    progress: TreasuryWalletRefreshProgress,
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

async fn wallet_snapshot_from_wallet(wallet: &SparkWallet) -> Result<TreasuryWalletSnapshot> {
    wallet_snapshot_from_wallet_with_plan_result(wallet, &TreasuryWalletRefreshPlan::recent_only())
        .await
        .map(|result| result.snapshot)
}

async fn wallet_snapshot_from_wallet_for_funding_target(
    wallet: &SparkWallet,
) -> Result<TreasuryWalletSnapshot> {
    let balance = wallet
        .get_balance_cached()
        .await
        .context("failed to fetch cached treasury Spark balance for funding target")?;
    Ok(TreasuryWalletSnapshot {
        runtime_status: "connected".to_string(),
        runtime_detail: Some(
            "funding target returned without full wallet sync; refresh loop owns reconciliation"
                .to_string(),
        ),
        wallet_hydration_mode: Some("cached_balance_for_funding_target".to_string()),
        wallet_payment_scan_mode: Some("skipped_for_funding_target".to_string()),
        balance_sats: balance.total_sats(),
        payments: Vec::new(),
    })
}

async fn wallet_snapshot_from_wallet_with_plan_result(
    wallet: &SparkWallet,
    plan: &TreasuryWalletRefreshPlan,
) -> Result<TreasuryWalletRefreshResult> {
    let (hydration_mode, runtime_detail) = match tokio::time::timeout(
        Duration::from_millis(TREASURY_WALLET_REFRESH_SYNC_TIMEOUT_MS),
        wallet.sync_wallet_state(),
    )
    .await
    {
        Ok(Ok(())) => ("sync_wallet_then_cached_balance", None),
        Ok(Err(error)) => {
            let detail = format!(
                "sync_wallet_failed:{error}; using cached balance and bounded payment scan"
            );
            tracing::warn!(
                error = %error,
                "treasury wallet refresh fell back after Spark sync failed"
            );
            ("cached_balance_after_sync_failure", Some(detail))
        }
        Err(_) => {
            let detail = format!(
                "sync_wallet_timeout:{TREASURY_WALLET_REFRESH_SYNC_TIMEOUT_MS}; using cached balance and bounded payment scan"
            );
            tracing::warn!(
                timeout_ms = TREASURY_WALLET_REFRESH_SYNC_TIMEOUT_MS,
                "treasury wallet refresh fell back after Spark sync timed out"
            );
            ("cached_balance_after_sync_timeout", Some(detail))
        }
    };
    let balance = wallet
        .get_balance_cached()
        .await
        .context("failed to fetch treasury Spark balance")?;
    let refresh = wallet_refresh_payments(wallet, plan).await?;
    validate_wallet_hydration_balance(plan, balance.total_sats(), hydration_mode)?;
    Ok(TreasuryWalletRefreshResult {
        snapshot: TreasuryWalletSnapshot {
            runtime_status: "connected".to_string(),
            runtime_detail,
            wallet_hydration_mode: Some(hydration_mode.to_string()),
            wallet_payment_scan_mode: Some(wallet_payment_scan_mode(plan).to_string()),
            balance_sats: balance.total_sats(),
            payments: refresh.payments,
        },
        progress: refresh.progress,
    })
}

async fn wallet_snapshot_from_wallet_with_plan(
    wallet: &SparkWallet,
    plan: &TreasuryWalletRefreshPlan,
) -> Result<TreasuryWalletSnapshot> {
    wallet_snapshot_from_wallet_with_plan_result(wallet, plan)
        .await
        .map(|result| result.snapshot)
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

fn ensure_rustls_crypto_provider() -> Result<()> {
    if rustls::crypto::CryptoProvider::get_default().is_some() {
        return Ok(());
    }
    rustls::crypto::ring::default_provider()
        .install_default()
        .map_err(|error| anyhow!("failed to install rustls crypto provider: {error:?}"))
}

fn parse_wallet_network(raw: &str) -> Result<SparkNetwork> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "mainnet" => Ok(SparkNetwork::Mainnet),
        "regtest" => Ok(SparkNetwork::Regtest),
        "testnet" | "signet" => {
            bail!("unsupported treasury Spark network '{raw}' (supported: mainnet, regtest)")
        }
        _ => bail!("invalid treasury wallet_network '{raw}' (supported: mainnet, regtest)"),
    }
}

fn resolve_wallet_api_key(config_env: Option<&str>) -> Option<String> {
    if let Some(name) = config_env
        .map(str::trim)
        .filter(|value| !value.is_empty() && *value != "OPENAGENTS_SPARK_API_KEY")
        && let Some(value) = read_env_nonempty(name)
    {
        return Some(value);
    }
    if let Some(value) = read_env_nonempty("OPENAGENTS_SPARK_API_KEY") {
        return Some(value);
    }
    if let Some(value) = read_env_nonempty("BREEZ_API_KEY") {
        return Some(value);
    }
    Some(DEFAULT_OPENAGENTS_SPARK_API_KEY.to_string())
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
fn dispatch_with_test_hooks(plans: &[TreasuryDispatchPlan]) -> TreasuryDispatchBatchResult {
    let send_hook = test_wallet_send_hook()
        .lock()
        .expect("treasury send hook")
        .clone();
    let snapshot_hook = test_wallet_snapshot_hook()
        .lock()
        .expect("treasury snapshot hook")
        .clone();
    let mut outcomes = Vec::with_capacity(plans.len());
    for plan in plans {
        match send_hook
            .as_ref()
            .ok_or_else(|| anyhow!("missing treasury send hook"))
            .and_then(|hook| hook(plan.payment_request.clone(), plan.amount_sats))
        {
            Ok(payment_id) => outcomes.push(TreasuryDispatchOutcome::Dispatched {
                payout_key: plan.payout_key.clone(),
                payment_id,
            }),
            Err(error) => outcomes.push(TreasuryDispatchOutcome::Failed {
                payout_key: plan.payout_key.clone(),
                reason: error.to_string(),
            }),
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
        OnlinePylonIdentity, TREASURY_IMPOSSIBLE_ZERO_BALANCE_THRESHOLD_SATS,
        TREASURY_WALLET_REFRESH_CURSOR_PAYMENT_PAGES, TREASURY_WALLET_REFRESH_MAX_PAYMENT_PAGES,
        TREASURY_WALLET_REFRESH_PAYMENT_PAGE_SIZE, TREASURY_WALLET_REFRESH_RECENT_PAYMENT_PAGES,
        TreasuryConfig, TreasuryDispatchOutcome, TreasuryFundingMaterial, TreasuryFundingReceive,
        TreasuryFundingTargetRequest, TreasuryPayoutClass, TreasuryPayoutClassification,
        TreasuryPayoutRecord, TreasuryPlaceholderPayoutMode, TreasuryPublicStats,
        TreasuryQueuedPayoutRequest, TreasuryState, TreasuryWalletInspection,
        TreasuryWalletPaymentAggregate, TreasuryWalletRecoveryComparison,
        TreasuryWalletRecoveryReport, TreasuryWalletRefreshPlan, TreasuryWalletRefreshProgress,
        TreasuryWalletSnapshot, apply_treasury_wallet_recovery_cutover,
        build_treasury_wallet_recovery_comparison, create_live_funding_target,
        dispatch_live_payouts, parse_treasury_command, payout_phase_offset_ms, payout_window_key,
        payout_window_started_at, payout_window_started_at_for_identity,
        set_test_wallet_funding_hook, set_test_wallet_send_hook, set_test_wallet_snapshot_hook,
        treasury_test_hook_lock, validate_wallet_hydration_balance,
        verify_payout_target_registration_signature, wallet_refresh_page_offsets,
        wallet_refresh_payment_page_budget, write_json_file,
    };
    use openagents_provider_substrate::sign_provider_payout_target_registration;
    use openagents_spark::PaymentSummary;
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
            placeholder_payout_mode: TreasuryPlaceholderPayoutMode::InferenceReady,
            dedupe_placeholder_hosts: true,
            min_new_accrual_pylon_version: None,
            min_new_accrual_started_at_unix_ms: None,
            reconciliation_horizon_seconds: 300,
            apply_env_policy: false,
            allow_destructive_env_policy_change: false,
            policy_change_reason: None,
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
            simulated_wallet_enabled: false,
            simulated_wallet_balance_sats: 1_000_000,
            max_concurrent_sends: 16,
            registration_challenge_ttl_seconds: 300,
            integration_token: None,
        }
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

    #[test]
    fn treasury_wallet_config_disables_background_processing() {
        let config = test_treasury_config();
        let wallet_config =
            super::treasury_wallet_config(&config, config.wallet_storage_dir.clone())
                .expect("wallet config");
        assert!(!wallet_config.background_processing);
    }

    #[test]
    fn treasury_wallet_config_keeps_realtime_sync_disabled_by_default() {
        let config = test_treasury_config();
        let wallet_config =
            super::treasury_wallet_config(&config, config.wallet_storage_dir.clone())
                .expect("wallet config");

        assert!(!wallet_config.real_time_sync_enabled);
    }

    #[test]
    fn treasury_wallet_config_can_reenable_realtime_sync() {
        let mut config = test_treasury_config();
        config.wallet_real_time_sync_enabled = true;
        let wallet_config =
            super::treasury_wallet_config(&config, config.wallet_storage_dir.clone())
                .expect("wallet config");

        assert!(wallet_config.real_time_sync_enabled);
    }

    #[test]
    fn payout_target_signature_round_trip_is_valid() {
        let private_key_hex = "1111111111111111111111111111111111111111111111111111111111111111";
        let nostr_pubkey_hex = "4f355bdcb7cc0af728ef3cceb9615d90684bb5b2ca5f859ab0f0b704075871aa";
        let signature = sign_provider_payout_target_registration(
            private_key_hex,
            nostr_pubkey_hex,
            "session-a",
            "challenge-a",
            "spark:alice",
        )
        .expect("signature should build");
        verify_payout_target_registration_signature(
            nostr_pubkey_hex,
            "session-a",
            "challenge-a",
            "spark:alice",
            signature.as_str(),
        )
        .expect("signature should verify");
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
                spark_address: "spark:alice".to_string(),
                bitcoin_address: Some("bc1qalice".to_string()),
                registered_at_unix_ms: 500,
                last_verified_at_unix_ms: 500,
            },
        );

        let challenge =
            state.issue_registration_challenge(&config, nostr_pubkey_hex, "session-a", 1_000);
        let signature = sign_provider_payout_target_registration(
            private_key_hex,
            nostr_pubkey_hex,
            "session-a",
            challenge.challenge.as_str(),
            "spark:alice",
        )
        .expect("signature should build");
        let (response, receipt_events) = state
            .register_payout_target(
                &super::ProviderPayoutTargetRegistrationRequest {
                    nostr_pubkey_hex: nostr_pubkey_hex.to_string(),
                    session_id: "session-a".to_string(),
                    spark_address: "spark:alice".to_string(),
                    bitcoin_address: Some("bc1qalice".to_string()),
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
                spark_address: "spark:alice".to_string(),
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
            &[OnlinePylonIdentity {
                nostr_pubkey_hex: "pubkey-a".to_string(),
                sellable: true,
                client_version: None,
                inference_ready: true,
                host_fingerprint: None,
            }],
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
                spark_address: "spark:alice".to_string(),
                bitcoin_address: None,
                registered_at_unix_ms: 10,
                last_verified_at_unix_ms: 10,
            },
        );

        let online = vec![OnlinePylonIdentity {
            nostr_pubkey_hex: "pubkey-a".to_string(),
            sellable: true,
            client_version: None,
            inference_ready: true,
            host_fingerprint: None,
        }];
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
                payout_target: "spark:alice".to_string(),
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
    fn wallet_snapshot_without_ledger_changes_does_not_rewrite_treasury_state() {
        let path = unique_treasury_state_path("wallet-refresh-noop");
        let mut state = TreasuryState::default();
        state.next_challenge_nonce = 1;
        state.state_path = Some(path.clone());
        state.persist();

        let before = std::fs::read_to_string(path.as_path()).expect("read persisted state");
        let receipts = state.apply_wallet_snapshot(
            &TreasuryWalletSnapshot {
                runtime_status: "connected".to_string(),
                runtime_detail: None,
                wallet_hydration_mode: Some("sync_wallet_then_cached_balance".to_string()),
                wallet_payment_scan_mode: Some("recent_only".to_string()),
                balance_sats: 321,
                payments: Vec::new(),
            },
            1_776_028_000_000u64,
        );

        assert!(receipts.is_empty());
        assert_eq!(
            std::fs::read_to_string(path.as_path()).expect("read persisted state"),
            before
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
                    payout_target: format!("spark:{index:04}"),
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
                payout_target: "spark:homework".to_string(),
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

        assert!(state.trim_retention());

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
                    payout_target: format!("spark:{index:08}"),
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
    fn payout_preparation_keeps_pre_cutoff_backlog_but_blocks_new_accrual_below_floor() {
        let mut state = TreasuryState::default();
        let mut config = test_treasury_config();
        config.min_new_accrual_pylon_version = Some("pylon-v0.1.1-rc1".to_string());
        state.payout_targets_by_identity.insert(
            "pubkey-a".to_string(),
            super::RegisteredPayoutTarget {
                nostr_pubkey_hex: "pubkey-a".to_string(),
                source_session_id: "session-a".to_string(),
                spark_address: "spark:alice".to_string(),
                bitcoin_address: None,
                registered_at_unix_ms: 10,
                last_verified_at_unix_ms: 10,
            },
        );

        let now_unix_ms = 1_800_000;
        let payout_interval_ms = config.payout_interval_ms();
        let current_window_started_at_unix_ms =
            payout_window_started_at_for_identity(now_unix_ms, payout_interval_ms, "pubkey-a");
        config.min_new_accrual_started_at_unix_ms = Some(current_window_started_at_unix_ms);
        state.last_payout_reconciliation_at_unix_ms =
            Some(current_window_started_at_unix_ms.saturating_sub(payout_interval_ms));

        let prepared = state.prepare_due_payouts(
            &config,
            &[OnlinePylonIdentity {
                nostr_pubkey_hex: "pubkey-a".to_string(),
                sellable: true,
                client_version: Some("0.0.1-rc12".to_string()),
                inference_ready: true,
                host_fingerprint: None,
            }],
            now_unix_ms,
        );

        assert_eq!(prepared.dispatch_plans.len(), 1);
        assert_eq!(
            prepared.dispatch_plans[0].payout_key,
            payout_window_key(
                current_window_started_at_unix_ms.saturating_sub(payout_interval_ms),
                "pubkey-a"
            )
        );

        let blocked_record = state
            .payout_records_by_key
            .get(payout_window_key(current_window_started_at_unix_ms, "pubkey-a").as_str())
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
                spark_address: "spark:alice".to_string(),
                bitcoin_address: None,
                registered_at_unix_ms: 10,
                last_verified_at_unix_ms: 10,
            },
        );

        let now_unix_ms = 1_800_000;
        config.min_new_accrual_started_at_unix_ms = Some(payout_window_started_at_for_identity(
            now_unix_ms,
            config.payout_interval_ms(),
            "pubkey-a",
        ));
        state.observe_payout_eligibility(
            &config,
            &[OnlinePylonIdentity {
                nostr_pubkey_hex: "pubkey-a".to_string(),
                sellable: true,
                client_version: None,
                inference_ready: true,
                host_fingerprint: None,
            }],
            now_unix_ms,
        );

        let stats = state.public_stats(&config, now_unix_ms);
        assert!(stats.min_new_accrual_version_gate_active);
        assert_eq!(
            stats.min_new_accrual_pylon_version.as_deref(),
            Some("pylon-v0.1.1-rc1")
        );
        assert_eq!(stats.eligible_online_payout_targets, 0);
        assert_eq!(stats.min_new_accrual_version_blocked_online_targets, 1);
        assert_eq!(stats.min_new_accrual_unknown_version_online_targets, 1);
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
                spark_address: "spark:alice".to_string(),
                bitcoin_address: None,
                registered_at_unix_ms: 10,
                last_verified_at_unix_ms: 10,
            },
        );

        let now_unix_ms = 1_800_000;
        let prepared = state.prepare_due_payouts(
            &config,
            &[OnlinePylonIdentity {
                nostr_pubkey_hex: "pubkey-a".to_string(),
                sellable: true,
                client_version: Some("pylon-v0.1.1-rc1".to_string()),
                inference_ready: false,
                host_fingerprint: None,
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
            "pubkey-a",
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
                spark_address: "spark:alice".to_string(),
                bitcoin_address: None,
                registered_at_unix_ms: 10,
                last_verified_at_unix_ms: 10,
            },
        );

        let now_unix_ms = super::now_unix_ms();
        let online = vec![OnlinePylonIdentity {
            nostr_pubkey_hex: "pubkey-a".to_string(),
            sellable: true,
            client_version: Some("pylon-v0.1.1-rc1".to_string()),
            inference_ready: true,
            host_fingerprint: None,
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
        assert_eq!(prepared.dispatch_plans[0].payment_request, "spark:alice");
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
                spark_address: "spark:alice".to_string(),
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
                payout_target: "spark:alice".to_string(),
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
                payout_target: "spark:placeholder".to_string(),
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
        assert_eq!(prepared.dispatch_plans[0].payment_request, "spark:alice");
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
        for (nostr_pubkey_hex, spark_address) in
            [("pubkey-a", "spark:alice"), ("pubkey-b", "spark:bob")]
        {
            state.payout_targets_by_identity.insert(
                nostr_pubkey_hex.to_string(),
                super::RegisteredPayoutTarget {
                    nostr_pubkey_hex: nostr_pubkey_hex.to_string(),
                    source_session_id: format!("session-{nostr_pubkey_hex}"),
                    spark_address: spark_address.to_string(),
                    bitcoin_address: None,
                    registered_at_unix_ms: 10,
                    last_verified_at_unix_ms: 10,
                },
            );
        }

        let now_unix_ms = 1_800_000;
        let online = vec![
            OnlinePylonIdentity {
                nostr_pubkey_hex: "pubkey-a".to_string(),
                sellable: true,
                client_version: Some("pylon-v0.1.1-rc1".to_string()),
                inference_ready: true,
                host_fingerprint: Some("sha256:host-alpha".to_string()),
            },
            OnlinePylonIdentity {
                nostr_pubkey_hex: "pubkey-b".to_string(),
                sellable: true,
                client_version: Some("pylon-v0.1.1-rc1".to_string()),
                inference_ready: true,
                host_fingerprint: Some("sha256:host-alpha".to_string()),
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
                "pubkey-b",
            ),
            "pubkey-b",
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
    fn payout_windows_are_staggered_per_identity() {
        let interval_ms = test_treasury_config().payout_interval_ms();
        let pubkey_a = "pubkey-a";
        let pubkey_b = "pubkey-b";
        let phase_a = payout_phase_offset_ms(pubkey_a, interval_ms);
        let phase_b = payout_phase_offset_ms(pubkey_b, interval_ms);

        assert_ne!(phase_a, phase_b);

        let now_unix_ms = 1_800_000;
        let window_a = payout_window_started_at_for_identity(now_unix_ms, interval_ms, pubkey_a);
        let window_b = payout_window_started_at_for_identity(now_unix_ms, interval_ms, pubkey_b);

        assert_ne!(window_a, window_b);
        assert_eq!(window_a % interval_ms, phase_a);
        assert_eq!(window_b % interval_ms, phase_b);
    }

    #[test]
    fn payout_preparation_uses_identity_phased_windows() {
        let mut state = TreasuryState::default();
        let config = test_treasury_config();
        for (nostr_pubkey_hex, spark_address) in
            [("pubkey-a", "spark:alice"), ("pubkey-b", "spark:bob")]
        {
            state.payout_targets_by_identity.insert(
                nostr_pubkey_hex.to_string(),
                super::RegisteredPayoutTarget {
                    nostr_pubkey_hex: nostr_pubkey_hex.to_string(),
                    source_session_id: format!("session-{nostr_pubkey_hex}"),
                    spark_address: spark_address.to_string(),
                    bitcoin_address: None,
                    registered_at_unix_ms: 10,
                    last_verified_at_unix_ms: 10,
                },
            );
        }

        let online = vec![
            OnlinePylonIdentity {
                nostr_pubkey_hex: "pubkey-a".to_string(),
                sellable: true,
                client_version: None,
                inference_ready: true,
                host_fingerprint: None,
            },
            OnlinePylonIdentity {
                nostr_pubkey_hex: "pubkey-b".to_string(),
                sellable: true,
                client_version: None,
                inference_ready: true,
                host_fingerprint: None,
            },
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
            "pubkey-a",
        );
        let expected_window_b = payout_window_started_at_for_identity(
            now_unix_ms,
            config.payout_interval_ms(),
            "pubkey-b",
        );
        assert!(
            prepared
                .dispatch_plans
                .iter()
                .any(|plan| plan.payout_key == format!("{expected_window_a}:pubkey-a"))
        );
        assert!(
            prepared
                .dispatch_plans
                .iter()
                .any(|plan| plan.payout_key == format!("{expected_window_b}:pubkey-b"))
        );
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
                spark_address: "spark:alice".to_string(),
                bitcoin_address: None,
                registered_at_unix_ms: 10,
                last_verified_at_unix_ms: 10,
            },
        );

        let interval_ms = config.payout_interval_ms();
        let now_unix_ms = 1_800_000;
        state.last_payout_reconciliation_at_unix_ms = Some(now_unix_ms - (interval_ms * 3));

        let prepared = state.prepare_due_payouts(
            &config,
            &[OnlinePylonIdentity {
                nostr_pubkey_hex: "pubkey-a".to_string(),
                sellable: true,
                client_version: None,
                inference_ready: true,
                host_fingerprint: None,
            }],
            now_unix_ms,
        );

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
                spark_address: "spark:alice".to_string(),
                bitcoin_address: None,
                registered_at_unix_ms: 10,
                last_verified_at_unix_ms: 10,
            },
        );

        let interval_ms = config.payout_interval_ms();
        let now_unix_ms = 1_800_000;
        state.last_payout_reconciliation_at_unix_ms = Some(now_unix_ms - (interval_ms * 10));

        let prepared = state.prepare_due_payouts(
            &config,
            &[OnlinePylonIdentity {
                nostr_pubkey_hex: "pubkey-a".to_string(),
                sellable: true,
                client_version: None,
                inference_ready: true,
                host_fingerprint: None,
            }],
            now_unix_ms,
        );

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
                    spark_address: format!("spark:{target_index:03}"),
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
                        payout_target: format!("spark:{target_index:03}"),
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
                payout_target: "spark:unregistered".to_string(),
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
                payout_target: "spark:alice".to_string(),
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
                payments: vec![
                    PaymentSummary {
                        id: "payment-receive-001".to_string(),
                        direction: "receive".to_string(),
                        status: "completed".to_string(),
                        amount_sats: 500,
                        fees_sats: 0,
                        timestamp: now_unix_ms.saturating_div(1_000).saturating_sub(1),
                        method: "spark".to_string(),
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
                        method: "spark".to_string(),
                        description: None,
                        invoice: Some("spark:alice".to_string()),
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
                payout_target: "spark:alice".to_string(),
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
                classification: TreasuryPayoutClassification::default(),
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
                payments: vec![PaymentSummary {
                    id: "payment-send-recovered".to_string(),
                    direction: "send".to_string(),
                    status: "completed".to_string(),
                    amount_sats: 50,
                    fees_sats: 0,
                    timestamp: payment_timestamp,
                    method: "spark".to_string(),
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
                payout_target: "spark:alice".to_string(),
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
                classification: TreasuryPayoutClassification::default(),
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
                payments: vec![PaymentSummary {
                    id: "payment-send-unmatched".to_string(),
                    direction: "send".to_string(),
                    status: "completed".to_string(),
                    amount_sats: 50,
                    fees_sats: 0,
                    timestamp: payment_timestamp,
                    method: "spark".to_string(),
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
    fn public_stats_include_unconfirmed_dispatched_sats_in_visible_total() {
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
                payout_target: "spark:bob".to_string(),
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
        assert_eq!(stats.payout_sats_paid_total, 122);
        assert_eq!(stats.payout_sats_paid_24h, 2);
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
                spark_address: "spark:replay".to_string(),
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
        assert_eq!(prepared.dispatch_plans[0].payment_request, "spark:replay");
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
                spark_address: "spark:homework".to_string(),
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
        assert_eq!(prepared.dispatch_plans[0].payment_request, "spark:homework");
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
    fn degraded_reason_flags_zero_balance_with_receive_history() {
        let mut state = TreasuryState::default();
        state.wallet_balance_sats = 0;
        state.funding_receives_by_payment_id.insert(
            "payment-receive-001".to_string(),
            super::TreasuryFundingReceive {
                payment_id: "payment-receive-001".to_string(),
                status: "completed".to_string(),
                amount_sats: 100_000,
                method: "spark".to_string(),
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
                payout_target: "spark:balance-blocked".to_string(),
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
                payout_target: "spark:balance-blocked".to_string(),
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

        state
            .payout_records_by_key
            .get_mut(payout_key.as_str())
            .expect("queued payout")
            .reason = Some("missing_payout_target".to_string());

        assert!(!state.due_wallet_refresh_requires_reconciliation());
    }

    #[test]
    fn treasury_wallet_refresh_state_treats_confirmations_stalled_as_due() {
        let mut config = test_treasury_config();
        config.enabled = true;
        let mut service_config = crate::ServiceConfig::from_env().expect("service config");
        service_config.treasury = config.clone();
        let state = crate::build_app_state(service_config);
        let now_unix_ms = 1_000_000;
        {
            let mut store = state.store.write().expect("write store");
            store.treasury.last_wallet_sync_at_unix_ms = Some(now_unix_ms);
            let payout_key = "accepted-work:confirmations-stalled".to_string();
            store.treasury.payout_records_by_key.insert(
                payout_key.clone(),
                TreasuryPayoutRecord {
                    payout_key,
                    nostr_pubkey_hex: "pubkey-confirmations-stalled".to_string(),
                    payout_target: "spark:confirmations-stalled".to_string(),
                    amount_sats: 25,
                    status: "dispatched".to_string(),
                    reason: None,
                    payment_id: Some("payment-confirmations-stalled".to_string()),
                    window_started_at_unix_ms: now_unix_ms.saturating_sub(400_000),
                    window_ends_at_unix_ms: now_unix_ms.saturating_sub(399_000),
                    created_at_unix_ms: now_unix_ms.saturating_sub(400_000),
                    updated_at_unix_ms: now_unix_ms.saturating_sub(400_000),
                    sellable_at_window_open: true,
                    dispatch_receipt_recorded: true,
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
        }

        let refresh_state = crate::treasury_wallet_refresh_state(&state, now_unix_ms, false)
            .expect("wallet refresh state");
        assert_eq!(refresh_state, crate::TreasuryWalletRefreshState::Due);
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
                payout_target: "spark:alice".to_string(),
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
    fn failed_accepted_work_retry_claim_respects_wallet_balance_and_placeholder_disable() {
        let mut config = test_treasury_config();
        config.placeholder_payout_mode = TreasuryPlaceholderPayoutMode::Disabled;
        config.daily_budget_cap_sats = 1_000_000;
        let mut state = TreasuryState::default();
        state.wallet_balance_sats = 4;
        let now_unix_ms = super::now_unix_ms();
        let retry_due_updated_at = now_unix_ms
            .saturating_sub(super::TREASURY_FAILED_ACCEPTED_WORK_RETRY_AFTER_MS)
            .saturating_sub(1);

        for (pubkey, target) in [
            ("pubkey-one", "spark:one"),
            ("pubkey-old", "spark:old"),
            ("pubkey-placeholder", "spark:placeholder"),
        ] {
            state.payout_targets_by_identity.insert(
                pubkey.to_string(),
                super::RegisteredPayoutTarget {
                    nostr_pubkey_hex: pubkey.to_string(),
                    source_session_id: format!("session-{pubkey}"),
                    spark_address: target.to_string(),
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
                payout_target: "spark:one".to_string(),
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
                payout_target: "spark:old".to_string(),
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
            "placeholder:old".to_string(),
            TreasuryPayoutRecord {
                payout_key: "placeholder:old".to_string(),
                nostr_pubkey_hex: "pubkey-placeholder".to_string(),
                payout_target: "spark:placeholder".to_string(),
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
        assert!(super::retryable_failed_accepted_work_payout_is_due(
            state
                .payout_records_by_key
                .get("accepted-work:one")
                .expect("one-sat failed accepted-work record"),
            now_unix_ms
        ));

        let prepared = state.prepare_due_payouts(&config, &[], now_unix_ms);

        assert_eq!(prepared.dispatch_plans.len(), 1);
        assert_eq!(prepared.dispatch_plans[0].payout_key, "accepted-work:one");
        assert_eq!(prepared.dispatch_plans[0].amount_sats, 1);
        assert_eq!(prepared.dispatch_plans[0].payment_request, "spark:one");
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
                .get("accepted-work:old")
                .map(|record| (record.status.as_str(), record.reason.as_deref())),
            Some(("failed", Some("wallet_balance_insufficient")))
        );
        assert_eq!(
            state
                .payout_records_by_key
                .get("placeholder:old")
                .map(|record| (record.status.as_str(), record.reason.as_deref())),
            Some(("queued", Some("placeholder_payouts_disabled")))
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
                payout_target: "spark:alice".to_string(),
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
                payout_target: "spark:alice".to_string(),
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
                payout_target: "spark:alice".to_string(),
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
        assert_eq!(raised.len(), 2);
        assert!(
            raised
                .iter()
                .any(|event| event.receipt_type == "treasury.alert.raised")
        );
        assert_eq!(state.active_continuity_alerts.len(), 2);

        if let Some(record) = state.payout_records_by_key.get_mut("pending-a") {
            record.status = "confirmed".to_string();
            record.updated_at_unix_ms = alert_at_unix_ms;
        }
        state.last_dispatch_at_unix_ms = Some(alert_at_unix_ms);
        state.last_confirmed_payout_at_unix_ms = Some(alert_at_unix_ms);
        let cleared = state.sync_continuity_alerts(&config, alert_at_unix_ms + 1);
        assert_eq!(cleared.len(), 2);
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
                payout_target: "spark:alice".to_string(),
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
        assert_eq!(raised.len(), 2);

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
            now_unix_ms.saturating_sub(super::TREASURY_CONTINUITY_ALERT_THRESHOLD_MS + 60_000),
        );
        state.payout_records_by_key.insert(
            "dispatch-backlog".to_string(),
            super::TreasuryPayoutRecord {
                payout_key: "dispatch-backlog".to_string(),
                nostr_pubkey_hex: "pubkey-a".to_string(),
                payout_target: "spark:alice".to_string(),
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

        state.payout_records_by_key.insert(
            "legacy-placeholder-dispatched".to_string(),
            super::TreasuryPayoutRecord {
                payout_key: "legacy-placeholder-dispatched".to_string(),
                nostr_pubkey_hex: "pubkey-a".to_string(),
                payout_target: "spark:alice".to_string(),
                amount_sats: 2,
                status: "dispatched".to_string(),
                reason: None,
                payment_id: Some("legacy-payment-a".to_string()),
                window_started_at_unix_ms: now_unix_ms.saturating_sub(120_000),
                window_ends_at_unix_ms: now_unix_ms.saturating_sub(60_000),
                created_at_unix_ms: now_unix_ms
                    .saturating_sub(super::TREASURY_CONTINUITY_ALERT_THRESHOLD_MS + 10_000),
                updated_at_unix_ms: now_unix_ms
                    .saturating_sub(super::TREASURY_CONTINUITY_ALERT_THRESHOLD_MS + 10_000),
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
                payout_target: "spark:alice".to_string(),
                amount_sats: 2,
                status: "dispatched".to_string(),
                reason: None,
                payment_id: Some("accepted-payment-a".to_string()),
                window_started_at_unix_ms: now_unix_ms.saturating_sub(120_000),
                window_ends_at_unix_ms: now_unix_ms.saturating_sub(60_000),
                created_at_unix_ms: now_unix_ms
                    .saturating_sub(super::TREASURY_CONTINUITY_ALERT_THRESHOLD_MS + 10_000),
                updated_at_unix_ms: now_unix_ms
                    .saturating_sub(super::TREASURY_CONTINUITY_ALERT_THRESHOLD_MS + 10_000),
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
    fn budget_cap_alert_ignores_stale_historical_skips() {
        let mut state = TreasuryState::default();
        let config = test_treasury_config();
        state.payout_targets_by_identity.insert(
            "pubkey-a".to_string(),
            super::RegisteredPayoutTarget {
                nostr_pubkey_hex: "pubkey-a".to_string(),
                source_session_id: "session-a".to_string(),
                spark_address: "spark:alice".to_string(),
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
                payout_target: "spark:alice".to_string(),
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
                nostr_pubkey_hex: "pubkey-a".to_string(),
                sellable: true,
                client_version: None,
                inference_ready: false,
                host_fingerprint: None,
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
        assert_eq!(state.last_payout_reconciliation_at_unix_ms, Some(1_234_567));
        assert_eq!(state.payout_loop_last_completed_at_unix_ms, Some(1_345_678));
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
                spark_address: "spark:alice".to_string(),
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
                payout_target: "spark:stale".to_string(),
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

        let prepared = state.prepare_due_payouts(
            &config,
            &[OnlinePylonIdentity {
                nostr_pubkey_hex: "pubkey-a".to_string(),
                sellable: true,
                client_version: None,
                inference_ready: true,
                host_fingerprint: None,
            }],
            now_unix_ms,
        );

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
    }

    #[tokio::test]
    async fn funding_and_dispatch_hooks_cover_happy_path() {
        let _lock = treasury_test_hook_lock().lock().expect("guard");

        set_test_wallet_funding_hook(Some(Arc::new(|request| {
            Box::pin(async move {
                assert_eq!(request.amount_sats, Some(210));
                Ok(TreasuryFundingMaterial {
                    spark_address: "spark:treasury".to_string(),
                    bitcoin_address: "bc1qtreasury".to_string(),
                    bolt11_invoice: Some("lnbc210fund".to_string()),
                    wallet_snapshot: TreasuryWalletSnapshot {
                        runtime_status: "connected".to_string(),
                        runtime_detail: None,
                        wallet_hydration_mode: None,
                        wallet_payment_scan_mode: None,
                        balance_sats: 500,
                        payments: Vec::new(),
                    },
                })
            })
        })));
        let funding = create_live_funding_target(
            &test_treasury_config(),
            TreasuryFundingTargetRequest {
                amount_sats: Some(210),
                description: Some("fund treasury".to_string()),
                expiry_seconds: Some(60),
            },
        )
        .await
        .expect("funding target should build");
        assert_eq!(funding.spark_address, "spark:treasury");
        assert_eq!(funding.bolt11_invoice.as_deref(), Some("lnbc210fund"));
        set_test_wallet_funding_hook(None);

        set_test_wallet_send_hook(Some(Arc::new(|target, amount_sats| {
            assert_eq!(target, "spark:alice");
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
                payments: vec![PaymentSummary {
                    id: "payment-send-001".to_string(),
                    direction: "send".to_string(),
                    status: "completed".to_string(),
                    amount_sats: 120,
                    fees_sats: 0,
                    timestamp: 300,
                    method: "spark".to_string(),
                    description: None,
                    invoice: Some("spark:alice".to_string()),
                    destination_pubkey: None,
                    payment_hash: None,
                    htlc_status: None,
                    htlc_expiry_epoch_seconds: None,
                    status_detail: None,
                }],
            })
        })));

        let batch = dispatch_live_payouts(
            &test_treasury_config(),
            &[super::TreasuryDispatchPlan {
                payout_key: "window-a:pubkey-a".to_string(),
                payment_request: "spark:alice".to_string(),
                amount_sats: 120,
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
            funding.spark_address,
            "spark:simulated-treasury-proof-wallet"
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
                payment_request: "spark:alice".to_string(),
                amount_sats: 120,
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
                payment_request: "spark:alice".to_string(),
                amount_sats: 120,
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
    fn max_concurrent_send_operations_clamps_to_configured_limit() {
        let mut config = test_treasury_config();
        config.max_concurrent_sends = 16;

        assert_eq!(config.max_concurrent_send_operations(0), 1);
        assert_eq!(config.max_concurrent_send_operations(1), 1);
        assert_eq!(config.max_concurrent_send_operations(4), 4);
        assert_eq!(config.max_concurrent_send_operations(16), 16);
        assert_eq!(config.max_concurrent_send_operations(128), 16);
    }

    #[test]
    fn wallet_refresh_plan_tracks_only_unconfirmed_dispatched_payment_ids() {
        let mut state = TreasuryState::new(PathBuf::from("var/test-treasury-state.json"));
        state.payout_records_by_key.insert(
            "window-a:pubkey-a".to_string(),
            TreasuryPayoutRecord {
                payout_key: "window-a:pubkey-a".to_string(),
                nostr_pubkey_hex: "pubkey-a".to_string(),
                payout_target: "spark:alice".to_string(),
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
                payout_target: "spark:bob".to_string(),
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
                payout_target: "spark:carol".to_string(),
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
    fn wallet_refresh_plan_marks_funded_history_as_nonzero_expected() {
        let mut state = TreasuryState::default();
        state.payout_sats_paid_total = 100;
        state.funding_receives_by_payment_id.insert(
            "receive-001".to_string(),
            TreasuryFundingReceive {
                payment_id: "receive-001".to_string(),
                status: "completed".to_string(),
                amount_sats: TREASURY_IMPOSSIBLE_ZERO_BALANCE_THRESHOLD_SATS + 1_500,
                method: "spark".to_string(),
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
                payout_target: "spark:alice".to_string(),
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
                payout_target: "spark:alice".to_string(),
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
