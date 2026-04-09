use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(test)]
use std::sync::Mutex;

use anyhow::{Context, Result, anyhow, bail};
use bip39::{Language, Mnemonic};
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
            registration_challenge_ttl_seconds,
        })
    }

    pub fn wallet_status_refresh_interval_ms(&self) -> u64 {
        self.wallet_status_refresh_seconds.saturating_mul(1_000)
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
    pub payout_loop_runtime_status: Option<String>,
    pub payout_loop_last_error: Option<String>,
    pub last_payout_reconciliation_at_unix_ms: Option<u64>,
    pub payout_loop_last_started_at_unix_ms: Option<u64>,
    pub payout_loop_last_completed_at_unix_ms: Option<u64>,
    pub public_snapshot_generated_at_unix_ms: Option<u64>,
    pub snapshot_age_ms: Option<u64>,
    pub wallet_sync_lag_ms: Option<u64>,
    pub payout_loop_health: String,
    pub degraded_reason: Option<String>,
    pub payout_sats_paid_total: u64,
    pub payout_sats_paid_24h: u64,
    pub payouts_dispatched_24h: u64,
    pub payouts_confirmed_24h: u64,
    pub payouts_failed_24h: u64,
    pub payouts_skipped_24h: u64,
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
    #[serde(default)]
    pub funding_receives_by_payment_id: BTreeMap<String, TreasuryFundingReceive>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wallet_runtime_status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wallet_last_error: Option<String>,
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

#[derive(Debug, Clone)]
pub struct TreasuryFundingMaterial {
    pub spark_address: String,
    pub bitcoin_address: String,
    pub bolt11_invoice: Option<String>,
    pub wallet_snapshot: TreasuryWalletSnapshot,
}

impl TreasuryState {
    pub fn new(state_path: PathBuf) -> Self {
        let mut loaded = fs::read_to_string(state_path.as_path())
            .ok()
            .and_then(|payload| serde_json::from_str::<Self>(payload.as_str()).ok())
            .unwrap_or_default();
        loaded.state_path = Some(state_path);
        if loaded.next_challenge_nonce == 0 {
            loaded.next_challenge_nonce = 1;
        }
        loaded.trim_policy_change_history();
        loaded.trim_retention();
        loaded
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
        self.payout_loop_runtime_status
            .clone()
            .unwrap_or_else(|| "unknown".to_string())
    }

    fn degraded_reason(&self, config: &TreasuryConfig, now_unix_ms: u64) -> Option<String> {
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
        if matches!(self.wallet_runtime_status.as_deref(), Some("error")) {
            return self
                .wallet_last_error
                .clone()
                .or_else(|| Some("wallet_error".to_string()));
        }
        if self.treasury_enabled(config) {
            let Some(last_wallet_sync_at_unix_ms) = self.last_wallet_sync_at_unix_ms else {
                return Some("wallet_unsynced".to_string());
            };
            if self.wallet_refresh_due(config, now_unix_ms) {
                let lag_ms = now_unix_ms.saturating_sub(last_wallet_sync_at_unix_ms);
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
        let policy = self.active_policy(config);
        let window_started_at_unix_ms = now_unix_ms.saturating_sub(TREASURY_PUBLIC_STATS_WINDOW_MS);
        let mut payout_sats_paid_24h = 0u64;
        let mut payouts_dispatched_24h = 0u64;
        let mut payouts_confirmed_24h = 0u64;
        let mut payouts_failed_24h = 0u64;
        let mut payouts_skipped_24h = 0u64;

        for record in self.payout_records_by_key.values() {
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
            wallet_runtime_status: self.wallet_runtime_status.clone(),
            wallet_last_error: self.wallet_last_error.clone(),
            payout_loop_runtime_status: self.payout_loop_runtime_status.clone(),
            payout_loop_last_error: self.payout_loop_last_error.clone(),
            payout_loop_health: self.payout_loop_health(config),
            last_payout_reconciliation_at_unix_ms: self.last_payout_reconciliation_at_unix_ms,
            payout_loop_last_started_at_unix_ms: self.payout_loop_last_started_at_unix_ms,
            payout_loop_last_completed_at_unix_ms: self.payout_loop_last_completed_at_unix_ms,
            payout_sats_paid_total: self.payout_sats_paid_total,
            payout_sats_paid_24h,
            payouts_dispatched_24h,
            payouts_confirmed_24h,
            payouts_failed_24h,
            payouts_skipped_24h,
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
        let wallet_sync_lag_ms = self
            .last_wallet_sync_at_unix_ms
            .map(|last_sync| now_unix_ms.saturating_sub(last_sync));
        TreasuryPublicStats {
            treasury_enabled: snapshot.treasury_enabled,
            payout_sats_per_window: snapshot.payout_sats_per_window,
            payout_interval_seconds: snapshot.payout_interval_seconds,
            require_sellable: snapshot.require_sellable,
            daily_budget_cap_sats: snapshot.daily_budget_cap_sats,
            registered_payout_identities: snapshot.registered_payout_identities,
            wallet_balance_sats: snapshot.wallet_balance_sats,
            wallet_balance_updated_at_unix_ms: snapshot.wallet_balance_updated_at_unix_ms,
            wallet_runtime_status: snapshot.wallet_runtime_status,
            wallet_last_error: snapshot.wallet_last_error,
            payout_loop_runtime_status: snapshot.payout_loop_runtime_status,
            payout_loop_last_error: snapshot.payout_loop_last_error,
            last_payout_reconciliation_at_unix_ms: snapshot.last_payout_reconciliation_at_unix_ms,
            payout_loop_last_started_at_unix_ms: snapshot.payout_loop_last_started_at_unix_ms,
            payout_loop_last_completed_at_unix_ms: snapshot.payout_loop_last_completed_at_unix_ms,
            public_snapshot_generated_at_unix_ms: Some(snapshot.generated_at_unix_ms),
            snapshot_age_ms: Some(now_unix_ms.saturating_sub(snapshot.generated_at_unix_ms)),
            wallet_sync_lag_ms,
            payout_loop_health: self.payout_loop_health(config),
            degraded_reason: self.degraded_reason(config, now_unix_ms),
            payout_sats_paid_total: snapshot.payout_sats_paid_total,
            payout_sats_paid_24h: snapshot.payout_sats_paid_24h,
            payouts_dispatched_24h: snapshot.payouts_dispatched_24h,
            payouts_confirmed_24h: snapshot.payouts_confirmed_24h,
            payouts_failed_24h: snapshot.payouts_failed_24h,
            payouts_skipped_24h: snapshot.payouts_skipped_24h,
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
            payout_loop_runtime_status: stats.payout_loop_runtime_status,
            payout_loop_last_error: stats.payout_loop_last_error,
            last_payout_reconciliation_at_unix_ms: stats.last_payout_reconciliation_at_unix_ms,
            payout_loop_last_started_at_unix_ms: stats.payout_loop_last_started_at_unix_ms,
            payout_loop_last_completed_at_unix_ms: stats.payout_loop_last_completed_at_unix_ms,
            public_snapshot_generated_at_unix_ms: stats.public_snapshot_generated_at_unix_ms,
            snapshot_age_ms: stats.snapshot_age_ms,
            wallet_sync_lag_ms: stats.wallet_sync_lag_ms,
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
        }
    }

    pub fn record_wallet_error(&mut self, detail: impl Into<String>) {
        self.wallet_runtime_status = Some("error".to_string());
        self.wallet_last_error = Some(detail.into());
        self.persist();
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
            let Some(record) = self
                .payout_records_by_key
                .values_mut()
                .find(|record| record.payment_id.as_deref() == Some(payment.id.as_str()))
            else {
                continue;
            };
            record.updated_at_unix_ms = payment.timestamp.saturating_mul(1_000);
            if wallet_payment_is_confirmed(payment) {
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
                if let Err(error) = fs::write(state_path.as_path(), format!("{payload}\n")) {
                    self.last_persistence_error = Some(format!(
                        "failed to write treasury state {}: {error}",
                        state_path.display()
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
    #[cfg(test)]
    if let Some(hook) = test_wallet_snapshot_hook()
        .lock()
        .expect("treasury snapshot hook")
        .as_ref()
    {
        return hook();
    }

    with_live_wallet(config, create_if_missing, |wallet| async move {
        wallet_snapshot_from_wallet(wallet.as_ref()).await
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

    let mut outcomes = Vec::with_capacity(plans.len());
    for plan in plans {
        match wallet
            .send_payment_simple(plan.payment_request.as_str(), Some(plan.amount_sats))
            .await
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

    let (wallet_snapshot, wallet_error) = match wallet_snapshot_from_wallet(wallet.as_ref()).await {
        Ok(snapshot) => (Some(snapshot), None),
        Err(error) => (None, Some(error.to_string())),
    };

    TreasuryDispatchBatchResult {
        outcomes,
        wallet_snapshot,
        wallet_error,
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
    }
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
    "treasury [status [--json] | funding-target [--amount-sats <n>] [--description <text>] [--expiry-seconds <n>] [--json]]"
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

async fn wallet_snapshot_from_wallet(wallet: &SparkWallet) -> Result<TreasuryWalletSnapshot> {
    let network_status = wallet.network_status().await;
    let balance = wallet
        .get_balance()
        .await
        .context("failed to fetch treasury Spark balance")?;
    let payments = wallet
        .list_all_payments()
        .await
        .context("failed to list treasury Spark payments")?;
    Ok(TreasuryWalletSnapshot {
        runtime_status: wallet_network_status_label(&network_status).to_string(),
        runtime_detail: network_status.detail,
        balance_sats: balance.total_sats(),
        payments,
    })
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

fn wallet_network_status_label(status: &openagents_spark::NetworkStatusReport) -> &'static str {
    match status.status {
        NetworkStatus::Connected => "connected",
        NetworkStatus::Disconnected => "disconnected",
    }
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
        OnlinePylonIdentity, TreasuryConfig, TreasuryDispatchOutcome, TreasuryFundingMaterial,
        TreasuryFundingTargetRequest, TreasuryPublicStats, TreasuryState, TreasuryWalletSnapshot,
        create_live_funding_target, dispatch_live_payouts, payout_phase_offset_ms,
        payout_window_started_at, payout_window_started_at_for_identity,
        set_test_wallet_funding_hook, set_test_wallet_send_hook, set_test_wallet_snapshot_hook,
        treasury_test_hook_lock, verify_payout_target_registration_signature,
    };
    use openagents_provider_substrate::sign_provider_payout_target_registration;
    use openagents_spark::PaymentSummary;
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
}
