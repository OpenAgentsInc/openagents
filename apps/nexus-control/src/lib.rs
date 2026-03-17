#![cfg_attr(
    test,
    allow(
        clippy::all,
        clippy::expect_used,
        clippy::panic,
        clippy::panic_in_result_fn,
        clippy::pedantic,
        clippy::unwrap_used
    )
)]

mod economy;
mod kernel;

use std::collections::{BTreeMap, HashMap};
use std::convert::Infallible;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use std::time::Duration;

use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use openagents_kernel_core::authority::{
    AcceptAccessGrantRequest, AcceptAccessGrantResponse, AdjustReservePartitionRequest,
    AdjustReservePartitionResponse, BindCoverageRequest, BindCoverageResponse,
    CreateAccessGrantRequest, CreateAccessGrantResponse, CreateContractRequest,
    CreateContractResponse, CreateLiquidityQuoteRequest, CreateLiquidityQuoteResponse,
    CreatePredictionPositionRequest, CreatePredictionPositionResponse, CreateRiskClaimRequest,
    CreateRiskClaimResponse, CreateWorkUnitRequest, CreateWorkUnitResponse,
    ExecuteSettlementIntentRequest, ExecuteSettlementIntentResponse, FinalizeVerdictRequest,
    FinalizeVerdictResponse, IssueDeliveryBundleRequest, IssueDeliveryBundleResponse,
    IssueLiquidityEnvelopeRequest, IssueLiquidityEnvelopeResponse, PlaceCoverageOfferRequest,
    PlaceCoverageOfferResponse, PublishRiskSignalRequest, PublishRiskSignalResponse,
    RegisterDataAssetRequest, RegisterDataAssetResponse, RegisterReservePartitionRequest,
    RegisterReservePartitionResponse, ResolveRiskClaimRequest, ResolveRiskClaimResponse,
    RevokeAccessGrantRequest, RevokeAccessGrantResponse, SelectRoutePlanRequest,
    SelectRoutePlanResponse, SubmitOutputRequest, SubmitOutputResponse,
};
use openagents_kernel_core::compute::{
    CapacityInstrumentStatus, CapacityLotStatus, ComputeAcceptedOutcomeKind,
    ComputeAdapterContributionDisposition, ComputeAdapterWindowStatus,
    ComputeEnvironmentPackageStatus, ComputeEvaluationRunStatus, ComputeProductStatus,
    ComputeRegistryStatus, ComputeSyntheticDataJobStatus, ComputeTrainingRunStatus,
    ComputeValidatorChallengeContext, ComputeValidatorChallengeFailureCode,
    ComputeValidatorChallengeLease, ComputeValidatorChallengeProtocolKind,
    ComputeValidatorChallengeRequest, ComputeValidatorChallengeResult,
    ComputeValidatorChallengeSnapshot, ComputeValidatorChallengeStatus,
    ComputeValidatorChallengeVerdict, DeliveryProofStatus, StructuredCapacityInstrumentStatus,
};
use openagents_kernel_core::compute_contracts;
use openagents_kernel_core::data::{
    AccessGrant, AccessGrantStatus, DataAsset, DataAssetStatus, DeliveryBundle,
    DeliveryBundleStatus, RevocationReceipt, RevocationStatus,
};
use openagents_kernel_core::receipts::Receipt;
use openagents_kernel_core::snapshots::EconomySnapshot;
use openagents_kernel_proto::openagents::compute::v1 as proto_compute;
use openagents_validator_service::ValidatorChallengeStatus as ServiceValidatorChallengeStatus;
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;
use tokio_stream::StreamExt;
use tokio_stream::wrappers::BroadcastStream;

use crate::economy::{
    AuthorityReceiptContext, PublicRuntimeSnapshot, PublicStatsSnapshot, ReceiptLedger,
};
use crate::kernel::{
    FinalizeValidatorChallengeRequest, FinalizeValidatorChallengeResponse, KernelMutationContext,
    KernelState, LeaseValidatorChallengeRequest, LeaseValidatorChallengeResponse,
    ReceiptProjectionEvent, ScheduleValidatorChallengeRequest, ScheduleValidatorChallengeResponse,
    SnapshotProjectionEvent,
};

const ENV_LISTEN_ADDR: &str = "NEXUS_CONTROL_LISTEN_ADDR";
const ENV_SESSION_TTL_SECONDS: &str = "NEXUS_CONTROL_SESSION_TTL_SECONDS";
const ENV_SYNC_TOKEN_TTL_SECONDS: &str = "NEXUS_CONTROL_SYNC_TOKEN_TTL_SECONDS";
const ENV_SYNC_TOKEN_REFRESH_AFTER_SECONDS: &str = "NEXUS_CONTROL_SYNC_TOKEN_REFRESH_AFTER_SECONDS";
const ENV_SYNC_STREAM_GRANTS: &str = "NEXUS_CONTROL_SYNC_STREAM_GRANTS";
const ENV_HOSTED_NEXUS_RELAY_URL: &str = "NEXUS_CONTROL_HOSTED_NEXUS_RELAY_URL";
const ENV_RECEIPT_LOG_PATH: &str = "NEXUS_CONTROL_RECEIPT_LOG_PATH";
const ENV_KERNEL_STATE_PATH: &str = "NEXUS_CONTROL_KERNEL_STATE_PATH";
const ENV_COMPUTE_ENABLE_FORWARD_PHYSICAL: &str = "NEXUS_CONTROL_COMPUTE_ENABLE_FORWARD_PHYSICAL";
const ENV_COMPUTE_ENABLE_FUTURE_CASH: &str = "NEXUS_CONTROL_COMPUTE_ENABLE_FUTURE_CASH";
const ENV_COMPUTE_ENABLE_STRUCTURED_PRODUCTS: &str =
    "NEXUS_CONTROL_COMPUTE_ENABLE_STRUCTURED_PRODUCTS";
const ENV_COMPUTE_ENABLE_RECONCILIATION_DIAGNOSTICS: &str =
    "NEXUS_CONTROL_COMPUTE_ENABLE_RECONCILIATION_DIAGNOSTICS";
const ENV_COMPUTE_POLICY_BUNDLE_ID: &str = "NEXUS_CONTROL_COMPUTE_POLICY_BUNDLE_ID";
const ENV_COMPUTE_POLICY_VERSION: &str = "NEXUS_CONTROL_COMPUTE_POLICY_VERSION";
const ENV_STARTER_DEMAND_BUDGET_CAP_SATS: &str = "NEXUS_CONTROL_STARTER_DEMAND_BUDGET_CAP_SATS";
const ENV_STARTER_DEMAND_DISPATCH_INTERVAL_SECONDS: &str =
    "NEXUS_CONTROL_STARTER_DEMAND_DISPATCH_INTERVAL_SECONDS";
const ENV_STARTER_DEMAND_REQUEST_TTL_SECONDS: &str =
    "NEXUS_CONTROL_STARTER_DEMAND_REQUEST_TTL_SECONDS";
const ENV_STARTER_DEMAND_MAX_ACTIVE_OFFERS_PER_SESSION: &str =
    "NEXUS_CONTROL_STARTER_DEMAND_MAX_ACTIVE_OFFERS_PER_SESSION";
const ENV_STARTER_DEMAND_START_CONFIRM_SECONDS: &str =
    "NEXUS_CONTROL_STARTER_DEMAND_START_CONFIRM_SECONDS";
const ENV_STARTER_DEMAND_HEARTBEAT_TIMEOUT_SECONDS: &str =
    "NEXUS_CONTROL_STARTER_DEMAND_HEARTBEAT_TIMEOUT_SECONDS";

const DEFAULT_LISTEN_ADDR: &str = "127.0.0.1:42020";
const DEFAULT_SESSION_TTL_SECONDS: u64 = 86_400;
const DEFAULT_SYNC_TOKEN_TTL_SECONDS: u64 = 900;
const DEFAULT_SYNC_TOKEN_REFRESH_AFTER_SECONDS: u64 = 300;
const DEFAULT_HOSTED_NEXUS_RELAY_URL: &str = "wss://nexus.openagents.com/";
const DEFAULT_KERNEL_STATE_PATH: &str = "var/nexus-control/kernel-state.json";
const DEFAULT_COMPUTE_POLICY_BUNDLE_ID: &str = "policy.compute.market.default";
const DEFAULT_COMPUTE_POLICY_VERSION: &str = "1";
const DEFAULT_STARTER_DEMAND_BUDGET_CAP_SATS: u64 = 5_000;
const DEFAULT_STARTER_DEMAND_DISPATCH_INTERVAL_SECONDS: u64 = 12;
const DEFAULT_STARTER_DEMAND_REQUEST_TTL_SECONDS: u64 = 75;
const DEFAULT_STARTER_DEMAND_MAX_ACTIVE_OFFERS_PER_SESSION: usize = 1;
const DEFAULT_STARTER_DEMAND_START_CONFIRM_SECONDS: u64 = 15;
const DEFAULT_STARTER_DEMAND_HEARTBEAT_TIMEOUT_SECONDS: u64 = 30;
const DEFAULT_SYNC_STREAM_GRANTS: [&str; 2] = [
    "stream.activity_projection.v1",
    "stream.earn_job_lifecycle_projection.v1",
];
const DEFAULT_SYNC_SCOPES: [&str; 3] = ["sync.subscribe", "sync.checkpoint.write", "sync.append"];
const STARTER_DEMAND_REQUESTER: &str = "openagents-hosted-nexus";
const STARTER_DEMAND_REQUEST_KIND: u16 = 5050;
const AUTOPILOT_DESKTOP_CLIENT_ID_PREFIX: &str = "autopilot-desktop";

#[derive(Debug, Clone)]
pub struct ServiceConfig {
    pub listen_addr: SocketAddr,
    pub session_ttl_seconds: u64,
    pub sync_token_ttl_seconds: u64,
    pub sync_token_refresh_after_seconds: u64,
    pub sync_stream_grants: Vec<String>,
    pub hosted_nexus_relay_url: String,
    pub receipt_log_path: Option<PathBuf>,
    pub kernel_state_path: Option<PathBuf>,
    pub compute_enable_forward_physical: bool,
    pub compute_enable_future_cash: bool,
    pub compute_enable_structured_products: bool,
    pub compute_enable_reconciliation_diagnostics: bool,
    pub compute_policy_bundle_id: String,
    pub compute_policy_version: String,
    pub starter_demand_budget_cap_sats: u64,
    pub starter_demand_dispatch_interval_seconds: u64,
    pub starter_demand_request_ttl_seconds: u64,
    pub starter_demand_max_active_offers_per_session: usize,
    pub starter_demand_start_confirm_seconds: u64,
    pub starter_demand_heartbeat_timeout_seconds: u64,
}

impl ServiceConfig {
    pub fn from_env() -> Result<Self, String> {
        let listen_addr = std::env::var(ENV_LISTEN_ADDR)
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| DEFAULT_LISTEN_ADDR.to_string())
            .parse::<SocketAddr>()
            .map_err(|error| format!("invalid {ENV_LISTEN_ADDR}: {error}"))?;
        let session_ttl_seconds =
            parse_u64_env(ENV_SESSION_TTL_SECONDS, DEFAULT_SESSION_TTL_SECONDS)?;
        let sync_token_ttl_seconds =
            parse_u64_env(ENV_SYNC_TOKEN_TTL_SECONDS, DEFAULT_SYNC_TOKEN_TTL_SECONDS)?;
        let sync_token_refresh_after_seconds = parse_u64_env(
            ENV_SYNC_TOKEN_REFRESH_AFTER_SECONDS,
            DEFAULT_SYNC_TOKEN_REFRESH_AFTER_SECONDS,
        )?;
        if sync_token_refresh_after_seconds == 0 {
            return Err(format!(
                "{ENV_SYNC_TOKEN_REFRESH_AFTER_SECONDS} must be greater than zero"
            ));
        }
        if sync_token_refresh_after_seconds >= sync_token_ttl_seconds {
            return Err(format!(
                "{ENV_SYNC_TOKEN_REFRESH_AFTER_SECONDS} must be less than {ENV_SYNC_TOKEN_TTL_SECONDS}"
            ));
        }

        let sync_stream_grants = std::env::var(ENV_SYNC_STREAM_GRANTS)
            .ok()
            .map(|value| {
                value
                    .split(',')
                    .map(str::trim)
                    .filter(|item| !item.is_empty())
                    .map(ToOwned::to_owned)
                    .collect::<Vec<_>>()
            })
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| {
                DEFAULT_SYNC_STREAM_GRANTS
                    .iter()
                    .map(|value| value.to_string())
                    .collect()
            });

        let hosted_nexus_relay_url = std::env::var(ENV_HOSTED_NEXUS_RELAY_URL)
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| DEFAULT_HOSTED_NEXUS_RELAY_URL.to_string());
        let receipt_log_path = std::env::var(ENV_RECEIPT_LOG_PATH)
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .map(PathBuf::from);
        let kernel_state_path = std::env::var(ENV_KERNEL_STATE_PATH)
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
            .or_else(|| Some(PathBuf::from(DEFAULT_KERNEL_STATE_PATH)));
        let compute_enable_forward_physical =
            parse_bool_env(ENV_COMPUTE_ENABLE_FORWARD_PHYSICAL, true)?;
        let compute_enable_future_cash = parse_bool_env(ENV_COMPUTE_ENABLE_FUTURE_CASH, true)?;
        let compute_enable_structured_products =
            parse_bool_env(ENV_COMPUTE_ENABLE_STRUCTURED_PRODUCTS, true)?;
        let compute_enable_reconciliation_diagnostics =
            parse_bool_env(ENV_COMPUTE_ENABLE_RECONCILIATION_DIAGNOSTICS, true)?;
        let compute_policy_bundle_id = std::env::var(ENV_COMPUTE_POLICY_BUNDLE_ID)
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| DEFAULT_COMPUTE_POLICY_BUNDLE_ID.to_string());
        let compute_policy_version = std::env::var(ENV_COMPUTE_POLICY_VERSION)
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| DEFAULT_COMPUTE_POLICY_VERSION.to_string());
        let starter_demand_budget_cap_sats = parse_u64_env(
            ENV_STARTER_DEMAND_BUDGET_CAP_SATS,
            DEFAULT_STARTER_DEMAND_BUDGET_CAP_SATS,
        )?
        .max(1);
        let starter_demand_dispatch_interval_seconds = parse_u64_env(
            ENV_STARTER_DEMAND_DISPATCH_INTERVAL_SECONDS,
            DEFAULT_STARTER_DEMAND_DISPATCH_INTERVAL_SECONDS,
        )?;
        let starter_demand_request_ttl_seconds = parse_u64_env(
            ENV_STARTER_DEMAND_REQUEST_TTL_SECONDS,
            DEFAULT_STARTER_DEMAND_REQUEST_TTL_SECONDS,
        )?;
        let starter_demand_max_active_offers_per_session = parse_usize_env(
            ENV_STARTER_DEMAND_MAX_ACTIVE_OFFERS_PER_SESSION,
            DEFAULT_STARTER_DEMAND_MAX_ACTIVE_OFFERS_PER_SESSION,
        )?;
        let starter_demand_start_confirm_seconds = parse_u64_env(
            ENV_STARTER_DEMAND_START_CONFIRM_SECONDS,
            DEFAULT_STARTER_DEMAND_START_CONFIRM_SECONDS,
        )?;
        let starter_demand_heartbeat_timeout_seconds = parse_u64_env(
            ENV_STARTER_DEMAND_HEARTBEAT_TIMEOUT_SECONDS,
            DEFAULT_STARTER_DEMAND_HEARTBEAT_TIMEOUT_SECONDS,
        )?;
        if starter_demand_dispatch_interval_seconds == 0 {
            return Err(format!(
                "{ENV_STARTER_DEMAND_DISPATCH_INTERVAL_SECONDS} must be greater than zero"
            ));
        }
        if starter_demand_request_ttl_seconds == 0 {
            return Err(format!(
                "{ENV_STARTER_DEMAND_REQUEST_TTL_SECONDS} must be greater than zero"
            ));
        }
        if starter_demand_max_active_offers_per_session == 0 {
            return Err(format!(
                "{ENV_STARTER_DEMAND_MAX_ACTIVE_OFFERS_PER_SESSION} must be greater than zero"
            ));
        }
        if starter_demand_start_confirm_seconds == 0 {
            return Err(format!(
                "{ENV_STARTER_DEMAND_START_CONFIRM_SECONDS} must be greater than zero"
            ));
        }
        if starter_demand_start_confirm_seconds >= starter_demand_request_ttl_seconds {
            return Err(format!(
                "{ENV_STARTER_DEMAND_START_CONFIRM_SECONDS} must be less than {ENV_STARTER_DEMAND_REQUEST_TTL_SECONDS}"
            ));
        }
        if starter_demand_heartbeat_timeout_seconds == 0 {
            return Err(format!(
                "{ENV_STARTER_DEMAND_HEARTBEAT_TIMEOUT_SECONDS} must be greater than zero"
            ));
        }
        if starter_demand_heartbeat_timeout_seconds >= starter_demand_request_ttl_seconds {
            return Err(format!(
                "{ENV_STARTER_DEMAND_HEARTBEAT_TIMEOUT_SECONDS} must be less than {ENV_STARTER_DEMAND_REQUEST_TTL_SECONDS}"
            ));
        }

        Ok(Self {
            listen_addr,
            session_ttl_seconds,
            sync_token_ttl_seconds,
            sync_token_refresh_after_seconds,
            sync_stream_grants,
            hosted_nexus_relay_url,
            receipt_log_path,
            kernel_state_path,
            compute_enable_forward_physical,
            compute_enable_future_cash,
            compute_enable_structured_products,
            compute_enable_reconciliation_diagnostics,
            compute_policy_bundle_id,
            compute_policy_version,
            starter_demand_budget_cap_sats,
            starter_demand_dispatch_interval_seconds,
            starter_demand_request_ttl_seconds,
            starter_demand_max_active_offers_per_session,
            starter_demand_start_confirm_seconds,
            starter_demand_heartbeat_timeout_seconds,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DesktopSessionCreateRequest {
    pub desktop_client_id: String,
    pub device_name: Option<String>,
    pub bound_nostr_pubkey: Option<String>,
    pub client_version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DesktopSessionResponse {
    pub session_id: String,
    pub account_id: String,
    pub access_token: String,
    pub token_type: String,
    pub desktop_client_id: String,
    pub device_name: Option<String>,
    pub bound_nostr_pubkey: Option<String>,
    pub client_version: Option<String>,
    pub issued_at_unix_ms: u64,
    pub expires_at_unix_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SyncTokenClaimsResponse {
    pub session_id: String,
    pub account_id: String,
    pub scope: Vec<String>,
    pub stream_grants: Vec<String>,
    pub issued_at_unix_ms: u64,
    pub not_before_unix_ms: u64,
    pub expires_at_unix_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SyncTokenResponse {
    pub token: String,
    pub transport: String,
    pub protocol_version: String,
    pub refresh_after_in: u64,
    pub rotation_id: String,
    pub claims: SyncTokenClaimsResponse,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionMeResponse {
    pub session_id: String,
    pub account_id: String,
    pub desktop_client_id: String,
    pub device_name: Option<String>,
    pub bound_nostr_pubkey: Option<String>,
    pub client_version: Option<String>,
    pub issued_at_unix_ms: u64,
    pub expires_at_unix_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StarterDemandPollRequest {
    pub provider_nostr_pubkey: Option<String>,
    pub primary_relay_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StarterDemandOffer {
    pub request_id: String,
    pub requester: String,
    pub request_kind: u16,
    pub capability: String,
    pub execution_input: Option<String>,
    pub price_sats: u64,
    pub ttl_seconds: u64,
    pub created_at_unix_ms: u64,
    pub expires_at_unix_ms: u64,
    pub status: String,
    pub start_confirm_by_unix_ms: Option<u64>,
    pub execution_started_at_unix_ms: Option<u64>,
    pub execution_expires_at_unix_ms: Option<u64>,
    pub last_heartbeat_at_unix_ms: Option<u64>,
    pub next_heartbeat_due_at_unix_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StarterDemandPollResponse {
    pub authority: String,
    pub hosted_nexus_relay_url: String,
    pub eligible: bool,
    pub reason: Option<String>,
    pub budget_cap_sats: u64,
    pub budget_allocated_sats: u64,
    pub dispatch_interval_seconds: u64,
    pub request_ttl_seconds: u64,
    pub max_active_offers_per_session: usize,
    pub start_confirm_seconds: u64,
    pub heartbeat_timeout_seconds: u64,
    pub heartbeat_interval_seconds: u64,
    pub offers: Vec<StarterDemandOffer>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StarterDemandAckRequest {
    pub provider_nostr_pubkey: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StarterDemandAckResponse {
    pub request_id: String,
    pub status: String,
    pub started_at_unix_ms: u64,
    pub execution_expires_at_unix_ms: u64,
    pub last_heartbeat_at_unix_ms: u64,
    pub next_heartbeat_due_at_unix_ms: u64,
    pub heartbeat_timeout_seconds: u64,
    pub heartbeat_interval_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StarterDemandHeartbeatRequest {
    pub provider_nostr_pubkey: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StarterDemandHeartbeatResponse {
    pub request_id: String,
    pub status: String,
    pub last_heartbeat_at_unix_ms: u64,
    pub next_heartbeat_due_at_unix_ms: u64,
    pub execution_expires_at_unix_ms: u64,
    pub heartbeat_timeout_seconds: u64,
    pub heartbeat_interval_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StarterDemandFailRequest {
    pub failure_reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StarterDemandFailResponse {
    pub request_id: String,
    pub status: String,
    pub released_at_unix_ms: u64,
    pub failure_reason: String,
    pub budget_cap_sats: u64,
    pub budget_allocated_sats: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StarterDemandCompleteRequest {
    pub payment_pointer: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StarterDemandCompleteResponse {
    pub request_id: String,
    pub status: String,
    pub payment_pointer: String,
    pub completed_at_unix_ms: u64,
    pub budget_cap_sats: u64,
    pub budget_allocated_sats: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HealthResponse {
    pub ok: bool,
    pub service: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct ErrorResponse {
    error: String,
    reason: String,
}

#[derive(Debug, Clone)]
struct AppState {
    config: ServiceConfig,
    store: Arc<RwLock<ControlStore>>,
    kernel_receipt_tx: broadcast::Sender<ReceiptProjectionEvent>,
    kernel_snapshot_tx: broadcast::Sender<SnapshotProjectionEvent>,
}

#[derive(Debug)]
struct ControlStore {
    sessions_by_access_token: HashMap<String, DesktopSessionRecord>,
    sync_tokens: HashMap<String, SyncTokenRecord>,
    starter_demand: StarterDemandState,
    economy: ReceiptLedger,
    kernel: KernelState,
}

impl ControlStore {
    fn new(config: &ServiceConfig) -> Self {
        let mut kernel = KernelState::new_with_persistence(config.kernel_state_path.clone());
        kernel.set_compute_runtime_policy(crate::kernel::ComputeRuntimePolicy {
            enable_forward_physical: config.compute_enable_forward_physical,
            enable_future_cash: config.compute_enable_future_cash,
            enable_structured_products: config.compute_enable_structured_products,
            enable_reconciliation_diagnostics: config.compute_enable_reconciliation_diagnostics,
            policy_bundle_id: config.compute_policy_bundle_id.clone(),
            policy_version: config.compute_policy_version.clone(),
        });
        Self {
            sessions_by_access_token: HashMap::new(),
            sync_tokens: HashMap::new(),
            starter_demand: StarterDemandState::default(),
            economy: ReceiptLedger::new(config.receipt_log_path.clone()),
            kernel,
        }
    }
}

#[derive(Debug, Default)]
struct StarterDemandState {
    budget_allocated_sats: u64,
    next_offer_seq: u64,
    next_template_index: usize,
    last_dispatch_by_session: HashMap<String, u64>,
    offers_by_session: HashMap<String, Vec<StarterDemandOfferRecord>>,
}

#[derive(Debug, Clone)]
struct DesktopSessionRecord {
    session_id: String,
    account_id: String,
    desktop_client_id: String,
    device_name: Option<String>,
    bound_nostr_pubkey: Option<String>,
    client_version: Option<String>,
    issued_at_unix_ms: u64,
    expires_at_unix_ms: u64,
}

#[derive(Debug, Clone)]
struct SyncTokenRecord {
    token: String,
    rotation_id: String,
    session_id: String,
    account_id: String,
    scopes: Vec<String>,
    stream_grants: Vec<String>,
    issued_at_unix_ms: u64,
    not_before_unix_ms: u64,
    expires_at_unix_ms: u64,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum StarterOfferStatus {
    Offered,
    Running,
    Completed,
    Released,
    Expired,
}

impl StarterOfferStatus {
    const fn label(self) -> &'static str {
        match self {
            Self::Offered => "awaiting_ack",
            Self::Running => "running",
            Self::Completed => "completed",
            Self::Released => "released",
            Self::Expired => "expired",
        }
    }
}

#[derive(Debug, Clone)]
struct StarterDemandOfferRecord {
    request_id: String,
    requester: String,
    request_kind: u16,
    capability: String,
    execution_input: Option<String>,
    price_sats: u64,
    ttl_seconds: u64,
    created_at_unix_ms: u64,
    expires_at_unix_ms: u64,
    start_confirm_by_unix_ms: Option<u64>,
    execution_started_at_unix_ms: Option<u64>,
    execution_expires_at_unix_ms: Option<u64>,
    last_heartbeat_at_unix_ms: Option<u64>,
    next_heartbeat_due_at_unix_ms: Option<u64>,
    status: StarterOfferStatus,
    payment_pointer: Option<String>,
    completed_at_unix_ms: Option<u64>,
    released_at_unix_ms: Option<u64>,
    failure_reason: Option<String>,
}

#[derive(Debug, Clone)]
struct StarterDemandOfferEvent {
    session_id: String,
    offer: StarterDemandOfferRecord,
}

#[derive(Debug, Clone, Copy)]
struct StarterDemandTemplate {
    capability: &'static str,
    summary: &'static str,
    payout_sats: u64,
}

const STARTER_DEMAND_TEMPLATES: [StarterDemandTemplate; 4] = [
    StarterDemandTemplate {
        capability: "starter.quest.text_generation",
        summary: "Summarize a short project update into three bullets.",
        payout_sats: 120,
    },
    StarterDemandTemplate {
        capability: "starter.quest.text_generation",
        summary: "Extract action items from a meeting note.",
        payout_sats: 150,
    },
    StarterDemandTemplate {
        capability: "starter.quest.text_generation",
        summary: "Translate a paragraph to plain English.",
        payout_sats: 90,
    },
    StarterDemandTemplate {
        capability: "starter.quest.text_generation",
        summary: "Classify a support ticket and suggest a response.",
        payout_sats: 110,
    },
];

#[derive(Debug, Clone)]
struct ApiError {
    status: StatusCode,
    error: &'static str,
    reason: String,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(ErrorResponse {
                error: self.error.to_string(),
                reason: self.reason,
            }),
        )
            .into_response()
    }
}

pub fn build_router(config: ServiceConfig) -> Router {
    Router::new()
        .route("/healthz", get(healthz))
        .merge(build_api_router(config))
}

pub fn build_api_router(config: ServiceConfig) -> Router {
    let (kernel_receipt_tx, _) = broadcast::channel(256);
    let (kernel_snapshot_tx, _) = broadcast::channel(256);
    let state = AppState {
        store: Arc::new(RwLock::new(ControlStore::new(&config))),
        config,
        kernel_receipt_tx,
        kernel_snapshot_tx,
    };
    Router::new()
        .route("/stats", get(public_stats))
        .route("/api/stats", get(public_stats))
        .route("/api/session/desktop", post(create_desktop_session))
        .route("/api/session/me", get(session_me))
        .route("/api/sync/token", post(create_sync_token))
        .route("/api/starter-demand/poll", post(poll_starter_demand))
        .route(
            "/api/starter-demand/offers/{request_id}/ack",
            post(ack_starter_demand_offer),
        )
        .route(
            "/api/starter-demand/offers/{request_id}/heartbeat",
            post(heartbeat_starter_demand_offer),
        )
        .route(
            "/api/starter-demand/offers/{request_id}/fail",
            post(fail_starter_demand_offer),
        )
        .route(
            "/api/starter-demand/offers/{request_id}/complete",
            post(complete_starter_demand_offer),
        )
        .route("/v1/kernel/work_units", post(create_kernel_work_unit))
        .route("/v1/kernel/contracts", post(create_kernel_contract))
        .route(
            "/v1/kernel/contracts/{contract_id}/submit",
            post(submit_kernel_output),
        )
        .route(
            "/v1/kernel/contracts/{contract_id}/verdict/finalize",
            post(finalize_kernel_verdict),
        )
        .route(
            "/v1/kernel/compute/products",
            get(list_kernel_compute_products).post(create_kernel_compute_product),
        )
        .route(
            "/v1/kernel/compute/products/{product_id}",
            get(get_kernel_compute_product),
        )
        .route(
            "/v1/kernel/compute/environments",
            get(list_kernel_compute_environment_packages)
                .post(register_kernel_compute_environment_package),
        )
        .route(
            "/v1/kernel/compute/environments/{environment_ref}",
            get(get_kernel_compute_environment_package),
        )
        .route(
            "/v1/kernel/compute/checkpoints/policies",
            get(list_kernel_compute_checkpoint_family_policies)
                .post(register_kernel_compute_checkpoint_family_policy),
        )
        .route(
            "/v1/kernel/compute/checkpoints/policies/{checkpoint_family}",
            get(get_kernel_compute_checkpoint_family_policy),
        )
        .route(
            "/v1/kernel/compute/validators/policies",
            get(list_kernel_compute_validator_policies)
                .post(register_kernel_compute_validator_policy),
        )
        .route(
            "/v1/kernel/compute/validators/policies/{policy_ref}",
            get(get_kernel_compute_validator_policy),
        )
        .route(
            "/v1/kernel/compute/benchmarks/packages",
            get(list_kernel_compute_benchmark_packages)
                .post(register_kernel_compute_benchmark_package),
        )
        .route(
            "/v1/kernel/compute/benchmarks/packages/{benchmark_package_ref}",
            get(get_kernel_compute_benchmark_package),
        )
        .route(
            "/v1/kernel/compute/training/policies",
            get(list_kernel_compute_training_policies)
                .post(register_kernel_compute_training_policy),
        )
        .route(
            "/v1/kernel/compute/training/policies/{training_policy_ref}",
            get(get_kernel_compute_training_policy),
        )
        .route(
            "/v1/kernel/compute/evals",
            get(list_kernel_compute_evaluation_runs).post(create_kernel_compute_evaluation_run),
        )
        .route(
            "/v1/kernel/compute/evals/{eval_run_id}",
            get(get_kernel_compute_evaluation_run),
        )
        .route(
            "/v1/kernel/compute/evals/{eval_run_id}/samples",
            get(list_kernel_compute_evaluation_samples)
                .post(append_kernel_compute_evaluation_samples),
        )
        .route(
            "/v1/kernel/compute/evals/{eval_run_id}/finalize",
            post(finalize_kernel_compute_evaluation_run),
        )
        .route(
            "/v1/kernel/compute/training/runs",
            get(list_kernel_compute_training_runs).post(create_kernel_compute_training_run),
        )
        .route(
            "/v1/kernel/compute/training/runs/{training_run_id}",
            get(get_kernel_compute_training_run),
        )
        .route(
            "/v1/kernel/compute/training/runs/{training_run_id}/finalize",
            post(finalize_kernel_compute_training_run),
        )
        .route(
            "/v1/kernel/compute/training/adapter-windows",
            get(list_kernel_compute_adapter_training_windows)
                .post(record_kernel_compute_adapter_window),
        )
        .route(
            "/v1/kernel/compute/training/adapter-windows/{window_id}",
            get(get_kernel_compute_adapter_training_window),
        )
        .route(
            "/v1/kernel/compute/training/adapter-contributions",
            get(list_kernel_compute_adapter_contribution_outcomes),
        )
        .route(
            "/v1/kernel/compute/training/adapter-contributions/{contribution_id}",
            get(get_kernel_compute_adapter_contribution_outcome),
        )
        .route(
            "/v1/kernel/compute/outcomes",
            get(list_kernel_compute_accepted_outcomes).post(accept_kernel_compute_outcome),
        )
        .route(
            "/v1/kernel/compute/outcomes/{outcome_id}",
            get(get_kernel_compute_accepted_outcome),
        )
        .route(
            "/v1/kernel/compute/synthetic",
            get(list_kernel_compute_synthetic_data_jobs)
                .post(create_kernel_compute_synthetic_data_job),
        )
        .route(
            "/v1/kernel/compute/synthetic/{synthetic_job_id}",
            get(get_kernel_compute_synthetic_data_job),
        )
        .route(
            "/v1/kernel/compute/synthetic/{synthetic_job_id}/samples",
            get(list_kernel_compute_synthetic_data_samples)
                .post(append_kernel_compute_synthetic_data_samples),
        )
        .route(
            "/v1/kernel/compute/synthetic/{synthetic_job_id}/finalize_generation",
            post(finalize_kernel_compute_synthetic_data_generation),
        )
        .route(
            "/v1/kernel/compute/synthetic/{synthetic_job_id}/record_verification",
            post(record_kernel_compute_synthetic_data_verification),
        )
        .route(
            "/v1/kernel/compute/lots",
            get(list_kernel_capacity_lots).post(create_kernel_capacity_lot),
        )
        .route(
            "/v1/kernel/compute/lots/{lot_id}",
            get(get_kernel_capacity_lot),
        )
        .route(
            "/v1/kernel/compute/instruments",
            get(list_kernel_capacity_instruments).post(create_kernel_capacity_instrument),
        )
        .route(
            "/v1/kernel/compute/instruments/{instrument_id}",
            get(get_kernel_capacity_instrument),
        )
        .route(
            "/v1/kernel/compute/instruments/{instrument_id}/close",
            post(close_kernel_capacity_instrument),
        )
        .route(
            "/v1/kernel/compute/instruments/{instrument_id}/cash_settle",
            post(cash_settle_kernel_capacity_instrument),
        )
        .route(
            "/v1/kernel/compute/structured_instruments",
            get(list_kernel_structured_capacity_instruments)
                .post(create_kernel_structured_capacity_instrument),
        )
        .route(
            "/v1/kernel/compute/structured_instruments/{structured_instrument_id}",
            get(get_kernel_structured_capacity_instrument),
        )
        .route(
            "/v1/kernel/compute/structured_instruments/{structured_instrument_id}/close",
            post(close_kernel_structured_capacity_instrument),
        )
        .route(
            "/v1/kernel/compute/lots/{lot_id}/delivery_proofs",
            get(list_kernel_delivery_proofs).post(record_kernel_delivery_proof),
        )
        .route(
            "/v1/kernel/compute/delivery_proofs/{delivery_proof_id}",
            get(get_kernel_delivery_proof),
        )
        .route(
            "/v1/kernel/compute/validator_challenges",
            get(list_kernel_validator_challenges).post(schedule_kernel_validator_challenge),
        )
        .route(
            "/v1/kernel/compute/validator_challenges/{challenge_id}",
            get(get_kernel_validator_challenge),
        )
        .route(
            "/v1/kernel/compute/validator_challenges/{challenge_id}/lease",
            post(lease_kernel_validator_challenge),
        )
        .route(
            "/v1/kernel/compute/validator_challenges/{challenge_id}/finalize",
            post(finalize_kernel_validator_challenge),
        )
        .route(
            "/v1/kernel/compute/indices",
            get(list_kernel_compute_indices).post(publish_kernel_compute_index),
        )
        .route(
            "/v1/kernel/compute/indices/{index_id}",
            get(get_kernel_compute_index),
        )
        .route(
            "/v1/kernel/compute/indices/{index_id}/correct",
            post(correct_kernel_compute_index),
        )
        .route(
            "/v1/kernel/data/assets",
            get(list_kernel_data_assets).post(register_kernel_data_asset),
        )
        .route(
            "/v1/kernel/data/assets/{asset_id}",
            get(get_kernel_data_asset),
        )
        .route(
            "/v1/kernel/data/grants",
            get(list_kernel_access_grants).post(create_kernel_access_grant),
        )
        .route(
            "/v1/kernel/data/grants/{grant_id}",
            get(get_kernel_access_grant),
        )
        .route(
            "/v1/kernel/data/grants/{grant_id}/accept",
            post(accept_kernel_access_grant),
        )
        .route(
            "/v1/kernel/data/grants/{grant_id}/deliveries",
            post(issue_kernel_delivery_bundle),
        )
        .route(
            "/v1/kernel/data/grants/{grant_id}/revoke",
            post(revoke_kernel_access_grant),
        )
        .route(
            "/v1/kernel/data/deliveries",
            get(list_kernel_delivery_bundles),
        )
        .route(
            "/v1/kernel/data/deliveries/{delivery_bundle_id}",
            get(get_kernel_delivery_bundle),
        )
        .route("/v1/kernel/data/revocations", get(list_kernel_revocations))
        .route(
            "/v1/kernel/data/revocations/{revocation_id}",
            get(get_kernel_revocation),
        )
        .route(
            "/v1/kernel/liquidity/quotes",
            post(create_kernel_liquidity_quote),
        )
        .route(
            "/v1/kernel/liquidity/routes",
            post(select_kernel_route_plan),
        )
        .route(
            "/v1/kernel/liquidity/envelopes",
            post(issue_kernel_liquidity_envelope),
        )
        .route(
            "/v1/kernel/liquidity/settlements",
            post(execute_kernel_settlement_intent),
        )
        .route(
            "/v1/kernel/liquidity/reserve_partitions",
            post(register_kernel_reserve_partition),
        )
        .route(
            "/v1/kernel/liquidity/reserve_partitions/{partition_id}/adjust",
            post(adjust_kernel_reserve_partition),
        )
        .route(
            "/v1/kernel/risk/coverage_offers",
            post(place_kernel_coverage_offer),
        )
        .route(
            "/v1/kernel/risk/coverage_bindings",
            post(bind_kernel_coverage),
        )
        .route(
            "/v1/kernel/risk/positions",
            post(create_kernel_prediction_position),
        )
        .route("/v1/kernel/risk/claims", post(create_kernel_risk_claim))
        .route(
            "/v1/kernel/risk/claims/{claim_id}/resolve",
            post(resolve_kernel_risk_claim),
        )
        .route("/v1/kernel/risk/signals", post(publish_kernel_risk_signal))
        .route(
            "/v1/kernel/snapshots/{minute_start_ms}",
            get(get_kernel_snapshot),
        )
        .route("/v1/kernel/receipts/{receipt_id}", get(get_kernel_receipt))
        .route("/v1/kernel/stream/receipts", get(stream_kernel_receipts))
        .route("/v1/kernel/stream/snapshots", get(stream_kernel_snapshots))
        .with_state(state)
}

pub async fn run_server(config: ServiceConfig) -> Result<(), anyhow::Error> {
    let listener = tokio::net::TcpListener::bind(config.listen_addr).await?;
    let local_addr = listener.local_addr()?;
    tracing::info!("nexus-control listening on {}", local_addr);
    axum::serve(listener, build_router(config)).await?;
    Ok(())
}

async fn healthz() -> Json<HealthResponse> {
    Json(HealthResponse {
        ok: true,
        service: "nexus-control".to_string(),
    })
}

async fn public_stats(
    State(state): State<AppState>,
) -> Result<Json<PublicStatsSnapshot>, ApiError> {
    let now = now_unix_ms();
    let mut store = state.store.write().map_err(|_| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        error: "internal_error",
        reason: "session_store_poisoned".to_string(),
    })?;
    let expired_events =
        prune_expired_starter_offers(&mut store.starter_demand, &state.config, now);
    record_expired_offer_receipts(&mut store, expired_events, now);
    let stats = store
        .economy
        .snapshot(&runtime_snapshot(&state.config, &store, now), now);
    Ok(Json(stats))
}

async fn create_desktop_session(
    State(state): State<AppState>,
    Json(request): Json<DesktopSessionCreateRequest>,
) -> Result<Json<DesktopSessionResponse>, ApiError> {
    let desktop_client_id = normalize_required_field(
        request.desktop_client_id.as_str(),
        "desktop_client_id_missing",
    )?;
    let now = now_unix_ms();
    let access_token = random_token();
    let session_id = format!("sess_{}", random_token());
    let account_id = format!(
        "desktop:{}",
        sanitize_identifier(desktop_client_id.as_str())
    );
    let issued_at_unix_ms = now;
    let expires_at_unix_ms =
        now.saturating_add(state.config.session_ttl_seconds.saturating_mul(1_000));
    let record = DesktopSessionRecord {
        session_id: session_id.clone(),
        account_id: account_id.clone(),
        desktop_client_id: desktop_client_id.clone(),
        device_name: normalize_optional_field(request.device_name.as_deref()),
        bound_nostr_pubkey: normalize_optional_field(request.bound_nostr_pubkey.as_deref()),
        client_version: normalize_optional_field(request.client_version.as_deref()),
        issued_at_unix_ms,
        expires_at_unix_ms,
    };

    let mut store = state.store.write().map_err(|_| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        error: "internal_error",
        reason: "session_store_poisoned".to_string(),
    })?;
    store
        .sessions_by_access_token
        .insert(access_token.clone(), record.clone());
    store.economy.record(
        "desktop_session.created",
        now,
        desktop_session_receipt_context(&record),
    );
    drop(store);

    Ok(Json(DesktopSessionResponse {
        session_id,
        account_id,
        access_token,
        token_type: "Bearer".to_string(),
        desktop_client_id,
        device_name: record.device_name,
        bound_nostr_pubkey: record.bound_nostr_pubkey,
        client_version: record.client_version,
        issued_at_unix_ms,
        expires_at_unix_ms,
    }))
}

async fn session_me(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<SessionMeResponse>, ApiError> {
    let session = authenticate_session(&state, &headers)?;
    Ok(Json(SessionMeResponse {
        session_id: session.session_id,
        account_id: session.account_id,
        desktop_client_id: session.desktop_client_id,
        device_name: session.device_name,
        bound_nostr_pubkey: session.bound_nostr_pubkey,
        client_version: session.client_version,
        issued_at_unix_ms: session.issued_at_unix_ms,
        expires_at_unix_ms: session.expires_at_unix_ms,
    }))
}

async fn create_sync_token(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<SyncTokenResponse>, ApiError> {
    let session = authenticate_session(&state, &headers)?;
    let now = now_unix_ms();
    let token = format!("sync_{}", random_token());
    let rotation_id = format!("rot_{}", random_token());
    let scopes = DEFAULT_SYNC_SCOPES
        .iter()
        .map(|value| value.to_string())
        .collect::<Vec<_>>();
    let stream_grants = state.config.sync_stream_grants.clone();
    let expires_at_unix_ms =
        now.saturating_add(state.config.sync_token_ttl_seconds.saturating_mul(1_000));
    let session_id = session.session_id.clone();
    let account_id = session.account_id.clone();
    let record = SyncTokenRecord {
        token: token.clone(),
        rotation_id: rotation_id.clone(),
        session_id,
        account_id,
        scopes,
        stream_grants,
        issued_at_unix_ms: now,
        not_before_unix_ms: now,
        expires_at_unix_ms,
    };
    let mut store = state.store.write().map_err(|_| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        error: "internal_error",
        reason: "session_store_poisoned".to_string(),
    })?;
    store.sync_tokens.insert(token.clone(), record.clone());
    store.economy.record(
        "sync_token.issued",
        now,
        sync_token_receipt_context(&session, &record),
    );
    drop(store);

    Ok(Json(SyncTokenResponse {
        token: record.token,
        transport: "spacetime_ws".to_string(),
        protocol_version: "spacetime.sync.v1".to_string(),
        refresh_after_in: state.config.sync_token_refresh_after_seconds,
        rotation_id: record.rotation_id,
        claims: SyncTokenClaimsResponse {
            session_id: record.session_id,
            account_id: record.account_id,
            scope: record.scopes,
            stream_grants: record.stream_grants,
            issued_at_unix_ms: record.issued_at_unix_ms,
            not_before_unix_ms: record.not_before_unix_ms,
            expires_at_unix_ms: record.expires_at_unix_ms,
        },
    }))
}

async fn poll_starter_demand(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<StarterDemandPollRequest>,
) -> Result<Json<StarterDemandPollResponse>, ApiError> {
    let session = authenticate_session(&state, &headers)?;
    let now = now_unix_ms();
    let relay_url = normalize_optional_field(request.primary_relay_url.as_deref());
    let provider_nostr_pubkey = normalize_optional_field(request.provider_nostr_pubkey.as_deref());
    let mut response = StarterDemandPollResponse {
        authority: "openagents-hosted-nexus".to_string(),
        hosted_nexus_relay_url: state.config.hosted_nexus_relay_url.clone(),
        eligible: false,
        reason: None,
        budget_cap_sats: state.config.starter_demand_budget_cap_sats,
        budget_allocated_sats: 0,
        dispatch_interval_seconds: state.config.starter_demand_dispatch_interval_seconds,
        request_ttl_seconds: state.config.starter_demand_request_ttl_seconds,
        max_active_offers_per_session: state.config.starter_demand_max_active_offers_per_session,
        start_confirm_seconds: state.config.starter_demand_start_confirm_seconds,
        heartbeat_timeout_seconds: state.config.starter_demand_heartbeat_timeout_seconds,
        heartbeat_interval_seconds: starter_demand_heartbeat_interval_seconds(&state.config),
        offers: Vec::new(),
    };

    if relay_url.as_deref() != Some(state.config.hosted_nexus_relay_url.as_str()) {
        let mut store = state.store.write().map_err(|_| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            error: "internal_error",
            reason: "session_store_poisoned".to_string(),
        })?;
        response.budget_allocated_sats = store.starter_demand.budget_allocated_sats;
        let reason = "starter_demand_requires_openagents_hosted_nexus".to_string();
        response.reason = Some(reason.clone());
        store.economy.record(
            "starter_demand.ineligible",
            now,
            starter_offer_receipt_context(
                Some(&session),
                None,
                Some(reason.as_str()),
                relay_url.as_deref(),
                None,
            ),
        );
        return Ok(Json(response));
    }
    if let Some(reason) =
        starter_demand_provider_proof_reason(&session, provider_nostr_pubkey.as_deref())
    {
        let mut store = state.store.write().map_err(|_| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            error: "internal_error",
            reason: "session_store_poisoned".to_string(),
        })?;
        response.budget_allocated_sats = store.starter_demand.budget_allocated_sats;
        response.reason = Some(reason.clone());
        store.economy.record(
            "starter_demand.ineligible",
            now,
            starter_offer_receipt_context(
                Some(&session),
                None,
                Some(reason.as_str()),
                relay_url.as_deref(),
                None,
            ),
        );
        return Ok(Json(response));
    }

    let mut store = state.store.write().map_err(|_| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        error: "internal_error",
        reason: "session_store_poisoned".to_string(),
    })?;
    let expired_events =
        prune_expired_starter_offers(&mut store.starter_demand, &state.config, now);
    record_expired_offer_receipts(&mut store, expired_events, now);
    let dispatched_offer =
        maybe_dispatch_starter_offer(&mut store.starter_demand, &state.config, &session, now);
    if let Some(offer) = dispatched_offer {
        store.economy.record(
            "starter_offer.dispatched",
            now,
            starter_offer_receipt_context(
                Some(&session),
                Some(&offer),
                None,
                relay_url.as_deref(),
                None,
            ),
        );
    }
    response.eligible = true;
    response.budget_allocated_sats = store.starter_demand.budget_allocated_sats;
    response.offers = store
        .starter_demand
        .offers_by_session
        .get(session.session_id.as_str())
        .into_iter()
        .flat_map(|offers| offers.iter())
        .filter(|offer| {
            matches!(
                offer.status,
                StarterOfferStatus::Offered | StarterOfferStatus::Running
            )
        })
        .map(starter_offer_response)
        .collect();
    Ok(Json(response))
}

async fn ack_starter_demand_offer(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(request_id): Path<String>,
    Json(request): Json<StarterDemandAckRequest>,
) -> Result<Json<StarterDemandAckResponse>, ApiError> {
    let session = authenticate_session(&state, &headers)?;
    let request_id = normalize_required_field(request_id.as_str(), "request_id_missing")?;
    if let Some(reason) = starter_demand_provider_proof_reason(
        &session,
        normalize_optional_field(request.provider_nostr_pubkey.as_deref()).as_deref(),
    ) {
        return Err(ApiError {
            status: StatusCode::FORBIDDEN,
            error: "forbidden",
            reason,
        });
    }
    let now = now_unix_ms();

    let mut store = state.store.write().map_err(|_| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        error: "internal_error",
        reason: "session_store_poisoned".to_string(),
    })?;
    let expired_events =
        prune_expired_starter_offers(&mut store.starter_demand, &state.config, now);
    record_expired_offer_receipts(&mut store, expired_events, now);
    let mut started_receipt_offer = None;
    let mut expired_offer = None;
    let mut released_budget_sats = 0u64;
    let response = {
        let offers = store
            .starter_demand
            .offers_by_session
            .get_mut(session.session_id.as_str())
            .ok_or_else(|| ApiError {
                status: StatusCode::NOT_FOUND,
                error: "not_found",
                reason: "starter_offer_not_found".to_string(),
            })?;
        let offer = offers
            .iter_mut()
            .find(|offer| offer.request_id == request_id)
            .ok_or_else(|| ApiError {
                status: StatusCode::NOT_FOUND,
                error: "not_found",
                reason: "starter_offer_not_found".to_string(),
            })?;

        if offer.status == StarterOfferStatus::Offered {
            let start_confirm_by_unix_ms = offer
                .start_confirm_by_unix_ms
                .unwrap_or(offer.created_at_unix_ms);
            if now > start_confirm_by_unix_ms {
                released_budget_sats =
                    expire_offer(offer, now, "starter_offer_start_confirm_missed");
                expired_offer = Some(offer.clone());
                None
            } else {
                let heartbeat_timeout_ms = state
                    .config
                    .starter_demand_heartbeat_timeout_seconds
                    .saturating_mul(1_000);
                let execution_expires_at_unix_ms = now.saturating_add(
                    state
                        .config
                        .starter_demand_request_ttl_seconds
                        .saturating_mul(1_000),
                );
                offer.status = StarterOfferStatus::Running;
                offer.execution_started_at_unix_ms = Some(now);
                offer.execution_expires_at_unix_ms = Some(execution_expires_at_unix_ms);
                offer.last_heartbeat_at_unix_ms = Some(now);
                offer.next_heartbeat_due_at_unix_ms =
                    Some(now.saturating_add(heartbeat_timeout_ms));
                offer.expires_at_unix_ms = execution_expires_at_unix_ms;
                started_receipt_offer = Some(offer.clone());
                Some(StarterDemandAckResponse {
                    request_id: offer.request_id.clone(),
                    status: offer.status.label().to_string(),
                    started_at_unix_ms: offer.execution_started_at_unix_ms.unwrap_or(now),
                    execution_expires_at_unix_ms: offer.execution_expires_at_unix_ms.unwrap_or(now),
                    last_heartbeat_at_unix_ms: offer.last_heartbeat_at_unix_ms.unwrap_or(now),
                    next_heartbeat_due_at_unix_ms: offer
                        .next_heartbeat_due_at_unix_ms
                        .unwrap_or(now),
                    heartbeat_timeout_seconds: state
                        .config
                        .starter_demand_heartbeat_timeout_seconds,
                    heartbeat_interval_seconds: starter_demand_heartbeat_interval_seconds(
                        &state.config,
                    ),
                })
            }
        } else if offer.status != StarterOfferStatus::Running {
            return Err(ApiError {
                status: StatusCode::CONFLICT,
                error: "conflict",
                reason: format!("starter_offer_not_ackable status={}", offer.status.label()),
            });
        } else {
            Some(StarterDemandAckResponse {
                request_id: offer.request_id.clone(),
                status: offer.status.label().to_string(),
                started_at_unix_ms: offer.execution_started_at_unix_ms.unwrap_or(now),
                execution_expires_at_unix_ms: offer.execution_expires_at_unix_ms.unwrap_or(now),
                last_heartbeat_at_unix_ms: offer.last_heartbeat_at_unix_ms.unwrap_or(now),
                next_heartbeat_due_at_unix_ms: offer.next_heartbeat_due_at_unix_ms.unwrap_or(now),
                heartbeat_timeout_seconds: state.config.starter_demand_heartbeat_timeout_seconds,
                heartbeat_interval_seconds: starter_demand_heartbeat_interval_seconds(
                    &state.config,
                ),
            })
        }
    };

    if let Some(expired_offer) = expired_offer {
        store.starter_demand.budget_allocated_sats = store
            .starter_demand
            .budget_allocated_sats
            .saturating_sub(released_budget_sats);
        store.economy.record(
            "starter_offer.expired",
            now,
            starter_offer_receipt_context(
                Some(&session),
                Some(&expired_offer),
                expired_offer.failure_reason.as_deref(),
                None,
                None,
            ),
        );
        return Err(ApiError {
            status: StatusCode::CONFLICT,
            error: "conflict",
            reason: "starter_offer_start_confirm_missed".to_string(),
        });
    }

    if let Some(started_offer) = started_receipt_offer {
        store.economy.record(
            "starter_offer.started",
            now,
            starter_offer_receipt_context(Some(&session), Some(&started_offer), None, None, None),
        );
    }

    Ok(Json(response.ok_or_else(|| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        error: "internal_error",
        reason: "starter_offer_ack_response_missing".to_string(),
    })?))
}

async fn heartbeat_starter_demand_offer(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(request_id): Path<String>,
    Json(request): Json<StarterDemandHeartbeatRequest>,
) -> Result<Json<StarterDemandHeartbeatResponse>, ApiError> {
    let session = authenticate_session(&state, &headers)?;
    let request_id = normalize_required_field(request_id.as_str(), "request_id_missing")?;
    if let Some(reason) = starter_demand_provider_proof_reason(
        &session,
        normalize_optional_field(request.provider_nostr_pubkey.as_deref()).as_deref(),
    ) {
        return Err(ApiError {
            status: StatusCode::FORBIDDEN,
            error: "forbidden",
            reason,
        });
    }
    let now = now_unix_ms();

    let mut store = state.store.write().map_err(|_| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        error: "internal_error",
        reason: "session_store_poisoned".to_string(),
    })?;
    let expired_events =
        prune_expired_starter_offers(&mut store.starter_demand, &state.config, now);
    record_expired_offer_receipts(&mut store, expired_events, now);
    let mut expired_offer = None;
    let mut released_budget_sats = 0u64;
    let mut heartbeat_offer = None;
    let mut heartbeat_error_reason = None;
    let response = {
        let offers = store
            .starter_demand
            .offers_by_session
            .get_mut(session.session_id.as_str())
            .ok_or_else(|| ApiError {
                status: StatusCode::NOT_FOUND,
                error: "not_found",
                reason: "starter_offer_not_found".to_string(),
            })?;
        let offer = offers
            .iter_mut()
            .find(|offer| offer.request_id == request_id)
            .ok_or_else(|| ApiError {
                status: StatusCode::NOT_FOUND,
                error: "not_found",
                reason: "starter_offer_not_found".to_string(),
            })?;

        if offer.status != StarterOfferStatus::Running {
            return Err(ApiError {
                status: StatusCode::CONFLICT,
                error: "conflict",
                reason: format!("starter_offer_not_running status={}", offer.status.label()),
            });
        }

        let execution_expires_at_unix_ms = offer.execution_expires_at_unix_ms.unwrap_or(now);
        let next_heartbeat_due_at_unix_ms = offer.next_heartbeat_due_at_unix_ms.unwrap_or(now);
        if now > execution_expires_at_unix_ms {
            released_budget_sats = expire_offer(offer, now, "starter_offer_execution_expired");
            expired_offer = Some(offer.clone());
            heartbeat_error_reason = Some("starter_offer_execution_expired".to_string());
            None
        } else if now > next_heartbeat_due_at_unix_ms {
            released_budget_sats = expire_offer(offer, now, "starter_offer_heartbeat_missed");
            expired_offer = Some(offer.clone());
            heartbeat_error_reason = Some("starter_offer_heartbeat_missed".to_string());
            None
        } else {
            let heartbeat_timeout_ms = state
                .config
                .starter_demand_heartbeat_timeout_seconds
                .saturating_mul(1_000);
            offer.last_heartbeat_at_unix_ms = Some(now);
            offer.next_heartbeat_due_at_unix_ms = Some(now.saturating_add(heartbeat_timeout_ms));
            heartbeat_offer = Some(offer.clone());
            Some(StarterDemandHeartbeatResponse {
                request_id: offer.request_id.clone(),
                status: offer.status.label().to_string(),
                last_heartbeat_at_unix_ms: offer.last_heartbeat_at_unix_ms.unwrap_or(now),
                next_heartbeat_due_at_unix_ms: offer.next_heartbeat_due_at_unix_ms.unwrap_or(now),
                execution_expires_at_unix_ms,
                heartbeat_timeout_seconds: state.config.starter_demand_heartbeat_timeout_seconds,
                heartbeat_interval_seconds: starter_demand_heartbeat_interval_seconds(
                    &state.config,
                ),
            })
        }
    };

    if let Some(expired_offer) = expired_offer {
        store.starter_demand.budget_allocated_sats = store
            .starter_demand
            .budget_allocated_sats
            .saturating_sub(released_budget_sats);
        store.economy.record(
            "starter_offer.expired",
            now,
            starter_offer_receipt_context(
                Some(&session),
                Some(&expired_offer),
                expired_offer.failure_reason.as_deref(),
                None,
                None,
            ),
        );
        return Err(ApiError {
            status: StatusCode::CONFLICT,
            error: "conflict",
            reason: heartbeat_error_reason
                .unwrap_or_else(|| "starter_offer_heartbeat_conflict".to_string()),
        });
    }

    if let Some(heartbeat_offer) = heartbeat_offer {
        store.economy.record(
            "starter_offer.heartbeat",
            now,
            starter_offer_receipt_context(Some(&session), Some(&heartbeat_offer), None, None, None),
        );
    }

    Ok(Json(response.ok_or_else(|| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        error: "internal_error",
        reason: "starter_offer_heartbeat_response_missing".to_string(),
    })?))
}

async fn fail_starter_demand_offer(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(request_id): Path<String>,
    Json(request): Json<StarterDemandFailRequest>,
) -> Result<Json<StarterDemandFailResponse>, ApiError> {
    let session = authenticate_session(&state, &headers)?;
    let request_id = normalize_required_field(request_id.as_str(), "request_id_missing")?;
    let failure_reason =
        normalize_required_field(request.failure_reason.as_str(), "failure_reason_missing")?;
    let now = now_unix_ms();

    let mut store = state.store.write().map_err(|_| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        error: "internal_error",
        reason: "session_store_poisoned".to_string(),
    })?;
    let expired_events =
        prune_expired_starter_offers(&mut store.starter_demand, &state.config, now);
    record_expired_offer_receipts(&mut store, expired_events, now);
    let (
        response_request_id,
        response_status,
        response_released_at_unix_ms,
        released_budget_sats,
        released_offer,
    ) = {
        let offers = store
            .starter_demand
            .offers_by_session
            .get_mut(session.session_id.as_str())
            .ok_or_else(|| ApiError {
                status: StatusCode::NOT_FOUND,
                error: "not_found",
                reason: "starter_offer_not_found".to_string(),
            })?;
        let offer = offers
            .iter_mut()
            .find(|offer| offer.request_id == request_id)
            .ok_or_else(|| ApiError {
                status: StatusCode::NOT_FOUND,
                error: "not_found",
                reason: "starter_offer_not_found".to_string(),
            })?;

        let mut released_offer = None;
        let released_budget_sats = match offer.status {
            StarterOfferStatus::Offered | StarterOfferStatus::Running => {
                let released_budget_sats = release_offer(offer, now, failure_reason.as_str());
                released_offer = Some(offer.clone());
                released_budget_sats
            }
            StarterOfferStatus::Released => {
                if offer.failure_reason.as_deref() != Some(failure_reason.as_str()) {
                    return Err(ApiError {
                        status: StatusCode::CONFLICT,
                        error: "conflict",
                        reason: "starter_offer_already_released_with_different_reason".to_string(),
                    });
                }
                0
            }
            StarterOfferStatus::Completed | StarterOfferStatus::Expired => {
                return Err(ApiError {
                    status: StatusCode::CONFLICT,
                    error: "conflict",
                    reason: format!(
                        "starter_offer_not_releasable status={}",
                        offer.status.label()
                    ),
                });
            }
        };

        (
            offer.request_id.clone(),
            offer.status.label().to_string(),
            offer.released_at_unix_ms.unwrap_or(now),
            released_budget_sats,
            released_offer,
        )
    };
    store.starter_demand.budget_allocated_sats = store
        .starter_demand
        .budget_allocated_sats
        .saturating_sub(released_budget_sats);
    if let Some(released_offer) = released_offer {
        store.economy.record(
            "starter_offer.released",
            now,
            starter_offer_receipt_context(
                Some(&session),
                Some(&released_offer),
                released_offer.failure_reason.as_deref(),
                None,
                None,
            ),
        );
    }

    Ok(Json(StarterDemandFailResponse {
        request_id: response_request_id,
        status: response_status,
        released_at_unix_ms: response_released_at_unix_ms,
        failure_reason,
        budget_cap_sats: state.config.starter_demand_budget_cap_sats,
        budget_allocated_sats: store.starter_demand.budget_allocated_sats,
    }))
}

async fn complete_starter_demand_offer(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(request_id): Path<String>,
    Json(request): Json<StarterDemandCompleteRequest>,
) -> Result<Json<StarterDemandCompleteResponse>, ApiError> {
    let session = authenticate_session(&state, &headers)?;
    let request_id = normalize_required_field(request_id.as_str(), "request_id_missing")?;
    let payment_pointer =
        normalize_required_field(request.payment_pointer.as_str(), "payment_pointer_missing")?;
    let now = now_unix_ms();

    let mut store = state.store.write().map_err(|_| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        error: "internal_error",
        reason: "session_store_poisoned".to_string(),
    })?;
    let expired_events =
        prune_expired_starter_offers(&mut store.starter_demand, &state.config, now);
    record_expired_offer_receipts(&mut store, expired_events, now);
    let (
        response_request_id,
        response_status,
        response_completed_at_unix_ms,
        released_budget_sats,
        completed_offer,
    ) = {
        let offers = store
            .starter_demand
            .offers_by_session
            .get_mut(session.session_id.as_str())
            .ok_or_else(|| ApiError {
                status: StatusCode::NOT_FOUND,
                error: "not_found",
                reason: "starter_offer_not_found".to_string(),
            })?;
        let offer = offers
            .iter_mut()
            .find(|offer| offer.request_id == request_id)
            .ok_or_else(|| ApiError {
                status: StatusCode::NOT_FOUND,
                error: "not_found",
                reason: "starter_offer_not_found".to_string(),
            })?;

        let mut completed_offer = None;
        let released_budget_sats = match offer.status {
            StarterOfferStatus::Running => {
                offer.status = StarterOfferStatus::Completed;
                offer.payment_pointer = Some(payment_pointer.clone());
                offer.completed_at_unix_ms = Some(now);
                offer.last_heartbeat_at_unix_ms = Some(now);
                offer.next_heartbeat_due_at_unix_ms = None;
                completed_offer = Some(offer.clone());
                offer.price_sats
            }
            StarterOfferStatus::Completed => {
                if offer.payment_pointer.as_deref() != Some(payment_pointer.as_str()) {
                    return Err(ApiError {
                        status: StatusCode::CONFLICT,
                        error: "conflict",
                        reason: "starter_offer_already_completed_with_different_payment_pointer"
                            .to_string(),
                    });
                }
                0
            }
            StarterOfferStatus::Offered => {
                return Err(ApiError {
                    status: StatusCode::CONFLICT,
                    error: "conflict",
                    reason: "starter_offer_start_not_confirmed".to_string(),
                });
            }
            StarterOfferStatus::Released | StarterOfferStatus::Expired => {
                return Err(ApiError {
                    status: StatusCode::CONFLICT,
                    error: "conflict",
                    reason: format!(
                        "starter_offer_not_completable status={}",
                        offer.status.label()
                    ),
                });
            }
        };

        (
            offer.request_id.clone(),
            offer.status.label().to_string(),
            offer.completed_at_unix_ms.unwrap_or(now),
            released_budget_sats,
            completed_offer,
        )
    };
    store.starter_demand.budget_allocated_sats = store
        .starter_demand
        .budget_allocated_sats
        .saturating_sub(released_budget_sats);
    if let Some(completed_offer) = completed_offer {
        store.economy.record(
            "starter_offer.completed",
            now,
            starter_offer_receipt_context(
                Some(&session),
                Some(&completed_offer),
                None,
                None,
                Some(payment_pointer.as_str()),
            ),
        );
    }

    Ok(Json(StarterDemandCompleteResponse {
        request_id: response_request_id,
        status: response_status,
        payment_pointer,
        completed_at_unix_ms: response_completed_at_unix_ms,
        budget_cap_sats: state.config.starter_demand_budget_cap_sats,
        budget_allocated_sats: store.starter_demand.budget_allocated_sats,
    }))
}

async fn create_kernel_work_unit(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<CreateWorkUnitRequest>,
) -> Result<Json<CreateWorkUnitResponse>, ApiError> {
    let session = authenticate_session(&state, &headers)?;
    let now = now_unix_ms();
    let result = {
        let mut store = state.store.write().map_err(|_| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            error: "internal_error",
            reason: "session_store_poisoned".to_string(),
        })?;
        store
            .kernel
            .create_work_unit(&kernel_mutation_context(&session, now), request)
            .map_err(kernel_api_error)?
    };
    record_kernel_mutation_observability(
        &state,
        &session,
        now,
        "kernel.work_unit.created",
        result.receipt_event.clone(),
        result.snapshot_event.clone(),
    );
    Ok(Json(result.response))
}

async fn create_kernel_contract(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<CreateContractRequest>,
) -> Result<Json<CreateContractResponse>, ApiError> {
    let session = authenticate_session(&state, &headers)?;
    let now = now_unix_ms();
    let result = {
        let mut store = state.store.write().map_err(|_| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            error: "internal_error",
            reason: "session_store_poisoned".to_string(),
        })?;
        store
            .kernel
            .create_contract(&kernel_mutation_context(&session, now), request)
            .map_err(kernel_api_error)?
    };
    record_kernel_mutation_observability(
        &state,
        &session,
        now,
        "kernel.contract.created",
        result.receipt_event.clone(),
        result.snapshot_event.clone(),
    );
    Ok(Json(result.response))
}

async fn submit_kernel_output(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(contract_id): Path<String>,
    Json(mut request): Json<SubmitOutputRequest>,
) -> Result<Json<SubmitOutputResponse>, ApiError> {
    let session = authenticate_session(&state, &headers)?;
    let contract_id = normalize_required_field(contract_id.as_str(), "contract_id_missing")?;
    if !request.submission.contract_id.trim().is_empty()
        && request.submission.contract_id != contract_id
    {
        return Err(ApiError {
            status: StatusCode::CONFLICT,
            error: "conflict",
            reason: "kernel_contract_id_mismatch".to_string(),
        });
    }
    request.submission.contract_id = contract_id;
    let now = now_unix_ms();
    let result = {
        let mut store = state.store.write().map_err(|_| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            error: "internal_error",
            reason: "session_store_poisoned".to_string(),
        })?;
        store
            .kernel
            .submit_output(&kernel_mutation_context(&session, now), request)
            .map_err(kernel_api_error)?
    };
    record_kernel_mutation_observability(
        &state,
        &session,
        now,
        "kernel.submission.received",
        result.receipt_event.clone(),
        result.snapshot_event.clone(),
    );
    Ok(Json(result.response))
}

async fn finalize_kernel_verdict(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(contract_id): Path<String>,
    Json(mut request): Json<FinalizeVerdictRequest>,
) -> Result<Json<FinalizeVerdictResponse>, ApiError> {
    let session = authenticate_session(&state, &headers)?;
    let contract_id = normalize_required_field(contract_id.as_str(), "contract_id_missing")?;
    if !request.verdict.contract_id.trim().is_empty() && request.verdict.contract_id != contract_id
    {
        return Err(ApiError {
            status: StatusCode::CONFLICT,
            error: "conflict",
            reason: "kernel_contract_id_mismatch".to_string(),
        });
    }
    request.verdict.contract_id = contract_id;
    let now = now_unix_ms();
    let result = {
        let mut store = state.store.write().map_err(|_| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            error: "internal_error",
            reason: "session_store_poisoned".to_string(),
        })?;
        store
            .kernel
            .finalize_verdict(&kernel_mutation_context(&session, now), request)
            .map_err(kernel_api_error)?
    };
    record_kernel_mutation_observability(
        &state,
        &session,
        now,
        "kernel.verdict.finalized",
        result.receipt_event.clone(),
        result.snapshot_event.clone(),
    );
    Ok(Json(result.response))
}

#[derive(Debug, Deserialize)]
struct ComputeProductsQuery {
    status: Option<ComputeProductStatus>,
}

#[derive(Debug, Deserialize)]
struct ComputeEnvironmentPackagesQuery {
    family: Option<String>,
    status: Option<ComputeEnvironmentPackageStatus>,
}

#[derive(Debug, Deserialize)]
struct ComputeEnvironmentPackageVersionQuery {
    version: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ComputeRegistryVersionQuery {
    version: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ComputeCheckpointFamilyPoliciesQuery {
    status: Option<ComputeRegistryStatus>,
}

#[derive(Debug, Deserialize)]
struct ComputeValidatorPoliciesQuery {
    validator_pool_ref: Option<String>,
    status: Option<ComputeRegistryStatus>,
}

#[derive(Debug, Deserialize)]
struct ComputeBenchmarkPackagesQuery {
    family: Option<String>,
    environment_ref: Option<String>,
    status: Option<ComputeRegistryStatus>,
}

#[derive(Debug, Deserialize)]
struct ComputeTrainingPoliciesQuery {
    environment_ref: Option<String>,
    status: Option<ComputeRegistryStatus>,
}

#[derive(Debug, Deserialize)]
struct ComputeEvaluationRunsQuery {
    environment_ref: Option<String>,
    product_id: Option<String>,
    status: Option<ComputeEvaluationRunStatus>,
}

#[derive(Debug, Deserialize)]
struct ComputeTrainingRunsQuery {
    training_policy_ref: Option<String>,
    environment_ref: Option<String>,
    status: Option<ComputeTrainingRunStatus>,
}

#[derive(Debug, Deserialize)]
struct ComputeAdapterTrainingWindowsQuery {
    training_run_id: Option<String>,
    status: Option<ComputeAdapterWindowStatus>,
}

#[derive(Debug, Deserialize)]
struct ComputeAdapterContributionOutcomesQuery {
    training_run_id: Option<String>,
    window_id: Option<String>,
    disposition: Option<ComputeAdapterContributionDisposition>,
}

#[derive(Debug, Deserialize)]
struct ComputeAcceptedOutcomesQuery {
    outcome_kind: Option<ComputeAcceptedOutcomeKind>,
    environment_ref: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ComputeSyntheticDataJobsQuery {
    environment_ref: Option<String>,
    generation_product_id: Option<String>,
    status: Option<ComputeSyntheticDataJobStatus>,
}

#[derive(Debug, Deserialize)]
struct CapacityLotsQuery {
    product_id: Option<String>,
    status: Option<CapacityLotStatus>,
}

#[derive(Debug, Deserialize)]
struct CapacityInstrumentsQuery {
    product_id: Option<String>,
    capacity_lot_id: Option<String>,
    status: Option<CapacityInstrumentStatus>,
}

#[derive(Debug, Deserialize)]
struct StructuredCapacityInstrumentsQuery {
    product_id: Option<String>,
    status: Option<StructuredCapacityInstrumentStatus>,
}

#[derive(Debug, Deserialize)]
struct DeliveryProofsQuery {
    status: Option<DeliveryProofStatus>,
}

#[derive(Debug, Deserialize)]
struct ValidatorChallengesQuery {
    status: Option<ComputeValidatorChallengeStatus>,
}

#[derive(Debug, Deserialize)]
struct ComputeIndicesQuery {
    product_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DataAssetsQuery {
    provider_id: Option<String>,
    asset_kind: Option<String>,
    status: Option<DataAssetStatus>,
}

#[derive(Debug, Deserialize)]
struct AccessGrantsQuery {
    asset_id: Option<String>,
    provider_id: Option<String>,
    consumer_id: Option<String>,
    status: Option<AccessGrantStatus>,
}

#[derive(Debug, Deserialize)]
struct DeliveryBundlesQuery {
    asset_id: Option<String>,
    grant_id: Option<String>,
    provider_id: Option<String>,
    consumer_id: Option<String>,
    status: Option<DeliveryBundleStatus>,
}

#[derive(Debug, Deserialize)]
struct RevocationsQuery {
    asset_id: Option<String>,
    grant_id: Option<String>,
    provider_id: Option<String>,
    consumer_id: Option<String>,
    status: Option<RevocationStatus>,
}

fn canonical_challenge_status(
    status: ServiceValidatorChallengeStatus,
) -> ComputeValidatorChallengeStatus {
    match status {
        ServiceValidatorChallengeStatus::Queued => ComputeValidatorChallengeStatus::Queued,
        ServiceValidatorChallengeStatus::Leased => ComputeValidatorChallengeStatus::Leased,
        ServiceValidatorChallengeStatus::Retrying => ComputeValidatorChallengeStatus::Retrying,
        ServiceValidatorChallengeStatus::Verified => ComputeValidatorChallengeStatus::Verified,
        ServiceValidatorChallengeStatus::Rejected => ComputeValidatorChallengeStatus::Rejected,
        ServiceValidatorChallengeStatus::TimedOut => ComputeValidatorChallengeStatus::TimedOut,
    }
}

fn service_challenge_status(
    status: ComputeValidatorChallengeStatus,
) -> ServiceValidatorChallengeStatus {
    match status {
        ComputeValidatorChallengeStatus::Queued => ServiceValidatorChallengeStatus::Queued,
        ComputeValidatorChallengeStatus::Leased => ServiceValidatorChallengeStatus::Leased,
        ComputeValidatorChallengeStatus::Retrying => ServiceValidatorChallengeStatus::Retrying,
        ComputeValidatorChallengeStatus::Verified => ServiceValidatorChallengeStatus::Verified,
        ComputeValidatorChallengeStatus::Rejected => ServiceValidatorChallengeStatus::Rejected,
        ComputeValidatorChallengeStatus::TimedOut => ServiceValidatorChallengeStatus::TimedOut,
    }
}

fn canonical_challenge_protocol(
    protocol: openagents_validator_service::ValidatorChallengeProtocolKind,
) -> ComputeValidatorChallengeProtocolKind {
    match protocol {
        openagents_validator_service::ValidatorChallengeProtocolKind::GpuFreivaldsMerkleV1 => {
            ComputeValidatorChallengeProtocolKind::GpuFreivaldsMerkleV1
        }
    }
}

fn canonical_challenge_verdict(
    verdict: openagents_validator_service::ValidatorChallengeVerdict,
) -> ComputeValidatorChallengeVerdict {
    match verdict {
        openagents_validator_service::ValidatorChallengeVerdict::Verified => {
            ComputeValidatorChallengeVerdict::Verified
        }
        openagents_validator_service::ValidatorChallengeVerdict::Rejected => {
            ComputeValidatorChallengeVerdict::Rejected
        }
        openagents_validator_service::ValidatorChallengeVerdict::RetryScheduled => {
            ComputeValidatorChallengeVerdict::RetryScheduled
        }
        openagents_validator_service::ValidatorChallengeVerdict::TimedOut => {
            ComputeValidatorChallengeVerdict::TimedOut
        }
    }
}

fn canonical_challenge_failure_code(
    code: openagents_validator_service::ValidatorChallengeFailureCode,
) -> ComputeValidatorChallengeFailureCode {
    match code {
        openagents_validator_service::ValidatorChallengeFailureCode::DimensionMismatch => {
            ComputeValidatorChallengeFailureCode::DimensionMismatch
        }
        openagents_validator_service::ValidatorChallengeFailureCode::FieldMismatch => {
            ComputeValidatorChallengeFailureCode::FieldMismatch
        }
        openagents_validator_service::ValidatorChallengeFailureCode::RowOpeningMissing => {
            ComputeValidatorChallengeFailureCode::RowOpeningMissing
        }
        openagents_validator_service::ValidatorChallengeFailureCode::MerkleProofInvalid => {
            ComputeValidatorChallengeFailureCode::MerkleProofInvalid
        }
        openagents_validator_service::ValidatorChallengeFailureCode::FreivaldsMismatch => {
            ComputeValidatorChallengeFailureCode::FreivaldsMismatch
        }
        openagents_validator_service::ValidatorChallengeFailureCode::LeaseExpired => {
            ComputeValidatorChallengeFailureCode::LeaseExpired
        }
        openagents_validator_service::ValidatorChallengeFailureCode::RetryBudgetExhausted => {
            ComputeValidatorChallengeFailureCode::RetryBudgetExhausted
        }
    }
}

fn canonical_challenge_snapshot(
    snapshot: openagents_validator_service::ValidatorChallengeSnapshot,
) -> ComputeValidatorChallengeSnapshot {
    ComputeValidatorChallengeSnapshot {
        request: ComputeValidatorChallengeRequest {
            context: ComputeValidatorChallengeContext {
                challenge_id: snapshot.request.context.challenge_id,
                proof_bundle_digest: snapshot.request.context.proof_bundle_digest,
                request_digest: snapshot.request.context.request_digest,
                delivery_proof_id: snapshot.request.context.delivery_proof_id,
                product_id: snapshot.request.context.product_id,
                runtime_backend: snapshot.request.context.runtime_backend,
                model_id: snapshot.request.context.model_id,
                validator_pool_ref: snapshot.request.context.validator_pool_ref,
                created_at_ms: snapshot.request.context.created_at_ms,
                max_attempts: snapshot.request.context.max_attempts,
                lease_timeout_ms: snapshot.request.context.lease_timeout_ms,
            },
            protocol: canonical_challenge_protocol(snapshot.request.protocol),
        },
        status: canonical_challenge_status(snapshot.status),
        attempts_used: snapshot.attempts_used,
        active_lease: snapshot
            .active_lease
            .map(|lease| ComputeValidatorChallengeLease {
                challenge_id: lease.challenge_id,
                attempt: lease.attempt,
                validator_id: lease.validator_id,
                leased_at_ms: lease.leased_at_ms,
                expires_at_ms: lease.expires_at_ms,
            }),
        final_result: snapshot
            .final_result
            .map(|result| ComputeValidatorChallengeResult {
                challenge_id: result.challenge_id,
                proof_bundle_digest: result.proof_bundle_digest,
                protocol_id: result.protocol_id,
                attempt: result.attempt,
                status: canonical_challenge_status(result.status),
                verdict: canonical_challenge_verdict(result.verdict),
                reason_code: result.reason_code.map(canonical_challenge_failure_code),
                detail: result.detail,
                created_at_ms: result.created_at_ms,
                finalized_at_ms: result.finalized_at_ms,
                challenge_seed_digest: result.challenge_seed_digest,
                verified_row_count: result.verified_row_count,
                result_digest: result.result_digest,
                challenge_result_ref: result.challenge_result_ref,
            }),
    }
}

async fn list_kernel_compute_products(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<ComputeProductsQuery>,
) -> Result<Json<proto_compute::ListComputeProductsResponse>, ApiError> {
    let _session = authenticate_session(&state, &headers)?;
    let store = state.store.read().map_err(|_| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        error: "internal_error",
        reason: "session_store_poisoned".to_string(),
    })?;
    let response = compute_contracts::list_compute_products_response_to_proto(
        store.kernel.list_compute_products(query.status).as_slice(),
    )
    .map_err(kernel_contract_error)?;
    Ok(Json(response))
}

async fn get_kernel_compute_product(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(product_id): Path<String>,
) -> Result<Json<proto_compute::GetComputeProductResponse>, ApiError> {
    let _session = authenticate_session(&state, &headers)?;
    let product_id = normalize_required_field(product_id.as_str(), "product_id_missing")?;
    let store = state.store.read().map_err(|_| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        error: "internal_error",
        reason: "session_store_poisoned".to_string(),
    })?;
    let Some(product) = store.kernel.get_compute_product(product_id.as_str()) else {
        return Err(ApiError {
            status: StatusCode::NOT_FOUND,
            error: "not_found",
            reason: "kernel_compute_product_not_found".to_string(),
        });
    };
    let response = compute_contracts::get_compute_product_response_to_proto(&product)
        .map_err(kernel_contract_error)?;
    Ok(Json(response))
}

async fn list_kernel_compute_environment_packages(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<ComputeEnvironmentPackagesQuery>,
) -> Result<Json<proto_compute::ListComputeEnvironmentPackagesResponse>, ApiError> {
    let _session = authenticate_session(&state, &headers)?;
    let store = state.store.read().map_err(|_| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        error: "internal_error",
        reason: "session_store_poisoned".to_string(),
    })?;
    let response = compute_contracts::list_compute_environment_packages_response_to_proto(
        store
            .kernel
            .list_compute_environment_packages(query.family.as_deref(), query.status)
            .as_slice(),
    )
    .map_err(kernel_contract_error)?;
    Ok(Json(response))
}

async fn get_kernel_compute_environment_package(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(environment_ref): Path<String>,
    Query(query): Query<ComputeEnvironmentPackageVersionQuery>,
) -> Result<Json<proto_compute::GetComputeEnvironmentPackageResponse>, ApiError> {
    let _session = authenticate_session(&state, &headers)?;
    let environment_ref =
        normalize_required_field(environment_ref.as_str(), "compute_environment_ref_missing")?;
    let store = state.store.read().map_err(|_| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        error: "internal_error",
        reason: "session_store_poisoned".to_string(),
    })?;
    let Some(package) = store
        .kernel
        .get_compute_environment_package(environment_ref.as_str(), query.version.as_deref())
    else {
        return Err(ApiError {
            status: StatusCode::NOT_FOUND,
            error: "not_found",
            reason: "kernel_compute_environment_package_not_found".to_string(),
        });
    };
    let response = compute_contracts::get_compute_environment_package_response_to_proto(&package)
        .map_err(kernel_contract_error)?;
    Ok(Json(response))
}

async fn list_kernel_compute_checkpoint_family_policies(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<ComputeCheckpointFamilyPoliciesQuery>,
) -> Result<Json<proto_compute::ListComputeCheckpointFamilyPoliciesResponse>, ApiError> {
    let _session = authenticate_session(&state, &headers)?;
    let store = state.store.read().map_err(|_| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        error: "internal_error",
        reason: "session_store_poisoned".to_string(),
    })?;
    let response = compute_contracts::list_compute_checkpoint_family_policies_response_to_proto(
        store
            .kernel
            .list_compute_checkpoint_family_policies(query.status)
            .as_slice(),
    )
    .map_err(kernel_contract_error)?;
    Ok(Json(response))
}

async fn get_kernel_compute_checkpoint_family_policy(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(checkpoint_family): Path<String>,
    Query(query): Query<ComputeRegistryVersionQuery>,
) -> Result<Json<proto_compute::GetComputeCheckpointFamilyPolicyResponse>, ApiError> {
    let _session = authenticate_session(&state, &headers)?;
    let checkpoint_family = normalize_required_field(
        checkpoint_family.as_str(),
        "compute_checkpoint_family_missing",
    )?;
    let store = state.store.read().map_err(|_| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        error: "internal_error",
        reason: "session_store_poisoned".to_string(),
    })?;
    let Some(policy_record) = store
        .kernel
        .get_compute_checkpoint_family_policy(checkpoint_family.as_str(), query.version.as_deref())
    else {
        return Err(ApiError {
            status: StatusCode::NOT_FOUND,
            error: "not_found",
            reason: "kernel_compute_checkpoint_family_policy_not_found".to_string(),
        });
    };
    let response =
        compute_contracts::get_compute_checkpoint_family_policy_response_to_proto(&policy_record)
            .map_err(kernel_contract_error)?;
    Ok(Json(response))
}

async fn list_kernel_compute_validator_policies(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<ComputeValidatorPoliciesQuery>,
) -> Result<Json<proto_compute::ListComputeValidatorPoliciesResponse>, ApiError> {
    let _session = authenticate_session(&state, &headers)?;
    let store = state.store.read().map_err(|_| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        error: "internal_error",
        reason: "session_store_poisoned".to_string(),
    })?;
    let response = compute_contracts::list_compute_validator_policies_response_to_proto(
        store
            .kernel
            .list_compute_validator_policies(query.validator_pool_ref.as_deref(), query.status)
            .as_slice(),
    )
    .map_err(kernel_contract_error)?;
    Ok(Json(response))
}

async fn get_kernel_compute_validator_policy(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(policy_ref): Path<String>,
    Query(query): Query<ComputeRegistryVersionQuery>,
) -> Result<Json<proto_compute::GetComputeValidatorPolicyResponse>, ApiError> {
    let _session = authenticate_session(&state, &headers)?;
    let policy_ref =
        normalize_required_field(policy_ref.as_str(), "compute_validator_policy_ref_missing")?;
    let store = state.store.read().map_err(|_| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        error: "internal_error",
        reason: "session_store_poisoned".to_string(),
    })?;
    let Some(policy_record) = store
        .kernel
        .get_compute_validator_policy(policy_ref.as_str(), query.version.as_deref())
    else {
        return Err(ApiError {
            status: StatusCode::NOT_FOUND,
            error: "not_found",
            reason: "kernel_compute_validator_policy_not_found".to_string(),
        });
    };
    let response =
        compute_contracts::get_compute_validator_policy_response_to_proto(&policy_record)
            .map_err(kernel_contract_error)?;
    Ok(Json(response))
}

async fn list_kernel_compute_benchmark_packages(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<ComputeBenchmarkPackagesQuery>,
) -> Result<Json<proto_compute::ListComputeBenchmarkPackagesResponse>, ApiError> {
    let _session = authenticate_session(&state, &headers)?;
    let store = state.store.read().map_err(|_| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        error: "internal_error",
        reason: "session_store_poisoned".to_string(),
    })?;
    let response = compute_contracts::list_compute_benchmark_packages_response_to_proto(
        store
            .kernel
            .list_compute_benchmark_packages(
                query.family.as_deref(),
                query.environment_ref.as_deref(),
                query.status,
            )
            .as_slice(),
    )
    .map_err(kernel_contract_error)?;
    Ok(Json(response))
}

async fn get_kernel_compute_benchmark_package(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(benchmark_package_ref): Path<String>,
    Query(query): Query<ComputeRegistryVersionQuery>,
) -> Result<Json<proto_compute::GetComputeBenchmarkPackageResponse>, ApiError> {
    let _session = authenticate_session(&state, &headers)?;
    let benchmark_package_ref = normalize_required_field(
        benchmark_package_ref.as_str(),
        "compute_benchmark_package_ref_missing",
    )?;
    let store = state.store.read().map_err(|_| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        error: "internal_error",
        reason: "session_store_poisoned".to_string(),
    })?;
    let Some(benchmark_package) = store
        .kernel
        .get_compute_benchmark_package(benchmark_package_ref.as_str(), query.version.as_deref())
    else {
        return Err(ApiError {
            status: StatusCode::NOT_FOUND,
            error: "not_found",
            reason: "kernel_compute_benchmark_package_not_found".to_string(),
        });
    };
    let response =
        compute_contracts::get_compute_benchmark_package_response_to_proto(&benchmark_package)
            .map_err(kernel_contract_error)?;
    Ok(Json(response))
}

async fn list_kernel_compute_training_policies(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<ComputeTrainingPoliciesQuery>,
) -> Result<Json<proto_compute::ListComputeTrainingPoliciesResponse>, ApiError> {
    let _session = authenticate_session(&state, &headers)?;
    let store = state.store.read().map_err(|_| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        error: "internal_error",
        reason: "session_store_poisoned".to_string(),
    })?;
    let response = compute_contracts::list_compute_training_policies_response_to_proto(
        store
            .kernel
            .list_compute_training_policies(query.environment_ref.as_deref(), query.status)
            .as_slice(),
    )
    .map_err(kernel_contract_error)?;
    Ok(Json(response))
}

async fn get_kernel_compute_training_policy(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(training_policy_ref): Path<String>,
    Query(query): Query<ComputeRegistryVersionQuery>,
) -> Result<Json<proto_compute::GetComputeTrainingPolicyResponse>, ApiError> {
    let _session = authenticate_session(&state, &headers)?;
    let training_policy_ref = normalize_required_field(
        training_policy_ref.as_str(),
        "compute_training_policy_ref_missing",
    )?;
    let store = state.store.read().map_err(|_| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        error: "internal_error",
        reason: "session_store_poisoned".to_string(),
    })?;
    let Some(training_policy) = store
        .kernel
        .get_compute_training_policy(training_policy_ref.as_str(), query.version.as_deref())
    else {
        return Err(ApiError {
            status: StatusCode::NOT_FOUND,
            error: "not_found",
            reason: "kernel_compute_training_policy_not_found".to_string(),
        });
    };
    let response =
        compute_contracts::get_compute_training_policy_response_to_proto(&training_policy)
            .map_err(kernel_contract_error)?;
    Ok(Json(response))
}

async fn list_kernel_compute_evaluation_runs(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<ComputeEvaluationRunsQuery>,
) -> Result<Json<proto_compute::ListComputeEvaluationRunsResponse>, ApiError> {
    let _session = authenticate_session(&state, &headers)?;
    let store = state.store.read().map_err(|_| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        error: "internal_error",
        reason: "session_store_poisoned".to_string(),
    })?;
    let response = compute_contracts::list_compute_evaluation_runs_response_to_proto(
        store
            .kernel
            .list_compute_evaluation_runs(
                query.environment_ref.as_deref(),
                query.product_id.as_deref(),
                query.status,
            )
            .as_slice(),
    )
    .map_err(kernel_contract_error)?;
    Ok(Json(response))
}

async fn get_kernel_compute_evaluation_run(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(eval_run_id): Path<String>,
) -> Result<Json<proto_compute::GetComputeEvaluationRunResponse>, ApiError> {
    let _session = authenticate_session(&state, &headers)?;
    let eval_run_id =
        normalize_required_field(eval_run_id.as_str(), "compute_eval_run_id_missing")?;
    let store = state.store.read().map_err(|_| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        error: "internal_error",
        reason: "session_store_poisoned".to_string(),
    })?;
    let Some(eval_run) = store
        .kernel
        .get_compute_evaluation_run(eval_run_id.as_str())
    else {
        return Err(ApiError {
            status: StatusCode::NOT_FOUND,
            error: "not_found",
            reason: "kernel_compute_eval_run_not_found".to_string(),
        });
    };
    let response = compute_contracts::get_compute_evaluation_run_response_to_proto(&eval_run)
        .map_err(kernel_contract_error)?;
    Ok(Json(response))
}

async fn list_kernel_compute_evaluation_samples(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(eval_run_id): Path<String>,
) -> Result<Json<proto_compute::ListComputeEvaluationSamplesResponse>, ApiError> {
    let _session = authenticate_session(&state, &headers)?;
    let eval_run_id =
        normalize_required_field(eval_run_id.as_str(), "compute_eval_run_id_missing")?;
    let store = state.store.read().map_err(|_| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        error: "internal_error",
        reason: "session_store_poisoned".to_string(),
    })?;
    if store
        .kernel
        .get_compute_evaluation_run(eval_run_id.as_str())
        .is_none()
    {
        return Err(ApiError {
            status: StatusCode::NOT_FOUND,
            error: "not_found",
            reason: "kernel_compute_eval_run_not_found".to_string(),
        });
    }
    let response = compute_contracts::list_compute_evaluation_samples_response_to_proto(
        store
            .kernel
            .list_compute_evaluation_samples(eval_run_id.as_str())
            .as_slice(),
    )
    .map_err(kernel_contract_error)?;
    Ok(Json(response))
}

async fn list_kernel_compute_synthetic_data_jobs(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<ComputeSyntheticDataJobsQuery>,
) -> Result<Json<proto_compute::ListComputeSyntheticDataJobsResponse>, ApiError> {
    let _session = authenticate_session(&state, &headers)?;
    let store = state.store.read().map_err(|_| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        error: "internal_error",
        reason: "session_store_poisoned".to_string(),
    })?;
    let response = compute_contracts::list_compute_synthetic_data_jobs_response_to_proto(
        store
            .kernel
            .list_compute_synthetic_data_jobs(
                query.environment_ref.as_deref(),
                query.generation_product_id.as_deref(),
                query.status,
            )
            .as_slice(),
    )
    .map_err(kernel_contract_error)?;
    Ok(Json(response))
}

async fn get_kernel_compute_synthetic_data_job(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(synthetic_job_id): Path<String>,
) -> Result<Json<proto_compute::GetComputeSyntheticDataJobResponse>, ApiError> {
    let _session = authenticate_session(&state, &headers)?;
    let synthetic_job_id = normalize_required_field(
        synthetic_job_id.as_str(),
        "compute_synthetic_job_id_missing",
    )?;
    let store = state.store.read().map_err(|_| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        error: "internal_error",
        reason: "session_store_poisoned".to_string(),
    })?;
    let Some(synthetic_job) = store
        .kernel
        .get_compute_synthetic_data_job(synthetic_job_id.as_str())
    else {
        return Err(ApiError {
            status: StatusCode::NOT_FOUND,
            error: "not_found",
            reason: "kernel_compute_synthetic_job_not_found".to_string(),
        });
    };
    let response =
        compute_contracts::get_compute_synthetic_data_job_response_to_proto(&synthetic_job)
            .map_err(kernel_contract_error)?;
    Ok(Json(response))
}

async fn list_kernel_compute_synthetic_data_samples(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(synthetic_job_id): Path<String>,
) -> Result<Json<proto_compute::ListComputeSyntheticDataSamplesResponse>, ApiError> {
    let _session = authenticate_session(&state, &headers)?;
    let synthetic_job_id = normalize_required_field(
        synthetic_job_id.as_str(),
        "compute_synthetic_job_id_missing",
    )?;
    let store = state.store.read().map_err(|_| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        error: "internal_error",
        reason: "session_store_poisoned".to_string(),
    })?;
    if store
        .kernel
        .get_compute_synthetic_data_job(synthetic_job_id.as_str())
        .is_none()
    {
        return Err(ApiError {
            status: StatusCode::NOT_FOUND,
            error: "not_found",
            reason: "kernel_compute_synthetic_job_not_found".to_string(),
        });
    }
    let response = compute_contracts::list_compute_synthetic_data_samples_response_to_proto(
        store
            .kernel
            .list_compute_synthetic_data_samples(synthetic_job_id.as_str())
            .as_slice(),
    )
    .map_err(kernel_contract_error)?;
    Ok(Json(response))
}

async fn list_kernel_capacity_lots(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<CapacityLotsQuery>,
) -> Result<Json<proto_compute::ListCapacityLotsResponse>, ApiError> {
    let _session = authenticate_session(&state, &headers)?;
    let store = state.store.read().map_err(|_| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        error: "internal_error",
        reason: "session_store_poisoned".to_string(),
    })?;
    let response = compute_contracts::list_capacity_lots_response_to_proto(
        store
            .kernel
            .list_capacity_lots(query.product_id.as_deref(), query.status)
            .as_slice(),
    )
    .map_err(kernel_contract_error)?;
    Ok(Json(response))
}

async fn get_kernel_capacity_lot(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(lot_id): Path<String>,
) -> Result<Json<proto_compute::GetCapacityLotResponse>, ApiError> {
    let _session = authenticate_session(&state, &headers)?;
    let lot_id = normalize_required_field(lot_id.as_str(), "capacity_lot_id_missing")?;
    let store = state.store.read().map_err(|_| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        error: "internal_error",
        reason: "session_store_poisoned".to_string(),
    })?;
    let Some(lot) = store.kernel.get_capacity_lot(lot_id.as_str()) else {
        return Err(ApiError {
            status: StatusCode::NOT_FOUND,
            error: "not_found",
            reason: "kernel_capacity_lot_not_found".to_string(),
        });
    };
    let response = compute_contracts::get_capacity_lot_response_to_proto(&lot)
        .map_err(kernel_contract_error)?;
    Ok(Json(response))
}

async fn list_kernel_capacity_instruments(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<CapacityInstrumentsQuery>,
) -> Result<Json<proto_compute::ListCapacityInstrumentsResponse>, ApiError> {
    let _session = authenticate_session(&state, &headers)?;
    let store = state.store.read().map_err(|_| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        error: "internal_error",
        reason: "session_store_poisoned".to_string(),
    })?;
    let response = compute_contracts::list_capacity_instruments_response_to_proto(
        store
            .kernel
            .list_capacity_instruments(
                query.product_id.as_deref(),
                query.capacity_lot_id.as_deref(),
                query.status,
            )
            .as_slice(),
    )
    .map_err(kernel_contract_error)?;
    Ok(Json(response))
}

async fn get_kernel_capacity_instrument(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(instrument_id): Path<String>,
) -> Result<Json<proto_compute::GetCapacityInstrumentResponse>, ApiError> {
    let _session = authenticate_session(&state, &headers)?;
    let instrument_id = normalize_required_field(instrument_id.as_str(), "instrument_id_missing")?;
    let store = state.store.read().map_err(|_| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        error: "internal_error",
        reason: "session_store_poisoned".to_string(),
    })?;
    let Some(instrument) = store.kernel.get_capacity_instrument(instrument_id.as_str()) else {
        return Err(ApiError {
            status: StatusCode::NOT_FOUND,
            error: "not_found",
            reason: "kernel_capacity_instrument_not_found".to_string(),
        });
    };
    let response = compute_contracts::get_capacity_instrument_response_to_proto(&instrument)
        .map_err(kernel_contract_error)?;
    Ok(Json(response))
}

async fn list_kernel_structured_capacity_instruments(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<StructuredCapacityInstrumentsQuery>,
) -> Result<Json<proto_compute::ListStructuredCapacityInstrumentsResponse>, ApiError> {
    let _session = authenticate_session(&state, &headers)?;
    let store = state.store.read().map_err(|_| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        error: "internal_error",
        reason: "session_store_poisoned".to_string(),
    })?;
    let response = compute_contracts::list_structured_capacity_instruments_response_to_proto(
        store
            .kernel
            .list_structured_capacity_instruments(query.product_id.as_deref(), query.status)
            .as_slice(),
    )
    .map_err(kernel_contract_error)?;
    Ok(Json(response))
}

async fn get_kernel_structured_capacity_instrument(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(structured_instrument_id): Path<String>,
) -> Result<Json<proto_compute::GetStructuredCapacityInstrumentResponse>, ApiError> {
    let _session = authenticate_session(&state, &headers)?;
    let structured_instrument_id = normalize_required_field(
        structured_instrument_id.as_str(),
        "structured_capacity_instrument_id_missing",
    )?;
    let store = state.store.read().map_err(|_| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        error: "internal_error",
        reason: "session_store_poisoned".to_string(),
    })?;
    let Some(structured_instrument) = store
        .kernel
        .get_structured_capacity_instrument(structured_instrument_id.as_str())
    else {
        return Err(ApiError {
            status: StatusCode::NOT_FOUND,
            error: "not_found",
            reason: "kernel_structured_capacity_instrument_not_found".to_string(),
        });
    };
    let response = compute_contracts::get_structured_capacity_instrument_response_to_proto(
        &structured_instrument,
    )
    .map_err(kernel_contract_error)?;
    Ok(Json(response))
}

async fn list_kernel_delivery_proofs(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(lot_id): Path<String>,
    Query(query): Query<DeliveryProofsQuery>,
) -> Result<Json<proto_compute::ListDeliveryProofsResponse>, ApiError> {
    let _session = authenticate_session(&state, &headers)?;
    let lot_id = normalize_required_field(lot_id.as_str(), "capacity_lot_id_missing")?;
    let store = state.store.read().map_err(|_| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        error: "internal_error",
        reason: "session_store_poisoned".to_string(),
    })?;
    let response = compute_contracts::list_delivery_proofs_response_to_proto(
        store
            .kernel
            .list_delivery_proofs(Some(lot_id.as_str()), query.status)
            .as_slice(),
    )
    .map_err(kernel_contract_error)?;
    Ok(Json(response))
}

async fn get_kernel_delivery_proof(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(delivery_proof_id): Path<String>,
) -> Result<Json<proto_compute::GetDeliveryProofResponse>, ApiError> {
    let _session = authenticate_session(&state, &headers)?;
    let delivery_proof_id =
        normalize_required_field(delivery_proof_id.as_str(), "delivery_proof_id_missing")?;
    let store = state.store.read().map_err(|_| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        error: "internal_error",
        reason: "session_store_poisoned".to_string(),
    })?;
    let Some(delivery_proof) = store.kernel.get_delivery_proof(delivery_proof_id.as_str()) else {
        return Err(ApiError {
            status: StatusCode::NOT_FOUND,
            error: "not_found",
            reason: "kernel_delivery_proof_not_found".to_string(),
        });
    };
    let response = compute_contracts::get_delivery_proof_response_to_proto(&delivery_proof)
        .map_err(kernel_contract_error)?;
    Ok(Json(response))
}

async fn list_kernel_validator_challenges(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<ValidatorChallengesQuery>,
) -> Result<Json<Vec<ComputeValidatorChallengeSnapshot>>, ApiError> {
    let _session = authenticate_session(&state, &headers)?;
    let store = state.store.read().map_err(|_| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        error: "internal_error",
        reason: "session_store_poisoned".to_string(),
    })?;
    Ok(Json(
        store
            .kernel
            .list_validator_challenges(query.status.map(service_challenge_status))
            .into_iter()
            .map(canonical_challenge_snapshot)
            .collect(),
    ))
}

async fn get_kernel_validator_challenge(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(challenge_id): Path<String>,
) -> Result<Json<ComputeValidatorChallengeSnapshot>, ApiError> {
    let _session = authenticate_session(&state, &headers)?;
    let challenge_id =
        normalize_required_field(challenge_id.as_str(), "validator_challenge_id_missing")?;
    let store = state.store.read().map_err(|_| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        error: "internal_error",
        reason: "session_store_poisoned".to_string(),
    })?;
    let Some(challenge) = store.kernel.get_validator_challenge(challenge_id.as_str()) else {
        return Err(ApiError {
            status: StatusCode::NOT_FOUND,
            error: "not_found",
            reason: "kernel_validator_challenge_not_found".to_string(),
        });
    };
    Ok(Json(canonical_challenge_snapshot(challenge)))
}

async fn schedule_kernel_validator_challenge(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<ScheduleValidatorChallengeRequest>,
) -> Result<Json<ScheduleValidatorChallengeResponse>, ApiError> {
    let session = authenticate_session(&state, &headers)?;
    let now = now_unix_ms();
    let result = {
        let mut store = state.store.write().map_err(|_| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            error: "internal_error",
            reason: "session_store_poisoned".to_string(),
        })?;
        store
            .kernel
            .schedule_validator_challenge(&kernel_mutation_context(&session, now), request)
            .map_err(kernel_api_error)?
    };
    record_kernel_mutation_observability(
        &state,
        &session,
        now,
        "kernel.compute.validator_challenge.scheduled",
        result.receipt_event.clone(),
        result.snapshot_event.clone(),
    );
    Ok(Json(result.response))
}

async fn lease_kernel_validator_challenge(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(challenge_id): Path<String>,
    Json(mut request): Json<LeaseValidatorChallengeRequest>,
) -> Result<Json<LeaseValidatorChallengeResponse>, ApiError> {
    let session = authenticate_session(&state, &headers)?;
    let challenge_id =
        normalize_required_field(challenge_id.as_str(), "validator_challenge_id_missing")?;
    if !request.challenge_id.trim().is_empty() && request.challenge_id != challenge_id {
        return Err(ApiError {
            status: StatusCode::CONFLICT,
            error: "conflict",
            reason: "kernel_validator_challenge_id_mismatch".to_string(),
        });
    }
    request.challenge_id = challenge_id;
    let now = now_unix_ms();
    let result = {
        let mut store = state.store.write().map_err(|_| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            error: "internal_error",
            reason: "session_store_poisoned".to_string(),
        })?;
        store
            .kernel
            .lease_validator_challenge(&kernel_mutation_context(&session, now), request)
            .map_err(kernel_api_error)?
    };
    record_kernel_mutation_observability(
        &state,
        &session,
        now,
        "kernel.compute.validator_challenge.leased",
        result.receipt_event.clone(),
        result.snapshot_event.clone(),
    );
    Ok(Json(result.response))
}

async fn finalize_kernel_validator_challenge(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(challenge_id): Path<String>,
    Json(mut request): Json<FinalizeValidatorChallengeRequest>,
) -> Result<Json<FinalizeValidatorChallengeResponse>, ApiError> {
    let session = authenticate_session(&state, &headers)?;
    let challenge_id =
        normalize_required_field(challenge_id.as_str(), "validator_challenge_id_missing")?;
    if !request.lease.challenge_id.trim().is_empty() && request.lease.challenge_id != challenge_id {
        return Err(ApiError {
            status: StatusCode::CONFLICT,
            error: "conflict",
            reason: "kernel_validator_challenge_id_mismatch".to_string(),
        });
    }
    if !request.result.challenge_id.trim().is_empty() && request.result.challenge_id != challenge_id
    {
        return Err(ApiError {
            status: StatusCode::CONFLICT,
            error: "conflict",
            reason: "kernel_validator_challenge_result_id_mismatch".to_string(),
        });
    }
    request.lease.challenge_id.clone_from(&challenge_id);
    request.result.challenge_id = challenge_id;
    let now = now_unix_ms();
    let result = {
        let mut store = state.store.write().map_err(|_| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            error: "internal_error",
            reason: "session_store_poisoned".to_string(),
        })?;
        store
            .kernel
            .finalize_validator_challenge(&kernel_mutation_context(&session, now), request)
            .map_err(kernel_api_error)?
    };
    record_kernel_mutation_observability(
        &state,
        &session,
        now,
        "kernel.compute.validator_challenge.finalized",
        result.receipt_event.clone(),
        result.snapshot_event.clone(),
    );
    Ok(Json(result.response))
}

async fn list_kernel_compute_indices(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<ComputeIndicesQuery>,
) -> Result<Json<proto_compute::ListComputeIndicesResponse>, ApiError> {
    let _session = authenticate_session(&state, &headers)?;
    let store = state.store.read().map_err(|_| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        error: "internal_error",
        reason: "session_store_poisoned".to_string(),
    })?;
    let response = compute_contracts::list_compute_indices_response_to_proto(
        store
            .kernel
            .list_compute_indices(query.product_id.as_deref())
            .as_slice(),
    )
    .map_err(kernel_contract_error)?;
    Ok(Json(response))
}

async fn get_kernel_compute_index(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(index_id): Path<String>,
) -> Result<Json<proto_compute::GetComputeIndexResponse>, ApiError> {
    let _session = authenticate_session(&state, &headers)?;
    let index_id = normalize_required_field(index_id.as_str(), "index_id_missing")?;
    let store = state.store.read().map_err(|_| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        error: "internal_error",
        reason: "session_store_poisoned".to_string(),
    })?;
    let Some(index) = store.kernel.get_compute_index(index_id.as_str()) else {
        return Err(ApiError {
            status: StatusCode::NOT_FOUND,
            error: "not_found",
            reason: "kernel_compute_index_not_found".to_string(),
        });
    };
    let response = compute_contracts::get_compute_index_response_to_proto(&index)
        .map_err(kernel_contract_error)?;
    Ok(Json(response))
}

async fn create_kernel_compute_product(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<proto_compute::CreateComputeProductRequest>,
) -> Result<Json<proto_compute::CreateComputeProductResponse>, ApiError> {
    let session = authenticate_session(&state, &headers)?;
    let request = compute_contracts::create_compute_product_request_from_proto(&request)
        .map_err(kernel_contract_error)?;
    let now = now_unix_ms();
    let result = {
        let mut store = state.store.write().map_err(|_| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            error: "internal_error",
            reason: "session_store_poisoned".to_string(),
        })?;
        store
            .kernel
            .create_compute_product(&kernel_mutation_context(&session, now), request)
            .map_err(kernel_api_error)?
    };
    record_kernel_mutation_observability(
        &state,
        &session,
        now,
        "kernel.compute.product.created",
        result.receipt_event.clone(),
        result.snapshot_event.clone(),
    );
    let response = compute_contracts::create_compute_product_response_to_proto(&result.response)
        .map_err(kernel_contract_error)?;
    Ok(Json(response))
}

async fn register_kernel_compute_environment_package(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<proto_compute::RegisterComputeEnvironmentPackageRequest>,
) -> Result<Json<proto_compute::RegisterComputeEnvironmentPackageResponse>, ApiError> {
    let session = authenticate_session(&state, &headers)?;
    let request =
        compute_contracts::register_compute_environment_package_request_from_proto(&request)
            .map_err(kernel_contract_error)?;
    let now = now_unix_ms();
    let result = {
        let mut store = state.store.write().map_err(|_| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            error: "internal_error",
            reason: "session_store_poisoned".to_string(),
        })?;
        store
            .kernel
            .register_compute_environment_package(&kernel_mutation_context(&session, now), request)
            .map_err(kernel_api_error)?
    };
    record_kernel_mutation_observability(
        &state,
        &session,
        now,
        "kernel.compute.environment.registered",
        result.receipt_event.clone(),
        result.snapshot_event.clone(),
    );
    let response =
        compute_contracts::register_compute_environment_package_response_to_proto(&result.response)
            .map_err(kernel_contract_error)?;
    Ok(Json(response))
}

async fn register_kernel_compute_checkpoint_family_policy(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<proto_compute::RegisterComputeCheckpointFamilyPolicyRequest>,
) -> Result<Json<proto_compute::RegisterComputeCheckpointFamilyPolicyResponse>, ApiError> {
    let session = authenticate_session(&state, &headers)?;
    let request =
        compute_contracts::register_compute_checkpoint_family_policy_request_from_proto(&request)
            .map_err(kernel_contract_error)?;
    let now = now_unix_ms();
    let result = {
        let mut store = state.store.write().map_err(|_| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            error: "internal_error",
            reason: "session_store_poisoned".to_string(),
        })?;
        store
            .kernel
            .register_compute_checkpoint_family_policy(
                &kernel_mutation_context(&session, now),
                request,
            )
            .map_err(kernel_api_error)?
    };
    record_kernel_mutation_observability(
        &state,
        &session,
        now,
        "kernel.compute.checkpoint_policy.registered",
        result.receipt_event.clone(),
        result.snapshot_event.clone(),
    );
    let response = compute_contracts::register_compute_checkpoint_family_policy_response_to_proto(
        &result.response,
    )
    .map_err(kernel_contract_error)?;
    Ok(Json(response))
}

async fn register_kernel_compute_validator_policy(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<proto_compute::RegisterComputeValidatorPolicyRequest>,
) -> Result<Json<proto_compute::RegisterComputeValidatorPolicyResponse>, ApiError> {
    let session = authenticate_session(&state, &headers)?;
    let request = compute_contracts::register_compute_validator_policy_request_from_proto(&request)
        .map_err(kernel_contract_error)?;
    let now = now_unix_ms();
    let result = {
        let mut store = state.store.write().map_err(|_| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            error: "internal_error",
            reason: "session_store_poisoned".to_string(),
        })?;
        store
            .kernel
            .register_compute_validator_policy(&kernel_mutation_context(&session, now), request)
            .map_err(kernel_api_error)?
    };
    record_kernel_mutation_observability(
        &state,
        &session,
        now,
        "kernel.compute.validator_policy.registered",
        result.receipt_event.clone(),
        result.snapshot_event.clone(),
    );
    let response =
        compute_contracts::register_compute_validator_policy_response_to_proto(&result.response)
            .map_err(kernel_contract_error)?;
    Ok(Json(response))
}

async fn register_kernel_compute_benchmark_package(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<proto_compute::RegisterComputeBenchmarkPackageRequest>,
) -> Result<Json<proto_compute::RegisterComputeBenchmarkPackageResponse>, ApiError> {
    let session = authenticate_session(&state, &headers)?;
    let request =
        compute_contracts::register_compute_benchmark_package_request_from_proto(&request)
            .map_err(kernel_contract_error)?;
    let now = now_unix_ms();
    let result = {
        let mut store = state.store.write().map_err(|_| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            error: "internal_error",
            reason: "session_store_poisoned".to_string(),
        })?;
        store
            .kernel
            .register_compute_benchmark_package(&kernel_mutation_context(&session, now), request)
            .map_err(kernel_api_error)?
    };
    record_kernel_mutation_observability(
        &state,
        &session,
        now,
        "kernel.compute.benchmark_package.registered",
        result.receipt_event.clone(),
        result.snapshot_event.clone(),
    );
    let response =
        compute_contracts::register_compute_benchmark_package_response_to_proto(&result.response)
            .map_err(kernel_contract_error)?;
    Ok(Json(response))
}

async fn register_kernel_compute_training_policy(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<proto_compute::RegisterComputeTrainingPolicyRequest>,
) -> Result<Json<proto_compute::RegisterComputeTrainingPolicyResponse>, ApiError> {
    let session = authenticate_session(&state, &headers)?;
    let request = compute_contracts::register_compute_training_policy_request_from_proto(&request)
        .map_err(kernel_contract_error)?;
    let now = now_unix_ms();
    let result = {
        let mut store = state.store.write().map_err(|_| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            error: "internal_error",
            reason: "session_store_poisoned".to_string(),
        })?;
        store
            .kernel
            .register_compute_training_policy(&kernel_mutation_context(&session, now), request)
            .map_err(kernel_api_error)?
    };
    record_kernel_mutation_observability(
        &state,
        &session,
        now,
        "kernel.compute.training_policy.registered",
        result.receipt_event.clone(),
        result.snapshot_event.clone(),
    );
    let response =
        compute_contracts::register_compute_training_policy_response_to_proto(&result.response)
            .map_err(kernel_contract_error)?;
    Ok(Json(response))
}

async fn create_kernel_compute_evaluation_run(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<proto_compute::CreateComputeEvaluationRunRequest>,
) -> Result<Json<proto_compute::CreateComputeEvaluationRunResponse>, ApiError> {
    let session = authenticate_session(&state, &headers)?;
    let request = compute_contracts::create_compute_evaluation_run_request_from_proto(&request)
        .map_err(kernel_contract_error)?;
    let now = now_unix_ms();
    let result = {
        let mut store = state.store.write().map_err(|_| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            error: "internal_error",
            reason: "session_store_poisoned".to_string(),
        })?;
        store
            .kernel
            .create_compute_evaluation_run(&kernel_mutation_context(&session, now), request)
            .map_err(kernel_api_error)?
    };
    record_kernel_mutation_observability(
        &state,
        &session,
        now,
        "kernel.compute.eval_run.created",
        result.receipt_event.clone(),
        result.snapshot_event.clone(),
    );
    let response =
        compute_contracts::create_compute_evaluation_run_response_to_proto(&result.response)
            .map_err(kernel_contract_error)?;
    Ok(Json(response))
}

async fn append_kernel_compute_evaluation_samples(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(eval_run_id): Path<String>,
    Json(mut request): Json<proto_compute::AppendComputeEvaluationSamplesRequest>,
) -> Result<Json<proto_compute::AppendComputeEvaluationSamplesResponse>, ApiError> {
    let session = authenticate_session(&state, &headers)?;
    let eval_run_id =
        normalize_required_field(eval_run_id.as_str(), "compute_eval_run_id_missing")?;
    if !request.eval_run_id.trim().is_empty() && request.eval_run_id != eval_run_id {
        return Err(ApiError {
            status: StatusCode::CONFLICT,
            error: "conflict",
            reason: "kernel_compute_eval_run_id_mismatch".to_string(),
        });
    }
    request.eval_run_id = eval_run_id;
    let request = compute_contracts::append_compute_evaluation_samples_request_from_proto(&request)
        .map_err(kernel_contract_error)?;
    let now = now_unix_ms();
    let result = {
        let mut store = state.store.write().map_err(|_| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            error: "internal_error",
            reason: "session_store_poisoned".to_string(),
        })?;
        store
            .kernel
            .append_compute_evaluation_samples(&kernel_mutation_context(&session, now), request)
            .map_err(kernel_api_error)?
    };
    record_kernel_mutation_observability(
        &state,
        &session,
        now,
        "kernel.compute.eval_run.samples_appended",
        result.receipt_event.clone(),
        result.snapshot_event.clone(),
    );
    let response =
        compute_contracts::append_compute_evaluation_samples_response_to_proto(&result.response)
            .map_err(kernel_contract_error)?;
    Ok(Json(response))
}

async fn finalize_kernel_compute_evaluation_run(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(eval_run_id): Path<String>,
    Json(mut request): Json<proto_compute::FinalizeComputeEvaluationRunRequest>,
) -> Result<Json<proto_compute::FinalizeComputeEvaluationRunResponse>, ApiError> {
    let session = authenticate_session(&state, &headers)?;
    let eval_run_id =
        normalize_required_field(eval_run_id.as_str(), "compute_eval_run_id_missing")?;
    if !request.eval_run_id.trim().is_empty() && request.eval_run_id != eval_run_id {
        return Err(ApiError {
            status: StatusCode::CONFLICT,
            error: "conflict",
            reason: "kernel_compute_eval_run_id_mismatch".to_string(),
        });
    }
    request.eval_run_id = eval_run_id;
    let request = compute_contracts::finalize_compute_evaluation_run_request_from_proto(&request)
        .map_err(kernel_contract_error)?;
    let now = now_unix_ms();
    let result = {
        let mut store = state.store.write().map_err(|_| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            error: "internal_error",
            reason: "session_store_poisoned".to_string(),
        })?;
        store
            .kernel
            .finalize_compute_evaluation_run(&kernel_mutation_context(&session, now), request)
            .map_err(kernel_api_error)?
    };
    record_kernel_mutation_observability(
        &state,
        &session,
        now,
        "kernel.compute.eval_run.finalized",
        result.receipt_event.clone(),
        result.snapshot_event.clone(),
    );
    let response =
        compute_contracts::finalize_compute_evaluation_run_response_to_proto(&result.response)
            .map_err(kernel_contract_error)?;
    Ok(Json(response))
}

async fn list_kernel_compute_training_runs(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<ComputeTrainingRunsQuery>,
) -> Result<Json<proto_compute::ListComputeTrainingRunsResponse>, ApiError> {
    let _session = authenticate_session(&state, &headers)?;
    let store = state.store.read().map_err(|_| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        error: "internal_error",
        reason: "session_store_poisoned".to_string(),
    })?;
    let response = compute_contracts::list_compute_training_runs_response_to_proto(
        store
            .kernel
            .list_compute_training_runs(
                query.training_policy_ref.as_deref(),
                query.environment_ref.as_deref(),
                query.status,
            )
            .as_slice(),
    )
    .map_err(kernel_contract_error)?;
    Ok(Json(response))
}

async fn get_kernel_compute_training_run(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(training_run_id): Path<String>,
) -> Result<Json<proto_compute::GetComputeTrainingRunResponse>, ApiError> {
    let _session = authenticate_session(&state, &headers)?;
    let training_run_id =
        normalize_required_field(training_run_id.as_str(), "compute_training_run_id_missing")?;
    let store = state.store.read().map_err(|_| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        error: "internal_error",
        reason: "session_store_poisoned".to_string(),
    })?;
    let Some(training_run) = store
        .kernel
        .get_compute_training_run(training_run_id.as_str())
    else {
        return Err(ApiError {
            status: StatusCode::NOT_FOUND,
            error: "not_found",
            reason: "kernel_compute_training_run_not_found".to_string(),
        });
    };
    let response = compute_contracts::get_compute_training_run_response_to_proto(&training_run)
        .map_err(kernel_contract_error)?;
    Ok(Json(response))
}

async fn create_kernel_compute_training_run(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<proto_compute::CreateComputeTrainingRunRequest>,
) -> Result<Json<proto_compute::CreateComputeTrainingRunResponse>, ApiError> {
    let session = authenticate_session(&state, &headers)?;
    let request = compute_contracts::create_compute_training_run_request_from_proto(&request)
        .map_err(kernel_contract_error)?;
    let now = now_unix_ms();
    let result = {
        let mut store = state.store.write().map_err(|_| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            error: "internal_error",
            reason: "session_store_poisoned".to_string(),
        })?;
        store
            .kernel
            .create_compute_training_run(&kernel_mutation_context(&session, now), request)
            .map_err(kernel_api_error)?
    };
    record_kernel_mutation_observability(
        &state,
        &session,
        now,
        "kernel.compute.training_run.created",
        result.receipt_event.clone(),
        result.snapshot_event.clone(),
    );
    let response =
        compute_contracts::create_compute_training_run_response_to_proto(&result.response)
            .map_err(kernel_contract_error)?;
    Ok(Json(response))
}

async fn finalize_kernel_compute_training_run(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(training_run_id): Path<String>,
    Json(mut request): Json<proto_compute::FinalizeComputeTrainingRunRequest>,
) -> Result<Json<proto_compute::FinalizeComputeTrainingRunResponse>, ApiError> {
    let session = authenticate_session(&state, &headers)?;
    let training_run_id =
        normalize_required_field(training_run_id.as_str(), "compute_training_run_id_missing")?;
    if !request.training_run_id.trim().is_empty() && request.training_run_id != training_run_id {
        return Err(ApiError {
            status: StatusCode::CONFLICT,
            error: "conflict",
            reason: "kernel_compute_training_run_id_mismatch".to_string(),
        });
    }
    request.training_run_id = training_run_id;
    let request = compute_contracts::finalize_compute_training_run_request_from_proto(&request)
        .map_err(kernel_contract_error)?;
    let now = now_unix_ms();
    let result = {
        let mut store = state.store.write().map_err(|_| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            error: "internal_error",
            reason: "session_store_poisoned".to_string(),
        })?;
        store
            .kernel
            .finalize_compute_training_run(&kernel_mutation_context(&session, now), request)
            .map_err(kernel_api_error)?
    };
    record_kernel_mutation_observability(
        &state,
        &session,
        now,
        "kernel.compute.training_run.finalized",
        result.receipt_event.clone(),
        result.snapshot_event.clone(),
    );
    let response =
        compute_contracts::finalize_compute_training_run_response_to_proto(&result.response)
            .map_err(kernel_contract_error)?;
    Ok(Json(response))
}

async fn list_kernel_compute_adapter_training_windows(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<ComputeAdapterTrainingWindowsQuery>,
) -> Result<Json<proto_compute::ListComputeAdapterTrainingWindowsResponse>, ApiError> {
    let _session = authenticate_session(&state, &headers)?;
    let store = state.store.read().map_err(|_| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        error: "internal_error",
        reason: "session_store_poisoned".to_string(),
    })?;
    let response = compute_contracts::list_compute_adapter_training_windows_response_to_proto(
        store
            .kernel
            .list_compute_adapter_training_windows(query.training_run_id.as_deref(), query.status)
            .as_slice(),
    )
    .map_err(kernel_contract_error)?;
    Ok(Json(response))
}

async fn get_kernel_compute_adapter_training_window(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(window_id): Path<String>,
) -> Result<Json<proto_compute::GetComputeAdapterTrainingWindowResponse>, ApiError> {
    let _session = authenticate_session(&state, &headers)?;
    let window_id =
        normalize_required_field(window_id.as_str(), "compute_adapter_window_id_missing")?;
    let store = state.store.read().map_err(|_| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        error: "internal_error",
        reason: "session_store_poisoned".to_string(),
    })?;
    let Some(window) = store
        .kernel
        .get_compute_adapter_training_window(window_id.as_str())
    else {
        return Err(ApiError {
            status: StatusCode::NOT_FOUND,
            error: "not_found",
            reason: "kernel_compute_adapter_window_not_found".to_string(),
        });
    };
    let response =
        compute_contracts::get_compute_adapter_training_window_response_to_proto(&window)
            .map_err(kernel_contract_error)?;
    Ok(Json(response))
}

async fn record_kernel_compute_adapter_window(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<proto_compute::RecordComputeAdapterWindowRequest>,
) -> Result<Json<proto_compute::RecordComputeAdapterWindowResponse>, ApiError> {
    let session = authenticate_session(&state, &headers)?;
    let request = compute_contracts::record_compute_adapter_window_request_from_proto(&request)
        .map_err(kernel_contract_error)?;
    let now = now_unix_ms();
    let result = {
        let mut store = state.store.write().map_err(|_| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            error: "internal_error",
            reason: "session_store_poisoned".to_string(),
        })?;
        store
            .kernel
            .record_compute_adapter_window(&kernel_mutation_context(&session, now), request)
            .map_err(kernel_api_error)?
    };
    record_kernel_mutation_observability(
        &state,
        &session,
        now,
        "kernel.compute.adapter_window.recorded",
        result.receipt_event.clone(),
        result.snapshot_event.clone(),
    );
    let response =
        compute_contracts::record_compute_adapter_window_response_to_proto(&result.response)
            .map_err(kernel_contract_error)?;
    Ok(Json(response))
}

async fn list_kernel_compute_adapter_contribution_outcomes(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<ComputeAdapterContributionOutcomesQuery>,
) -> Result<Json<proto_compute::ListComputeAdapterContributionOutcomesResponse>, ApiError> {
    let _session = authenticate_session(&state, &headers)?;
    let store = state.store.read().map_err(|_| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        error: "internal_error",
        reason: "session_store_poisoned".to_string(),
    })?;
    let response = compute_contracts::list_compute_adapter_contribution_outcomes_response_to_proto(
        store
            .kernel
            .list_compute_adapter_contribution_outcomes(
                query.training_run_id.as_deref(),
                query.window_id.as_deref(),
                query.disposition,
            )
            .as_slice(),
    )
    .map_err(kernel_contract_error)?;
    Ok(Json(response))
}

async fn get_kernel_compute_adapter_contribution_outcome(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(contribution_id): Path<String>,
) -> Result<Json<proto_compute::GetComputeAdapterContributionOutcomeResponse>, ApiError> {
    let _session = authenticate_session(&state, &headers)?;
    let contribution_id = normalize_required_field(
        contribution_id.as_str(),
        "compute_adapter_contribution_id_missing",
    )?;
    let store = state.store.read().map_err(|_| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        error: "internal_error",
        reason: "session_store_poisoned".to_string(),
    })?;
    let Some(contribution) = store
        .kernel
        .get_compute_adapter_contribution_outcome(contribution_id.as_str())
    else {
        return Err(ApiError {
            status: StatusCode::NOT_FOUND,
            error: "not_found",
            reason: "kernel_compute_adapter_contribution_not_found".to_string(),
        });
    };
    let response = compute_contracts::get_compute_adapter_contribution_outcome_response_to_proto(
        &contribution,
    )
    .map_err(kernel_contract_error)?;
    Ok(Json(response))
}

async fn list_kernel_compute_accepted_outcomes(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<ComputeAcceptedOutcomesQuery>,
) -> Result<Json<proto_compute::ListComputeAcceptedOutcomesResponse>, ApiError> {
    let _session = authenticate_session(&state, &headers)?;
    let store = state.store.read().map_err(|_| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        error: "internal_error",
        reason: "session_store_poisoned".to_string(),
    })?;
    let response = compute_contracts::list_compute_accepted_outcomes_response_to_proto(
        store
            .kernel
            .list_compute_accepted_outcomes(query.outcome_kind, query.environment_ref.as_deref())
            .as_slice(),
    )
    .map_err(kernel_contract_error)?;
    Ok(Json(response))
}

async fn get_kernel_compute_accepted_outcome(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(outcome_id): Path<String>,
) -> Result<Json<proto_compute::GetComputeAcceptedOutcomeResponse>, ApiError> {
    let _session = authenticate_session(&state, &headers)?;
    let outcome_id =
        normalize_required_field(outcome_id.as_str(), "compute_accepted_outcome_id_missing")?;
    let store = state.store.read().map_err(|_| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        error: "internal_error",
        reason: "session_store_poisoned".to_string(),
    })?;
    let Some(outcome) = store
        .kernel
        .get_compute_accepted_outcome(outcome_id.as_str())
    else {
        return Err(ApiError {
            status: StatusCode::NOT_FOUND,
            error: "not_found",
            reason: "kernel_compute_accepted_outcome_not_found".to_string(),
        });
    };
    let response = compute_contracts::get_compute_accepted_outcome_response_to_proto(&outcome)
        .map_err(kernel_contract_error)?;
    Ok(Json(response))
}

async fn accept_kernel_compute_outcome(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<proto_compute::AcceptComputeOutcomeRequest>,
) -> Result<Json<proto_compute::AcceptComputeOutcomeResponse>, ApiError> {
    let session = authenticate_session(&state, &headers)?;
    let request = compute_contracts::accept_compute_outcome_request_from_proto(&request)
        .map_err(kernel_contract_error)?;
    let now = now_unix_ms();
    let result = {
        let mut store = state.store.write().map_err(|_| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            error: "internal_error",
            reason: "session_store_poisoned".to_string(),
        })?;
        store
            .kernel
            .accept_compute_outcome(&kernel_mutation_context(&session, now), request)
            .map_err(kernel_api_error)?
    };
    record_kernel_mutation_observability(
        &state,
        &session,
        now,
        "kernel.compute.outcome.accepted",
        result.receipt_event.clone(),
        result.snapshot_event.clone(),
    );
    let response = compute_contracts::accept_compute_outcome_response_to_proto(&result.response)
        .map_err(kernel_contract_error)?;
    Ok(Json(response))
}

async fn create_kernel_compute_synthetic_data_job(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<proto_compute::CreateComputeSyntheticDataJobRequest>,
) -> Result<Json<proto_compute::CreateComputeSyntheticDataJobResponse>, ApiError> {
    let session = authenticate_session(&state, &headers)?;
    let request = compute_contracts::create_compute_synthetic_data_job_request_from_proto(&request)
        .map_err(kernel_contract_error)?;
    let now = now_unix_ms();
    let result = {
        let mut store = state.store.write().map_err(|_| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            error: "internal_error",
            reason: "session_store_poisoned".to_string(),
        })?;
        store
            .kernel
            .create_compute_synthetic_data_job(&kernel_mutation_context(&session, now), request)
            .map_err(kernel_api_error)?
    };
    record_kernel_mutation_observability(
        &state,
        &session,
        now,
        "kernel.compute.synthetic.created",
        result.receipt_event.clone(),
        result.snapshot_event.clone(),
    );
    let response =
        compute_contracts::create_compute_synthetic_data_job_response_to_proto(&result.response)
            .map_err(kernel_contract_error)?;
    Ok(Json(response))
}

async fn append_kernel_compute_synthetic_data_samples(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(synthetic_job_id): Path<String>,
    Json(mut request): Json<proto_compute::AppendComputeSyntheticDataSamplesRequest>,
) -> Result<Json<proto_compute::AppendComputeSyntheticDataSamplesResponse>, ApiError> {
    let session = authenticate_session(&state, &headers)?;
    let synthetic_job_id = normalize_required_field(
        synthetic_job_id.as_str(),
        "compute_synthetic_job_id_missing",
    )?;
    if !request.synthetic_job_id.trim().is_empty() && request.synthetic_job_id != synthetic_job_id {
        return Err(ApiError {
            status: StatusCode::CONFLICT,
            error: "conflict",
            reason: "kernel_compute_synthetic_job_id_mismatch".to_string(),
        });
    }
    request.synthetic_job_id = synthetic_job_id;
    let request =
        compute_contracts::append_compute_synthetic_data_samples_request_from_proto(&request)
            .map_err(kernel_contract_error)?;
    let now = now_unix_ms();
    let result = {
        let mut store = state.store.write().map_err(|_| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            error: "internal_error",
            reason: "session_store_poisoned".to_string(),
        })?;
        store
            .kernel
            .append_compute_synthetic_data_samples(&kernel_mutation_context(&session, now), request)
            .map_err(kernel_api_error)?
    };
    record_kernel_mutation_observability(
        &state,
        &session,
        now,
        "kernel.compute.synthetic.samples_appended",
        result.receipt_event.clone(),
        result.snapshot_event.clone(),
    );
    let response = compute_contracts::append_compute_synthetic_data_samples_response_to_proto(
        &result.response,
    )
    .map_err(kernel_contract_error)?;
    Ok(Json(response))
}

async fn finalize_kernel_compute_synthetic_data_generation(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(synthetic_job_id): Path<String>,
    Json(mut request): Json<proto_compute::FinalizeComputeSyntheticDataGenerationRequest>,
) -> Result<Json<proto_compute::FinalizeComputeSyntheticDataGenerationResponse>, ApiError> {
    let session = authenticate_session(&state, &headers)?;
    let synthetic_job_id = normalize_required_field(
        synthetic_job_id.as_str(),
        "compute_synthetic_job_id_missing",
    )?;
    if !request.synthetic_job_id.trim().is_empty() && request.synthetic_job_id != synthetic_job_id {
        return Err(ApiError {
            status: StatusCode::CONFLICT,
            error: "conflict",
            reason: "kernel_compute_synthetic_job_id_mismatch".to_string(),
        });
    }
    request.synthetic_job_id = synthetic_job_id;
    let request =
        compute_contracts::finalize_compute_synthetic_data_generation_request_from_proto(&request)
            .map_err(kernel_contract_error)?;
    let now = now_unix_ms();
    let result = {
        let mut store = state.store.write().map_err(|_| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            error: "internal_error",
            reason: "session_store_poisoned".to_string(),
        })?;
        store
            .kernel
            .finalize_compute_synthetic_data_generation(
                &kernel_mutation_context(&session, now),
                request,
            )
            .map_err(kernel_api_error)?
    };
    record_kernel_mutation_observability(
        &state,
        &session,
        now,
        "kernel.compute.synthetic.generation_finalized",
        result.receipt_event.clone(),
        result.snapshot_event.clone(),
    );
    let response = compute_contracts::finalize_compute_synthetic_data_generation_response_to_proto(
        &result.response,
    )
    .map_err(kernel_contract_error)?;
    Ok(Json(response))
}

async fn record_kernel_compute_synthetic_data_verification(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(synthetic_job_id): Path<String>,
    Json(mut request): Json<proto_compute::RecordComputeSyntheticDataVerificationRequest>,
) -> Result<Json<proto_compute::RecordComputeSyntheticDataVerificationResponse>, ApiError> {
    let session = authenticate_session(&state, &headers)?;
    let synthetic_job_id = normalize_required_field(
        synthetic_job_id.as_str(),
        "compute_synthetic_job_id_missing",
    )?;
    if !request.synthetic_job_id.trim().is_empty() && request.synthetic_job_id != synthetic_job_id {
        return Err(ApiError {
            status: StatusCode::CONFLICT,
            error: "conflict",
            reason: "kernel_compute_synthetic_job_id_mismatch".to_string(),
        });
    }
    request.synthetic_job_id = synthetic_job_id;
    let request =
        compute_contracts::record_compute_synthetic_data_verification_request_from_proto(&request)
            .map_err(kernel_contract_error)?;
    let now = now_unix_ms();
    let result = {
        let mut store = state.store.write().map_err(|_| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            error: "internal_error",
            reason: "session_store_poisoned".to_string(),
        })?;
        store
            .kernel
            .record_compute_synthetic_data_verification(
                &kernel_mutation_context(&session, now),
                request,
            )
            .map_err(kernel_api_error)?
    };
    record_kernel_mutation_observability(
        &state,
        &session,
        now,
        "kernel.compute.synthetic.verification_recorded",
        result.receipt_event.clone(),
        result.snapshot_event.clone(),
    );
    let response = compute_contracts::record_compute_synthetic_data_verification_response_to_proto(
        &result.response,
    )
    .map_err(kernel_contract_error)?;
    Ok(Json(response))
}

async fn create_kernel_capacity_lot(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<proto_compute::CreateCapacityLotRequest>,
) -> Result<Json<proto_compute::CreateCapacityLotResponse>, ApiError> {
    let session = authenticate_session(&state, &headers)?;
    let request = compute_contracts::create_capacity_lot_request_from_proto(&request)
        .map_err(kernel_contract_error)?;
    let now = now_unix_ms();
    let result = {
        let mut store = state.store.write().map_err(|_| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            error: "internal_error",
            reason: "session_store_poisoned".to_string(),
        })?;
        store
            .kernel
            .create_capacity_lot(&kernel_mutation_context(&session, now), request)
            .map_err(kernel_api_error)?
    };
    record_kernel_mutation_observability(
        &state,
        &session,
        now,
        "kernel.compute.lot.created",
        result.receipt_event.clone(),
        result.snapshot_event.clone(),
    );
    let response = compute_contracts::create_capacity_lot_response_to_proto(&result.response)
        .map_err(kernel_contract_error)?;
    Ok(Json(response))
}

async fn create_kernel_capacity_instrument(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<proto_compute::CreateCapacityInstrumentRequest>,
) -> Result<Json<proto_compute::CreateCapacityInstrumentResponse>, ApiError> {
    let session = authenticate_session(&state, &headers)?;
    let request = compute_contracts::create_capacity_instrument_request_from_proto(&request)
        .map_err(kernel_contract_error)?;
    let now = now_unix_ms();
    let result = {
        let mut store = state.store.write().map_err(|_| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            error: "internal_error",
            reason: "session_store_poisoned".to_string(),
        })?;
        store
            .kernel
            .create_capacity_instrument(&kernel_mutation_context(&session, now), request)
            .map_err(kernel_api_error)?
    };
    record_kernel_mutation_observability(
        &state,
        &session,
        now,
        "kernel.compute.instrument.created",
        result.receipt_event.clone(),
        result.snapshot_event.clone(),
    );
    let response =
        compute_contracts::create_capacity_instrument_response_to_proto(&result.response)
            .map_err(kernel_contract_error)?;
    Ok(Json(response))
}

async fn close_kernel_capacity_instrument(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(instrument_id): Path<String>,
    Json(request): Json<proto_compute::CloseCapacityInstrumentRequest>,
) -> Result<Json<proto_compute::CloseCapacityInstrumentResponse>, ApiError> {
    let session = authenticate_session(&state, &headers)?;
    let mut request = compute_contracts::close_capacity_instrument_request_from_proto(&request)
        .map_err(kernel_contract_error)?;
    let instrument_id = normalize_required_field(instrument_id.as_str(), "instrument_id_missing")?;
    if !request.instrument_id.trim().is_empty() && request.instrument_id != instrument_id {
        return Err(ApiError {
            status: StatusCode::CONFLICT,
            error: "conflict",
            reason: "kernel_capacity_instrument_id_mismatch".to_string(),
        });
    }
    request.instrument_id = instrument_id;
    let now = now_unix_ms();
    let result = {
        let mut store = state.store.write().map_err(|_| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            error: "internal_error",
            reason: "session_store_poisoned".to_string(),
        })?;
        store
            .kernel
            .close_capacity_instrument(&kernel_mutation_context(&session, now), request)
            .map_err(kernel_api_error)?
    };
    record_kernel_mutation_observability(
        &state,
        &session,
        now,
        "kernel.compute.instrument.closed",
        result.receipt_event.clone(),
        result.snapshot_event.clone(),
    );
    let response = compute_contracts::close_capacity_instrument_response_to_proto(&result.response)
        .map_err(kernel_contract_error)?;
    Ok(Json(response))
}

async fn cash_settle_kernel_capacity_instrument(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(instrument_id): Path<String>,
    Json(request): Json<proto_compute::CashSettleCapacityInstrumentRequest>,
) -> Result<Json<proto_compute::CashSettleCapacityInstrumentResponse>, ApiError> {
    let session = authenticate_session(&state, &headers)?;
    let mut request =
        compute_contracts::cash_settle_capacity_instrument_request_from_proto(&request)
            .map_err(kernel_contract_error)?;
    let instrument_id = normalize_required_field(instrument_id.as_str(), "instrument_id_missing")?;
    if !request.instrument_id.trim().is_empty() && request.instrument_id != instrument_id {
        return Err(ApiError {
            status: StatusCode::CONFLICT,
            error: "conflict",
            reason: "kernel_capacity_instrument_id_mismatch".to_string(),
        });
    }
    request.instrument_id = instrument_id;
    let now = now_unix_ms();
    let result = {
        let mut store = state.store.write().map_err(|_| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            error: "internal_error",
            reason: "session_store_poisoned".to_string(),
        })?;
        store
            .kernel
            .cash_settle_capacity_instrument(&kernel_mutation_context(&session, now), request)
            .map_err(kernel_api_error)?
    };
    record_kernel_mutation_observability(
        &state,
        &session,
        now,
        "kernel.compute.instrument.cash_settled",
        result.receipt_event.clone(),
        result.snapshot_event.clone(),
    );
    let response =
        compute_contracts::cash_settle_capacity_instrument_response_to_proto(&result.response)
            .map_err(kernel_contract_error)?;
    Ok(Json(response))
}

async fn create_kernel_structured_capacity_instrument(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<proto_compute::CreateStructuredCapacityInstrumentRequest>,
) -> Result<Json<proto_compute::CreateStructuredCapacityInstrumentResponse>, ApiError> {
    let session = authenticate_session(&state, &headers)?;
    let request =
        compute_contracts::create_structured_capacity_instrument_request_from_proto(&request)
            .map_err(kernel_contract_error)?;
    let now = now_unix_ms();
    let result = {
        let mut store = state.store.write().map_err(|_| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            error: "internal_error",
            reason: "session_store_poisoned".to_string(),
        })?;
        store
            .kernel
            .create_structured_capacity_instrument(&kernel_mutation_context(&session, now), request)
            .map_err(kernel_api_error)?
    };
    record_kernel_mutation_observability(
        &state,
        &session,
        now,
        "kernel.compute.structured_instrument.created",
        result.receipt_event.clone(),
        result.snapshot_event.clone(),
    );
    let response = compute_contracts::create_structured_capacity_instrument_response_to_proto(
        &result.response,
    )
    .map_err(kernel_contract_error)?;
    Ok(Json(response))
}

async fn close_kernel_structured_capacity_instrument(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(structured_instrument_id): Path<String>,
    Json(request): Json<proto_compute::CloseStructuredCapacityInstrumentRequest>,
) -> Result<Json<proto_compute::CloseStructuredCapacityInstrumentResponse>, ApiError> {
    let session = authenticate_session(&state, &headers)?;
    let mut request =
        compute_contracts::close_structured_capacity_instrument_request_from_proto(&request)
            .map_err(kernel_contract_error)?;
    let structured_instrument_id = normalize_required_field(
        structured_instrument_id.as_str(),
        "structured_capacity_instrument_id_missing",
    )?;
    if !request.structured_instrument_id.trim().is_empty()
        && request.structured_instrument_id != structured_instrument_id
    {
        return Err(ApiError {
            status: StatusCode::CONFLICT,
            error: "conflict",
            reason: "kernel_structured_capacity_instrument_id_mismatch".to_string(),
        });
    }
    request.structured_instrument_id = structured_instrument_id;
    let now = now_unix_ms();
    let result = {
        let mut store = state.store.write().map_err(|_| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            error: "internal_error",
            reason: "session_store_poisoned".to_string(),
        })?;
        store
            .kernel
            .close_structured_capacity_instrument(&kernel_mutation_context(&session, now), request)
            .map_err(kernel_api_error)?
    };
    record_kernel_mutation_observability(
        &state,
        &session,
        now,
        "kernel.compute.structured_instrument.closed",
        result.receipt_event.clone(),
        result.snapshot_event.clone(),
    );
    let response =
        compute_contracts::close_structured_capacity_instrument_response_to_proto(&result.response)
            .map_err(kernel_contract_error)?;
    Ok(Json(response))
}

async fn record_kernel_delivery_proof(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(lot_id): Path<String>,
    Json(request): Json<proto_compute::RecordDeliveryProofRequest>,
) -> Result<Json<proto_compute::RecordDeliveryProofResponse>, ApiError> {
    let session = authenticate_session(&state, &headers)?;
    let mut request = compute_contracts::record_delivery_proof_request_from_proto(&request)
        .map_err(kernel_contract_error)?;
    let lot_id = normalize_required_field(lot_id.as_str(), "capacity_lot_id_missing")?;
    if !request.delivery_proof.capacity_lot_id.trim().is_empty()
        && request.delivery_proof.capacity_lot_id != lot_id
    {
        return Err(ApiError {
            status: StatusCode::CONFLICT,
            error: "conflict",
            reason: "kernel_capacity_lot_id_mismatch".to_string(),
        });
    }
    request.delivery_proof.capacity_lot_id = lot_id;
    let now = now_unix_ms();
    let result = {
        let mut store = state.store.write().map_err(|_| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            error: "internal_error",
            reason: "session_store_poisoned".to_string(),
        })?;
        store
            .kernel
            .record_delivery_proof(&kernel_mutation_context(&session, now), request)
            .map_err(kernel_api_error)?
    };
    record_kernel_mutation_observability(
        &state,
        &session,
        now,
        "kernel.compute.delivery.recorded",
        result.receipt_event.clone(),
        result.snapshot_event.clone(),
    );
    let response = compute_contracts::record_delivery_proof_response_to_proto(&result.response)
        .map_err(kernel_contract_error)?;
    Ok(Json(response))
}

async fn publish_kernel_compute_index(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<proto_compute::PublishComputeIndexRequest>,
) -> Result<Json<proto_compute::PublishComputeIndexResponse>, ApiError> {
    let session = authenticate_session(&state, &headers)?;
    let request = compute_contracts::publish_compute_index_request_from_proto(&request)
        .map_err(kernel_contract_error)?;
    let now = now_unix_ms();
    let result = {
        let mut store = state.store.write().map_err(|_| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            error: "internal_error",
            reason: "session_store_poisoned".to_string(),
        })?;
        store
            .kernel
            .publish_compute_index(&kernel_mutation_context(&session, now), request)
            .map_err(kernel_api_error)?
    };
    record_kernel_mutation_observability(
        &state,
        &session,
        now,
        "kernel.compute.index.published",
        result.receipt_event.clone(),
        result.snapshot_event.clone(),
    );
    let response = compute_contracts::publish_compute_index_response_to_proto(&result.response)
        .map_err(kernel_contract_error)?;
    Ok(Json(response))
}

async fn correct_kernel_compute_index(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(index_id): Path<String>,
    Json(request): Json<proto_compute::CorrectComputeIndexRequest>,
) -> Result<Json<proto_compute::CorrectComputeIndexResponse>, ApiError> {
    let session = authenticate_session(&state, &headers)?;
    let mut request = compute_contracts::correct_compute_index_request_from_proto(&request)
        .map_err(kernel_contract_error)?;
    request.superseded_index_id =
        normalize_required_field(index_id.as_str(), "compute_index_id_missing")?;
    let now = now_unix_ms();
    let result = {
        let mut store = state.store.write().map_err(|_| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            error: "internal_error",
            reason: "session_store_poisoned".to_string(),
        })?;
        store
            .kernel
            .correct_compute_index(&kernel_mutation_context(&session, now), request)
            .map_err(kernel_api_error)?
    };
    record_kernel_mutation_observability(
        &state,
        &session,
        now,
        "kernel.compute.index.corrected",
        result.receipt_event.clone(),
        result.snapshot_event.clone(),
    );
    let response = compute_contracts::correct_compute_index_response_to_proto(&result.response)
        .map_err(kernel_contract_error)?;
    Ok(Json(response))
}

async fn list_kernel_data_assets(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<DataAssetsQuery>,
) -> Result<Json<Vec<DataAsset>>, ApiError> {
    let _session = authenticate_session(&state, &headers)?;
    let store = state.store.read().map_err(|_| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        error: "internal_error",
        reason: "session_store_poisoned".to_string(),
    })?;
    Ok(Json(store.kernel.list_data_assets(
        query.provider_id.as_deref(),
        query.asset_kind.as_deref(),
        query.status,
    )))
}

async fn get_kernel_data_asset(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(asset_id): Path<String>,
) -> Result<Json<DataAsset>, ApiError> {
    let _session = authenticate_session(&state, &headers)?;
    let asset_id = normalize_required_field(asset_id.as_str(), "asset_id_missing")?;
    let store = state.store.read().map_err(|_| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        error: "internal_error",
        reason: "session_store_poisoned".to_string(),
    })?;
    let Some(asset) = store.kernel.get_data_asset(asset_id.as_str()) else {
        return Err(ApiError {
            status: StatusCode::NOT_FOUND,
            error: "not_found",
            reason: "kernel_data_asset_not_found".to_string(),
        });
    };
    Ok(Json(asset))
}

async fn list_kernel_access_grants(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<AccessGrantsQuery>,
) -> Result<Json<Vec<AccessGrant>>, ApiError> {
    let _session = authenticate_session(&state, &headers)?;
    let store = state.store.read().map_err(|_| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        error: "internal_error",
        reason: "session_store_poisoned".to_string(),
    })?;
    Ok(Json(store.kernel.list_access_grants(
        query.asset_id.as_deref(),
        query.provider_id.as_deref(),
        query.consumer_id.as_deref(),
        query.status,
    )))
}

async fn get_kernel_access_grant(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(grant_id): Path<String>,
) -> Result<Json<AccessGrant>, ApiError> {
    let _session = authenticate_session(&state, &headers)?;
    let grant_id = normalize_required_field(grant_id.as_str(), "access_grant_id_missing")?;
    let store = state.store.read().map_err(|_| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        error: "internal_error",
        reason: "session_store_poisoned".to_string(),
    })?;
    let Some(grant) = store.kernel.get_access_grant(grant_id.as_str()) else {
        return Err(ApiError {
            status: StatusCode::NOT_FOUND,
            error: "not_found",
            reason: "kernel_access_grant_not_found".to_string(),
        });
    };
    Ok(Json(grant))
}

async fn list_kernel_delivery_bundles(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<DeliveryBundlesQuery>,
) -> Result<Json<Vec<DeliveryBundle>>, ApiError> {
    let _session = authenticate_session(&state, &headers)?;
    let store = state.store.read().map_err(|_| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        error: "internal_error",
        reason: "session_store_poisoned".to_string(),
    })?;
    Ok(Json(store.kernel.list_delivery_bundles(
        query.asset_id.as_deref(),
        query.grant_id.as_deref(),
        query.provider_id.as_deref(),
        query.consumer_id.as_deref(),
        query.status,
    )))
}

async fn get_kernel_delivery_bundle(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(delivery_bundle_id): Path<String>,
) -> Result<Json<DeliveryBundle>, ApiError> {
    let _session = authenticate_session(&state, &headers)?;
    let delivery_bundle_id =
        normalize_required_field(delivery_bundle_id.as_str(), "delivery_bundle_id_missing")?;
    let store = state.store.read().map_err(|_| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        error: "internal_error",
        reason: "session_store_poisoned".to_string(),
    })?;
    let Some(bundle) = store
        .kernel
        .get_delivery_bundle(delivery_bundle_id.as_str())
    else {
        return Err(ApiError {
            status: StatusCode::NOT_FOUND,
            error: "not_found",
            reason: "kernel_delivery_bundle_not_found".to_string(),
        });
    };
    Ok(Json(bundle))
}

async fn list_kernel_revocations(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<RevocationsQuery>,
) -> Result<Json<Vec<RevocationReceipt>>, ApiError> {
    let _session = authenticate_session(&state, &headers)?;
    let store = state.store.read().map_err(|_| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        error: "internal_error",
        reason: "session_store_poisoned".to_string(),
    })?;
    Ok(Json(store.kernel.list_revocations(
        query.asset_id.as_deref(),
        query.grant_id.as_deref(),
        query.provider_id.as_deref(),
        query.consumer_id.as_deref(),
        query.status,
    )))
}

async fn get_kernel_revocation(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(revocation_id): Path<String>,
) -> Result<Json<RevocationReceipt>, ApiError> {
    let _session = authenticate_session(&state, &headers)?;
    let revocation_id = normalize_required_field(revocation_id.as_str(), "revocation_id_missing")?;
    let store = state.store.read().map_err(|_| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        error: "internal_error",
        reason: "session_store_poisoned".to_string(),
    })?;
    let Some(revocation) = store.kernel.get_revocation(revocation_id.as_str()) else {
        return Err(ApiError {
            status: StatusCode::NOT_FOUND,
            error: "not_found",
            reason: "kernel_revocation_not_found".to_string(),
        });
    };
    Ok(Json(revocation))
}

async fn register_kernel_data_asset(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<RegisterDataAssetRequest>,
) -> Result<Json<RegisterDataAssetResponse>, ApiError> {
    let session = authenticate_session(&state, &headers)?;
    let now = now_unix_ms();
    let result = {
        let mut store = state.store.write().map_err(|_| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            error: "internal_error",
            reason: "session_store_poisoned".to_string(),
        })?;
        store
            .kernel
            .register_data_asset(&kernel_mutation_context(&session, now), request)
            .map_err(kernel_api_error)?
    };
    record_kernel_mutation_observability(
        &state,
        &session,
        now,
        "kernel.data.asset.registered",
        result.receipt_event.clone(),
        result.snapshot_event.clone(),
    );
    Ok(Json(result.response))
}

async fn create_kernel_access_grant(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<CreateAccessGrantRequest>,
) -> Result<Json<CreateAccessGrantResponse>, ApiError> {
    let session = authenticate_session(&state, &headers)?;
    let now = now_unix_ms();
    let result = {
        let mut store = state.store.write().map_err(|_| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            error: "internal_error",
            reason: "session_store_poisoned".to_string(),
        })?;
        store
            .kernel
            .create_access_grant(&kernel_mutation_context(&session, now), request)
            .map_err(kernel_api_error)?
    };
    record_kernel_mutation_observability(
        &state,
        &session,
        now,
        "kernel.data.grant.offered",
        result.receipt_event.clone(),
        result.snapshot_event.clone(),
    );
    Ok(Json(result.response))
}

async fn accept_kernel_access_grant(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(grant_id): Path<String>,
    Json(mut request): Json<AcceptAccessGrantRequest>,
) -> Result<Json<AcceptAccessGrantResponse>, ApiError> {
    let session = authenticate_session(&state, &headers)?;
    let grant_id = normalize_required_field(grant_id.as_str(), "access_grant_id_missing")?;
    if !request.grant_id.trim().is_empty() && request.grant_id != grant_id {
        return Err(ApiError {
            status: StatusCode::CONFLICT,
            error: "conflict",
            reason: "kernel_access_grant_id_mismatch".to_string(),
        });
    }
    request.grant_id = grant_id;
    let now = now_unix_ms();
    let result = {
        let mut store = state.store.write().map_err(|_| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            error: "internal_error",
            reason: "session_store_poisoned".to_string(),
        })?;
        store
            .kernel
            .accept_access_grant(&kernel_mutation_context(&session, now), request)
            .map_err(kernel_api_error)?
    };
    record_kernel_mutation_observability(
        &state,
        &session,
        now,
        "kernel.data.grant.accepted",
        result.receipt_event.clone(),
        result.snapshot_event.clone(),
    );
    Ok(Json(result.response))
}

async fn issue_kernel_delivery_bundle(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(grant_id): Path<String>,
    Json(mut request): Json<IssueDeliveryBundleRequest>,
) -> Result<Json<IssueDeliveryBundleResponse>, ApiError> {
    let session = authenticate_session(&state, &headers)?;
    let grant_id = normalize_required_field(grant_id.as_str(), "access_grant_id_missing")?;
    if !request.delivery_bundle.grant_id.trim().is_empty()
        && request.delivery_bundle.grant_id != grant_id
    {
        return Err(ApiError {
            status: StatusCode::CONFLICT,
            error: "conflict",
            reason: "kernel_access_grant_id_mismatch".to_string(),
        });
    }
    request.delivery_bundle.grant_id = grant_id;
    let now = now_unix_ms();
    let result = {
        let mut store = state.store.write().map_err(|_| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            error: "internal_error",
            reason: "session_store_poisoned".to_string(),
        })?;
        store
            .kernel
            .issue_delivery_bundle(&kernel_mutation_context(&session, now), request)
            .map_err(kernel_api_error)?
    };
    record_kernel_mutation_observability(
        &state,
        &session,
        now,
        "kernel.data.delivery.issued",
        result.receipt_event.clone(),
        result.snapshot_event.clone(),
    );
    Ok(Json(result.response))
}

async fn revoke_kernel_access_grant(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(grant_id): Path<String>,
    Json(mut request): Json<RevokeAccessGrantRequest>,
) -> Result<Json<RevokeAccessGrantResponse>, ApiError> {
    let session = authenticate_session(&state, &headers)?;
    let grant_id = normalize_required_field(grant_id.as_str(), "access_grant_id_missing")?;
    if !request.revocation.grant_id.trim().is_empty() && request.revocation.grant_id != grant_id {
        return Err(ApiError {
            status: StatusCode::CONFLICT,
            error: "conflict",
            reason: "kernel_access_grant_id_mismatch".to_string(),
        });
    }
    request.revocation.grant_id = grant_id;
    let now = now_unix_ms();
    let result = {
        let mut store = state.store.write().map_err(|_| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            error: "internal_error",
            reason: "session_store_poisoned".to_string(),
        })?;
        store
            .kernel
            .revoke_access_grant(&kernel_mutation_context(&session, now), request)
            .map_err(kernel_api_error)?
    };
    record_kernel_mutation_observability(
        &state,
        &session,
        now,
        "kernel.data.revocation.recorded",
        result.receipt_event.clone(),
        result.snapshot_event.clone(),
    );
    Ok(Json(result.response))
}

async fn create_kernel_liquidity_quote(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<CreateLiquidityQuoteRequest>,
) -> Result<Json<CreateLiquidityQuoteResponse>, ApiError> {
    let session = authenticate_session(&state, &headers)?;
    let now = now_unix_ms();
    let result = {
        let mut store = state.store.write().map_err(|_| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            error: "internal_error",
            reason: "session_store_poisoned".to_string(),
        })?;
        store
            .kernel
            .create_liquidity_quote(&kernel_mutation_context(&session, now), request)
            .map_err(kernel_api_error)?
    };
    record_kernel_mutation_observability(
        &state,
        &session,
        now,
        "kernel.liquidity.quote.created",
        result.receipt_event.clone(),
        result.snapshot_event.clone(),
    );
    Ok(Json(result.response))
}

async fn select_kernel_route_plan(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<SelectRoutePlanRequest>,
) -> Result<Json<SelectRoutePlanResponse>, ApiError> {
    let session = authenticate_session(&state, &headers)?;
    let now = now_unix_ms();
    let result = {
        let mut store = state.store.write().map_err(|_| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            error: "internal_error",
            reason: "session_store_poisoned".to_string(),
        })?;
        store
            .kernel
            .select_route_plan(&kernel_mutation_context(&session, now), request)
            .map_err(kernel_api_error)?
    };
    record_kernel_mutation_observability(
        &state,
        &session,
        now,
        "kernel.liquidity.route.selected",
        result.receipt_event.clone(),
        result.snapshot_event.clone(),
    );
    Ok(Json(result.response))
}

async fn issue_kernel_liquidity_envelope(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<IssueLiquidityEnvelopeRequest>,
) -> Result<Json<IssueLiquidityEnvelopeResponse>, ApiError> {
    let session = authenticate_session(&state, &headers)?;
    let now = now_unix_ms();
    let result = {
        let mut store = state.store.write().map_err(|_| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            error: "internal_error",
            reason: "session_store_poisoned".to_string(),
        })?;
        store
            .kernel
            .issue_liquidity_envelope(&kernel_mutation_context(&session, now), request)
            .map_err(kernel_api_error)?
    };
    record_kernel_mutation_observability(
        &state,
        &session,
        now,
        "kernel.liquidity.envelope.issued",
        result.receipt_event.clone(),
        result.snapshot_event.clone(),
    );
    Ok(Json(result.response))
}

async fn execute_kernel_settlement_intent(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<ExecuteSettlementIntentRequest>,
) -> Result<Json<ExecuteSettlementIntentResponse>, ApiError> {
    let session = authenticate_session(&state, &headers)?;
    let now = now_unix_ms();
    let result = {
        let mut store = state.store.write().map_err(|_| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            error: "internal_error",
            reason: "session_store_poisoned".to_string(),
        })?;
        store
            .kernel
            .execute_settlement_intent(&kernel_mutation_context(&session, now), request)
            .map_err(kernel_api_error)?
    };
    record_kernel_mutation_observability(
        &state,
        &session,
        now,
        "kernel.liquidity.settlement.executed",
        result.receipt_event.clone(),
        result.snapshot_event.clone(),
    );
    Ok(Json(result.response))
}

async fn register_kernel_reserve_partition(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<RegisterReservePartitionRequest>,
) -> Result<Json<RegisterReservePartitionResponse>, ApiError> {
    let session = authenticate_session(&state, &headers)?;
    let now = now_unix_ms();
    let result = {
        let mut store = state.store.write().map_err(|_| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            error: "internal_error",
            reason: "session_store_poisoned".to_string(),
        })?;
        store
            .kernel
            .register_reserve_partition(&kernel_mutation_context(&session, now), request)
            .map_err(kernel_api_error)?
    };
    record_kernel_mutation_observability(
        &state,
        &session,
        now,
        "kernel.liquidity.reserve_partition.registered",
        result.receipt_event.clone(),
        result.snapshot_event.clone(),
    );
    Ok(Json(result.response))
}

async fn adjust_kernel_reserve_partition(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(partition_id): Path<String>,
    Json(mut request): Json<AdjustReservePartitionRequest>,
) -> Result<Json<AdjustReservePartitionResponse>, ApiError> {
    let session = authenticate_session(&state, &headers)?;
    let partition_id =
        normalize_required_field(partition_id.as_str(), "reserve_partition_id_missing")?;
    if !request.partition_id.trim().is_empty() && request.partition_id != partition_id {
        return Err(ApiError {
            status: StatusCode::CONFLICT,
            error: "conflict",
            reason: "kernel_reserve_partition_id_mismatch".to_string(),
        });
    }
    request.partition_id = partition_id;
    let now = now_unix_ms();
    let result = {
        let mut store = state.store.write().map_err(|_| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            error: "internal_error",
            reason: "session_store_poisoned".to_string(),
        })?;
        store
            .kernel
            .adjust_reserve_partition(&kernel_mutation_context(&session, now), request)
            .map_err(kernel_api_error)?
    };
    record_kernel_mutation_observability(
        &state,
        &session,
        now,
        "kernel.liquidity.reserve_partition.adjusted",
        result.receipt_event.clone(),
        result.snapshot_event.clone(),
    );
    Ok(Json(result.response))
}

async fn place_kernel_coverage_offer(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<PlaceCoverageOfferRequest>,
) -> Result<Json<PlaceCoverageOfferResponse>, ApiError> {
    let session = authenticate_session(&state, &headers)?;
    let now = now_unix_ms();
    let result = {
        let mut store = state.store.write().map_err(|_| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            error: "internal_error",
            reason: "session_store_poisoned".to_string(),
        })?;
        store
            .kernel
            .place_coverage_offer(&kernel_mutation_context(&session, now), request)
            .map_err(kernel_api_error)?
    };
    record_kernel_mutation_observability(
        &state,
        &session,
        now,
        "kernel.risk.coverage_offer.placed",
        result.receipt_event.clone(),
        result.snapshot_event.clone(),
    );
    Ok(Json(result.response))
}

async fn bind_kernel_coverage(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<BindCoverageRequest>,
) -> Result<Json<BindCoverageResponse>, ApiError> {
    let session = authenticate_session(&state, &headers)?;
    let now = now_unix_ms();
    let result = {
        let mut store = state.store.write().map_err(|_| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            error: "internal_error",
            reason: "session_store_poisoned".to_string(),
        })?;
        store
            .kernel
            .bind_coverage(&kernel_mutation_context(&session, now), request)
            .map_err(kernel_api_error)?
    };
    record_kernel_mutation_observability(
        &state,
        &session,
        now,
        "kernel.risk.coverage_binding.bound",
        result.receipt_event.clone(),
        result.snapshot_event.clone(),
    );
    Ok(Json(result.response))
}

async fn create_kernel_prediction_position(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<CreatePredictionPositionRequest>,
) -> Result<Json<CreatePredictionPositionResponse>, ApiError> {
    let session = authenticate_session(&state, &headers)?;
    let now = now_unix_ms();
    let result = {
        let mut store = state.store.write().map_err(|_| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            error: "internal_error",
            reason: "session_store_poisoned".to_string(),
        })?;
        store
            .kernel
            .create_prediction_position(&kernel_mutation_context(&session, now), request)
            .map_err(kernel_api_error)?
    };
    record_kernel_mutation_observability(
        &state,
        &session,
        now,
        "kernel.risk.position.created",
        result.receipt_event.clone(),
        result.snapshot_event.clone(),
    );
    Ok(Json(result.response))
}

async fn create_kernel_risk_claim(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<CreateRiskClaimRequest>,
) -> Result<Json<CreateRiskClaimResponse>, ApiError> {
    let session = authenticate_session(&state, &headers)?;
    let now = now_unix_ms();
    let result = {
        let mut store = state.store.write().map_err(|_| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            error: "internal_error",
            reason: "session_store_poisoned".to_string(),
        })?;
        store
            .kernel
            .create_risk_claim(&kernel_mutation_context(&session, now), request)
            .map_err(kernel_api_error)?
    };
    record_kernel_mutation_observability(
        &state,
        &session,
        now,
        "kernel.risk.claim.created",
        result.receipt_event.clone(),
        result.snapshot_event.clone(),
    );
    Ok(Json(result.response))
}

async fn resolve_kernel_risk_claim(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(claim_id): Path<String>,
    Json(mut request): Json<ResolveRiskClaimRequest>,
) -> Result<Json<ResolveRiskClaimResponse>, ApiError> {
    let session = authenticate_session(&state, &headers)?;
    let claim_id = normalize_required_field(claim_id.as_str(), "risk_claim_id_missing")?;
    if !request.claim_id.trim().is_empty() && request.claim_id != claim_id {
        return Err(ApiError {
            status: StatusCode::CONFLICT,
            error: "conflict",
            reason: "kernel_risk_claim_id_mismatch".to_string(),
        });
    }
    request.claim_id = claim_id;
    let now = now_unix_ms();
    let result = {
        let mut store = state.store.write().map_err(|_| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            error: "internal_error",
            reason: "session_store_poisoned".to_string(),
        })?;
        store
            .kernel
            .resolve_risk_claim(&kernel_mutation_context(&session, now), request)
            .map_err(kernel_api_error)?
    };
    record_kernel_mutation_observability(
        &state,
        &session,
        now,
        "kernel.risk.claim.resolved",
        result.receipt_event.clone(),
        result.snapshot_event.clone(),
    );
    Ok(Json(result.response))
}

async fn publish_kernel_risk_signal(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<PublishRiskSignalRequest>,
) -> Result<Json<PublishRiskSignalResponse>, ApiError> {
    let session = authenticate_session(&state, &headers)?;
    let now = now_unix_ms();
    let result = {
        let mut store = state.store.write().map_err(|_| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            error: "internal_error",
            reason: "session_store_poisoned".to_string(),
        })?;
        store
            .kernel
            .publish_risk_signal(&kernel_mutation_context(&session, now), request)
            .map_err(kernel_api_error)?
    };
    record_kernel_mutation_observability(
        &state,
        &session,
        now,
        "kernel.risk.signal.published",
        result.receipt_event.clone(),
        result.snapshot_event.clone(),
    );
    Ok(Json(result.response))
}

async fn get_kernel_snapshot(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(minute_start_ms): Path<i64>,
) -> Result<Json<EconomySnapshot>, ApiError> {
    let _session = authenticate_session(&state, &headers)?;
    let snapshot = {
        let mut store = state.store.write().map_err(|_| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            error: "internal_error",
            reason: "session_store_poisoned".to_string(),
        })?;
        store
            .kernel
            .get_snapshot(minute_start_ms)
            .map_err(kernel_api_error)?
    };
    Ok(Json(snapshot))
}

async fn get_kernel_receipt(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(receipt_id): Path<String>,
) -> Result<Json<Receipt>, ApiError> {
    let _session = authenticate_session(&state, &headers)?;
    let receipt_id = normalize_required_field(receipt_id.as_str(), "receipt_id_missing")?;
    let receipt = {
        let store = state.store.read().map_err(|_| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            error: "internal_error",
            reason: "session_store_poisoned".to_string(),
        })?;
        store.kernel.get_receipt(receipt_id.as_str())
    }
    .ok_or_else(|| ApiError {
        status: StatusCode::NOT_FOUND,
        error: "not_found",
        reason: "kernel_receipt_not_found".to_string(),
    })?;
    Ok(Json(receipt))
}

async fn stream_kernel_receipts(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>>, ApiError> {
    let _session = authenticate_session(&state, &headers)?;
    let stream =
        BroadcastStream::new(state.kernel_receipt_tx.subscribe()).filter_map(
            |message| match message {
                Ok(event) => {
                    let data = serde_json::to_string(&event).ok()?;
                    Some(Ok(Event::default().event("receipt").data(data)))
                }
                Err(_) => None,
            },
        );
    Ok(Sse::new(stream).keep_alive(KeepAlive::new().interval(Duration::from_secs(15))))
}

async fn stream_kernel_snapshots(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>>, ApiError> {
    let _session = authenticate_session(&state, &headers)?;
    let stream = BroadcastStream::new(state.kernel_snapshot_tx.subscribe()).filter_map(|message| {
        match message {
            Ok(event) => {
                let data = serde_json::to_string(&event).ok()?;
                Some(Ok(Event::default().event("snapshot").data(data)))
            }
            Err(_) => None,
        }
    });
    Ok(Sse::new(stream).keep_alive(KeepAlive::new().interval(Duration::from_secs(15))))
}

fn kernel_mutation_context(
    session: &DesktopSessionRecord,
    now_unix_ms: u64,
) -> KernelMutationContext {
    KernelMutationContext {
        caller_id: session.account_id.clone(),
        session_id: session.session_id.clone(),
        now_unix_ms,
    }
}

fn kernel_api_error(reason: String) -> ApiError {
    let status = match reason.as_str() {
        "kernel_contract_not_found"
        | "kernel_work_unit_not_found"
        | "compute_product_not_found"
        | "compute_environment_package_not_found"
        | "kernel_compute_environment_package_not_found"
        | "compute_eval_run_not_found"
        | "compute_synthetic_job_not_found"
        | "kernel_compute_synthetic_job_not_found"
        | "compute_index_not_found"
        | "capacity_lot_not_found"
        | "capacity_instrument_not_found"
        | "delivery_proof_not_found"
        | "structured_capacity_instrument_not_found"
        | "structured_capacity_leg_not_found"
        | "data_asset_not_found"
        | "access_grant_not_found"
        | "delivery_bundle_not_found"
        | "liquidity_quote_not_found"
        | "liquidity_route_plan_not_found"
        | "liquidity_envelope_not_found"
        | "reserve_partition_not_found"
        | "coverage_offer_not_found"
        | "coverage_binding_not_found"
        | "risk_claim_not_found" => StatusCode::NOT_FOUND,
        "kernel_idempotency_conflict"
        | "kernel_contract_id_mismatch"
        | "kernel_capacity_lot_id_mismatch"
        | "kernel_compute_eval_run_id_mismatch"
        | "kernel_compute_synthetic_job_id_mismatch"
        | "kernel_access_grant_id_mismatch"
        | "kernel_reserve_partition_id_mismatch"
        | "kernel_risk_claim_id_mismatch"
        | "compute_product_capacity_lot_mismatch"
        | "compute_product_capacity_instrument_mismatch"
        | "compute_product_reference_index_mismatch"
        | "capacity_lot_instrument_mismatch"
        | "compute_eval_run_id_conflict"
        | "compute_eval_run_product_mismatch"
        | "compute_eval_run_capacity_lot_mismatch"
        | "compute_eval_run_instrument_mismatch"
        | "compute_eval_run_environment_mismatch"
        | "compute_eval_sample_run_mismatch"
        | "compute_eval_run_finalized"
        | "compute_eval_run_already_finalized"
        | "compute_eval_sample_already_exists"
        | "compute_eval_sample_count_exceeds_expected"
        | "compute_eval_sample_count_incomplete"
        | "delivery_proof_eval_run_conflict"
        | "compute_synthetic_job_id_conflict"
        | "compute_synthetic_job_product_mismatch"
        | "compute_synthetic_job_environment_mismatch"
        | "compute_synthetic_sample_job_mismatch"
        | "compute_synthetic_job_finalized"
        | "compute_synthetic_job_already_finalized"
        | "compute_synthetic_job_already_verified"
        | "compute_synthetic_job_not_ready_for_verification"
        | "compute_synthetic_sample_already_exists"
        | "compute_synthetic_sample_count_exceeds_expected"
        | "compute_synthetic_sample_count_incomplete"
        | "compute_synthetic_verification_eval_run_invalid"
        | "compute_synthetic_verification_sample_mismatch"
        | "structured_capacity_instrument_id_conflict"
        | "capacity_instrument_already_structured"
        | "structured_capacity_leg_product_mismatch"
        | "structured_capacity_leg_buyer_mismatch"
        | "structured_capacity_leg_provider_mismatch"
        | "structured_reservation_leg_count_invalid"
        | "structured_reservation_role_invalid"
        | "structured_reservation_leg_kind_invalid"
        | "structured_reservation_settlement_mode_invalid"
        | "structured_reservation_capacity_lot_required"
        | "structured_reservation_terms_missing"
        | "structured_swap_leg_count_invalid"
        | "structured_swap_roles_invalid"
        | "structured_swap_leg_kind_invalid"
        | "structured_swap_quantity_mismatch"
        | "structured_swap_window_mismatch"
        | "structured_swap_settlement_mode_invalid"
        | "structured_swap_reference_index_required"
        | "structured_swap_capacity_lot_required"
        | "structured_strip_leg_count_invalid"
        | "structured_strip_role_invalid"
        | "structured_strip_leg_kind_invalid"
        | "structured_strip_quantity_mismatch"
        | "structured_strip_window_sequence_invalid"
        | "structured_capacity_instrument_settlement_propagation_invalid"
        | "structured_capacity_instrument_live_legs_require_propagation"
        | "structured_capacity_instrument_legs_not_settled"
        | "structured_capacity_instrument_close_status_mismatch"
        | "compute_product_not_index_eligible"
        | "compute_index_window_already_published"
        | "compute_index_already_superseded"
        | "compute_index_correction_requires_new_index_id"
        | "compute_index_id_conflict"
        | "capacity_instrument_not_cash_settleable"
        | "future_cash_index_quality_too_low"
        | "future_cash_paper_to_physical_limit"
        | "future_cash_deliverable_coverage_limit"
        | "future_cash_concentration_limit"
        | "compute_forward_physical_disabled"
        | "compute_future_cash_disabled"
        | "compute_structured_products_disabled"
        | "clustered_forward_topology_required"
        | "clustered_forward_proof_posture_insufficient"
        | "clustered_reservation_topology_required"
        | "clustered_reservation_proof_posture_insufficient"
        | "future_cash_strike_asset_mismatch"
        | "compute_index_reference_price_missing"
        | "future_cash_settlement_window_open"
        | "data_asset_provider_mismatch"
        | "access_grant_consumer_mismatch"
        | "access_grant_already_revoked"
        | "delivery_bundle_grant_mismatch"
        | "liquidity_quote_requester_mismatch"
        | "liquidity_quote_route_kind_mismatch"
        | "liquidity_quote_not_selectable"
        | "liquidity_quote_mismatch"
        | "liquidity_route_plan_mismatch"
        | "reserve_partition_asset_mismatch"
        | "reserve_partition_insufficient_available"
        | "risk_outcome_ref_mismatch"
        | "risk_market_asset_mismatch" => StatusCode::CONFLICT,
        "work_unit_id_missing"
        | "contract_id_missing"
        | "receipt_id_missing"
        | "compute_product_id_missing"
        | "compute_eval_run_id_missing"
        | "compute_synthetic_job_id_missing"
        | "compute_eval_sample_id_missing"
        | "compute_synthetic_sample_id_missing"
        | "compute_eval_samples_missing"
        | "compute_synthetic_samples_missing"
        | "compute_eval_run_create_status_invalid"
        | "compute_eval_finalize_status_invalid"
        | "compute_synthetic_job_create_status_invalid"
        | "compute_synthetic_generation_finalize_status_invalid"
        | "compute_eval_expected_sample_count_invalid"
        | "compute_synthetic_teacher_model_ref_missing"
        | "compute_synthetic_target_sample_count_invalid"
        | "compute_synthetic_output_artifact_ref_missing"
        | "compute_synthetic_generated_at_missing"
        | "compute_synthetic_verification_eval_run_id_missing"
        | "compute_synthetic_verified_at_missing"
        | "compute_synthetic_verification_summary_missing"
        | "compute_synthetic_prompt_ref_missing"
        | "compute_synthetic_output_ref_missing"
        | "compute_synthetic_sample_recorded_at_invalid"
        | "compute_synthetic_verification_score_invalid"
        | "compute_synthetic_verification_status_missing"
        | "compute_synthetic_verification_eval_sample_id_missing"
        | "compute_eval_finalized_at_missing"
        | "compute_eval_summary_missing"
        | "compute_eval_summary_counts_invalid"
        | "compute_eval_metric_id_missing"
        | "compute_eval_metric_value_invalid"
        | "compute_eval_metric_unit_invalid"
        | "compute_eval_artifact_kind_missing"
        | "compute_eval_artifact_ref_missing"
        | "compute_eval_artifact_digest_invalid"
        | "compute_eval_score_invalid"
        | "compute_eval_pass_rate_invalid"
        | "compute_eval_sample_recorded_at_invalid"
        | "compute_eval_sample_error_reason_missing"
        | "compute_environment_ref_missing"
        | "compute_environment_version_missing"
        | "compute_environment_family_missing"
        | "compute_environment_display_name_missing"
        | "compute_environment_owner_id_missing"
        | "compute_environment_timestamps_invalid"
        | "compute_environment_package_digest_invalid"
        | "compute_environment_harness_ref_missing"
        | "compute_environment_runtime_family_missing"
        | "compute_environment_entrypoint_invalid"
        | "compute_environment_time_budget_invalid"
        | "compute_environment_dataset_ref_missing"
        | "compute_environment_dataset_mount_invalid"
        | "compute_environment_dataset_integrity_invalid"
        | "compute_environment_rubric_ref_missing"
        | "compute_environment_rubric_threshold_invalid"
        | "compute_environment_artifact_kind_missing"
        | "compute_environment_artifact_ref_invalid"
        | "compute_environment_policy_ref_invalid"
        | "capacity_lot_id_missing"
        | "capacity_instrument_id_missing"
        | "structured_capacity_instrument_id_missing"
        | "structured_capacity_leg_instrument_id_missing"
        | "delivery_proof_id_missing"
        | "compute_index_id_missing"
        | "compute_product_resource_class_invalid"
        | "compute_product_capacity_unit_missing"
        | "compute_product_window_spec_missing"
        | "compute_product_launch_taxonomy_version_missing"
        | "compute_product_launch_taxonomy_version_invalid"
        | "compute_product_launch_product_id_unsupported"
        | "compute_product_capability_envelope_missing"
        | "compute_product_backend_family_missing"
        | "compute_product_backend_family_mismatch"
        | "compute_product_execution_kind_missing"
        | "compute_product_execution_kind_invalid"
        | "compute_product_compute_family_missing"
        | "compute_product_compute_family_mismatch"
        | "compute_product_model_identity_missing"
        | "compute_product_host_accelerator_vendor_missing"
        | "compute_product_host_memory_gb_invalid"
        | "compute_product_ollama_runtime_missing"
        | "compute_product_gpt_oss_runtime_missing"
        | "compute_product_apple_platform_gates_missing"
        | "compute_product_apple_silicon_requirement_missing"
        | "capacity_lot_window_invalid"
        | "capacity_instrument_window_invalid"
        | "reservation_capacity_lot_required"
        | "reservation_window_not_future"
        | "reservation_settlement_mode_invalid"
        | "reservation_premium_price_missing"
        | "reservation_buyer_required"
        | "reservation_terms_missing"
        | "reservation_exercise_window_start_missing"
        | "reservation_exercise_window_end_missing"
        | "reservation_exercise_window_invalid"
        | "reservation_exercise_window_outside_delivery"
        | "reservation_exercise_price_missing"
        | "reservation_exercise_price_invalid"
        | "reservation_price_asset_mismatch"
        | "structured_capacity_instrument_legs_missing"
        | "structured_capacity_leg_duplicate"
        | "structured_capacity_leg_order_duplicate"
        | "structured_capacity_leg_not_live"
        | "structured_capacity_leg_metadata_invalid"
        | "structured_capacity_instrument_metadata_invalid"
        | "structured_capacity_instrument_close_status_invalid"
        | "future_cash_capacity_lot_not_allowed"
        | "future_cash_settlement_mode_invalid"
        | "future_cash_window_not_future"
        | "future_cash_buyer_required"
        | "future_cash_strike_price_missing"
        | "future_cash_reference_index_required"
        | "compute_index_window_invalid"
        | "data_asset_id_missing"
        | "data_asset_provider_id_missing"
        | "data_asset_kind_missing"
        | "data_asset_title_missing"
        | "access_grant_id_missing"
        | "access_grant_window_invalid"
        | "access_grant_consumer_id_missing"
        | "access_grant_not_accepting"
        | "access_grant_not_ready_for_delivery"
        | "delivery_bundle_id_missing"
        | "delivery_bundle_ref_missing"
        | "revocation_id_missing"
        | "revocation_reason_missing"
        | "permission_policy_scope_missing"
        | "liquidity_quote_id_missing"
        | "liquidity_requester_id_missing"
        | "liquidity_route_kind_missing"
        | "liquidity_source_amount_missing"
        | "liquidity_quote_window_invalid"
        | "liquidity_route_plan_id_missing"
        | "liquidity_solver_id_missing"
        | "liquidity_route_plan_window_invalid"
        | "liquidity_envelope_id_missing"
        | "liquidity_envelope_owner_id_missing"
        | "liquidity_envelope_reserved_amount_missing"
        | "liquidity_envelope_window_invalid"
        | "liquidity_route_plan_not_ready"
        | "liquidity_settlement_intent_id_missing"
        | "liquidity_settlement_proof_missing"
        | "liquidity_settlement_amount_mismatch"
        | "reserve_partition_id_missing"
        | "reserve_partition_owner_id_missing"
        | "reserve_partition_amount_invalid"
        | "reserve_partition_reason_missing"
        | "coverage_offer_id_missing"
        | "coverage_underwriter_id_missing"
        | "coverage_cap_missing"
        | "coverage_premium_missing"
        | "coverage_offer_window_invalid"
        | "coverage_binding_id_missing"
        | "coverage_binding_offer_missing"
        | "prediction_position_id_missing"
        | "prediction_participant_id_missing"
        | "prediction_position_not_bounded"
        | "prediction_position_window_invalid"
        | "risk_claim_id_missing"
        | "risk_claimant_id_missing"
        | "risk_claim_reason_missing"
        | "risk_claim_payout_exceeds_coverage"
        | "risk_resolution_ref_missing"
        | "risk_claim_approved_payout_missing"
        | "risk_claim_payout_exceeds_request"
        | "risk_signal_id_missing"
        | "risk_outcome_ref_missing"
        | "risk_implied_fail_probability_invalid"
        | "risk_calibration_score_invalid"
        | "risk_concentration_invalid" => StatusCode::BAD_REQUEST,
        _ => StatusCode::BAD_REQUEST,
    };
    ApiError {
        status,
        error: "kernel_error",
        reason,
    }
}

fn kernel_contract_error(error: anyhow::Error) -> ApiError {
    ApiError {
        status: StatusCode::BAD_REQUEST,
        error: "kernel_contract_error",
        reason: error.to_string(),
    }
}

fn record_kernel_mutation_observability(
    state: &AppState,
    session: &DesktopSessionRecord,
    now_unix_ms: u64,
    receipt_type: &str,
    receipt_event: Option<ReceiptProjectionEvent>,
    snapshot_event: Option<SnapshotProjectionEvent>,
) {
    if let Some(receipt_event) = receipt_event {
        if let Ok(mut store) = state.store.write() {
            let mut attributes = BTreeMap::new();
            if let Some(work_unit_id) = receipt_event.receipt.trace.work_unit_id.clone() {
                attributes.insert("work_unit_id".to_string(), work_unit_id);
            }
            if let Some(contract_id) = receipt_event.receipt.trace.contract_id.clone() {
                attributes.insert("contract_id".to_string(), contract_id);
            }
            attributes.insert(
                "canonical_receipt_id".to_string(),
                receipt_event.receipt.receipt_id.clone(),
            );
            attributes.insert(
                "canonical_receipt_type".to_string(),
                receipt_event.receipt.receipt_type.clone(),
            );
            store.economy.record(
                receipt_type,
                now_unix_ms,
                AuthorityReceiptContext {
                    session_id: Some(session.session_id.clone()),
                    account_id: Some(session.account_id.clone()),
                    request_id: Some(receipt_event.receipt.receipt_id.clone()),
                    status: Some(if receipt_event.seq > 0 {
                        "recorded".to_string()
                    } else {
                        "replayed".to_string()
                    }),
                    reason: None,
                    relay_url: None,
                    amount_sats: None,
                    payment_pointer: None,
                    attributes,
                },
            );
        }
        let _ = state.kernel_receipt_tx.send(receipt_event);
    }
    if let Some(snapshot_event) = snapshot_event {
        let _ = state.kernel_snapshot_tx.send(snapshot_event);
    }
}

fn authenticate_session(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<DesktopSessionRecord, ApiError> {
    let header = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .ok_or_else(|| ApiError {
            status: StatusCode::UNAUTHORIZED,
            error: "unauthorized",
            reason: "missing_bearer_token".to_string(),
        })?;
    let token = header
        .strip_prefix("Bearer ")
        .or_else(|| header.strip_prefix("bearer "))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ApiError {
            status: StatusCode::UNAUTHORIZED,
            error: "unauthorized",
            reason: "invalid_bearer_token".to_string(),
        })?;
    let store = state.store.read().map_err(|_| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        error: "internal_error",
        reason: "session_store_poisoned".to_string(),
    })?;
    let session = store
        .sessions_by_access_token
        .get(token)
        .cloned()
        .ok_or_else(|| ApiError {
            status: StatusCode::UNAUTHORIZED,
            error: "unauthorized",
            reason: "session_not_found".to_string(),
        })?;
    drop(store);
    if now_unix_ms() >= session.expires_at_unix_ms {
        return Err(ApiError {
            status: StatusCode::UNAUTHORIZED,
            error: "unauthorized",
            reason: "session_expired".to_string(),
        });
    }
    Ok(session)
}

fn prune_expired_starter_offers(
    state: &mut StarterDemandState,
    config: &ServiceConfig,
    now_unix_ms: u64,
) -> Vec<StarterDemandOfferEvent> {
    let mut released_budget_sats = 0u64;
    let mut expired_events = Vec::new();
    for (session_id, offers) in &mut state.offers_by_session {
        for offer in offers.iter_mut() {
            if offer.status == StarterOfferStatus::Offered
                && now_unix_ms
                    > offer
                        .start_confirm_by_unix_ms
                        .unwrap_or(offer.created_at_unix_ms)
            {
                released_budget_sats = released_budget_sats.saturating_add(expire_offer(
                    offer,
                    now_unix_ms,
                    "starter_offer_start_confirm_missed",
                ));
                expired_events.push(StarterDemandOfferEvent {
                    session_id: session_id.clone(),
                    offer: offer.clone(),
                });
            } else if offer.status == StarterOfferStatus::Running {
                if now_unix_ms
                    > offer
                        .execution_expires_at_unix_ms
                        .unwrap_or(offer.expires_at_unix_ms)
                {
                    released_budget_sats = released_budget_sats.saturating_add(expire_offer(
                        offer,
                        now_unix_ms,
                        "starter_offer_execution_expired",
                    ));
                    expired_events.push(StarterDemandOfferEvent {
                        session_id: session_id.clone(),
                        offer: offer.clone(),
                    });
                } else if now_unix_ms
                    > offer
                        .next_heartbeat_due_at_unix_ms
                        .unwrap_or(offer.expires_at_unix_ms)
                {
                    released_budget_sats = released_budget_sats.saturating_add(expire_offer(
                        offer,
                        now_unix_ms,
                        "starter_offer_heartbeat_missed",
                    ));
                    expired_events.push(StarterDemandOfferEvent {
                        session_id: session_id.clone(),
                        offer: offer.clone(),
                    });
                }
            }
        }
        offers.retain(|offer| {
            if offer.status == StarterOfferStatus::Completed {
                return true;
            }
            if matches!(
                offer.status,
                StarterOfferStatus::Offered | StarterOfferStatus::Running
            ) {
                return true;
            }
            let terminal_at_unix_ms = offer
                .released_at_unix_ms
                .or(offer.completed_at_unix_ms)
                .unwrap_or(offer.expires_at_unix_ms);
            now_unix_ms.saturating_sub(terminal_at_unix_ms)
                < config
                    .starter_demand_dispatch_interval_seconds
                    .saturating_mul(1_000)
        });
    }
    state.budget_allocated_sats = state
        .budget_allocated_sats
        .saturating_sub(released_budget_sats);
    expired_events
}

fn maybe_dispatch_starter_offer(
    state: &mut StarterDemandState,
    config: &ServiceConfig,
    session: &DesktopSessionRecord,
    now_unix_ms: u64,
) -> Option<StarterDemandOfferRecord> {
    let active_offers = state
        .offers_by_session
        .get(session.session_id.as_str())
        .map(|offers| {
            offers
                .iter()
                .filter(|offer| {
                    matches!(
                        offer.status,
                        StarterOfferStatus::Offered | StarterOfferStatus::Running
                    )
                })
                .count()
        })
        .unwrap_or(0);
    if active_offers >= config.starter_demand_max_active_offers_per_session {
        return None;
    }

    let last_dispatch_at = state
        .last_dispatch_by_session
        .get(session.session_id.as_str())
        .copied()
        .unwrap_or(0);
    let dispatch_interval_ms = config
        .starter_demand_dispatch_interval_seconds
        .saturating_mul(1_000);
    if last_dispatch_at != 0 && now_unix_ms < last_dispatch_at.saturating_add(dispatch_interval_ms)
    {
        return None;
    }

    let remaining_budget_sats = config
        .starter_demand_budget_cap_sats
        .saturating_sub(state.budget_allocated_sats);
    let template = next_template_for_remaining_budget(state, remaining_budget_sats)?;

    let request_id = format!("starter-hosted-{:06}", state.next_offer_seq);
    state.next_offer_seq = state.next_offer_seq.saturating_add(1);
    let created_at_unix_ms = now_unix_ms;
    let ttl_seconds = config.starter_demand_request_ttl_seconds;
    let start_confirm_by_unix_ms = created_at_unix_ms.saturating_add(
        config
            .starter_demand_start_confirm_seconds
            .saturating_mul(1_000),
    );
    let offer = StarterDemandOfferRecord {
        request_id: request_id.clone(),
        requester: STARTER_DEMAND_REQUESTER.to_string(),
        request_kind: STARTER_DEMAND_REQUEST_KIND,
        capability: template.capability.to_string(),
        execution_input: Some(template.summary.to_string()),
        price_sats: template.payout_sats,
        ttl_seconds,
        created_at_unix_ms,
        expires_at_unix_ms: start_confirm_by_unix_ms,
        start_confirm_by_unix_ms: Some(start_confirm_by_unix_ms),
        execution_started_at_unix_ms: None,
        execution_expires_at_unix_ms: None,
        last_heartbeat_at_unix_ms: None,
        next_heartbeat_due_at_unix_ms: None,
        status: StarterOfferStatus::Offered,
        payment_pointer: None,
        completed_at_unix_ms: None,
        released_at_unix_ms: None,
        failure_reason: None,
    };
    state
        .offers_by_session
        .entry(session.session_id.clone())
        .or_default()
        .push(offer.clone());
    state.budget_allocated_sats = state
        .budget_allocated_sats
        .saturating_add(template.payout_sats);
    state
        .last_dispatch_by_session
        .insert(session.session_id.clone(), now_unix_ms);
    Some(offer)
}

fn starter_demand_provider_proof_reason(
    session: &DesktopSessionRecord,
    provider_nostr_pubkey: Option<&str>,
) -> Option<String> {
    if !session
        .desktop_client_id
        .starts_with(AUTOPILOT_DESKTOP_CLIENT_ID_PREFIX)
    {
        return Some("starter_demand_requires_autopilot_desktop_session".to_string());
    }
    let Some(bound_nostr_pubkey) = session
        .bound_nostr_pubkey
        .as_deref()
        .filter(|value| !value.is_empty())
    else {
        return Some("starter_demand_requires_bound_nostr_identity".to_string());
    };
    let Some(provider_nostr_pubkey) = provider_nostr_pubkey.filter(|value| !value.is_empty())
    else {
        return Some("starter_demand_provider_nostr_pubkey_missing".to_string());
    };
    if bound_nostr_pubkey != provider_nostr_pubkey {
        return Some("starter_demand_provider_nostr_pubkey_mismatch".to_string());
    }
    None
}

fn starter_demand_heartbeat_interval_seconds(config: &ServiceConfig) -> u64 {
    config
        .starter_demand_heartbeat_timeout_seconds
        .saturating_div(2)
        .max(1)
}

fn expire_offer(
    offer: &mut StarterDemandOfferRecord,
    now_unix_ms: u64,
    failure_reason: &str,
) -> u64 {
    if matches!(
        offer.status,
        StarterOfferStatus::Completed | StarterOfferStatus::Released | StarterOfferStatus::Expired
    ) {
        return 0;
    }
    let released_budget_sats = if matches!(
        offer.status,
        StarterOfferStatus::Offered | StarterOfferStatus::Running
    ) {
        offer.price_sats
    } else {
        0
    };
    offer.status = StarterOfferStatus::Expired;
    offer.released_at_unix_ms = Some(now_unix_ms);
    offer.failure_reason = Some(failure_reason.to_string());
    offer.next_heartbeat_due_at_unix_ms = None;
    offer.expires_at_unix_ms = now_unix_ms;
    released_budget_sats
}

fn release_offer(
    offer: &mut StarterDemandOfferRecord,
    now_unix_ms: u64,
    failure_reason: &str,
) -> u64 {
    if matches!(
        offer.status,
        StarterOfferStatus::Completed | StarterOfferStatus::Released | StarterOfferStatus::Expired
    ) {
        return 0;
    }
    let released_budget_sats = if matches!(
        offer.status,
        StarterOfferStatus::Offered | StarterOfferStatus::Running
    ) {
        offer.price_sats
    } else {
        0
    };
    offer.status = StarterOfferStatus::Released;
    offer.released_at_unix_ms = Some(now_unix_ms);
    offer.failure_reason = Some(failure_reason.to_string());
    offer.next_heartbeat_due_at_unix_ms = None;
    offer.expires_at_unix_ms = now_unix_ms;
    released_budget_sats
}

fn next_template_for_remaining_budget(
    state: &mut StarterDemandState,
    remaining_budget_sats: u64,
) -> Option<StarterDemandTemplate> {
    for offset in 0..STARTER_DEMAND_TEMPLATES.len() {
        let index = (state.next_template_index + offset) % STARTER_DEMAND_TEMPLATES.len();
        let template = STARTER_DEMAND_TEMPLATES[index];
        if template.payout_sats <= remaining_budget_sats {
            state.next_template_index = (index + 1) % STARTER_DEMAND_TEMPLATES.len();
            return Some(template);
        }
    }
    None
}

fn starter_offer_response(record: &StarterDemandOfferRecord) -> StarterDemandOffer {
    StarterDemandOffer {
        request_id: record.request_id.clone(),
        requester: record.requester.clone(),
        request_kind: record.request_kind,
        capability: record.capability.clone(),
        execution_input: record.execution_input.clone(),
        price_sats: record.price_sats,
        ttl_seconds: record.ttl_seconds,
        created_at_unix_ms: record.created_at_unix_ms,
        expires_at_unix_ms: record.expires_at_unix_ms,
        status: record.status.label().to_string(),
        start_confirm_by_unix_ms: record.start_confirm_by_unix_ms,
        execution_started_at_unix_ms: record.execution_started_at_unix_ms,
        execution_expires_at_unix_ms: record.execution_expires_at_unix_ms,
        last_heartbeat_at_unix_ms: record.last_heartbeat_at_unix_ms,
        next_heartbeat_due_at_unix_ms: record.next_heartbeat_due_at_unix_ms,
    }
}

fn record_expired_offer_receipts(
    store: &mut ControlStore,
    expired_events: Vec<StarterDemandOfferEvent>,
    now_unix_ms: u64,
) {
    for event in expired_events {
        let session = find_session_by_id(store, event.session_id.as_str()).cloned();
        store.economy.record(
            "starter_offer.expired",
            now_unix_ms,
            starter_offer_receipt_context(
                session.as_ref(),
                Some(&event.offer),
                event.offer.failure_reason.as_deref(),
                None,
                None,
            ),
        );
    }
}

fn find_session_by_id<'a>(
    store: &'a ControlStore,
    session_id: &str,
) -> Option<&'a DesktopSessionRecord> {
    store
        .sessions_by_access_token
        .values()
        .find(|session| session.session_id == session_id)
}

fn runtime_snapshot(
    config: &ServiceConfig,
    store: &ControlStore,
    now_unix_ms: u64,
) -> PublicRuntimeSnapshot {
    let (starter_offers_waiting_ack, starter_offers_running) = store
        .starter_demand
        .offers_by_session
        .values()
        .flat_map(|offers| offers.iter())
        .fold(
            (0usize, 0usize),
            |(waiting_ack, running), offer| match offer.status {
                StarterOfferStatus::Offered => (waiting_ack.saturating_add(1), running),
                StarterOfferStatus::Running => (waiting_ack, running.saturating_add(1)),
                StarterOfferStatus::Completed
                | StarterOfferStatus::Released
                | StarterOfferStatus::Expired => (waiting_ack, running),
            },
        );
    let compute_metrics = store.kernel.compute_market_metrics(now_unix_ms as i64);
    let liquidity_metrics = store.kernel.liquidity_market_metrics(now_unix_ms as i64);
    let risk_metrics = store.kernel.risk_market_metrics(now_unix_ms as i64);
    PublicRuntimeSnapshot {
        hosted_nexus_relay_url: config.hosted_nexus_relay_url.clone(),
        sessions_active: store.sessions_by_access_token.len(),
        sync_tokens_active: store.sync_tokens.len(),
        starter_demand_budget_cap_sats: config.starter_demand_budget_cap_sats,
        starter_demand_budget_allocated_sats: store.starter_demand.budget_allocated_sats,
        starter_offers_waiting_ack,
        starter_offers_running,
        compute_products_active: compute_metrics.compute_products_active,
        compute_capacity_lots_open: compute_metrics.compute_capacity_lots_open,
        compute_capacity_lots_delivering: compute_metrics.compute_capacity_lots_delivering,
        compute_instruments_active: compute_metrics.compute_instruments_active,
        compute_inventory_quantity_open: compute_metrics.compute_inventory_quantity_open,
        compute_inventory_quantity_reserved: compute_metrics.compute_inventory_quantity_reserved,
        compute_inventory_quantity_delivering: compute_metrics
            .compute_inventory_quantity_delivering,
        compute_delivery_proofs_24h: compute_metrics.compute_delivery_proofs_24h,
        compute_delivery_quantity_24h: compute_metrics.compute_delivery_quantity_24h,
        compute_delivery_rejections_24h: compute_metrics.compute_delivery_rejections_24h,
        compute_delivery_variances_24h: compute_metrics.compute_delivery_variances_24h,
        compute_validator_challenges_open: compute_metrics.compute_validator_challenges_open,
        compute_validator_challenges_queued: compute_metrics.compute_validator_challenges_queued,
        compute_validator_challenges_verified_24h: compute_metrics
            .compute_validator_challenges_verified_24h,
        compute_validator_challenges_rejected_24h: compute_metrics
            .compute_validator_challenges_rejected_24h,
        compute_validator_challenges_timed_out_24h: compute_metrics
            .compute_validator_challenges_timed_out_24h,
        compute_delivery_accept_rate_24h: compute_metrics.compute_delivery_accept_rate_24h,
        compute_fill_ratio_24h: compute_metrics.compute_fill_ratio_24h,
        compute_priced_instruments_24h: compute_metrics.compute_priced_instruments_24h,
        compute_indices_published_24h: compute_metrics.compute_indices_published_24h,
        compute_index_corrections_24h: compute_metrics.compute_index_corrections_24h,
        compute_index_thin_windows_24h: compute_metrics.compute_index_thin_windows_24h,
        compute_index_settlement_eligible_24h: compute_metrics
            .compute_index_settlement_eligible_24h,
        compute_index_quality_score_24h: compute_metrics.compute_index_quality_score_24h,
        compute_active_provider_count: compute_metrics.compute_active_provider_count,
        compute_provider_concentration_hhi: compute_metrics.compute_provider_concentration_hhi,
        compute_forward_physical_instruments_active: compute_metrics
            .compute_forward_physical_instruments_active,
        compute_forward_physical_open_quantity: compute_metrics
            .compute_forward_physical_open_quantity,
        compute_forward_physical_defaults_24h: compute_metrics
            .compute_forward_physical_defaults_24h,
        compute_future_cash_instruments_active: compute_metrics
            .compute_future_cash_instruments_active,
        compute_future_cash_open_interest: compute_metrics.compute_future_cash_open_interest,
        compute_future_cash_cash_settlements_24h: compute_metrics
            .compute_future_cash_cash_settlements_24h,
        compute_future_cash_cash_flow_24h: compute_metrics.compute_future_cash_cash_flow_24h,
        compute_future_cash_defaults_24h: compute_metrics.compute_future_cash_defaults_24h,
        compute_future_cash_collateral_shortfall_24h: compute_metrics
            .compute_future_cash_collateral_shortfall_24h,
        compute_structured_instruments_active: compute_metrics
            .compute_structured_instruments_active,
        compute_structured_instruments_closed_24h: compute_metrics
            .compute_structured_instruments_closed_24h,
        compute_max_buyer_concentration_share: compute_metrics
            .compute_max_buyer_concentration_share,
        compute_paper_to_physical_ratio: compute_metrics.compute_paper_to_physical_ratio,
        compute_deliverable_coverage_ratio: compute_metrics.compute_deliverable_coverage_ratio,
        compute_breakers_tripped: compute_metrics.compute_breakers_tripped,
        compute_breakers_guarded: compute_metrics.compute_breakers_guarded,
        compute_breaker_states: compute_metrics.compute_breaker_states.clone(),
        compute_rollout_gates: compute_metrics.compute_rollout_gates.clone(),
        compute_truth_labels: compute_metrics.compute_truth_labels.clone(),
        compute_reconciliation_gap_24h: compute_metrics.compute_reconciliation_gap_24h,
        compute_policy_bundle_id: compute_metrics.compute_policy_bundle_id.clone(),
        compute_policy_version: compute_metrics.compute_policy_version.clone(),
        liquidity_quotes_active: liquidity_metrics.liquidity_quotes_active,
        liquidity_route_plans_active: liquidity_metrics.liquidity_route_plans_active,
        liquidity_envelopes_open: liquidity_metrics.liquidity_envelopes_open,
        liquidity_settlements_24h: liquidity_metrics.liquidity_settlements_24h,
        liquidity_reserve_partitions_active: liquidity_metrics.liquidity_reserve_partitions_active,
        liquidity_value_moved_24h: liquidity_metrics.liquidity_value_moved_24h,
        risk_coverage_offers_open: risk_metrics.risk_coverage_offers_open,
        risk_coverage_bindings_active: risk_metrics.risk_coverage_bindings_active,
        risk_prediction_positions_open: risk_metrics.risk_prediction_positions_open,
        risk_claims_open: risk_metrics.risk_claims_open,
        risk_signals_active: risk_metrics.risk_signals_active,
        risk_implied_fail_probability_bps: risk_metrics.risk_implied_fail_probability_bps,
        risk_calibration_score: risk_metrics.risk_calibration_score,
        risk_coverage_concentration_hhi: risk_metrics.risk_coverage_concentration_hhi,
    }
}

fn desktop_session_receipt_context(record: &DesktopSessionRecord) -> AuthorityReceiptContext {
    let mut attributes = std::collections::BTreeMap::new();
    attributes.insert(
        "desktop_client_id".to_string(),
        record.desktop_client_id.clone(),
    );
    if let Some(device_name) = record.device_name.clone() {
        attributes.insert("device_name".to_string(), device_name);
    }
    if let Some(client_version) = record.client_version.clone() {
        attributes.insert("client_version".to_string(), client_version);
    }
    if let Some(bound_nostr_pubkey) = record.bound_nostr_pubkey.clone() {
        attributes.insert("bound_nostr_pubkey".to_string(), bound_nostr_pubkey);
    }
    AuthorityReceiptContext {
        session_id: Some(record.session_id.clone()),
        account_id: Some(record.account_id.clone()),
        status: Some("issued".to_string()),
        attributes,
        ..AuthorityReceiptContext::default()
    }
}

fn sync_token_receipt_context(
    session: &DesktopSessionRecord,
    token: &SyncTokenRecord,
) -> AuthorityReceiptContext {
    let mut attributes = std::collections::BTreeMap::new();
    attributes.insert("rotation_id".to_string(), token.rotation_id.clone());
    attributes.insert("transport".to_string(), "spacetime_ws".to_string());
    attributes.insert(
        "protocol_version".to_string(),
        "spacetime.sync.v1".to_string(),
    );
    AuthorityReceiptContext {
        session_id: Some(session.session_id.clone()),
        account_id: Some(session.account_id.clone()),
        status: Some("issued".to_string()),
        attributes,
        ..AuthorityReceiptContext::default()
    }
}

fn starter_offer_receipt_context(
    session: Option<&DesktopSessionRecord>,
    offer: Option<&StarterDemandOfferRecord>,
    reason: Option<&str>,
    relay_url: Option<&str>,
    payment_pointer: Option<&str>,
) -> AuthorityReceiptContext {
    let mut attributes = std::collections::BTreeMap::new();
    if let Some(session) = session {
        attributes.insert(
            "desktop_client_id".to_string(),
            session.desktop_client_id.clone(),
        );
        if let Some(bound_nostr_pubkey) = session.bound_nostr_pubkey.clone() {
            attributes.insert("bound_nostr_pubkey".to_string(), bound_nostr_pubkey);
        }
    }
    if let Some(offer) = offer {
        attributes.insert("capability".to_string(), offer.capability.clone());
        attributes.insert("requester".to_string(), offer.requester.clone());
        attributes.insert("request_kind".to_string(), offer.request_kind.to_string());
    }
    AuthorityReceiptContext {
        session_id: session.map(|session| session.session_id.clone()),
        account_id: session.map(|session| session.account_id.clone()),
        request_id: offer.map(|offer| offer.request_id.clone()),
        status: offer.map(|offer| offer.status.label().to_string()),
        reason: reason.map(ToOwned::to_owned),
        relay_url: relay_url.map(ToOwned::to_owned),
        amount_sats: offer.map(|offer| offer.price_sats),
        payment_pointer: payment_pointer.map(ToOwned::to_owned),
        attributes,
    }
}

fn parse_u64_env(key: &str, default: u64) -> Result<u64, String> {
    std::env::var(key)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .map_or(Ok(default), |value| {
            value
                .parse::<u64>()
                .map_err(|error| format!("invalid {key}: {error}"))
        })
}

fn parse_bool_env(key: &str, default: bool) -> Result<bool, String> {
    std::env::var(key)
        .ok()
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty())
        .map_or(Ok(default), |value| match value.as_str() {
            "1" | "true" | "yes" | "on" => Ok(true),
            "0" | "false" | "no" | "off" => Ok(false),
            _ => Err(format!("invalid {key}: expected boolean")),
        })
}

fn parse_usize_env(key: &str, default: usize) -> Result<usize, String> {
    std::env::var(key)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .map_or(Ok(default), |value| {
            value
                .parse::<usize>()
                .map_err(|error| format!("invalid {key}: {error}"))
        })
}

fn normalize_required_field(value: &str, reason: &str) -> Result<String, ApiError> {
    let normalized = value.trim();
    if normalized.is_empty() {
        return Err(ApiError {
            status: StatusCode::BAD_REQUEST,
            error: "invalid_request",
            reason: reason.to_string(),
        });
    }
    Ok(normalized.to_string())
}

fn normalize_optional_field(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|candidate| !candidate.is_empty())
        .map(ToOwned::to_owned)
}

fn sanitize_identifier(value: &str) -> String {
    let sanitized = value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '-' || character == '_' {
                character
            } else {
                '_'
            }
        })
        .collect::<String>();
    if sanitized.is_empty() {
        "desktop".to_string()
    } else {
        sanitized
    }
}

fn random_token() -> String {
    hex::encode(rand::random::<[u8; 24]>())
}

fn now_unix_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_millis() as u64)
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use axum::body::{Body, to_bytes};
    use axum::http::{Request, StatusCode};
    use axum::response::Response;
    use openagents_kernel_core::authority::{
        AcceptAccessGrantRequest, AcceptAccessGrantResponse, AcceptComputeOutcomeRequest,
        AdjustReservePartitionRequest, AdjustReservePartitionResponse,
        AppendComputeEvaluationSamplesRequest, AppendComputeSyntheticDataSamplesRequest,
        BindCoverageRequest, BindCoverageResponse, CashSettleCapacityInstrumentRequest,
        CloseCapacityInstrumentRequest, CloseStructuredCapacityInstrumentRequest,
        CorrectComputeIndexRequest, CreateAccessGrantRequest, CreateAccessGrantResponse,
        CreateCapacityInstrumentRequest, CreateCapacityLotRequest,
        CreateComputeEvaluationRunRequest, CreateComputeProductRequest,
        CreateComputeSyntheticDataJobRequest, CreateComputeTrainingRunRequest,
        CreateContractRequest, CreateContractResponse, CreateLiquidityQuoteRequest,
        CreateLiquidityQuoteResponse, CreatePredictionPositionRequest,
        CreatePredictionPositionResponse, CreateRiskClaimRequest, CreateRiskClaimResponse,
        CreateStructuredCapacityInstrumentRequest, CreateWorkUnitRequest, CreateWorkUnitResponse,
        ExecuteSettlementIntentRequest, ExecuteSettlementIntentResponse,
        FinalizeComputeEvaluationRunRequest, FinalizeComputeSyntheticDataGenerationRequest,
        FinalizeComputeTrainingRunRequest, FinalizeVerdictRequest, FinalizeVerdictResponse,
        HttpKernelAuthorityClient, IssueDeliveryBundleRequest, IssueDeliveryBundleResponse,
        IssueLiquidityEnvelopeRequest, IssueLiquidityEnvelopeResponse, KernelAuthority,
        PlaceCoverageOfferRequest, PlaceCoverageOfferResponse, PublishComputeIndexRequest,
        PublishRiskSignalRequest, PublishRiskSignalResponse, RecordComputeAdapterWindowRequest,
        RecordComputeSyntheticDataVerificationRequest, RecordDeliveryProofRequest,
        RegisterComputeBenchmarkPackageRequest, RegisterComputeCheckpointFamilyPolicyRequest,
        RegisterComputeEnvironmentPackageRequest, RegisterComputeTrainingPolicyRequest,
        RegisterComputeValidatorPolicyRequest, RegisterDataAssetRequest, RegisterDataAssetResponse,
        RegisterReservePartitionRequest, RegisterReservePartitionResponse, ResolveRiskClaimRequest,
        ResolveRiskClaimResponse, RevokeAccessGrantRequest, RevokeAccessGrantResponse,
        SelectRoutePlanRequest, SelectRoutePlanResponse, SubmitOutputRequest, SubmitOutputResponse,
    };
    use openagents_kernel_core::compute::{
        ApplePlatformCapability, COMPUTE_LAUNCH_TAXONOMY_VERSION, CapacityInstrument,
        CapacityInstrumentClosureReason, CapacityInstrumentKind, CapacityInstrumentStatus,
        CapacityLot, CapacityLotStatus, CapacityNonDeliveryReason, CapacityReserveState,
        ComputeAcceptedOutcome, ComputeAcceptedOutcomeKind, ComputeAdapterAggregationEligibility,
        ComputeAdapterCheckpointPointer, ComputeAdapterContributionDisposition,
        ComputeAdapterContributionOutcome, ComputeAdapterContributionValidationReasonCode,
        ComputeAdapterDatasetSlice, ComputeAdapterPolicyRevision,
        ComputeAdapterPromotionDisposition, ComputeAdapterTrainingWindow,
        ComputeAdapterWindowStatus, ComputeBackendFamily, ComputeBenchmarkPackage,
        ComputeCapabilityEnvelope, ComputeCheckpointBinding, ComputeCheckpointFamilyPolicy,
        ComputeEnvironmentArtifactExpectation, ComputeEnvironmentBinding,
        ComputeEnvironmentDatasetBinding, ComputeEnvironmentHarness, ComputeEnvironmentPackage,
        ComputeEnvironmentPackageStatus, ComputeEnvironmentRubricBinding,
        ComputeEvaluationArtifact, ComputeEvaluationMetric, ComputeEvaluationRun,
        ComputeEvaluationRunStatus, ComputeEvaluationSample, ComputeEvaluationSampleStatus,
        ComputeExecutionKind, ComputeFamily, ComputeHostCapability, ComputeIndex,
        ComputeIndexCorrectionReason, ComputeIndexStatus, ComputeProduct, ComputeProductStatus,
        ComputeProofPosture, ComputeRegistryStatus, ComputeSettlementFailureReason,
        ComputeSettlementMode, ComputeSyntheticDataJob, ComputeSyntheticDataJobStatus,
        ComputeSyntheticDataSample, ComputeSyntheticDataSampleStatus, ComputeTrainingPolicy,
        ComputeTrainingRun, ComputeTrainingRunStatus, ComputeTrainingSummary,
        ComputeValidatorPolicy, DeliveryProof, DeliveryProofStatus, DeliveryVerificationEvidence,
        GptOssRuntimeCapability, StructuredCapacityInstrument, StructuredCapacityInstrumentKind,
        StructuredCapacityInstrumentStatus, StructuredCapacityLeg, StructuredCapacityLegRole,
    };
    use openagents_kernel_core::compute_benchmarks::{
        ComputeBenchmarkAdapterKind, ComputeBenchmarkCaseImport, ComputeBenchmarkImportRequest,
        MmluMultipleChoiceCaseMetadata, adapt_compute_benchmark_import,
    };
    use openagents_kernel_core::compute_contracts;
    use openagents_kernel_core::data::{
        AccessGrant, AccessGrantStatus, DataAsset, DataAssetStatus, DeliveryBundle,
        DeliveryBundleStatus, PermissionPolicy, RevocationReceipt, RevocationStatus,
    };
    use openagents_kernel_core::labor::{
        Contract, ContractStatus, SettlementLink, SettlementStatus, Submission, SubmissionStatus,
        Verdict, VerdictOutcome, WorkUnit, WorkUnitStatus,
    };
    use openagents_kernel_core::liquidity::{
        Envelope, EnvelopeStatus, Quote, QuoteStatus, ReservePartition, ReservePartitionStatus,
        RoutePlan, RoutePlanStatus, SettlementIntent, SettlementIntentStatus,
    };
    use openagents_kernel_core::receipts::{
        Asset, Money, MoneyAmount, PolicyContext, Receipt, ReceiptHints, TraceContext,
        VerificationTier,
    };
    use openagents_kernel_core::risk::{
        CoverageBinding, CoverageBindingStatus, CoverageOffer, CoverageOfferStatus,
        PredictionPosition, PredictionPositionStatus, PredictionSide, RiskClaim, RiskClaimStatus,
        RiskSignal, RiskSignalStatus,
    };
    use openagents_kernel_core::time::floor_to_minute_utc;
    use openagents_kernel_proto::openagents::compute::v1 as proto_compute;
    use openagents_validator_service::{
        GpuFreivaldsMerkleWitness, ValidatorChallengeContext, ValidatorChallengeRequest,
        ValidatorChallengeResult, ValidatorChallengeStatus, ValidatorChallengeVerdict,
    };
    use serde_json::json;
    use tower::ServiceExt;

    use super::{
        DEFAULT_COMPUTE_POLICY_BUNDLE_ID, DEFAULT_COMPUTE_POLICY_VERSION,
        DesktopSessionCreateRequest, DesktopSessionResponse, FinalizeValidatorChallengeRequest,
        LeaseValidatorChallengeRequest, LeaseValidatorChallengeResponse, PublicStatsSnapshot,
        ScheduleValidatorChallengeRequest, ScheduleValidatorChallengeResponse, ServiceConfig,
        StarterDemandAckRequest, StarterDemandAckResponse, StarterDemandCompleteRequest,
        StarterDemandCompleteResponse, StarterDemandHeartbeatRequest,
        StarterDemandHeartbeatResponse, StarterDemandPollRequest, StarterDemandPollResponse,
        SyncTokenResponse, build_router,
    };

    fn test_config() -> Result<ServiceConfig> {
        Ok(ServiceConfig {
            listen_addr: "127.0.0.1:0".parse()?,
            session_ttl_seconds: 60,
            sync_token_ttl_seconds: 600,
            sync_token_refresh_after_seconds: 120,
            sync_stream_grants: vec![
                "stream.activity_projection.v1".to_string(),
                "stream.earn_job_lifecycle_projection.v1".to_string(),
            ],
            hosted_nexus_relay_url: "wss://nexus.openagents.com/".to_string(),
            receipt_log_path: None,
            kernel_state_path: None,
            compute_enable_forward_physical: true,
            compute_enable_future_cash: true,
            compute_enable_structured_products: true,
            compute_enable_reconciliation_diagnostics: true,
            compute_policy_bundle_id: DEFAULT_COMPUTE_POLICY_BUNDLE_ID.to_string(),
            compute_policy_version: DEFAULT_COMPUTE_POLICY_VERSION.to_string(),
            starter_demand_budget_cap_sats: 500,
            starter_demand_dispatch_interval_seconds: 1,
            starter_demand_request_ttl_seconds: 120,
            starter_demand_max_active_offers_per_session: 1,
            starter_demand_start_confirm_seconds: 5,
            starter_demand_heartbeat_timeout_seconds: 5,
        })
    }

    fn test_config_with_leases(
        start_confirm_seconds: u64,
        request_ttl_seconds: u64,
        heartbeat_timeout_seconds: u64,
    ) -> Result<ServiceConfig> {
        Ok(ServiceConfig {
            starter_demand_start_confirm_seconds: start_confirm_seconds,
            starter_demand_request_ttl_seconds: request_ttl_seconds,
            starter_demand_heartbeat_timeout_seconds: heartbeat_timeout_seconds,
            ..test_config()?
        })
    }

    async fn response_json<T: serde::de::DeserializeOwned>(response: Response) -> Result<T> {
        let bytes = to_bytes(response.into_body(), usize::MAX).await?;
        Ok(serde_json::from_slice(bytes.as_ref())?)
    }

    async fn create_session_token(app: &axum::Router) -> Result<DesktopSessionResponse> {
        create_session_token_for(app, "autopilot-desktop-alpha", Some("npub1alpha")).await
    }

    async fn create_session_token_for(
        app: &axum::Router,
        desktop_client_id: &str,
        bound_nostr_pubkey: Option<&str>,
    ) -> Result<DesktopSessionResponse> {
        let create_request = DesktopSessionCreateRequest {
            desktop_client_id: desktop_client_id.to_string(),
            device_name: Some("Chris MacBook".to_string()),
            bound_nostr_pubkey: bound_nostr_pubkey.map(str::to_string),
            client_version: Some("mvp".to_string()),
        };
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/session/desktop")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&create_request)?))?,
            )
            .await?;
        assert_eq!(response.status(), StatusCode::OK);
        response_json(response).await
    }

    fn authorization(session: &DesktopSessionResponse) -> String {
        format!("Bearer {}", session.access_token)
    }

    fn kernel_policy() -> PolicyContext {
        PolicyContext::default()
    }

    fn kernel_trace(work_unit_id: Option<&str>, contract_id: Option<&str>) -> TraceContext {
        TraceContext {
            work_unit_id: work_unit_id.map(str::to_string),
            contract_id: contract_id.map(str::to_string),
            ..TraceContext::default()
        }
    }

    fn work_unit_request(
        work_unit_id: &str,
        idempotency_key: &str,
        created_at_ms: i64,
    ) -> CreateWorkUnitRequest {
        CreateWorkUnitRequest {
            idempotency_key: idempotency_key.to_string(),
            trace: kernel_trace(Some(work_unit_id), None),
            policy: kernel_policy(),
            work_unit: WorkUnit {
                work_unit_id: work_unit_id.to_string(),
                external_request_id: Some(format!("request.{work_unit_id}")),
                requester_id: Some("buyer.alpha".to_string()),
                provider_id: Some("desktop".to_string()),
                capability: Some("starter.compute.job".to_string()),
                demand_source: Some("starter_demand".to_string()),
                created_at_ms,
                status: WorkUnitStatus::Created,
                quoted_price: Some(Money {
                    asset: Asset::Btc,
                    amount: MoneyAmount::AmountSats(21),
                }),
                metadata: json!({
                    "summary": "Run the provider earn loop job."
                }),
            },
            evidence: Vec::new(),
            hints: ReceiptHints::default(),
        }
    }

    fn contract_request(
        contract_id: &str,
        work_unit_id: &str,
        idempotency_key: &str,
        created_at_ms: i64,
    ) -> CreateContractRequest {
        CreateContractRequest {
            idempotency_key: idempotency_key.to_string(),
            trace: kernel_trace(Some(work_unit_id), Some(contract_id)),
            policy: kernel_policy(),
            contract: Contract {
                contract_id: contract_id.to_string(),
                work_unit_id: work_unit_id.to_string(),
                provider_id: Some("desktop".to_string()),
                created_at_ms,
                status: ContractStatus::Created,
                settlement_asset: Some(Asset::Btc),
                quoted_price: Some(Money {
                    asset: Asset::Btc,
                    amount: MoneyAmount::AmountSats(21),
                }),
                warranty_window_ms: Some(300_000),
                metadata: json!({
                    "provider": "desktop"
                }),
            },
            evidence: Vec::new(),
            hints: ReceiptHints::default(),
        }
    }

    fn submission_request(
        contract_id: &str,
        work_unit_id: &str,
        idempotency_key: &str,
        created_at_ms: i64,
    ) -> SubmitOutputRequest {
        SubmitOutputRequest {
            idempotency_key: idempotency_key.to_string(),
            trace: kernel_trace(Some(work_unit_id), Some(contract_id)),
            policy: kernel_policy(),
            submission: Submission {
                submission_id: format!("submission.{contract_id}"),
                contract_id: contract_id.to_string(),
                work_unit_id: work_unit_id.to_string(),
                created_at_ms,
                status: SubmissionStatus::Received,
                output_ref: Some("file://result.json".to_string()),
                provenance_digest: Some("sha256:submission.alpha".to_string()),
                metadata: json!({
                    "status": "completed"
                }),
            },
            evidence: Vec::new(),
            hints: ReceiptHints::default(),
        }
    }

    fn verdict_request(
        contract_id: &str,
        work_unit_id: &str,
        idempotency_key: &str,
        created_at_ms: i64,
    ) -> FinalizeVerdictRequest {
        let verdict_id = format!("verdict.{contract_id}");
        FinalizeVerdictRequest {
            idempotency_key: idempotency_key.to_string(),
            trace: kernel_trace(Some(work_unit_id), Some(contract_id)),
            policy: kernel_policy(),
            verdict: Verdict {
                verdict_id: verdict_id.clone(),
                contract_id: contract_id.to_string(),
                work_unit_id: work_unit_id.to_string(),
                created_at_ms,
                outcome: VerdictOutcome::Pass,
                verification_tier: Some(VerificationTier::TierOObjective),
                settlement_status: SettlementStatus::Settled,
                reason_code: Some("starter.compute.accepted".to_string()),
                metadata: json!({
                    "settlement": "approved"
                }),
            },
            settlement_link: Some(SettlementLink {
                settlement_id: format!("settlement.{contract_id}"),
                contract_id: contract_id.to_string(),
                work_unit_id: work_unit_id.to_string(),
                verdict_id,
                created_at_ms,
                payment_pointer: Some("wallet:receive:001".to_string()),
                settled_amount: Some(Money {
                    asset: Asset::Btc,
                    amount: MoneyAmount::AmountSats(21),
                }),
                status: SettlementStatus::Settled,
                metadata: json!({
                    "source": "starter_demand"
                }),
            }),
            claim_hook: None,
            evidence: Vec::new(),
            hints: ReceiptHints {
                verification_correlated: Some(false),
                ..ReceiptHints::default()
            },
        }
    }

    fn compute_product_request(
        product_id: &str,
        idempotency_key: &str,
        created_at_ms: i64,
    ) -> CreateComputeProductRequest {
        CreateComputeProductRequest {
            idempotency_key: idempotency_key.to_string(),
            trace: TraceContext::default(),
            policy: kernel_policy(),
            product: ComputeProduct {
                product_id: product_id.to_string(),
                resource_class: "compute".to_string(),
                capacity_unit: "request".to_string(),
                window_spec: "1h".to_string(),
                region_spec: vec!["global".to_string()],
                performance_band: Some("balanced".to_string()),
                sla_terms_ref: Some("sla.compute.launch".to_string()),
                cost_proof_required: false,
                attestation_required: false,
                settlement_mode: ComputeSettlementMode::Physical,
                index_eligible: true,
                status: ComputeProductStatus::Active,
                version: "v1".to_string(),
                created_at_ms,
                taxonomy_version: Some(COMPUTE_LAUNCH_TAXONOMY_VERSION.to_string()),
                capability_envelope: Some(ComputeCapabilityEnvelope {
                    backend_family: Some(ComputeBackendFamily::GptOss),
                    execution_kind: Some(ComputeExecutionKind::LocalInference),
                    compute_family: Some(ComputeFamily::Inference),
                    topology_kind: None,
                    provisioning_kind: None,
                    proof_posture: None,
                    validator_requirements: None,
                    artifact_residency: None,
                    environment_binding: None,
                    checkpoint_binding: None,
                    model_policy: Some("text-generation".to_string()),
                    model_family: Some("llama3.3".to_string()),
                    host_capability: Some(ComputeHostCapability {
                        accelerator_vendor: Some("nvidia".to_string()),
                        accelerator_family: Some("h100".to_string()),
                        memory_gb: Some(80),
                    }),
                    apple_platform: None,
                    gpt_oss_runtime: Some(GptOssRuntimeCapability {
                        runtime_ready: Some(true),
                        model_name: Some("llama3.3".to_string()),
                        quantization: Some("q4_k_m".to_string()),
                    }),
                    latency_ms_p50: Some(400),
                    throughput_per_minute: Some(1_200),
                    concurrency_limit: Some(2),
                }),
                metadata: json!({
                    "summary": "Standardized launch compute product."
                }),
            },
            evidence: Vec::new(),
            hints: ReceiptHints::default(),
        }
    }

    fn compute_environment_package_request(
        environment_ref: &str,
        version: &str,
        idempotency_key: &str,
        created_at_ms: i64,
    ) -> RegisterComputeEnvironmentPackageRequest {
        RegisterComputeEnvironmentPackageRequest {
            idempotency_key: idempotency_key.to_string(),
            trace: TraceContext::default(),
            policy: kernel_policy(),
            package: ComputeEnvironmentPackage {
                environment_ref: environment_ref.to_string(),
                version: version.to_string(),
                family: "evaluation".to_string(),
                display_name: "OpenAgents Math Basic".to_string(),
                owner_id: "openagents".to_string(),
                created_at_ms,
                updated_at_ms: created_at_ms + 1_000,
                status: ComputeEnvironmentPackageStatus::Active,
                description: Some("Reference math environment".to_string()),
                package_digest: Some(format!("sha256:{environment_ref}:{version}")),
                dataset_bindings: vec![ComputeEnvironmentDatasetBinding {
                    dataset_ref: "dataset://math/basic".to_string(),
                    split_ref: Some("validation".to_string()),
                    mount_path: Some("/datasets/math/basic".to_string()),
                    integrity_ref: Some("sha256:dataset.math.basic".to_string()),
                    access_policy_ref: Some("policy://dataset/math/basic".to_string()),
                    required: true,
                    metadata: json!({"format": "jsonl"}),
                }],
                harness: Some(ComputeEnvironmentHarness {
                    harness_ref: "harness://openagents/math/basic".to_string(),
                    runtime_family: "rust-native".to_string(),
                    entrypoint: Some("oa-eval-harness".to_string()),
                    args: vec!["--suite".to_string(), "math-basic".to_string()],
                    sandbox_profile_ref: Some("sandbox://strict".to_string()),
                    evaluator_policy_ref: Some("policy://eval/math/basic".to_string()),
                    time_budget_ms: Some(300_000),
                    metadata: json!({"max_concurrency": 4}),
                }),
                rubric_bindings: vec![ComputeEnvironmentRubricBinding {
                    rubric_ref: "rubric://math/basic".to_string(),
                    score_type: Some("accuracy".to_string()),
                    pass_threshold_bps: Some(9_000),
                    metadata: json!({"top_k": 1}),
                }],
                expected_artifacts: vec![ComputeEnvironmentArtifactExpectation {
                    artifact_kind: "scorecard".to_string(),
                    artifact_ref: Some("artifact://math/basic/scorecard".to_string()),
                    required: true,
                    verification_policy_ref: Some("policy://artifact/scorecard".to_string()),
                    metadata: json!({"schema": "v1"}),
                }],
                policy_refs: vec![
                    "policy://eval/math/basic".to_string(),
                    "policy://artifact/scorecard".to_string(),
                ],
                metadata: json!({"tier": "reference"}),
            },
            evidence: Vec::new(),
            hints: ReceiptHints::default(),
        }
    }

    fn compute_checkpoint_family_policy_request(
        idempotency_key: &str,
        created_at_ms: i64,
    ) -> RegisterComputeCheckpointFamilyPolicyRequest {
        RegisterComputeCheckpointFamilyPolicyRequest {
            idempotency_key: idempotency_key.to_string(),
            trace: TraceContext::default(),
            policy: kernel_policy(),
            policy_record: ComputeCheckpointFamilyPolicy {
                checkpoint_family: "decoder".to_string(),
                version: "2026.03.14".to_string(),
                owner_id: "openagents".to_string(),
                created_at_ms,
                updated_at_ms: created_at_ms + 100,
                status: ComputeRegistryStatus::Active,
                description: Some("Decoder checkpoint policy".to_string()),
                source_family: Some("sft".to_string()),
                default_recovery_posture: Some("warm-resume".to_string()),
                allowed_environment_refs: vec!["env.openagents.math.basic".to_string()],
                validator_policy_ref: Some("policy://validator/training".to_string()),
                retention_policy_ref: Some("policy://retention/checkpoints".to_string()),
                metadata: json!({"tier": "reference"}),
            },
            evidence: Vec::new(),
            hints: ReceiptHints::default(),
        }
    }

    fn compute_validator_policy_request(
        idempotency_key: &str,
        created_at_ms: i64,
    ) -> RegisterComputeValidatorPolicyRequest {
        RegisterComputeValidatorPolicyRequest {
            idempotency_key: idempotency_key.to_string(),
            trace: TraceContext::default(),
            policy: kernel_policy(),
            policy_record: ComputeValidatorPolicy {
                policy_ref: "policy://validator/training".to_string(),
                version: "2026.03.14".to_string(),
                owner_id: "openagents".to_string(),
                created_at_ms,
                updated_at_ms: created_at_ms + 100,
                status: ComputeRegistryStatus::Active,
                validator_pool_ref: "validator-pool.training".to_string(),
                minimum_validator_count: Some(2),
                challenge_window_ms: Some(60_000),
                required_proof_posture: Some(ComputeProofPosture::ChallengeEligible),
                benchmark_package_refs: vec!["benchmark://mmlu/reference".to_string()],
                metadata: json!({"repeat_runs": 2}),
            },
            evidence: Vec::new(),
            hints: ReceiptHints::default(),
        }
    }

    fn compute_benchmark_package_request(
        idempotency_key: &str,
        created_at_ms: i64,
    ) -> RegisterComputeBenchmarkPackageRequest {
        RegisterComputeBenchmarkPackageRequest {
            idempotency_key: idempotency_key.to_string(),
            trace: TraceContext::default(),
            policy: kernel_policy(),
            benchmark_package: ComputeBenchmarkPackage {
                benchmark_package_ref: "benchmark://mmlu/reference".to_string(),
                version: "2026.03.14".to_string(),
                family: "mmlu".to_string(),
                display_name: "Reference MMLU".to_string(),
                owner_id: "openagents".to_string(),
                created_at_ms,
                updated_at_ms: created_at_ms + 100,
                status: ComputeRegistryStatus::Active,
                environment_ref: "env.openagents.math.basic".to_string(),
                environment_version: None,
                benchmark_suite_ref: Some("benchmark://mmlu/pro".to_string()),
                adapter_kind: Some("mmlu_multiple_choice_v1".to_string()),
                evaluator_policy_ref: Some("policy://eval/math/basic".to_string()),
                pass_threshold_bps: Some(9_000),
                required_metric_ids: vec!["accuracy".to_string()],
                artifact_refs: vec!["artifact://benchmarks/mmlu/manifest".to_string()],
                metadata: json!({"repeat_runs": 2}),
            },
            evidence: Vec::new(),
            hints: ReceiptHints::default(),
        }
    }

    fn compute_training_policy_request(
        idempotency_key: &str,
        created_at_ms: i64,
    ) -> RegisterComputeTrainingPolicyRequest {
        RegisterComputeTrainingPolicyRequest {
            idempotency_key: idempotency_key.to_string(),
            trace: TraceContext::default(),
            policy: kernel_policy(),
            training_policy: ComputeTrainingPolicy {
                training_policy_ref: "policy://training/math/basic".to_string(),
                version: "2026.03.14".to_string(),
                owner_id: "openagents".to_string(),
                created_at_ms,
                updated_at_ms: created_at_ms + 100,
                status: ComputeRegistryStatus::Active,
                environment_refs: vec!["env.openagents.math.basic".to_string()],
                checkpoint_family: "decoder".to_string(),
                validator_policy_ref: "policy://validator/training".to_string(),
                benchmark_package_refs: vec!["benchmark://mmlu/reference".to_string()],
                stage_policy_refs: vec![
                    "policy://training/math/basic/general_sft".to_string(),
                    "policy://training/math/basic/agentic_sft".to_string(),
                    "policy://training/math/basic/rl".to_string(),
                ],
                metadata: json!({"curriculum_policy_ref": "policy://curriculum/math/basic"}),
            },
            evidence: Vec::new(),
            hints: ReceiptHints::default(),
        }
    }

    fn compute_evaluation_run_request(
        eval_run_id: &str,
        delivery_proof_id: &str,
        idempotency_key: &str,
        created_at_ms: i64,
    ) -> CreateComputeEvaluationRunRequest {
        CreateComputeEvaluationRunRequest {
            idempotency_key: idempotency_key.to_string(),
            trace: TraceContext::default(),
            policy: kernel_policy(),
            eval_run: ComputeEvaluationRun {
                eval_run_id: eval_run_id.to_string(),
                environment_binding: ComputeEnvironmentBinding {
                    environment_ref: "env.openagents.math.basic".to_string(),
                    environment_version: None,
                    dataset_ref: None,
                    rubric_ref: None,
                    evaluator_policy_ref: None,
                },
                product_id: Some("ollama.text_generation".to_string()),
                capacity_lot_id: Some("lot.compute.client".to_string()),
                instrument_id: Some("instrument.compute.client".to_string()),
                delivery_proof_id: Some(delivery_proof_id.to_string()),
                model_ref: Some("model://llama3.3".to_string()),
                source_ref: Some("artifact://eval/input-bundle".to_string()),
                created_at_ms,
                expected_sample_count: Some(2),
                status: ComputeEvaluationRunStatus::Queued,
                started_at_ms: None,
                finalized_at_ms: None,
                summary: None,
                run_artifacts: Vec::new(),
                metadata: json!({"suite": "math-basic"}),
            },
            evidence: Vec::new(),
            hints: ReceiptHints::default(),
        }
    }

    fn compute_evaluation_sample(
        eval_run_id: &str,
        sample_id: &str,
        ordinal: u64,
        score_bps: u32,
        recorded_at_ms: i64,
    ) -> ComputeEvaluationSample {
        ComputeEvaluationSample {
            eval_run_id: eval_run_id.to_string(),
            sample_id: sample_id.to_string(),
            ordinal: Some(ordinal),
            status: ComputeEvaluationSampleStatus::Scored,
            input_ref: Some(format!("artifact://eval/input/{sample_id}")),
            output_ref: Some(format!("artifact://eval/output/{sample_id}")),
            expected_output_ref: Some(format!("artifact://eval/expected/{sample_id}")),
            score_bps: Some(score_bps),
            metrics: vec![ComputeEvaluationMetric {
                metric_id: "accuracy".to_string(),
                metric_value: score_bps as f64 / 10_000.0,
                unit: Some("fraction".to_string()),
                metadata: json!({"sample_id": sample_id}),
            }],
            artifacts: vec![ComputeEvaluationArtifact {
                artifact_kind: "sample_report".to_string(),
                artifact_ref: format!("artifact://eval/sample/{sample_id}/report"),
                digest: Some(format!("sha256:sample:{sample_id}")),
                metadata: json!({"ordinal": ordinal}),
            }],
            error_reason: None,
            recorded_at_ms,
            metadata: json!({"prompt_tokens": 64}),
        }
    }

    fn append_compute_evaluation_samples_request(
        eval_run_id: &str,
        idempotency_key: &str,
        created_at_ms: i64,
    ) -> AppendComputeEvaluationSamplesRequest {
        AppendComputeEvaluationSamplesRequest {
            idempotency_key: idempotency_key.to_string(),
            trace: TraceContext::default(),
            policy: kernel_policy(),
            eval_run_id: eval_run_id.to_string(),
            samples: vec![
                compute_evaluation_sample(eval_run_id, "sample.alpha", 1, 9_500, created_at_ms),
                compute_evaluation_sample(
                    eval_run_id,
                    "sample.beta",
                    2,
                    8_500,
                    created_at_ms + 100,
                ),
            ],
            evidence: Vec::new(),
            hints: ReceiptHints::default(),
        }
    }

    fn finalize_compute_evaluation_run_request(
        eval_run_id: &str,
        idempotency_key: &str,
        finalized_at_ms: i64,
    ) -> FinalizeComputeEvaluationRunRequest {
        FinalizeComputeEvaluationRunRequest {
            idempotency_key: idempotency_key.to_string(),
            trace: TraceContext::default(),
            policy: kernel_policy(),
            eval_run_id: eval_run_id.to_string(),
            status: ComputeEvaluationRunStatus::Finalized,
            finalized_at_ms,
            artifacts: vec![ComputeEvaluationArtifact {
                artifact_kind: "scorecard".to_string(),
                artifact_ref: "artifact://eval/scorecard".to_string(),
                digest: Some("sha256:scorecard".to_string()),
                metadata: json!({"schema": "v1"}),
            }],
            metadata: json!({"source": "test"}),
            evidence: Vec::new(),
            hints: ReceiptHints::default(),
        }
    }

    fn accept_compute_evaluation_outcome_request(
        eval_run_id: &str,
        idempotency_key: &str,
        accepted_at_ms: i64,
    ) -> AcceptComputeOutcomeRequest {
        AcceptComputeOutcomeRequest {
            idempotency_key: idempotency_key.to_string(),
            trace: TraceContext::default(),
            policy: kernel_policy(),
            outcome: ComputeAcceptedOutcome {
                outcome_id: "accepted.evaluation.client".to_string(),
                outcome_kind: ComputeAcceptedOutcomeKind::EvaluationRun,
                source_run_id: eval_run_id.to_string(),
                environment_binding: ComputeEnvironmentBinding {
                    environment_ref: "env.openagents.math.basic".to_string(),
                    environment_version: Some("2026.03.13".to_string()),
                    dataset_ref: Some("dataset://math/basic".to_string()),
                    rubric_ref: Some("rubric://math/basic".to_string()),
                    evaluator_policy_ref: Some("policy://eval/math/basic".to_string()),
                },
                checkpoint_binding: None,
                validator_policy_ref: Some("policy://validator/training".to_string()),
                benchmark_package_refs: vec!["benchmark://mmlu/reference".to_string()],
                accepted_at_ms,
                evaluation_summary: None,
                training_summary: None,
                metadata: json!({"accepted_by": "client", "review_lane": "evaluation"}),
            },
            evidence: Vec::new(),
            hints: ReceiptHints::default(),
        }
    }

    fn compute_training_run_request(
        idempotency_key: &str,
        created_at_ms: i64,
    ) -> CreateComputeTrainingRunRequest {
        CreateComputeTrainingRunRequest {
            idempotency_key: idempotency_key.to_string(),
            trace: TraceContext::default(),
            policy: kernel_policy(),
            training_run: ComputeTrainingRun {
                training_run_id: "train.math.basic.client".to_string(),
                training_policy_ref: "policy://training/math/basic".to_string(),
                environment_binding: ComputeEnvironmentBinding {
                    environment_ref: "env.openagents.math.basic".to_string(),
                    environment_version: None,
                    dataset_ref: None,
                    rubric_ref: None,
                    evaluator_policy_ref: None,
                },
                checkpoint_binding: ComputeCheckpointBinding {
                    checkpoint_family: "decoder".to_string(),
                    latest_checkpoint_ref: Some("checkpoint://decoder/base".to_string()),
                    recovery_posture: Some("warm-resume".to_string()),
                },
                validator_policy_ref: "policy://validator/training".to_string(),
                benchmark_package_refs: vec!["benchmark://mmlu/reference".to_string()],
                product_id: Some("psionic.training.gradient.elastic".to_string()),
                capacity_lot_id: Some("lot.compute.client".to_string()),
                instrument_id: Some("instrument.compute.client".to_string()),
                delivery_proof_id: Some("delivery.compute.client".to_string()),
                model_ref: Some("model://gpt-oss-20b".to_string()),
                source_ref: Some("artifact://training/math-basic/input".to_string()),
                rollout_verification_eval_run_ids: vec!["eval.compute.client".to_string()],
                created_at_ms,
                started_at_ms: None,
                finalized_at_ms: None,
                expected_step_count: Some(64),
                completed_step_count: None,
                status: ComputeTrainingRunStatus::Queued,
                final_checkpoint_ref: None,
                promotion_checkpoint_ref: None,
                summary: None,
                metadata: json!({"stability_verdict": "continue"}),
            },
            evidence: Vec::new(),
            hints: ReceiptHints::default(),
        }
    }

    fn finalize_compute_training_run_request(
        idempotency_key: &str,
        finalized_at_ms: i64,
    ) -> FinalizeComputeTrainingRunRequest {
        FinalizeComputeTrainingRunRequest {
            idempotency_key: idempotency_key.to_string(),
            trace: TraceContext::default(),
            policy: kernel_policy(),
            training_run_id: "train.math.basic.client".to_string(),
            status: ComputeTrainingRunStatus::Accepted,
            finalized_at_ms,
            final_checkpoint_ref: Some(
                "checkpoint://decoder/train.math.basic.client/final".to_string(),
            ),
            promotion_checkpoint_ref: Some(
                "checkpoint://decoder/train.math.basic.client/promotion".to_string(),
            ),
            summary: Some(ComputeTrainingSummary {
                completed_step_count: Some(64),
                processed_token_count: Some(128_000),
                average_loss: Some(0.42),
                best_eval_score_bps: Some(9_350),
                accepted_checkpoint_ref: Some(
                    "checkpoint://decoder/train.math.basic.client/promotion".to_string(),
                ),
                aggregate_metrics: vec![ComputeEvaluationMetric {
                    metric_id: "accuracy".to_string(),
                    metric_value: 0.935,
                    unit: Some("fraction".to_string()),
                    metadata: json!({"benchmark_package_ref": "benchmark://mmlu/reference"}),
                }],
                artifacts: vec![ComputeEvaluationArtifact {
                    artifact_kind: "training_manifest".to_string(),
                    artifact_ref: "artifact://training/math-basic/manifest".to_string(),
                    digest: Some("sha256:train-manifest".to_string()),
                    metadata: json!({"schema": "v1"}),
                }],
            }),
            metadata: json!({"source": "test"}),
            evidence: Vec::new(),
            hints: ReceiptHints::default(),
        }
    }

    fn accept_compute_training_outcome_request(
        idempotency_key: &str,
        accepted_at_ms: i64,
    ) -> AcceptComputeOutcomeRequest {
        AcceptComputeOutcomeRequest {
            idempotency_key: idempotency_key.to_string(),
            trace: TraceContext::default(),
            policy: kernel_policy(),
            outcome: ComputeAcceptedOutcome {
                outcome_id: "accepted.training.client".to_string(),
                outcome_kind: ComputeAcceptedOutcomeKind::TrainingRun,
                source_run_id: "train.math.basic.client".to_string(),
                environment_binding: ComputeEnvironmentBinding {
                    environment_ref: "env.openagents.math.basic".to_string(),
                    environment_version: Some("2026.03.13".to_string()),
                    dataset_ref: Some("dataset://math/basic".to_string()),
                    rubric_ref: Some("rubric://math/basic".to_string()),
                    evaluator_policy_ref: Some("policy://eval/math/basic".to_string()),
                },
                checkpoint_binding: None,
                validator_policy_ref: None,
                benchmark_package_refs: Vec::new(),
                accepted_at_ms,
                evaluation_summary: None,
                training_summary: None,
                metadata: json!({"accepted_by": "client"}),
            },
            evidence: Vec::new(),
            hints: ReceiptHints::default(),
        }
    }

    fn compute_adapter_policy_revision(
        revision_id: &str,
        produced_at_ms: i64,
    ) -> ComputeAdapterPolicyRevision {
        ComputeAdapterPolicyRevision {
            policy_family: "policy://training/math/basic".to_string(),
            revision_id: revision_id.to_string(),
            revision_number: Some(7),
            policy_digest: format!("sha256:{revision_id}"),
            parent_revision_id: Some("policy-rev-6".to_string()),
            produced_at_ms,
        }
    }

    fn compute_adapter_checkpoint_pointer(
        pointer_digest: &str,
        updated_at_ms: i64,
    ) -> ComputeAdapterCheckpointPointer {
        ComputeAdapterCheckpointPointer {
            scope_kind: "training_run".to_string(),
            scope_id: "train.math.basic.client".to_string(),
            checkpoint_family: "decoder".to_string(),
            checkpoint_ref: "checkpoint://decoder/train.math.basic.client/promotion".to_string(),
            manifest_digest: "sha256:checkpoint-manifest".to_string(),
            updated_at_ms,
            pointer_digest: pointer_digest.to_string(),
        }
    }

    fn compute_adapter_window_request(
        idempotency_key: &str,
        recorded_at_ms: i64,
        accepted_outcome_id: Option<&str>,
    ) -> RecordComputeAdapterWindowRequest {
        let source_policy_revision =
            compute_adapter_policy_revision("policy-rev-7", recorded_at_ms - 50);
        let source_checkpoint_pointer =
            compute_adapter_checkpoint_pointer("sha256:pointer-7", recorded_at_ms - 25);
        RecordComputeAdapterWindowRequest {
            idempotency_key: idempotency_key.to_string(),
            trace: TraceContext::default(),
            policy: kernel_policy(),
            window: ComputeAdapterTrainingWindow {
                window_id: "adapter.window.client".to_string(),
                training_run_id: "train.math.basic.client".to_string(),
                stage_id: "sft".to_string(),
                contributor_set_revision_id: "contributors.rev.client.1".to_string(),
                validator_policy_ref: "policy://validator/training".to_string(),
                adapter_target_id: "adapter.target.client".to_string(),
                adapter_family: "openagents.adapter.reference".to_string(),
                base_model_ref: "model://gpt-oss-20b".to_string(),
                adapter_format: "openagents.adapter.delta.v1".to_string(),
                source_policy_revision: source_policy_revision.clone(),
                source_checkpoint_pointer: source_checkpoint_pointer.clone(),
                status: ComputeAdapterWindowStatus::Reconciled,
                total_contributions: 2,
                admitted_contributions: 2,
                accepted_contributions: 1,
                quarantined_contributions: 0,
                rejected_contributions: 1,
                replay_required_contributions: 0,
                replay_checked_contributions: 1,
                held_out_average_score_bps: Some(9_400),
                benchmark_pass_rate_bps: Some(9_250),
                runtime_smoke_passed: Some(true),
                promotion_ready: true,
                gate_reason_codes: Vec::new(),
                window_summary_digest: "sha256:window-summary-client".to_string(),
                promotion_disposition: Some(ComputeAdapterPromotionDisposition::Promoted),
                hold_reason_codes: Vec::new(),
                aggregated_delta_digest: Some("sha256:adapter-aggregate-client".to_string()),
                output_policy_revision: Some(compute_adapter_policy_revision(
                    "policy-rev-8",
                    recorded_at_ms,
                )),
                output_checkpoint_pointer: Some(compute_adapter_checkpoint_pointer(
                    "sha256:pointer-8",
                    recorded_at_ms,
                )),
                accepted_outcome_id: accepted_outcome_id.map(ToOwned::to_owned),
                recorded_at_ms,
                metadata: json!({"validator_window_id": "validator.window.client"}),
            },
            contribution_outcomes: vec![
                ComputeAdapterContributionOutcome {
                    contribution_id: "contrib.client.alpha".to_string(),
                    training_run_id: "train.math.basic.client".to_string(),
                    stage_id: "sft".to_string(),
                    window_id: "adapter.window.client".to_string(),
                    contributor_set_revision_id: "contributors.rev.client.1".to_string(),
                    assignment_id: "assignment.client.alpha".to_string(),
                    contributor_node_id: "node.client.alpha".to_string(),
                    worker_id: "worker.client.alpha".to_string(),
                    validator_policy_ref: "policy://validator/training".to_string(),
                    adapter_target_id: "adapter.target.client".to_string(),
                    adapter_family: "openagents.adapter.reference".to_string(),
                    base_model_ref: "model://gpt-oss-20b".to_string(),
                    adapter_format: "openagents.adapter.delta.v1".to_string(),
                    dataset_slice: ComputeAdapterDatasetSlice {
                        dataset_id: "dataset://math/basic".to_string(),
                        split_name: "train".to_string(),
                        slice_id: "slice.client.alpha".to_string(),
                        slice_digest: "sha256:slice-client-alpha".to_string(),
                    },
                    source_policy_revision: source_policy_revision.clone(),
                    source_checkpoint_pointer: source_checkpoint_pointer.clone(),
                    submission_receipt_digest: "sha256:submission-client-alpha".to_string(),
                    artifact_id: "artifact.client.alpha".to_string(),
                    manifest_digest: "sha256:manifest-client-alpha".to_string(),
                    object_digest: "sha256:object-client-alpha".to_string(),
                    artifact_receipt_digest: "sha256:artifact-receipt-client-alpha".to_string(),
                    provenance_bundle_digest: "sha256:provenance-client-alpha".to_string(),
                    security_receipt_digest: "sha256:security-client-alpha".to_string(),
                    replay_receipt_digest: Some("sha256:replay-client-alpha".to_string()),
                    validator_disposition: ComputeAdapterContributionDisposition::Accepted,
                    validation_reason_codes: Vec::new(),
                    validator_receipt_digest: "sha256:validator-client-alpha".to_string(),
                    aggregation_eligibility: ComputeAdapterAggregationEligibility::Eligible,
                    accepted_for_aggregation: true,
                    aggregation_weight_bps: Some(10_000),
                    promotion_receipt_digest: Some("sha256:promotion-client".to_string()),
                    recorded_at_ms,
                    metadata: json!({"loss_bps": 420}),
                },
                ComputeAdapterContributionOutcome {
                    contribution_id: "contrib.client.beta".to_string(),
                    training_run_id: "train.math.basic.client".to_string(),
                    stage_id: "sft".to_string(),
                    window_id: "adapter.window.client".to_string(),
                    contributor_set_revision_id: "contributors.rev.client.1".to_string(),
                    assignment_id: "assignment.client.beta".to_string(),
                    contributor_node_id: "node.client.beta".to_string(),
                    worker_id: "worker.client.beta".to_string(),
                    validator_policy_ref: "policy://validator/training".to_string(),
                    adapter_target_id: "adapter.target.client".to_string(),
                    adapter_family: "openagents.adapter.reference".to_string(),
                    base_model_ref: "model://gpt-oss-20b".to_string(),
                    adapter_format: "openagents.adapter.delta.v1".to_string(),
                    dataset_slice: ComputeAdapterDatasetSlice {
                        dataset_id: "dataset://math/basic".to_string(),
                        split_name: "train".to_string(),
                        slice_id: "slice.client.beta".to_string(),
                        slice_digest: "sha256:slice-client-beta".to_string(),
                    },
                    source_policy_revision,
                    source_checkpoint_pointer,
                    submission_receipt_digest: "sha256:submission-client-beta".to_string(),
                    artifact_id: "artifact.client.beta".to_string(),
                    manifest_digest: "sha256:manifest-client-beta".to_string(),
                    object_digest: "sha256:object-client-beta".to_string(),
                    artifact_receipt_digest: "sha256:artifact-receipt-client-beta".to_string(),
                    provenance_bundle_digest: "sha256:provenance-client-beta".to_string(),
                    security_receipt_digest: "sha256:security-client-beta".to_string(),
                    replay_receipt_digest: Some("sha256:replay-client-beta".to_string()),
                    validator_disposition: ComputeAdapterContributionDisposition::Rejected,
                    validation_reason_codes: vec![
                        ComputeAdapterContributionValidationReasonCode::ReplayMismatch,
                    ],
                    validator_receipt_digest: "sha256:validator-client-beta".to_string(),
                    aggregation_eligibility: ComputeAdapterAggregationEligibility::Ineligible,
                    accepted_for_aggregation: false,
                    aggregation_weight_bps: None,
                    promotion_receipt_digest: None,
                    recorded_at_ms,
                    metadata: json!({"loss_bps": 780}),
                },
            ],
            evidence: Vec::new(),
            hints: ReceiptHints::default(),
        }
    }

    fn compute_synthetic_data_job_request(
        synthetic_job_id: &str,
        idempotency_key: &str,
        created_at_ms: i64,
    ) -> CreateComputeSyntheticDataJobRequest {
        CreateComputeSyntheticDataJobRequest {
            idempotency_key: idempotency_key.to_string(),
            trace: TraceContext::default(),
            policy: kernel_policy(),
            synthetic_job: ComputeSyntheticDataJob {
                synthetic_job_id: synthetic_job_id.to_string(),
                environment_binding: ComputeEnvironmentBinding {
                    environment_ref: "env.openagents.math.basic".to_string(),
                    environment_version: None,
                    dataset_ref: None,
                    rubric_ref: None,
                    evaluator_policy_ref: None,
                },
                teacher_model_ref: "model://llama3.3-instruct".to_string(),
                generation_product_id: Some("ollama.text_generation".to_string()),
                generation_delivery_proof_id: Some("delivery.compute.client".to_string()),
                output_artifact_ref: None,
                created_at_ms,
                generated_at_ms: None,
                verification_eval_run_id: None,
                verified_at_ms: None,
                target_sample_count: Some(2),
                status: ComputeSyntheticDataJobStatus::Queued,
                verification_summary: None,
                metadata: json!({"pipeline": "teacher-verify"}),
            },
            evidence: Vec::new(),
            hints: ReceiptHints::default(),
        }
    }

    fn compute_synthetic_data_sample(
        synthetic_job_id: &str,
        sample_id: &str,
        ordinal: u64,
        recorded_at_ms: i64,
    ) -> ComputeSyntheticDataSample {
        ComputeSyntheticDataSample {
            synthetic_job_id: synthetic_job_id.to_string(),
            sample_id: sample_id.to_string(),
            ordinal: Some(ordinal),
            prompt_ref: format!("artifact://synthetic/prompts/{sample_id}"),
            output_ref: format!("artifact://synthetic/outputs/{sample_id}"),
            generation_config_ref: Some("config://synthetic/default".to_string()),
            generator_machine_ref: Some("machine://provider.alpha/gpu0".to_string()),
            verification_eval_sample_id: None,
            verification_status: None,
            verification_score_bps: None,
            status: ComputeSyntheticDataSampleStatus::Generated,
            recorded_at_ms,
            metadata: json!({"prompt_tokens": 64}),
        }
    }

    fn append_compute_synthetic_data_samples_request(
        synthetic_job_id: &str,
        idempotency_key: &str,
        created_at_ms: i64,
    ) -> AppendComputeSyntheticDataSamplesRequest {
        AppendComputeSyntheticDataSamplesRequest {
            idempotency_key: idempotency_key.to_string(),
            trace: TraceContext::default(),
            policy: kernel_policy(),
            synthetic_job_id: synthetic_job_id.to_string(),
            samples: vec![
                compute_synthetic_data_sample(synthetic_job_id, "sample.alpha", 1, created_at_ms),
                compute_synthetic_data_sample(
                    synthetic_job_id,
                    "sample.beta",
                    2,
                    created_at_ms + 100,
                ),
            ],
            evidence: Vec::new(),
            hints: ReceiptHints::default(),
        }
    }

    fn finalize_compute_synthetic_data_generation_request(
        synthetic_job_id: &str,
        idempotency_key: &str,
        generated_at_ms: i64,
    ) -> FinalizeComputeSyntheticDataGenerationRequest {
        FinalizeComputeSyntheticDataGenerationRequest {
            idempotency_key: idempotency_key.to_string(),
            trace: TraceContext::default(),
            policy: kernel_policy(),
            synthetic_job_id: synthetic_job_id.to_string(),
            status: ComputeSyntheticDataJobStatus::Generated,
            generated_at_ms,
            output_artifact_ref: Some(format!("artifact://synthetic/output/{synthetic_job_id}")),
            metadata: json!({"stage": "generation"}),
            evidence: Vec::new(),
            hints: ReceiptHints::default(),
        }
    }

    fn record_compute_synthetic_data_verification_request(
        synthetic_job_id: &str,
        verification_eval_run_id: &str,
        idempotency_key: &str,
        verified_at_ms: i64,
    ) -> RecordComputeSyntheticDataVerificationRequest {
        RecordComputeSyntheticDataVerificationRequest {
            idempotency_key: idempotency_key.to_string(),
            trace: TraceContext::default(),
            policy: kernel_policy(),
            synthetic_job_id: synthetic_job_id.to_string(),
            verification_eval_run_id: verification_eval_run_id.to_string(),
            verified_at_ms,
            metadata: json!({"stage": "verification"}),
            evidence: Vec::new(),
            hints: ReceiptHints::default(),
        }
    }

    fn mmlu_benchmark_import_request(
        eval_run_id: &str,
        idempotency_prefix: &str,
        created_at_ms: i64,
        finalized_at_ms: i64,
    ) -> ComputeBenchmarkImportRequest {
        ComputeBenchmarkImportRequest {
            idempotency_prefix: idempotency_prefix.to_string(),
            trace: TraceContext::default(),
            policy: kernel_policy(),
            adapter_kind: ComputeBenchmarkAdapterKind::MmluMultipleChoiceV1,
            benchmark_family: "mmlu".to_string(),
            benchmark_suite_ref: Some("benchmark://mmlu/pro".to_string()),
            eval_run_id: eval_run_id.to_string(),
            environment_binding: ComputeEnvironmentBinding {
                environment_ref: "env.openagents.math.basic".to_string(),
                environment_version: None,
                dataset_ref: None,
                rubric_ref: None,
                evaluator_policy_ref: None,
            },
            product_id: Some("ollama.text_generation".to_string()),
            capacity_lot_id: Some("lot.compute.client".to_string()),
            instrument_id: Some("instrument.compute.client".to_string()),
            delivery_proof_id: Some("delivery.compute.client".to_string()),
            model_ref: Some("model://llama3.3".to_string()),
            source_ref: Some("artifact://benchmarks/mmlu/input".to_string()),
            created_at_ms,
            finalized_at_ms,
            cases: vec![
                ComputeBenchmarkCaseImport {
                    sample_id: "sample.alpha".to_string(),
                    ordinal: Some(1),
                    input_ref: Some("artifact://benchmarks/mmlu/input/alpha".to_string()),
                    output_ref: Some("artifact://benchmarks/mmlu/output/alpha".to_string()),
                    expected_output_ref: Some(
                        "artifact://benchmarks/mmlu/expected/alpha".to_string(),
                    ),
                    artifacts: Vec::new(),
                    metadata: serde_json::to_value(MmluMultipleChoiceCaseMetadata {
                        subject: "biology".to_string(),
                        choices: vec![
                            "A".to_string(),
                            "B".to_string(),
                            "C".to_string(),
                            "D".to_string(),
                        ],
                        correct_choice_index: 1,
                        predicted_choice_index: 1,
                        prompt_id: Some("bio-1".to_string()),
                    })
                    .expect("mmlu metadata"),
                    recorded_at_ms: created_at_ms + 100,
                },
                ComputeBenchmarkCaseImport {
                    sample_id: "sample.beta".to_string(),
                    ordinal: Some(2),
                    input_ref: Some("artifact://benchmarks/mmlu/input/beta".to_string()),
                    output_ref: Some("artifact://benchmarks/mmlu/output/beta".to_string()),
                    expected_output_ref: Some(
                        "artifact://benchmarks/mmlu/expected/beta".to_string(),
                    ),
                    artifacts: Vec::new(),
                    metadata: serde_json::to_value(MmluMultipleChoiceCaseMetadata {
                        subject: "history".to_string(),
                        choices: vec![
                            "A".to_string(),
                            "B".to_string(),
                            "C".to_string(),
                            "D".to_string(),
                        ],
                        correct_choice_index: 2,
                        predicted_choice_index: 0,
                        prompt_id: Some("hist-1".to_string()),
                    })
                    .expect("mmlu metadata"),
                    recorded_at_ms: created_at_ms + 200,
                },
            ],
            run_artifacts: vec![ComputeEvaluationArtifact {
                artifact_kind: "benchmark_manifest".to_string(),
                artifact_ref: "artifact://benchmarks/mmlu/manifest".to_string(),
                digest: Some("sha256:mmlu-manifest".to_string()),
                metadata: json!({"suite": "mmlu-pro"}),
            }],
            metadata: json!({"split": "test"}),
            hints: ReceiptHints::default(),
        }
    }

    fn capacity_lot_request(
        capacity_lot_id: &str,
        product_id: &str,
        idempotency_key: &str,
        delivery_start_ms: i64,
    ) -> CreateCapacityLotRequest {
        CreateCapacityLotRequest {
            idempotency_key: idempotency_key.to_string(),
            trace: TraceContext::default(),
            policy: kernel_policy(),
            lot: CapacityLot {
                capacity_lot_id: capacity_lot_id.to_string(),
                product_id: product_id.to_string(),
                provider_id: "desktop-provider.alpha".to_string(),
                delivery_start_ms,
                delivery_end_ms: delivery_start_ms + 3_600_000,
                quantity: 10,
                min_unit_price: Some(Money {
                    asset: Asset::Btc,
                    amount: MoneyAmount::AmountSats(150),
                }),
                region_hint: Some("us-central1".to_string()),
                attestation_posture: Some("tpm+quote".to_string()),
                reserve_state: CapacityReserveState::Available,
                offer_expires_at_ms: delivery_start_ms + 300_000,
                status: CapacityLotStatus::Open,
                environment_binding: None,
                metadata: json!({
                    "provider_class": "desktop"
                }),
            },
            evidence: Vec::new(),
            hints: ReceiptHints::default(),
        }
    }

    fn capacity_instrument_request(
        instrument_id: &str,
        product_id: &str,
        capacity_lot_id: &str,
        idempotency_key: &str,
        created_at_ms: i64,
    ) -> CreateCapacityInstrumentRequest {
        CreateCapacityInstrumentRequest {
            idempotency_key: idempotency_key.to_string(),
            trace: TraceContext::default(),
            policy: kernel_policy(),
            instrument: CapacityInstrument {
                instrument_id: instrument_id.to_string(),
                product_id: product_id.to_string(),
                capacity_lot_id: Some(capacity_lot_id.to_string()),
                buyer_id: Some("buyer.compute.alpha".to_string()),
                provider_id: Some("desktop-provider.alpha".to_string()),
                delivery_start_ms: created_at_ms + 60_000,
                delivery_end_ms: created_at_ms + 3_000_000,
                quantity: 10,
                fixed_price: Some(Money {
                    asset: Asset::Btc,
                    amount: MoneyAmount::AmountSats(1_500),
                }),
                reference_index_id: Some("index.compute.alpha".to_string()),
                kind: CapacityInstrumentKind::ForwardPhysical,
                settlement_mode: ComputeSettlementMode::Physical,
                created_at_ms,
                status: CapacityInstrumentStatus::Active,
                environment_binding: None,
                closure_reason: None,
                non_delivery_reason: None,
                settlement_failure_reason: None,
                lifecycle_reason_detail: None,
                metadata: json!({
                    "desk": "forward-physical"
                }),
            },
            evidence: Vec::new(),
            hints: ReceiptHints::default(),
        }
    }

    fn close_capacity_instrument_request(
        instrument_id: &str,
        idempotency_key: &str,
        closed_at_ms: i64,
    ) -> CloseCapacityInstrumentRequest {
        CloseCapacityInstrumentRequest {
            idempotency_key: idempotency_key.to_string(),
            trace: TraceContext::default(),
            policy: kernel_policy(),
            instrument_id: instrument_id.to_string(),
            status: CapacityInstrumentStatus::Defaulted,
            closed_at_ms,
            closure_reason: Some(CapacityInstrumentClosureReason::Defaulted),
            non_delivery_reason: Some(CapacityNonDeliveryReason::ProviderOffline),
            settlement_failure_reason: Some(ComputeSettlementFailureReason::NonDelivery),
            lifecycle_reason_detail: Some(
                "provider went offline before the forward window".to_string(),
            ),
            metadata: json!({
                "requested_by": "buyer.alpha",
                "remedy_profile": "forward_physical.inference.v1"
            }),
            evidence: Vec::new(),
            hints: ReceiptHints::default(),
        }
    }

    fn delivery_proof_request(
        delivery_proof_id: &str,
        product_id: &str,
        capacity_lot_id: &str,
        instrument_id: &str,
        idempotency_key: &str,
        created_at_ms: i64,
    ) -> RecordDeliveryProofRequest {
        RecordDeliveryProofRequest {
            idempotency_key: idempotency_key.to_string(),
            trace: TraceContext::default(),
            policy: kernel_policy(),
            delivery_proof: DeliveryProof {
                delivery_proof_id: delivery_proof_id.to_string(),
                capacity_lot_id: capacity_lot_id.to_string(),
                product_id: product_id.to_string(),
                instrument_id: Some(instrument_id.to_string()),
                contract_id: None,
                created_at_ms,
                metered_quantity: 6,
                accepted_quantity: 6,
                performance_band_observed: Some("sxm".to_string()),
                variance_reason: None,
                variance_reason_detail: None,
                attestation_digest: Some("sha256:attestation.compute.alpha".to_string()),
                cost_attestation_ref: Some("oa://attestations/compute-cost-alpha".to_string()),
                status: DeliveryProofStatus::Accepted,
                rejection_reason: None,
                topology_evidence: None,
                sandbox_evidence: None,
                verification_evidence: None,
                promised_capability_envelope: None,
                observed_capability_envelope: Some(ComputeCapabilityEnvelope {
                    backend_family: Some(ComputeBackendFamily::GptOss),
                    execution_kind: Some(ComputeExecutionKind::LocalInference),
                    compute_family: Some(ComputeFamily::Inference),
                    topology_kind: None,
                    provisioning_kind: None,
                    proof_posture: None,
                    validator_requirements: None,
                    artifact_residency: None,
                    environment_binding: None,
                    checkpoint_binding: None,
                    model_policy: Some("ollama.text_generation.launch".to_string()),
                    model_family: Some("llama3.2:latest".to_string()),
                    host_capability: None,
                    apple_platform: None,
                    gpt_oss_runtime: Some(GptOssRuntimeCapability {
                        runtime_ready: Some(true),
                        model_name: Some("llama3.2:latest".to_string()),
                        quantization: None,
                    }),
                    latency_ms_p50: Some(140),
                    throughput_per_minute: Some(36),
                    concurrency_limit: Some(1),
                }),
                metadata: json!({
                    "sample_count": 12
                }),
            },
            evidence: Vec::new(),
            hints: ReceiptHints::default(),
        }
    }

    fn validator_challenge_request(
        challenge_id: &str,
        delivery_proof_id: Option<&str>,
        created_at_ms: u64,
    ) -> ScheduleValidatorChallengeRequest {
        let mut context = ValidatorChallengeContext::new(
            challenge_id,
            "proof-bundle-digest.alpha",
            "request-digest.alpha",
            "ollama.text_generation",
            "cuda",
            created_at_ms,
        )
        .with_model_id("llama3.2:latest")
        .with_validator_pool_ref("validators.alpha")
        .with_max_attempts(2)
        .with_lease_timeout_ms(250);
        if let Some(delivery_proof_id) = delivery_proof_id {
            context = context.with_delivery_proof_id(delivery_proof_id);
        }
        ScheduleValidatorChallengeRequest {
            idempotency_key: format!("idemp.compute.validator.schedule.{challenge_id}"),
            trace: TraceContext::default(),
            policy: kernel_policy(),
            challenge: ValidatorChallengeRequest::new(
                context,
                GpuFreivaldsMerkleWitness::from_matrices(
                    &[vec![1, 2], vec![3, 4]],
                    &[vec![5, 6], vec![7, 8]],
                    &[vec![19, 22], vec![43, 50]],
                )
                .expect("challenge witness"),
            ),
            evidence: Vec::new(),
            hints: ReceiptHints::default(),
        }
    }

    fn validator_lease_request(
        challenge_id: &str,
        validator_id: &str,
        requested_at_ms: u64,
        idempotency_key: &str,
    ) -> LeaseValidatorChallengeRequest {
        LeaseValidatorChallengeRequest {
            idempotency_key: idempotency_key.to_string(),
            trace: TraceContext::default(),
            policy: kernel_policy(),
            challenge_id: challenge_id.to_string(),
            validator_id: validator_id.to_string(),
            requested_at_ms,
            evidence: Vec::new(),
            hints: ReceiptHints::default(),
        }
    }

    fn validator_result(
        challenge_id: &str,
        finalized_at_ms: u64,
        attempt: u32,
        status: ValidatorChallengeStatus,
        verdict: ValidatorChallengeVerdict,
        detail: &str,
    ) -> ValidatorChallengeResult {
        ValidatorChallengeResult {
            challenge_id: challenge_id.to_string(),
            proof_bundle_digest: "proof-bundle-digest.alpha".to_string(),
            protocol_id: "openagents.validator.gpu_freivalds_merkle.v1".to_string(),
            attempt,
            status,
            verdict,
            reason_code: None,
            detail: detail.to_string(),
            created_at_ms: 0,
            finalized_at_ms,
            challenge_seed_digest: None,
            verified_row_count: None,
            result_digest: format!("sha256:result:{challenge_id}:{attempt}:{finalized_at_ms}"),
            challenge_result_ref: format!(
                "validator_challenge_result:{challenge_id}:{attempt}:{finalized_at_ms}"
            ),
        }
    }

    fn compute_index_request(
        index_id: &str,
        product_id: &str,
        idempotency_key: &str,
        published_at_ms: i64,
    ) -> PublishComputeIndexRequest {
        PublishComputeIndexRequest {
            idempotency_key: idempotency_key.to_string(),
            trace: TraceContext::default(),
            policy: kernel_policy(),
            index: ComputeIndex {
                index_id: index_id.to_string(),
                product_id: product_id.to_string(),
                observation_window_start_ms: published_at_ms - 60_000,
                observation_window_end_ms: published_at_ms + 1,
                published_at_ms,
                observation_count: 0,
                total_accepted_quantity: 0,
                reference_price: Some(Money {
                    asset: Asset::Btc,
                    amount: MoneyAmount::AmountSats(150),
                }),
                methodology: Some("accepted_quantity_vwap".to_string()),
                status: ComputeIndexStatus::Published,
                correction_reason: None,
                corrected_from_index_id: None,
                metadata: json!({
                    "source": "authoritative-kernel"
                }),
            },
            evidence: Vec::new(),
            hints: ReceiptHints::default(),
        }
    }

    fn correct_compute_index_request(
        superseded_index_id: &str,
        corrected_index_id: &str,
        idempotency_key: &str,
        published_at_ms: i64,
        correction_reason: ComputeIndexCorrectionReason,
    ) -> CorrectComputeIndexRequest {
        CorrectComputeIndexRequest {
            idempotency_key: idempotency_key.to_string(),
            trace: TraceContext::default(),
            policy: kernel_policy(),
            superseded_index_id: superseded_index_id.to_string(),
            corrected_index: ComputeIndex {
                index_id: corrected_index_id.to_string(),
                product_id: "ollama.text_generation".to_string(),
                observation_window_start_ms: published_at_ms - 60_000,
                observation_window_end_ms: published_at_ms + 1,
                published_at_ms,
                observation_count: 0,
                total_accepted_quantity: 0,
                reference_price: None,
                methodology: None,
                status: ComputeIndexStatus::Published,
                correction_reason: Some(correction_reason),
                corrected_from_index_id: Some(superseded_index_id.to_string()),
                metadata: json!({
                    "source": "authoritative-kernel"
                }),
            },
            correction_reason,
            evidence: Vec::new(),
            hints: ReceiptHints::default(),
        }
    }

    fn future_cash_instrument_request(
        instrument_id: &str,
        reference_index_id: &str,
        idempotency_key: &str,
        created_at_ms: i64,
        strike_sats_per_unit: u64,
        quantity: u64,
    ) -> CreateCapacityInstrumentRequest {
        CreateCapacityInstrumentRequest {
            idempotency_key: idempotency_key.to_string(),
            trace: TraceContext::default(),
            policy: kernel_policy(),
            instrument: CapacityInstrument {
                instrument_id: instrument_id.to_string(),
                product_id: "ollama.text_generation".to_string(),
                capacity_lot_id: None,
                buyer_id: Some("buyer.hedge.alpha".to_string()),
                provider_id: Some("provider.hedge.alpha".to_string()),
                delivery_start_ms: created_at_ms + 30_000,
                delivery_end_ms: created_at_ms + 60_000,
                quantity,
                fixed_price: Some(Money {
                    asset: Asset::Btc,
                    amount: MoneyAmount::AmountSats(strike_sats_per_unit),
                }),
                reference_index_id: Some(reference_index_id.to_string()),
                kind: CapacityInstrumentKind::FutureCash,
                settlement_mode: ComputeSettlementMode::Cash,
                created_at_ms,
                status: CapacityInstrumentStatus::Open,
                environment_binding: None,
                closure_reason: None,
                non_delivery_reason: None,
                settlement_failure_reason: None,
                lifecycle_reason_detail: None,
                metadata: json!({}),
            },
            evidence: Vec::new(),
            hints: ReceiptHints::default(),
        }
    }

    fn cash_settle_capacity_instrument_request(
        instrument_id: &str,
        idempotency_key: &str,
        settled_at_ms: i64,
    ) -> CashSettleCapacityInstrumentRequest {
        CashSettleCapacityInstrumentRequest {
            idempotency_key: idempotency_key.to_string(),
            trace: TraceContext::default(),
            policy: kernel_policy(),
            instrument_id: instrument_id.to_string(),
            settled_at_ms,
            settlement_index_id: None,
            metadata: json!({}),
            evidence: Vec::new(),
            hints: ReceiptHints::default(),
        }
    }

    fn reservation_instrument_request(
        instrument_id: &str,
        capacity_lot_id: &str,
        idempotency_key: &str,
        created_at_ms: i64,
    ) -> CreateCapacityInstrumentRequest {
        CreateCapacityInstrumentRequest {
            idempotency_key: idempotency_key.to_string(),
            trace: TraceContext::default(),
            policy: kernel_policy(),
            instrument: CapacityInstrument {
                instrument_id: instrument_id.to_string(),
                product_id: "ollama.text_generation".to_string(),
                capacity_lot_id: Some(capacity_lot_id.to_string()),
                buyer_id: Some("buyer.compute.alpha".to_string()),
                provider_id: Some("desktop-provider.alpha".to_string()),
                delivery_start_ms: created_at_ms + 60_000,
                delivery_end_ms: created_at_ms + 180_000,
                quantity: 10,
                fixed_price: Some(Money {
                    asset: Asset::Btc,
                    amount: MoneyAmount::AmountSats(50),
                }),
                reference_index_id: None,
                kind: CapacityInstrumentKind::Reservation,
                settlement_mode: ComputeSettlementMode::BuyerElection,
                created_at_ms,
                status: CapacityInstrumentStatus::Open,
                environment_binding: None,
                closure_reason: None,
                non_delivery_reason: None,
                settlement_failure_reason: None,
                lifecycle_reason_detail: None,
                metadata: json!({
                    "reservation_terms": {
                        "exercise_window_start_ms": created_at_ms + 75_000,
                        "exercise_window_end_ms": created_at_ms + 150_000,
                        "exercise_price": serde_json::to_value(Money {
                            asset: Asset::Btc,
                            amount: MoneyAmount::AmountSats(1_500),
                        }).expect("reservation exercise price")
                    }
                }),
            },
            evidence: Vec::new(),
            hints: ReceiptHints::default(),
        }
    }

    fn structured_reservation_request(
        structured_instrument_id: &str,
        leg_instrument_id: &str,
        idempotency_key: &str,
        created_at_ms: i64,
    ) -> CreateStructuredCapacityInstrumentRequest {
        CreateStructuredCapacityInstrumentRequest {
            idempotency_key: idempotency_key.to_string(),
            trace: TraceContext::default(),
            policy: kernel_policy(),
            structured_instrument: StructuredCapacityInstrument {
                structured_instrument_id: structured_instrument_id.to_string(),
                product_id: "ollama.text_generation".to_string(),
                buyer_id: Some("buyer.compute.alpha".to_string()),
                provider_id: Some("desktop-provider.alpha".to_string()),
                kind: StructuredCapacityInstrumentKind::Reservation,
                created_at_ms,
                status: StructuredCapacityInstrumentStatus::Open,
                lifecycle_reason_detail: None,
                legs: vec![StructuredCapacityLeg {
                    instrument_id: leg_instrument_id.to_string(),
                    role: StructuredCapacityLegRole::ReservationRight,
                    leg_order: 1,
                    metadata: json!({"summary": "reservation right"}),
                }],
                metadata: json!({}),
            },
            evidence: Vec::new(),
            hints: ReceiptHints::default(),
        }
    }

    fn close_structured_capacity_instrument_request(
        structured_instrument_id: &str,
        idempotency_key: &str,
        closed_at_ms: i64,
    ) -> CloseStructuredCapacityInstrumentRequest {
        CloseStructuredCapacityInstrumentRequest {
            idempotency_key: idempotency_key.to_string(),
            trace: TraceContext::default(),
            policy: kernel_policy(),
            structured_instrument_id: structured_instrument_id.to_string(),
            status: StructuredCapacityInstrumentStatus::Cancelled,
            closed_at_ms,
            propagate_to_open_legs: true,
            lifecycle_reason_detail: Some("operator cancelled advanced reservation".to_string()),
            metadata: json!({"source": "test"}),
            evidence: Vec::new(),
            hints: ReceiptHints::default(),
        }
    }

    fn compute_product_wire_request(
        product_id: &str,
        idempotency_key: &str,
        created_at_ms: i64,
    ) -> proto_compute::CreateComputeProductRequest {
        compute_contracts::create_compute_product_request_to_proto(&compute_product_request(
            product_id,
            idempotency_key,
            created_at_ms,
        ))
        .expect("compute product wire request")
    }

    fn capacity_lot_wire_request(
        capacity_lot_id: &str,
        product_id: &str,
        idempotency_key: &str,
        delivery_start_ms: i64,
    ) -> proto_compute::CreateCapacityLotRequest {
        compute_contracts::create_capacity_lot_request_to_proto(&capacity_lot_request(
            capacity_lot_id,
            product_id,
            idempotency_key,
            delivery_start_ms,
        ))
        .expect("capacity lot wire request")
    }

    fn capacity_instrument_wire_request(
        instrument_id: &str,
        product_id: &str,
        capacity_lot_id: &str,
        idempotency_key: &str,
        created_at_ms: i64,
    ) -> proto_compute::CreateCapacityInstrumentRequest {
        compute_contracts::create_capacity_instrument_request_to_proto(
            &capacity_instrument_request(
                instrument_id,
                product_id,
                capacity_lot_id,
                idempotency_key,
                created_at_ms,
            ),
        )
        .expect("capacity instrument wire request")
    }

    fn close_capacity_instrument_wire_request(
        instrument_id: &str,
        idempotency_key: &str,
        closed_at_ms: i64,
    ) -> proto_compute::CloseCapacityInstrumentRequest {
        compute_contracts::close_capacity_instrument_request_to_proto(
            &close_capacity_instrument_request(instrument_id, idempotency_key, closed_at_ms),
        )
        .expect("close capacity instrument wire request")
    }

    fn delivery_proof_wire_request(
        delivery_proof_id: &str,
        product_id: &str,
        capacity_lot_id: &str,
        instrument_id: &str,
        idempotency_key: &str,
        created_at_ms: i64,
    ) -> proto_compute::RecordDeliveryProofRequest {
        compute_contracts::record_delivery_proof_request_to_proto(&delivery_proof_request(
            delivery_proof_id,
            product_id,
            capacity_lot_id,
            instrument_id,
            idempotency_key,
            created_at_ms,
        ))
        .expect("delivery proof wire request")
    }

    fn compute_index_wire_request(
        index_id: &str,
        product_id: &str,
        idempotency_key: &str,
        published_at_ms: i64,
    ) -> proto_compute::PublishComputeIndexRequest {
        compute_contracts::publish_compute_index_request_to_proto(&compute_index_request(
            index_id,
            product_id,
            idempotency_key,
            published_at_ms,
        ))
        .expect("compute index wire request")
    }

    fn correct_compute_index_wire_request(
        superseded_index_id: &str,
        corrected_index_id: &str,
        idempotency_key: &str,
        published_at_ms: i64,
        correction_reason: ComputeIndexCorrectionReason,
    ) -> proto_compute::CorrectComputeIndexRequest {
        compute_contracts::correct_compute_index_request_to_proto(&correct_compute_index_request(
            superseded_index_id,
            corrected_index_id,
            idempotency_key,
            published_at_ms,
            correction_reason,
        ))
        .expect("correct compute index wire request")
    }

    fn cash_settle_capacity_instrument_wire_request(
        instrument_id: &str,
        idempotency_key: &str,
        settled_at_ms: i64,
    ) -> proto_compute::CashSettleCapacityInstrumentRequest {
        compute_contracts::cash_settle_capacity_instrument_request_to_proto(
            &cash_settle_capacity_instrument_request(instrument_id, idempotency_key, settled_at_ms),
        )
        .expect("cash settle capacity instrument wire request")
    }

    fn reservation_instrument_wire_request(
        instrument_id: &str,
        capacity_lot_id: &str,
        idempotency_key: &str,
        created_at_ms: i64,
    ) -> proto_compute::CreateCapacityInstrumentRequest {
        compute_contracts::create_capacity_instrument_request_to_proto(
            &reservation_instrument_request(
                instrument_id,
                capacity_lot_id,
                idempotency_key,
                created_at_ms,
            ),
        )
        .expect("reservation instrument wire request")
    }

    fn structured_reservation_wire_request(
        structured_instrument_id: &str,
        leg_instrument_id: &str,
        idempotency_key: &str,
        created_at_ms: i64,
    ) -> proto_compute::CreateStructuredCapacityInstrumentRequest {
        compute_contracts::create_structured_capacity_instrument_request_to_proto(
            &structured_reservation_request(
                structured_instrument_id,
                leg_instrument_id,
                idempotency_key,
                created_at_ms,
            ),
        )
        .expect("structured reservation wire request")
    }

    fn close_structured_capacity_instrument_wire_request(
        structured_instrument_id: &str,
        idempotency_key: &str,
        closed_at_ms: i64,
    ) -> proto_compute::CloseStructuredCapacityInstrumentRequest {
        compute_contracts::close_structured_capacity_instrument_request_to_proto(
            &close_structured_capacity_instrument_request(
                structured_instrument_id,
                idempotency_key,
                closed_at_ms,
            ),
        )
        .expect("close structured capacity instrument wire request")
    }

    fn data_asset_request(
        asset_id: &str,
        idempotency_key: &str,
        created_at_ms: i64,
    ) -> RegisterDataAssetRequest {
        RegisterDataAssetRequest {
            idempotency_key: idempotency_key.to_string(),
            trace: TraceContext::default(),
            policy: kernel_policy(),
            asset: DataAsset {
                asset_id: asset_id.to_string(),
                provider_id: "provider.data.alpha".to_string(),
                asset_kind: "conversation_bundle".to_string(),
                title: "Claude sessions for repo alpha".to_string(),
                description: Some("Private coding context from prior sessions.".to_string()),
                content_digest: Some("sha256:data.asset.alpha".to_string()),
                provenance_ref: Some("oa://data/assets/alpha/manifest".to_string()),
                default_policy: Some(PermissionPolicy {
                    policy_id: String::new(),
                    allowed_scopes: vec!["read.context".to_string(), "derive.summary".to_string()],
                    allowed_tool_tags: vec!["autopilot".to_string(), "codex".to_string()],
                    allowed_origins: vec!["openagents.com".to_string()],
                    export_allowed: false,
                    derived_outputs_allowed: true,
                    retention_seconds: Some(86_400),
                    max_bundle_size_bytes: Some(32_768),
                    metadata: json!({
                        "classification": "private"
                    }),
                }),
                price_hint: Some(Money {
                    asset: Asset::Btc,
                    amount: MoneyAmount::AmountSats(250),
                }),
                created_at_ms,
                status: DataAssetStatus::Active,
                metadata: json!({
                    "source": "desktop"
                }),
            },
            evidence: Vec::new(),
            hints: ReceiptHints::default(),
        }
    }

    fn access_grant_request(
        grant_id: &str,
        asset_id: &str,
        idempotency_key: &str,
        created_at_ms: i64,
    ) -> CreateAccessGrantRequest {
        CreateAccessGrantRequest {
            idempotency_key: idempotency_key.to_string(),
            trace: TraceContext::default(),
            policy: kernel_policy(),
            grant: AccessGrant {
                grant_id: grant_id.to_string(),
                asset_id: asset_id.to_string(),
                provider_id: "provider.data.alpha".to_string(),
                consumer_id: None,
                permission_policy: PermissionPolicy {
                    policy_id: String::new(),
                    allowed_scopes: vec!["read.context".to_string(), "derive.summary".to_string()],
                    allowed_tool_tags: vec!["autopilot".to_string()],
                    allowed_origins: vec!["openagents.com".to_string()],
                    export_allowed: false,
                    derived_outputs_allowed: true,
                    retention_seconds: Some(7_200),
                    max_bundle_size_bytes: Some(16_384),
                    metadata: json!({
                        "license": "bounded"
                    }),
                },
                offer_price: Some(Money {
                    asset: Asset::Btc,
                    amount: MoneyAmount::AmountSats(250),
                }),
                warranty_window_ms: Some(300_000),
                created_at_ms,
                expires_at_ms: created_at_ms + 300_000,
                accepted_at_ms: None,
                status: AccessGrantStatus::Offered,
                metadata: json!({
                    "channel": "marketplace"
                }),
            },
            evidence: Vec::new(),
            hints: ReceiptHints::default(),
        }
    }

    fn accept_access_grant_request(
        grant_id: &str,
        consumer_id: &str,
        idempotency_key: &str,
        accepted_at_ms: i64,
    ) -> AcceptAccessGrantRequest {
        AcceptAccessGrantRequest {
            idempotency_key: idempotency_key.to_string(),
            trace: TraceContext::default(),
            policy: kernel_policy(),
            grant_id: grant_id.to_string(),
            consumer_id: consumer_id.to_string(),
            accepted_at_ms,
            settlement_price: Some(Money {
                asset: Asset::Btc,
                amount: MoneyAmount::AmountSats(250),
            }),
            metadata: json!({
                "purchase_channel": "accept"
            }),
            evidence: Vec::new(),
            hints: ReceiptHints::default(),
        }
    }

    fn delivery_bundle_request(
        delivery_bundle_id: &str,
        grant_id: &str,
        idempotency_key: &str,
        created_at_ms: i64,
    ) -> IssueDeliveryBundleRequest {
        IssueDeliveryBundleRequest {
            idempotency_key: idempotency_key.to_string(),
            trace: TraceContext::default(),
            policy: kernel_policy(),
            delivery_bundle: DeliveryBundle {
                delivery_bundle_id: delivery_bundle_id.to_string(),
                asset_id: String::new(),
                grant_id: grant_id.to_string(),
                provider_id: String::new(),
                consumer_id: String::new(),
                created_at_ms,
                delivery_ref: "oa://deliveries/data-bundle-alpha".to_string(),
                delivery_digest: Some("sha256:delivery.bundle.alpha".to_string()),
                bundle_size_bytes: Some(8_192),
                manifest_refs: vec!["oa://deliveries/data-bundle-alpha/manifest".to_string()],
                expires_at_ms: Some(created_at_ms + 7_200_000),
                status: DeliveryBundleStatus::Issued,
                metadata: json!({
                    "format": "jsonl"
                }),
            },
            evidence: Vec::new(),
            hints: ReceiptHints::default(),
        }
    }

    fn revoke_access_grant_request(
        revocation_id: &str,
        grant_id: &str,
        idempotency_key: &str,
        created_at_ms: i64,
    ) -> RevokeAccessGrantRequest {
        RevokeAccessGrantRequest {
            idempotency_key: idempotency_key.to_string(),
            trace: TraceContext::default(),
            policy: kernel_policy(),
            revocation: RevocationReceipt {
                revocation_id: revocation_id.to_string(),
                asset_id: String::new(),
                grant_id: grant_id.to_string(),
                provider_id: String::new(),
                consumer_id: None,
                created_at_ms,
                reason_code: "policy_violation_detected".to_string(),
                refund_amount: Some(Money {
                    asset: Asset::Btc,
                    amount: MoneyAmount::AmountSats(250),
                }),
                revoked_delivery_bundle_ids: Vec::new(),
                replacement_delivery_bundle_id: None,
                status: RevocationStatus::Refunded,
                metadata: json!({
                    "remedy": "full_refund"
                }),
            },
            evidence: Vec::new(),
            hints: ReceiptHints::default(),
        }
    }

    fn reserve_partition_request(
        partition_id: &str,
        idempotency_key: &str,
        created_at_ms: i64,
    ) -> RegisterReservePartitionRequest {
        RegisterReservePartitionRequest {
            idempotency_key: idempotency_key.to_string(),
            trace: TraceContext::default(),
            policy: kernel_policy(),
            reserve_partition: ReservePartition {
                partition_id: partition_id.to_string(),
                owner_id: "treasury-router.alpha".to_string(),
                created_at_ms,
                updated_at_ms: created_at_ms,
                total_amount: Money {
                    asset: Asset::Btc,
                    amount: MoneyAmount::AmountSats(10_000),
                },
                available_amount: Money {
                    asset: Asset::Btc,
                    amount: MoneyAmount::AmountSats(10_000),
                },
                reserved_amount: Money {
                    asset: Asset::Btc,
                    amount: MoneyAmount::AmountSats(0),
                },
                status: ReservePartitionStatus::Active,
                metadata: json!({
                    "rail": "lightning"
                }),
            },
            evidence: Vec::new(),
            hints: ReceiptHints::default(),
        }
    }

    fn liquidity_quote_request(
        quote_id: &str,
        idempotency_key: &str,
        created_at_ms: i64,
    ) -> CreateLiquidityQuoteRequest {
        CreateLiquidityQuoteRequest {
            idempotency_key: idempotency_key.to_string(),
            trace: TraceContext::default(),
            policy: kernel_policy(),
            quote: Quote {
                quote_id: quote_id.to_string(),
                requester_id: "buyer.liquidity.alpha".to_string(),
                solver_id: None,
                route_kind: "lightning".to_string(),
                created_at_ms,
                expires_at_ms: created_at_ms + 300_000,
                source_amount: Money {
                    asset: Asset::Btc,
                    amount: MoneyAmount::AmountSats(2_500),
                },
                expected_output: Some(Money {
                    asset: Asset::Btc,
                    amount: MoneyAmount::AmountSats(2_490),
                }),
                fee_ceiling: Some(Money {
                    asset: Asset::Btc,
                    amount: MoneyAmount::AmountSats(25),
                }),
                source_payment_pointer: Some("ln://payer.alpha".to_string()),
                destination_payment_pointer: Some("ln://payee.alpha".to_string()),
                status: QuoteStatus::Quoted,
                metadata: json!({
                    "lane": "earn"
                }),
            },
            evidence: Vec::new(),
            hints: ReceiptHints::default(),
        }
    }

    fn route_plan_request(
        route_plan_id: &str,
        quote_id: &str,
        idempotency_key: &str,
        selected_at_ms: i64,
    ) -> SelectRoutePlanRequest {
        SelectRoutePlanRequest {
            idempotency_key: idempotency_key.to_string(),
            trace: TraceContext::default(),
            policy: kernel_policy(),
            route_plan: RoutePlan {
                route_plan_id: route_plan_id.to_string(),
                quote_id: quote_id.to_string(),
                requester_id: "buyer.liquidity.alpha".to_string(),
                solver_id: "solver.lightning.alpha".to_string(),
                route_kind: "lightning".to_string(),
                selected_at_ms,
                expires_at_ms: selected_at_ms + 300_000,
                quoted_input: None,
                quoted_output: None,
                fee_ceiling: None,
                route_hops: vec!["node-a".to_string(), "node-b".to_string()],
                quote_receipt: None,
                status: RoutePlanStatus::Selected,
                metadata: json!({
                    "selection": "best_fee"
                }),
            },
            evidence: Vec::new(),
            hints: ReceiptHints::default(),
        }
    }

    fn liquidity_envelope_request(
        envelope_id: &str,
        route_plan_id: &str,
        quote_id: &str,
        partition_id: &str,
        idempotency_key: &str,
        issued_at_ms: i64,
    ) -> IssueLiquidityEnvelopeRequest {
        IssueLiquidityEnvelopeRequest {
            idempotency_key: idempotency_key.to_string(),
            trace: TraceContext::default(),
            policy: kernel_policy(),
            envelope: Envelope {
                envelope_id: envelope_id.to_string(),
                route_plan_id: route_plan_id.to_string(),
                quote_id: quote_id.to_string(),
                reserve_partition_id: Some(partition_id.to_string()),
                owner_id: "treasury-router.alpha".to_string(),
                spend_limit: Money {
                    asset: Asset::Btc,
                    amount: MoneyAmount::AmountSats(2_500),
                },
                reserved_amount: Some(Money {
                    asset: Asset::Btc,
                    amount: MoneyAmount::AmountSats(2_500),
                }),
                fee_limit: Some(Money {
                    asset: Asset::Btc,
                    amount: MoneyAmount::AmountSats(25),
                }),
                allowed_destinations: vec!["ln://payee.alpha".to_string()],
                issued_at_ms,
                expires_at_ms: issued_at_ms + 300_000,
                status: EnvelopeStatus::Issued,
                metadata: json!({
                    "policy": "bounded"
                }),
            },
            evidence: Vec::new(),
            hints: ReceiptHints::default(),
        }
    }

    fn settlement_intent_request(
        settlement_intent_id: &str,
        route_plan_id: &str,
        quote_id: &str,
        envelope_id: &str,
        partition_id: &str,
        idempotency_key: &str,
        created_at_ms: i64,
    ) -> ExecuteSettlementIntentRequest {
        ExecuteSettlementIntentRequest {
            idempotency_key: idempotency_key.to_string(),
            trace: TraceContext::default(),
            policy: kernel_policy(),
            settlement_intent: SettlementIntent {
                settlement_intent_id: settlement_intent_id.to_string(),
                route_plan_id: route_plan_id.to_string(),
                quote_id: quote_id.to_string(),
                envelope_id: envelope_id.to_string(),
                reserve_partition_id: Some(partition_id.to_string()),
                created_at_ms,
                executed_at_ms: Some(created_at_ms + 1_000),
                source_amount: Money {
                    asset: Asset::Btc,
                    amount: MoneyAmount::AmountSats(2_500),
                },
                settled_amount: Some(Money {
                    asset: Asset::Btc,
                    amount: MoneyAmount::AmountSats(2_490),
                }),
                fee_paid: Some(Money {
                    asset: Asset::Btc,
                    amount: MoneyAmount::AmountSats(10),
                }),
                settlement_proof_ref: Some("oa://settlements/liquidity-alpha".to_string()),
                reason_code: None,
                status: SettlementIntentStatus::Settled,
                metadata: json!({
                    "rail": "lightning"
                }),
            },
            evidence: Vec::new(),
            hints: ReceiptHints::default(),
        }
    }

    fn adjust_reserve_partition_request(
        partition_id: &str,
        idempotency_key: &str,
        updated_at_ms: i64,
    ) -> AdjustReservePartitionRequest {
        AdjustReservePartitionRequest {
            idempotency_key: idempotency_key.to_string(),
            trace: TraceContext::default(),
            policy: kernel_policy(),
            partition_id: partition_id.to_string(),
            updated_at_ms,
            total_amount: Money {
                asset: Asset::Btc,
                amount: MoneyAmount::AmountSats(7_990),
            },
            available_amount: Money {
                asset: Asset::Btc,
                amount: MoneyAmount::AmountSats(7_990),
            },
            reserved_amount: Money {
                asset: Asset::Btc,
                amount: MoneyAmount::AmountSats(0),
            },
            reason_code: "rebalance".to_string(),
            metadata: json!({
                "operator": "hydra"
            }),
            evidence: Vec::new(),
            hints: ReceiptHints::default(),
        }
    }

    fn coverage_offer_request(
        offer_id: &str,
        outcome_ref: &str,
        idempotency_key: &str,
        created_at_ms: i64,
    ) -> PlaceCoverageOfferRequest {
        PlaceCoverageOfferRequest {
            idempotency_key: idempotency_key.to_string(),
            trace: TraceContext::default(),
            policy: kernel_policy(),
            coverage_offer: CoverageOffer {
                offer_id: offer_id.to_string(),
                outcome_ref: outcome_ref.to_string(),
                contract_id: Some("contract.alpha".to_string()),
                underwriter_id: "underwriter.alpha".to_string(),
                created_at_ms,
                expires_at_ms: created_at_ms + 300_000,
                coverage_cap: Money {
                    asset: Asset::Btc,
                    amount: MoneyAmount::AmountSats(1_500),
                },
                premium: Money {
                    asset: Asset::Btc,
                    amount: MoneyAmount::AmountSats(120),
                },
                deductible: Some(Money {
                    asset: Asset::Btc,
                    amount: MoneyAmount::AmountSats(50),
                }),
                status: CoverageOfferStatus::Open,
                metadata: json!({
                    "lane": "risk"
                }),
            },
            evidence: Vec::new(),
            hints: ReceiptHints::default(),
        }
    }

    fn coverage_binding_request(
        binding_id: &str,
        outcome_ref: &str,
        offer_ids: Vec<String>,
        idempotency_key: &str,
        created_at_ms: i64,
    ) -> BindCoverageRequest {
        BindCoverageRequest {
            idempotency_key: idempotency_key.to_string(),
            trace: TraceContext::default(),
            policy: kernel_policy(),
            coverage_binding: CoverageBinding {
                binding_id: binding_id.to_string(),
                outcome_ref: outcome_ref.to_string(),
                contract_id: Some("contract.alpha".to_string()),
                offer_ids,
                created_at_ms,
                warranty_window_end_ms: Some(created_at_ms + 600_000),
                total_coverage: Money {
                    asset: Asset::Btc,
                    amount: MoneyAmount::AmountSats(0),
                },
                premium_total: Money {
                    asset: Asset::Btc,
                    amount: MoneyAmount::AmountSats(0),
                },
                status: CoverageBindingStatus::Active,
                metadata: json!({
                    "policy_bundle": "policy.nexus.default"
                }),
            },
            evidence: Vec::new(),
            hints: ReceiptHints::default(),
        }
    }

    fn prediction_position_request(
        position_id: &str,
        outcome_ref: &str,
        idempotency_key: &str,
        created_at_ms: i64,
    ) -> CreatePredictionPositionRequest {
        CreatePredictionPositionRequest {
            idempotency_key: idempotency_key.to_string(),
            trace: TraceContext::default(),
            policy: kernel_policy(),
            prediction_position: PredictionPosition {
                position_id: position_id.to_string(),
                outcome_ref: outcome_ref.to_string(),
                participant_id: "agent.predictor.alpha".to_string(),
                side: PredictionSide::Fail,
                created_at_ms,
                expires_at_ms: created_at_ms + 300_000,
                collateral: Money {
                    asset: Asset::Btc,
                    amount: MoneyAmount::AmountSats(600),
                },
                max_payout: Money {
                    asset: Asset::Btc,
                    amount: MoneyAmount::AmountSats(600),
                },
                status: PredictionPositionStatus::Open,
                metadata: json!({
                    "earning_lane": "bet_outcomes"
                }),
            },
            evidence: Vec::new(),
            hints: ReceiptHints::default(),
        }
    }

    fn risk_claim_request(
        claim_id: &str,
        binding_id: &str,
        outcome_ref: &str,
        idempotency_key: &str,
        created_at_ms: i64,
    ) -> CreateRiskClaimRequest {
        CreateRiskClaimRequest {
            idempotency_key: idempotency_key.to_string(),
            trace: TraceContext::default(),
            policy: kernel_policy(),
            risk_claim: RiskClaim {
                claim_id: claim_id.to_string(),
                binding_id: binding_id.to_string(),
                outcome_ref: outcome_ref.to_string(),
                claimant_id: "buyer.alpha".to_string(),
                created_at_ms,
                requested_payout: Money {
                    asset: Asset::Btc,
                    amount: MoneyAmount::AmountSats(900),
                },
                approved_payout: None,
                resolution_ref: None,
                reason_code: "delivery_failed".to_string(),
                status: RiskClaimStatus::Open,
                metadata: json!({
                    "severity": "high"
                }),
            },
            evidence: Vec::new(),
            hints: ReceiptHints::default(),
        }
    }

    fn resolve_risk_claim_request(
        claim_id: &str,
        idempotency_key: &str,
        resolved_at_ms: i64,
    ) -> ResolveRiskClaimRequest {
        ResolveRiskClaimRequest {
            idempotency_key: idempotency_key.to_string(),
            trace: TraceContext::default(),
            policy: kernel_policy(),
            claim_id: claim_id.to_string(),
            resolved_at_ms,
            status: RiskClaimStatus::Paid,
            approved_payout: Some(Money {
                asset: Asset::Btc,
                amount: MoneyAmount::AmountSats(900),
            }),
            resolution_ref: "oa://claims/risk-alpha-resolution".to_string(),
            metadata: json!({
                "resolver": "kernel"
            }),
            evidence: Vec::new(),
            hints: ReceiptHints::default(),
        }
    }

    fn risk_signal_request(
        signal_id: &str,
        outcome_ref: &str,
        idempotency_key: &str,
        created_at_ms: i64,
    ) -> PublishRiskSignalRequest {
        PublishRiskSignalRequest {
            idempotency_key: idempotency_key.to_string(),
            trace: TraceContext::default(),
            policy: kernel_policy(),
            risk_signal: RiskSignal {
                signal_id: signal_id.to_string(),
                outcome_ref: outcome_ref.to_string(),
                created_at_ms,
                implied_fail_probability_bps: 6_200,
                calibration_score: 0.55,
                coverage_concentration_hhi: 0.45,
                verification_tier_floor: None,
                collateral_multiplier_bps: 0,
                autonomy_mode: String::new(),
                status: RiskSignalStatus::Active,
                metadata: json!({
                    "source": "prediction_book"
                }),
            },
            evidence: Vec::new(),
            hints: ReceiptHints::default(),
        }
    }

    #[tokio::test]
    async fn desktop_session_flow_mints_bearer_and_sync_token() -> Result<()> {
        let app = build_router(test_config()?);
        let session = create_session_token(&app).await?;

        let token_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/sync/token")
                    .header("authorization", format!("Bearer {}", session.access_token))
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(token_response.status(), StatusCode::OK);
        let token: SyncTokenResponse = response_json(token_response).await?;
        assert_eq!(token.transport, "spacetime_ws");
        assert_eq!(token.protocol_version, "spacetime.sync.v1");
        assert!(
            token
                .claims
                .scope
                .iter()
                .any(|scope| scope == "sync.subscribe")
        );
        assert_eq!(
            token.claims.stream_grants,
            vec![
                "stream.activity_projection.v1".to_string(),
                "stream.earn_job_lifecycle_projection.v1".to_string()
            ]
        );

        Ok(())
    }

    #[tokio::test]
    async fn sync_token_requires_bearer_auth() -> Result<()> {
        let app = build_router(test_config()?);
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/sync/token")
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        let body: serde_json::Value = response_json(response).await?;
        assert_eq!(body["reason"], "missing_bearer_token");
        Ok(())
    }

    #[tokio::test]
    async fn public_stats_reflects_backend_session_and_sync_receipts() -> Result<()> {
        let app = build_router(test_config()?);

        let empty_stats = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/stats")
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(empty_stats.status(), StatusCode::OK);
        let empty: PublicStatsSnapshot = response_json(empty_stats).await?;
        assert_eq!(empty.receipt_count, 0);
        assert_eq!(empty.sessions_active, 0);
        assert_eq!(empty.sync_tokens_active, 0);

        let empty_api_stats = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/api/stats")
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(empty_api_stats.status(), StatusCode::OK);

        let session = create_session_token(&app).await?;
        let token_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/sync/token")
                    .header("authorization", format!("Bearer {}", session.access_token))
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(token_response.status(), StatusCode::OK);

        let stats_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/stats")
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(stats_response.status(), StatusCode::OK);
        let stats: PublicStatsSnapshot = response_json(stats_response).await?;
        let api_stats_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/api/stats")
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(api_stats_response.status(), StatusCode::OK);
        let api_stats: PublicStatsSnapshot = response_json(api_stats_response).await?;
        assert_eq!(stats.sessions_active, 1);
        assert_eq!(stats.sessions_issued_24h, 1);
        assert_eq!(stats.sync_tokens_active, 1);
        assert_eq!(stats.sync_tokens_issued_24h, 1);
        assert!(stats.receipt_count >= 2);
        assert_eq!(api_stats.receipt_count, stats.receipt_count);
        assert_eq!(api_stats.sessions_active, stats.sessions_active);
        assert_eq!(api_stats.sync_tokens_active, stats.sync_tokens_active);
        assert!(
            stats
                .recent_receipts
                .iter()
                .any(|receipt| receipt.receipt_type == "desktop_session.created")
        );
        assert!(
            stats
                .recent_receipts
                .iter()
                .any(|receipt| receipt.receipt_type == "sync_token.issued")
        );
        Ok(())
    }

    #[tokio::test]
    async fn hosted_starter_demand_dispatches_and_completes_offer() -> Result<()> {
        let app = build_router(test_config()?);
        let session = create_session_token(&app).await?;

        let poll_request = StarterDemandPollRequest {
            provider_nostr_pubkey: Some("npub1alpha".to_string()),
            primary_relay_url: Some("wss://nexus.openagents.com/".to_string()),
        };
        let first_poll = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/starter-demand/poll")
                    .header("authorization", format!("Bearer {}", session.access_token))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&poll_request)?))?,
            )
            .await?;
        assert_eq!(first_poll.status(), StatusCode::OK);
        let first: StarterDemandPollResponse = response_json(first_poll).await?;
        assert!(first.eligible);
        assert_eq!(first.offers.len(), 1);
        let request_id = first.offers[0].request_id.clone();

        let second_poll = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/starter-demand/poll")
                    .header("authorization", format!("Bearer {}", session.access_token))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&poll_request)?))?,
            )
            .await?;
        let second: StarterDemandPollResponse = response_json(second_poll).await?;
        assert_eq!(second.offers.len(), 1);
        assert_eq!(second.offers[0].request_id, request_id);

        let ack_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/starter-demand/offers/{}/ack", request_id))
                    .header("authorization", format!("Bearer {}", session.access_token))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&StarterDemandAckRequest {
                        provider_nostr_pubkey: Some("npub1alpha".to_string()),
                    })?))?,
            )
            .await?;
        assert_eq!(ack_response.status(), StatusCode::OK);
        let acked: StarterDemandAckResponse = response_json(ack_response).await?;
        assert_eq!(acked.request_id, request_id);
        assert_eq!(acked.status, "running");

        let heartbeat_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!(
                        "/api/starter-demand/offers/{}/heartbeat",
                        request_id
                    ))
                    .header("authorization", format!("Bearer {}", session.access_token))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(
                        &StarterDemandHeartbeatRequest {
                            provider_nostr_pubkey: Some("npub1alpha".to_string()),
                        },
                    )?))?,
            )
            .await?;
        assert_eq!(heartbeat_response.status(), StatusCode::OK);
        let heartbeat: StarterDemandHeartbeatResponse = response_json(heartbeat_response).await?;
        assert_eq!(heartbeat.request_id, request_id);
        assert_eq!(heartbeat.status, "running");

        let complete_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!(
                        "/api/starter-demand/offers/{}/complete",
                        request_id
                    ))
                    .header("authorization", format!("Bearer {}", session.access_token))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(
                        &StarterDemandCompleteRequest {
                            payment_pointer: "wallet:receive:001".to_string(),
                        },
                    )?))?,
            )
            .await?;
        assert_eq!(complete_response.status(), StatusCode::OK);
        let completed: StarterDemandCompleteResponse = response_json(complete_response).await?;
        assert_eq!(completed.request_id, request_id);
        assert_eq!(completed.status, "completed");
        assert_eq!(completed.payment_pointer, "wallet:receive:001");

        let stats_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/stats")
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(stats_response.status(), StatusCode::OK);
        let stats: PublicStatsSnapshot = response_json(stats_response).await?;
        assert_eq!(stats.starter_offers_dispatched_24h, 1);
        assert_eq!(stats.starter_offers_started_24h, 1);
        assert_eq!(stats.starter_offer_heartbeats_24h, 1);
        assert_eq!(stats.starter_offers_completed_24h, 1);
        assert_eq!(
            stats.starter_demand_paid_sats_24h,
            first.offers[0].price_sats
        );
        assert_eq!(stats.starter_offers_waiting_ack, 0);
        assert_eq!(stats.starter_offers_running, 0);

        Ok(())
    }

    #[tokio::test]
    async fn hosted_starter_demand_reissues_after_start_confirm_timeout() -> Result<()> {
        let app = build_router(test_config_with_leases(1, 10, 3)?);
        let session_alpha =
            create_session_token_for(&app, "autopilot-desktop-alpha", Some("npub1alpha")).await?;
        let session_beta =
            create_session_token_for(&app, "autopilot-desktop-beta", Some("npub1beta")).await?;

        let alpha_poll = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/starter-demand/poll")
                    .header(
                        "authorization",
                        format!("Bearer {}", session_alpha.access_token),
                    )
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&StarterDemandPollRequest {
                        provider_nostr_pubkey: Some("npub1alpha".to_string()),
                        primary_relay_url: Some("wss://nexus.openagents.com/".to_string()),
                    })?))?,
            )
            .await?;
        assert_eq!(alpha_poll.status(), StatusCode::OK);
        let alpha_payload: StarterDemandPollResponse = response_json(alpha_poll).await?;
        let first_request_id = alpha_payload.offers[0].request_id.clone();

        tokio::time::sleep(std::time::Duration::from_millis(1_100)).await;

        let beta_poll = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/starter-demand/poll")
                    .header(
                        "authorization",
                        format!("Bearer {}", session_beta.access_token),
                    )
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&StarterDemandPollRequest {
                        provider_nostr_pubkey: Some("npub1beta".to_string()),
                        primary_relay_url: Some("wss://nexus.openagents.com/".to_string()),
                    })?))?,
            )
            .await?;
        assert_eq!(beta_poll.status(), StatusCode::OK);
        let beta_payload: StarterDemandPollResponse = response_json(beta_poll).await?;
        assert_eq!(beta_payload.offers.len(), 1);
        assert_ne!(beta_payload.offers[0].request_id, first_request_id);

        Ok(())
    }

    #[tokio::test]
    async fn hosted_starter_demand_reissues_after_heartbeat_timeout() -> Result<()> {
        let app = build_router(test_config_with_leases(1, 10, 1)?);
        let session_alpha =
            create_session_token_for(&app, "autopilot-desktop-alpha", Some("npub1alpha")).await?;
        let session_beta =
            create_session_token_for(&app, "autopilot-desktop-beta", Some("npub1beta")).await?;

        let alpha_poll = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/starter-demand/poll")
                    .header(
                        "authorization",
                        format!("Bearer {}", session_alpha.access_token),
                    )
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&StarterDemandPollRequest {
                        provider_nostr_pubkey: Some("npub1alpha".to_string()),
                        primary_relay_url: Some("wss://nexus.openagents.com/".to_string()),
                    })?))?,
            )
            .await?;
        let alpha_payload: StarterDemandPollResponse = response_json(alpha_poll).await?;
        let first_request_id = alpha_payload.offers[0].request_id.clone();

        let ack_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!(
                        "/api/starter-demand/offers/{}/ack",
                        first_request_id
                    ))
                    .header(
                        "authorization",
                        format!("Bearer {}", session_alpha.access_token),
                    )
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&StarterDemandAckRequest {
                        provider_nostr_pubkey: Some("npub1alpha".to_string()),
                    })?))?,
            )
            .await?;
        assert_eq!(ack_response.status(), StatusCode::OK);

        tokio::time::sleep(std::time::Duration::from_millis(1_100)).await;

        let beta_poll = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/starter-demand/poll")
                    .header(
                        "authorization",
                        format!("Bearer {}", session_beta.access_token),
                    )
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&StarterDemandPollRequest {
                        provider_nostr_pubkey: Some("npub1beta".to_string()),
                        primary_relay_url: Some("wss://nexus.openagents.com/".to_string()),
                    })?))?,
            )
            .await?;
        assert_eq!(beta_poll.status(), StatusCode::OK);
        let beta_payload: StarterDemandPollResponse = response_json(beta_poll).await?;
        assert_eq!(beta_payload.offers.len(), 1);
        assert_ne!(beta_payload.offers[0].request_id, first_request_id);

        Ok(())
    }

    #[tokio::test]
    async fn hosted_starter_demand_requires_hosted_relay_path() -> Result<()> {
        let app = build_router(test_config()?);
        let session = create_session_token(&app).await?;
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/starter-demand/poll")
                    .header("authorization", format!("Bearer {}", session.access_token))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&StarterDemandPollRequest {
                        provider_nostr_pubkey: Some("npub1alpha".to_string()),
                        primary_relay_url: Some("wss://relay.example.com".to_string()),
                    })?))?,
            )
            .await?;
        assert_eq!(response.status(), StatusCode::OK);
        let payload: StarterDemandPollResponse = response_json(response).await?;
        assert!(!payload.eligible);
        assert_eq!(
            payload.reason.as_deref(),
            Some("starter_demand_requires_openagents_hosted_nexus")
        );
        assert!(payload.offers.is_empty());
        Ok(())
    }

    #[tokio::test]
    async fn hosted_starter_demand_requires_autopilot_desktop_session() -> Result<()> {
        let app = build_router(test_config()?);
        let session = create_session_token_for(&app, "desktop-alpha", Some("npub1alpha")).await?;
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/starter-demand/poll")
                    .header("authorization", format!("Bearer {}", session.access_token))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&StarterDemandPollRequest {
                        provider_nostr_pubkey: Some("npub1alpha".to_string()),
                        primary_relay_url: Some("wss://nexus.openagents.com/".to_string()),
                    })?))?,
            )
            .await?;
        assert_eq!(response.status(), StatusCode::OK);
        let payload: StarterDemandPollResponse = response_json(response).await?;
        assert!(!payload.eligible);
        assert_eq!(
            payload.reason.as_deref(),
            Some("starter_demand_requires_autopilot_desktop_session")
        );
        Ok(())
    }

    #[tokio::test]
    async fn hosted_starter_demand_requires_bound_nostr_identity() -> Result<()> {
        let app = build_router(test_config()?);
        let session = create_session_token_for(&app, "autopilot-desktop-alpha", None).await?;
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/starter-demand/poll")
                    .header("authorization", format!("Bearer {}", session.access_token))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&StarterDemandPollRequest {
                        provider_nostr_pubkey: Some("npub1alpha".to_string()),
                        primary_relay_url: Some("wss://nexus.openagents.com/".to_string()),
                    })?))?,
            )
            .await?;
        assert_eq!(response.status(), StatusCode::OK);
        let payload: StarterDemandPollResponse = response_json(response).await?;
        assert!(!payload.eligible);
        assert_eq!(
            payload.reason.as_deref(),
            Some("starter_demand_requires_bound_nostr_identity")
        );
        Ok(())
    }

    #[tokio::test]
    async fn hosted_starter_demand_requires_matching_provider_nostr_pubkey() -> Result<()> {
        let app = build_router(test_config()?);
        let session = create_session_token(&app).await?;

        let poll_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/starter-demand/poll")
                    .header("authorization", format!("Bearer {}", session.access_token))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&StarterDemandPollRequest {
                        provider_nostr_pubkey: Some("npub1alpha".to_string()),
                        primary_relay_url: Some("wss://nexus.openagents.com/".to_string()),
                    })?))?,
            )
            .await?;
        assert_eq!(poll_response.status(), StatusCode::OK);
        let payload: StarterDemandPollResponse = response_json(poll_response).await?;
        let request_id = payload.offers[0].request_id.clone();

        let ack_response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/starter-demand/offers/{request_id}/ack"))
                    .header("authorization", format!("Bearer {}", session.access_token))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&StarterDemandAckRequest {
                        provider_nostr_pubkey: Some("npub1mismatch".to_string()),
                    })?))?,
            )
            .await?;
        assert_eq!(ack_response.status(), StatusCode::FORBIDDEN);
        let body: serde_json::Value = response_json(ack_response).await?;
        assert_eq!(
            body["reason"].as_str(),
            Some("starter_demand_provider_nostr_pubkey_mismatch")
        );
        Ok(())
    }

    #[tokio::test]
    async fn receipt_log_reloads_backend_receipts_when_configured() -> Result<()> {
        let receipt_log_path = std::env::temp_dir().join(format!(
            "nexus-control-receipts-{}.jsonl",
            super::random_token()
        ));
        let _ = std::fs::remove_file(receipt_log_path.as_path());

        let mut config = test_config()?;
        config.receipt_log_path = Some(receipt_log_path.clone());

        let app = build_router(config.clone());
        let _session = create_session_token(&app).await?;

        let log_contents = std::fs::read_to_string(receipt_log_path.as_path())?;
        assert!(log_contents.contains("\"receipt_type\":\"desktop_session.created\""));

        let reloaded_app = build_router(config);
        let stats_response = reloaded_app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/stats")
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(stats_response.status(), StatusCode::OK);
        let stats: PublicStatsSnapshot = response_json(stats_response).await?;
        assert!(stats.receipt_persistence_enabled);
        assert_eq!(stats.sessions_issued_24h, 1);
        assert_eq!(stats.receipt_count, 1);

        let _ = std::fs::remove_file(receipt_log_path.as_path());
        Ok(())
    }

    #[tokio::test]
    async fn kernel_authority_flow_persists_receipts_and_snapshot() -> Result<()> {
        let app = build_router(test_config()?);
        let session = create_session_token(&app).await?;
        let created_at_ms = 1_762_000_101_234i64;
        let minute_start_ms = floor_to_minute_utc(created_at_ms);

        let work_unit = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/kernel/work_units")
                    .header("authorization", authorization(&session))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&work_unit_request(
                        "work_unit.alpha",
                        "idemp.work.alpha",
                        created_at_ms,
                    ))?))?,
            )
            .await?;
        assert_eq!(work_unit.status(), StatusCode::OK);
        let work_unit_payload: CreateWorkUnitResponse = response_json(work_unit).await?;
        assert_eq!(
            work_unit_payload.receipt.receipt_type,
            "kernel.work_unit.create.v1"
        );
        assert_eq!(work_unit_payload.work_unit.work_unit_id, "work_unit.alpha");

        let contract = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/kernel/contracts")
                    .header("authorization", authorization(&session))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&contract_request(
                        "contract.alpha",
                        "work_unit.alpha",
                        "idemp.contract.alpha",
                        created_at_ms + 1_000,
                    ))?))?,
            )
            .await?;
        assert_eq!(contract.status(), StatusCode::OK);
        let contract_payload: CreateContractResponse = response_json(contract).await?;
        assert_eq!(
            contract_payload.receipt.receipt_type,
            "kernel.contract.create.v1"
        );
        assert_eq!(contract_payload.contract.contract_id, "contract.alpha");

        let submission = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/kernel/contracts/contract.alpha/submit")
                    .header("authorization", authorization(&session))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&submission_request(
                        "contract.alpha",
                        "work_unit.alpha",
                        "idemp.submit.alpha",
                        created_at_ms + 2_000,
                    ))?))?,
            )
            .await?;
        assert_eq!(submission.status(), StatusCode::OK);
        let submission_payload: SubmitOutputResponse = response_json(submission).await?;
        assert_eq!(
            submission_payload.receipt.receipt_type,
            "kernel.output.submit.v1"
        );
        assert_eq!(
            submission_payload.submission.submission_id,
            "submission.contract.alpha"
        );

        let verdict = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/kernel/contracts/contract.alpha/verdict/finalize")
                    .header("authorization", authorization(&session))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&verdict_request(
                        "contract.alpha",
                        "work_unit.alpha",
                        "idemp.verdict.alpha",
                        created_at_ms + 3_000,
                    ))?))?,
            )
            .await?;
        assert_eq!(verdict.status(), StatusCode::OK);
        let verdict_payload: FinalizeVerdictResponse = response_json(verdict).await?;
        assert_eq!(
            verdict_payload.receipt.receipt_type,
            "kernel.verdict.finalize.v1"
        );
        assert_eq!(verdict_payload.verdict.verdict_id, "verdict.contract.alpha");
        assert_eq!(
            verdict_payload
                .settlement_link
                .as_ref()
                .map(|settlement| settlement.settlement_id.as_str()),
            Some("settlement.contract.alpha")
        );

        let receipt_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!(
                        "/v1/kernel/receipts/{}",
                        verdict_payload.receipt.receipt_id
                    ))
                    .header("authorization", authorization(&session))
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(receipt_response.status(), StatusCode::OK);
        let canonical_receipt: Receipt = response_json(receipt_response).await?;
        assert_eq!(
            canonical_receipt.canonical_hash,
            verdict_payload.receipt.canonical_hash
        );

        let snapshot_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!("/v1/kernel/snapshots/{minute_start_ms}"))
                    .header("authorization", authorization(&session))
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(snapshot_response.status(), StatusCode::OK);
        let snapshot: super::EconomySnapshot = response_json(snapshot_response).await?;
        assert_eq!(snapshot.as_of_ms, minute_start_ms);
        assert_eq!(snapshot.n, 1);
        assert_eq!(snapshot.nv, 1.0);
        assert_eq!(snapshot.sv, 1.0);
        assert_eq!(snapshot.sv_effective, 1.0);
        assert_eq!(snapshot.inputs.len(), 4);

        Ok(())
    }

    #[tokio::test]
    async fn compute_market_flow_persists_authoritative_objects_and_metrics() -> Result<()> {
        let app = build_router(test_config()?);
        let session = create_session_token(&app).await?;
        let minute_start_ms =
            floor_to_minute_utc((super::now_unix_ms() as i64).saturating_sub(30_000));
        let created_at_ms = minute_start_ms.saturating_add(10_000);

        let product = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/kernel/compute/products")
                    .header("authorization", authorization(&session))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(
                        &compute_product_wire_request(
                            "ollama.text_generation",
                            "idemp.compute.product.alpha",
                            created_at_ms,
                        ),
                    )?))?,
            )
            .await?;
        assert_eq!(product.status(), StatusCode::OK);
        let product_payload = compute_contracts::create_compute_product_response_from_proto(
            &response_json::<proto_compute::CreateComputeProductResponse>(product).await?,
        )?;
        assert_eq!(
            product_payload.receipt.receipt_type,
            "kernel.compute.product.create.v1"
        );

        let lot = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/kernel/compute/lots")
                    .header("authorization", authorization(&session))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&capacity_lot_wire_request(
                        "lot.compute.alpha",
                        "ollama.text_generation",
                        "idemp.compute.lot.alpha",
                        created_at_ms + 1_000,
                    ))?))?,
            )
            .await?;
        assert_eq!(lot.status(), StatusCode::OK);
        let lot_payload = compute_contracts::create_capacity_lot_response_from_proto(
            &response_json::<proto_compute::CreateCapacityLotResponse>(lot).await?,
        )?;
        assert_eq!(
            lot_payload.receipt.receipt_type,
            "kernel.compute.lot.create.v1"
        );

        let instrument = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/kernel/compute/instruments")
                    .header("authorization", authorization(&session))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(
                        &capacity_instrument_wire_request(
                            "instrument.compute.alpha",
                            "ollama.text_generation",
                            "lot.compute.alpha",
                            "idemp.compute.instrument.alpha",
                            created_at_ms + 2_000,
                        ),
                    )?))?,
            )
            .await?;
        assert_eq!(instrument.status(), StatusCode::OK);
        let instrument_payload = compute_contracts::create_capacity_instrument_response_from_proto(
            &response_json::<proto_compute::CreateCapacityInstrumentResponse>(instrument).await?,
        )?;
        assert_eq!(
            instrument_payload.receipt.receipt_type,
            "kernel.compute.instrument.create.v1"
        );

        let delivery = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/kernel/compute/lots/lot.compute.alpha/delivery_proofs")
                    .header("authorization", authorization(&session))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(
                        &delivery_proof_wire_request(
                            "delivery.compute.alpha",
                            "ollama.text_generation",
                            "lot.compute.alpha",
                            "instrument.compute.alpha",
                            "idemp.compute.delivery.alpha",
                            created_at_ms + 3_000,
                        ),
                    )?))?,
            )
            .await?;
        assert_eq!(delivery.status(), StatusCode::OK);
        let delivery_payload = compute_contracts::record_delivery_proof_response_from_proto(
            &response_json::<proto_compute::RecordDeliveryProofResponse>(delivery).await?,
        )?;
        assert_eq!(
            delivery_payload.receipt.receipt_type,
            "kernel.compute.delivery.record.v1"
        );

        let index = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/kernel/compute/indices")
                    .header("authorization", authorization(&session))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(
                        &compute_index_wire_request(
                            "index.compute.alpha",
                            "ollama.text_generation",
                            "idemp.compute.index.alpha",
                            created_at_ms + 4_000,
                        ),
                    )?))?,
            )
            .await?;
        assert_eq!(index.status(), StatusCode::OK);
        let index_payload = compute_contracts::publish_compute_index_response_from_proto(
            &response_json::<proto_compute::PublishComputeIndexResponse>(index).await?,
        )?;
        assert_eq!(
            index_payload.receipt.receipt_type,
            "kernel.compute.index.publish.v1"
        );
        assert_eq!(index_payload.index.observation_count, 1);
        assert_eq!(index_payload.index.total_accepted_quantity, 6);
        assert_eq!(index_payload.index.reference_price, None);
        assert_eq!(
            index_payload
                .index
                .metadata
                .get("quality")
                .and_then(serde_json::Value::as_object)
                .and_then(|quality| quality.get("thin_market"))
                .and_then(serde_json::Value::as_bool),
            Some(true)
        );

        let snapshot_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!("/v1/kernel/snapshots/{minute_start_ms}"))
                    .header("authorization", authorization(&session))
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(snapshot_response.status(), StatusCode::OK);
        let snapshot: super::EconomySnapshot = response_json(snapshot_response).await?;
        assert_eq!(snapshot.compute_products_active, 1);
        assert_eq!(snapshot.compute_capacity_lots_open, 0);
        assert_eq!(snapshot.compute_capacity_lots_delivering, 1);
        assert_eq!(snapshot.compute_instruments_active, 1);
        assert_eq!(snapshot.compute_delivery_proofs_24h, 1);
        assert_eq!(snapshot.compute_delivery_quantity_24h, 6);
        assert_eq!(snapshot.compute_indices_published_24h, 1);
        assert_eq!(snapshot.compute_index_corrections_24h, 0);
        assert_eq!(snapshot.compute_index_thin_windows_24h, 1);
        assert_eq!(snapshot.compute_index_settlement_eligible_24h, 0);

        let stats_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/stats")
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(stats_response.status(), StatusCode::OK);
        let stats: PublicStatsSnapshot = response_json(stats_response).await?;
        assert_eq!(stats.compute_products_active, 1);
        assert_eq!(stats.compute_capacity_lots_open, 0);
        assert_eq!(stats.compute_capacity_lots_delivering, 1);
        assert_eq!(stats.compute_instruments_active, 1);
        assert_eq!(stats.compute_delivery_proofs_24h, 1);
        assert_eq!(stats.compute_delivery_quantity_24h, 6);
        assert_eq!(stats.compute_indices_published_24h, 1);
        assert_eq!(stats.compute_index_corrections_24h, 0);
        assert_eq!(stats.compute_index_thin_windows_24h, 1);
        assert_eq!(stats.compute_index_settlement_eligible_24h, 0);
        assert!(
            stats
                .recent_receipts
                .iter()
                .any(|receipt| { receipt.receipt_type == "kernel.compute.product.created" })
        );
        assert!(
            stats
                .recent_receipts
                .iter()
                .any(|receipt| { receipt.receipt_type == "kernel.compute.delivery.recorded" })
        );

        Ok(())
    }

    #[tokio::test]
    async fn validator_challenge_routes_schedule_lease_finalize_and_list() -> Result<()> {
        let app = build_router(test_config()?);
        let session = create_session_token(&app).await?;
        let created_at_ms = (super::now_unix_ms() as i64).saturating_sub(30_000);

        let product = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/kernel/compute/products")
                    .header("authorization", authorization(&session))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(
                        &compute_product_wire_request(
                            "ollama.text_generation",
                            "idemp.compute.product.validator-route",
                            created_at_ms,
                        ),
                    )?))?,
            )
            .await?;
        assert_eq!(product.status(), StatusCode::OK);

        let lot = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/kernel/compute/lots")
                    .header("authorization", authorization(&session))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&capacity_lot_wire_request(
                        "lot.compute.validator-route",
                        "ollama.text_generation",
                        "idemp.compute.lot.validator-route",
                        created_at_ms + 1_000,
                    ))?))?,
            )
            .await?;
        assert_eq!(lot.status(), StatusCode::OK);

        let instrument = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/kernel/compute/instruments")
                    .header("authorization", authorization(&session))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(
                        &capacity_instrument_wire_request(
                            "instrument.compute.validator-route",
                            "ollama.text_generation",
                            "lot.compute.validator-route",
                            "idemp.compute.instrument.validator-route",
                            created_at_ms + 2_000,
                        ),
                    )?))?,
            )
            .await?;
        assert_eq!(instrument.status(), StatusCode::OK);

        let mut delivery_request = delivery_proof_request(
            "delivery.compute.validator-route",
            "ollama.text_generation",
            "lot.compute.validator-route",
            "instrument.compute.validator-route",
            "idemp.compute.delivery.validator-route",
            created_at_ms + 3_000,
        );
        delivery_request.delivery_proof.verification_evidence = Some(
            openagents_kernel_core::compute::DeliveryVerificationEvidence {
                proof_bundle_ref: Some("proof_bundle:cluster".to_string()),
                activation_fingerprint_ref: None,
                validator_pool_ref: Some("validators.alpha".to_string()),
                validator_run_ref: None,
                challenge_result_refs: Vec::new(),
                environment_ref: None,
                environment_version: None,
                eval_run_ref: None,
            },
        );
        let delivery = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/kernel/compute/lots/lot.compute.validator-route/delivery_proofs")
                    .header("authorization", authorization(&session))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(
                        &compute_contracts::record_delivery_proof_request_to_proto(
                            &delivery_request,
                        )?,
                    )?))?,
            )
            .await?;
        assert_eq!(delivery.status(), StatusCode::OK);

        let schedule = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/kernel/compute/validator_challenges")
                    .header("authorization", authorization(&session))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(
                        &validator_challenge_request(
                            "challenge.compute.validator-route",
                            Some("delivery.compute.validator-route"),
                            (created_at_ms + 4_000) as u64,
                        ),
                    )?))?,
            )
            .await?;
        assert_eq!(schedule.status(), StatusCode::OK);
        let scheduled: ScheduleValidatorChallengeResponse = response_json(schedule).await?;
        assert_eq!(scheduled.challenge.status, ValidatorChallengeStatus::Queued);

        let queued_list = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/v1/kernel/compute/validator_challenges?status=queued")
                    .header("authorization", authorization(&session))
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(queued_list.status(), StatusCode::OK);
        let queued: Vec<openagents_kernel_core::compute::ComputeValidatorChallengeSnapshot> =
            response_json(queued_list).await?;
        assert_eq!(queued.len(), 1);
        assert_eq!(
            queued[0].request.context.challenge_id,
            "challenge.compute.validator-route"
        );

        let lease = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/kernel/compute/validator_challenges/challenge.compute.validator-route/lease")
                    .header("authorization", authorization(&session))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&validator_lease_request(
                        "challenge.compute.validator-route",
                        "validator.route",
                        (created_at_ms + 5_000) as u64,
                        "idemp.compute.validator.lease.route",
                    ))?))?,
            )
            .await?;
        assert_eq!(lease.status(), StatusCode::OK);
        let leased: LeaseValidatorChallengeResponse = response_json(lease).await?;
        assert_eq!(leased.challenge.status, ValidatorChallengeStatus::Leased);

        let finalize = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/kernel/compute/validator_challenges/challenge.compute.validator-route/finalize")
                    .header("authorization", authorization(&session))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(
                        &FinalizeValidatorChallengeRequest {
                            idempotency_key: "idemp.compute.validator.finalize.route".to_string(),
                            trace: TraceContext::default(),
                            policy: kernel_policy(),
                            lease: leased.lease.clone(),
                            result: validator_result(
                                "challenge.compute.validator-route",
                                (created_at_ms + 6_000) as u64,
                                leased.lease.attempt,
                                ValidatorChallengeStatus::Verified,
                                ValidatorChallengeVerdict::Verified,
                                "validator verified the claimed matrix product",
                            ),
                            evidence: Vec::new(),
                            hints: ReceiptHints::default(),
                        },
                    )?))?,
            )
            .await?;
        assert_eq!(finalize.status(), StatusCode::OK);
        let finalized: super::FinalizeValidatorChallengeResponse = response_json(finalize).await?;
        assert_eq!(
            finalized.challenge.status,
            ValidatorChallengeStatus::Verified
        );

        let challenge = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(
                        "/v1/kernel/compute/validator_challenges/challenge.compute.validator-route",
                    )
                    .header("authorization", authorization(&session))
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(challenge.status(), StatusCode::OK);
        let challenge: openagents_kernel_core::compute::ComputeValidatorChallengeSnapshot =
            response_json(challenge).await?;
        assert_eq!(
            challenge.status,
            openagents_kernel_core::compute::ComputeValidatorChallengeStatus::Verified
        );
        assert_eq!(
            challenge.final_result.as_ref().map(|result| result.verdict),
            Some(openagents_kernel_core::compute::ComputeValidatorChallengeVerdict::Verified)
        );

        let delivery = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/v1/kernel/compute/delivery_proofs/delivery.compute.validator-route")
                    .header("authorization", authorization(&session))
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(delivery.status(), StatusCode::OK);
        let delivery = compute_contracts::get_delivery_proof_response_from_proto(
            &response_json::<proto_compute::GetDeliveryProofResponse>(delivery).await?,
        )?;
        assert_eq!(delivery.status, DeliveryProofStatus::Accepted);
        assert_eq!(
            delivery
                .verification_evidence
                .as_ref()
                .map(|evidence| evidence.challenge_result_refs.len()),
            Some(1)
        );

        let stats_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/stats")
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(stats_response.status(), StatusCode::OK);
        let stats: PublicStatsSnapshot = response_json(stats_response).await?;
        assert_eq!(stats.compute_validator_challenges_verified_24h, 1);

        Ok(())
    }

    #[tokio::test]
    async fn compute_close_instrument_route_emits_remedy_receipt() -> Result<()> {
        let app = build_router(test_config()?);
        let session = create_session_token(&app).await?;
        let created_at_ms = super::now_unix_ms() as i64;

        let product = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/kernel/compute/products")
                    .header("authorization", authorization(&session))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(
                        &compute_product_wire_request(
                            "ollama.text_generation",
                            "idemp.compute.product.close-route",
                            created_at_ms,
                        ),
                    )?))?,
            )
            .await?;
        assert_eq!(product.status(), StatusCode::OK);

        let lot = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/kernel/compute/lots")
                    .header("authorization", authorization(&session))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&capacity_lot_wire_request(
                        "lot.compute.close-route",
                        "ollama.text_generation",
                        "idemp.compute.lot.close-route",
                        created_at_ms + 1_000,
                    ))?))?,
            )
            .await?;
        assert_eq!(lot.status(), StatusCode::OK);

        let instrument = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/kernel/compute/instruments")
                    .header("authorization", authorization(&session))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(
                        &capacity_instrument_wire_request(
                            "instrument.compute.close-route",
                            "ollama.text_generation",
                            "lot.compute.close-route",
                            "idemp.compute.instrument.close-route",
                            created_at_ms + 2_000,
                        ),
                    )?))?,
            )
            .await?;
        assert_eq!(instrument.status(), StatusCode::OK);

        let closed = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/kernel/compute/instruments/instrument.compute.close-route/close")
                    .header("authorization", authorization(&session))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(
                        &close_capacity_instrument_wire_request(
                            "instrument.compute.close-route",
                            "idemp.compute.instrument.close-route.close",
                            created_at_ms + 3_000,
                        ),
                    )?))?,
            )
            .await?;
        assert_eq!(closed.status(), StatusCode::OK);
        let closed_payload = compute_contracts::close_capacity_instrument_response_from_proto(
            &response_json::<proto_compute::CloseCapacityInstrumentResponse>(closed).await?,
        )?;
        assert_eq!(
            closed_payload.receipt.receipt_type,
            "kernel.compute.instrument.close.v1"
        );
        assert_eq!(
            closed_payload.instrument.closure_reason,
            Some(CapacityInstrumentClosureReason::Defaulted)
        );

        Ok(())
    }

    #[tokio::test]
    async fn compute_index_correction_route_supersedes_prior_index_and_updates_stats() -> Result<()>
    {
        let app = build_router(test_config()?);
        let session = create_session_token(&app).await?;
        let created_at_ms = (super::now_unix_ms() as i64).saturating_sub(30_000);

        let product = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/kernel/compute/products")
                    .header("authorization", authorization(&session))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(
                        &compute_product_wire_request(
                            "ollama.text_generation",
                            "idemp.compute.product.index-correct",
                            created_at_ms,
                        ),
                    )?))?,
            )
            .await?;
        assert_eq!(product.status(), StatusCode::OK);

        let lot = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/kernel/compute/lots")
                    .header("authorization", authorization(&session))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&capacity_lot_wire_request(
                        "lot.compute.index-correct",
                        "ollama.text_generation",
                        "idemp.compute.lot.index-correct",
                        created_at_ms + 1_000,
                    ))?))?,
            )
            .await?;
        assert_eq!(lot.status(), StatusCode::OK);

        let first_instrument = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/kernel/compute/instruments")
                    .header("authorization", authorization(&session))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(
                        &capacity_instrument_wire_request(
                            "instrument.compute.index-correct.alpha",
                            "ollama.text_generation",
                            "lot.compute.index-correct",
                            "idemp.compute.instrument.index-correct.alpha",
                            created_at_ms + 2_000,
                        ),
                    )?))?,
            )
            .await?;
        assert_eq!(first_instrument.status(), StatusCode::OK);

        let first_delivery = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/kernel/compute/lots/lot.compute.index-correct/delivery_proofs")
                    .header("authorization", authorization(&session))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(
                        &delivery_proof_wire_request(
                            "delivery.compute.index-correct.alpha",
                            "ollama.text_generation",
                            "lot.compute.index-correct",
                            "instrument.compute.index-correct.alpha",
                            "idemp.compute.delivery.index-correct.alpha",
                            created_at_ms + 3_000,
                        ),
                    )?))?,
            )
            .await?;
        assert_eq!(first_delivery.status(), StatusCode::OK);

        let published = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/kernel/compute/indices")
                    .header("authorization", authorization(&session))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(
                        &compute_index_wire_request(
                            "index.compute.index-correct",
                            "ollama.text_generation",
                            "idemp.compute.index-correct.publish",
                            created_at_ms + 4_000,
                        ),
                    )?))?,
            )
            .await?;
        assert_eq!(published.status(), StatusCode::OK);

        let mut second_lot = capacity_lot_request(
            "lot.compute.index-correct.beta",
            "ollama.text_generation",
            "idemp.compute.lot.index-correct.beta",
            created_at_ms + 1_500,
        );
        second_lot.lot.provider_id = "desktop-provider.beta".to_string();
        let second_lot = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/kernel/compute/lots")
                    .header("authorization", authorization(&session))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(
                        &compute_contracts::create_capacity_lot_request_to_proto(&second_lot)?,
                    )?))?,
            )
            .await?;
        assert_eq!(second_lot.status(), StatusCode::OK);

        let mut second_instrument = capacity_instrument_request(
            "instrument.compute.index-correct.beta",
            "ollama.text_generation",
            "lot.compute.index-correct.beta",
            "idemp.compute.instrument.index-correct.beta",
            created_at_ms + 5_000,
        );
        second_instrument.instrument.provider_id = Some("desktop-provider.beta".to_string());
        second_instrument.instrument.fixed_price = Some(Money {
            asset: Asset::Btc,
            amount: MoneyAmount::AmountSats(1_800),
        });
        let second_instrument = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/kernel/compute/instruments")
                    .header("authorization", authorization(&session))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(
                        &compute_contracts::create_capacity_instrument_request_to_proto(
                            &second_instrument,
                        )?,
                    )?))?,
            )
            .await?;
        assert_eq!(second_instrument.status(), StatusCode::OK);

        let mut second_delivery = delivery_proof_request(
            "delivery.compute.index-correct.beta",
            "ollama.text_generation",
            "lot.compute.index-correct.beta",
            "instrument.compute.index-correct.beta",
            "idemp.compute.delivery.index-correct.beta",
            created_at_ms + 3_500,
        );
        second_delivery.delivery_proof.created_at_ms = created_at_ms + 3_500;
        let second_delivery = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/kernel/compute/lots/lot.compute.index-correct.beta/delivery_proofs")
                    .header("authorization", authorization(&session))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(
                        &compute_contracts::record_delivery_proof_request_to_proto(
                            &second_delivery,
                        )?,
                    )?))?,
            )
            .await?;
        assert_eq!(second_delivery.status(), StatusCode::OK);

        let corrected = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/kernel/compute/indices/index.compute.index-correct/correct")
                    .header("authorization", authorization(&session))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(
                        &correct_compute_index_wire_request(
                            "index.compute.index-correct",
                            "index.compute.index-correct.v2",
                            "idemp.compute.index-correct.correct",
                            created_at_ms + 7_000,
                            ComputeIndexCorrectionReason::LateObservation,
                        ),
                    )?))?,
            )
            .await?;
        assert_eq!(corrected.status(), StatusCode::OK);
        let corrected_payload = compute_contracts::correct_compute_index_response_from_proto(
            &response_json::<proto_compute::CorrectComputeIndexResponse>(corrected).await?,
        )?;
        assert_eq!(
            corrected_payload.receipt.receipt_type,
            "kernel.compute.index.correct.v1"
        );
        assert_eq!(
            corrected_payload.superseded_index.status,
            ComputeIndexStatus::Superseded
        );
        assert_eq!(
            corrected_payload
                .corrected_index
                .corrected_from_index_id
                .as_deref(),
            Some("index.compute.index-correct")
        );
        assert!(corrected_payload.corrected_index.reference_price.is_some());
        assert_eq!(
            corrected_payload
                .corrected_index
                .metadata
                .get("governance")
                .and_then(serde_json::Value::as_object)
                .and_then(|governance| governance.get("settlement_eligible"))
                .and_then(serde_json::Value::as_bool),
            Some(true)
        );

        let stats_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/stats")
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(stats_response.status(), StatusCode::OK);
        let stats: PublicStatsSnapshot = response_json(stats_response).await?;
        assert_eq!(stats.compute_indices_published_24h, 2);
        assert_eq!(stats.compute_index_corrections_24h, 1);
        assert_eq!(stats.compute_index_thin_windows_24h, 1);
        assert_eq!(stats.compute_index_settlement_eligible_24h, 1);

        Ok(())
    }

    #[tokio::test]
    async fn future_cash_route_settles_and_updates_stats() -> Result<()> {
        let app = build_router(test_config()?);
        let session = create_session_token(&app).await?;
        let created_at_ms = (super::now_unix_ms() as i64).saturating_sub(120_000);

        for request in [
            Request::builder()
                .method("POST")
                .uri("/v1/kernel/compute/products")
                .header("authorization", authorization(&session))
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(
                    &compute_product_wire_request(
                        "ollama.text_generation",
                        "idemp.compute.product.future-cash",
                        created_at_ms,
                    ),
                )?))?,
            Request::builder()
                .method("POST")
                .uri("/v1/kernel/compute/lots")
                .header("authorization", authorization(&session))
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&capacity_lot_wire_request(
                    "lot.compute.future-cash.alpha",
                    "ollama.text_generation",
                    "idemp.compute.lot.future-cash.alpha",
                    created_at_ms + 1_000,
                ))?))?,
            Request::builder()
                .method("POST")
                .uri("/v1/kernel/compute/instruments")
                .header("authorization", authorization(&session))
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(
                    &capacity_instrument_wire_request(
                        "instrument.compute.future-cash.alpha",
                        "ollama.text_generation",
                        "lot.compute.future-cash.alpha",
                        "idemp.compute.instrument.future-cash.alpha",
                        created_at_ms + 2_000,
                    ),
                )?))?,
            Request::builder()
                .method("POST")
                .uri("/v1/kernel/compute/lots/lot.compute.future-cash.alpha/delivery_proofs")
                .header("authorization", authorization(&session))
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(
                    &delivery_proof_wire_request(
                        "delivery.compute.future-cash.alpha",
                        "ollama.text_generation",
                        "lot.compute.future-cash.alpha",
                        "instrument.compute.future-cash.alpha",
                        "idemp.compute.delivery.future-cash.alpha",
                        created_at_ms + 3_000,
                    ),
                )?))?,
            Request::builder()
                .method("POST")
                .uri("/v1/kernel/compute/indices")
                .header("authorization", authorization(&session))
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(
                    &compute_index_wire_request(
                        "index.compute.future-cash",
                        "ollama.text_generation",
                        "idemp.compute.index.future-cash.publish",
                        created_at_ms + 4_000,
                    ),
                )?))?,
        ] {
            let response = app.clone().oneshot(request).await?;
            assert_eq!(response.status(), StatusCode::OK);
        }

        let mut second_lot = capacity_lot_request(
            "lot.compute.future-cash.beta",
            "ollama.text_generation",
            "idemp.compute.lot.future-cash.beta",
            created_at_ms + 1_500,
        );
        second_lot.lot.provider_id = "desktop-provider.beta".to_string();
        let second_lot_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/kernel/compute/lots")
                    .header("authorization", authorization(&session))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(
                        &compute_contracts::create_capacity_lot_request_to_proto(&second_lot)?,
                    )?))?,
            )
            .await?;
        assert_eq!(second_lot_response.status(), StatusCode::OK);

        let mut second_instrument = capacity_instrument_request(
            "instrument.compute.future-cash.beta",
            "ollama.text_generation",
            "lot.compute.future-cash.beta",
            "idemp.compute.instrument.future-cash.beta",
            created_at_ms + 5_000,
        );
        second_instrument.instrument.provider_id = Some("desktop-provider.beta".to_string());
        second_instrument.instrument.fixed_price = Some(Money {
            asset: Asset::Btc,
            amount: MoneyAmount::AmountSats(1_800),
        });
        let second_instrument_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/kernel/compute/instruments")
                    .header("authorization", authorization(&session))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(
                        &compute_contracts::create_capacity_instrument_request_to_proto(
                            &second_instrument,
                        )?,
                    )?))?,
            )
            .await?;
        assert_eq!(second_instrument_response.status(), StatusCode::OK);

        let mut second_delivery = delivery_proof_request(
            "delivery.compute.future-cash.beta",
            "ollama.text_generation",
            "lot.compute.future-cash.beta",
            "instrument.compute.future-cash.beta",
            "idemp.compute.delivery.future-cash.beta",
            created_at_ms + 3_500,
        );
        second_delivery.delivery_proof.created_at_ms = created_at_ms + 3_500;
        let second_delivery_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/kernel/compute/lots/lot.compute.future-cash.beta/delivery_proofs")
                    .header("authorization", authorization(&session))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(
                        &compute_contracts::record_delivery_proof_request_to_proto(
                            &second_delivery,
                        )?,
                    )?))?,
            )
            .await?;
        assert_eq!(second_delivery_response.status(), StatusCode::OK);

        let corrected_index = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/kernel/compute/indices/index.compute.future-cash/correct")
                    .header("authorization", authorization(&session))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(
                        &correct_compute_index_wire_request(
                            "index.compute.future-cash",
                            "index.compute.future-cash.v2",
                            "idemp.compute.index.future-cash.correct",
                            created_at_ms + 7_000,
                            ComputeIndexCorrectionReason::LateObservation,
                        ),
                    )?))?,
            )
            .await?;
        assert_eq!(corrected_index.status(), StatusCode::OK);

        let future_cash = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/kernel/compute/instruments")
                    .header("authorization", authorization(&session))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(
                        &compute_contracts::create_capacity_instrument_request_to_proto(
                            &future_cash_instrument_request(
                                "instrument.compute.future_cash.route",
                                "index.compute.future-cash",
                                "idemp.compute.instrument.future-cash.route",
                                created_at_ms + 8_000,
                                150,
                                10,
                            ),
                        )?,
                    )?))?,
            )
            .await?;
        assert_eq!(future_cash.status(), StatusCode::OK);
        let future_cash_payload =
            compute_contracts::create_capacity_instrument_response_from_proto(
                &response_json::<proto_compute::CreateCapacityInstrumentResponse>(future_cash)
                    .await?,
            )?;
        assert_eq!(
            future_cash_payload.instrument.reference_index_id.as_deref(),
            Some("index.compute.future-cash.v2")
        );

        let settlement = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/kernel/compute/instruments/instrument.compute.future_cash.route/cash_settle")
                    .header("authorization", authorization(&session))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(
                        &cash_settle_capacity_instrument_wire_request(
                            "instrument.compute.future_cash.route",
                            "idemp.compute.instrument.future-cash.route.settle",
                            created_at_ms + 70_000,
                        ),
                    )?))?,
            )
            .await?;
        assert_eq!(settlement.status(), StatusCode::OK);
        let settlement_payload =
            compute_contracts::cash_settle_capacity_instrument_response_from_proto(
                &response_json::<proto_compute::CashSettleCapacityInstrumentResponse>(settlement)
                    .await?,
            )?;
        assert_eq!(
            settlement_payload.instrument.status,
            CapacityInstrumentStatus::Settled
        );
        assert_eq!(
            settlement_payload
                .cash_flow
                .as_ref()
                .map(|money| match money.amount {
                    MoneyAmount::AmountSats(value) | MoneyAmount::AmountMsats(value) => value,
                }),
            Some(150)
        );

        let stats_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/stats")
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(stats_response.status(), StatusCode::OK);
        let stats: PublicStatsSnapshot = response_json(stats_response).await?;
        assert_eq!(stats.compute_active_provider_count, 2);
        assert!((stats.compute_provider_concentration_hhi - 0.5).abs() < f64::EPSILON);
        assert_eq!(stats.compute_forward_physical_instruments_active, 2);
        assert_eq!(stats.compute_forward_physical_open_quantity, 20);
        assert_eq!(stats.compute_future_cash_instruments_active, 0);
        assert_eq!(stats.compute_future_cash_open_interest, 0);
        assert_eq!(stats.compute_future_cash_cash_settlements_24h, 1);
        assert_eq!(stats.compute_future_cash_cash_flow_24h, 150);
        assert_eq!(stats.compute_priced_instruments_24h, 3);
        assert_eq!(stats.compute_breakers_tripped, 1);
        assert_eq!(stats.compute_breakers_guarded, 1);
        assert_eq!(stats.compute_breaker_states.len(), 6);
        assert!(
            stats
                .compute_breaker_states
                .iter()
                .any(|row| row.breaker_id == "future_cash.index_quality" && row.state == "tripped")
        );
        assert!(
            stats
                .compute_breaker_states
                .iter()
                .any(|row| row.breaker_id == "provider_concentration" && row.state == "guarded")
        );
        assert_eq!(
            stats
                .compute_truth_labels
                .iter()
                .find(|row| row.truth_label == "canonical")
                .map(|row| row.count_24h),
            Some(2)
        );
        assert_eq!(
            stats
                .compute_truth_labels
                .iter()
                .find(|row| row.truth_label == "legacy")
                .map(|row| row.count_24h),
            Some(0)
        );
        assert_eq!(
            stats
                .compute_truth_labels
                .iter()
                .find(|row| row.truth_label == "transitional")
                .map(|row| row.count_24h),
            Some(0)
        );
        assert_eq!(stats.compute_rollout_gates.len(), 4);
        assert!(stats.compute_rollout_gates.iter().all(|gate| gate.enabled));
        assert_eq!(
            stats.compute_policy_bundle_id,
            DEFAULT_COMPUTE_POLICY_BUNDLE_ID
        );
        assert_eq!(stats.compute_policy_version, DEFAULT_COMPUTE_POLICY_VERSION);

        Ok(())
    }

    #[tokio::test]
    async fn future_cash_route_rejects_when_runtime_gate_disabled() -> Result<()> {
        let mut config = test_config()?;
        config.compute_enable_future_cash = false;
        let app = build_router(config);
        let session = create_session_token(&app).await?;
        let created_at_ms = super::now_unix_ms() as i64;

        let product = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/kernel/compute/products")
                    .header("authorization", authorization(&session))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(
                        &compute_product_wire_request(
                            "ollama.text_generation",
                            "idemp.compute.product.future-cash.gate",
                            created_at_ms,
                        ),
                    )?))?,
            )
            .await?;
        assert_eq!(product.status(), StatusCode::OK);

        let future_cash = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/kernel/compute/instruments")
                    .header("authorization", authorization(&session))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(
                        &compute_contracts::create_capacity_instrument_request_to_proto(
                            &future_cash_instrument_request(
                                "instrument.compute.future_cash.disabled",
                                "index.compute.missing",
                                "idemp.compute.instrument.future-cash.disabled",
                                created_at_ms + 1_000,
                                150,
                                10,
                            ),
                        )?,
                    )?))?,
            )
            .await?;
        assert_eq!(future_cash.status(), StatusCode::CONFLICT);
        let body: serde_json::Value = response_json(future_cash).await?;
        assert_eq!(body["reason"], "compute_future_cash_disabled");

        Ok(())
    }

    #[tokio::test]
    async fn structured_route_rejects_when_disabled_and_stats_publish_rollout_state() -> Result<()>
    {
        let mut config = test_config()?;
        config.compute_enable_future_cash = false;
        config.compute_enable_structured_products = false;
        config.compute_enable_reconciliation_diagnostics = false;
        config.compute_policy_bundle_id = "policy.compute.market.launch-ops".to_string();
        config.compute_policy_version = "2026-03-07".to_string();
        let app = build_router(config);
        let session = create_session_token(&app).await?;
        let created_at_ms = super::now_unix_ms() as i64;

        let structured = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/kernel/compute/structured_instruments")
                    .header("authorization", authorization(&session))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(
                        &structured_reservation_wire_request(
                            "structured.compute.disabled",
                            "instrument.compute.disabled",
                            "idemp.compute.structured.disabled",
                            created_at_ms,
                        ),
                    )?))?,
            )
            .await?;
        assert_eq!(structured.status(), StatusCode::CONFLICT);
        let structured_body: serde_json::Value = response_json(structured).await?;
        assert_eq!(
            structured_body["reason"],
            "compute_structured_products_disabled"
        );

        let stats_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/stats")
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(stats_response.status(), StatusCode::OK);
        let stats: PublicStatsSnapshot = response_json(stats_response).await?;
        assert_eq!(
            stats.compute_policy_bundle_id,
            "policy.compute.market.launch-ops"
        );
        assert_eq!(stats.compute_policy_version, "2026-03-07");
        assert!(stats.compute_truth_labels.is_empty());
        assert_eq!(stats.compute_rollout_gates.len(), 4);
        assert_eq!(
            stats
                .compute_rollout_gates
                .iter()
                .find(|gate| gate.gate_id == "forward_physical")
                .map(|gate| gate.enabled),
            Some(true)
        );
        assert_eq!(
            stats
                .compute_rollout_gates
                .iter()
                .find(|gate| gate.gate_id == "future_cash")
                .map(|gate| gate.enabled),
            Some(false)
        );
        assert_eq!(
            stats
                .compute_rollout_gates
                .iter()
                .find(|gate| gate.gate_id == "structured_products")
                .map(|gate| gate.enabled),
            Some(false)
        );
        assert_eq!(
            stats
                .compute_rollout_gates
                .iter()
                .find(|gate| gate.gate_id == "reconciliation_diagnostics")
                .map(|gate| gate.enabled),
            Some(false)
        );
        assert!(
            stats
                .compute_breaker_states
                .iter()
                .any(|row| row.breaker_id == "provider_concentration")
        );
        assert!(
            stats
                .compute_breaker_states
                .iter()
                .any(|row| row.breaker_id == "future_cash.index_quality")
        );

        Ok(())
    }

    #[tokio::test]
    async fn structured_reservation_routes_roundtrip_explicit_legs() -> Result<()> {
        let app = build_router(test_config()?);
        let session = create_session_token(&app).await?;
        let created_at_ms = (super::now_unix_ms() as i64).saturating_sub(90_000);

        for request in [
            Request::builder()
                .method("POST")
                .uri("/v1/kernel/compute/products")
                .header("authorization", authorization(&session))
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(
                    &compute_product_wire_request(
                        "ollama.text_generation",
                        "idemp.compute.product.structured",
                        created_at_ms,
                    ),
                )?))?,
            Request::builder()
                .method("POST")
                .uri("/v1/kernel/compute/lots")
                .header("authorization", authorization(&session))
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&capacity_lot_wire_request(
                    "lot.compute.structured.alpha",
                    "ollama.text_generation",
                    "idemp.compute.lot.structured.alpha",
                    created_at_ms + 1_000,
                ))?))?,
            Request::builder()
                .method("POST")
                .uri("/v1/kernel/compute/instruments")
                .header("authorization", authorization(&session))
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(
                    &reservation_instrument_wire_request(
                        "instrument.compute.reservation.route",
                        "lot.compute.structured.alpha",
                        "idemp.compute.instrument.reservation.route",
                        created_at_ms + 2_000,
                    ),
                )?))?,
        ] {
            let response = app.clone().oneshot(request).await?;
            assert_eq!(response.status(), StatusCode::OK);
        }

        let created = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/kernel/compute/structured_instruments")
                    .header("authorization", authorization(&session))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(
                        &structured_reservation_wire_request(
                            "structured.compute.reservation.route",
                            "instrument.compute.reservation.route",
                            "idemp.compute.structured.reservation.route",
                            created_at_ms + 3_000,
                        ),
                    )?))?,
            )
            .await?;
        assert_eq!(created.status(), StatusCode::OK);
        let created_payload =
            compute_contracts::create_structured_capacity_instrument_response_from_proto(
                &response_json::<proto_compute::CreateStructuredCapacityInstrumentResponse>(
                    created,
                )
                .await?,
            )?;
        assert_eq!(
            created_payload.receipt.receipt_type,
            "kernel.compute.structured_instrument.create.v1"
        );
        assert_eq!(created_payload.legs.len(), 1);
        assert_eq!(
            created_payload.structured_instrument.kind,
            StructuredCapacityInstrumentKind::Reservation
        );

        let listed = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/v1/kernel/compute/structured_instruments?product_id=ollama.text_generation")
                    .header("authorization", authorization(&session))
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(listed.status(), StatusCode::OK);
        let listed_payload =
            compute_contracts::list_structured_capacity_instruments_response_from_proto(
                &response_json::<proto_compute::ListStructuredCapacityInstrumentsResponse>(listed)
                    .await?,
            )?;
        assert_eq!(listed_payload.len(), 1);
        assert_eq!(
            listed_payload[0]
                .metadata
                .get("visibility_scope")
                .and_then(serde_json::Value::as_str),
            Some("advanced_only")
        );

        let fetched = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/v1/kernel/compute/structured_instruments/structured.compute.reservation.route")
                    .header("authorization", authorization(&session))
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(fetched.status(), StatusCode::OK);
        let fetched_payload =
            compute_contracts::get_structured_capacity_instrument_response_from_proto(
                &response_json::<proto_compute::GetStructuredCapacityInstrumentResponse>(fetched)
                    .await?,
            )?;
        assert_eq!(fetched_payload.legs.len(), 1);

        let closed = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/kernel/compute/structured_instruments/structured.compute.reservation.route/close")
                    .header("authorization", authorization(&session))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(
                        &close_structured_capacity_instrument_wire_request(
                            "structured.compute.reservation.route",
                            "idemp.compute.structured.reservation.route.close",
                            created_at_ms + 4_000,
                        ),
                    )?))?,
            )
            .await?;
        assert_eq!(closed.status(), StatusCode::OK);
        let closed_payload =
            compute_contracts::close_structured_capacity_instrument_response_from_proto(
                &response_json::<proto_compute::CloseStructuredCapacityInstrumentResponse>(closed)
                    .await?,
            )?;
        assert_eq!(
            closed_payload.structured_instrument.status,
            StructuredCapacityInstrumentStatus::Cancelled
        );
        assert_eq!(
            closed_payload.legs[0].status,
            CapacityInstrumentStatus::Cancelled
        );

        Ok(())
    }

    #[tokio::test]
    async fn compute_read_models_reload_after_restart_when_kernel_state_is_persisted() -> Result<()>
    {
        let kernel_state_path = std::env::temp_dir().join(format!(
            "nexus-control-kernel-state-{}.json",
            super::random_token()
        ));
        let _ = std::fs::remove_file(kernel_state_path.as_path());

        let mut config = test_config()?;
        config.kernel_state_path = Some(kernel_state_path.clone());
        let app = build_router(config.clone());
        let session = create_session_token(&app).await?;
        let created_at_ms = super::now_unix_ms() as i64;

        let product_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/kernel/compute/products")
                    .header("authorization", authorization(&session))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(
                        &compute_product_wire_request(
                            "ollama.text_generation",
                            "idemp.compute.product.persisted",
                            created_at_ms,
                        ),
                    )?))?,
            )
            .await?;
        assert_eq!(product_response.status(), StatusCode::OK);
        let product_payload = compute_contracts::create_compute_product_response_from_proto(
            &response_json::<proto_compute::CreateComputeProductResponse>(product_response).await?,
        )?;

        let environment_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/kernel/compute/environments")
                    .header("authorization", authorization(&session))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(
                        &compute_contracts::register_compute_environment_package_request_to_proto(
                            &compute_environment_package_request(
                                "env.openagents.math.basic",
                                "2026.03.13",
                                "idemp.compute.environment.persisted",
                                created_at_ms + 500,
                            ),
                        )?,
                    )?))?,
            )
            .await?;
        assert_eq!(environment_response.status(), StatusCode::OK);

        let lot_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/kernel/compute/lots")
                    .header("authorization", authorization(&session))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&capacity_lot_wire_request(
                        "lot.compute.persisted",
                        "ollama.text_generation",
                        "idemp.compute.lot.persisted",
                        created_at_ms + 1_000,
                    ))?))?,
            )
            .await?;
        assert_eq!(lot_response.status(), StatusCode::OK);

        let instrument_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/kernel/compute/instruments")
                    .header("authorization", authorization(&session))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(
                        &capacity_instrument_wire_request(
                            "instrument.compute.persisted",
                            "ollama.text_generation",
                            "lot.compute.persisted",
                            "idemp.compute.instrument.persisted",
                            created_at_ms + 2_000,
                        ),
                    )?))?,
            )
            .await?;
        assert_eq!(instrument_response.status(), StatusCode::OK);

        let delivery_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/kernel/compute/lots/lot.compute.persisted/delivery_proofs")
                    .header("authorization", authorization(&session))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(
                        &delivery_proof_wire_request(
                            "delivery.compute.persisted",
                            "ollama.text_generation",
                            "lot.compute.persisted",
                            "instrument.compute.persisted",
                            "idemp.compute.delivery.persisted",
                            created_at_ms + 3_000,
                        ),
                    )?))?,
            )
            .await?;
        assert_eq!(delivery_response.status(), StatusCode::OK);

        let index_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/kernel/compute/indices")
                    .header("authorization", authorization(&session))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(
                        &compute_index_wire_request(
                            "index.compute.persisted",
                            "ollama.text_generation",
                            "idemp.compute.index.persisted",
                            created_at_ms + 4_000,
                        ),
                    )?))?,
            )
            .await?;
        assert_eq!(index_response.status(), StatusCode::OK);

        let reloaded_app = build_router(config);
        let reloaded_session = create_session_token(&reloaded_app).await?;

        let products_response = reloaded_app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/v1/kernel/compute/products?status=active")
                    .header("authorization", authorization(&reloaded_session))
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(products_response.status(), StatusCode::OK);
        let products = compute_contracts::list_compute_products_response_from_proto(
            &response_json::<proto_compute::ListComputeProductsResponse>(products_response).await?,
        )?;
        assert!(products.iter().any(|product| {
            product.product_id == "ollama.text_generation"
                || product.product_id == product_payload.product.product_id
        }));

        let product_get_response = reloaded_app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/v1/kernel/compute/products/ollama.text_generation")
                    .header("authorization", authorization(&reloaded_session))
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(product_get_response.status(), StatusCode::OK);
        let product_get = compute_contracts::get_compute_product_response_from_proto(
            &response_json::<proto_compute::GetComputeProductResponse>(product_get_response)
                .await?,
        )?;
        assert_eq!(product_get.product_id, "ollama.text_generation");

        let environments_response = reloaded_app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/v1/kernel/compute/environments?family=evaluation&status=active")
                    .header("authorization", authorization(&reloaded_session))
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(environments_response.status(), StatusCode::OK);
        let environments =
            compute_contracts::list_compute_environment_packages_response_from_proto(
                &response_json::<proto_compute::ListComputeEnvironmentPackagesResponse>(
                    environments_response,
                )
                .await?,
            )?;
        assert!(environments.iter().any(|package| {
            package.environment_ref == "env.openagents.math.basic"
                && package.version == "2026.03.13"
        }));

        let lots_response = reloaded_app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/v1/kernel/compute/lots?product_id=ollama.text_generation")
                    .header("authorization", authorization(&reloaded_session))
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(lots_response.status(), StatusCode::OK);
        let lots = compute_contracts::list_capacity_lots_response_from_proto(
            &response_json::<proto_compute::ListCapacityLotsResponse>(lots_response).await?,
        )?;
        assert!(
            lots.iter()
                .any(|lot| lot.capacity_lot_id == "lot.compute.persisted")
        );

        let instruments_response = reloaded_app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/v1/kernel/compute/instruments?capacity_lot_id=lot.compute.persisted")
                    .header("authorization", authorization(&reloaded_session))
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(instruments_response.status(), StatusCode::OK);
        let instruments = compute_contracts::list_capacity_instruments_response_from_proto(
            &response_json::<proto_compute::ListCapacityInstrumentsResponse>(instruments_response)
                .await?,
        )?;
        assert!(
            instruments
                .iter()
                .any(|instrument| instrument.instrument_id == "instrument.compute.persisted")
        );

        let delivery_proofs_response = reloaded_app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/v1/kernel/compute/lots/lot.compute.persisted/delivery_proofs")
                    .header("authorization", authorization(&reloaded_session))
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(delivery_proofs_response.status(), StatusCode::OK);
        let delivery_proofs = compute_contracts::list_delivery_proofs_response_from_proto(
            &response_json::<proto_compute::ListDeliveryProofsResponse>(delivery_proofs_response)
                .await?,
        )?;
        assert!(
            delivery_proofs
                .iter()
                .any(|proof| proof.delivery_proof_id == "delivery.compute.persisted")
        );

        let indices_response = reloaded_app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/v1/kernel/compute/indices?product_id=ollama.text_generation")
                    .header("authorization", authorization(&reloaded_session))
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(indices_response.status(), StatusCode::OK);
        let indices = compute_contracts::list_compute_indices_response_from_proto(
            &response_json::<proto_compute::ListComputeIndicesResponse>(indices_response).await?,
        )?;
        assert!(
            indices
                .iter()
                .any(|index| index.index_id == "index.compute.persisted")
        );

        let _ = std::fs::remove_file(kernel_state_path.as_path());
        Ok(())
    }

    #[tokio::test]
    async fn compute_http_client_roundtrips_generated_contracts() -> Result<()> {
        let app = build_router(test_config()?);
        let session = create_session_token(&app).await?;

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let local_addr = listener.local_addr()?;
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.expect("serve test app");
        });

        let client = HttpKernelAuthorityClient::new(
            format!("http://{local_addr}"),
            Some(session.access_token.clone()),
        )?;
        let created_at_ms = super::now_unix_ms() as i64;

        let environment = client
            .register_compute_environment_package(compute_environment_package_request(
                "env.openagents.math.basic",
                "2026.03.13",
                "idemp.compute.client.environment",
                created_at_ms + 500,
            ))
            .await?;
        assert_eq!(
            environment.package.environment_ref,
            "env.openagents.math.basic"
        );

        let checkpoint_policy = client
            .register_compute_checkpoint_family_policy(compute_checkpoint_family_policy_request(
                "idemp.compute.client.checkpoint_policy",
                created_at_ms + 510,
            ))
            .await?;
        assert_eq!(checkpoint_policy.policy_record.checkpoint_family, "decoder");
        assert_eq!(
            checkpoint_policy.receipt.receipt_type,
            "kernel.compute.checkpoint_policy.register.v1"
        );

        let validator_policy = client
            .register_compute_validator_policy(compute_validator_policy_request(
                "idemp.compute.client.validator_policy",
                created_at_ms + 520,
            ))
            .await?;
        assert_eq!(
            validator_policy.policy_record.policy_ref,
            "policy://validator/training"
        );
        assert_eq!(
            validator_policy.receipt.receipt_type,
            "kernel.compute.validator_policy.register.v1"
        );

        let benchmark_package = client
            .register_compute_benchmark_package(compute_benchmark_package_request(
                "idemp.compute.client.benchmark_package",
                created_at_ms + 530,
            ))
            .await?;
        assert_eq!(
            benchmark_package.benchmark_package.benchmark_package_ref,
            "benchmark://mmlu/reference"
        );
        assert_eq!(
            benchmark_package
                .benchmark_package
                .environment_version
                .as_deref(),
            Some("2026.03.13")
        );
        assert_eq!(
            benchmark_package.receipt.receipt_type,
            "kernel.compute.benchmark_package.register.v1"
        );

        let training_policy = client
            .register_compute_training_policy(compute_training_policy_request(
                "idemp.compute.client.training_policy",
                created_at_ms + 540,
            ))
            .await?;
        assert_eq!(
            training_policy.training_policy.training_policy_ref,
            "policy://training/math/basic"
        );
        assert_eq!(
            training_policy.receipt.receipt_type,
            "kernel.compute.training_policy.register.v1"
        );

        let mut product_request = compute_product_request(
            "ollama.text_generation",
            "idemp.compute.client.product",
            created_at_ms,
        );
        product_request
            .product
            .capability_envelope
            .as_mut()
            .expect("capability envelope")
            .environment_binding = Some(ComputeEnvironmentBinding {
            environment_ref: "env.openagents.math.basic".to_string(),
            environment_version: None,
            dataset_ref: Some("dataset://math/basic".to_string()),
            rubric_ref: Some("rubric://math/basic".to_string()),
            evaluator_policy_ref: Some("policy://eval/math/basic".to_string()),
        });
        let product = client.create_compute_product(product_request).await?;
        assert_eq!(product.product.product_id, "ollama.text_generation");
        assert_eq!(
            product
                .product
                .capability_envelope
                .as_ref()
                .and_then(|envelope| envelope.environment_binding.as_ref())
                .and_then(|binding| binding.environment_version.as_deref()),
            Some("2026.03.13")
        );

        let lot = client
            .create_capacity_lot(capacity_lot_request(
                "lot.compute.client",
                "ollama.text_generation",
                "idemp.compute.client.lot",
                created_at_ms + 1_000,
            ))
            .await?;
        assert_eq!(lot.lot.capacity_lot_id, "lot.compute.client");
        assert_eq!(
            lot.lot
                .environment_binding
                .as_ref()
                .and_then(|binding| binding.environment_version.as_deref()),
            Some("2026.03.13")
        );

        let instrument = client
            .create_capacity_instrument(capacity_instrument_request(
                "instrument.compute.client",
                "ollama.text_generation",
                "lot.compute.client",
                "idemp.compute.client.instrument",
                created_at_ms + 2_000,
            ))
            .await?;
        assert_eq!(
            instrument.instrument.instrument_id,
            "instrument.compute.client"
        );
        assert_eq!(
            instrument
                .instrument
                .environment_binding
                .as_ref()
                .and_then(|binding| binding.environment_version.as_deref()),
            Some("2026.03.13")
        );

        let mut delivery_request = delivery_proof_request(
            "delivery.compute.client",
            "ollama.text_generation",
            "lot.compute.client",
            "instrument.compute.client",
            "idemp.compute.client.delivery",
            created_at_ms + 3_000,
        );
        delivery_request
            .delivery_proof
            .observed_capability_envelope
            .as_mut()
            .expect("observed capability envelope")
            .proof_posture = Some(ComputeProofPosture::ChallengeEligible);
        delivery_request.delivery_proof.verification_evidence =
            Some(DeliveryVerificationEvidence {
                proof_bundle_ref: Some("proof_bundle:delivery.compute.client".to_string()),
                activation_fingerprint_ref: None,
                validator_pool_ref: Some("validators.alpha".to_string()),
                validator_run_ref: None,
                challenge_result_refs: Vec::new(),
                environment_ref: None,
                environment_version: None,
                eval_run_ref: None,
            });
        let delivery = client.record_delivery_proof(delivery_request).await?;
        assert_eq!(
            delivery.delivery_proof.delivery_proof_id,
            "delivery.compute.client"
        );
        assert_eq!(
            delivery
                .delivery_proof
                .verification_evidence
                .as_ref()
                .and_then(|evidence| evidence.environment_ref.as_deref()),
            Some("env.openagents.math.basic")
        );
        assert_eq!(
            delivery
                .delivery_proof
                .verification_evidence
                .as_ref()
                .and_then(|evidence| evidence.environment_version.as_deref()),
            Some("2026.03.13")
        );

        let eval_run = client
            .create_compute_evaluation_run(compute_evaluation_run_request(
                "eval.run.client",
                "delivery.compute.client",
                "idemp.compute.client.eval_run",
                created_at_ms + 3_200,
            ))
            .await?;
        assert_eq!(eval_run.eval_run.eval_run_id, "eval.run.client");
        assert_eq!(
            eval_run
                .eval_run
                .environment_binding
                .environment_version
                .as_deref(),
            Some("2026.03.13")
        );

        let appended = client
            .append_compute_evaluation_samples(append_compute_evaluation_samples_request(
                "eval.run.client",
                "idemp.compute.client.eval_run.samples",
                created_at_ms + 3_300,
            ))
            .await?;
        assert_eq!(appended.samples.len(), 2);
        assert_eq!(
            appended.eval_run.status,
            ComputeEvaluationRunStatus::Running
        );

        let finalized = client
            .finalize_compute_evaluation_run(finalize_compute_evaluation_run_request(
                "eval.run.client",
                "idemp.compute.client.eval_run.finalize",
                created_at_ms + 3_400,
            ))
            .await?;
        assert_eq!(
            finalized.eval_run.status,
            ComputeEvaluationRunStatus::Finalized
        );
        assert_eq!(
            finalized
                .eval_run
                .summary
                .as_ref()
                .map(|summary| summary.pass_rate_bps),
            Some(Some(5_000))
        );
        assert_eq!(
            finalized.receipt.receipt_type,
            "kernel.compute.eval_run.finalize.v1"
        );
        let fetched_eval_run = client.get_compute_evaluation_run("eval.run.client").await?;
        assert_eq!(fetched_eval_run.eval_run_id, "eval.run.client");
        let eval_samples = client
            .list_compute_evaluation_samples("eval.run.client")
            .await?;
        assert_eq!(eval_samples.len(), 2);

        let accepted_eval_outcome = client
            .accept_compute_outcome(accept_compute_evaluation_outcome_request(
                "eval.run.client",
                "idemp.compute.client.eval_run.accept",
                created_at_ms + 3_425,
            ))
            .await?;
        assert_eq!(
            accepted_eval_outcome.outcome.outcome_kind,
            ComputeAcceptedOutcomeKind::EvaluationRun
        );
        assert_eq!(
            accepted_eval_outcome.receipt.receipt_type,
            "kernel.compute.outcome.accept.v1"
        );
        assert_eq!(
            accepted_eval_outcome
                .outcome
                .evaluation_summary
                .as_ref()
                .and_then(|summary| summary.pass_rate_bps),
            Some(5_000)
        );

        let delivery_with_eval = client.get_delivery_proof("delivery.compute.client").await?;
        assert_eq!(
            delivery_with_eval
                .verification_evidence
                .as_ref()
                .and_then(|evidence| evidence.eval_run_ref.as_deref()),
            Some("eval.run.client")
        );

        let synthetic_job = client
            .create_compute_synthetic_data_job(compute_synthetic_data_job_request(
                "synthetic.job.client",
                "idemp.compute.client.synthetic",
                created_at_ms + 3_450,
            ))
            .await?;
        assert_eq!(
            synthetic_job.synthetic_job.synthetic_job_id,
            "synthetic.job.client"
        );
        assert_eq!(
            synthetic_job
                .synthetic_job
                .environment_binding
                .environment_version
                .as_deref(),
            Some("2026.03.13")
        );

        let synthetic_samples = client
            .append_compute_synthetic_data_samples(append_compute_synthetic_data_samples_request(
                "synthetic.job.client",
                "idemp.compute.client.synthetic.samples",
                created_at_ms + 3_500,
            ))
            .await?;
        assert_eq!(synthetic_samples.samples.len(), 2);
        assert_eq!(
            synthetic_samples.synthetic_job.status,
            ComputeSyntheticDataJobStatus::Generating
        );

        let synthetic_generated = client
            .finalize_compute_synthetic_data_generation(
                finalize_compute_synthetic_data_generation_request(
                    "synthetic.job.client",
                    "idemp.compute.client.synthetic.finalize",
                    created_at_ms + 3_550,
                ),
            )
            .await?;
        assert_eq!(
            synthetic_generated.synthetic_job.status,
            ComputeSyntheticDataJobStatus::Generated
        );

        let synthetic_verified = client
            .record_compute_synthetic_data_verification(
                record_compute_synthetic_data_verification_request(
                    "synthetic.job.client",
                    "eval.run.client",
                    "idemp.compute.client.synthetic.verify",
                    created_at_ms + 3_600,
                ),
            )
            .await?;
        assert_eq!(
            synthetic_verified.synthetic_job.status,
            ComputeSyntheticDataJobStatus::Verified
        );
        assert_eq!(
            synthetic_verified
                .synthetic_job
                .verification_eval_run_id
                .as_deref(),
            Some("eval.run.client")
        );
        let fetched_synthetic_job = client
            .get_compute_synthetic_data_job("synthetic.job.client")
            .await?;
        assert_eq!(
            fetched_synthetic_job.synthetic_job_id,
            "synthetic.job.client"
        );
        let listed_synthetic_jobs = client
            .list_compute_synthetic_data_jobs(
                Some("env.openagents.math.basic"),
                Some("ollama.text_generation"),
                Some(ComputeSyntheticDataJobStatus::Verified),
            )
            .await?;
        assert_eq!(listed_synthetic_jobs.len(), 1);
        let listed_synthetic_samples = client
            .list_compute_synthetic_data_samples("synthetic.job.client")
            .await?;
        assert_eq!(listed_synthetic_samples.len(), 2);
        assert_eq!(
            listed_synthetic_samples[0]
                .verification_eval_sample_id
                .as_deref(),
            Some("sample.alpha")
        );

        let mut training_run_request = compute_training_run_request(
            "idemp.compute.client.training_run",
            created_at_ms + 3_650,
        );
        training_run_request
            .training_run
            .rollout_verification_eval_run_ids = vec!["eval.run.client".to_string()];
        let training_run = client
            .create_compute_training_run(training_run_request)
            .await?;
        assert_eq!(
            training_run.training_run.training_run_id,
            "train.math.basic.client"
        );
        assert_eq!(
            training_run
                .training_run
                .environment_binding
                .environment_version
                .as_deref(),
            Some("2026.03.13")
        );
        assert_eq!(
            training_run.receipt.receipt_type,
            "kernel.compute.training_run.create.v1"
        );

        let finalized_training_run = client
            .finalize_compute_training_run(finalize_compute_training_run_request(
                "idemp.compute.client.training_run.finalize",
                created_at_ms + 3_700,
            ))
            .await?;
        assert_eq!(
            finalized_training_run.training_run.status,
            ComputeTrainingRunStatus::Accepted
        );
        assert_eq!(
            finalized_training_run
                .training_run
                .final_checkpoint_ref
                .as_deref(),
            Some("checkpoint://decoder/train.math.basic.client/final")
        );
        assert_eq!(
            finalized_training_run.receipt.receipt_type,
            "kernel.compute.training_run.finalize.v1"
        );

        let accepted_training_outcome = client
            .accept_compute_outcome(accept_compute_training_outcome_request(
                "idemp.compute.client.training_run.accept",
                created_at_ms + 3_750,
            ))
            .await?;
        assert_eq!(
            accepted_training_outcome.outcome.outcome_kind,
            ComputeAcceptedOutcomeKind::TrainingRun
        );
        assert_eq!(
            accepted_training_outcome
                .outcome
                .validator_policy_ref
                .as_deref(),
            Some("policy://validator/training")
        );
        assert_eq!(
            accepted_training_outcome
                .outcome
                .training_summary
                .as_ref()
                .and_then(|summary| summary.best_eval_score_bps),
            Some(9_350)
        );

        let listed_checkpoint_policies = client
            .list_compute_checkpoint_family_policies(Some(ComputeRegistryStatus::Active))
            .await?;
        assert_eq!(listed_checkpoint_policies.len(), 1);
        let fetched_checkpoint_policy = client
            .get_compute_checkpoint_family_policy("decoder", Some("2026.03.14"))
            .await?;
        assert_eq!(
            fetched_checkpoint_policy
                .default_recovery_posture
                .as_deref(),
            Some("warm-resume")
        );

        let listed_validator_policies = client
            .list_compute_validator_policies(
                Some("validator-pool.training"),
                Some(ComputeRegistryStatus::Active),
            )
            .await?;
        assert_eq!(listed_validator_policies.len(), 1);
        let fetched_validator_policy = client
            .get_compute_validator_policy("policy://validator/training", Some("2026.03.14"))
            .await?;
        assert_eq!(
            fetched_validator_policy.required_proof_posture,
            Some(ComputeProofPosture::ChallengeEligible)
        );

        let listed_benchmark_packages = client
            .list_compute_benchmark_packages(
                Some("mmlu"),
                Some("env.openagents.math.basic"),
                Some(ComputeRegistryStatus::Active),
            )
            .await?;
        assert_eq!(listed_benchmark_packages.len(), 1);
        let fetched_benchmark_package = client
            .get_compute_benchmark_package("benchmark://mmlu/reference", Some("2026.03.14"))
            .await?;
        assert_eq!(
            fetched_benchmark_package.adapter_kind.as_deref(),
            Some("mmlu_multiple_choice_v1")
        );

        let listed_training_policies = client
            .list_compute_training_policies(
                Some("env.openagents.math.basic"),
                Some(ComputeRegistryStatus::Active),
            )
            .await?;
        assert_eq!(listed_training_policies.len(), 1);
        let fetched_training_policy = client
            .get_compute_training_policy("policy://training/math/basic", Some("2026.03.14"))
            .await?;
        assert_eq!(fetched_training_policy.checkpoint_family, "decoder");

        let listed_training_runs = client
            .list_compute_training_runs(
                Some("policy://training/math/basic"),
                Some("env.openagents.math.basic"),
                Some(ComputeTrainingRunStatus::Accepted),
            )
            .await?;
        assert_eq!(listed_training_runs.len(), 1);
        let fetched_training_run = client
            .get_compute_training_run("train.math.basic.client")
            .await?;
        assert_eq!(
            fetched_training_run
                .summary
                .as_ref()
                .and_then(|summary| summary.completed_step_count),
            Some(64)
        );

        let recorded_adapter_window = client
            .record_compute_adapter_window(compute_adapter_window_request(
                "idemp.compute.client.adapter_window",
                created_at_ms + 3_800,
                None,
            ))
            .await?;
        assert_eq!(
            recorded_adapter_window.window.window_id,
            "adapter.window.client"
        );
        assert_eq!(
            recorded_adapter_window.receipt.receipt_type,
            "kernel.compute.adapter_window.record.v1"
        );

        let accepted_adapter_window = client
            .record_compute_adapter_window(compute_adapter_window_request(
                "idemp.compute.client.adapter_window.accepted",
                created_at_ms + 3_850,
                Some(accepted_training_outcome.outcome.outcome_id.as_str()),
            ))
            .await?;
        assert_eq!(
            accepted_adapter_window
                .window
                .accepted_outcome_id
                .as_deref(),
            Some("accepted.training.client")
        );

        let listed_adapter_windows = client
            .list_compute_adapter_training_windows(
                Some("train.math.basic.client"),
                Some(ComputeAdapterWindowStatus::Reconciled),
            )
            .await?;
        assert_eq!(listed_adapter_windows.len(), 1);
        assert_eq!(
            listed_adapter_windows[0].promotion_disposition,
            Some(ComputeAdapterPromotionDisposition::Promoted)
        );

        let fetched_adapter_window = client
            .get_compute_adapter_training_window("adapter.window.client")
            .await?;
        assert_eq!(
            fetched_adapter_window.accepted_outcome_id.as_deref(),
            Some("accepted.training.client")
        );

        let listed_adapter_contributions = client
            .list_compute_adapter_contribution_outcomes(
                Some("train.math.basic.client"),
                Some("adapter.window.client"),
                Some(ComputeAdapterContributionDisposition::Accepted),
            )
            .await?;
        assert_eq!(listed_adapter_contributions.len(), 1);
        assert_eq!(
            listed_adapter_contributions[0].contribution_id,
            "contrib.client.alpha"
        );

        let fetched_rejected_contribution = client
            .get_compute_adapter_contribution_outcome("contrib.client.beta")
            .await?;
        assert_eq!(
            fetched_rejected_contribution.validator_disposition,
            ComputeAdapterContributionDisposition::Rejected
        );

        let listed_eval_outcomes = client
            .list_compute_accepted_outcomes(
                Some(ComputeAcceptedOutcomeKind::EvaluationRun),
                Some("env.openagents.math.basic"),
            )
            .await?;
        assert_eq!(listed_eval_outcomes.len(), 1);
        let listed_training_outcomes = client
            .list_compute_accepted_outcomes(
                Some(ComputeAcceptedOutcomeKind::TrainingRun),
                Some("env.openagents.math.basic"),
            )
            .await?;
        assert_eq!(listed_training_outcomes.len(), 1);
        let fetched_eval_outcome = client
            .get_compute_accepted_outcome("accepted.evaluation.client")
            .await?;
        assert_eq!(fetched_eval_outcome.source_run_id, "eval.run.client");
        let fetched_training_outcome = client
            .get_compute_accepted_outcome("accepted.training.client")
            .await?;
        assert_eq!(
            fetched_training_outcome.source_run_id,
            "train.math.basic.client"
        );

        let index = client
            .publish_compute_index(compute_index_request(
                "index.compute.client",
                "ollama.text_generation",
                "idemp.compute.client.index",
                created_at_ms + 4_000,
            ))
            .await?;
        assert_eq!(index.index.index_id, "index.compute.client");
        assert_eq!(index.index.reference_price, None);

        let mut second_lot = capacity_lot_request(
            "lot.compute.client.beta",
            "ollama.text_generation",
            "idemp.compute.client.lot.beta",
            created_at_ms + 1_500,
        );
        second_lot.lot.provider_id = "desktop-provider.beta".to_string();
        let second_lot = client.create_capacity_lot(second_lot).await?;
        assert_eq!(second_lot.lot.capacity_lot_id, "lot.compute.client.beta");

        let reservation_lot = client
            .create_capacity_lot(capacity_lot_request(
                "lot.compute.client.reservation",
                "ollama.text_generation",
                "idemp.compute.client.lot.reservation",
                created_at_ms + 2_500,
            ))
            .await?;
        assert_eq!(
            reservation_lot.lot.capacity_lot_id,
            "lot.compute.client.reservation"
        );

        let mut second_instrument = capacity_instrument_request(
            "instrument.compute.client.beta",
            "ollama.text_generation",
            "lot.compute.client.beta",
            "idemp.compute.client.instrument.beta",
            created_at_ms + 5_000,
        );
        second_instrument.instrument.provider_id = Some("desktop-provider.beta".to_string());
        second_instrument.instrument.fixed_price = Some(Money {
            asset: Asset::Btc,
            amount: MoneyAmount::AmountSats(1_800),
        });
        let second_instrument = client.create_capacity_instrument(second_instrument).await?;
        assert_eq!(
            second_instrument.instrument.instrument_id,
            "instrument.compute.client.beta"
        );

        let mut second_delivery_request = delivery_proof_request(
            "delivery.compute.client.beta",
            "ollama.text_generation",
            "lot.compute.client.beta",
            "instrument.compute.client.beta",
            "idemp.compute.client.delivery.beta",
            created_at_ms + 3_500,
        );
        second_delivery_request
            .delivery_proof
            .observed_capability_envelope
            .as_mut()
            .expect("observed capability envelope")
            .proof_posture = Some(ComputeProofPosture::ChallengeEligible);
        second_delivery_request.delivery_proof.verification_evidence =
            Some(DeliveryVerificationEvidence {
                proof_bundle_ref: Some("proof_bundle:delivery.compute.client.beta".to_string()),
                activation_fingerprint_ref: None,
                validator_pool_ref: Some("validators.alpha".to_string()),
                validator_run_ref: None,
                challenge_result_refs: Vec::new(),
                environment_ref: None,
                environment_version: None,
                eval_run_ref: None,
            });
        let second_delivery = client
            .record_delivery_proof(second_delivery_request)
            .await?;
        assert_eq!(
            second_delivery.delivery_proof.delivery_proof_id,
            "delivery.compute.client.beta"
        );

        let corrected_index = client
            .correct_compute_index(correct_compute_index_request(
                "index.compute.client",
                "index.compute.client.v2",
                "idemp.compute.client.index.correct",
                created_at_ms + 7_000,
                ComputeIndexCorrectionReason::LateObservation,
            ))
            .await?;
        assert_eq!(
            corrected_index.superseded_index.status,
            ComputeIndexStatus::Superseded
        );
        assert_eq!(
            corrected_index
                .corrected_index
                .corrected_from_index_id
                .as_deref(),
            Some("index.compute.client")
        );
        assert!(corrected_index.corrected_index.reference_price.is_some());

        let future_cash = client
            .create_capacity_instrument(future_cash_instrument_request(
                "instrument.compute.client.future",
                "index.compute.client",
                "idemp.compute.client.instrument.future",
                created_at_ms + 8_000,
                150,
                10,
            ))
            .await?;
        assert_eq!(
            future_cash.instrument.kind,
            CapacityInstrumentKind::FutureCash
        );
        assert_eq!(
            future_cash.instrument.reference_index_id.as_deref(),
            Some("index.compute.client.v2")
        );

        let future_settlement = client
            .cash_settle_capacity_instrument(cash_settle_capacity_instrument_request(
                "instrument.compute.client.future",
                "idemp.compute.client.instrument.future.settle",
                created_at_ms + 70_000,
            ))
            .await?;
        assert_eq!(
            future_settlement.instrument.status,
            CapacityInstrumentStatus::Settled
        );
        assert_eq!(
            future_settlement.settlement_index_id,
            "index.compute.client.v2"
        );
        assert_eq!(
            future_settlement
                .cash_flow
                .as_ref()
                .map(|money| match money.amount {
                    MoneyAmount::AmountSats(value) | MoneyAmount::AmountMsats(value) => value,
                }),
            Some(150)
        );

        let reservation_leg = client
            .create_capacity_instrument(reservation_instrument_request(
                "instrument.compute.client.reservation",
                "lot.compute.client.reservation",
                "idemp.compute.client.instrument.reservation",
                created_at_ms + 9_000,
            ))
            .await?;
        assert_eq!(
            reservation_leg.instrument.kind,
            CapacityInstrumentKind::Reservation
        );

        let structured_reservation = client
            .create_structured_capacity_instrument(structured_reservation_request(
                "structured.compute.client.reservation",
                "instrument.compute.client.reservation",
                "idemp.compute.client.structured.reservation",
                created_at_ms + 10_000,
            ))
            .await?;
        assert_eq!(
            structured_reservation.structured_instrument.kind,
            StructuredCapacityInstrumentKind::Reservation
        );
        assert_eq!(structured_reservation.legs.len(), 1);

        let listed_products = client
            .list_compute_products(Some(ComputeProductStatus::Active))
            .await?;
        assert!(
            listed_products
                .iter()
                .any(|item| item.product_id == "ollama.text_generation")
        );

        let listed_environments = client
            .list_compute_environment_packages(Some("evaluation"), None)
            .await?;
        assert!(listed_environments.iter().any(|item| {
            item.environment_ref == "env.openagents.math.basic" && item.version == "2026.03.13"
        }));

        let listed_lots = client
            .list_capacity_lots(Some("ollama.text_generation"), None)
            .await?;
        assert!(
            listed_lots
                .iter()
                .any(|item| item.capacity_lot_id == "lot.compute.client")
        );

        let listed_instruments = client
            .list_capacity_instruments(None, Some("lot.compute.client"), None)
            .await?;
        assert!(
            listed_instruments
                .iter()
                .any(|item| item.instrument_id == "instrument.compute.client")
        );

        let listed_structured = client
            .list_structured_capacity_instruments(Some("ollama.text_generation"), None)
            .await?;
        assert!(listed_structured.iter().any(|item| {
            item.structured_instrument_id == "structured.compute.client.reservation"
        }));

        let listed_delivery_proofs = client
            .list_delivery_proofs(Some("lot.compute.client"), None)
            .await?;
        assert!(
            listed_delivery_proofs
                .iter()
                .any(|item| item.delivery_proof_id == "delivery.compute.client")
        );

        let listed_indices = client
            .list_compute_indices(Some("ollama.text_generation"))
            .await?;
        assert!(
            listed_indices
                .iter()
                .any(|item| item.index_id == "index.compute.client")
        );
        assert!(
            listed_indices
                .iter()
                .any(|item| item.index_id == "index.compute.client.v2")
        );

        let fetched_product = client.get_compute_product("ollama.text_generation").await?;
        assert_eq!(fetched_product.product_id, "ollama.text_generation");

        let fetched_environment = client
            .get_compute_environment_package("env.openagents.math.basic", Some("2026.03.13"))
            .await?;
        assert_eq!(fetched_environment.family, "evaluation");

        let fetched_lot = client.get_capacity_lot("lot.compute.client").await?;
        assert_eq!(fetched_lot.capacity_lot_id, "lot.compute.client");

        let fetched_instrument = client
            .get_capacity_instrument("instrument.compute.client")
            .await?;
        assert_eq!(
            fetched_instrument.instrument_id,
            "instrument.compute.client"
        );

        let fetched_structured = client
            .get_structured_capacity_instrument("structured.compute.client.reservation")
            .await?;
        assert_eq!(
            fetched_structured.structured_instrument_id,
            "structured.compute.client.reservation"
        );

        let fetched_delivery = client.get_delivery_proof("delivery.compute.client").await?;
        assert_eq!(
            fetched_delivery.delivery_proof_id,
            "delivery.compute.client"
        );

        let fetched_index = client.get_compute_index("index.compute.client.v2").await?;
        assert_eq!(fetched_index.index_id, "index.compute.client.v2");

        let closed = client
            .close_capacity_instrument(close_capacity_instrument_request(
                "instrument.compute.client",
                "idemp.compute.client.instrument.close",
                created_at_ms + 5_000,
            ))
            .await?;
        assert_eq!(
            closed.instrument.status,
            CapacityInstrumentStatus::Defaulted
        );
        assert_eq!(
            closed.instrument.closure_reason,
            Some(CapacityInstrumentClosureReason::Defaulted)
        );
        assert_eq!(
            closed.instrument.non_delivery_reason,
            Some(CapacityNonDeliveryReason::ProviderOffline)
        );

        let structured_closed = client
            .close_structured_capacity_instrument(close_structured_capacity_instrument_request(
                "structured.compute.client.reservation",
                "idemp.compute.client.structured.reservation.close",
                created_at_ms + 11_000,
            ))
            .await?;
        assert_eq!(
            structured_closed.structured_instrument.status,
            StructuredCapacityInstrumentStatus::Cancelled
        );
        assert_eq!(
            structured_closed.legs[0].status,
            CapacityInstrumentStatus::Cancelled
        );

        server.abort();
        Ok(())
    }

    #[tokio::test]
    async fn compute_benchmark_adapter_mmlu_import_roundtrips_into_evals() -> Result<()> {
        let app = build_router(test_config()?);
        let session = create_session_token(&app).await?;

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let local_addr = listener.local_addr()?;
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.expect("serve test app");
        });

        let client = HttpKernelAuthorityClient::new(
            format!("http://{local_addr}"),
            Some(session.access_token.clone()),
        )?;
        let created_at_ms = super::now_unix_ms() as i64;

        client
            .register_compute_environment_package(compute_environment_package_request(
                "env.openagents.math.basic",
                "2026.03.13",
                "idemp.compute.benchmark.environment",
                created_at_ms,
            ))
            .await?;

        let mut product_request = compute_product_request(
            "ollama.text_generation",
            "idemp.compute.benchmark.product",
            created_at_ms + 100,
        );
        product_request
            .product
            .capability_envelope
            .as_mut()
            .expect("capability envelope")
            .environment_binding = Some(ComputeEnvironmentBinding {
            environment_ref: "env.openagents.math.basic".to_string(),
            environment_version: None,
            dataset_ref: Some("dataset://math/basic".to_string()),
            rubric_ref: Some("rubric://math/basic".to_string()),
            evaluator_policy_ref: Some("policy://eval/math/basic".to_string()),
        });
        client.create_compute_product(product_request).await?;
        client
            .create_capacity_lot(capacity_lot_request(
                "lot.compute.client",
                "ollama.text_generation",
                "idemp.compute.benchmark.lot",
                created_at_ms + 200,
            ))
            .await?;
        client
            .create_capacity_instrument(capacity_instrument_request(
                "instrument.compute.client",
                "ollama.text_generation",
                "lot.compute.client",
                "idemp.compute.benchmark.instrument",
                created_at_ms + 300,
            ))
            .await?;
        client
            .record_delivery_proof(delivery_proof_request(
                "delivery.compute.client",
                "ollama.text_generation",
                "lot.compute.client",
                "instrument.compute.client",
                "idemp.compute.benchmark.delivery",
                created_at_ms + 400,
            ))
            .await?;

        let adapted = adapt_compute_benchmark_import(mmlu_benchmark_import_request(
            "eval.benchmark.mmlu.client",
            "idemp.compute.benchmark.mmlu",
            created_at_ms + 500,
            created_at_ms + 800,
        ))
        .map_err(anyhow::Error::msg)?;
        client
            .create_compute_evaluation_run(adapted.create_eval_run)
            .await?;
        client
            .append_compute_evaluation_samples(adapted.append_samples)
            .await?;
        let finalized = client
            .finalize_compute_evaluation_run(adapted.finalize_eval_run)
            .await?;
        assert_eq!(
            finalized
                .eval_run
                .metadata
                .get("benchmark_adapter_kind")
                .and_then(serde_json::Value::as_str),
            Some("mmlu_multiple_choice_v1")
        );
        assert_eq!(
            finalized
                .eval_run
                .summary
                .as_ref()
                .map(|summary| summary.pass_rate_bps),
            Some(Some(5_000))
        );
        let samples = client
            .list_compute_evaluation_samples("eval.benchmark.mmlu.client")
            .await?;
        assert_eq!(samples.len(), 2);
        assert_eq!(
            samples[0]
                .metadata
                .get("benchmark_case")
                .and_then(|value| value.get("subject"))
                .and_then(serde_json::Value::as_str),
            Some("biology")
        );
        assert_eq!(samples[0].status, ComputeEvaluationSampleStatus::Passed);
        assert_eq!(samples[1].status, ComputeEvaluationSampleStatus::Failed);

        server.abort();
        Ok(())
    }

    #[tokio::test]
    async fn compute_product_creation_rejects_raw_accelerator_identity() -> Result<()> {
        let app = build_router(test_config()?);
        let session = create_session_token(&app).await?;
        let mut request = compute_product_request(
            "gpu.h100",
            "idemp.compute.product.invalid-raw-hardware",
            super::now_unix_ms() as i64,
        );
        request.product.resource_class = "gpu.h100".to_string();
        let request = compute_contracts::create_compute_product_request_to_proto(&request)?;

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/kernel/compute/products")
                    .header("authorization", authorization(&session))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&request)?))?,
            )
            .await?;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body: serde_json::Value = response_json(response).await?;
        assert_eq!(body["reason"], "compute_product_resource_class_invalid");
        Ok(())
    }

    #[tokio::test]
    async fn compute_product_creation_rejects_apple_embeddings_launch_claim() -> Result<()> {
        let app = build_router(test_config()?);
        let session = create_session_token(&app).await?;
        let mut request = compute_product_request(
            "apple_foundation_models.embeddings",
            "idemp.compute.product.invalid-apple-embeddings",
            super::now_unix_ms() as i64,
        );
        request.product.capability_envelope = Some(ComputeCapabilityEnvelope {
            backend_family: Some(ComputeBackendFamily::AppleFoundationModels),
            execution_kind: Some(ComputeExecutionKind::LocalInference),
            compute_family: Some(ComputeFamily::Embeddings),
            topology_kind: None,
            provisioning_kind: None,
            proof_posture: None,
            validator_requirements: None,
            artifact_residency: None,
            environment_binding: None,
            checkpoint_binding: None,
            model_policy: Some("embeddings".to_string()),
            model_family: Some("apple.foundation".to_string()),
            host_capability: Some(ComputeHostCapability {
                accelerator_vendor: Some("apple".to_string()),
                accelerator_family: Some("m4_max".to_string()),
                memory_gb: Some(64),
            }),
            apple_platform: Some(ApplePlatformCapability {
                apple_silicon_required: true,
                apple_intelligence_required: true,
                apple_intelligence_available: Some(true),
                minimum_macos_version: Some("15.1".to_string()),
            }),
            gpt_oss_runtime: None,
            latency_ms_p50: Some(150),
            throughput_per_minute: Some(600),
            concurrency_limit: Some(1),
        });
        let request = compute_contracts::create_compute_product_request_to_proto(&request)?;

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/kernel/compute/products")
                    .header("authorization", authorization(&session))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&request)?))?,
            )
            .await?;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body: serde_json::Value = response_json(response).await?;
        assert_eq!(
            body["reason"],
            "compute_product_launch_product_id_unsupported"
        );
        Ok(())
    }

    #[tokio::test]
    async fn data_market_flow_receipts_asset_grant_delivery_and_revocation() -> Result<()> {
        let app = build_router(test_config()?);
        let session = create_session_token(&app).await?;
        let created_at_ms = (super::now_unix_ms() as i64).saturating_sub(8_000);

        let asset = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/kernel/data/assets")
                    .header("authorization", authorization(&session))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&data_asset_request(
                        "asset.data.alpha",
                        "idemp.data.asset.alpha",
                        created_at_ms,
                    ))?))?,
            )
            .await?;
        assert_eq!(asset.status(), StatusCode::OK);
        let asset_payload: RegisterDataAssetResponse = response_json(asset).await?;
        assert_eq!(
            asset_payload.receipt.receipt_type,
            "kernel.data.asset.register.v1"
        );

        let grant = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/kernel/data/grants")
                    .header("authorization", authorization(&session))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&access_grant_request(
                        "grant.data.alpha",
                        "asset.data.alpha",
                        "idemp.data.grant.alpha",
                        created_at_ms + 1_000,
                    ))?))?,
            )
            .await?;
        assert_eq!(grant.status(), StatusCode::OK);
        let grant_payload: CreateAccessGrantResponse = response_json(grant).await?;
        assert_eq!(
            grant_payload.receipt.receipt_type,
            "kernel.data.grant.offer.v1"
        );
        assert_eq!(
            grant_payload.grant.permission_policy.policy_id,
            "policy.grant.grant.data.alpha"
        );

        let accepted = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/kernel/data/grants/grant.data.alpha/accept")
                    .header("authorization", authorization(&session))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(
                        &accept_access_grant_request(
                            "grant.data.alpha",
                            "consumer.data.alpha",
                            "idemp.data.accept.alpha",
                            created_at_ms + 2_000,
                        ),
                    )?))?,
            )
            .await?;
        assert_eq!(accepted.status(), StatusCode::OK);
        let accepted_payload: AcceptAccessGrantResponse = response_json(accepted).await?;
        assert_eq!(
            accepted_payload.receipt.receipt_type,
            "kernel.data.grant.accept.v1"
        );
        assert_eq!(
            accepted_payload.grant.consumer_id.as_deref(),
            Some("consumer.data.alpha")
        );

        let delivery = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/kernel/data/grants/grant.data.alpha/deliveries")
                    .header("authorization", authorization(&session))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&delivery_bundle_request(
                        "delivery.data.alpha",
                        "grant.data.alpha",
                        "idemp.data.delivery.alpha",
                        created_at_ms + 3_000,
                    ))?))?,
            )
            .await?;
        assert_eq!(delivery.status(), StatusCode::OK);
        let delivery_payload: IssueDeliveryBundleResponse = response_json(delivery).await?;
        assert_eq!(
            delivery_payload.receipt.receipt_type,
            "kernel.data.delivery.issue.v1"
        );
        assert_eq!(
            delivery_payload.delivery_bundle.consumer_id,
            "consumer.data.alpha"
        );

        let revocation = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/kernel/data/grants/grant.data.alpha/revoke")
                    .header("authorization", authorization(&session))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(
                        &revoke_access_grant_request(
                            "revocation.data.alpha",
                            "grant.data.alpha",
                            "idemp.data.revoke.alpha",
                            created_at_ms + 4_000,
                        ),
                    )?))?,
            )
            .await?;
        assert_eq!(revocation.status(), StatusCode::OK);
        let revocation_payload: RevokeAccessGrantResponse = response_json(revocation).await?;
        assert_eq!(
            revocation_payload.receipt.receipt_type,
            "kernel.data.revocation.record.v1"
        );
        assert_eq!(
            revocation_payload.revocation.status,
            RevocationStatus::Refunded
        );
        assert_eq!(
            revocation_payload
                .revocation
                .revoked_delivery_bundle_ids
                .len(),
            1
        );

        let listed_assets = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/v1/kernel/data/assets?provider_id=provider.data.alpha&asset_kind=conversation_bundle&status=active")
                    .header("authorization", authorization(&session))
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(listed_assets.status(), StatusCode::OK);
        let listed_assets_payload: Vec<DataAsset> = response_json(listed_assets).await?;
        assert_eq!(listed_assets_payload.len(), 1);
        assert_eq!(listed_assets_payload[0].asset_id, "asset.data.alpha");

        let fetched_asset = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/v1/kernel/data/assets/asset.data.alpha")
                    .header("authorization", authorization(&session))
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(fetched_asset.status(), StatusCode::OK);
        let fetched_asset_payload: DataAsset = response_json(fetched_asset).await?;
        assert_eq!(fetched_asset_payload.asset_kind, "conversation_bundle");

        let listed_grants = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/v1/kernel/data/grants?asset_id=asset.data.alpha&consumer_id=consumer.data.alpha&status=refunded")
                    .header("authorization", authorization(&session))
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(listed_grants.status(), StatusCode::OK);
        let listed_grants_payload: Vec<AccessGrant> = response_json(listed_grants).await?;
        assert_eq!(listed_grants_payload.len(), 1);
        assert_eq!(listed_grants_payload[0].grant_id, "grant.data.alpha");

        let fetched_grant = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/v1/kernel/data/grants/grant.data.alpha")
                    .header("authorization", authorization(&session))
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(fetched_grant.status(), StatusCode::OK);
        let fetched_grant_payload: AccessGrant = response_json(fetched_grant).await?;
        assert_eq!(fetched_grant_payload.status, AccessGrantStatus::Refunded);

        let listed_deliveries = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/v1/kernel/data/deliveries?grant_id=grant.data.alpha&consumer_id=consumer.data.alpha&status=revoked")
                    .header("authorization", authorization(&session))
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(listed_deliveries.status(), StatusCode::OK);
        let listed_deliveries_payload: Vec<DeliveryBundle> =
            response_json(listed_deliveries).await?;
        assert_eq!(listed_deliveries_payload.len(), 1);
        assert_eq!(
            listed_deliveries_payload[0].delivery_bundle_id,
            "delivery.data.alpha"
        );

        let fetched_delivery = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/v1/kernel/data/deliveries/delivery.data.alpha")
                    .header("authorization", authorization(&session))
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(fetched_delivery.status(), StatusCode::OK);
        let fetched_delivery_payload: DeliveryBundle = response_json(fetched_delivery).await?;
        assert_eq!(
            fetched_delivery_payload.status,
            DeliveryBundleStatus::Revoked
        );

        let listed_revocations = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/v1/kernel/data/revocations?grant_id=grant.data.alpha&consumer_id=consumer.data.alpha&status=refunded")
                    .header("authorization", authorization(&session))
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(listed_revocations.status(), StatusCode::OK);
        let listed_revocations_payload: Vec<RevocationReceipt> =
            response_json(listed_revocations).await?;
        assert_eq!(listed_revocations_payload.len(), 1);
        assert_eq!(
            listed_revocations_payload[0].revocation_id,
            "revocation.data.alpha"
        );

        let fetched_revocation = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/v1/kernel/data/revocations/revocation.data.alpha")
                    .header("authorization", authorization(&session))
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(fetched_revocation.status(), StatusCode::OK);
        let fetched_revocation_payload: RevocationReceipt =
            response_json(fetched_revocation).await?;
        assert_eq!(
            fetched_revocation_payload.status,
            RevocationStatus::Refunded
        );

        let stats_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/stats")
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(stats_response.status(), StatusCode::OK);
        let stats: PublicStatsSnapshot = response_json(stats_response).await?;
        assert!(
            stats
                .recent_receipts
                .iter()
                .any(|receipt| { receipt.receipt_type == "kernel.data.asset.registered" })
        );
        assert!(
            stats
                .recent_receipts
                .iter()
                .any(|receipt| { receipt.receipt_type == "kernel.data.grant.offered" })
        );
        assert!(
            stats
                .recent_receipts
                .iter()
                .any(|receipt| { receipt.receipt_type == "kernel.data.delivery.issued" })
        );
        assert!(
            stats
                .recent_receipts
                .iter()
                .any(|receipt| { receipt.receipt_type == "kernel.data.revocation.recorded" })
        );

        Ok(())
    }

    #[tokio::test]
    async fn data_http_client_roundtrips_authority_reads() -> Result<()> {
        let app = build_router(test_config()?);
        let session = create_session_token(&app).await?;

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let local_addr = listener.local_addr()?;
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.expect("serve test app");
        });

        let client = HttpKernelAuthorityClient::new(
            format!("http://{local_addr}"),
            Some(session.access_token.clone()),
        )?;
        let created_at_ms = (super::now_unix_ms() as i64).saturating_sub(8_000);

        client
            .register_data_asset(data_asset_request(
                "asset.data.client",
                "idemp.data.client.asset",
                created_at_ms,
            ))
            .await?;
        client
            .create_access_grant(access_grant_request(
                "grant.data.client",
                "asset.data.client",
                "idemp.data.client.grant",
                created_at_ms + 1_000,
            ))
            .await?;
        client
            .accept_access_grant(accept_access_grant_request(
                "grant.data.client",
                "consumer.data.client",
                "idemp.data.client.accept",
                created_at_ms + 2_000,
            ))
            .await?;
        client
            .issue_delivery_bundle(delivery_bundle_request(
                "delivery.data.client",
                "grant.data.client",
                "idemp.data.client.delivery",
                created_at_ms + 3_000,
            ))
            .await?;
        client
            .revoke_access_grant(revoke_access_grant_request(
                "revocation.data.client",
                "grant.data.client",
                "idemp.data.client.revoke",
                created_at_ms + 4_000,
            ))
            .await?;

        let assets = client
            .list_data_assets(
                Some("provider.data.alpha"),
                Some("conversation_bundle"),
                Some(DataAssetStatus::Active),
            )
            .await?;
        assert!(
            assets
                .iter()
                .any(|asset| asset.asset_id == "asset.data.client")
        );

        let asset = client.get_data_asset("asset.data.client").await?;
        assert_eq!(asset.provider_id, "provider.data.alpha");

        let grants = client
            .list_access_grants(
                Some("asset.data.client"),
                Some("provider.data.alpha"),
                Some("consumer.data.client"),
                Some(AccessGrantStatus::Refunded),
            )
            .await?;
        assert_eq!(grants.len(), 1);
        assert_eq!(grants[0].grant_id, "grant.data.client");

        let grant = client.get_access_grant("grant.data.client").await?;
        assert_eq!(grant.status, AccessGrantStatus::Refunded);

        let deliveries = client
            .list_delivery_bundles(
                Some("asset.data.client"),
                Some("grant.data.client"),
                Some("provider.data.alpha"),
                Some("consumer.data.client"),
                Some(DeliveryBundleStatus::Revoked),
            )
            .await?;
        assert_eq!(deliveries.len(), 1);
        assert_eq!(deliveries[0].delivery_bundle_id, "delivery.data.client");

        let delivery = client.get_delivery_bundle("delivery.data.client").await?;
        assert_eq!(delivery.status, DeliveryBundleStatus::Revoked);

        let revocations = client
            .list_revocations(
                Some("asset.data.client"),
                Some("grant.data.client"),
                Some("provider.data.alpha"),
                Some("consumer.data.client"),
                Some(RevocationStatus::Refunded),
            )
            .await?;
        assert_eq!(revocations.len(), 1);
        assert_eq!(revocations[0].revocation_id, "revocation.data.client");

        let revocation = client.get_revocation("revocation.data.client").await?;
        assert_eq!(revocation.status, RevocationStatus::Refunded);

        server.abort();
        Ok(())
    }

    #[tokio::test]
    async fn liquidity_market_flow_receipts_quotes_routes_envelopes_and_settlement() -> Result<()> {
        let app = build_router(test_config()?);
        let session = create_session_token(&app).await?;
        let minute_start_ms =
            floor_to_minute_utc((super::now_unix_ms() as i64).saturating_sub(30_000));
        let created_at_ms = minute_start_ms.saturating_add(12_000);

        let partition = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/kernel/liquidity/reserve_partitions")
                    .header("authorization", authorization(&session))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&reserve_partition_request(
                        "reserve.alpha",
                        "idemp.reserve.alpha",
                        created_at_ms,
                    ))?))?,
            )
            .await?;
        assert_eq!(partition.status(), StatusCode::OK);
        let partition_payload: RegisterReservePartitionResponse = response_json(partition).await?;
        assert_eq!(
            partition_payload.receipt.receipt_type,
            "kernel.liquidity.reserve.partition.register.v1"
        );

        let quote = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/kernel/liquidity/quotes")
                    .header("authorization", authorization(&session))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&liquidity_quote_request(
                        "quote.alpha",
                        "idemp.quote.alpha",
                        created_at_ms + 1_000,
                    ))?))?,
            )
            .await?;
        assert_eq!(quote.status(), StatusCode::OK);
        let quote_payload: CreateLiquidityQuoteResponse = response_json(quote).await?;
        assert_eq!(
            quote_payload.receipt.receipt_type,
            "kernel.liquidity.quote.create.v1"
        );

        let route = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/kernel/liquidity/routes")
                    .header("authorization", authorization(&session))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&route_plan_request(
                        "route.alpha",
                        "quote.alpha",
                        "idemp.route.alpha",
                        created_at_ms + 2_000,
                    ))?))?,
            )
            .await?;
        assert_eq!(route.status(), StatusCode::OK);
        let route_payload: SelectRoutePlanResponse = response_json(route).await?;
        assert_eq!(
            route_payload.receipt.receipt_type,
            "kernel.liquidity.route.select.v1"
        );
        assert!(route_payload.route_plan.quote_receipt.is_some());

        let envelope = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/kernel/liquidity/envelopes")
                    .header("authorization", authorization(&session))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(
                        &liquidity_envelope_request(
                            "envelope.alpha",
                            "route.alpha",
                            "quote.alpha",
                            "reserve.alpha",
                            "idemp.envelope.alpha",
                            created_at_ms + 3_000,
                        ),
                    )?))?,
            )
            .await?;
        assert_eq!(envelope.status(), StatusCode::OK);
        let envelope_payload: IssueLiquidityEnvelopeResponse = response_json(envelope).await?;
        assert_eq!(
            envelope_payload.receipt.receipt_type,
            "kernel.liquidity.envelope.issue.v1"
        );

        let settlement = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/kernel/liquidity/settlements")
                    .header("authorization", authorization(&session))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&settlement_intent_request(
                        "settlement.alpha",
                        "route.alpha",
                        "quote.alpha",
                        "envelope.alpha",
                        "reserve.alpha",
                        "idemp.settlement.alpha",
                        created_at_ms + 4_000,
                    ))?))?,
            )
            .await?;
        assert_eq!(settlement.status(), StatusCode::OK);
        let settlement_payload: ExecuteSettlementIntentResponse = response_json(settlement).await?;
        assert_eq!(
            settlement_payload.receipt.receipt_type,
            "kernel.liquidity.settlement.execute.v1"
        );
        assert_eq!(
            settlement_payload.settlement_intent.status,
            SettlementIntentStatus::Settled
        );

        let adjusted = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/kernel/liquidity/reserve_partitions/reserve.alpha/adjust")
                    .header("authorization", authorization(&session))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(
                        &adjust_reserve_partition_request(
                            "reserve.alpha",
                            "idemp.reserve.adjust.alpha",
                            created_at_ms + 5_000,
                        ),
                    )?))?,
            )
            .await?;
        assert_eq!(adjusted.status(), StatusCode::OK);
        let adjusted_payload: AdjustReservePartitionResponse = response_json(adjusted).await?;
        assert_eq!(
            adjusted_payload.receipt.receipt_type,
            "kernel.liquidity.reserve.partition.adjust.v1"
        );
        assert_eq!(
            adjusted_payload.reserve_partition.status,
            ReservePartitionStatus::Adjusted
        );

        let snapshot_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!("/v1/kernel/snapshots/{minute_start_ms}"))
                    .header("authorization", authorization(&session))
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(snapshot_response.status(), StatusCode::OK);
        let snapshot: super::EconomySnapshot = response_json(snapshot_response).await?;
        assert_eq!(snapshot.liquidity_quotes_active, 1);
        assert_eq!(snapshot.liquidity_route_plans_active, 0);
        assert_eq!(snapshot.liquidity_envelopes_open, 0);
        assert_eq!(snapshot.liquidity_settlements_24h, 1);
        assert_eq!(snapshot.liquidity_reserve_partitions_active, 1);
        assert_eq!(snapshot.liquidity_value_moved_24h, 2_490);

        let stats_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/stats")
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(stats_response.status(), StatusCode::OK);
        let stats: PublicStatsSnapshot = response_json(stats_response).await?;
        assert_eq!(stats.liquidity_quotes_active, 1);
        assert_eq!(stats.liquidity_route_plans_active, 0);
        assert_eq!(stats.liquidity_envelopes_open, 0);
        assert_eq!(stats.liquidity_settlements_24h, 1);
        assert_eq!(stats.liquidity_reserve_partitions_active, 1);
        assert_eq!(stats.liquidity_value_moved_24h, 2_490);
        assert!(
            stats
                .recent_receipts
                .iter()
                .any(|receipt| receipt.receipt_type == "kernel.liquidity.quote.created")
        );
        assert!(
            stats
                .recent_receipts
                .iter()
                .any(|receipt| receipt.receipt_type == "kernel.liquidity.route.selected")
        );
        assert!(
            stats
                .recent_receipts
                .iter()
                .any(|receipt| receipt.receipt_type == "kernel.liquidity.envelope.issued")
        );
        assert!(
            stats
                .recent_receipts
                .iter()
                .any(|receipt| receipt.receipt_type == "kernel.liquidity.settlement.executed")
        );
        assert!(
            stats.recent_receipts.iter().any(
                |receipt| receipt.receipt_type == "kernel.liquidity.reserve_partition.adjusted"
            )
        );

        Ok(())
    }

    #[tokio::test]
    async fn risk_market_flow_receipts_offers_positions_claims_and_signals() -> Result<()> {
        let app = build_router(test_config()?);
        let session = create_session_token(&app).await?;
        let minute_start_ms =
            floor_to_minute_utc((super::now_unix_ms() as i64).saturating_sub(30_000));
        let created_at_ms = minute_start_ms.saturating_add(14_000);
        let outcome_ref = "outcome.compute.alpha";

        let offer = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/kernel/risk/coverage_offers")
                    .header("authorization", authorization(&session))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&coverage_offer_request(
                        "coverage_offer.alpha",
                        outcome_ref,
                        "idemp.coverage_offer.alpha",
                        created_at_ms,
                    ))?))?,
            )
            .await?;
        assert_eq!(offer.status(), StatusCode::OK);
        let offer_payload: PlaceCoverageOfferResponse = response_json(offer).await?;
        assert_eq!(
            offer_payload.receipt.receipt_type,
            "kernel.risk.coverage_offer.place.v1"
        );

        let binding = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/kernel/risk/coverage_bindings")
                    .header("authorization", authorization(&session))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&coverage_binding_request(
                        "coverage_binding.alpha",
                        outcome_ref,
                        vec!["coverage_offer.alpha".to_string()],
                        "idemp.coverage_binding.alpha",
                        created_at_ms + 1_000,
                    ))?))?,
            )
            .await?;
        assert_eq!(binding.status(), StatusCode::OK);
        let binding_payload: BindCoverageResponse = response_json(binding).await?;
        assert_eq!(
            binding_payload.receipt.receipt_type,
            "kernel.risk.coverage_binding.bind.v1"
        );
        assert_eq!(
            binding_payload.coverage_binding.total_coverage.amount,
            MoneyAmount::AmountSats(1_500)
        );

        let position = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/kernel/risk/positions")
                    .header("authorization", authorization(&session))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(
                        &prediction_position_request(
                            "position.alpha",
                            outcome_ref,
                            "idemp.position.alpha",
                            created_at_ms + 2_000,
                        ),
                    )?))?,
            )
            .await?;
        assert_eq!(position.status(), StatusCode::OK);
        let position_payload: CreatePredictionPositionResponse = response_json(position).await?;
        assert_eq!(
            position_payload.receipt.receipt_type,
            "kernel.risk.position.create.v1"
        );
        assert_eq!(
            position_payload.prediction_position.side,
            PredictionSide::Fail
        );

        let claim = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/kernel/risk/claims")
                    .header("authorization", authorization(&session))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&risk_claim_request(
                        "claim.alpha",
                        "coverage_binding.alpha",
                        outcome_ref,
                        "idemp.claim.alpha",
                        created_at_ms + 3_000,
                    ))?))?,
            )
            .await?;
        assert_eq!(claim.status(), StatusCode::OK);
        let claim_payload: CreateRiskClaimResponse = response_json(claim).await?;
        assert_eq!(
            claim_payload.receipt.receipt_type,
            "kernel.risk.claim.create.v1"
        );

        let resolved_claim = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/kernel/risk/claims/claim.alpha/resolve")
                    .header("authorization", authorization(&session))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(
                        &resolve_risk_claim_request(
                            "claim.alpha",
                            "idemp.claim.resolve.alpha",
                            created_at_ms + 4_000,
                        ),
                    )?))?,
            )
            .await?;
        assert_eq!(resolved_claim.status(), StatusCode::OK);
        let resolved_claim_payload: ResolveRiskClaimResponse =
            response_json(resolved_claim).await?;
        assert_eq!(
            resolved_claim_payload.receipt.receipt_type,
            "kernel.risk.claim.resolve.v1"
        );
        assert_eq!(
            resolved_claim_payload.risk_claim.status,
            RiskClaimStatus::Paid
        );

        let signal = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/kernel/risk/signals")
                    .header("authorization", authorization(&session))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&risk_signal_request(
                        "signal.alpha",
                        outcome_ref,
                        "idemp.signal.alpha",
                        created_at_ms + 5_000,
                    ))?))?,
            )
            .await?;
        assert_eq!(signal.status(), StatusCode::OK);
        let signal_payload: PublishRiskSignalResponse = response_json(signal).await?;
        assert_eq!(
            signal_payload.receipt.receipt_type,
            "kernel.risk.signal.publish.v1"
        );
        assert_eq!(
            signal_payload.risk_signal.verification_tier_floor,
            Some(VerificationTier::Tier2Heterogeneous)
        );
        assert_eq!(signal_payload.risk_signal.collateral_multiplier_bps, 15_000);
        assert_eq!(signal_payload.risk_signal.autonomy_mode, "guarded");

        let snapshot_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!("/v1/kernel/snapshots/{minute_start_ms}"))
                    .header("authorization", authorization(&session))
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(snapshot_response.status(), StatusCode::OK);
        let snapshot: super::EconomySnapshot = response_json(snapshot_response).await?;
        assert_eq!(snapshot.risk_coverage_offers_open, 0);
        assert_eq!(snapshot.risk_coverage_bindings_active, 0);
        assert_eq!(snapshot.risk_prediction_positions_open, 1);
        assert_eq!(snapshot.risk_claims_open, 0);
        assert_eq!(snapshot.risk_signals_active, 1);
        assert_eq!(snapshot.risk_implied_fail_probability_bps, 6_200);
        assert_eq!(snapshot.risk_calibration_score, 0.55);
        assert_eq!(snapshot.risk_coverage_concentration_hhi, 1.0);
        assert_eq!(
            snapshot.liability_premiums_collected_24h.amount,
            MoneyAmount::AmountSats(120)
        );
        assert_eq!(
            snapshot.claims_paid_24h.amount,
            MoneyAmount::AmountSats(900)
        );
        assert_eq!(snapshot.loss_ratio, 7.5);

        let stats_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/stats")
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(stats_response.status(), StatusCode::OK);
        let stats: PublicStatsSnapshot = response_json(stats_response).await?;
        assert_eq!(stats.risk_coverage_offers_open, 0);
        assert_eq!(stats.risk_coverage_bindings_active, 0);
        assert_eq!(stats.risk_prediction_positions_open, 1);
        assert_eq!(stats.risk_claims_open, 0);
        assert_eq!(stats.risk_signals_active, 1);
        assert_eq!(stats.risk_implied_fail_probability_bps, 6_200);
        assert_eq!(stats.risk_calibration_score, 0.55);
        assert_eq!(stats.risk_coverage_concentration_hhi, 1.0);
        assert!(
            stats
                .recent_receipts
                .iter()
                .any(|receipt| receipt.receipt_type == "kernel.risk.coverage_offer.placed")
        );
        assert!(
            stats
                .recent_receipts
                .iter()
                .any(|receipt| receipt.receipt_type == "kernel.risk.coverage_binding.bound")
        );
        assert!(
            stats
                .recent_receipts
                .iter()
                .any(|receipt| receipt.receipt_type == "kernel.risk.position.created")
        );
        assert!(
            stats
                .recent_receipts
                .iter()
                .any(|receipt| receipt.receipt_type == "kernel.risk.claim.created")
        );
        assert!(
            stats
                .recent_receipts
                .iter()
                .any(|receipt| receipt.receipt_type == "kernel.risk.claim.resolved")
        );
        assert!(
            stats
                .recent_receipts
                .iter()
                .any(|receipt| receipt.receipt_type == "kernel.risk.signal.published")
        );

        Ok(())
    }

    #[tokio::test]
    async fn kernel_authority_enforces_idempotency_per_caller() -> Result<()> {
        let app = build_router(test_config()?);
        let session = create_session_token(&app).await?;
        let request = work_unit_request("work_unit.idempotent", "idemp.shared", 1_762_000_200_123);

        let first_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/kernel/work_units")
                    .header("authorization", authorization(&session))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&request)?))?,
            )
            .await?;
        assert_eq!(first_response.status(), StatusCode::OK);
        let first: CreateWorkUnitResponse = response_json(first_response).await?;

        let replay_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/kernel/work_units")
                    .header("authorization", authorization(&session))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&request)?))?,
            )
            .await?;
        assert_eq!(replay_response.status(), StatusCode::OK);
        let replay: CreateWorkUnitResponse = response_json(replay_response).await?;
        assert_eq!(replay.receipt.receipt_id, first.receipt.receipt_id);
        assert_eq!(replay.receipt.canonical_hash, first.receipt.canonical_hash);

        let mut conflicting = request.clone();
        conflicting.work_unit.metadata = json!({
            "summary": "Changed payload with same idempotency key."
        });
        let conflict_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/kernel/work_units")
                    .header("authorization", authorization(&session))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&conflicting)?))?,
            )
            .await?;
        assert_eq!(conflict_response.status(), StatusCode::CONFLICT);
        let conflict: serde_json::Value = response_json(conflict_response).await?;
        assert_eq!(conflict["reason"], "kernel_idempotency_conflict");

        Ok(())
    }

    #[tokio::test]
    async fn kernel_stream_routes_require_auth_and_return_sse() -> Result<()> {
        let app = build_router(test_config()?);
        let session = create_session_token(&app).await?;

        let unauthorized_receipts = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/v1/kernel/stream/receipts")
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(unauthorized_receipts.status(), StatusCode::UNAUTHORIZED);

        let receipts_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/v1/kernel/stream/receipts")
                    .header("authorization", authorization(&session))
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(receipts_response.status(), StatusCode::OK);
        assert!(
            receipts_response
                .headers()
                .get("content-type")
                .and_then(|value| value.to_str().ok())
                .is_some_and(|value| value.starts_with("text/event-stream"))
        );

        let snapshots_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/v1/kernel/stream/snapshots")
                    .header("authorization", authorization(&session))
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(snapshots_response.status(), StatusCode::OK);
        assert!(
            snapshots_response
                .headers()
                .get("content-type")
                .and_then(|value| value.to_str().ok())
                .is_some_and(|value| value.starts_with("text/event-stream"))
        );

        Ok(())
    }
}
