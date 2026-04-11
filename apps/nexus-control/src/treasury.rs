use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(test)]
use std::sync::Mutex;

use anyhow::{Context, Result, anyhow, bail};
use bip39::{Language, Mnemonic};
use futures::stream::{self, StreamExt};
use openagents_provider_substrate::verify_provider_payout_target_registration_signature;
use openagents_spark::{
    DepositClaimFeePolicy, Network as SparkNetwork, NetworkStatus, PaymentSummary, SparkSigner,
    SparkWallet, WalletConfig,
};
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
const ENV_TREASURY_WALLET_MNEMONIC_PATH: &str = "NEXUS_CONTROL_TREASURY_WALLET_MNEMONIC_PATH";
const ENV_TREASURY_WALLET_STORAGE_DIR: &str = "NEXUS_CONTROL_TREASURY_WALLET_STORAGE_DIR";
const ENV_TREASURY_WALLET_NETWORK: &str = "NEXUS_CONTROL_TREASURY_WALLET_NETWORK";
const ENV_TREASURY_WALLET_API_KEY_ENV: &str = "NEXUS_CONTROL_TREASURY_WALLET_API_KEY_ENV";
const ENV_TREASURY_WALLET_STATUS_REFRESH_SECONDS: &str =
    "NEXUS_CONTROL_TREASURY_WALLET_STATUS_REFRESH_SECONDS";
const ENV_TREASURY_MAX_CONCURRENT_SENDS: &str = "NEXUS_CONTROL_TREASURY_MAX_CONCURRENT_SENDS";
const ENV_TREASURY_RECONCILIATION_HORIZON_SECONDS: &str =
    "NEXUS_CONTROL_TREASURY_RECONCILIATION_HORIZON_SECONDS";
const ENV_TREASURY_POLICY_APPLY_ENV: &str = "NEXUS_CONTROL_TREASURY_POLICY_APPLY_ENV";
const ENV_TREASURY_POLICY_ALLOW_DESTRUCTIVE_ENV_CHANGE: &str =
    "NEXUS_CONTROL_TREASURY_POLICY_ALLOW_DESTRUCTIVE_ENV_CHANGE";
const ENV_TREASURY_POLICY_CHANGE_REASON: &str = "NEXUS_CONTROL_TREASURY_POLICY_CHANGE_REASON";
const ENV_TREASURY_REGISTRATION_CHALLENGE_TTL_SECONDS: &str =
    "NEXUS_CONTROL_TREASURY_REGISTRATION_CHALLENGE_TTL_SECONDS";

const DEFAULT_TREASURY_STATE_PATH: &str = "var/nexus-control/treasury-state.json";
const DEFAULT_TREASURY_ENABLED: bool = false;
const DEFAULT_TREASURY_PAYOUT_SATS_PER_WINDOW: u64 = 0;
const DEFAULT_TREASURY_PAYOUT_INTERVAL_SECONDS: u64 = 3_600;
const DEFAULT_TREASURY_REQUIRE_SELLABLE: bool = false;
const DEFAULT_TREASURY_DAILY_BUDGET_CAP_SATS: u64 = 21_000;
const DEFAULT_TREASURY_WALLET_MNEMONIC_PATH: &str = "var/nexus-control/treasury.mnemonic";
const DEFAULT_TREASURY_WALLET_STORAGE_DIR: &str = "var/nexus-control/treasury-wallet";
const DEFAULT_TREASURY_WALLET_NETWORK: &str = "mainnet";
const DEFAULT_TREASURY_WALLET_STATUS_REFRESH_SECONDS: u64 = 3;
const DEFAULT_TREASURY_MAX_CONCURRENT_SENDS: usize = 16;
const DEFAULT_TREASURY_RECONCILIATION_HORIZON_SECONDS: u64 = 86_400;
const DEFAULT_TREASURY_POLICY_APPLY_ENV: bool = false;
const DEFAULT_TREASURY_POLICY_ALLOW_DESTRUCTIVE_ENV_CHANGE: bool = false;
const DEFAULT_TREASURY_REGISTRATION_CHALLENGE_TTL_SECONDS: u64 = 300;
const TREASURY_PUBLIC_STATS_WINDOW_MS: u64 = 86_400_000;
const TREASURY_PAYOUT_TARGET_DOMAIN: &str = "openagents:nexus-treasury-payout-target:v1";
const TREASURY_POLICY_SCHEMA_VERSION: u32 = 1;
const TREASURY_STATE_RETENTION_WINDOW_MS: u64 = 30 * 86_400_000;
const TREASURY_DISPATCH_RESULT_TIMEOUT_MS: u64 = 60_000;
const TREASURY_TARGET_LIMIT: usize = 8_192;
const TREASURY_PAYOUT_LIMIT: usize = 262_144;
const TREASURY_RECEIVE_LIMIT: usize = 16_384;
const TREASURY_POLICY_CHANGE_LIMIT: usize = 64;
const TREASURY_STATUS_POLICY_CHANGE_LIMIT: usize = 8;
const TREASURY_IMPOSSIBLE_ZERO_BALANCE_THRESHOLD_SATS: u64 = 1_000;
const TREASURY_CONTINUITY_ALERT_THRESHOLD_MS: u64 = 300_000;
const TREASURY_STALE_SNAPSHOT_ALERT_THRESHOLD_MS: u64 = 15_000;
const TREASURY_MAX_CONCURRENT_SENDS_LIMIT: usize = 64;
const TREASURY_MIN_WALLET_REFRESH_TIMEOUT_MS: u64 = 5_000;
const TREASURY_WALLET_REFRESH_PAYMENT_PAGE_SIZE: usize = 100;
const TREASURY_WALLET_REFRESH_MAX_PAYMENT_PAGES: usize = 16;
const TREASURY_STATE_RECOVERY_DROP_FIELD_SETS: &[&[&str]] = &[
    &["public_snapshot"],
    &["public_snapshot", "active_continuity_alerts"],
    &[
        "public_snapshot",
        "active_continuity_alerts",
        "last_wallet_recovery_report",
    ],
];

