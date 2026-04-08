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
    Network as SparkNetwork, NetworkStatus, PaymentSummary, SparkSigner, SparkWallet, WalletConfig,
};
use serde::{Deserialize, Serialize};
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
const DEFAULT_TREASURY_WALLET_STATUS_REFRESH_SECONDS: u64 = 30;
const DEFAULT_TREASURY_REGISTRATION_CHALLENGE_TTL_SECONDS: u64 = 300;
const TREASURY_PUBLIC_STATS_WINDOW_MS: u64 = 86_400_000;
const TREASURY_PAYOUT_TARGET_DOMAIN: &str = "openagents:nexus-treasury-payout-target:v1";
const TREASURY_STATE_RETENTION_WINDOW_MS: u64 = 30 * 86_400_000;
const TREASURY_DISPATCH_RESULT_TIMEOUT_MS: u64 = 60_000;
const TREASURY_TARGET_LIMIT: usize = 8_192;
const TREASURY_PAYOUT_LIMIT: usize = 16_384;
const TREASURY_RECEIVE_LIMIT: usize = 16_384;

#[derive(Debug, Clone)]
pub struct TreasuryConfig {
    pub enabled: bool,
    pub payout_sats_per_window: u64,
    pub payout_interval_seconds: u64,
    pub require_sellable: bool,
    pub daily_budget_cap_sats: u64,
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

    pub fn registration_challenge_ttl_ms(&self) -> u64 {
        self.registration_challenge_ttl_seconds
            .saturating_mul(1_000)
    }

    pub fn payout_interval_ms(&self) -> u64 {
        self.payout_interval_seconds.saturating_mul(1_000)
    }

    pub fn dispatch_result_timeout_ms(&self) -> u64 {
        TREASURY_DISPATCH_RESULT_TIMEOUT_MS
            .max(self.wallet_status_refresh_seconds.saturating_mul(2_000))
            .max(self.payout_interval_ms().saturating_mul(2))
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
    pub payout_sats_paid_total: u64,
    pub payout_sats_paid_24h: u64,
    pub payouts_dispatched_24h: u64,
    pub payouts_confirmed_24h: u64,
    pub payouts_failed_24h: u64,
    pub payouts_skipped_24h: u64,
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
        loaded.trim_retention();
        loaded
    }

    pub fn wallet_refresh_due(&self, config: &TreasuryConfig, now_unix_ms: u64) -> bool {
        self.last_wallet_sync_at_unix_ms.is_none_or(|last_sync| {
            now_unix_ms.saturating_sub(last_sync) >= config.wallet_status_refresh_interval_ms()
        })
    }

    pub fn public_stats(&self, config: &TreasuryConfig, now_unix_ms: u64) -> TreasuryPublicStats {
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

        TreasuryPublicStats {
            treasury_enabled: config.enabled,
            payout_sats_per_window: config.payout_sats_per_window,
            payout_interval_seconds: config.payout_interval_seconds,
            require_sellable: config.require_sellable,
            daily_budget_cap_sats: config.daily_budget_cap_sats,
            registered_payout_identities: self.payout_targets_by_identity.len() as u64,
            wallet_balance_sats: self.wallet_balance_sats,
            wallet_balance_updated_at_unix_ms: self.wallet_balance_updated_at_unix_ms,
            wallet_runtime_status: self.wallet_runtime_status.clone(),
            wallet_last_error: self.wallet_last_error.clone(),
            payout_sats_paid_total: self.payout_sats_paid_total,
            payout_sats_paid_24h,
            payouts_dispatched_24h,
            payouts_confirmed_24h,
            payouts_failed_24h,
            payouts_skipped_24h,
        }
    }