#[derive(Debug, Clone)]
pub struct TreasuryConfig {
    pub enabled: bool,
    pub payout_sats_per_window: u64,
    pub payout_interval_seconds: u64,
    pub require_sellable: bool,
    pub daily_budget_cap_sats: u64,
    pub reconciliation_horizon_seconds: u64,
    pub apply_env_policy: bool,
    pub allow_destructive_env_policy_change: bool,
    pub policy_change_reason: Option<String>,
    pub state_path: PathBuf,
    pub wallet_mnemonic_path: PathBuf,
    pub wallet_storage_dir: PathBuf,
    pub wallet_network: String,
    pub wallet_api_key_env: Option<String>,
    pub wallet_status_refresh_seconds: u64,
    pub max_concurrent_sends: usize,
    pub registration_challenge_ttl_seconds: u64,
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
        let wallet_status_refresh_seconds = parse_u64_env(
            ENV_TREASURY_WALLET_STATUS_REFRESH_SECONDS,
            DEFAULT_TREASURY_WALLET_STATUS_REFRESH_SECONDS,
        )?
        .max(1);
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
            wallet_status_refresh_seconds,
            max_concurrent_sends,
            registration_challenge_ttl_seconds,
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
        TREASURY_DISPATCH_RESULT_TIMEOUT_MS
            .max(self.wallet_status_refresh_seconds.saturating_mul(2_000))
            .max(payout_interval_ms.saturating_mul(2))
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
        )
    }

    pub fn new(
        treasury_enabled: bool,
        payout_sats_per_window: u64,
        payout_interval_seconds: u64,
        require_sellable: bool,
        daily_budget_cap_sats: u64,
    ) -> Self {
        let payload = TreasuryRuntimePolicyChecksumPayload {
            schema_version: TREASURY_POLICY_SCHEMA_VERSION,
            treasury_enabled,
            payout_sats_per_window,
            payout_interval_seconds,
            require_sellable,
            daily_budget_cap_sats,
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
            checksum,
        }
    }

    pub fn payout_interval_ms(&self) -> u64 {
        self.payout_interval_seconds.saturating_mul(1_000)
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
    pub registered_payout_identities: u64,
    pub wallet_balance_sats: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wallet_balance_updated_at_unix_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wallet_runtime_status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wallet_last_error: Option<String>,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snapshot_age_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wallet_sync_lag_ms: Option<u64>,
    #[serde(default)]
    pub eligible_online_payout_targets: u64,
    #[serde(default)]
    pub sellable_pylons_online_now: u64,
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
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TreasuryPublicSnapshot {
    pub generated_at_unix_ms: u64,
    pub treasury_enabled: bool,
    pub payout_sats_per_window: u64,
    pub payout_interval_seconds: u64,
    pub require_sellable: bool,
    pub daily_budget_cap_sats: u64,
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
    pub payouts_dispatched_24h: u64,
    pub payouts_confirmed_24h: u64,
    pub payouts_failed_24h: u64,
    pub payouts_skipped_24h: u64,
    pub eligible_online_payout_targets: u64,
    pub sellable_pylons_online_now: u64,
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
    pub registered_payout_identities: u64,
    pub wallet_balance_sats: u64,
    pub wallet_balance_updated_at_unix_ms: Option<u64>,
    pub wallet_runtime_status: Option<String>,
    pub wallet_last_error: Option<String>,
    pub wallet_storage_runtime_mode: String,
    pub payout_loop_runtime_status: Option<String>,
    pub payout_loop_last_error: Option<String>,
    pub last_payout_reconciliation_at_unix_ms: Option<u64>,
    pub payout_loop_last_started_at_unix_ms: Option<u64>,
    pub payout_loop_last_completed_at_unix_ms: Option<u64>,
    pub public_snapshot_generated_at_unix_ms: Option<u64>,
    pub snapshot_age_ms: Option<u64>,
    pub wallet_sync_lag_ms: Option<u64>,
    pub eligible_online_payout_targets: u64,
    pub sellable_pylons_online_now: u64,
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
    pub payouts_dispatched_24h: u64,
    pub payouts_confirmed_24h: u64,
    pub payouts_failed_24h: u64,
    pub payouts_skipped_24h: u64,
    pub skip_reason_metrics_24h: Vec<TreasuryReasonMetric>,
    pub fail_reason_metrics_24h: Vec<TreasuryReasonMetric>,
    pub active_continuity_alerts: Vec<TreasuryContinuityAlert>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OnlinePylonIdentity {
    pub nostr_pubkey_hex: String,
    pub sellable: bool,
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

#[derive(Debug, Clone, Default)]
pub struct TreasuryWalletSnapshot {
    pub runtime_status: String,
    pub runtime_detail: Option<String>,
    pub balance_sats: u64,
    pub payments: Vec<PaymentSummary>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TreasuryWalletRefreshPlan {
    tracked_payment_ids: BTreeSet<String>,
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
        wallet_refresh_payment_page_budget(self.tracked_payment_count())
    }
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

impl TreasuryState {
    pub fn new(state_path: PathBuf) -> Self {
        let mut loaded = match fs::read_to_string(state_path.as_path()) {
            Ok(payload) => match serde_json::from_str::<Self>(payload.as_str()) {
                Ok(state) => state,
                Err(error) => recovered_treasury_state_from_payload(payload.as_str(), &error),
            },
            Err(_) => Self::default(),
        };
        loaded.state_path = Some(state_path);
        if loaded.next_challenge_nonce == 0 {
            loaded.next_challenge_nonce = 1;
        }
        loaded.trim_policy_change_history();
        loaded.trim_retention();
        loaded.rebuild_payment_index();
        loaded
    }

    pub fn apply_paid_total_floor(&mut self, payout_sats_paid_total_floor: u64) -> Option<u64> {
        if payout_sats_paid_total_floor <= self.payout_sats_paid_total {
            return None;
        }
        let previous_total = self.payout_sats_paid_total;
        self.payout_sats_paid_total = payout_sats_paid_total_floor;
        if let Some(snapshot) = self.public_snapshot.as_mut() {
            snapshot.payout_sats_paid_total = snapshot
                .payout_sats_paid_total
                .max(payout_sats_paid_total_floor);
        }
        self.persist();
        Some(previous_total)
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
        self.last_wallet_sync_at_unix_ms.is_none_or(|last_sync| {
            now_unix_ms.saturating_sub(last_sync) >= config.wallet_status_refresh_interval_ms()
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
        if !policy.treasury_enabled || policy.payout_interval_seconds == 0 {
            return;
        }

        let payout_interval_ms = policy.payout_interval_ms();
        let mut latest_eligible_window_started_at_unix_ms: Option<u64> = None;
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
            self.eligible_online_payout_targets =
                self.eligible_online_payout_targets.saturating_add(1);
            let window_started_at_unix_ms = payout_window_started_at_for_identity(
                now_unix_ms,
                payout_interval_ms,
                identity.nostr_pubkey_hex.as_str(),
            );
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

        if policy.treasury_enabled {
            if latest_eligible_window_started_at_unix_ms.is_some_and(|window_started_at| {
                self.eligible_online_payout_targets > 0
                    && window_started_at > self.last_dispatch_at_unix_ms.unwrap_or(0)
                    && now_unix_ms.saturating_sub(window_started_at)
                        >= TREASURY_CONTINUITY_ALERT_THRESHOLD_MS
            }) {
                active_alerts.push(TreasuryContinuityAlert {
                    alert_id: "dispatch_stalled".to_string(),
                    severity: "critical".to_string(),
                    reason: "eligible_windows_not_dispatching".to_string(),
                    started_at_unix_ms: latest_eligible_window_started_at_unix_ms
                        .unwrap_or(now_unix_ms),
                    observed_at_unix_ms: now_unix_ms,
                });
            }

            if latest_eligible_window_started_at_unix_ms.is_some_and(|window_started_at| {
                self.eligible_online_payout_targets > 0
                    && window_started_at > self.last_confirmed_payout_at_unix_ms.unwrap_or(0)
                    && now_unix_ms.saturating_sub(window_started_at)
                        >= TREASURY_CONTINUITY_ALERT_THRESHOLD_MS
            }) {
                active_alerts.push(TreasuryContinuityAlert {
                    alert_id: "confirmations_stalled".to_string(),
                    severity: "critical".to_string(),
                    reason: "eligible_windows_not_confirming".to_string(),
                    started_at_unix_ms: latest_eligible_window_started_at_unix_ms
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
        if snapshot_age_ms.is_some_and(|lag| lag >= stale_after_ms)
            || wallet_sync_lag_ms.is_some_and(|lag| lag >= stale_after_ms)
        {
            active_alerts.push(TreasuryContinuityAlert {
                alert_id: "snapshot_stale".to_string(),
                severity: "warning".to_string(),
                reason: "treasury_snapshot_or_wallet_sync_stale".to_string(),
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

        self.active_continuity_alerts = next_alerts;
        self.persist();
        receipts
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
        (
            self.wallet_runtime_status.clone(),
            self.wallet_last_error.clone(),
        )
    }

    fn degraded_reason(&self, config: &TreasuryConfig, now_unix_ms: u64) -> Option<String> {
        let (wallet_runtime_status, wallet_last_error) =
            self.wallet_runtime_view(config, now_unix_ms);
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
        if let Some(alert) = self
            .active_continuity_alerts
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
            if lag_ms >= config.wallet_snapshot_stale_after_ms() {
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
        let mut payout_sats_paid_24h = 0u64;
        let mut unconfirmed_dispatched_sats_total = 0u64;
        let mut unconfirmed_dispatched_sats_24h = 0u64;
        let mut payouts_dispatched_24h = 0u64;
        let mut payouts_confirmed_24h = 0u64;
        let mut payouts_failed_24h = 0u64;
        let mut payouts_skipped_24h = 0u64;

        for record in self.payout_records_by_key.values() {
            if record.status == "dispatched" && !record.counted_in_paid_total {
                unconfirmed_dispatched_sats_total =
                    unconfirmed_dispatched_sats_total.saturating_add(record.amount_sats);
                if record.updated_at_unix_ms >= window_started_at_unix_ms {
                    unconfirmed_dispatched_sats_24h =
                        unconfirmed_dispatched_sats_24h.saturating_add(record.amount_sats);
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
                    payout_sats_paid_24h = payout_sats_paid_24h.saturating_add(record.amount_sats);
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

        TreasuryPublicSnapshot {
            generated_at_unix_ms: now_unix_ms,
            treasury_enabled: policy.treasury_enabled,
            payout_sats_per_window: policy.payout_sats_per_window,
            payout_interval_seconds: policy.payout_interval_seconds,
            require_sellable: policy.require_sellable,
            daily_budget_cap_sats: policy.daily_budget_cap_sats,
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
            payout_sats_paid_total: self
                .payout_sats_paid_total
                .saturating_add(unconfirmed_dispatched_sats_total),
            payout_sats_paid_24h: payout_sats_paid_24h
                .saturating_add(unconfirmed_dispatched_sats_24h),
            payouts_dispatched_24h,
            payouts_confirmed_24h,
            payouts_failed_24h,
            payouts_skipped_24h,
            eligible_online_payout_targets: continuity.eligible_online_payout_targets,
            sellable_pylons_online_now: continuity.sellable_pylons_online_now,
            latest_eligible_window_started_at_unix_ms: continuity
                .latest_eligible_window_started_at_unix_ms,
            last_dispatch_at_unix_ms: continuity.last_dispatch_at_unix_ms,
            last_confirmed_payout_at_unix_ms: continuity.last_confirmed_at_unix_ms,
            skip_reason_metrics_24h: continuity.skip_reason_metrics_24h,
            fail_reason_metrics_24h: continuity.fail_reason_metrics_24h,
            active_continuity_alerts: self.active_continuity_alerts.clone(),
            degraded_reason: self.degraded_reason(config, now_unix_ms),
        }
    }

    pub fn refresh_public_snapshot(&mut self, config: &TreasuryConfig, now_unix_ms: u64) {
        self.public_snapshot = Some(self.build_public_snapshot(config, now_unix_ms));
        self.persist();
    }

    pub fn public_stats(&self, config: &TreasuryConfig, now_unix_ms: u64) -> TreasuryPublicStats {
        let snapshot = self
            .public_snapshot
            .clone()
            .unwrap_or_else(|| self.build_public_snapshot(config, now_unix_ms));
        let (wallet_runtime_status, wallet_last_error) =
            self.wallet_runtime_view(config, now_unix_ms);
        let wallet_sync_lag_ms = self
            .latest_wallet_activity_at_unix_ms()
            .map(|last_activity| now_unix_ms.saturating_sub(last_activity));
        TreasuryPublicStats {
            treasury_enabled: snapshot.treasury_enabled,
            payout_sats_per_window: snapshot.payout_sats_per_window,
            payout_interval_seconds: snapshot.payout_interval_seconds,
            require_sellable: snapshot.require_sellable,
            daily_budget_cap_sats: snapshot.daily_budget_cap_sats,
            registered_payout_identities: snapshot.registered_payout_identities,
            wallet_balance_sats: snapshot.wallet_balance_sats,
            wallet_balance_updated_at_unix_ms: snapshot.wallet_balance_updated_at_unix_ms,
            wallet_runtime_status,
            wallet_last_error,
            wallet_storage_runtime_mode: snapshot.wallet_storage_runtime_mode,
            payout_loop_runtime_status: snapshot.payout_loop_runtime_status,
            payout_loop_last_error: snapshot.payout_loop_last_error,
            last_payout_reconciliation_at_unix_ms: snapshot.last_payout_reconciliation_at_unix_ms,
            payout_loop_last_started_at_unix_ms: snapshot.payout_loop_last_started_at_unix_ms,
            payout_loop_last_completed_at_unix_ms: snapshot.payout_loop_last_completed_at_unix_ms,
            public_snapshot_generated_at_unix_ms: Some(snapshot.generated_at_unix_ms),
            snapshot_age_ms: Some(now_unix_ms.saturating_sub(snapshot.generated_at_unix_ms)),
            wallet_sync_lag_ms,
            eligible_online_payout_targets: snapshot.eligible_online_payout_targets,
            sellable_pylons_online_now: snapshot.sellable_pylons_online_now,
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
            payout_loop_health: self.payout_loop_health(config),
            degraded_reason: self.degraded_reason(config, now_unix_ms),
            payout_sats_paid_total: snapshot.payout_sats_paid_total,
            payout_sats_paid_24h: snapshot.payout_sats_paid_24h,
            payouts_dispatched_24h: snapshot.payouts_dispatched_24h,
            payouts_confirmed_24h: snapshot.payouts_confirmed_24h,
            payouts_failed_24h: snapshot.payouts_failed_24h,
            payouts_skipped_24h: snapshot.payouts_skipped_24h,
            skip_reason_metrics_24h: snapshot.skip_reason_metrics_24h,
            fail_reason_metrics_24h: snapshot.fail_reason_metrics_24h,
            active_continuity_alerts: snapshot.active_continuity_alerts,
        }
    }

    pub fn status_response(
        &self,
        config: &TreasuryConfig,
        now_unix_ms: u64,
    ) -> TreasuryStatusResponse {
        let stats = self.public_stats(config, now_unix_ms);
        let policy = self.active_policy(config);
        TreasuryStatusResponse {
            authority: "openagents-hosted-nexus".to_string(),
            treasury_enabled: stats.treasury_enabled,
            payout_sats_per_window: stats.payout_sats_per_window,
            payout_interval_seconds: stats.payout_interval_seconds,
            require_sellable: stats.require_sellable,
            daily_budget_cap_sats: stats.daily_budget_cap_sats,
            registered_payout_identities: stats.registered_payout_identities,
            wallet_balance_sats: stats.wallet_balance_sats,
            wallet_balance_updated_at_unix_ms: stats.wallet_balance_updated_at_unix_ms,
            wallet_runtime_status: stats.wallet_runtime_status,
            wallet_last_error: stats.wallet_last_error,
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
            snapshot_age_ms: stats.snapshot_age_ms,
            wallet_sync_lag_ms: stats.wallet_sync_lag_ms,
            eligible_online_payout_targets: stats.eligible_online_payout_targets,
            sellable_pylons_online_now: stats.sellable_pylons_online_now,
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
            payouts_dispatched_24h: stats.payouts_dispatched_24h,
            payouts_confirmed_24h: stats.payouts_confirmed_24h,
            payouts_failed_24h: stats.payouts_failed_24h,
            payouts_skipped_24h: stats.payouts_skipped_24h,
            skip_reason_metrics_24h: stats.skip_reason_metrics_24h,
            fail_reason_metrics_24h: stats.fail_reason_metrics_24h,
            active_continuity_alerts: stats.active_continuity_alerts,
        }
    }

    pub fn record_wallet_error(&mut self, detail: impl Into<String>) {
        self.wallet_runtime_status = Some("error".to_string());
        self.wallet_last_error = Some(detail.into());
        self.persist();
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
    }

    pub fn note_payout_loop_completed(
        &mut self,
        now_unix_ms: u64,
        reconciliation_degraded_reason: Option<String>,
    ) {
        self.last_payout_reconciliation_at_unix_ms = Some(now_unix_ms);
        self.payout_loop_last_completed_at_unix_ms = Some(now_unix_ms);
        if let Some(reason) = reconciliation_degraded_reason {
            self.payout_loop_runtime_status = Some("degraded".to_string());
            self.payout_loop_last_error = Some(reason);
        } else {
            self.payout_loop_runtime_status = Some("idle".to_string());
            self.payout_loop_last_error = None;
        }
        self.persist();
    }

    pub fn note_payout_loop_error(&mut self, now_unix_ms: u64, detail: impl Into<String>) {
        self.payout_loop_runtime_status = Some("error".to_string());
        self.payout_loop_last_error = Some(detail.into());
        self.payout_loop_last_completed_at_unix_ms = Some(now_unix_ms);
        self.persist();
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
        self.persist();
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
        let Some(challenge) = self.registration_challenges_by_key.get_mut(&challenge_key) else {
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
        challenge.consumed = true;

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

    pub fn prepare_due_payouts(
        &mut self,
        config: &TreasuryConfig,
        online_identities: &[OnlinePylonIdentity],
        now_unix_ms: u64,
    ) -> TreasuryPayoutPreparation {
        self.trim_retention();
        let mut receipt_events = self.expire_stale_dispatches(config, now_unix_ms);
        let policy = self.active_policy(config);
        if !policy.treasury_enabled
            || policy.payout_sats_per_window == 0
            || policy.payout_interval_seconds == 0
            || online_identities.is_empty()
        {
            self.refresh_public_snapshot(config, now_unix_ms);
            return TreasuryPayoutPreparation {
                dispatch_plans: Vec::new(),
                receipt_events,
                reconciliation_degraded_reason: None,
            };
        }

        let payout_interval_ms = policy.payout_interval_ms();
        let (reconciliation_started_at_unix_ms, reconciliation_degraded_reason) =
            self.payout_reconciliation_started_at(config, now_unix_ms);
        let mut reserved_budget_sats = self.reserved_budget_last_24h(now_unix_ms);
        let mut dispatch_plans = Vec::new();

        for identity in online_identities {
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
                        };
                        self.payout_records_by_key
                            .insert(payout_key, record.clone());
                        receipt_events.push(skipped_payout_receipt(&record));
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
                        };
                        self.payout_records_by_key
                            .insert(payout_key, record.clone());
                        receipt_events.push(skipped_payout_receipt(&record));
                        if window_started_at_unix_ms >= current_window_started_at_unix_ms {
                            break;
                        }
                        window_started_at_unix_ms =
                            window_started_at_unix_ms.saturating_add(payout_interval_ms);
                        continue;
                    }

                    if policy.daily_budget_cap_sats > 0
                        && reserved_budget_sats.saturating_add(policy.payout_sats_per_window)
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
                        };
                        self.payout_records_by_key
                            .insert(payout_key, record.clone());
                        receipt_events.push(skipped_payout_receipt(&record));
                        if window_started_at_unix_ms >= current_window_started_at_unix_ms {
                            break;
                        }
                        window_started_at_unix_ms =
                            window_started_at_unix_ms.saturating_add(payout_interval_ms);
                        continue;
                    }

                    reserved_budget_sats =
                        reserved_budget_sats.saturating_add(policy.payout_sats_per_window);
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
                        },
                    );
                    dispatch_plans.push(TreasuryDispatchPlan {
                        payout_key,
                        payment_request: target.spark_address,
                        amount_sats: policy.payout_sats_per_window,
                    });
                }

                if window_started_at_unix_ms >= current_window_started_at_unix_ms {
                    break;
                }
                window_started_at_unix_ms =
                    window_started_at_unix_ms.saturating_add(payout_interval_ms);
            }
        }

        self.refresh_public_snapshot(config, now_unix_ms);
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
        self.wallet_balance_sats = snapshot.balance_sats;
        self.wallet_balance_updated_at_unix_ms = Some(now_unix_ms);
        self.last_wallet_sync_at_unix_ms = Some(now_unix_ms);

        let mut receipt_events = Vec::new();
        let mut last_confirmed_payout_at_unix_ms = self.last_confirmed_payout_at_unix_ms;
        for payment in &snapshot.payments {
            if payment.direction.eq_ignore_ascii_case("receive") {
                self.funding_receives_by_payment_id
                    .entry(payment.id.clone())
                    .and_modify(|existing| {
                        existing.status = payment.status.clone();
                        existing.amount_sats = payment.amount_sats;
                        existing.method = payment.method.clone();
                        existing.description = payment.description.clone();
                        existing.updated_at_unix_ms = payment.timestamp.saturating_mul(1_000);
                    })
                    .or_insert(TreasuryFundingReceive {
                        payment_id: payment.id.clone(),
                        status: payment.status.clone(),
                        amount_sats: payment.amount_sats,
                        method: payment.method.clone(),
                        description: payment.description.clone(),
                        recorded_at_unix_ms: payment.timestamp.saturating_mul(1_000),
                        updated_at_unix_ms: payment.timestamp.saturating_mul(1_000),
                    });
            }

            if !payment.direction.eq_ignore_ascii_case("send") {
                continue;
            }
            let Some(payout_key) = self.payout_key_for_payment_id(payment.id.as_str()) else {
                continue;
            };
            let Some(record) = self.payout_records_by_key.get_mut(&payout_key) else {
                self.payout_key_by_payment_id.remove(payment.id.as_str());
                continue;
            };
            record.updated_at_unix_ms = payment.timestamp.saturating_mul(1_000);
            if wallet_payment_is_confirmed(payment) {
                let confirmed_at_unix_ms = payment.timestamp.saturating_mul(1_000);
                last_confirmed_payout_at_unix_ms = Some(
                    last_confirmed_payout_at_unix_ms
                        .unwrap_or(confirmed_at_unix_ms)
                        .max(confirmed_at_unix_ms),
                );
                record.status = "confirmed".to_string();
                record.reason = None;
                if !record.confirm_receipt_recorded {
                    record.confirm_receipt_recorded = true;
                    receipt_events.push(confirmed_payout_receipt(record, payment.id.as_str()));
                }
                if !record.counted_in_paid_total {
                    record.counted_in_paid_total = true;
                    self.payout_sats_paid_total = self
                        .payout_sats_paid_total
                        .saturating_add(record.amount_sats);
                }
            } else if wallet_payment_is_failed(payment) {
                record.status = "failed".to_string();
                record.reason = payment
                    .status_detail
                    .clone()
                    .or_else(|| Some(payment.status.clone()));
                if !record.fail_receipt_recorded {
                    record.fail_receipt_recorded = true;
                    receipt_events.push(failed_payout_receipt(record));
                }
            }
        }
        self.last_confirmed_payout_at_unix_ms = last_confirmed_payout_at_unix_ms;

        self.trim_retention();
        self.persist();
        receipt_events
    }

    pub fn last_persistence_error(&self) -> Option<String> {
        self.last_persistence_error.clone()
    }

    fn reserved_budget_last_24h(&self, now_unix_ms: u64) -> u64 {
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
    ) -> Vec<TreasuryReceiptEvent> {
        let timeout_ms =
            config.dispatch_result_timeout_ms(self.active_policy(config).payout_interval_ms());
        let mut receipt_events = Vec::new();
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
        }
        receipt_events
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

    fn trim_policy_change_history(&mut self) {
        if self.policy_change_history.len() <= TREASURY_POLICY_CHANGE_LIMIT {
            return;
        }
        let overflow = self
            .policy_change_history
            .len()
            .saturating_sub(TREASURY_POLICY_CHANGE_LIMIT);
        self.policy_change_history.drain(0..overflow);
    }

    fn prune_challenges(&mut self, now_unix_ms: u64) {
        self.registration_challenges_by_key.retain(|_, challenge| {
            !challenge.consumed && now_unix_ms <= challenge.expires_at_unix_ms
        });
    }

    fn trim_retention(&mut self) {
        if self.next_challenge_nonce == 0 {
            self.next_challenge_nonce = 1;
        }
        self.trim_policy_change_history();
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
        }
        let now_unix_ms = now_unix_ms();
        let oldest_allowed = now_unix_ms.saturating_sub(TREASURY_STATE_RETENTION_WINDOW_MS);
        self.payout_records_by_key
            .retain(|_, record| record.updated_at_unix_ms >= oldest_allowed);
        self.funding_receives_by_payment_id
            .retain(|_, receive| receive.updated_at_unix_ms >= oldest_allowed);
        self.prune_challenges(now_unix_ms);
        self.rebuild_payment_index();
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

pub async fn create_live_funding_target(
    config: &TreasuryConfig,
    request: TreasuryFundingTargetRequest,
) -> Result<TreasuryFundingMaterial> {
    #[cfg(test)]
    if let Some(hook) = test_wallet_funding_hook()
        .lock()
        .expect("treasury funding hook")
        .as_ref()
    {
        return hook(request);
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
        let wallet_snapshot = wallet_snapshot_from_wallet(wallet.as_ref()).await?;
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
    load_live_wallet_snapshot_with_plan(
        config,
        create_if_missing,
        TreasuryWalletRefreshPlan::recent_only(),
    )
    .await
}

pub async fn load_live_wallet_snapshot_with_plan(
    config: &TreasuryConfig,
    create_if_missing: bool,
    refresh_plan: TreasuryWalletRefreshPlan,
) -> Result<TreasuryWalletSnapshot> {
    #[cfg(test)]
    if let Some(hook) = test_wallet_snapshot_hook()
        .lock()
        .expect("treasury snapshot hook")
        .as_ref()
    {
        return hook();
    }

    with_live_wallet(config, create_if_missing, move |wallet| async move {
        wallet_snapshot_from_wallet_with_plan(wallet.as_ref(), &refresh_plan).await
    })
    .await
}

pub async fn dispatch_live_payouts(
    config: &TreasuryConfig,
    plans: &[TreasuryDispatchPlan],
) -> TreasuryDispatchBatchResult {
    if plans.is_empty() {
        return TreasuryDispatchBatchResult::default();
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

    // Keep the wallet lock held for less than one payout interval even when a
    // large provider set becomes due in the same cycle.
    let max_concurrent_sends = config.max_concurrent_send_operations(plans.len());
    let mut indexed_outcomes = stream::iter(plans.iter().cloned().enumerate())
        .map(|(index, plan)| {
            let wallet = wallet.clone();
            async move {
                let outcome = match wallet
                    .send_payment_simple(plan.payment_request.as_str(), Some(plan.amount_sats))
                    .await
                {
                    Ok(payment_id) => TreasuryDispatchOutcome::Dispatched {
                        payout_key: plan.payout_key,
                        payment_id,
                    },
                    Err(error) => TreasuryDispatchOutcome::Failed {
                        payout_key: plan.payout_key,
                        reason: error.to_string(),
                    },
                };
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

    TreasuryDispatchBatchResult {
        outcomes,
        // The dedicated wallet refresh loop reconciles confirms and balance.
        // Keeping the full wallet scan out of the dispatch path preserves the
        // intended payout cadence even when many Pylons are online.
        wallet_snapshot: None,
        wallet_error: None,
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
    let validation_passed = current_storage.error.is_none()
        && rebuilt_storage.error.is_none()
        && wallet_identity_pubkey_match;
    let recommended_action = if !validation_passed {
        "inspect_errors".to_string()
    } else if major_divergence_detected {
        "cutover_rebuilt_storage_after_service_stop".to_string()
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
    config: &TreasuryConfig,
    mnemonic: &str,
    storage_dir: &Path,
) -> TreasuryWalletInspection {
    let mut inspection = TreasuryWalletInspection {
        inspected_storage_dir: storage_dir.display().to_string(),
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
    let network = match parse_wallet_network(config.wallet_network.as_str()) {
        Ok(network) => network,
        Err(error) => {
            inspection.error = Some(error.to_string());
            return inspection;
        }
    };
    let wallet = match SparkWallet::new(
        signer,
        WalletConfig {
            network,
            api_key: resolve_wallet_api_key(config.wallet_api_key_env.as_deref()),
            storage_dir: storage_dir.to_path_buf(),
            deposit_claim_fee_policy: DepositClaimFeePolicy::Auto,
        },
    )
    .await
    {
        Ok(wallet) => wallet,
        Err(error) => {
            inspection.error = Some(format!(
                "failed to initialize treasury Spark wallet: {error}"
            ));
            return inspection;
        }
    };

    let network_status = wallet.network_status().await;
    inspection.runtime_status = Some(wallet_network_status_label(&network_status).to_string());
    inspection.runtime_detail = network_status.detail;

    match wallet.get_balance().await {
        Ok(balance) => inspection.balance_sats = Some(balance.total_sats()),
        Err(error) => {
            inspection.error = Some(format!("failed to fetch treasury Spark balance: {error}"));
            let _ = wallet.disconnect().await;
            return inspection;
        }
    }

    match wallet.list_all_payments().await {
        Ok(payments) => {
            inspection.payment_totals = aggregate_payment_summaries(&payments);
        }
        Err(error) => {
            inspection.error = Some(format!("failed to list treasury Spark payments: {error}"));
            let _ = wallet.disconnect().await;
            return inspection;
        }
    }

    match wallet.list_unclaimed_deposits().await {
        Ok(deposits) => {
            inspection.unclaimed_deposit_totals = aggregate_unclaimed_deposits(&deposits);
        }
        Err(error) => {
            inspection.error = Some(format!(
                "failed to list treasury Spark unclaimed deposits: {error}"
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

    let current_storage = inspect_treasury_wallet_storage(
        config,
        mnemonic.as_str(),
        current_storage_backup_dir.as_path(),
    )
    .await;
    let rebuilt_storage =
        inspect_treasury_wallet_storage(config, mnemonic.as_str(), rebuilt_storage_dir.as_path())
            .await;
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

fn render_treasury_status_response(response: &TreasuryStatusResponse) -> String {
    let mut lines = vec![
        format!("treasury_enabled: {}", response.treasury_enabled),
        format!("wallet_balance_sats: {}", response.wallet_balance_sats),
        format!(
            "wallet_storage_runtime_mode: {}",
            response.wallet_storage_runtime_mode
        ),
        format!(
            "payout_sats_paid_total: {}",
            response.payout_sats_paid_total
        ),
        format!("payout_sats_paid_24h: {}", response.payout_sats_paid_24h),
        format!(
            "registered_payout_identities: {}",
            response.registered_payout_identities
        ),
    ];
    if let Some(status) = response.wallet_runtime_status.as_deref() {
        lines.push(format!("wallet_runtime_status: {status}"));
    }
    if let Some(error) = response.wallet_last_error.as_deref() {
        lines.push(format!("wallet_last_error: {error}"));
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
    if let Some(snapshot_age_ms) = response.snapshot_age_ms {
        lines.push(format!("snapshot_age_ms: {snapshot_age_ms}"));
    }
    if let Some(wallet_sync_lag_ms) = response.wallet_sync_lag_ms {
        lines.push(format!("wallet_sync_lag_ms: {wallet_sync_lag_ms}"));
    }
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
    if !record.payout_target.is_empty() {
        attributes.insert(
            "payout_target".to_string(),
            truncate_target(record.payout_target.as_str()),
        );
    }
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

#[derive(Debug, Clone, PartialEq, Eq)]
struct LiveWalletCacheKey {
    wallet_mnemonic_path: PathBuf,
    wallet_storage_dir: PathBuf,
    wallet_network: String,
    wallet_api_key_env: Option<String>,
}

#[derive(Clone)]
struct LiveWalletCacheEntry {
    key: LiveWalletCacheKey,
    wallet: Arc<SparkWallet>,
}

fn live_wallet_cache() -> &'static AsyncMutex<Option<LiveWalletCacheEntry>> {
    static CACHE: OnceLock<AsyncMutex<Option<LiveWalletCacheEntry>>> = OnceLock::new();
    CACHE.get_or_init(|| AsyncMutex::new(None))
}

fn live_wallet_operation_lock() -> &'static AsyncMutex<()> {
    static LOCK: OnceLock<AsyncMutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| AsyncMutex::new(()))
}

fn live_wallet_cache_key(config: &TreasuryConfig) -> LiveWalletCacheKey {
    LiveWalletCacheKey {
        wallet_mnemonic_path: config.wallet_mnemonic_path.clone(),
        wallet_storage_dir: config.wallet_storage_dir.clone(),
        wallet_network: config.wallet_network.clone(),
        wallet_api_key_env: config.wallet_api_key_env.clone(),
    }
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
    operation(wallet).await
}

async fn open_wallet(config: &TreasuryConfig, create_if_missing: bool) -> Result<Arc<SparkWallet>> {
    let cache_key = live_wallet_cache_key(config);
    let cache = live_wallet_cache();
    let mut cache_guard = cache.lock().await;
    if let Some(entry) = cache_guard.as_ref()
        && entry.key == cache_key
    {
        return Ok(entry.wallet.clone());
    }
    let wallet = Arc::new(open_wallet_uncached(config, create_if_missing).await?);
    *cache_guard = Some(LiveWalletCacheEntry {
        key: cache_key,
        wallet: wallet.clone(),
    });
    Ok(wallet)
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
        WalletConfig {
            network: parse_wallet_network(config.wallet_network.as_str())?,
            api_key: resolve_wallet_api_key(config.wallet_api_key_env.as_deref()),
            storage_dir: config.wallet_storage_dir.clone(),
            deposit_claim_fee_policy: DepositClaimFeePolicy::Auto,
        },
    )
    .await
    .context("failed to initialize treasury Spark wallet")
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
) -> Result<Vec<PaymentSummary>> {
    let mut payments = Vec::new();
    let mut unresolved_payment_ids = plan.tracked_payment_ids.clone();
    let page_budget = plan.payment_page_budget();
    let page_size = TREASURY_WALLET_REFRESH_PAYMENT_PAGE_SIZE as u32;
    let mut scanned_pages = 0usize;

    for page_index in 0..page_budget {
        let offset = (page_index * TREASURY_WALLET_REFRESH_PAYMENT_PAGE_SIZE) as u32;
        let mut page = wallet
            .list_payments(Some(page_size), Some(offset))
            .await
            .context("failed to list treasury Spark payments")?;
        if page.is_empty() {
            break;
        }

        scanned_pages = scanned_pages.saturating_add(1);
        for payment in &page {
            unresolved_payment_ids.remove(payment.id.as_str());
        }

        let page_len = page.len();
        payments.append(&mut page);

        if page_len < TREASURY_WALLET_REFRESH_PAYMENT_PAGE_SIZE || unresolved_payment_ids.is_empty()
        {
            break;
        }
    }

    if !unresolved_payment_ids.is_empty() {
        tracing::warn!(
            tracked_payment_count = plan.tracked_payment_count(),
            remaining_payment_count = unresolved_payment_ids.len(),
            scanned_pages,
            page_budget,
            "treasury wallet refresh bounded payment scan left unresolved payouts for a later cycle",
        );
    }

    Ok(payments)
}

async fn wallet_snapshot_from_wallet(wallet: &SparkWallet) -> Result<TreasuryWalletSnapshot> {
    wallet_snapshot_from_wallet_with_plan(wallet, &TreasuryWalletRefreshPlan::recent_only()).await
}

async fn wallet_snapshot_from_wallet_with_plan(
    wallet: &SparkWallet,
    plan: &TreasuryWalletRefreshPlan,
) -> Result<TreasuryWalletSnapshot> {
    let balance = wallet
        .get_balance_cached()
        .await
        .context("failed to fetch treasury Spark balance")?;
    let payments = wallet_refresh_payments(wallet, plan).await?;
    Ok(TreasuryWalletSnapshot {
        runtime_status: "connected".to_string(),
        runtime_detail: None,
        balance_sats: balance.total_sats(),
        payments,
    })
}

fn wallet_network_status_label(status: &openagents_spark::NetworkStatusReport) -> &'static str {
    match status.status {
        NetworkStatus::Connected => "connected",
        NetworkStatus::Disconnected => "disconnected",
    }
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

fn read_path_env(name: &str, default: &str) -> PathBuf {
    std::env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(default))
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
    dyn Fn(TreasuryFundingTargetRequest) -> Result<TreasuryFundingMaterial> + Send + Sync,
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
        OnlinePylonIdentity, TREASURY_WALLET_REFRESH_MAX_PAYMENT_PAGES,
        TREASURY_WALLET_REFRESH_PAYMENT_PAGE_SIZE, TreasuryConfig, TreasuryDispatchOutcome,
        TreasuryFundingMaterial, TreasuryFundingTargetRequest, TreasuryPayoutRecord,
        TreasuryPublicStats, TreasuryState, TreasuryWalletInspection,
        TreasuryWalletPaymentAggregate, TreasuryWalletRecoveryComparison,
        TreasuryWalletRecoveryReport, TreasuryWalletSnapshot,
        apply_treasury_wallet_recovery_cutover, build_treasury_wallet_recovery_comparison,
        create_live_funding_target, dispatch_live_payouts, parse_treasury_command,
        payout_phase_offset_ms, payout_window_started_at, payout_window_started_at_for_identity,
        set_test_wallet_funding_hook, set_test_wallet_send_hook, set_test_wallet_snapshot_hook,
        treasury_test_hook_lock, verify_payout_target_registration_signature,
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
            reconciliation_horizon_seconds: 300,
            apply_env_policy: false,
            allow_destructive_env_policy_change: false,
            policy_change_reason: None,
            state_path: PathBuf::from("/tmp/test-nexus-treasury-state.json"),
            wallet_mnemonic_path: PathBuf::from("/tmp/test-nexus-treasury.mnemonic"),
            wallet_storage_dir: PathBuf::from("/tmp/test-nexus-treasury-wallet"),
            wallet_network: "regtest".to_string(),
            wallet_api_key_env: None,
            wallet_status_refresh_seconds: 30,
            max_concurrent_sends: 16,
            registration_challenge_ttl_seconds: 300,
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
            },
            OnlinePylonIdentity {
                nostr_pubkey_hex: "pubkey-b".to_string(),
                sellable: true,
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
            },
        );

        let receipts = state.apply_wallet_snapshot(
            &TreasuryWalletSnapshot {
                runtime_status: "connected".to_string(),
                runtime_detail: None,
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
            },
        );

        let stats: TreasuryPublicStats = state.public_stats(&config, now_unix_ms);
        assert_eq!(stats.payout_sats_paid_total, 122);
        assert_eq!(stats.payout_sats_paid_24h, 2);
        assert_eq!(stats.payouts_dispatched_24h, 1);
        assert_eq!(stats.payouts_confirmed_24h, 0);
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
                balance_sats: 500,
                payments: Vec::new(),
            },
            now_unix_ms,
        );

        let healthy_stats = state.public_stats(&config, now_unix_ms.saturating_add(30_000));
        assert_eq!(healthy_stats.degraded_reason, None);

        let warning_stats = state.public_stats(&config, now_unix_ms.saturating_add(60_001));
        assert_eq!(
            warning_stats.degraded_reason.as_deref(),
            Some("wallet_snapshot_stale:60001")
        );
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

        let eligible_at_unix_ms = 1_800_000;
        state.observe_payout_eligibility(
            &config,
            &[OnlinePylonIdentity {
                nostr_pubkey_hex: "pubkey-a".to_string(),
                sellable: true,
            }],
            eligible_at_unix_ms,
        );

        let alert_at_unix_ms =
            eligible_at_unix_ms + super::TREASURY_CONTINUITY_ALERT_THRESHOLD_MS + 1;
        let raised = state.sync_continuity_alerts(&config, alert_at_unix_ms);
        assert_eq!(raised.len(), 2);
        assert!(
            raised
                .iter()
                .any(|event| event.receipt_type == "treasury.alert.raised")
        );
        assert_eq!(state.active_continuity_alerts.len(), 2);

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
            },
        );

        let eligible_at_unix_ms = 1_800_000;
        state.observe_payout_eligibility(
            &config,
            &[OnlinePylonIdentity {
                nostr_pubkey_hex: "pubkey-a".to_string(),
                sellable: true,
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
            },
        );

        let prepared = state.prepare_due_payouts(
            &config,
            &[OnlinePylonIdentity {
                nostr_pubkey_hex: "pubkey-a".to_string(),
                sellable: true,
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

    #[tokio::test]
    async fn funding_and_dispatch_hooks_cover_happy_path() {
        let _lock = treasury_test_hook_lock().lock().expect("guard");

        set_test_wallet_funding_hook(Some(Arc::new(|request| {
            assert_eq!(request.amount_sats, Some(210));
            Ok(TreasuryFundingMaterial {
                spark_address: "spark:treasury".to_string(),
                bitcoin_address: "bc1qtreasury".to_string(),
                bolt11_invoice: Some("lnbc210fund".to_string()),
                wallet_snapshot: TreasuryWalletSnapshot {
                    runtime_status: "connected".to_string(),
                    runtime_detail: None,
                    balance_sats: 500,
                    payments: Vec::new(),
                },
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
            },
        );

        let plan = state.wallet_refresh_plan();

        assert_eq!(plan.tracked_payment_count(), 1);
        assert!(plan.tracked_payment_ids.contains("pay-confirm-me"));
        assert!(!plan.tracked_payment_ids.contains("pay-already-confirmed"));
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
}