    pub fn status_response(
        &self,
        config: &TreasuryConfig,
        now_unix_ms: u64,
    ) -> TreasuryStatusResponse {
        let stats = self.public_stats(config, now_unix_ms);
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
    ) -> (Vec<TreasuryDispatchPlan>, Vec<TreasuryReceiptEvent>) {
        self.trim_retention();
        let mut receipt_events = self.expire_stale_dispatches(config, now_unix_ms);
        if !config.enabled
            || config.payout_sats_per_window == 0
            || config.payout_interval_seconds == 0
            || online_identities.is_empty()
        {
            self.persist();
            return (Vec::new(), receipt_events);
        }

        let payout_interval_ms = config.payout_interval_ms();
        let mut reserved_budget_sats = self.reserved_budget_last_24h(now_unix_ms);
        let mut dispatch_plans = Vec::new();

        for identity in online_identities {
            let window_started_at_unix_ms = payout_window_started_at_for_identity(
                now_unix_ms,
                payout_interval_ms,
                identity.nostr_pubkey_hex.as_str(),
            );
            let window_ends_at_unix_ms =
                window_started_at_unix_ms.saturating_add(payout_interval_ms);
            let payout_key = payout_window_key(
                window_started_at_unix_ms,
                identity.nostr_pubkey_hex.as_str(),
            );
            if self.payout_records_by_key.contains_key(&payout_key) {
                continue;
            }

            let Some(target) = self
                .payout_targets_by_identity
                .get(identity.nostr_pubkey_hex.as_str())
                .cloned()
            else {
                let record = TreasuryPayoutRecord {
                    payout_key: payout_key.clone(),
                    nostr_pubkey_hex: identity.nostr_pubkey_hex.clone(),
                    payout_target: String::new(),
                    amount_sats: config.payout_sats_per_window,
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
                continue;
            };

            if config.require_sellable && !identity.sellable {
                let record = TreasuryPayoutRecord {
                    payout_key: payout_key.clone(),
                    nostr_pubkey_hex: identity.nostr_pubkey_hex.clone(),
                    payout_target: target.spark_address.clone(),
                    amount_sats: config.payout_sats_per_window,
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
                continue;
            }

            if config.daily_budget_cap_sats > 0
                && reserved_budget_sats.saturating_add(config.payout_sats_per_window)
                    > config.daily_budget_cap_sats
            {
                let record = TreasuryPayoutRecord {
                    payout_key: payout_key.clone(),
                    nostr_pubkey_hex: identity.nostr_pubkey_hex.clone(),
                    payout_target: target.spark_address.clone(),
                    amount_sats: config.payout_sats_per_window,
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
                continue;
            }

            reserved_budget_sats =
                reserved_budget_sats.saturating_add(config.payout_sats_per_window);
            self.payout_records_by_key.insert(
                payout_key.clone(),
                TreasuryPayoutRecord {
                    payout_key: payout_key.clone(),
                    nostr_pubkey_hex: identity.nostr_pubkey_hex.clone(),
                    payout_target: target.spark_address.clone(),
                    amount_sats: config.payout_sats_per_window,
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
                amount_sats: config.payout_sats_per_window,
            });
        }

        self.persist();
        (dispatch_plans, receipt_events)
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
        let timeout_ms = config.dispatch_result_timeout_ms();
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

    fn prune_challenges(&mut self, now_unix_ms: u64) {
        self.registration_challenges_by_key.retain(|_, challenge| {
            !challenge.consumed && now_unix_ms <= challenge.expires_at_unix_ms
        });
    }

    fn trim_retention(&mut self) {
        if self.next_challenge_nonce == 0 {
            self.next_challenge_nonce = 1;
        }
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
            state.apply_wallet_snapshot(&snapshot, now_unix_ms);
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
            state_path: PathBuf::from("/tmp/test-nexus-treasury-state.json"),
            wallet_mnemonic_path: PathBuf::from("/tmp/test-nexus-treasury.mnemonic"),
            wallet_storage_dir: PathBuf::from("/tmp/test-nexus-treasury-wallet"),
            wallet_network: "regtest".to_string(),
            wallet_api_key_env: None,
            wallet_status_refresh_seconds: 30,
            registration_challenge_ttl_seconds: 300,
        }
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
        let (plans, skips) = state.prepare_due_payouts(&config, &online, now_unix_ms);
        assert_eq!(plans.len(), 1);
        assert!(skips.is_empty());

        let (plans_again, skips_again) = state.prepare_due_payouts(&config, &online, now_unix_ms);
        assert!(plans_again.is_empty());
        assert!(skips_again.is_empty());
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
        let (plans, skips) = state.prepare_due_payouts(&config, &online, now_unix_ms);

        assert!(skips.is_empty());
        assert_eq!(plans.len(), 2);
        assert_ne!(plans[0].payout_key, plans[1].payout_key);

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
            plans
                .iter()
                .any(|plan| plan.payout_key == format!("{expected_window_a}:pubkey-a"))
        );
        assert!(
            plans
                .iter()
                .any(|plan| plan.payout_key == format!("{expected_window_b}:pubkey-b"))
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
            now_unix_ms.saturating_sub(config.dispatch_result_timeout_ms() + 5_000),
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
                updated_at_unix_ms: now_unix_ms
                    .saturating_sub(config.dispatch_result_timeout_ms() + 1),
                sellable_at_window_open: true,
                dispatch_receipt_recorded: false,
                confirm_receipt_recorded: false,
                fail_receipt_recorded: false,
                skip_receipt_recorded: false,
                counted_in_paid_total: false,
            },
        );

        let (plans, receipts) = state.prepare_due_payouts(
            &config,
            &[OnlinePylonIdentity {
                nostr_pubkey_hex: "pubkey-a".to_string(),
                sellable: true,
            }],
            now_unix_ms,
        );

        assert_eq!(plans.len(), 1);
        assert_eq!(receipts.len(), 1);
        assert_eq!(receipts[0].receipt_type, "treasury.payout.failed");
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
