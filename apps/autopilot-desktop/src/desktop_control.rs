use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fs;
use std::net::{IpAddr, SocketAddr};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread::JoinHandle;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::routing::{get, post};
use axum::{Json, Router};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use chrono::{TimeZone, Utc};
use openagents_kernel_core::authority::KernelAuthority;
use openagents_kernel_core::compute::{
    CapacityInstrument, CapacityInstrumentKind, ComputeAcceptedOutcome, ComputeAcceptedOutcomeKind,
    ComputeCapabilityEnvelope, ComputeProofPosture, ComputeProvisioningKind, ComputeTopologyKind,
    ComputeTrainingRun, ComputeTrainingRunStatus, ComputeValidatorChallengeSnapshot,
    ComputeValidatorChallengeStatus, DeliveryProof, DeliveryProofStatus,
    StructuredCapacityInstrument, StructuredCapacityInstrumentKind,
    StructuredCapacityInstrumentStatus,
};
use openagents_kernel_core::ids::sha256_prefixed_text;
use openagents_kernel_core::receipts::{Money, MoneyAmount};
use openagents_provider_substrate::ProviderDesiredMode;
use psionic_apple_fm::{AppleFmAdapterInventoryEntry, AppleFmAdapterSelection};
use psionic_sandbox::{
    InMemorySandboxJobService, ProviderSandboxBackgroundJobSnapshot, ProviderSandboxEntrypointType,
    ProviderSandboxExecutionControls, ProviderSandboxFileTransferReceipt,
    ProviderSandboxJobRequest, ProviderSandboxProfile,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::sync::{Notify, mpsc as tokio_mpsc, oneshot};

pub use crate::provider_inventory::{
    DesktopControlInventoryProductStatus, DesktopControlInventoryProjectionStatus,
    DesktopControlInventorySectionStatus, DesktopControlInventoryStatus,
};

use crate::app_state::{
    DefaultNip28ChannelConfig, MISSION_CONTROL_BUY_MODE_BUDGET_SATS,
    MISSION_CONTROL_BUY_MODE_INTERVAL_MILLIS, MissionControlLocalRuntimeAction,
    MissionControlLocalRuntimeLane, MissionControlLocalRuntimePolicy, RenderState,
    SnapshotTimingSample, mission_control_buy_mode_interval_label,
    mission_control_local_runtime_view_model,
};
use crate::apple_adapter_training_control::{
    AppleAdapterOperatorLaunchRequest, AppleAdapterOperatorStageState,
};
use crate::apple_fm_bridge::{
    AppleFmBridgeCommand, AppleFmWorkbenchCommand, AppleFmWorkbenchOperation,
};
use crate::bitcoin_display::format_sats_amount;
use crate::local_inference_runtime::{LocalInferenceRuntimeCommand, LocalRuntimeDiagnostics};
use crate::local_runtime_capabilities::{
    LocalRuntimeWorkbenchAction, active_local_runtime_capability_surface,
};
use crate::pane_registry::{enabled_pane_specs, pane_spec};
use crate::pane_system::{BuyModePaymentsPaneAction, PaneController, ProviderControlPaneAction};
use crate::research_control;
use crate::spark_pane::{PayInvoicePaneAction, SparkPaneAction};

const DESKTOP_CONTROL_SCHEMA_VERSION: u16 = 13;
const DESKTOP_CONTROL_SYNC_INTERVAL: Duration = Duration::from_millis(250);
const DESKTOP_CONTROL_MANIFEST_SCHEMA_VERSION: u16 = 1;
const DESKTOP_CONTROL_MANIFEST_FILENAME: &str = "desktop-control.json";
const DESKTOP_CONTROL_LOG_TAIL_LIMIT: usize = 64;
const DESKTOP_CONTROL_EVENT_BUFFER_LIMIT: usize = 512;
const DESKTOP_CONTROL_EVENT_QUERY_LIMIT: usize = 128;
const DESKTOP_CONTROL_EVENT_WAIT_TIMEOUT_MS: u64 = 20_000;
const DESKTOP_CONTROL_COMPUTE_HISTORY_REFRESH_INTERVAL_MS: u64 = 15_000;
const DESKTOP_CONTROL_COMPUTE_HISTORY_LIMIT: usize = 8;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DesktopControlRuntimeConfig {
    pub listen_addr: SocketAddr,
    pub auth_token: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct DesktopControlSnapshot {
    pub schema_version: u16,
    pub snapshot_revision: u64,
    pub state_signature: String,
    pub generated_at_epoch_ms: u64,
    pub session: DesktopControlSessionStatus,
    pub mission_control: DesktopControlMissionControlStatus,
    pub provider: DesktopControlProviderStatus,
    pub local_runtime: DesktopControlLocalRuntimeStatus,
    pub gpt_oss: DesktopControlGptOssStatus,
    pub apple_fm: DesktopControlAppleFmStatus,
    pub wallet: DesktopControlWalletStatus,
    pub tunnels: DesktopControlTunnelsStatus,
    pub inventory: DesktopControlInventoryStatus,
    pub buyer_procurement: DesktopControlBuyerProcurementStatus,
    pub cluster: DesktopControlClusterStatus,
    pub sandbox: DesktopControlSandboxStatus,
    pub training: DesktopControlTrainingStatus,
    pub proofs: DesktopControlProofStatus,
    pub challenges: DesktopControlChallengeStatus,
    pub buy_mode: DesktopControlBuyModeStatus,
    pub active_job: Option<DesktopControlActiveJobStatus>,
    pub nip28: DesktopControlNip28Status,
    pub recent_logs: Vec<String>,
    pub last_command: Option<DesktopControlLastCommandStatus>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct DesktopControlSessionStatus {
    pub pid: u32,
    pub shell_mode: String,
    pub dev_mode_enabled: bool,
    pub buy_mode_surface_enabled: bool,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct DesktopControlMissionControlStatus {
    pub last_action: Option<String>,
    pub last_error: Option<String>,
    pub can_go_online: bool,
    pub blocker_codes: Vec<String>,
    pub log_line_count: usize,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct DesktopControlProviderStatus {
    pub mode: String,
    pub runtime_mode: String,
    pub desired_mode_hint: String,
    pub online: bool,
    pub blocker_codes: Vec<String>,
    pub connected_relays: usize,
    pub degraded_reason_code: Option<String>,
    pub last_request_event_id: Option<String>,
    pub last_action: Option<String>,
    pub last_error: Option<String>,
    pub relay_urls: Vec<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct DesktopControlLocalRuntimeStatus {
    pub policy: String,
    pub lane: Option<String>,
    pub runtime_ready: bool,
    pub go_online_ready: bool,
    pub supports_sell_compute: bool,
    pub workbench_label: String,
    pub supports_run_text: bool,
    pub supports_streaming: bool,
    pub supports_structured: bool,
    pub supports_model_management: bool,
    pub supports_sessions: bool,
    pub show_action_button: bool,
    pub action: String,
    pub action_enabled: bool,
    pub action_label: String,
    pub model_label: String,
    pub backend_label: String,
    pub load_label: String,
    pub go_online_hint: Option<String>,
    pub status_stream: String,
    pub status_line: String,
    pub detail_lines: Vec<String>,
    pub diagnostics: LocalRuntimeDiagnostics,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct DesktopControlGptOssStatus {
    pub detected: bool,
    pub backend: Option<String>,
    pub reachable: bool,
    pub ready: bool,
    pub busy: bool,
    pub supports_sell_compute: bool,
    pub artifact_present: bool,
    pub loaded: bool,
    pub configured_model: Option<String>,
    pub ready_model: Option<String>,
    pub configured_model_path: Option<String>,
    pub loaded_models: Vec<String>,
    pub last_action: Option<String>,
    pub last_error: Option<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct DesktopControlAppleFmStatus {
    pub reachable: bool,
    pub ready: bool,
    pub model_available: bool,
    pub ready_model: Option<String>,
    pub adapter_inventory_supported: bool,
    pub adapter_attach_supported: bool,
    pub loaded_adapters: Vec<AppleFmAdapterInventoryEntry>,
    pub active_session_id: Option<String>,
    pub active_session_adapter: Option<AppleFmAdapterSelection>,
    pub bridge_status: Option<String>,
    pub last_action: Option<String>,
    pub last_error: Option<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct DesktopControlWalletStatus {
    pub balance_sats: u64,
    pub balance_known: bool,
    pub balance_reconciling: bool,
    pub network: String,
    pub network_status: String,
    pub can_withdraw: bool,
    pub withdraw_block_reason: Option<String>,
    pub last_action: Option<String>,
    pub last_error: Option<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct DesktopControlNip90SentPaymentsReport {
    pub report_date: Option<String>,
    pub window_start_epoch_seconds: u64,
    pub window_end_epoch_seconds: u64,
    pub window_start_rfc3339: String,
    pub window_end_rfc3339: String,
    pub payment_count: usize,
    pub total_sats_sent: u64,
    pub total_fee_sats: u64,
    pub total_wallet_debit_sats: u64,
    pub connected_relay_count: usize,
    pub relay_urls_considered: Vec<String>,
    pub deduped_request_count: usize,
    pub degraded_binding_count: usize,
    pub generated_at_epoch_seconds: u64,
    pub generated_at_rfc3339: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct DesktopControlTunnelServiceStatus {
    pub service_id: String,
    pub kind: String,
    pub protocol: String,
    pub active: bool,
    pub allowed_peer_count: usize,
    pub request_count: u64,
    pub response_count: u64,
    pub bytes_in: u64,
    pub bytes_out: u64,
    pub last_error: Option<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct DesktopControlTunnelStatus {
    pub tunnel_id: String,
    pub direction: String,
    pub peer_node_id: String,
    pub service_id: String,
    pub state: String,
    pub transport_class: String,
    pub session_path_kind: String,
    pub request_count: u64,
    pub response_count: u64,
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub close_reason: Option<String>,
    pub last_error: Option<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct DesktopControlTunnelsStatus {
    pub available: bool,
    pub approved_service_count: usize,
    pub active_service_count: usize,
    pub open_tunnel_count: usize,
    pub services: Vec<DesktopControlTunnelServiceStatus>,
    pub tunnels: Vec<DesktopControlTunnelStatus>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct DesktopControlBuyerProcurementQuoteStatus {
    pub quote_id: String,
    pub rfq_id: String,
    pub product_id: String,
    pub provider_id: String,
    pub compute_family: String,
    pub backend: String,
    pub execution: String,
    pub topology: String,
    pub provisioning: String,
    pub proof_posture: String,
    pub requested_quantity: u64,
    pub available_quantity: u64,
    pub price_sats: u64,
    pub delivery_window_label: String,
    pub environment_ref: Option<String>,
    pub sandbox_profile_ref: Option<String>,
    pub source_badge: String,
    pub terms_label: String,
    pub capability_summary: String,
    pub collateral_summary: Option<String>,
    pub remedy_summary: Option<String>,
    pub selected: bool,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct DesktopControlBuyerProcurementOrderStatus {
    pub order_id: String,
    pub rfq_id: String,
    pub quote_id: String,
    pub instrument_id: String,
    pub product_id: String,
    pub provider_id: String,
    pub compute_family: String,
    pub backend: String,
    pub execution: String,
    pub topology: String,
    pub provisioning: String,
    pub proof_posture: String,
    pub quantity: u64,
    pub price_sats: u64,
    pub delivery_window_label: String,
    pub environment_ref: Option<String>,
    pub sandbox_profile_ref: Option<String>,
    pub collateral_summary: Option<String>,
    pub remedy_summary: Option<String>,
    pub authority_status: String,
    pub accepted_at_epoch_seconds: u64,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct DesktopControlBuyerProcurementStatus {
    pub load_state: String,
    pub quote_mode: String,
    pub last_action: Option<String>,
    pub last_error: Option<String>,
    pub last_spot_rfq_summary: Option<String>,
    pub last_forward_rfq_summary: Option<String>,
    pub selected_spot_quote_id: Option<String>,
    pub selected_forward_quote_id: Option<String>,
    pub spot_quotes: Vec<DesktopControlBuyerProcurementQuoteStatus>,
    pub forward_quotes: Vec<DesktopControlBuyerProcurementQuoteStatus>,
    pub accepted_spot_orders: Vec<DesktopControlBuyerProcurementOrderStatus>,
    pub accepted_forward_orders: Vec<DesktopControlBuyerProcurementOrderStatus>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct DesktopControlClusterMemberStatus {
    pub peer_node_id: String,
    pub state: String,
    pub transport_class: String,
    pub session_path_kind: String,
    pub last_error: Option<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct DesktopControlClusterStatus {
    pub available: bool,
    pub topology_label: String,
    pub member_count: usize,
    pub members: Vec<DesktopControlClusterMemberStatus>,
    pub last_error: Option<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct DesktopControlSandboxProfileStatus {
    pub profile_id: String,
    pub profile_digest: String,
    pub execution_class: String,
    pub runtime_kind: String,
    pub runtime_ready: bool,
    pub capability_summary: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct DesktopControlSandboxJobStatus {
    pub job_id: String,
    pub profile_id: String,
    pub profile_digest: String,
    pub compute_product_id: String,
    pub state: String,
    pub created_at_epoch_ms: i64,
    pub updated_at_epoch_ms: i64,
    pub upload_count: usize,
    pub download_count: usize,
    pub last_detail: Option<String>,
    pub terminal_receipt_type: Option<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct DesktopControlSandboxStatus {
    pub available: bool,
    pub declared_profile_count: usize,
    pub ready_profile_count: usize,
    pub job_count: usize,
    pub active_job_count: usize,
    pub profiles: Vec<DesktopControlSandboxProfileStatus>,
    pub jobs: Vec<DesktopControlSandboxJobStatus>,
    pub last_error: Option<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct DesktopControlTrainingRunStatus {
    pub training_run_id: String,
    pub status: String,
    pub training_policy_ref: String,
    pub environment_ref: String,
    pub environment_version: Option<String>,
    pub checkpoint_family: String,
    pub validator_policy_ref: String,
    pub benchmark_package_count: usize,
    pub rollout_verification_eval_run_count: usize,
    pub expected_step_count: Option<u64>,
    pub completed_step_count: Option<u64>,
    pub final_checkpoint_ref: Option<String>,
    pub promotion_checkpoint_ref: Option<String>,
    pub accepted_outcome_id: Option<String>,
    pub best_eval_score_bps: Option<u32>,
    pub control_plane_state: String,
    pub artifact_plane_state: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct DesktopControlTrainingParticipantStatus {
    pub participant_id: String,
    pub visible_reason: String,
    pub admitted: bool,
    pub contributing: bool,
    pub priority_label: String,
    pub deweight_reason: Option<String>,
    pub exclusion_reason: Option<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct DesktopControlAppleAdapterOperatorAuthorityStatus {
    pub core_environment_ref: Option<String>,
    pub benchmark_environment_ref: Option<String>,
    pub benchmark_package_ref: Option<String>,
    pub validator_policy_ref: Option<String>,
    pub training_policy_ref: Option<String>,
    pub training_run_id: Option<String>,
    pub held_out_eval_run_id: Option<String>,
    pub runtime_validation_eval_run_id: Option<String>,
    pub accepted_outcome_id: Option<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct DesktopControlAppleAdapterOperatorRunStatus {
    pub run_id: String,
    pub package_name: String,
    pub author: String,
    pub description: String,
    pub license: String,
    pub train_dataset_path: String,
    pub held_out_dataset_path: String,
    pub created_at_epoch_ms: u64,
    pub updated_at_epoch_ms: u64,
    pub launched_at_epoch_ms: Option<u64>,
    pub evaluated_at_epoch_ms: Option<u64>,
    pub exported_at_epoch_ms: Option<u64>,
    pub accepted_at_epoch_ms: Option<u64>,
    pub launch_state: String,
    pub export_state: String,
    pub evaluation_state: String,
    pub acceptance_state: String,
    pub run_directory: String,
    pub staged_package_path: Option<String>,
    pub exported_package_path: Option<String>,
    pub completed_step_count: Option<u64>,
    pub expected_step_count: Option<u64>,
    pub average_loss_label: Option<String>,
    pub held_out_pass_rate_bps: Option<u32>,
    pub held_out_average_score_bps: Option<u32>,
    pub runtime_smoke_passed: Option<bool>,
    pub runtime_smoke_digest: Option<String>,
    pub package_digest: Option<String>,
    pub adapter_identifier: Option<String>,
    pub authority: DesktopControlAppleAdapterOperatorAuthorityStatus,
    pub last_action: Option<String>,
    pub last_error: Option<String>,
    pub log_lines: Vec<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct DesktopControlAppleAdapterOperatorStatus {
    pub available: bool,
    pub workflow_state: String,
    pub schema_version: Option<u16>,
    pub storage_path: Option<String>,
    pub last_action: Option<String>,
    pub last_error: Option<String>,
    pub run_count: usize,
    pub active_run_count: usize,
    pub accepted_run_count: usize,
    pub exported_run_count: usize,
    pub runs: Vec<DesktopControlAppleAdapterOperatorRunStatus>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct DesktopControlTrainingStatus {
    pub available: bool,
    pub source: String,
    pub control_plane_state: String,
    pub artifact_plane_state: String,
    pub last_synced_at_epoch_ms: Option<u64>,
    pub run_count: usize,
    pub active_run_count: usize,
    pub accepted_run_count: usize,
    pub accepted_outcome_count: usize,
    pub environment_versions: Vec<String>,
    pub checkpoint_refs: Vec<String>,
    pub contributor_set_revision: Option<String>,
    pub contributor_reselection_timing: Option<String>,
    pub admitted_participant_count: usize,
    pub contributing_participant_count: usize,
    pub stale_rollout_discard_count: usize,
    pub duplicate_rollout_quarantine_count: usize,
    pub duplicate_rollout_deweight_count: usize,
    pub validator_verified_count: usize,
    pub validator_rejected_count: usize,
    pub validator_timed_out_count: usize,
    pub sandbox_ready_profile_count: usize,
    pub sandbox_active_job_count: usize,
    pub runs: Vec<DesktopControlTrainingRunStatus>,
    pub participants: Vec<DesktopControlTrainingParticipantStatus>,
    pub operator: DesktopControlAppleAdapterOperatorStatus,
    pub last_error: Option<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct DesktopControlSettlementHistoryStatus {
    pub settlement_id: String,
    pub settlement_kind: String,
    pub status: String,
    pub product_id: String,
    pub delivery_proof_ids: Vec<String>,
    pub challenge_ids: Vec<String>,
    pub settlement_mode: String,
    pub quantity: u64,
    pub fixed_price_sats: Option<u64>,
    pub reference_index_id: Option<String>,
    pub created_at_epoch_ms: i64,
    pub delivery_window_label: String,
    pub reason_code: Option<String>,
    pub reason_detail: Option<String>,
    pub outcome_summary: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct DesktopControlProofHistoryStatus {
    pub delivery_proof_id: String,
    pub product_id: String,
    pub capacity_lot_id: String,
    pub instrument_id: Option<String>,
    pub contract_id: Option<String>,
    pub created_at_epoch_ms: i64,
    pub proof_status: String,
    pub proof_posture: String,
    pub topology_kind: String,
    pub provisioning_kind: String,
    pub environment_ref: Option<String>,
    pub environment_version: Option<String>,
    pub metered_quantity: u64,
    pub accepted_quantity: u64,
    pub acceptance_summary: String,
    pub settlement_status: Option<String>,
    pub settlement_summary: Option<String>,
    pub challenge_status: Option<String>,
    pub challenge_summary: Option<String>,
    pub proof_bundle_ref: Option<String>,
    pub activation_fingerprint_ref: Option<String>,
    pub validator_pool_ref: Option<String>,
    pub validator_run_ref: Option<String>,
    pub runtime_manifest_ref: Option<String>,
    pub runtime_manifest_digest: Option<String>,
    pub session_claims_ref: Option<String>,
    pub session_identity_posture: Option<String>,
    pub transport_identity_posture: Option<String>,
    pub runtime_config_identity_mode: Option<String>,
    pub mutable_runtime_variables_present: Option<bool>,
    pub command_digest: Option<String>,
    pub environment_digest: Option<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct DesktopControlProofStatus {
    pub available: bool,
    pub source: String,
    pub last_synced_at_epoch_ms: Option<u64>,
    pub pending_count: usize,
    pub accepted_count: usize,
    pub rejected_count: usize,
    pub challenged_count: usize,
    pub settlement_open_count: usize,
    pub settlement_terminal_count: usize,
    pub history: Vec<DesktopControlProofHistoryStatus>,
    pub settlements: Vec<DesktopControlSettlementHistoryStatus>,
    pub last_error: Option<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct DesktopControlChallengeHistoryStatus {
    pub challenge_id: String,
    pub delivery_proof_ids: Vec<String>,
    pub product_id: String,
    pub runtime_backend: String,
    pub model_id: Option<String>,
    pub protocol_id: String,
    pub status: String,
    pub verdict: Option<String>,
    pub reason_code: Option<String>,
    pub attempts_used: u32,
    pub active_attempt: Option<u32>,
    pub validator_id: Option<String>,
    pub validator_pool_ref: Option<String>,
    pub proof_bundle_digest: String,
    pub challenge_result_ref: Option<String>,
    pub created_at_epoch_ms: u64,
    pub finalized_at_epoch_ms: Option<u64>,
    pub verified_row_count: Option<u32>,
    pub settlement_impact_summary: Option<String>,
    pub detail: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct DesktopControlChallengeStatus {
    pub available: bool,
    pub source: String,
    pub last_synced_at_epoch_ms: Option<u64>,
    pub open_count: usize,
    pub queued_count: usize,
    pub leased_count: usize,
    pub retrying_count: usize,
    pub verified_count: usize,
    pub rejected_count: usize,
    pub timed_out_count: usize,
    pub history: Vec<DesktopControlChallengeHistoryStatus>,
    pub last_error: Option<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct DesktopControlBuyModeRequestStatus {
    pub request_id: String,
    pub phase: String,
    pub status: String,
    pub next_expected_event: String,
    pub request_event_id: Option<String>,
    pub selected_provider_pubkey: Option<String>,
    pub result_provider_pubkey: Option<String>,
    pub invoice_provider_pubkey: Option<String>,
    pub payable_provider_pubkey: Option<String>,
    pub last_feedback_status: Option<String>,
    pub last_feedback_event_id: Option<String>,
    pub last_result_event_id: Option<String>,
    pub winning_result_event_id: Option<String>,
    pub payment_pointer: Option<String>,
    pub pending_bolt11: Option<String>,
    pub payment_blocker_codes: Vec<String>,
    pub payment_blocker_summary: Option<String>,
    pub payment_notice: Option<String>,
    pub payment_error: Option<String>,
    pub wallet_status: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct DesktopControlAutopilotPeerStatus {
    pub pubkey: String,
    pub relay_url: String,
    pub ready_model: Option<String>,
    pub online_for_compute: bool,
    pub eligible_for_buy_mode: bool,
    pub eligibility_reason: String,
    pub last_chat_message_at: Option<u64>,
    pub last_presence_at: Option<u64>,
    pub presence_expires_at: Option<u64>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct DesktopControlBuyModeTargetSelectionStatus {
    pub selected_peer_pubkey: Option<String>,
    pub selected_relay_url: Option<String>,
    pub selected_ready_model: Option<String>,
    pub observed_peer_count: usize,
    pub eligible_peer_count: usize,
    pub blocked_reason_code: Option<String>,
    pub blocked_reason: Option<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct DesktopControlBuyModeStatus {
    pub enabled: bool,
    pub approved_budget_sats: u64,
    pub cadence_seconds: u64,
    pub cadence_millis: u64,
    pub next_dispatch_countdown_seconds: Option<u64>,
    pub next_dispatch_countdown_millis: Option<u64>,
    pub in_flight_request_id: Option<String>,
    pub in_flight_phase: Option<String>,
    pub in_flight_status: Option<String>,
    pub selected_provider_pubkey: Option<String>,
    pub result_provider_pubkey: Option<String>,
    pub invoice_provider_pubkey: Option<String>,
    pub payable_provider_pubkey: Option<String>,
    pub payment_blocker_codes: Vec<String>,
    pub payment_blocker_summary: Option<String>,
    pub target_selection: DesktopControlBuyModeTargetSelectionStatus,
    pub peer_roster: Vec<DesktopControlAutopilotPeerStatus>,
    pub recent_requests: Vec<DesktopControlBuyModeRequestStatus>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct DesktopControlActiveJobStatus {
    pub job_id: String,
    pub request_id: String,
    pub capability: String,
    pub stage: String,
    pub projection_stage: String,
    pub phase: String,
    pub next_expected_event: String,
    pub projection_authority: String,
    pub quoted_price_sats: u64,
    pub pending_result_publish_event_id: Option<String>,
    pub result_event_id: Option<String>,
    pub result_publish_status: String,
    pub result_publish_attempt_count: u32,
    pub result_publish_age_seconds: Option<u64>,
    pub payment_pointer: Option<String>,
    pub pending_bolt11: Option<String>,
    pub settlement_status: Option<String>,
    pub settlement_method: Option<String>,
    pub settlement_amount_sats: Option<u64>,
    pub settlement_fees_sats: Option<u64>,
    pub settlement_net_wallet_delta_sats: Option<i64>,
    pub continuity_window_seconds: Option<u64>,
    pub failure_reason: Option<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct DesktopControlNip28GroupStatus {
    pub group_id: String,
    pub name: String,
    pub selected: bool,
    pub unread_count: usize,
    pub mention_count: usize,
    pub channel_count: usize,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct DesktopControlNip28ChannelStatus {
    pub channel_id: String,
    pub group_id: String,
    pub name: String,
    pub relay_url: Option<String>,
    pub selected: bool,
    pub unread_count: usize,
    pub mention_count: usize,
    pub message_count: usize,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct DesktopControlNip28MessageStatus {
    pub event_id: String,
    pub author_pubkey: String,
    pub content: String,
    pub created_at: u64,
    pub reply_to_event_id: Option<String>,
    pub delivery_state: String,
    pub delivery_error: Option<String>,
    pub attempt_count: u32,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct DesktopControlNip28Status {
    pub available: bool,
    pub browse_mode: String,
    pub configured_relay_url: String,
    pub configured_channel_id: String,
    pub configured_channel_loaded: bool,
    pub local_pubkey: Option<String>,
    pub selected_group_id: Option<String>,
    pub selected_group_name: Option<String>,
    pub selected_channel_id: Option<String>,
    pub selected_channel_name: Option<String>,
    pub selected_channel_relay_url: Option<String>,
    pub publishing_outbound_count: usize,
    pub retryable_event_id: Option<String>,
    pub last_action: Option<String>,
    pub last_error: Option<String>,
    pub groups: Vec<DesktopControlNip28GroupStatus>,
    pub channels: Vec<DesktopControlNip28ChannelStatus>,
    pub recent_messages: Vec<DesktopControlNip28MessageStatus>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct DesktopControlLastCommandStatus {
    pub summary: String,
    pub error: Option<String>,
    pub completed_at_epoch_ms: u64,
    pub snapshot_revision: u64,
    pub state_signature: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DesktopControlManifest {
    pub schema_version: u16,
    pub generated_at_epoch_ms: u64,
    pub pid: u32,
    pub listen_addr: String,
    pub base_url: String,
    pub auth_token: String,
    pub latest_session_log_path: String,
}

static DESKTOP_CONTROL_SANDBOX_SERVICE: OnceLock<InMemorySandboxJobService> = OnceLock::new();

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum DesktopControlActionRequest {
    GetSnapshot,
    GetClusterStatus,
    GetClusterTopology,
    GetSandboxStatus,
    CreateSandboxJob {
        profile_id: String,
        job_id: String,
        workspace_root: String,
        entrypoint_type: ProviderSandboxEntrypointType,
        entrypoint: String,
        payload: Option<String>,
        arguments: Vec<String>,
        expected_outputs: Vec<String>,
        timeout_request_s: u64,
        network_request: String,
        filesystem_request: String,
        payout_reference: Option<String>,
        verification_posture: Option<String>,
    },
    GetSandboxJob {
        job_id: String,
    },
    UploadSandboxFile {
        job_id: String,
        relative_path: String,
        content_base64: String,
    },
    StartSandboxJob {
        job_id: String,
    },
    WaitSandboxJob {
        job_id: String,
        timeout_ms: u64,
    },
    DownloadSandboxArtifact {
        job_id: String,
        relative_path: String,
    },
    DownloadSandboxWorkspaceFile {
        job_id: String,
        relative_path: String,
    },
    GetResearchStatus,
    ResetResearchState,
    GetTrainingStatus,
    LaunchAppleAdapterTraining {
        train_dataset_path: String,
        held_out_dataset_path: String,
        package_name: String,
        author: String,
        description: String,
        license: String,
        apple_fm_base_url: String,
    },
    ExportAppleAdapterTraining {
        run_id: String,
        export_path: String,
    },
    AcceptAppleAdapterTraining {
        run_id: String,
    },
    GetProofStatus,
    GetChallengeStatus,
    ListPanes,
    OpenPane {
        pane: String,
    },
    FocusPane {
        pane: String,
    },
    ClosePane {
        pane: String,
    },
    GetPaneSnapshot {
        pane: String,
    },
    SetProviderMode {
        online: bool,
    },
    RefreshLocalRuntime,
    RefreshAppleFm,
    LoadAppleFmAdapter {
        package_path: String,
        requested_adapter_id: Option<String>,
    },
    UnloadAppleFmAdapter {
        adapter_id: String,
    },
    AttachAppleFmSessionAdapter {
        session_id: String,
        adapter_id: String,
    },
    DetachAppleFmSessionAdapter {
        session_id: String,
    },
    RunAppleFmSmokeTest,
    RefreshGptOss,
    WarmGptOss,
    UnloadGptOss,
    RefreshWallet,
    GetNip90SentPaymentsReport {
        start_epoch_seconds: u64,
        end_epoch_seconds: u64,
        report_date: Option<String>,
    },
    StartBuyMode,
    StopBuyMode,
    GetActiveJob,
    SelectNip28MainChannel,
    SelectNip28Group {
        group_id: String,
    },
    SelectNip28Channel {
        channel_id: String,
    },
    SendNip28Message {
        content: String,
        reply_to_event_id: Option<String>,
    },
    RetryNip28Message {
        event_id: String,
    },
    Withdraw {
        bolt11: String,
    },
    GetMissionControlLogTail {
        limit: usize,
    },
}

impl DesktopControlActionRequest {
    fn label(&self) -> &'static str {
        match self {
            Self::GetSnapshot => "get-snapshot",
            Self::GetClusterStatus => "cluster-status",
            Self::GetClusterTopology => "cluster-topology",
            Self::GetSandboxStatus => "sandbox-status",
            Self::CreateSandboxJob { .. } => "sandbox-create",
            Self::GetSandboxJob { .. } => "sandbox-get",
            Self::UploadSandboxFile { .. } => "sandbox-upload",
            Self::StartSandboxJob { .. } => "sandbox-start",
            Self::WaitSandboxJob { .. } => "sandbox-wait",
            Self::DownloadSandboxArtifact { .. } => "sandbox-download-artifact",
            Self::DownloadSandboxWorkspaceFile { .. } => "sandbox-download-workspace",
            Self::GetResearchStatus => "research-status",
            Self::ResetResearchState => "research-reset",
            Self::GetTrainingStatus => "training-status",
            Self::LaunchAppleAdapterTraining { .. } => "training-launch-apple-adapter",
            Self::ExportAppleAdapterTraining { .. } => "training-export-apple-adapter",
            Self::AcceptAppleAdapterTraining { .. } => "training-accept-apple-adapter",
            Self::GetProofStatus => "proof-status",
            Self::GetChallengeStatus => "challenge-status",
            Self::ListPanes => "pane-list",
            Self::OpenPane { .. } => "pane-open",
            Self::FocusPane { .. } => "pane-focus",
            Self::ClosePane { .. } => "pane-close",
            Self::GetPaneSnapshot { .. } => "pane-snapshot",
            Self::SetProviderMode { online: true } => "provider-online",
            Self::SetProviderMode { online: false } => "provider-offline",
            Self::RefreshLocalRuntime => "local-runtime-refresh",
            Self::RefreshAppleFm => "apple-fm-refresh",
            Self::LoadAppleFmAdapter { .. } => "apple-fm-load-adapter",
            Self::UnloadAppleFmAdapter { .. } => "apple-fm-unload-adapter",
            Self::AttachAppleFmSessionAdapter { .. } => "apple-fm-attach-adapter",
            Self::DetachAppleFmSessionAdapter { .. } => "apple-fm-detach-adapter",
            Self::RunAppleFmSmokeTest => "apple-fm-smoke-test",
            Self::RefreshGptOss => "gpt-oss-refresh",
            Self::WarmGptOss => "gpt-oss-warm",
            Self::UnloadGptOss => "gpt-oss-unload",
            Self::RefreshWallet => "wallet-refresh",
            Self::GetNip90SentPaymentsReport { .. } => "nip90-sent-payments-report",
            Self::StartBuyMode => "buy-mode-start",
            Self::StopBuyMode => "buy-mode-stop",
            Self::GetActiveJob => "active-job",
            Self::SelectNip28MainChannel => "nip28-main",
            Self::SelectNip28Group { .. } => "nip28-select-group",
            Self::SelectNip28Channel { .. } => "nip28-select-channel",
            Self::SendNip28Message { .. } => "nip28-send",
            Self::RetryNip28Message { .. } => "nip28-retry",
            Self::Withdraw { .. } => "withdraw",
            Self::GetMissionControlLogTail { .. } => "log-tail",
        }
    }

    fn provider_mode_online_target(&self) -> Option<bool> {
        match self {
            Self::SetProviderMode { online } => Some(*online),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct DesktopControlActionResponse {
    pub success: bool,
    pub message: String,
    pub payload: Option<Value>,
    pub snapshot_revision: Option<u64>,
    pub state_signature: Option<String>,
}

impl DesktopControlActionResponse {
    fn ok(message: impl Into<String>) -> Self {
        Self {
            success: true,
            message: message.into(),
            payload: None,
            snapshot_revision: None,
            state_signature: None,
        }
    }

    fn ok_with_payload(message: impl Into<String>, payload: Value) -> Self {
        Self {
            success: true,
            message: message.into(),
            payload: Some(payload),
            snapshot_revision: None,
            state_signature: None,
        }
    }

    fn error(message: impl Into<String>) -> Self {
        Self {
            success: false,
            message: message.into(),
            payload: None,
            snapshot_revision: None,
            state_signature: None,
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct DesktopControlEventDraft {
    pub event_type: String,
    pub summary: String,
    pub command_label: Option<String>,
    pub success: Option<bool>,
    pub payload: Option<Value>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct DesktopControlEvent {
    pub event_id: u64,
    pub event_type: String,
    pub at_epoch_ms: u64,
    pub summary: String,
    pub command_label: Option<String>,
    pub success: Option<bool>,
    pub snapshot_revision: Option<u64>,
    pub state_signature: Option<String>,
    pub payload: Option<Value>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct DesktopControlEventBatch {
    pub last_event_id: u64,
    pub timed_out: bool,
    pub events: Vec<DesktopControlEvent>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Deserialize)]
struct DesktopControlEventsQuery {
    #[serde(default)]
    after_event_id: u64,
    #[serde(default)]
    limit: Option<usize>,
    #[serde(default)]
    timeout_ms: Option<u64>,
}

#[derive(Default)]
struct DesktopControlEventBuffer {
    next_event_id: u64,
    events: VecDeque<DesktopControlEvent>,
}

#[derive(Debug)]
pub struct DesktopControlActionEnvelope {
    pub action: DesktopControlActionRequest,
    response_tx: oneshot::Sender<DesktopControlActionResponse>,
}

impl DesktopControlActionEnvelope {
    pub fn respond(self, response: DesktopControlActionResponse) {
        let _ = self.response_tx.send(response);
    }
}

#[derive(Debug)]
pub enum DesktopControlRuntimeUpdate {
    ActionRequest(DesktopControlActionEnvelope),
    WorkerError(String),
}

enum DesktopControlRuntimeCommand {
    SyncSnapshot(Box<DesktopControlSnapshot>),
    AppendEvents(Vec<DesktopControlEventDraft>),
    Shutdown,
}

#[derive(Clone)]
struct DesktopControlHttpState {
    snapshot: Arc<Mutex<DesktopControlSnapshot>>,
    events: Arc<Mutex<DesktopControlEventBuffer>>,
    event_notify: Arc<Notify>,
    auth_token: Arc<Mutex<String>>,
    update_tx: Sender<DesktopControlRuntimeUpdate>,
}

pub struct DesktopControlRuntime {
    command_tx: tokio_mpsc::UnboundedSender<DesktopControlRuntimeCommand>,
    update_rx: Receiver<DesktopControlRuntimeUpdate>,
    listen_addr: SocketAddr,
    last_event_snapshot: Option<DesktopControlSnapshot>,
    join_handle: Option<JoinHandle<()>>,
}

impl DesktopControlRuntime {
    pub fn spawn(config: DesktopControlRuntimeConfig) -> Result<Self, String> {
        let (command_tx, command_rx) = tokio_mpsc::unbounded_channel();
        let (update_tx, update_rx) = mpsc::channel::<DesktopControlRuntimeUpdate>();
        let (ready_tx, ready_rx) = mpsc::channel::<Result<SocketAddr, String>>();
        let join_handle = std::thread::spawn(move || {
            run_desktop_control_runtime_loop(command_rx, update_tx, ready_tx, config);
        });
        let listen_addr = ready_rx.recv().map_err(|error| {
            format!("Desktop control runtime failed to report readiness: {error}")
        })??;
        Ok(Self {
            command_tx,
            update_rx,
            listen_addr,
            last_event_snapshot: None,
            join_handle: Some(join_handle),
        })
    }

    pub fn listen_addr(&self) -> SocketAddr {
        self.listen_addr
    }

    pub fn sync_snapshot(&self, snapshot: DesktopControlSnapshot) -> Result<(), String> {
        self.command_tx
            .send(DesktopControlRuntimeCommand::SyncSnapshot(Box::new(
                snapshot,
            )))
            .map_err(|error| format!("Desktop control runtime offline: {error}"))
    }

    pub fn append_events(&self, events: Vec<DesktopControlEventDraft>) -> Result<(), String> {
        if events.is_empty() {
            return Ok(());
        }
        self.command_tx
            .send(DesktopControlRuntimeCommand::AppendEvents(events))
            .map_err(|error| format!("Desktop control runtime offline: {error}"))
    }

    pub fn drain_updates(&mut self) -> Vec<DesktopControlRuntimeUpdate> {
        let mut updates = Vec::new();
        while let Ok(update) = self.update_rx.try_recv() {
            updates.push(update);
        }
        updates
    }

    pub fn shutdown_async(&mut self) {
        let _ = self.command_tx.send(DesktopControlRuntimeCommand::Shutdown);
        if let Some(handle) = self.join_handle.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for DesktopControlRuntime {
    fn drop(&mut self) {
        self.shutdown_async();
    }
}

impl DesktopControlEventBuffer {
    fn append(&mut self, drafts: Vec<DesktopControlEventDraft>) -> Vec<DesktopControlEvent> {
        let mut appended = Vec::new();
        for draft in drafts {
            let event_type = draft.event_type.trim();
            let summary = draft.summary.trim();
            if event_type.is_empty() || summary.is_empty() {
                continue;
            }
            self.next_event_id = self.next_event_id.saturating_add(1);
            let event = DesktopControlEvent {
                event_id: self.next_event_id,
                event_type: event_type.to_string(),
                at_epoch_ms: current_epoch_ms(),
                summary: summary.to_string(),
                command_label: draft.command_label,
                success: draft.success,
                snapshot_revision: draft
                    .payload
                    .as_ref()
                    .and_then(|payload| payload.get("snapshot_revision"))
                    .and_then(Value::as_u64),
                state_signature: draft
                    .payload
                    .as_ref()
                    .and_then(|payload| payload.get("state_signature"))
                    .and_then(Value::as_str)
                    .map(str::to_string),
                payload: draft.payload,
            };
            self.events.push_back(event.clone());
            appended.push(event);
        }
        while self.events.len() > DESKTOP_CONTROL_EVENT_BUFFER_LIMIT {
            self.events.pop_front();
        }
        appended
    }

    fn collect_after(&self, after_event_id: u64, limit: usize) -> Vec<DesktopControlEvent> {
        self.events
            .iter()
            .filter(|event| event.event_id > after_event_id)
            .take(limit.max(1).min(DESKTOP_CONTROL_EVENT_QUERY_LIMIT))
            .cloned()
            .collect()
    }

    fn last_event_id(&self) -> u64 {
        self.events.back().map_or(0, |event| event.event_id)
    }
}

pub fn validate_control_bind_addr(raw: &str) -> Result<SocketAddr, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("Desktop control bind address cannot be empty".to_string());
    }
    let listen_addr = trimmed
        .parse::<SocketAddr>()
        .map_err(|error| format!("Invalid desktop control bind address `{trimmed}`: {error}"))?;
    if !matches!(listen_addr.ip(), IpAddr::V4(v4) if v4.is_loopback()) {
        return Err(format!(
            "Desktop control bind address `{trimmed}` must stay on loopback"
        ));
    }
    Ok(listen_addr)
}

pub fn generate_control_auth_token() -> Result<String, String> {
    let mut bytes = [0_u8; 24];
    getrandom::fill(&mut bytes)
        .map_err(|error| format!("Failed to generate desktop control auth token: {error}"))?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}

pub fn control_base_url(listen_addr: SocketAddr) -> String {
    format!("http://{listen_addr}")
}

pub fn control_manifest_path() -> PathBuf {
    crate::runtime_log::autopilot_log_dir().join(DESKTOP_CONTROL_MANIFEST_FILENAME)
}

pub fn load_control_manifest() -> Result<DesktopControlManifest, String> {
    let path = control_manifest_path();
    let raw = fs::read_to_string(path.as_path())
        .map_err(|error| format!("Failed to read desktop control manifest: {error}"))?;
    serde_json::from_str(raw.as_str())
        .map_err(|error| format!("Failed to decode desktop control manifest: {error}"))
}

pub fn enable_runtime(
    state: &mut RenderState,
    requested_bind_addr: Option<&str>,
) -> Result<String, String> {
    let bind_addr = validate_control_bind_addr(
        requested_bind_addr.unwrap_or(state.desktop_control.requested_bind_addr.as_str()),
    )?;
    let auth_token = generate_control_auth_token()?;
    let manifest_path = control_manifest_path();
    disable_runtime(state);

    let mut runtime = DesktopControlRuntime::spawn(DesktopControlRuntimeConfig {
        listen_addr: bind_addr,
        auth_token: auth_token.clone(),
    })?;
    let listen_addr = runtime.listen_addr();
    let base_url = control_base_url(listen_addr);
    let snapshot = snapshot_for_state(state);
    runtime.sync_snapshot(snapshot.clone())?;
    runtime.append_events(snapshot_change_events(None, &snapshot))?;
    runtime.last_event_snapshot = Some(snapshot.clone());

    let manifest = DesktopControlManifest {
        schema_version: DESKTOP_CONTROL_MANIFEST_SCHEMA_VERSION,
        generated_at_epoch_ms: current_epoch_ms(),
        pid: std::process::id(),
        listen_addr: listen_addr.to_string(),
        base_url: base_url.clone(),
        auth_token: auth_token.clone(),
        latest_session_log_path: crate::runtime_log::latest_session_log_path()
            .display()
            .to_string(),
    };
    write_control_manifest(manifest_path.as_path(), &manifest)?;

    state.desktop_control.enabled = true;
    state.desktop_control.requested_bind_addr = bind_addr.to_string();
    state.desktop_control.listen_addr = Some(listen_addr.to_string());
    state.desktop_control.base_url = Some(base_url.clone());
    state.desktop_control.manifest_path = Some(manifest_path.display().to_string());
    state.desktop_control.auth_token_preview = Some(auth_token_preview(auth_token.as_str()));
    state.desktop_control.last_error = None;
    state.desktop_control.last_action = Some(format!("Desktop control listening on {listen_addr}"));
    state.desktop_control.last_snapshot_revision = snapshot.snapshot_revision;
    state.desktop_control.last_snapshot_signature = Some(snapshot.state_signature.clone());
    state.desktop_control_runtime = Some(runtime);
    state.desktop_control_last_sync_signature = Some(snapshot.state_signature.clone());
    state.desktop_control_last_sync_at = Some(Instant::now());

    Ok(format!(
        "Desktop control enabled on {listen_addr}. URL: {base_url} token={}",
        auth_token_preview(auth_token.as_str())
    ))
}

pub fn disable_runtime(state: &mut RenderState) -> String {
    if let Some(mut runtime) = state.desktop_control_runtime.take() {
        runtime.shutdown_async();
    }
    let _ = fs::remove_file(control_manifest_path());
    state.desktop_control.enabled = false;
    state.desktop_control.listen_addr = None;
    state.desktop_control.base_url = None;
    state.desktop_control.manifest_path = None;
    state.desktop_control.auth_token_preview = None;
    state.desktop_control.last_error = None;
    state.desktop_control.last_action = Some("Desktop control runtime disabled".to_string());
    state.desktop_control.last_snapshot_revision = 0;
    state.desktop_control.last_snapshot_signature = None;
    state.desktop_control_last_sync_signature = None;
    state.desktop_control_last_sync_at = None;
    "Desktop control runtime disabled".to_string()
}

pub fn pump_runtime(state: &mut RenderState) -> bool {
    let mut changed = false;
    if drain_runtime_updates(state) {
        changed = true;
    }
    if poll_runtime(state) {
        changed = true;
    }
    changed
}

pub fn drain_runtime_updates(state: &mut RenderState) -> bool {
    let updates = match state.desktop_control_runtime.as_mut() {
        Some(runtime) => runtime.drain_updates(),
        None => return false,
    };
    let mut changed = false;
    for update in updates {
        match update {
            DesktopControlRuntimeUpdate::ActionRequest(envelope) => {
                emit_control_events(state, vec![command_received_event(&envelope.action)], false);
                let response = apply_action_request(state, &envelope.action);
                emit_control_events(
                    state,
                    vec![command_outcome_event(&envelope.action, &response)],
                    true,
                );
                envelope.respond(response);
                changed = true;
            }
            DesktopControlRuntimeUpdate::WorkerError(error) => {
                state.desktop_control.last_error = Some(error);
                changed = true;
            }
        }
    }
    changed
}

pub fn poll_runtime(state: &mut RenderState) -> bool {
    let mut changed = false;
    if refresh_compute_history_cache_if_due(state, false) {
        changed = true;
    }
    if sync_runtime_snapshot(state) {
        changed = true;
    }
    changed
}

fn sync_runtime_snapshot(state: &mut RenderState) -> bool {
    let total_started_at = Instant::now();
    let should_attempt_sync = state
        .desktop_control_last_sync_at
        .is_none_or(|last| last.elapsed() >= DESKTOP_CONTROL_SYNC_INTERVAL);
    if !should_attempt_sync {
        return false;
    }
    let signature_started_at = Instant::now();
    let signature = crate::snapshot_domains::desktop_control_signature(state);
    let signature_elapsed_ms = signature_started_at.elapsed().as_secs_f32() * 1_000.0;
    let signature_changed =
        state.desktop_control_last_sync_signature.as_deref() != Some(signature.as_str());
    let should_sync = signature_changed || state.desktop_control_last_sync_at.is_none();
    state
        .frame_debugger
        .record_snapshot_timing_sample(SnapshotTimingSample {
            subsystem: "desktop_control".to_string(),
            phase: "signature".to_string(),
            synced: should_sync,
            success: true,
            elapsed_ms: signature_elapsed_ms,
        });
    if !should_sync {
        state
            .frame_debugger
            .record_snapshot_timing_sample(SnapshotTimingSample {
                subsystem: "desktop_control".to_string(),
                phase: "total".to_string(),
                synced: false,
                success: true,
                elapsed_ms: total_started_at.elapsed().as_secs_f32() * 1_000.0,
            });
        return false;
    }
    let build_started_at = Instant::now();
    let snapshot = snapshot_for_state_with_signature(state, signature.clone());
    state
        .frame_debugger
        .record_snapshot_timing_sample(SnapshotTimingSample {
            subsystem: "desktop_control".to_string(),
            phase: "build".to_string(),
            synced: true,
            success: true,
            elapsed_ms: build_started_at.elapsed().as_secs_f32() * 1_000.0,
        });
    let snapshot_revision = snapshot.snapshot_revision;
    let Some(runtime) = state.desktop_control_runtime.as_mut() else {
        state
            .frame_debugger
            .record_snapshot_timing_sample(SnapshotTimingSample {
                subsystem: "desktop_control".to_string(),
                phase: "total".to_string(),
                synced: false,
                success: false,
                elapsed_ms: total_started_at.elapsed().as_secs_f32() * 1_000.0,
            });
        return false;
    };
    let sync_started_at = Instant::now();
    if let Err(error) = runtime.sync_snapshot(snapshot.clone()) {
        state
            .frame_debugger
            .record_snapshot_timing_sample(SnapshotTimingSample {
                subsystem: "desktop_control".to_string(),
                phase: "sync_snapshot".to_string(),
                synced: true,
                success: false,
                elapsed_ms: sync_started_at.elapsed().as_secs_f32() * 1_000.0,
            });
        state
            .frame_debugger
            .record_snapshot_timing_sample(SnapshotTimingSample {
                subsystem: "desktop_control".to_string(),
                phase: "total".to_string(),
                synced: true,
                success: false,
                elapsed_ms: total_started_at.elapsed().as_secs_f32() * 1_000.0,
            });
        state.desktop_control.last_error = Some(error);
        return false;
    }
    state
        .frame_debugger
        .record_snapshot_timing_sample(SnapshotTimingSample {
            subsystem: "desktop_control".to_string(),
            phase: "sync_snapshot".to_string(),
            synced: true,
            success: true,
            elapsed_ms: sync_started_at.elapsed().as_secs_f32() * 1_000.0,
        });
    if signature_changed {
        let previous_snapshot = runtime.last_event_snapshot.clone();
        let append_started_at = Instant::now();
        if let Err(error) = runtime.append_events(snapshot_change_events(
            previous_snapshot.as_ref(),
            &snapshot,
        )) {
            state
                .frame_debugger
                .record_snapshot_timing_sample(SnapshotTimingSample {
                    subsystem: "desktop_control".to_string(),
                    phase: "append_events".to_string(),
                    synced: true,
                    success: false,
                    elapsed_ms: append_started_at.elapsed().as_secs_f32() * 1_000.0,
                });
            state.desktop_control.last_error = Some(error);
        } else {
            state
                .frame_debugger
                .record_snapshot_timing_sample(SnapshotTimingSample {
                    subsystem: "desktop_control".to_string(),
                    phase: "append_events".to_string(),
                    synced: true,
                    success: true,
                    elapsed_ms: append_started_at.elapsed().as_secs_f32() * 1_000.0,
                });
        }
        runtime.last_event_snapshot = Some(snapshot.clone());
    }
    state.desktop_control.last_snapshot_revision = snapshot_revision;
    state.desktop_control.last_snapshot_signature = Some(signature.clone());
    state.desktop_control_last_sync_signature = Some(signature);
    state.desktop_control_last_sync_at = Some(Instant::now());
    state
        .frame_debugger
        .record_snapshot_timing_sample(SnapshotTimingSample {
            subsystem: "desktop_control".to_string(),
            phase: "total".to_string(),
            synced: true,
            success: true,
            elapsed_ms: total_started_at.elapsed().as_secs_f32() * 1_000.0,
        });
    true
}

#[cfg(test)]
fn snapshot_sync_signature(snapshot: &DesktopControlSnapshot) -> String {
    let mut stable_snapshot = snapshot.clone();
    stable_snapshot.generated_at_epoch_ms = 0;
    stable_snapshot.snapshot_revision = 0;
    stable_snapshot.state_signature.clear();
    if let Some(last_command) = stable_snapshot.last_command.as_mut() {
        last_command.completed_at_epoch_ms = 0;
        last_command.snapshot_revision = 0;
        last_command.state_signature.clear();
    }
    serde_json::to_string(&stable_snapshot)
        .map(|json| sha256_prefixed_text(json.as_str()))
        .unwrap_or_else(|_| "desktop-control-signature-unavailable".to_string())
}

fn emit_control_events(
    state: &mut RenderState,
    events: Vec<DesktopControlEventDraft>,
    mirror_to_mission_control: bool,
) {
    if events.is_empty() {
        return;
    }
    if mirror_to_mission_control {
        for event in &events {
            mirror_control_event_to_mission_control(state, event);
        }
    }
    let Some(runtime) = state.desktop_control_runtime.as_ref() else {
        return;
    };
    if let Err(error) = runtime.append_events(events) {
        state.desktop_control.last_error = Some(error);
    }
}

fn mirror_control_event_to_mission_control(
    state: &mut RenderState,
    event: &DesktopControlEventDraft,
) {
    if matches!(
        event.command_label.as_deref(),
        Some("get-snapshot" | "active-job" | "log-tail")
    ) {
        return;
    }
    let stream = if matches!(event.success, Some(false)) {
        wgpui::components::sections::TerminalStream::Stderr
    } else {
        wgpui::components::sections::TerminalStream::Stdout
    };
    state
        .log_stream
        .push_runtime_log_line(stream, format!("Control: {}", event.summary));
}

fn command_received_event(action: &DesktopControlActionRequest) -> DesktopControlEventDraft {
    DesktopControlEventDraft {
        event_type: "control.command.received".to_string(),
        summary: format!("{} received", action.label()),
        command_label: Some(action.label().to_string()),
        success: None,
        payload: Some(command_payload(action)),
    }
}

fn command_outcome_event(
    action: &DesktopControlActionRequest,
    response: &DesktopControlActionResponse,
) -> DesktopControlEventDraft {
    let (event_type, outcome_label) = if response.success {
        ("control.command.applied", "applied")
    } else {
        ("control.command.rejected", "rejected")
    };
    let include_response_payload = !matches!(
        action,
        DesktopControlActionRequest::GetSnapshot
            | DesktopControlActionRequest::ListPanes
            | DesktopControlActionRequest::GetPaneSnapshot { .. }
            | DesktopControlActionRequest::GetActiveJob
            | DesktopControlActionRequest::GetMissionControlLogTail { .. }
    );
    let mut payload = serde_json::Map::new();
    payload.insert(
        "command_label".to_string(),
        Value::String(action.label().to_string()),
    );
    payload.insert(
        "outcome".to_string(),
        Value::String(outcome_label.to_string()),
    );
    payload.insert(
        "message".to_string(),
        Value::String(response.message.clone()),
    );
    if let Some(snapshot_revision) = response.snapshot_revision {
        payload.insert(
            "snapshot_revision".to_string(),
            Value::from(snapshot_revision),
        );
    }
    if let Some(state_signature) = response.state_signature.clone() {
        payload.insert(
            "state_signature".to_string(),
            Value::String(state_signature),
        );
    }
    if include_response_payload {
        if let Some(response_payload) = response.payload.clone() {
            payload.insert("response_payload".to_string(), response_payload);
        }
    }
    DesktopControlEventDraft {
        event_type: event_type.to_string(),
        summary: format!(
            "{} {} // {}",
            action.label(),
            outcome_label,
            response.message
        ),
        command_label: Some(action.label().to_string()),
        success: Some(response.success),
        payload: Some(Value::Object(payload)),
    }
}

fn command_payload(action: &DesktopControlActionRequest) -> Value {
    match action {
        DesktopControlActionRequest::GetSnapshot => json!({ "command_label": action.label() }),
        DesktopControlActionRequest::GetClusterStatus
        | DesktopControlActionRequest::GetClusterTopology
        | DesktopControlActionRequest::GetSandboxStatus
        | DesktopControlActionRequest::GetResearchStatus
        | DesktopControlActionRequest::ResetResearchState
        | DesktopControlActionRequest::GetTrainingStatus
        | DesktopControlActionRequest::GetProofStatus
        | DesktopControlActionRequest::GetChallengeStatus => {
            json!({ "command_label": action.label() })
        }
        DesktopControlActionRequest::LaunchAppleAdapterTraining {
            train_dataset_path,
            held_out_dataset_path,
            package_name,
            author,
            description,
            license,
            apple_fm_base_url,
        } => json!({
            "command_label": action.label(),
            "train_dataset_path": train_dataset_path,
            "held_out_dataset_path": held_out_dataset_path,
            "package_name": package_name,
            "author": author,
            "description_length": description.trim().len(),
            "license": license,
            "apple_fm_base_url": apple_fm_base_url,
        }),
        DesktopControlActionRequest::ExportAppleAdapterTraining {
            run_id,
            export_path,
        } => json!({
            "command_label": action.label(),
            "run_id": run_id,
            "export_path": export_path,
        }),
        DesktopControlActionRequest::AcceptAppleAdapterTraining { run_id } => json!({
            "command_label": action.label(),
            "run_id": run_id,
        }),
        DesktopControlActionRequest::CreateSandboxJob {
            profile_id,
            job_id,
            workspace_root,
            entrypoint_type,
            entrypoint,
            payload,
            arguments,
            expected_outputs,
            timeout_request_s,
            network_request,
            filesystem_request,
            payout_reference,
            verification_posture,
        } => json!({
            "command_label": action.label(),
            "profile_id": profile_id,
            "job_id": job_id,
            "workspace_root": workspace_root,
            "entrypoint_type": entrypoint_type,
            "entrypoint": entrypoint,
            "payload_length": payload.as_ref().map(|value| value.len()),
            "argument_count": arguments.len(),
            "expected_output_count": expected_outputs.len(),
            "timeout_request_s": timeout_request_s,
            "network_request": network_request,
            "filesystem_request": filesystem_request,
            "payout_reference": payout_reference,
            "verification_posture": verification_posture,
        }),
        DesktopControlActionRequest::GetSandboxJob { job_id }
        | DesktopControlActionRequest::StartSandboxJob { job_id } => json!({
            "command_label": action.label(),
            "job_id": job_id,
        }),
        DesktopControlActionRequest::WaitSandboxJob { job_id, timeout_ms } => json!({
            "command_label": action.label(),
            "job_id": job_id,
            "timeout_ms": timeout_ms,
        }),
        DesktopControlActionRequest::UploadSandboxFile {
            job_id,
            relative_path,
            content_base64,
        } => json!({
            "command_label": action.label(),
            "job_id": job_id,
            "relative_path": relative_path,
            "content_length": content_base64.len(),
        }),
        DesktopControlActionRequest::DownloadSandboxArtifact {
            job_id,
            relative_path,
        }
        | DesktopControlActionRequest::DownloadSandboxWorkspaceFile {
            job_id,
            relative_path,
        } => json!({
            "command_label": action.label(),
            "job_id": job_id,
            "relative_path": relative_path,
        }),
        DesktopControlActionRequest::ListPanes => json!({ "command_label": action.label() }),
        DesktopControlActionRequest::OpenPane { pane }
        | DesktopControlActionRequest::FocusPane { pane }
        | DesktopControlActionRequest::ClosePane { pane }
        | DesktopControlActionRequest::GetPaneSnapshot { pane } => json!({
            "command_label": action.label(),
            "pane": pane,
        }),
        DesktopControlActionRequest::SetProviderMode { online } => {
            json!({ "command_label": action.label(), "online": online })
        }
        DesktopControlActionRequest::RefreshLocalRuntime
        | DesktopControlActionRequest::RefreshAppleFm
        | DesktopControlActionRequest::RefreshGptOss
        | DesktopControlActionRequest::WarmGptOss
        | DesktopControlActionRequest::UnloadGptOss
        | DesktopControlActionRequest::RefreshWallet
        | DesktopControlActionRequest::StartBuyMode
        | DesktopControlActionRequest::StopBuyMode
        | DesktopControlActionRequest::GetActiveJob
        | DesktopControlActionRequest::SelectNip28MainChannel => {
            json!({ "command_label": action.label() })
        }
        DesktopControlActionRequest::LoadAppleFmAdapter {
            package_path,
            requested_adapter_id,
        } => json!({
            "command_label": action.label(),
            "package_path": package_path,
            "requested_adapter_id": requested_adapter_id,
        }),
        DesktopControlActionRequest::UnloadAppleFmAdapter { adapter_id } => json!({
            "command_label": action.label(),
            "adapter_id": adapter_id,
        }),
        DesktopControlActionRequest::AttachAppleFmSessionAdapter {
            session_id,
            adapter_id,
        } => json!({
            "command_label": action.label(),
            "session_id": session_id,
            "adapter_id": adapter_id,
        }),
        DesktopControlActionRequest::DetachAppleFmSessionAdapter { session_id } => json!({
            "command_label": action.label(),
            "session_id": session_id,
        }),
        DesktopControlActionRequest::RunAppleFmSmokeTest => {
            json!({ "command_label": action.label() })
        }
        DesktopControlActionRequest::GetNip90SentPaymentsReport {
            start_epoch_seconds,
            end_epoch_seconds,
            report_date,
        } => json!({
            "command_label": action.label(),
            "start_epoch_seconds": start_epoch_seconds,
            "end_epoch_seconds": end_epoch_seconds,
            "report_date": report_date,
        }),
        DesktopControlActionRequest::SelectNip28Group { group_id } => json!({
            "command_label": action.label(),
            "group_id": group_id,
        }),
        DesktopControlActionRequest::SelectNip28Channel { channel_id } => json!({
            "command_label": action.label(),
            "channel_id": channel_id,
        }),
        DesktopControlActionRequest::SendNip28Message {
            content,
            reply_to_event_id,
        } => json!({
            "command_label": action.label(),
            "content_length": content.trim().len(),
            "reply_to_event_id": reply_to_event_id,
        }),
        DesktopControlActionRequest::RetryNip28Message { event_id } => json!({
            "command_label": action.label(),
            "event_id": event_id,
        }),
        DesktopControlActionRequest::Withdraw { bolt11 } => json!({
            "command_label": action.label(),
            "invoice_length": bolt11.trim().len(),
        }),
        DesktopControlActionRequest::GetMissionControlLogTail { limit } => json!({
            "command_label": action.label(),
            "limit": limit,
        }),
    }
}

fn snapshot_change_events(
    previous: Option<&DesktopControlSnapshot>,
    current: &DesktopControlSnapshot,
) -> Vec<DesktopControlEventDraft> {
    let mut events = Vec::new();
    let mut changed_domains = Vec::new();

    if previous.is_none_or(|snapshot| snapshot.provider != current.provider) {
        changed_domains.push("provider");
        events.push(DesktopControlEventDraft {
            event_type: "provider.mode.changed".to_string(),
            summary: provider_status_summary(&current.provider),
            command_label: None,
            success: None,
            payload: serde_json::to_value(&current.provider).ok(),
        });
    }
    if previous.is_none_or(|snapshot| snapshot.local_runtime != current.local_runtime) {
        changed_domains.push("local_runtime");
        events.push(DesktopControlEventDraft {
            event_type: "local_runtime.state.changed".to_string(),
            summary: local_runtime_status_summary(&current.local_runtime),
            command_label: None,
            success: None,
            payload: serde_json::to_value(&current.local_runtime).ok(),
        });
    }
    if previous.is_none_or(|snapshot| snapshot.gpt_oss != current.gpt_oss) {
        changed_domains.push("gpt_oss");
        events.push(DesktopControlEventDraft {
            event_type: "gpt_oss.state.changed".to_string(),
            summary: gpt_oss_status_summary(&current.gpt_oss),
            command_label: None,
            success: None,
            payload: serde_json::to_value(&current.gpt_oss).ok(),
        });
    }
    if previous.is_none_or(|snapshot| snapshot.apple_fm != current.apple_fm) {
        changed_domains.push("apple_fm");
        events.push(DesktopControlEventDraft {
            event_type: "apple_fm.readiness.changed".to_string(),
            summary: apple_fm_status_summary(&current.apple_fm),
            command_label: None,
            success: None,
            payload: serde_json::to_value(&current.apple_fm).ok(),
        });
    }
    if previous.is_none_or(|snapshot| snapshot.wallet != current.wallet) {
        changed_domains.push("wallet");
        events.push(DesktopControlEventDraft {
            event_type: "wallet.state.changed".to_string(),
            summary: wallet_status_summary(&current.wallet),
            command_label: None,
            success: None,
            payload: serde_json::to_value(&current.wallet).ok(),
        });
    }
    if previous.is_none_or(|snapshot| snapshot.inventory != current.inventory) {
        changed_domains.push("inventory");
        events.push(DesktopControlEventDraft {
            event_type: "inventory.state.changed".to_string(),
            summary: inventory_status_summary(&current.inventory),
            command_label: None,
            success: None,
            payload: serde_json::to_value(&current.inventory).ok(),
        });
    }
    if previous.is_none_or(|snapshot| snapshot.buyer_procurement != current.buyer_procurement) {
        changed_domains.push("buyer_procurement");
        events.push(DesktopControlEventDraft {
            event_type: "buyer_procurement.state.changed".to_string(),
            summary: buyer_procurement_status_summary(&current.buyer_procurement),
            command_label: None,
            success: None,
            payload: serde_json::to_value(&current.buyer_procurement).ok(),
        });
    }
    if previous.is_none_or(|snapshot| snapshot.cluster != current.cluster) {
        changed_domains.push("cluster");
        events.push(DesktopControlEventDraft {
            event_type: "cluster.state.changed".to_string(),
            summary: cluster_status_summary(&current.cluster),
            command_label: None,
            success: None,
            payload: serde_json::to_value(&current.cluster).ok(),
        });
    }
    if previous.is_none_or(|snapshot| snapshot.sandbox != current.sandbox) {
        changed_domains.push("sandbox");
        events.push(DesktopControlEventDraft {
            event_type: "sandbox.state.changed".to_string(),
            summary: sandbox_status_summary(&current.sandbox),
            command_label: None,
            success: None,
            payload: serde_json::to_value(&current.sandbox).ok(),
        });
    }
    if previous.is_none_or(|snapshot| snapshot.training != current.training) {
        changed_domains.push("training");
        events.push(DesktopControlEventDraft {
            event_type: "training.state.changed".to_string(),
            summary: training_status_summary(&current.training),
            command_label: None,
            success: None,
            payload: serde_json::to_value(&current.training).ok(),
        });
    }
    if previous.is_none_or(|snapshot| snapshot.proofs != current.proofs) {
        changed_domains.push("proofs");
        events.push(DesktopControlEventDraft {
            event_type: "proof.state.changed".to_string(),
            summary: proof_status_summary(&current.proofs),
            command_label: None,
            success: None,
            payload: serde_json::to_value(&current.proofs).ok(),
        });
    }
    if previous.is_none_or(|snapshot| snapshot.challenges != current.challenges) {
        changed_domains.push("challenges");
        events.push(DesktopControlEventDraft {
            event_type: "challenge.state.changed".to_string(),
            summary: challenge_status_summary(&current.challenges),
            command_label: None,
            success: None,
            payload: serde_json::to_value(&current.challenges).ok(),
        });
    }
    if buy_mode_status_changed(
        previous.map(|snapshot| &snapshot.buy_mode),
        &current.buy_mode,
    ) {
        changed_domains.push("buy_mode");
        events.push(DesktopControlEventDraft {
            event_type: "buyer.lifecycle.changed".to_string(),
            summary: buy_mode_status_summary(&current.buy_mode),
            command_label: None,
            success: None,
            payload: serde_json::to_value(&current.buy_mode).ok(),
        });
    }
    if active_job_status_changed(
        previous.and_then(|snapshot| snapshot.active_job.as_ref()),
        current.active_job.as_ref(),
    ) {
        changed_domains.push("active_job");
        events.push(DesktopControlEventDraft {
            event_type: "active_job.lifecycle.changed".to_string(),
            summary: active_job_status_summary(current.active_job.as_ref()),
            command_label: None,
            success: None,
            payload: serde_json::to_value(&current.active_job).ok(),
        });
    }
    if nip28_status_changed(previous.map(|snapshot| &snapshot.nip28), &current.nip28) {
        changed_domains.push("nip28");
        events.push(DesktopControlEventDraft {
            event_type: "nip28.state.changed".to_string(),
            summary: nip28_status_summary(&current.nip28),
            command_label: None,
            success: None,
            payload: serde_json::to_value(&current.nip28).ok(),
        });
    }
    if mission_control_status_changed(
        previous.map(|snapshot| &snapshot.mission_control),
        &current.mission_control,
    ) {
        changed_domains.push("mission_control");
    }

    if !changed_domains.is_empty() {
        events.insert(
            0,
            DesktopControlEventDraft {
                event_type: "control.snapshot.synced".to_string(),
                summary: format!(
                    "snapshot synced revision={} domains={}",
                    current.snapshot_revision,
                    changed_domains.join(",")
                ),
                command_label: None,
                success: Some(true),
                payload: Some(json!({
                    "snapshot_revision": current.snapshot_revision,
                    "state_signature": current.state_signature.clone(),
                    "changed_domains": changed_domains,
                })),
            },
        );
    }

    events
}

fn provider_status_summary(status: &DesktopControlProviderStatus) -> String {
    format!(
        "provider mode={} runtime={} relays={} blockers={}",
        status.mode,
        status.runtime_mode,
        status.connected_relays,
        status.blocker_codes.len()
    )
}

fn local_runtime_status_summary(status: &DesktopControlLocalRuntimeStatus) -> String {
    format!(
        "local runtime lane={} policy={} posture={:?} ready={} go_online_ready={} text={} streaming={} structured={} model_management={} sessions={} action={} enabled={}",
        status.lane.as_deref().unwrap_or("none"),
        status.policy,
        status.diagnostics.posture,
        status.runtime_ready,
        status.go_online_ready,
        status.supports_run_text,
        status.supports_streaming,
        status.supports_structured,
        status.supports_model_management,
        status.supports_sessions,
        status.action,
        status.action_enabled
    )
}

fn cluster_status_summary(status: &DesktopControlClusterStatus) -> String {
    format!(
        "cluster available={} topology={} members={}",
        status.available, status.topology_label, status.member_count
    )
}

fn sandbox_status_summary(status: &DesktopControlSandboxStatus) -> String {
    format!(
        "sandbox available={} profiles={}/{} jobs={} active_jobs={}",
        status.available,
        status.ready_profile_count,
        status.declared_profile_count,
        status.job_count,
        status.active_job_count
    )
}

fn training_status_summary(status: &DesktopControlTrainingStatus) -> String {
    format!(
        "training available={} source={} control={} artifact={} runs={} active_runs={} accepted_outcomes={} participants={}/{} validator={}/{}/{} operator={} operator_runs={}/{}",
        status.available,
        status.source,
        status.control_plane_state,
        status.artifact_plane_state,
        status.run_count,
        status.active_run_count,
        status.accepted_outcome_count,
        status.contributing_participant_count,
        status.admitted_participant_count,
        status.validator_verified_count,
        status.validator_rejected_count,
        status.validator_timed_out_count,
        status.operator.workflow_state,
        status.operator.active_run_count,
        status.operator.run_count
    )
}

fn proof_status_summary(status: &DesktopControlProofStatus) -> String {
    format!(
        "proofs available={} source={} pending={} accepted={} rejected={} challenged={} settlements_terminal={}",
        status.available,
        status.source,
        status.pending_count,
        status.accepted_count,
        status.rejected_count,
        status.challenged_count,
        status.settlement_terminal_count
    )
}

fn challenge_status_summary(status: &DesktopControlChallengeStatus) -> String {
    format!(
        "challenges available={} source={} open={} verified={} rejected={} timed_out={}",
        status.available,
        status.source,
        status.open_count,
        status.verified_count,
        status.rejected_count,
        status.timed_out_count
    )
}

fn gpt_oss_status_summary(status: &DesktopControlGptOssStatus) -> String {
    format!(
        "gpt-oss detected={} backend={} ready={} busy={} loaded={} artifact_present={}",
        status.detected,
        status.backend.as_deref().unwrap_or("unknown"),
        status.ready,
        status.busy,
        status.loaded,
        status.artifact_present
    )
}

fn apple_fm_status_summary(status: &DesktopControlAppleFmStatus) -> String {
    if status.ready {
        format!(
            "apple fm ready model={} adapters={} attached={}",
            status.ready_model.as_deref().unwrap_or("unknown"),
            status.loaded_adapters.len(),
            status
                .active_session_adapter
                .as_ref()
                .map(|adapter| adapter.adapter_id.as_str())
                .unwrap_or("-")
        )
    } else if status.reachable {
        format!(
            "apple fm reachable; waiting for model readiness adapters={}",
            status.loaded_adapters.len()
        )
    } else {
        status
            .last_error
            .clone()
            .unwrap_or_else(|| "apple fm unavailable".to_string())
    }
}

fn local_runtime_policy_label(policy: MissionControlLocalRuntimePolicy) -> &'static str {
    match policy {
        MissionControlLocalRuntimePolicy::None => "none",
        MissionControlLocalRuntimePolicy::AppleFoundationModels => "apple_foundation_models",
        MissionControlLocalRuntimePolicy::GptOssCuda => "gpt_oss_cuda",
        MissionControlLocalRuntimePolicy::GptOssMetal => "gpt_oss_metal",
        MissionControlLocalRuntimePolicy::GptOssCpu => "gpt_oss_cpu",
    }
}

fn local_runtime_lane_label(lane: MissionControlLocalRuntimeLane) -> &'static str {
    match lane {
        MissionControlLocalRuntimeLane::AppleFoundationModels => "apple_foundation_models",
        MissionControlLocalRuntimeLane::GptOss => "gpt_oss",
    }
}

fn local_runtime_action_label(action: MissionControlLocalRuntimeAction) -> &'static str {
    match action {
        MissionControlLocalRuntimeAction::None => "none",
        MissionControlLocalRuntimeAction::StartAppleFm => "start_apple_fm",
        MissionControlLocalRuntimeAction::RefreshAppleFm => "refresh_apple_fm",
        MissionControlLocalRuntimeAction::OpenAppleFmWorkbench => "open_apple_fm_workbench",
        MissionControlLocalRuntimeAction::RefreshGptOss => "refresh_gpt_oss",
        MissionControlLocalRuntimeAction::WarmGptOss => "warm_gpt_oss",
        MissionControlLocalRuntimeAction::UnloadGptOss => "unload_gpt_oss",
        MissionControlLocalRuntimeAction::OpenGptOssWorkbench => "open_gpt_oss_workbench",
    }
}

fn terminal_stream_label(stream: wgpui::components::sections::TerminalStream) -> &'static str {
    match stream {
        wgpui::components::sections::TerminalStream::Stdout => "stdout",
        wgpui::components::sections::TerminalStream::Stderr => "stderr",
    }
}

fn gpt_oss_loaded(
    snapshot: &crate::local_inference_runtime::LocalInferenceExecutionSnapshot,
) -> bool {
    snapshot.ready_model.is_some() || !snapshot.loaded_models.is_empty()
}

fn desktop_control_local_runtime_status(state: &RenderState) -> DesktopControlLocalRuntimeStatus {
    let runtime_view = mission_control_local_runtime_view_model(
        state.desktop_shell_mode,
        &state.provider_runtime,
        &state.gpt_oss_execution,
    );
    let capability_surface = active_local_runtime_capability_surface(
        state.desktop_shell_mode,
        &state.provider_runtime,
        &state.gpt_oss_execution,
    );
    DesktopControlLocalRuntimeStatus {
        policy: local_runtime_policy_label(runtime_view.policy).to_string(),
        lane: runtime_view
            .lane
            .map(local_runtime_lane_label)
            .map(str::to_string),
        runtime_ready: runtime_view.runtime_ready,
        go_online_ready: runtime_view.go_online_ready,
        supports_sell_compute: runtime_view.supports_sell_compute,
        workbench_label: capability_surface.workbench_label.to_string(),
        supports_run_text: capability_surface.supports_run_text,
        supports_streaming: capability_surface.supports_streaming,
        supports_structured: capability_surface.supports_structured,
        supports_model_management: capability_surface.supports_model_management,
        supports_sessions: capability_surface.supports_sessions,
        show_action_button: runtime_view.show_local_model_button,
        action: local_runtime_action_label(runtime_view.primary_action).to_string(),
        action_enabled: runtime_view.local_model_button_enabled,
        action_label: runtime_view.local_model_button_label,
        model_label: runtime_view.model_label,
        backend_label: runtime_view.backend_label,
        load_label: runtime_view.load_label,
        go_online_hint: (!runtime_view.go_online_hint.trim().is_empty())
            .then_some(runtime_view.go_online_hint),
        status_stream: terminal_stream_label(runtime_view.status_stream).to_string(),
        status_line: runtime_view.status_line,
        detail_lines: runtime_view
            .detail_lines
            .into_iter()
            .map(|(_, text)| text)
            .collect(),
        diagnostics: state.gpt_oss_execution.diagnostics.clone(),
    }
}

fn desktop_control_gpt_oss_status(state: &RenderState) -> DesktopControlGptOssStatus {
    let backend = state
        .gpt_oss_execution
        .backend_label
        .trim()
        .to_ascii_lowercase();
    let backend = (!backend.is_empty()).then_some(backend);
    DesktopControlGptOssStatus {
        detected: backend.is_some()
            || state.gpt_oss_execution.configured_model.is_some()
            || state.gpt_oss_execution.configured_model_path.is_some()
            || state.gpt_oss_execution.artifact_present
            || state.gpt_oss_execution.ready_model.is_some()
            || !state.gpt_oss_execution.loaded_models.is_empty(),
        backend: backend.clone(),
        reachable: state.gpt_oss_execution.reachable,
        ready: state.gpt_oss_execution.is_ready(),
        busy: state.gpt_oss_execution.busy,
        supports_sell_compute: backend.as_deref() == Some("cuda"),
        artifact_present: state.gpt_oss_execution.artifact_present,
        loaded: gpt_oss_loaded(&state.gpt_oss_execution),
        configured_model: state.gpt_oss_execution.configured_model.clone(),
        ready_model: state.gpt_oss_execution.ready_model.clone(),
        configured_model_path: state.gpt_oss_execution.configured_model_path.clone(),
        loaded_models: state.gpt_oss_execution.loaded_models.clone(),
        last_action: state.gpt_oss_execution.last_action.clone(),
        last_error: state.gpt_oss_execution.last_error.clone(),
    }
}

fn wallet_status_summary(status: &DesktopControlWalletStatus) -> String {
    format!(
        "wallet balance={} network_status={} balance_known={} reconciling={} withdraw_ready={}",
        if status.balance_known {
            status.balance_sats.to_string()
        } else {
            "unknown".to_string()
        },
        status.network_status,
        status.balance_known,
        status.balance_reconciling,
        status.can_withdraw
    )
}

fn inventory_status_summary(status: &DesktopControlInventoryStatus) -> String {
    crate::provider_inventory::inventory_status_summary(status)
}

fn buyer_procurement_status_summary(status: &DesktopControlBuyerProcurementStatus) -> String {
    format!(
        "buyer procurement load={} mode={} spot_quotes={} forward_quotes={} accepted_spot_orders={} accepted_forward_orders={}",
        status.load_state,
        status.quote_mode,
        status.spot_quotes.len(),
        status.forward_quotes.len(),
        status.accepted_spot_orders.len(),
        status.accepted_forward_orders.len()
    )
}

fn buy_mode_status_summary(status: &DesktopControlBuyModeStatus) -> String {
    match (
        status.enabled,
        status.in_flight_request_id.as_deref(),
        status.in_flight_status.as_deref(),
        status.in_flight_phase.as_deref(),
    ) {
        (false, _, _, _) => "buy mode stopped".to_string(),
        (true, Some(request_id), Some(request_status), Some(phase)) => format!(
            "buy mode request={} status={} phase={} target={} roster={}/{}",
            short_request_id(request_id),
            request_status,
            phase,
            status
                .target_selection
                .selected_peer_pubkey
                .as_deref()
                .map(short_request_id)
                .unwrap_or_else(|| "-".to_string()),
            status.target_selection.eligible_peer_count,
            status.target_selection.observed_peer_count,
        ),
        (true, Some(request_id), _, _) => {
            format!(
                "buy mode request={} in flight target={} roster={}/{}",
                short_request_id(request_id),
                status
                    .target_selection
                    .selected_peer_pubkey
                    .as_deref()
                    .map(short_request_id)
                    .unwrap_or_else(|| "-".to_string()),
                status.target_selection.eligible_peer_count,
                status.target_selection.observed_peer_count,
            )
        }
        (true, None, _, _) => {
            if let Some(target) = status.target_selection.selected_peer_pubkey.as_deref() {
                format!(
                    "buy mode armed target={} roster={}/{}",
                    short_request_id(target),
                    status.target_selection.eligible_peer_count,
                    status.target_selection.observed_peer_count,
                )
            } else {
                format!(
                    "buy mode blocked roster={}/{} reason={}",
                    status.target_selection.eligible_peer_count,
                    status.target_selection.observed_peer_count,
                    status
                        .target_selection
                        .blocked_reason_code
                        .as_deref()
                        .unwrap_or("no-target")
                )
            }
        }
    }
}

fn active_job_status_summary(active_job: Option<&DesktopControlActiveJobStatus>) -> String {
    let Some(active_job) = active_job else {
        return "no active job".to_string();
    };
    format!(
        "active job request={} stage={} next={}",
        short_request_id(active_job.request_id.as_str()),
        active_job.stage,
        active_job.next_expected_event
    )
}

fn nip28_status_summary(status: &DesktopControlNip28Status) -> String {
    if !status.available {
        return format!(
            "nip28 unavailable configured_channel={} loaded={}",
            short_request_id(status.configured_channel_id.as_str()),
            status.configured_channel_loaded
        );
    }
    format!(
        "nip28 group={} channel={} messages={} publishing_outbound={}",
        status.selected_group_name.as_deref().unwrap_or("-"),
        status.selected_channel_name.as_deref().unwrap_or("-"),
        status.recent_messages.len(),
        status.publishing_outbound_count
    )
}

fn mission_control_status_changed(
    previous: Option<&DesktopControlMissionControlStatus>,
    current: &DesktopControlMissionControlStatus,
) -> bool {
    previous.is_none_or(|previous| {
        previous.last_action != current.last_action
            || previous.last_error != current.last_error
            || previous.can_go_online != current.can_go_online
            || previous.blocker_codes != current.blocker_codes
    })
}

fn buy_mode_status_changed(
    previous: Option<&DesktopControlBuyModeStatus>,
    current: &DesktopControlBuyModeStatus,
) -> bool {
    previous.is_none_or(|previous| {
        previous.enabled != current.enabled
            || previous.approved_budget_sats != current.approved_budget_sats
            || previous.cadence_seconds != current.cadence_seconds
            || previous.cadence_millis != current.cadence_millis
            || previous.next_dispatch_countdown_millis != current.next_dispatch_countdown_millis
            || previous.in_flight_request_id != current.in_flight_request_id
            || previous.in_flight_phase != current.in_flight_phase
            || previous.in_flight_status != current.in_flight_status
            || previous.selected_provider_pubkey != current.selected_provider_pubkey
            || previous.result_provider_pubkey != current.result_provider_pubkey
            || previous.invoice_provider_pubkey != current.invoice_provider_pubkey
            || previous.payable_provider_pubkey != current.payable_provider_pubkey
            || previous.payment_blocker_codes != current.payment_blocker_codes
            || previous.payment_blocker_summary != current.payment_blocker_summary
            || previous.target_selection != current.target_selection
            || previous.peer_roster != current.peer_roster
            || previous.recent_requests != current.recent_requests
    })
}

fn active_job_status_changed(
    previous: Option<&DesktopControlActiveJobStatus>,
    current: Option<&DesktopControlActiveJobStatus>,
) -> bool {
    match (previous, current) {
        (None, None) => false,
        (Some(_), None) | (None, Some(_)) => true,
        (Some(previous), Some(current)) => {
            previous.job_id != current.job_id
                || previous.request_id != current.request_id
                || previous.capability != current.capability
                || previous.stage != current.stage
                || previous.projection_stage != current.projection_stage
                || previous.phase != current.phase
                || previous.next_expected_event != current.next_expected_event
                || previous.projection_authority != current.projection_authority
                || previous.quoted_price_sats != current.quoted_price_sats
                || previous.pending_result_publish_event_id
                    != current.pending_result_publish_event_id
                || previous.result_event_id != current.result_event_id
                || previous.result_publish_status != current.result_publish_status
                || previous.result_publish_attempt_count != current.result_publish_attempt_count
                || previous.payment_pointer != current.payment_pointer
                || previous.pending_bolt11 != current.pending_bolt11
                || previous.settlement_status != current.settlement_status
                || previous.settlement_method != current.settlement_method
                || previous.settlement_amount_sats != current.settlement_amount_sats
                || previous.settlement_fees_sats != current.settlement_fees_sats
                || previous.settlement_net_wallet_delta_sats
                    != current.settlement_net_wallet_delta_sats
                || previous.continuity_window_seconds != current.continuity_window_seconds
                || previous.failure_reason != current.failure_reason
        }
    }
}

fn nip28_status_changed(
    previous: Option<&DesktopControlNip28Status>,
    current: &DesktopControlNip28Status,
) -> bool {
    previous.is_none_or(|previous| previous != current)
}

fn short_request_id(request_id: &str) -> String {
    let trimmed = request_id.trim();
    if trimmed.len() <= 12 {
        trimmed.to_string()
    } else {
        format!("{}..", &trimmed[..12])
    }
}

fn apply_action_request(
    state: &mut RenderState,
    action: &DesktopControlActionRequest,
) -> DesktopControlActionResponse {
    if let Some(online) = action.provider_mode_online_target() {
        let response = apply_provider_mode_action(state, online);
        return finalize_action_response(
            state,
            action.label(),
            DesktopControlActionOutcome::response(response),
        );
    }
    if let DesktopControlActionRequest::Withdraw { bolt11 } = action {
        let response = withdraw_action(state, bolt11.as_str());
        return finalize_action_response(
            state,
            action.label(),
            DesktopControlActionOutcome::response(response),
        );
    }
    if matches!(
        action,
        DesktopControlActionRequest::StartBuyMode | DesktopControlActionRequest::StopBuyMode
    ) {
        let response = match action {
            DesktopControlActionRequest::StartBuyMode => start_buy_mode_action(state),
            DesktopControlActionRequest::StopBuyMode => stop_buy_mode_action(state),
            _ => unreachable!("guarded by matches!"),
        };
        return finalize_action_response(
            state,
            action.label(),
            DesktopControlActionOutcome::response(response),
        );
    }
    let outcome = match action {
        DesktopControlActionRequest::GetSnapshot => {
            snapshot_payload_response(state, "Captured desktop control snapshot")
        }
        DesktopControlActionRequest::GetClusterStatus
        | DesktopControlActionRequest::GetClusterTopology => cluster_payload_response(state),
        DesktopControlActionRequest::GetSandboxStatus => sandbox_status_payload_response(state),
        DesktopControlActionRequest::GetResearchStatus => research_payload_response().into(),
        DesktopControlActionRequest::ResetResearchState => reset_research_action().into(),
        DesktopControlActionRequest::GetTrainingStatus => training_payload_response(state).into(),
        DesktopControlActionRequest::LaunchAppleAdapterTraining {
            train_dataset_path,
            held_out_dataset_path,
            package_name,
            author,
            description,
            license,
            apple_fm_base_url,
        } => launch_apple_adapter_training_action(
            state,
            train_dataset_path.as_str(),
            held_out_dataset_path.as_str(),
            package_name.as_str(),
            author.as_str(),
            description.as_str(),
            license.as_str(),
            apple_fm_base_url.as_str(),
        )
        .into(),
        DesktopControlActionRequest::ExportAppleAdapterTraining {
            run_id,
            export_path,
        } => export_apple_adapter_training_action(state, run_id.as_str(), export_path.as_str())
            .into(),
        DesktopControlActionRequest::AcceptAppleAdapterTraining { run_id } => {
            accept_apple_adapter_training_action(state, run_id.as_str()).into()
        }
        DesktopControlActionRequest::CreateSandboxJob {
            profile_id,
            job_id,
            workspace_root,
            entrypoint_type,
            entrypoint,
            payload,
            arguments,
            expected_outputs,
            timeout_request_s,
            network_request,
            filesystem_request,
            payout_reference,
            verification_posture,
        } => create_sandbox_job_action(
            state,
            profile_id.as_str(),
            job_id.as_str(),
            workspace_root.as_str(),
            *entrypoint_type,
            entrypoint.as_str(),
            payload.as_deref(),
            arguments.as_slice(),
            expected_outputs.as_slice(),
            *timeout_request_s,
            network_request.as_str(),
            filesystem_request.as_str(),
            payout_reference.as_deref(),
            verification_posture.as_deref(),
        )
        .into(),
        DesktopControlActionRequest::GetSandboxJob { job_id } => {
            sandbox_job_payload_response(job_id.as_str()).into()
        }
        DesktopControlActionRequest::UploadSandboxFile {
            job_id,
            relative_path,
            content_base64,
        } => upload_sandbox_file_action(
            job_id.as_str(),
            relative_path.as_str(),
            content_base64.as_str(),
        )
        .into(),
        DesktopControlActionRequest::StartSandboxJob { job_id } => {
            start_sandbox_job_action(job_id.as_str()).into()
        }
        DesktopControlActionRequest::WaitSandboxJob { job_id, timeout_ms } => {
            wait_sandbox_job_action(job_id.as_str(), *timeout_ms).into()
        }
        DesktopControlActionRequest::DownloadSandboxArtifact {
            job_id,
            relative_path,
        } => download_sandbox_artifact_action(job_id.as_str(), relative_path.as_str()).into(),
        DesktopControlActionRequest::DownloadSandboxWorkspaceFile {
            job_id,
            relative_path,
        } => download_sandbox_workspace_action(job_id.as_str(), relative_path.as_str()).into(),
        DesktopControlActionRequest::GetProofStatus => proof_payload_response(state).into(),
        DesktopControlActionRequest::GetChallengeStatus => challenge_payload_response(state).into(),
        DesktopControlActionRequest::ListPanes => {
            pane_list_payload_response(state, "Captured desktop pane catalog").into()
        }
        DesktopControlActionRequest::OpenPane { pane } => {
            pane_open_action(state, pane.as_str()).into()
        }
        DesktopControlActionRequest::FocusPane { pane } => {
            pane_focus_action(state, pane.as_str()).into()
        }
        DesktopControlActionRequest::ClosePane { pane } => {
            pane_close_action(state, pane.as_str()).into()
        }
        DesktopControlActionRequest::GetPaneSnapshot { pane } => {
            pane_snapshot_payload_response(state, pane.as_str()).into()
        }
        DesktopControlActionRequest::RefreshLocalRuntime => {
            refresh_local_runtime_action(state).into()
        }
        DesktopControlActionRequest::RefreshAppleFm => refresh_apple_fm_action(state).into(),
        DesktopControlActionRequest::LoadAppleFmAdapter {
            package_path,
            requested_adapter_id,
        } => load_apple_fm_adapter_action(
            state,
            package_path.as_str(),
            requested_adapter_id.as_deref(),
        )
        .into(),
        DesktopControlActionRequest::UnloadAppleFmAdapter { adapter_id } => {
            unload_apple_fm_adapter_action(state, adapter_id.as_str()).into()
        }
        DesktopControlActionRequest::AttachAppleFmSessionAdapter {
            session_id,
            adapter_id,
        } => {
            attach_apple_fm_session_adapter_action(state, session_id.as_str(), adapter_id.as_str())
                .into()
        }
        DesktopControlActionRequest::DetachAppleFmSessionAdapter { session_id } => {
            detach_apple_fm_session_adapter_action(state, session_id.as_str()).into()
        }
        DesktopControlActionRequest::RunAppleFmSmokeTest => {
            run_apple_fm_smoke_test_action(state).into()
        }
        DesktopControlActionRequest::RefreshGptOss => queue_gpt_oss_runtime_action(
            state,
            LocalInferenceRuntimeCommand::Refresh,
            "Queued GPT-OSS runtime refresh",
        )
        .into(),
        DesktopControlActionRequest::RefreshWallet => {
            wallet_action_response(state, SparkPaneAction::Refresh, "Queued wallet refresh").into()
        }
        DesktopControlActionRequest::GetNip90SentPaymentsReport {
            start_epoch_seconds,
            end_epoch_seconds,
            report_date,
        } => nip90_sent_payments_report_response(
            state,
            *start_epoch_seconds,
            *end_epoch_seconds,
            report_date.as_deref(),
        )
        .into(),
        DesktopControlActionRequest::WarmGptOss => queue_gpt_oss_runtime_action(
            state,
            LocalInferenceRuntimeCommand::WarmConfiguredModel,
            "Queued GPT-OSS model warm",
        )
        .into(),
        DesktopControlActionRequest::UnloadGptOss => queue_gpt_oss_runtime_action(
            state,
            LocalInferenceRuntimeCommand::UnloadConfiguredModel,
            "Queued GPT-OSS model unload",
        )
        .into(),
        DesktopControlActionRequest::GetActiveJob => active_job_payload_response(state),
        DesktopControlActionRequest::SelectNip28MainChannel => {
            select_nip28_main_channel_action(state).into()
        }
        DesktopControlActionRequest::SelectNip28Group { group_id } => {
            select_nip28_group_action(state, group_id.as_str()).into()
        }
        DesktopControlActionRequest::SelectNip28Channel { channel_id } => {
            select_nip28_channel_action(state, channel_id.as_str()).into()
        }
        DesktopControlActionRequest::SendNip28Message {
            content,
            reply_to_event_id,
        } => {
            send_nip28_message_action(state, content.as_str(), reply_to_event_id.as_deref()).into()
        }
        DesktopControlActionRequest::RetryNip28Message { event_id } => {
            retry_nip28_message_action(state, event_id.as_str()).into()
        }
        DesktopControlActionRequest::GetMissionControlLogTail { limit } => {
            log_tail_response(state, *limit).into()
        }
        DesktopControlActionRequest::SetProviderMode { .. }
        | DesktopControlActionRequest::StartBuyMode
        | DesktopControlActionRequest::StopBuyMode
        | DesktopControlActionRequest::Withdraw { .. } => {
            unreachable!("action-specific routes should be handled above")
        }
    };
    finalize_action_response(state, action.label(), outcome)
}

struct DesktopControlActionOutcome {
    response: DesktopControlActionResponse,
    snapshot: Option<DesktopControlSnapshot>,
}

impl DesktopControlActionOutcome {
    fn response(response: DesktopControlActionResponse) -> Self {
        Self {
            response,
            snapshot: None,
        }
    }

    fn with_snapshot(
        response: DesktopControlActionResponse,
        snapshot: DesktopControlSnapshot,
    ) -> Self {
        Self {
            response,
            snapshot: Some(snapshot),
        }
    }
}

impl From<DesktopControlActionResponse> for DesktopControlActionOutcome {
    fn from(response: DesktopControlActionResponse) -> Self {
        Self::response(response)
    }
}

fn finalize_action_response(
    state: &mut RenderState,
    action_label: &str,
    outcome: DesktopControlActionOutcome,
) -> DesktopControlActionResponse {
    record_command_outcome(state, action_label, &outcome.response);
    attach_snapshot_metadata(state, outcome.response, outcome.snapshot)
}

fn record_command_outcome(
    state: &mut RenderState,
    action_label: &str,
    response: &DesktopControlActionResponse,
) {
    let completed_at_epoch_ms = current_epoch_ms();
    let summary = format!("{action_label}: {}", response.message);
    state.desktop_control.last_command_summary = Some(summary.clone());
    state.desktop_control.last_command_completed_at_epoch_ms = Some(completed_at_epoch_ms);
    if response.success {
        state.desktop_control.last_command_error = None;
        state.desktop_control.last_action = Some(summary);
        state.desktop_control.last_error = None;
    } else {
        state.desktop_control.last_command_error = Some(response.message.clone());
        state.desktop_control.last_error = Some(response.message.clone());
    }
}

fn snapshot_payload_response(
    state: &RenderState,
    message: impl Into<String>,
) -> DesktopControlActionOutcome {
    let snapshot = snapshot_for_state(state);
    match serde_json::to_value(&snapshot) {
        Ok(payload) => DesktopControlActionOutcome::with_snapshot(
            DesktopControlActionResponse::ok_with_payload(message, payload),
            snapshot,
        ),
        Err(error) => DesktopControlActionOutcome::response(DesktopControlActionResponse::error(
            format!("Failed to encode desktop control snapshot: {error}"),
        )),
    }
}

fn attach_snapshot_metadata(
    state: &RenderState,
    response: DesktopControlActionResponse,
    snapshot: Option<DesktopControlSnapshot>,
) -> DesktopControlActionResponse {
    let snapshot = snapshot.unwrap_or_else(|| snapshot_for_state(state));
    apply_response_snapshot_metadata(response, &snapshot)
}

fn apply_response_snapshot_metadata(
    mut response: DesktopControlActionResponse,
    snapshot: &DesktopControlSnapshot,
) -> DesktopControlActionResponse {
    response.snapshot_revision = Some(snapshot.snapshot_revision);
    response.state_signature = Some(snapshot.state_signature.clone());
    response
}

fn configured_nip28_main_channel(
    chat: &crate::app_state::AutopilotChatState,
) -> Option<(String, String)> {
    let config = DefaultNip28ChannelConfig::from_env_or_default();
    chat.managed_chat_projection
        .snapshot
        .channels
        .iter()
        .find(|channel| channel.channel_id == config.channel_id)
        .map(|channel| (channel.group_id.clone(), channel.channel_id.clone()))
}

fn select_nip28_group(
    chat: &mut crate::app_state::AutopilotChatState,
    group_id: &str,
) -> Result<String, String> {
    if chat.select_managed_chat_group_by_id(group_id) {
        Ok(format!("Selected NIP-28 group {group_id}"))
    } else {
        Err(chat
            .last_error
            .clone()
            .unwrap_or_else(|| format!("Unknown NIP-28 group: {group_id}")))
    }
}

fn select_nip28_channel(
    chat: &mut crate::app_state::AutopilotChatState,
    channel_id: &str,
) -> Result<String, String> {
    let Some(channel) = chat
        .managed_chat_projection
        .snapshot
        .channels
        .iter()
        .find(|channel| channel.channel_id == channel_id)
        .cloned()
    else {
        return Err(format!("Unknown NIP-28 channel: {channel_id}"));
    };
    match chat
        .managed_chat_projection
        .set_selected_channel(channel.group_id.as_str(), channel.channel_id.as_str())
    {
        Ok(()) => {
            chat.selected_workspace =
                crate::app_state::ChatWorkspaceSelection::ManagedGroup(channel.group_id.clone());
            chat.reset_transcript_scroll();
            chat.last_error = None;
            Ok(format!("Selected NIP-28 channel {}", channel.channel_id))
        }
        Err(error) => {
            chat.last_error = Some(error.clone());
            Err(error)
        }
    }
}

fn send_nip28_message(
    chat: &mut crate::app_state::AutopilotChatState,
    identity: &nostr::NostrIdentity,
    content: &str,
    reply_to_event_id: Option<&str>,
) -> Result<String, String> {
    let event_id = crate::input::queue_managed_chat_channel_message(
        chat,
        identity,
        content,
        reply_to_event_id,
    )?;
    Ok(event_id)
}

fn select_nip28_main_channel_action(state: &mut RenderState) -> DesktopControlActionResponse {
    let Some((_, channel_id)) = configured_nip28_main_channel(&state.autopilot_chat) else {
        return DesktopControlActionResponse::error(
            "Configured NIP-28 main channel is not loaded in the managed chat projection yet.",
        );
    };
    select_nip28_channel_action(state, channel_id.as_str())
}

fn select_nip28_group_action(
    state: &mut RenderState,
    group_id: &str,
) -> DesktopControlActionResponse {
    match select_nip28_group(&mut state.autopilot_chat, group_id) {
        Ok(message) => DesktopControlActionResponse::ok_with_payload(
            message,
            json!({
                "group_id": group_id,
            }),
        ),
        Err(error) => DesktopControlActionResponse::error(error),
    }
}

fn select_nip28_channel_action(
    state: &mut RenderState,
    channel_id: &str,
) -> DesktopControlActionResponse {
    match select_nip28_channel(&mut state.autopilot_chat, channel_id) {
        Ok(message) => {
            let group_id = state
                .autopilot_chat
                .active_managed_chat_group()
                .map(|group| group.group_id.clone());
            DesktopControlActionResponse::ok_with_payload(
                message,
                json!({
                    "group_id": group_id,
                    "channel_id": channel_id,
                }),
            )
        }
        Err(error) => DesktopControlActionResponse::error(error),
    }
}

fn send_nip28_message_action(
    state: &mut RenderState,
    content: &str,
    reply_to_event_id: Option<&str>,
) -> DesktopControlActionResponse {
    let Some(identity) = state.nostr_identity.as_ref() else {
        return DesktopControlActionResponse::error(
            "No Nostr identity is loaded for NIP-28 publishing.",
        );
    };
    match send_nip28_message(
        &mut state.autopilot_chat,
        identity,
        content,
        reply_to_event_id,
    ) {
        Ok(event_id) => {
            let channel_id = state
                .autopilot_chat
                .active_managed_chat_channel()
                .map(|channel| channel.channel_id.clone());
            DesktopControlActionResponse::ok_with_payload(
                format!("Queued NIP-28 message {event_id}"),
                json!({
                    "event_id": event_id,
                    "channel_id": channel_id,
                    "reply_to_event_id": reply_to_event_id,
                }),
            )
        }
        Err(error) => DesktopControlActionResponse::error(error),
    }
}

fn retry_nip28_message_action(
    state: &mut RenderState,
    event_id: &str,
) -> DesktopControlActionResponse {
    match state
        .autopilot_chat
        .managed_chat_projection
        .retry_outbound_message(event_id)
    {
        Ok(()) => DesktopControlActionResponse::ok_with_payload(
            format!("Retried NIP-28 message {event_id}"),
            json!({ "event_id": event_id }),
        ),
        Err(error) => DesktopControlActionResponse::error(error),
    }
}

fn apply_provider_mode_action(
    state: &mut RenderState,
    online: bool,
) -> DesktopControlActionResponse {
    let mode = state.provider_runtime.mode;
    if online
        && matches!(
            mode,
            crate::state::provider_runtime::ProviderMode::Online
                | crate::state::provider_runtime::ProviderMode::Connecting
        )
    {
        return DesktopControlActionResponse::ok(format!("Provider already {}", mode.label()));
    }
    if !online && matches!(mode, crate::state::provider_runtime::ProviderMode::Offline) {
        return DesktopControlActionResponse::ok("Provider already offline");
    }
    crate::input::apply_provider_mode_target(
        state,
        online,
        if online {
            ProviderDesiredMode::Online
        } else {
            ProviderDesiredMode::Offline
        },
        "desktop control",
    );
    if let Some(error) = state.provider_runtime.last_error_detail.clone() {
        DesktopControlActionResponse::error(error)
    } else {
        DesktopControlActionResponse::ok(if online {
            "Queued Mission Control go-online transition"
        } else {
            "Queued Mission Control go-offline transition"
        })
    }
}

fn refresh_local_runtime_action(state: &mut RenderState) -> DesktopControlActionResponse {
    let capability_surface = active_local_runtime_capability_surface(
        state.desktop_shell_mode,
        &state.provider_runtime,
        &state.gpt_oss_execution,
    );
    let Some(action) = capability_surface.refresh_action else {
        return DesktopControlActionResponse::error(
            "Local runtime refresh is unavailable because no supported runtime is detected.",
        );
    };
    run_desktop_control_local_runtime_workbench_action(
        state,
        action,
        "Queued local runtime refresh",
    )
}

fn refresh_apple_fm_action(state: &mut RenderState) -> DesktopControlActionResponse {
    if !crate::input::ensure_mission_control_apple_fm_refresh(state) {
        return DesktopControlActionResponse::error(
            "Apple FM refresh is unavailable in this session",
        );
    }
    mission_control_status_response(state, "Queued Apple FM refresh")
}

fn queue_apple_fm_workbench_command_response(
    state: &mut RenderState,
    command: AppleFmWorkbenchCommand,
    success_label: &str,
) -> DesktopControlActionResponse {
    match state.queue_apple_fm_bridge_command(AppleFmBridgeCommand::Workbench(command)) {
        Ok(()) => {
            state.apple_fm_workbench.load_state = crate::app_state::PaneLoadState::Loading;
            state.apple_fm_workbench.last_error = None;
            state.apple_fm_workbench.last_action = Some(success_label.to_string());
            state.mission_control.record_action(success_label);
            DesktopControlActionResponse::ok(success_label)
        }
        Err(error) => {
            state.apple_fm_workbench.load_state = crate::app_state::PaneLoadState::Error;
            state.apple_fm_workbench.last_error = Some(error.clone());
            state.apple_fm_workbench.last_action =
                Some("Apple FM desktop control enqueue failed".to_string());
            state.mission_control.last_action =
                Some("Apple FM desktop control action failed".to_string());
            state.mission_control.last_error = Some(error.clone());
            DesktopControlActionResponse::error(error)
        }
    }
}

fn load_apple_fm_adapter_action(
    state: &mut RenderState,
    package_path: &str,
    requested_adapter_id: Option<&str>,
) -> DesktopControlActionResponse {
    let request_id = format!(
        "desktop-control-apple-fm-{}",
        state.reserve_runtime_command_seq()
    );
    queue_apple_fm_workbench_command_response(
        state,
        AppleFmWorkbenchCommand {
            request_id,
            operation: AppleFmWorkbenchOperation::LoadAdapter,
            instructions: None,
            prompt: None,
            requested_model: None,
            session_id: None,
            adapter_id: requested_adapter_id.map(ToString::to_string),
            adapter_package_path: Some(package_path.to_string()),
            options: None,
            schema_json: None,
            transcript_json: None,
            tool_mode: crate::apple_fm_bridge::AppleFmWorkbenchToolMode::None,
        },
        "Queued Apple FM adapter load",
    )
}

fn unload_apple_fm_adapter_action(
    state: &mut RenderState,
    adapter_id: &str,
) -> DesktopControlActionResponse {
    let request_id = format!(
        "desktop-control-apple-fm-{}",
        state.reserve_runtime_command_seq()
    );
    queue_apple_fm_workbench_command_response(
        state,
        AppleFmWorkbenchCommand {
            request_id,
            operation: AppleFmWorkbenchOperation::UnloadAdapter,
            instructions: None,
            prompt: None,
            requested_model: None,
            session_id: None,
            adapter_id: Some(adapter_id.to_string()),
            adapter_package_path: None,
            options: None,
            schema_json: None,
            transcript_json: None,
            tool_mode: crate::apple_fm_bridge::AppleFmWorkbenchToolMode::None,
        },
        "Queued Apple FM adapter unload",
    )
}

fn attach_apple_fm_session_adapter_action(
    state: &mut RenderState,
    session_id: &str,
    adapter_id: &str,
) -> DesktopControlActionResponse {
    let request_id = format!(
        "desktop-control-apple-fm-{}",
        state.reserve_runtime_command_seq()
    );
    queue_apple_fm_workbench_command_response(
        state,
        AppleFmWorkbenchCommand {
            request_id,
            operation: AppleFmWorkbenchOperation::AttachSessionAdapter,
            instructions: None,
            prompt: None,
            requested_model: None,
            session_id: Some(session_id.to_string()),
            adapter_id: Some(adapter_id.to_string()),
            adapter_package_path: None,
            options: None,
            schema_json: None,
            transcript_json: None,
            tool_mode: crate::apple_fm_bridge::AppleFmWorkbenchToolMode::None,
        },
        "Queued Apple FM session adapter attach",
    )
}

fn detach_apple_fm_session_adapter_action(
    state: &mut RenderState,
    session_id: &str,
) -> DesktopControlActionResponse {
    let request_id = format!(
        "desktop-control-apple-fm-{}",
        state.reserve_runtime_command_seq()
    );
    queue_apple_fm_workbench_command_response(
        state,
        AppleFmWorkbenchCommand {
            request_id,
            operation: AppleFmWorkbenchOperation::DetachSessionAdapter,
            instructions: None,
            prompt: None,
            requested_model: None,
            session_id: Some(session_id.to_string()),
            adapter_id: None,
            adapter_package_path: None,
            options: None,
            schema_json: None,
            transcript_json: None,
            tool_mode: crate::apple_fm_bridge::AppleFmWorkbenchToolMode::None,
        },
        "Queued Apple FM session adapter detach",
    )
}

fn run_apple_fm_smoke_test_action(state: &mut RenderState) -> DesktopControlActionResponse {
    provider_control_action_response(
        state,
        ProviderControlPaneAction::RunLocalFmSummaryTest,
        "Provider Control action applied",
    )
}

fn desktop_control_local_runtime_error(
    state: &RenderState,
    action: LocalRuntimeWorkbenchAction,
) -> Option<String> {
    match action {
        LocalRuntimeWorkbenchAction::AppleFm(_) => state.apple_fm_workbench.last_error.clone(),
        LocalRuntimeWorkbenchAction::GptOss(_) => state.local_inference.last_error.clone(),
    }
}

fn desktop_control_local_runtime_failure_label(
    action: LocalRuntimeWorkbenchAction,
) -> &'static str {
    match action {
        LocalRuntimeWorkbenchAction::AppleFm(_) => "Apple FM desktop control action failed",
        LocalRuntimeWorkbenchAction::GptOss(_) => "GPT-OSS desktop control action failed",
    }
}

fn desktop_control_gpt_oss_workbench_action(
    command: LocalInferenceRuntimeCommand,
) -> Option<LocalRuntimeWorkbenchAction> {
    match command {
        LocalInferenceRuntimeCommand::Refresh => Some(LocalRuntimeWorkbenchAction::GptOss(
            crate::pane_system::LocalInferencePaneAction::RefreshRuntime,
        )),
        LocalInferenceRuntimeCommand::WarmConfiguredModel => {
            Some(LocalRuntimeWorkbenchAction::GptOss(
                crate::pane_system::LocalInferencePaneAction::WarmModel,
            ))
        }
        LocalInferenceRuntimeCommand::UnloadConfiguredModel => {
            Some(LocalRuntimeWorkbenchAction::GptOss(
                crate::pane_system::LocalInferencePaneAction::UnloadModel,
            ))
        }
        LocalInferenceRuntimeCommand::Generate(_) => None,
    }
}

fn run_desktop_control_local_runtime_workbench_action(
    state: &mut RenderState,
    action: LocalRuntimeWorkbenchAction,
    success_label: &str,
) -> DesktopControlActionResponse {
    if !crate::input::desktop_control_run_local_runtime_workbench_action(state, action) {
        return DesktopControlActionResponse::error(
            "Local runtime desktop control action was not handled.",
        );
    }
    if let Some(error) = desktop_control_local_runtime_error(state, action) {
        state.mission_control.last_action =
            Some(desktop_control_local_runtime_failure_label(action).to_string());
        state.mission_control.last_error = Some(error.clone());
        return DesktopControlActionResponse::error(error);
    }
    state.mission_control.record_action(success_label);
    DesktopControlActionResponse::ok(success_label)
}

fn queue_gpt_oss_runtime_action(
    state: &mut RenderState,
    command: LocalInferenceRuntimeCommand,
    action_label: &str,
) -> DesktopControlActionResponse {
    let gpt_oss_status = desktop_control_gpt_oss_status(state);
    if !gpt_oss_status.detected {
        return DesktopControlActionResponse::error(
            "GPT-OSS runtime is unavailable because no GPT-OSS backend is detected.",
        );
    }
    let Some(action) = desktop_control_gpt_oss_workbench_action(command) else {
        return DesktopControlActionResponse::error("Unsupported GPT-OSS desktop control action");
    };
    run_desktop_control_local_runtime_workbench_action(state, action, action_label)
}

fn provider_control_action_response(
    state: &mut RenderState,
    action: ProviderControlPaneAction,
    success_label: &str,
) -> DesktopControlActionResponse {
    crate::input::desktop_control_run_provider_control_action(state, action);
    mission_control_status_response(state, success_label)
}

fn buy_mode_action_response(
    state: &mut RenderState,
    action: BuyModePaymentsPaneAction,
) -> DesktopControlActionResponse {
    crate::input::desktop_control_run_buy_mode_action(state, action);
    if let Some(error) = state.buy_mode_payments.last_error.clone() {
        DesktopControlActionResponse::error(error)
    } else {
        DesktopControlActionResponse::ok(
            state
                .buy_mode_payments
                .last_action
                .clone()
                .unwrap_or_else(|| "Buy Mode action applied".to_string()),
        )
    }
}

fn mission_control_status_response(
    state: &RenderState,
    default_message: &str,
) -> DesktopControlActionResponse {
    if let Some(error) = state.mission_control.last_error.clone() {
        DesktopControlActionResponse::error(error)
    } else {
        DesktopControlActionResponse::ok(
            state
                .mission_control
                .last_action
                .clone()
                .unwrap_or_else(|| default_message.to_string()),
        )
    }
}

fn wallet_action_response(
    state: &mut RenderState,
    action: SparkPaneAction,
    default_message: &str,
) -> DesktopControlActionResponse {
    crate::input::desktop_control_run_spark_action(state, action);
    if let Some(error) = state.spark_wallet.last_error.clone() {
        DesktopControlActionResponse::error(error)
    } else {
        DesktopControlActionResponse::ok(
            state
                .spark_wallet
                .last_action
                .clone()
                .unwrap_or_else(|| default_message.to_string()),
        )
    }
}

fn pay_invoice_action_response(
    state: &mut RenderState,
    action: PayInvoicePaneAction,
    default_message: &str,
) -> DesktopControlActionResponse {
    crate::input::desktop_control_run_pay_invoice_action(state, action);
    if let Some(error) = state.spark_wallet.last_error.clone() {
        DesktopControlActionResponse::error(error)
    } else {
        DesktopControlActionResponse::ok(
            state
                .spark_wallet
                .last_action
                .clone()
                .unwrap_or_else(|| default_message.to_string()),
        )
    }
}

fn pane_resolution_error(pane_ref: &str) -> DesktopControlActionResponse {
    DesktopControlActionResponse::error(format!(
        "Could not resolve pane reference '{}'.",
        pane_ref.trim()
    ))
}

fn pane_list_payload_response(
    state: &RenderState,
    message: impl Into<String>,
) -> DesktopControlActionResponse {
    let registered = enabled_pane_specs()
        .filter(|spec| spec.kind != crate::app_state::PaneKind::Empty)
        .map(|spec| {
            json!({
                "kind": crate::input::desktop_control_pane_kind_key(spec.kind),
                "title": spec.title,
                "command_id": spec.command.map(|command| command.id),
                "singleton": spec.singleton,
                "startup": spec.startup,
            })
        })
        .collect::<Vec<_>>();
    let open = state
        .panes
        .iter()
        .map(|pane| {
            json!({
                "pane_id": pane.id,
                "kind": crate::input::desktop_control_pane_kind_key(pane.kind),
                "title": pane.title,
                "z_index": pane.z_index,
            })
        })
        .collect::<Vec<_>>();
    DesktopControlActionResponse::ok_with_payload(
        message,
        json!({
            "registered": registered,
            "open": open,
            "active_pane_id": PaneController::active(state),
        }),
    )
}

fn pane_open_action(state: &mut RenderState, pane_ref: &str) -> DesktopControlActionResponse {
    let Some(kind) = crate::input::desktop_control_resolve_pane_kind_for_runtime(pane_ref) else {
        return pane_resolution_error(pane_ref);
    };
    let pane_id = PaneController::create_for_kind(state, kind);
    DesktopControlActionResponse::ok_with_payload(
        format!(
            "Opened pane '{}'",
            crate::input::desktop_control_pane_kind_key(kind)
        ),
        json!({
            "pane_id": pane_id,
            "kind": crate::input::desktop_control_pane_kind_key(kind),
            "title": pane_spec(kind).title,
            "snapshot": crate::input::desktop_control_pane_snapshot_details(state, kind),
        }),
    )
}

fn pane_focus_action(state: &mut RenderState, pane_ref: &str) -> DesktopControlActionResponse {
    let Some(kind) = crate::input::desktop_control_resolve_pane_kind_for_runtime(pane_ref) else {
        return pane_resolution_error(pane_ref);
    };
    let Some((pane_id, pane_title)) = state
        .panes
        .iter()
        .filter(|pane| pane.kind == kind)
        .max_by_key(|pane| pane.z_index)
        .map(|pane| (pane.id, pane.title.clone()))
    else {
        return DesktopControlActionResponse::error(format!(
            "Pane '{}' is not currently open.",
            crate::input::desktop_control_pane_kind_key(kind)
        ));
    };
    PaneController::bring_to_front(state, pane_id);
    DesktopControlActionResponse::ok_with_payload(
        format!(
            "Focused pane '{}'",
            crate::input::desktop_control_pane_kind_key(kind)
        ),
        json!({
            "pane_id": pane_id,
            "kind": crate::input::desktop_control_pane_kind_key(kind),
            "title": pane_title,
            "snapshot": crate::input::desktop_control_pane_snapshot_details(state, kind),
        }),
    )
}

fn pane_close_action(state: &mut RenderState, pane_ref: &str) -> DesktopControlActionResponse {
    let Some(kind) = crate::input::desktop_control_resolve_pane_kind_for_runtime(pane_ref) else {
        return pane_resolution_error(pane_ref);
    };
    let Some((pane_id, pane_title)) = state
        .panes
        .iter()
        .filter(|pane| pane.kind == kind)
        .max_by_key(|pane| pane.z_index)
        .map(|pane| (pane.id, pane.title.clone()))
    else {
        return DesktopControlActionResponse::error(format!(
            "Pane '{}' is not currently open.",
            crate::input::desktop_control_pane_kind_key(kind)
        ));
    };
    PaneController::close(state, pane_id);
    DesktopControlActionResponse::ok_with_payload(
        format!(
            "Closed pane '{}'",
            crate::input::desktop_control_pane_kind_key(kind)
        ),
        json!({
            "pane_id": pane_id,
            "kind": crate::input::desktop_control_pane_kind_key(kind),
            "title": pane_title,
            "snapshot": crate::input::desktop_control_pane_snapshot_details(state, kind),
        }),
    )
}

fn pane_snapshot_payload_response(
    state: &RenderState,
    pane_ref: &str,
) -> DesktopControlActionResponse {
    let Some(kind) = crate::input::desktop_control_resolve_pane_kind_for_runtime(pane_ref) else {
        return pane_resolution_error(pane_ref);
    };
    DesktopControlActionResponse::ok_with_payload(
        format!(
            "Captured pane snapshot for '{}'",
            crate::input::desktop_control_pane_kind_key(kind)
        ),
        crate::input::desktop_control_pane_snapshot_details(state, kind),
    )
}

fn start_buy_mode_action(state: &mut RenderState) -> DesktopControlActionResponse {
    if !state.mission_control_buy_mode_enabled() {
        return DesktopControlActionResponse::error("Buy Mode is disabled for this session");
    }
    if state.buy_mode_payments.buy_mode_loop_enabled {
        return DesktopControlActionResponse::ok(format!(
            "Buy Mode already running ({} every {})",
            format_sats_amount(MISSION_CONTROL_BUY_MODE_BUDGET_SATS),
            mission_control_buy_mode_interval_label()
        ));
    }
    buy_mode_action_response(state, BuyModePaymentsPaneAction::ToggleLoop)
}

fn stop_buy_mode_action(state: &mut RenderState) -> DesktopControlActionResponse {
    if !state.buy_mode_payments.buy_mode_loop_enabled {
        return DesktopControlActionResponse::ok("Buy Mode already stopped");
    }
    buy_mode_action_response(state, BuyModePaymentsPaneAction::ToggleLoop)
}

fn active_job_payload_response(state: &RenderState) -> DesktopControlActionOutcome {
    let snapshot = snapshot_for_state(state);
    let payload = snapshot
        .active_job
        .clone()
        .and_then(|active_job| serde_json::to_value(active_job).ok())
        .unwrap_or(Value::Null);
    if payload.is_null() {
        DesktopControlActionOutcome::with_snapshot(
            DesktopControlActionResponse::ok_with_payload("No active job", payload),
            snapshot,
        )
    } else {
        DesktopControlActionOutcome::with_snapshot(
            DesktopControlActionResponse::ok_with_payload("Captured active job state", payload),
            snapshot,
        )
    }
}

fn nip90_sent_payments_report_response(
    state: &mut RenderState,
    start_epoch_seconds: u64,
    end_epoch_seconds: u64,
    report_date: Option<&str>,
) -> DesktopControlActionResponse {
    if end_epoch_seconds <= start_epoch_seconds {
        return DesktopControlActionResponse::error(
            "NIP-90 sent-payments report window end must be greater than start",
        );
    }

    let _ = state.refresh_nip90_payment_facts();
    let _ = state.refresh_nip90_buyer_payment_attempts();

    let report = state
        .nip90_buyer_payment_attempts
        .window_report(start_epoch_seconds, end_epoch_seconds);
    let relay_urls_considered = state
        .relay_connections
        .relays
        .iter()
        .filter(|relay| relay.status == crate::state::operations::RelayConnectionStatus::Connected)
        .map(|relay| relay.url.clone())
        .collect::<Vec<_>>();
    let generated_at_epoch_seconds = current_epoch_seconds();
    let payload = build_nip90_sent_payments_report_payload(
        &report,
        relay_urls_considered,
        start_epoch_seconds,
        end_epoch_seconds,
        report_date,
        generated_at_epoch_seconds,
    );
    match serde_json::to_value(&payload) {
        Ok(value) => DesktopControlActionResponse::ok_with_payload(
            "Captured NIP-90 sent-payments report",
            value,
        ),
        Err(error) => DesktopControlActionResponse::error(format!(
            "Failed to encode NIP-90 sent-payments report: {error}"
        )),
    }
}

fn build_nip90_sent_payments_report_payload(
    report: &crate::state::nip90_buyer_payment_attempts::Nip90BuyerPaymentWindowReport,
    relay_urls_considered: Vec<String>,
    start_epoch_seconds: u64,
    end_epoch_seconds: u64,
    report_date: Option<&str>,
    generated_at_epoch_seconds: u64,
) -> DesktopControlNip90SentPaymentsReport {
    DesktopControlNip90SentPaymentsReport {
        report_date: report_date
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string),
        window_start_epoch_seconds: start_epoch_seconds,
        window_end_epoch_seconds: end_epoch_seconds,
        window_start_rfc3339: epoch_seconds_rfc3339(start_epoch_seconds),
        window_end_rfc3339: epoch_seconds_rfc3339(end_epoch_seconds),
        payment_count: report.payment_count,
        total_sats_sent: report.total_sats_sent,
        total_fee_sats: report.total_fee_sats,
        total_wallet_debit_sats: report.total_wallet_debit_sats,
        connected_relay_count: relay_urls_considered.len(),
        relay_urls_considered,
        deduped_request_count: report.deduped_request_count,
        degraded_binding_count: report.degraded_binding_count,
        generated_at_epoch_seconds,
        generated_at_rfc3339: epoch_seconds_rfc3339(generated_at_epoch_seconds),
    }
}

fn cluster_payload_response(state: &RenderState) -> DesktopControlActionOutcome {
    let snapshot = snapshot_for_state(state);
    match serde_json::to_value(snapshot.cluster.clone()) {
        Ok(payload) => DesktopControlActionOutcome::with_snapshot(
            DesktopControlActionResponse::ok_with_payload(
                "Captured cluster control state",
                payload,
            ),
            snapshot,
        ),
        Err(error) => DesktopControlActionOutcome::response(DesktopControlActionResponse::error(
            format!("Failed to encode cluster control state: {error}"),
        )),
    }
}

fn sandbox_status_payload_response(state: &RenderState) -> DesktopControlActionOutcome {
    let snapshot = snapshot_for_state(state);
    match serde_json::to_value(snapshot.sandbox.clone()) {
        Ok(payload) => DesktopControlActionOutcome::with_snapshot(
            DesktopControlActionResponse::ok_with_payload(
                "Captured sandbox control state",
                payload,
            ),
            snapshot,
        ),
        Err(error) => DesktopControlActionOutcome::response(DesktopControlActionResponse::error(
            format!("Failed to encode sandbox control state: {error}"),
        )),
    }
}

fn proof_payload_response(state: &mut RenderState) -> DesktopControlActionResponse {
    refresh_compute_history_cache_if_due(state, true);
    match serde_json::to_value(desktop_control_proof_status(state)) {
        Ok(payload) => {
            DesktopControlActionResponse::ok_with_payload("Captured proof control state", payload)
        }
        Err(error) => DesktopControlActionResponse::error(format!(
            "Failed to encode proof control state: {error}"
        )),
    }
}

fn training_payload_response(state: &mut RenderState) -> DesktopControlActionResponse {
    refresh_compute_history_cache_if_due(state, true);
    match serde_json::to_value(desktop_control_training_status(state)) {
        Ok(payload) => DesktopControlActionResponse::ok_with_payload(
            "Captured training control state",
            payload,
        ),
        Err(error) => DesktopControlActionResponse::error(format!(
            "Failed to encode training control state: {error}"
        )),
    }
}

fn launch_apple_adapter_training_action(
    state: &mut RenderState,
    train_dataset_path: &str,
    held_out_dataset_path: &str,
    package_name: &str,
    author: &str,
    description: &str,
    license: &str,
    apple_fm_base_url: &str,
) -> DesktopControlActionResponse {
    match crate::apple_adapter_training_control::launch_run(AppleAdapterOperatorLaunchRequest {
        train_dataset_path: train_dataset_path.to_string(),
        held_out_dataset_path: held_out_dataset_path.to_string(),
        package_name: package_name.to_string(),
        author: author.to_string(),
        description: description.to_string(),
        license: license.to_string(),
        apple_fm_base_url: apple_fm_base_url.to_string(),
    }) {
        Ok(run) => training_action_payload_response(
            state,
            false,
            format!("Completed Apple adapter operator launch for {}", run.run_id),
        ),
        Err(error) => DesktopControlActionResponse::error(error),
    }
}

fn export_apple_adapter_training_action(
    state: &mut RenderState,
    run_id: &str,
    export_path: &str,
) -> DesktopControlActionResponse {
    match crate::apple_adapter_training_control::export_run(
        run_id,
        PathBuf::from(export_path).as_path(),
    ) {
        Ok(run) => training_action_payload_response(
            state,
            false,
            format!("Exported Apple adapter operator package for {}", run.run_id),
        ),
        Err(error) => DesktopControlActionResponse::error(error),
    }
}

fn accept_apple_adapter_training_action(
    state: &mut RenderState,
    run_id: &str,
) -> DesktopControlActionResponse {
    let authority_client = match crate::kernel_control::remote_authority_client_for_state(state) {
        Ok(client) => client,
        Err(error) => return DesktopControlActionResponse::error(error),
    };
    match crate::apple_adapter_training_control::accept_run(run_id, &authority_client) {
        Ok(run) => training_action_payload_response(
            state,
            true,
            format!(
                "Accepted Apple adapter operator run {} into kernel authority",
                run.run_id
            ),
        ),
        Err(error) => DesktopControlActionResponse::error(error),
    }
}

fn training_action_payload_response(
    state: &mut RenderState,
    refresh_authority: bool,
    message: impl Into<String>,
) -> DesktopControlActionResponse {
    if refresh_authority {
        refresh_compute_history_cache_if_due(state, true);
    }
    match serde_json::to_value(desktop_control_training_status(state)) {
        Ok(payload) => DesktopControlActionResponse::ok_with_payload(message, payload),
        Err(error) => DesktopControlActionResponse::error(format!(
            "Failed to encode training control state: {error}"
        )),
    }
}

fn challenge_payload_response(state: &mut RenderState) -> DesktopControlActionResponse {
    refresh_compute_history_cache_if_due(state, true);
    match serde_json::to_value(desktop_control_challenge_status(state)) {
        Ok(payload) => DesktopControlActionResponse::ok_with_payload(
            "Captured challenge control state",
            payload,
        ),
        Err(error) => DesktopControlActionResponse::error(format!(
            "Failed to encode challenge control state: {error}"
        )),
    }
}

fn research_payload_response() -> DesktopControlActionResponse {
    match research_control::research_status() {
        Ok(status) => match serde_json::to_value(status) {
            Ok(payload) => DesktopControlActionResponse::ok_with_payload(
                "Captured research frontier state",
                payload,
            ),
            Err(error) => DesktopControlActionResponse::error(format!(
                "Failed to encode research frontier state: {error}"
            )),
        },
        Err(error) => {
            DesktopControlActionResponse::error(format!("Research control error: {error}"))
        }
    }
}

fn reset_research_action() -> DesktopControlActionResponse {
    match research_control::reset_research_state() {
        Ok(status) => match serde_json::to_value(status) {
            Ok(payload) => DesktopControlActionResponse::ok_with_payload(
                "Reset research frontier state",
                payload,
            ),
            Err(error) => DesktopControlActionResponse::error(format!(
                "Failed to encode research frontier state: {error}"
            )),
        },
        Err(error) => {
            DesktopControlActionResponse::error(format!("Research control error: {error}"))
        }
    }
}

fn sandbox_error_response(error: impl Into<String>) -> DesktopControlActionResponse {
    DesktopControlActionResponse::error(format!("Sandbox control error: {}", error.into()))
}

fn find_sandbox_profile(
    state: &RenderState,
    profile_id: &str,
) -> Result<ProviderSandboxProfile, DesktopControlActionResponse> {
    state
        .provider_runtime
        .sandbox
        .profiles
        .iter()
        .find(|profile| profile.profile_id == profile_id)
        .cloned()
        .ok_or_else(|| {
            sandbox_error_response(format!(
                "unknown sandbox profile `{profile_id}` in current desktop runtime"
            ))
        })
}

fn create_sandbox_job_action(
    state: &RenderState,
    profile_id: &str,
    job_id: &str,
    workspace_root: &str,
    entrypoint_type: ProviderSandboxEntrypointType,
    entrypoint: &str,
    payload: Option<&str>,
    arguments: &[String],
    expected_outputs: &[String],
    timeout_request_s: u64,
    network_request: &str,
    filesystem_request: &str,
    payout_reference: Option<&str>,
    verification_posture: Option<&str>,
) -> DesktopControlActionResponse {
    let profile = match find_sandbox_profile(state, profile_id) {
        Ok(profile) => profile,
        Err(response) => return response,
    };
    let request = ProviderSandboxJobRequest {
        job_id: job_id.to_string(),
        provider_id: "desktop-control".to_string(),
        compute_product_id: profile.execution_class.product_id().to_string(),
        execution_class: profile.execution_class,
        entrypoint_type,
        entrypoint: entrypoint.to_string(),
        payload: payload.map(ToString::to_string),
        arguments: arguments.to_vec(),
        workspace_root: PathBuf::from(workspace_root),
        expected_outputs: expected_outputs.to_vec(),
        timeout_request_s,
        network_request: network_request.to_string(),
        filesystem_request: filesystem_request.to_string(),
        environment: Vec::new(),
        resource_request: Default::default(),
        payout_reference: payout_reference.map(ToString::to_string),
        verification_posture: verification_posture.map(ToString::to_string),
    };
    match desktop_control_sandbox_service().create_job(
        profile,
        request,
        ProviderSandboxExecutionControls::default(),
    ) {
        Ok(job) => DesktopControlActionResponse::ok_with_payload(
            format!("Created sandbox job {}", job.job_id),
            serde_json::to_value(job).unwrap_or(Value::Null),
        ),
        Err(error) => sandbox_error_response(error.to_string()),
    }
}

fn sandbox_job_payload_response(job_id: &str) -> DesktopControlActionResponse {
    match desktop_control_sandbox_service().poll_job(job_id) {
        Ok(job) => DesktopControlActionResponse::ok_with_payload(
            format!("Captured sandbox job {}", job.job_id),
            serde_json::to_value(job).unwrap_or(Value::Null),
        ),
        Err(error) => sandbox_error_response(error.to_string()),
    }
}

fn upload_sandbox_file_action(
    job_id: &str,
    relative_path: &str,
    content_base64: &str,
) -> DesktopControlActionResponse {
    let decoded = match URL_SAFE_NO_PAD.decode(content_base64.as_bytes()) {
        Ok(bytes) => bytes,
        Err(error) => {
            return sandbox_error_response(format!(
                "invalid base64 content for sandbox upload: {error}"
            ));
        }
    };
    match desktop_control_sandbox_service().upload_file(job_id, relative_path, decoded.as_slice()) {
        Ok(receipt) => DesktopControlActionResponse::ok_with_payload(
            format!("Uploaded sandbox file `{relative_path}` for job {job_id}"),
            serde_json::to_value(receipt).unwrap_or(Value::Null),
        ),
        Err(error) => sandbox_error_response(error.to_string()),
    }
}

fn start_sandbox_job_action(job_id: &str) -> DesktopControlActionResponse {
    match desktop_control_sandbox_service().start_job(job_id) {
        Ok(job) => DesktopControlActionResponse::ok_with_payload(
            format!("Started sandbox job {job_id}"),
            serde_json::to_value(job).unwrap_or(Value::Null),
        ),
        Err(error) => sandbox_error_response(error.to_string()),
    }
}

fn wait_sandbox_job_action(job_id: &str, timeout_ms: u64) -> DesktopControlActionResponse {
    match desktop_control_sandbox_service()
        .wait_for_job(job_id, Duration::from_millis(timeout_ms.max(1)))
    {
        Ok(job) => DesktopControlActionResponse::ok_with_payload(
            format!("Waited on sandbox job {job_id}"),
            serde_json::to_value(job).unwrap_or(Value::Null),
        ),
        Err(error) => sandbox_error_response(error.to_string()),
    }
}

fn sandbox_download_payload(receipt: ProviderSandboxFileTransferReceipt, bytes: Vec<u8>) -> Value {
    json!({
        "receipt": receipt,
        "content_base64": URL_SAFE_NO_PAD.encode(bytes.as_slice()),
        "utf8_preview": String::from_utf8(bytes).ok(),
    })
}

fn download_sandbox_artifact_action(
    job_id: &str,
    relative_path: &str,
) -> DesktopControlActionResponse {
    match desktop_control_sandbox_service().download_artifact(job_id, relative_path) {
        Ok(file) => DesktopControlActionResponse::ok_with_payload(
            format!("Downloaded sandbox artifact `{relative_path}` for job {job_id}"),
            sandbox_download_payload(file.receipt, file.bytes),
        ),
        Err(error) => sandbox_error_response(error.to_string()),
    }
}

fn download_sandbox_workspace_action(
    job_id: &str,
    relative_path: &str,
) -> DesktopControlActionResponse {
    match desktop_control_sandbox_service().download_workspace_file(job_id, relative_path) {
        Ok(file) => DesktopControlActionResponse::ok_with_payload(
            format!("Downloaded sandbox workspace file `{relative_path}` for job {job_id}"),
            sandbox_download_payload(file.receipt, file.bytes),
        ),
        Err(error) => sandbox_error_response(error.to_string()),
    }
}

fn withdraw_action(state: &mut RenderState, bolt11: &str) -> DesktopControlActionResponse {
    let trimmed = bolt11.trim();
    if trimmed.is_empty() {
        return DesktopControlActionResponse::error("Withdrawal bolt11 invoice is required");
    }
    state
        .pay_invoice_inputs
        .payment_request
        .set_value(trimmed.to_string());
    pay_invoice_action_response(
        state,
        PayInvoicePaneAction::SendPayment,
        "Queued Lightning withdrawal",
    )
}

fn log_tail_response(state: &RenderState, limit: usize) -> DesktopControlActionResponse {
    let lines = mission_control_recent_lines(state, limit);
    DesktopControlActionResponse::ok_with_payload(
        format!("Captured {} Mission Control log line(s)", lines.len()),
        json!({ "lines": lines }),
    )
}

fn desktop_control_sandbox_service() -> &'static InMemorySandboxJobService {
    DESKTOP_CONTROL_SANDBOX_SERVICE.get_or_init(InMemorySandboxJobService::default)
}

fn desktop_control_cluster_status() -> DesktopControlClusterStatus {
    DesktopControlClusterStatus {
        available: false,
        topology_label: "not_integrated".to_string(),
        member_count: 0,
        members: Vec::new(),
        last_error: Some(crate::provider_inventory::CLUSTER_NOT_INTEGRATED_REASON.to_string()),
    }
}

fn desktop_control_sandbox_profile_status(
    profile: &ProviderSandboxProfile,
) -> DesktopControlSandboxProfileStatus {
    DesktopControlSandboxProfileStatus {
        profile_id: profile.profile_id.clone(),
        profile_digest: profile.profile_digest.clone(),
        execution_class: profile.execution_class.product_id().to_string(),
        runtime_kind: profile.runtime_kind.id().to_string(),
        runtime_ready: profile.runtime_ready,
        capability_summary: profile.capability_summary.clone(),
    }
}

fn desktop_control_sandbox_job_status(
    job: &ProviderSandboxBackgroundJobSnapshot,
) -> DesktopControlSandboxJobStatus {
    DesktopControlSandboxJobStatus {
        job_id: job.job_id.clone(),
        profile_id: job.profile_id.clone(),
        profile_digest: job.profile_digest.clone(),
        compute_product_id: job.compute_product_id.clone(),
        state: format!("{:?}", job.state).to_ascii_lowercase(),
        created_at_epoch_ms: job.created_at_ms,
        updated_at_epoch_ms: job.updated_at_ms,
        upload_count: job.uploads.len(),
        download_count: job.downloads.len(),
        last_detail: job
            .lifecycle_events
            .last()
            .and_then(|event| event.detail.clone()),
        terminal_receipt_type: job
            .terminal_receipt
            .as_ref()
            .map(|receipt| receipt.receipt_type.clone()),
    }
}

fn desktop_control_sandbox_status(state: &RenderState) -> DesktopControlSandboxStatus {
    let mut jobs = desktop_control_sandbox_service().list_jobs();
    jobs.sort_by(|left, right| {
        right
            .updated_at_ms
            .cmp(&left.updated_at_ms)
            .then_with(|| right.job_id.cmp(&left.job_id))
    });
    let profiles = state
        .provider_runtime
        .sandbox
        .profiles
        .iter()
        .map(desktop_control_sandbox_profile_status)
        .collect::<Vec<_>>();
    let active_job_count = jobs.iter().filter(|job| !job.state.is_terminal()).count();
    DesktopControlSandboxStatus {
        available: !profiles.is_empty(),
        declared_profile_count: profiles.len(),
        ready_profile_count: profiles
            .iter()
            .filter(|profile| profile.runtime_ready)
            .count(),
        job_count: jobs.len(),
        active_job_count,
        profiles,
        jobs: jobs
            .iter()
            .map(desktop_control_sandbox_job_status)
            .collect::<Vec<_>>(),
        last_error: if state.provider_runtime.sandbox.profiles.is_empty() {
            Some(
                "no declared sandbox profiles are available in the current desktop runtime"
                    .to_string(),
            )
        } else {
            None
        },
    }
}

#[derive(Default)]
struct LoadedComputeHistory {
    delivery_proofs: Vec<DeliveryProof>,
    capacity_instruments: Vec<CapacityInstrument>,
    structured_capacity_instruments: Vec<StructuredCapacityInstrument>,
    training_runs: Vec<ComputeTrainingRun>,
    accepted_outcomes: Vec<ComputeAcceptedOutcome>,
    validator_challenges: Vec<ComputeValidatorChallengeSnapshot>,
}

fn refresh_compute_history_cache_if_due(state: &mut RenderState, force: bool) -> bool {
    let now_epoch_ms = current_epoch_ms();
    let provider_id = crate::kernel_control::provider_id_for_state(state);
    let provider_changed =
        state.desktop_control.compute_history.provider_id.as_deref() != Some(provider_id.as_str());
    let due = force
        || provider_changed
        || state
            .desktop_control
            .compute_history
            .last_refreshed_at_epoch_ms
            .is_none_or(|last| {
                now_epoch_ms.saturating_sub(last)
                    >= DESKTOP_CONTROL_COMPUTE_HISTORY_REFRESH_INTERVAL_MS
            });
    if !due {
        return false;
    }

    if provider_changed {
        state.desktop_control.compute_history.provider_id = Some(provider_id.clone());
        state
            .desktop_control
            .compute_history
            .delivery_proofs
            .clear();
        state
            .desktop_control
            .compute_history
            .capacity_instruments
            .clear();
        state
            .desktop_control
            .compute_history
            .structured_capacity_instruments
            .clear();
        state.desktop_control.compute_history.training_runs.clear();
        state
            .desktop_control
            .compute_history
            .accepted_outcomes
            .clear();
        state
            .desktop_control
            .compute_history
            .validator_challenges
            .clear();
    }

    let result = load_compute_history_from_authority(state, provider_id.as_str());
    let cache = &mut state.desktop_control.compute_history;
    let mut changed = false;
    match result {
        Ok(loaded) => {
            changed |= cache.delivery_proofs != loaded.delivery_proofs;
            changed |= cache.capacity_instruments != loaded.capacity_instruments;
            changed |=
                cache.structured_capacity_instruments != loaded.structured_capacity_instruments;
            changed |= cache.training_runs != loaded.training_runs;
            changed |= cache.accepted_outcomes != loaded.accepted_outcomes;
            changed |= cache.validator_challenges != loaded.validator_challenges;
            changed |= cache.last_error.is_some();
            cache.delivery_proofs = loaded.delivery_proofs;
            cache.capacity_instruments = loaded.capacity_instruments;
            cache.structured_capacity_instruments = loaded.structured_capacity_instruments;
            cache.training_runs = loaded.training_runs;
            cache.accepted_outcomes = loaded.accepted_outcomes;
            cache.validator_challenges = loaded.validator_challenges;
            cache.last_error = None;
            cache.last_action = Some(format!(
                "Loaded kernel compute proof, challenge, training, and outcome history for {} proofs / {} runs / {} outcomes",
                cache.delivery_proofs.len(),
                cache.training_runs.len(),
                cache.accepted_outcomes.len()
            ));
        }
        Err(error) => {
            changed |= cache.last_error.as_deref() != Some(error.as_str());
            cache.last_error = Some(error);
            cache.last_action = Some("Kernel compute history refresh failed".to_string());
        }
    }
    changed |= cache.last_refreshed_at_epoch_ms != Some(now_epoch_ms);
    cache.last_refreshed_at_epoch_ms = Some(now_epoch_ms);
    changed
}

fn load_compute_history_from_authority(
    state: &RenderState,
    provider_id: &str,
) -> Result<LoadedComputeHistory, String> {
    let client = crate::kernel_control::remote_authority_client_for_state(state)?;
    crate::kernel_control::run_kernel_call(async move {
        let lots = client.list_capacity_lots(None, None).await?;
        let provider_lot_ids = lots
            .into_iter()
            .filter(|lot| lot.provider_id == provider_id)
            .map(|lot| lot.capacity_lot_id)
            .collect::<BTreeSet<_>>();

        let mut delivery_proofs = Vec::new();
        for lot_id in &provider_lot_ids {
            delivery_proofs.extend(
                client
                    .list_delivery_proofs(Some(lot_id.as_str()), None)
                    .await?,
            );
        }
        delivery_proofs.sort_by(|left, right| {
            right
                .created_at_ms
                .cmp(&left.created_at_ms)
                .then_with(|| left.delivery_proof_id.cmp(&right.delivery_proof_id))
        });
        let provider_delivery_proof_ids = delivery_proofs
            .iter()
            .map(|proof| proof.delivery_proof_id.clone())
            .collect::<BTreeSet<_>>();

        let proof_instrument_ids = delivery_proofs
            .iter()
            .filter_map(|proof| proof.instrument_id.clone())
            .collect::<BTreeSet<_>>();
        let proof_bundle_digests =
            proof_bundle_digests_by_delivery_proof(delivery_proofs.as_slice());

        let mut capacity_instruments = client.list_capacity_instruments(None, None, None).await?;
        capacity_instruments.retain(|instrument| {
            instrument.provider_id.as_deref() == Some(provider_id)
                || proof_instrument_ids.contains(instrument.instrument_id.as_str())
        });
        capacity_instruments.sort_by(|left, right| {
            right
                .created_at_ms
                .cmp(&left.created_at_ms)
                .then_with(|| left.instrument_id.cmp(&right.instrument_id))
        });

        let mut structured_capacity_instruments = client
            .list_structured_capacity_instruments(None, None)
            .await?;
        structured_capacity_instruments.retain(|instrument| {
            instrument.provider_id.as_deref() == Some(provider_id)
                || instrument
                    .legs
                    .iter()
                    .any(|leg| proof_instrument_ids.contains(leg.instrument_id.as_str()))
        });
        structured_capacity_instruments.sort_by(|left, right| {
            right.created_at_ms.cmp(&left.created_at_ms).then_with(|| {
                left.structured_instrument_id
                    .cmp(&right.structured_instrument_id)
            })
        });

        let mut validator_challenges = client.list_validator_challenges(None).await?;
        validator_challenges.retain(|challenge| {
            proof_bundle_digests
                .get(challenge.request.context.proof_bundle_digest.as_str())
                .is_some()
                || challenge
                    .request
                    .context
                    .delivery_proof_id
                    .as_deref()
                    .is_some_and(|proof_id| {
                        delivery_proofs
                            .iter()
                            .any(|proof| proof.delivery_proof_id == proof_id)
                    })
        });
        validator_challenges.sort_by(|left, right| {
            challenge_sort_epoch_ms(right)
                .cmp(&challenge_sort_epoch_ms(left))
                .then_with(|| {
                    left.request
                        .context
                        .challenge_id
                        .cmp(&right.request.context.challenge_id)
                })
        });

        let mut training_runs = client.list_compute_training_runs(None, None, None).await?;
        training_runs.retain(|run| {
            run.capacity_lot_id
                .as_deref()
                .is_some_and(|lot_id| provider_lot_ids.contains(lot_id))
                || run
                    .delivery_proof_id
                    .as_deref()
                    .is_some_and(|proof_id| provider_delivery_proof_ids.contains(proof_id))
        });
        training_runs.sort_by(|left, right| {
            training_run_sort_epoch_ms(right)
                .cmp(&training_run_sort_epoch_ms(left))
                .then_with(|| left.training_run_id.cmp(&right.training_run_id))
        });

        let training_run_ids = training_runs
            .iter()
            .map(|run| run.training_run_id.clone())
            .collect::<BTreeSet<_>>();
        let related_eval_run_ids = training_runs
            .iter()
            .flat_map(|run| run.rollout_verification_eval_run_ids.iter().cloned())
            .collect::<BTreeSet<_>>();
        let mut accepted_outcomes = client.list_compute_accepted_outcomes(None, None).await?;
        accepted_outcomes.retain(|outcome| match outcome.outcome_kind {
            ComputeAcceptedOutcomeKind::TrainingRun => {
                training_run_ids.contains(outcome.source_run_id.as_str())
            }
            ComputeAcceptedOutcomeKind::EvaluationRun => {
                related_eval_run_ids.contains(outcome.source_run_id.as_str())
            }
        });
        accepted_outcomes.sort_by(|left, right| {
            right
                .accepted_at_ms
                .cmp(&left.accepted_at_ms)
                .then_with(|| left.outcome_id.cmp(&right.outcome_id))
        });

        Ok(LoadedComputeHistory {
            delivery_proofs,
            capacity_instruments,
            structured_capacity_instruments,
            training_runs,
            accepted_outcomes,
            validator_challenges,
        })
    })
}

fn proof_bundle_digests_by_delivery_proof(
    proofs: &[DeliveryProof],
) -> BTreeMap<String, Vec<String>> {
    let mut digests = BTreeMap::new();
    for proof in proofs {
        let Some(proof_bundle_ref) = proof
            .verification_evidence
            .as_ref()
            .and_then(|verification| verification.proof_bundle_ref.as_deref())
        else {
            continue;
        };
        digests
            .entry(sha256_prefixed_text(proof_bundle_ref))
            .or_insert_with(Vec::new)
            .push(proof.delivery_proof_id.clone());
    }
    digests
}

fn challenge_sort_epoch_ms(challenge: &ComputeValidatorChallengeSnapshot) -> u64 {
    challenge
        .final_result
        .as_ref()
        .map(|result| result.finalized_at_ms)
        .or_else(|| {
            challenge
                .active_lease
                .as_ref()
                .map(|lease| lease.leased_at_ms)
        })
        .unwrap_or(challenge.request.context.created_at_ms)
}

fn training_run_sort_epoch_ms(run: &ComputeTrainingRun) -> i64 {
    run.finalized_at_ms
        .or(run.started_at_ms)
        .unwrap_or(run.created_at_ms)
}

fn authority_history_source_label(state: &RenderState) -> String {
    let authority_configured = state
        .hosted_control_base_url
        .as_deref()
        .is_some_and(|value| !value.trim().is_empty())
        && state
            .hosted_control_bearer_token
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty());
    let cache = &state.desktop_control.compute_history;
    if !authority_configured {
        return "unavailable".to_string();
    }
    if cache.last_refreshed_at_epoch_ms.is_none() {
        return "kernel_authority_pending".to_string();
    }
    if cache.last_error.is_some() {
        if cache.delivery_proofs.is_empty()
            && cache.validator_challenges.is_empty()
            && cache.capacity_instruments.is_empty()
            && cache.structured_capacity_instruments.is_empty()
            && cache.training_runs.is_empty()
            && cache.accepted_outcomes.is_empty()
        {
            "kernel_authority_error".to_string()
        } else {
            "kernel_authority_stale".to_string()
        }
    } else {
        "kernel_authority".to_string()
    }
}

fn desktop_control_training_status(state: &RenderState) -> DesktopControlTrainingStatus {
    let cache = &state.desktop_control.compute_history;
    let source = authority_history_source_label(state);
    let cluster = desktop_control_cluster_status();
    let sandbox = desktop_control_sandbox_status(state);
    let operator = desktop_control_apple_adapter_operator_status();
    let available = cache.last_refreshed_at_epoch_ms.is_some();
    let control_plane_state = if cache.last_refreshed_at_epoch_ms.is_none() {
        if cache.last_error.is_some() {
            "error"
        } else {
            "pending"
        }
    } else if cache.last_error.is_some() {
        if cache.training_runs.is_empty() && cache.accepted_outcomes.is_empty() {
            "error"
        } else {
            "authority_projected_stale"
        }
    } else if cache.training_runs.is_empty() && cache.accepted_outcomes.is_empty() {
        "authority_projected_idle"
    } else {
        "authority_projected"
    }
    .to_string();
    let artifact_plane_state = if sandbox.active_job_count > 0 {
        "staging_active"
    } else if sandbox.available && sandbox.ready_profile_count > 0 {
        "staging_idle"
    } else if cluster.available && cluster.member_count > 0 {
        "cluster_ready"
    } else if sandbox.available || cluster.available {
        "available_idle"
    } else {
        "unavailable"
    }
    .to_string();

    let accepted_training_outcomes = cache
        .accepted_outcomes
        .iter()
        .filter(|outcome| outcome.outcome_kind == ComputeAcceptedOutcomeKind::TrainingRun)
        .map(|outcome| (outcome.source_run_id.as_str(), outcome))
        .collect::<BTreeMap<_, _>>();

    let environment_versions = cache
        .training_runs
        .iter()
        .filter_map(|run| run.environment_binding.environment_version.clone())
        .chain(
            cache
                .accepted_outcomes
                .iter()
                .filter_map(|outcome| outcome.environment_binding.environment_version.clone()),
        )
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();

    let checkpoint_refs = cache
        .training_runs
        .iter()
        .filter_map(|run| run.checkpoint_binding.latest_checkpoint_ref.clone())
        .chain(
            cache
                .training_runs
                .iter()
                .filter_map(|run| run.final_checkpoint_ref.clone()),
        )
        .chain(
            cache
                .training_runs
                .iter()
                .filter_map(|run| run.promotion_checkpoint_ref.clone()),
        )
        .chain(cache.training_runs.iter().filter_map(|run| {
            run.summary
                .as_ref()
                .and_then(|summary| summary.accepted_checkpoint_ref.clone())
        }))
        .chain(cache.accepted_outcomes.iter().filter_map(|outcome| {
            outcome
                .checkpoint_binding
                .as_ref()
                .and_then(|binding| binding.latest_checkpoint_ref.clone())
        }))
        .chain(cache.accepted_outcomes.iter().filter_map(|outcome| {
            outcome
                .training_summary
                .as_ref()
                .and_then(|summary| summary.accepted_checkpoint_ref.clone())
        }))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();

    let contributor_set_revision = cache.training_runs.iter().find_map(|run| {
        metadata_string(
            &run.metadata,
            &["contributor_set_revision_id", "contributor_set_revision"],
        )
    });
    let contributor_reselection_timing = cache.training_runs.iter().find_map(|run| {
        metadata_string(
            &run.metadata,
            &[
                "contributor_reselection_timing",
                "contributor_reselection_interval",
            ],
        )
        .or_else(|| {
            metadata_u64(&run.metadata, &["contributor_reselection_interval_ms"])
                .map(|value| format!("{value}ms"))
        })
    });
    let stale_rollout_discard_count = cache
        .training_runs
        .iter()
        .filter_map(|run| {
            metadata_u64(
                &run.metadata,
                &["stale_rollout_discard_count", "stale_rollout_drop_count"],
            )
        })
        .sum::<u64>() as usize;
    let duplicate_rollout_quarantine_count = cache
        .training_runs
        .iter()
        .filter_map(|run| metadata_u64(&run.metadata, &["duplicate_rollout_quarantine_count"]))
        .sum::<u64>() as usize;
    let duplicate_rollout_deweight_count = cache
        .training_runs
        .iter()
        .filter_map(|run| metadata_u64(&run.metadata, &["duplicate_rollout_deweight_count"]))
        .sum::<u64>() as usize;

    let runs = cache
        .training_runs
        .iter()
        .take(DESKTOP_CONTROL_COMPUTE_HISTORY_LIMIT)
        .map(|run| {
            let accepted_outcome = accepted_training_outcomes.get(run.training_run_id.as_str());
            let run_control_plane_state = match run.status {
                ComputeTrainingRunStatus::Queued => "queued",
                ComputeTrainingRunStatus::Preparing => "preparing",
                ComputeTrainingRunStatus::Running => "running",
                ComputeTrainingRunStatus::Finalizing => "finalizing",
                ComputeTrainingRunStatus::Accepted => "accepted",
                ComputeTrainingRunStatus::Failed => "failed",
                ComputeTrainingRunStatus::Cancelled => "cancelled",
            }
            .to_string();
            let run_artifact_plane_state = if run
                .promotion_checkpoint_ref
                .as_deref()
                .is_some_and(|value| !value.trim().is_empty())
            {
                "promotion_materialized"
            } else if run
                .final_checkpoint_ref
                .as_deref()
                .is_some_and(|value| !value.trim().is_empty())
            {
                "checkpoint_materialized"
            } else if run.status == ComputeTrainingRunStatus::Finalizing {
                "promotion_pending"
            } else if matches!(
                run.status,
                ComputeTrainingRunStatus::Preparing | ComputeTrainingRunStatus::Running
            ) {
                if sandbox.active_job_count > 0 {
                    "artifacts_active"
                } else {
                    "artifacts_pending"
                }
            } else if matches!(
                run.status,
                ComputeTrainingRunStatus::Failed | ComputeTrainingRunStatus::Cancelled
            ) {
                "incomplete"
            } else {
                "idle"
            }
            .to_string();
            DesktopControlTrainingRunStatus {
                training_run_id: run.training_run_id.clone(),
                status: run.status.label().to_string(),
                training_policy_ref: run.training_policy_ref.clone(),
                environment_ref: run.environment_binding.environment_ref.clone(),
                environment_version: run.environment_binding.environment_version.clone(),
                checkpoint_family: run.checkpoint_binding.checkpoint_family.clone(),
                validator_policy_ref: run.validator_policy_ref.clone(),
                benchmark_package_count: run.benchmark_package_refs.len(),
                rollout_verification_eval_run_count: run.rollout_verification_eval_run_ids.len(),
                expected_step_count: run.expected_step_count,
                completed_step_count: run.completed_step_count.or_else(|| {
                    run.summary
                        .as_ref()
                        .and_then(|summary| summary.completed_step_count)
                }),
                final_checkpoint_ref: run.final_checkpoint_ref.clone(),
                promotion_checkpoint_ref: run.promotion_checkpoint_ref.clone(),
                accepted_outcome_id: accepted_outcome.map(|outcome| outcome.outcome_id.clone()),
                best_eval_score_bps: run
                    .summary
                    .as_ref()
                    .and_then(|summary| summary.best_eval_score_bps)
                    .or_else(|| {
                        accepted_outcome
                            .and_then(|outcome| outcome.training_summary.as_ref())
                            .and_then(|summary| summary.best_eval_score_bps)
                    }),
                control_plane_state: run_control_plane_state,
                artifact_plane_state: run_artifact_plane_state,
            }
        })
        .collect::<Vec<_>>();

    let participants = cluster
        .members
        .iter()
        .map(|member| {
            let admitted = member.state != "removed";
            let contributing = matches!(member.state.as_str(), "ready" | "active" | "online");
            DesktopControlTrainingParticipantStatus {
                participant_id: member.peer_node_id.clone(),
                visible_reason: if cluster.available {
                    "cluster_member".to_string()
                } else {
                    "cluster_not_integrated".to_string()
                },
                admitted,
                contributing,
                priority_label: if contributing {
                    "selected".to_string()
                } else {
                    "standby".to_string()
                },
                deweight_reason: None,
                exclusion_reason: member.last_error.clone(),
            }
        })
        .collect::<Vec<_>>();

    DesktopControlTrainingStatus {
        available,
        source,
        control_plane_state,
        artifact_plane_state,
        last_synced_at_epoch_ms: cache.last_refreshed_at_epoch_ms,
        run_count: cache.training_runs.len(),
        active_run_count: cache
            .training_runs
            .iter()
            .filter(|run| {
                matches!(
                    run.status,
                    ComputeTrainingRunStatus::Preparing
                        | ComputeTrainingRunStatus::Running
                        | ComputeTrainingRunStatus::Finalizing
                )
            })
            .count(),
        accepted_run_count: cache
            .training_runs
            .iter()
            .filter(|run| {
                run.status == ComputeTrainingRunStatus::Accepted
                    || accepted_training_outcomes.contains_key(run.training_run_id.as_str())
            })
            .count(),
        accepted_outcome_count: cache.accepted_outcomes.len(),
        environment_versions,
        checkpoint_refs,
        contributor_set_revision,
        contributor_reselection_timing,
        admitted_participant_count: participants.iter().filter(|item| item.admitted).count(),
        contributing_participant_count: participants
            .iter()
            .filter(|item| item.contributing)
            .count(),
        stale_rollout_discard_count,
        duplicate_rollout_quarantine_count,
        duplicate_rollout_deweight_count,
        validator_verified_count: cache
            .validator_challenges
            .iter()
            .filter(|challenge| challenge.status == ComputeValidatorChallengeStatus::Verified)
            .count(),
        validator_rejected_count: cache
            .validator_challenges
            .iter()
            .filter(|challenge| challenge.status == ComputeValidatorChallengeStatus::Rejected)
            .count(),
        validator_timed_out_count: cache
            .validator_challenges
            .iter()
            .filter(|challenge| challenge.status == ComputeValidatorChallengeStatus::TimedOut)
            .count(),
        sandbox_ready_profile_count: sandbox.ready_profile_count,
        sandbox_active_job_count: sandbox.active_job_count,
        runs,
        participants,
        operator,
        last_error: cache.last_error.clone(),
    }
}

pub(crate) fn current_training_status(state: &RenderState) -> DesktopControlTrainingStatus {
    desktop_control_training_status(state)
}

fn desktop_control_apple_adapter_operator_status() -> DesktopControlAppleAdapterOperatorStatus {
    match crate::apple_adapter_training_control::operator_status() {
        Ok(status) => {
            let runs = status
                .runs
                .iter()
                .map(map_operator_run_status)
                .collect::<Vec<_>>();
            let active_run_count = status
                .runs
                .iter()
                .filter(|run| operator_run_is_active(run))
                .count();
            let accepted_run_count = status
                .runs
                .iter()
                .filter(|run| run.acceptance_state == AppleAdapterOperatorStageState::Completed)
                .count();
            let exported_run_count = status
                .runs
                .iter()
                .filter(|run| run.export_state == AppleAdapterOperatorStageState::Completed)
                .count();
            let workflow_state = if active_run_count > 0 {
                "running".to_string()
            } else if status.last_error.is_some() {
                "error".to_string()
            } else if accepted_run_count > 0 {
                "accepted_history".to_string()
            } else if status.runs.is_empty() {
                "idle".to_string()
            } else {
                "history".to_string()
            };
            DesktopControlAppleAdapterOperatorStatus {
                available: true,
                workflow_state,
                schema_version: Some(status.schema_version),
                storage_path: Some(status.storage_path),
                last_action: status.last_action,
                last_error: status.last_error,
                run_count: runs.len(),
                active_run_count,
                accepted_run_count,
                exported_run_count,
                runs,
            }
        }
        Err(error) => DesktopControlAppleAdapterOperatorStatus {
            available: false,
            workflow_state: "error".to_string(),
            schema_version: None,
            storage_path: None,
            last_action: None,
            last_error: Some(error),
            run_count: 0,
            active_run_count: 0,
            accepted_run_count: 0,
            exported_run_count: 0,
            runs: Vec::new(),
        },
    }
}

fn map_operator_run_status(
    run: &crate::apple_adapter_training_control::AppleAdapterOperatorRunStatus,
) -> DesktopControlAppleAdapterOperatorRunStatus {
    let local_summary = run.local_summary.as_ref();
    DesktopControlAppleAdapterOperatorRunStatus {
        run_id: run.run_id.clone(),
        package_name: run.package_name.clone(),
        author: run.author.clone(),
        description: run.description.clone(),
        license: run.license.clone(),
        train_dataset_path: run.train_dataset_path.clone(),
        held_out_dataset_path: run.held_out_dataset_path.clone(),
        created_at_epoch_ms: run.created_at_epoch_ms,
        updated_at_epoch_ms: run.updated_at_epoch_ms,
        launched_at_epoch_ms: run.launched_at_epoch_ms,
        evaluated_at_epoch_ms: run.evaluated_at_epoch_ms,
        exported_at_epoch_ms: run.exported_at_epoch_ms,
        accepted_at_epoch_ms: run.accepted_at_epoch_ms,
        launch_state: apple_adapter_operator_stage_label(run.launch_state).to_string(),
        export_state: apple_adapter_operator_stage_label(run.export_state).to_string(),
        evaluation_state: apple_adapter_operator_stage_label(run.evaluation_state).to_string(),
        acceptance_state: apple_adapter_operator_stage_label(run.acceptance_state).to_string(),
        run_directory: run.run_directory.clone(),
        staged_package_path: run.staged_package_path.clone(),
        exported_package_path: run.exported_package_path.clone(),
        completed_step_count: local_summary.map(|summary| summary.completed_steps),
        expected_step_count: local_summary.map(|summary| summary.expected_steps),
        average_loss_label: local_summary
            .and_then(|summary| summary.average_loss)
            .map(|loss| format!("{loss:.6}")),
        held_out_pass_rate_bps: local_summary.and_then(|summary| summary.held_out_pass_rate_bps),
        held_out_average_score_bps: local_summary
            .and_then(|summary| summary.held_out_average_score_bps),
        runtime_smoke_passed: local_summary.and_then(|summary| summary.runtime_smoke_passed),
        runtime_smoke_digest: local_summary
            .and_then(|summary| summary.runtime_smoke_digest.clone()),
        package_digest: local_summary.and_then(|summary| summary.package_digest.clone()),
        adapter_identifier: local_summary.and_then(|summary| summary.adapter_identifier.clone()),
        authority: DesktopControlAppleAdapterOperatorAuthorityStatus {
            core_environment_ref: run.authority_refs.core_environment_ref.clone(),
            benchmark_environment_ref: run.authority_refs.benchmark_environment_ref.clone(),
            benchmark_package_ref: run.authority_refs.benchmark_package_ref.clone(),
            validator_policy_ref: run.authority_refs.validator_policy_ref.clone(),
            training_policy_ref: run.authority_refs.training_policy_ref.clone(),
            training_run_id: run.authority_refs.training_run_id.clone(),
            held_out_eval_run_id: run.authority_refs.held_out_eval_run_id.clone(),
            runtime_validation_eval_run_id: run
                .authority_refs
                .runtime_validation_eval_run_id
                .clone(),
            accepted_outcome_id: run.authority_refs.accepted_outcome_id.clone(),
        },
        last_action: run.last_action.clone(),
        last_error: run.last_error.clone(),
        log_lines: run.log_lines.clone(),
    }
}

fn operator_run_is_active(
    run: &crate::apple_adapter_training_control::AppleAdapterOperatorRunStatus,
) -> bool {
    matches!(
        (
            run.launch_state,
            run.export_state,
            run.evaluation_state,
            run.acceptance_state,
        ),
        (AppleAdapterOperatorStageState::Running, _, _, _)
            | (_, AppleAdapterOperatorStageState::Running, _, _)
            | (_, _, AppleAdapterOperatorStageState::Running, _)
            | (_, _, _, AppleAdapterOperatorStageState::Running)
    )
}

fn apple_adapter_operator_stage_label(value: AppleAdapterOperatorStageState) -> &'static str {
    match value {
        AppleAdapterOperatorStageState::Pending => "pending",
        AppleAdapterOperatorStageState::Running => "running",
        AppleAdapterOperatorStageState::Completed => "completed",
        AppleAdapterOperatorStageState::Failed => "failed",
        AppleAdapterOperatorStageState::Interrupted => "interrupted",
    }
}

fn desktop_control_proof_status(state: &RenderState) -> DesktopControlProofStatus {
    let cache = &state.desktop_control.compute_history;
    let source = authority_history_source_label(state);
    let related_challenges = challenges_by_delivery_proof(
        cache.delivery_proofs.as_slice(),
        cache.validator_challenges.as_slice(),
    );
    let instruments_by_id = cache
        .capacity_instruments
        .iter()
        .map(|instrument| (instrument.instrument_id.as_str(), instrument))
        .collect::<BTreeMap<_, _>>();
    let structured_by_instrument_id =
        structured_instruments_by_leg(cache.structured_capacity_instruments.as_slice());

    let history = cache
        .delivery_proofs
        .iter()
        .take(DESKTOP_CONTROL_COMPUTE_HISTORY_LIMIT)
        .map(|proof| {
            let challenges = related_challenges
                .get(proof.delivery_proof_id.as_str())
                .map(Vec::as_slice)
                .unwrap_or(&[]);
            let instrument = proof
                .instrument_id
                .as_deref()
                .and_then(|instrument_id| instruments_by_id.get(instrument_id).copied());
            let structured = proof
                .instrument_id
                .as_deref()
                .and_then(|instrument_id| structured_by_instrument_id.get(instrument_id))
                .map(Vec::as_slice)
                .unwrap_or(&[]);
            desktop_control_proof_history_status(proof, instrument, structured, challenges)
        })
        .collect::<Vec<_>>();

    let settlements = build_settlement_history(
        cache.delivery_proofs.as_slice(),
        cache.capacity_instruments.as_slice(),
        cache.structured_capacity_instruments.as_slice(),
        &related_challenges,
    );

    DesktopControlProofStatus {
        available: cache.last_refreshed_at_epoch_ms.is_some(),
        source,
        last_synced_at_epoch_ms: cache.last_refreshed_at_epoch_ms,
        pending_count: cache
            .delivery_proofs
            .iter()
            .filter(|proof| proof.status == DeliveryProofStatus::Recorded)
            .count(),
        accepted_count: cache
            .delivery_proofs
            .iter()
            .filter(|proof| proof.status == DeliveryProofStatus::Accepted)
            .count(),
        rejected_count: cache
            .delivery_proofs
            .iter()
            .filter(|proof| proof.status == DeliveryProofStatus::Rejected)
            .count(),
        challenged_count: cache
            .delivery_proofs
            .iter()
            .filter(|proof| {
                related_challenges
                    .get(proof.delivery_proof_id.as_str())
                    .is_some_and(|items| !items.is_empty())
            })
            .count(),
        settlement_open_count: settlements
            .iter()
            .filter(|settlement| !settlement_status_terminal(settlement.status.as_str()))
            .count(),
        settlement_terminal_count: settlements
            .iter()
            .filter(|settlement| settlement_status_terminal(settlement.status.as_str()))
            .count(),
        history,
        settlements,
        last_error: cache.last_error.clone(),
    }
}

fn desktop_control_challenge_status(state: &RenderState) -> DesktopControlChallengeStatus {
    let cache = &state.desktop_control.compute_history;
    let source = authority_history_source_label(state);
    let related_challenges = challenges_by_delivery_proof(
        cache.delivery_proofs.as_slice(),
        cache.validator_challenges.as_slice(),
    );
    let proofs_by_id = cache
        .delivery_proofs
        .iter()
        .map(|proof| (proof.delivery_proof_id.as_str(), proof))
        .collect::<BTreeMap<_, _>>();
    let capacity_instruments = cache
        .capacity_instruments
        .iter()
        .map(|instrument| (instrument.instrument_id.as_str(), instrument))
        .collect::<BTreeMap<_, _>>();
    let structured_by_instrument_id =
        structured_instruments_by_leg(cache.structured_capacity_instruments.as_slice());

    let history = cache
        .validator_challenges
        .iter()
        .take(DESKTOP_CONTROL_COMPUTE_HISTORY_LIMIT)
        .map(|challenge| {
            let delivery_proof_ids =
                delivery_proof_ids_for_challenge(challenge, cache.delivery_proofs.as_slice());
            let settlement_impact_summary = settlement_impact_summary_for_challenge(
                delivery_proof_ids.as_slice(),
                &proofs_by_id,
                &capacity_instruments,
                &structured_by_instrument_id,
                &related_challenges,
            );
            desktop_control_challenge_history_status(
                challenge,
                delivery_proof_ids,
                settlement_impact_summary,
            )
        })
        .collect::<Vec<_>>();

    DesktopControlChallengeStatus {
        available: cache.last_refreshed_at_epoch_ms.is_some(),
        source,
        last_synced_at_epoch_ms: cache.last_refreshed_at_epoch_ms,
        open_count: cache
            .validator_challenges
            .iter()
            .filter(|challenge| {
                matches!(
                    challenge.status,
                    ComputeValidatorChallengeStatus::Queued
                        | ComputeValidatorChallengeStatus::Leased
                        | ComputeValidatorChallengeStatus::Retrying
                )
            })
            .count(),
        queued_count: cache
            .validator_challenges
            .iter()
            .filter(|challenge| challenge.status == ComputeValidatorChallengeStatus::Queued)
            .count(),
        leased_count: cache
            .validator_challenges
            .iter()
            .filter(|challenge| challenge.status == ComputeValidatorChallengeStatus::Leased)
            .count(),
        retrying_count: cache
            .validator_challenges
            .iter()
            .filter(|challenge| challenge.status == ComputeValidatorChallengeStatus::Retrying)
            .count(),
        verified_count: cache
            .validator_challenges
            .iter()
            .filter(|challenge| challenge.status == ComputeValidatorChallengeStatus::Verified)
            .count(),
        rejected_count: cache
            .validator_challenges
            .iter()
            .filter(|challenge| challenge.status == ComputeValidatorChallengeStatus::Rejected)
            .count(),
        timed_out_count: cache
            .validator_challenges
            .iter()
            .filter(|challenge| challenge.status == ComputeValidatorChallengeStatus::TimedOut)
            .count(),
        history,
        last_error: cache.last_error.clone(),
    }
}

fn challenges_by_delivery_proof<'a>(
    proofs: &'a [DeliveryProof],
    challenges: &'a [ComputeValidatorChallengeSnapshot],
) -> BTreeMap<String, Vec<&'a ComputeValidatorChallengeSnapshot>> {
    let proofs_by_bundle_digest = proof_bundle_digests_by_delivery_proof(proofs);
    let mut grouped = BTreeMap::<String, Vec<&ComputeValidatorChallengeSnapshot>>::new();
    for challenge in challenges {
        for proof_id in
            delivery_proof_ids_for_challenge_with_bundle_map(challenge, &proofs_by_bundle_digest)
        {
            grouped.entry(proof_id).or_default().push(challenge);
        }
    }
    grouped
}

fn delivery_proof_ids_for_challenge<'a>(
    challenge: &'a ComputeValidatorChallengeSnapshot,
    proofs: &'a [DeliveryProof],
) -> Vec<String> {
    let proofs_by_bundle_digest = proof_bundle_digests_by_delivery_proof(proofs);
    delivery_proof_ids_for_challenge_with_bundle_map(challenge, &proofs_by_bundle_digest)
}

fn delivery_proof_ids_for_challenge_with_bundle_map(
    challenge: &ComputeValidatorChallengeSnapshot,
    proofs_by_bundle_digest: &BTreeMap<String, Vec<String>>,
) -> Vec<String> {
    if let Some(proof_id) = challenge.request.context.delivery_proof_id.as_deref() {
        return vec![proof_id.to_string()];
    }
    proofs_by_bundle_digest
        .get(challenge.request.context.proof_bundle_digest.as_str())
        .cloned()
        .unwrap_or_default()
}

fn structured_instruments_by_leg<'a>(
    instruments: &'a [StructuredCapacityInstrument],
) -> BTreeMap<&'a str, Vec<&'a StructuredCapacityInstrument>> {
    let mut grouped = BTreeMap::<&str, Vec<&StructuredCapacityInstrument>>::new();
    for instrument in instruments {
        for leg in &instrument.legs {
            grouped
                .entry(leg.instrument_id.as_str())
                .or_default()
                .push(instrument);
        }
    }
    grouped
}

fn desktop_control_proof_history_status(
    proof: &DeliveryProof,
    instrument: Option<&CapacityInstrument>,
    structured: &[&StructuredCapacityInstrument],
    challenges: &[&ComputeValidatorChallengeSnapshot],
) -> DesktopControlProofHistoryStatus {
    let verification = proof.verification_evidence.as_ref();
    let challenge_status = challenges
        .first()
        .map(|challenge| challenge.status.label().to_string());
    let challenge_summary = challenges
        .first()
        .map(|challenge| challenge_summary_line(challenge, challenges.len()));
    let settlement_status = settlement_status_for_proof(instrument, structured);
    let settlement_summary = settlement_summary_for_proof(instrument, structured);
    let environment_binding = proof_environment_binding(proof);
    let metadata = &proof.metadata;
    DesktopControlProofHistoryStatus {
        delivery_proof_id: proof.delivery_proof_id.clone(),
        product_id: proof.product_id.clone(),
        capacity_lot_id: proof.capacity_lot_id.clone(),
        instrument_id: proof.instrument_id.clone(),
        contract_id: proof.contract_id.clone(),
        created_at_epoch_ms: proof.created_at_ms,
        proof_status: proof.status.label().to_string(),
        proof_posture: capability_proof_posture_label(proof).to_string(),
        topology_kind: capability_topology_kind_label(proof).to_string(),
        provisioning_kind: capability_provisioning_kind_label(proof).to_string(),
        environment_ref: verification
            .and_then(|value| value.environment_ref.clone())
            .or_else(|| {
                environment_binding
                    .as_ref()
                    .map(|binding| binding.0.clone())
            }),
        environment_version: verification
            .and_then(|value| value.environment_version.clone())
            .or_else(|| {
                environment_binding
                    .as_ref()
                    .and_then(|binding| binding.1.clone())
            }),
        metered_quantity: proof.metered_quantity,
        accepted_quantity: proof.accepted_quantity,
        acceptance_summary: delivery_acceptance_summary(proof),
        settlement_status,
        settlement_summary,
        challenge_status,
        challenge_summary,
        proof_bundle_ref: verification.and_then(|value| value.proof_bundle_ref.clone()),
        activation_fingerprint_ref: verification
            .and_then(|value| value.activation_fingerprint_ref.clone()),
        validator_pool_ref: verification.and_then(|value| value.validator_pool_ref.clone()),
        validator_run_ref: verification.and_then(|value| value.validator_run_ref.clone()),
        runtime_manifest_ref: metadata_string(
            metadata,
            &[
                "runtime_manifest_ref",
                "runtime_manifest_id",
                "sharded_model_manifest_ref",
            ],
        ),
        runtime_manifest_digest: metadata_string(
            metadata,
            &[
                "runtime_manifest_digest",
                "sharded_model_manifest_digest",
                "model_manifest_digest",
            ],
        ),
        session_claims_ref: metadata_string(
            metadata,
            &["session_claims_ref", "session_claims_bundle_ref"],
        ),
        session_identity_posture: metadata_string(
            metadata,
            &["session_identity_posture", "session_claims_posture"],
        ),
        transport_identity_posture: metadata_string(
            metadata,
            &["transport_identity_posture", "transport_claim_posture"],
        ),
        runtime_config_identity_mode: metadata_string(
            metadata,
            &["runtime_config_identity_mode", "config_identity_mode"],
        ),
        mutable_runtime_variables_present: metadata_bool(
            metadata,
            &[
                "mutable_runtime_variables_present",
                "runtime_variables_mutable",
            ],
        ),
        command_digest: proof
            .sandbox_evidence
            .as_ref()
            .and_then(|value| value.command_digest.clone()),
        environment_digest: proof
            .sandbox_evidence
            .as_ref()
            .and_then(|value| value.environment_digest.clone()),
    }
}

fn desktop_control_challenge_history_status(
    challenge: &ComputeValidatorChallengeSnapshot,
    delivery_proof_ids: Vec<String>,
    settlement_impact_summary: Option<String>,
) -> DesktopControlChallengeHistoryStatus {
    let final_result = challenge.final_result.as_ref();
    DesktopControlChallengeHistoryStatus {
        challenge_id: challenge.request.context.challenge_id.clone(),
        delivery_proof_ids,
        product_id: challenge.request.context.product_id.clone(),
        runtime_backend: challenge.request.context.runtime_backend.clone(),
        model_id: challenge.request.context.model_id.clone(),
        protocol_id: challenge.request.protocol.label().to_string(),
        status: challenge.status.label().to_string(),
        verdict: final_result.map(|result| result.verdict.label().to_string()),
        reason_code: final_result
            .and_then(|result| result.reason_code.map(|code| code.label().to_string())),
        attempts_used: challenge.attempts_used,
        active_attempt: challenge.active_lease.as_ref().map(|lease| lease.attempt),
        validator_id: challenge
            .active_lease
            .as_ref()
            .map(|lease| lease.validator_id.clone()),
        validator_pool_ref: challenge.request.context.validator_pool_ref.clone(),
        proof_bundle_digest: challenge.request.context.proof_bundle_digest.clone(),
        challenge_result_ref: final_result.map(|result| result.challenge_result_ref.clone()),
        created_at_epoch_ms: challenge.request.context.created_at_ms,
        finalized_at_epoch_ms: final_result.map(|result| result.finalized_at_ms),
        verified_row_count: final_result.and_then(|result| result.verified_row_count),
        settlement_impact_summary,
        detail: final_result
            .map(|result| result.detail.clone())
            .unwrap_or_else(|| challenge.status.label().to_string()),
    }
}

fn build_settlement_history(
    proofs: &[DeliveryProof],
    capacity_instruments: &[CapacityInstrument],
    structured_capacity_instruments: &[StructuredCapacityInstrument],
    related_challenges: &BTreeMap<String, Vec<&ComputeValidatorChallengeSnapshot>>,
) -> Vec<DesktopControlSettlementHistoryStatus> {
    let proof_ids_by_instrument_id = proofs_by_instrument_id(proofs);
    let mut settlements = capacity_instruments
        .iter()
        .map(|instrument| {
            let delivery_proof_ids = proof_ids_by_instrument_id
                .get(instrument.instrument_id.as_str())
                .cloned()
                .unwrap_or_default();
            let challenge_ids = challenge_ids_for_delivery_proofs(
                delivery_proof_ids.as_slice(),
                related_challenges,
            );
            DesktopControlSettlementHistoryStatus {
                settlement_id: instrument.instrument_id.clone(),
                settlement_kind: capacity_instrument_kind_label(instrument.kind).to_string(),
                status: instrument.status.label().to_string(),
                product_id: instrument.product_id.clone(),
                delivery_proof_ids,
                challenge_ids,
                settlement_mode: compute_settlement_mode_label(instrument.settlement_mode)
                    .to_string(),
                quantity: instrument.quantity,
                fixed_price_sats: instrument.fixed_price.as_ref().and_then(money_to_sats),
                reference_index_id: instrument.reference_index_id.clone(),
                created_at_epoch_ms: instrument.created_at_ms,
                delivery_window_label: format!(
                    "{}..{}",
                    instrument.delivery_start_ms, instrument.delivery_end_ms
                ),
                reason_code: settlement_reason_code_for_instrument(instrument),
                reason_detail: instrument.lifecycle_reason_detail.clone(),
                outcome_summary: capacity_instrument_summary(instrument),
            }
        })
        .collect::<Vec<_>>();
    settlements.extend(structured_capacity_instruments.iter().map(|instrument| {
        let delivery_proof_ids = instrument
            .legs
            .iter()
            .filter_map(|leg| proof_ids_by_instrument_id.get(leg.instrument_id.as_str()))
            .flat_map(|proof_ids| proof_ids.iter().cloned())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let challenge_ids =
            challenge_ids_for_delivery_proofs(delivery_proof_ids.as_slice(), related_challenges);
        DesktopControlSettlementHistoryStatus {
            settlement_id: instrument.structured_instrument_id.clone(),
            settlement_kind: structured_instrument_kind_label(instrument.kind).to_string(),
            status: instrument.status.label().to_string(),
            product_id: instrument.product_id.clone(),
            delivery_proof_ids,
            challenge_ids,
            settlement_mode: "structured".to_string(),
            quantity: instrument.legs.len() as u64,
            fixed_price_sats: None,
            reference_index_id: None,
            created_at_epoch_ms: instrument.created_at_ms,
            delivery_window_label: "structured".to_string(),
            reason_code: structured_instrument_reason_code(instrument),
            reason_detail: instrument.lifecycle_reason_detail.clone(),
            outcome_summary: structured_instrument_summary(instrument),
        }
    }));
    settlements.sort_by(|left, right| {
        right
            .created_at_epoch_ms
            .cmp(&left.created_at_epoch_ms)
            .then_with(|| left.settlement_id.cmp(&right.settlement_id))
    });
    settlements.truncate(DESKTOP_CONTROL_COMPUTE_HISTORY_LIMIT);
    settlements
}

fn proofs_by_instrument_id(proofs: &[DeliveryProof]) -> BTreeMap<&str, Vec<String>> {
    let mut grouped = BTreeMap::<&str, Vec<String>>::new();
    for proof in proofs {
        let Some(instrument_id) = proof.instrument_id.as_deref() else {
            continue;
        };
        grouped
            .entry(instrument_id)
            .or_default()
            .push(proof.delivery_proof_id.clone());
    }
    grouped
}

fn challenge_ids_for_delivery_proofs(
    delivery_proof_ids: &[String],
    related_challenges: &BTreeMap<String, Vec<&ComputeValidatorChallengeSnapshot>>,
) -> Vec<String> {
    delivery_proof_ids
        .iter()
        .filter_map(|delivery_proof_id| related_challenges.get(delivery_proof_id.as_str()))
        .flat_map(|items| items.iter())
        .map(|challenge| challenge.request.context.challenge_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn settlement_status_for_proof(
    instrument: Option<&CapacityInstrument>,
    structured: &[&StructuredCapacityInstrument],
) -> Option<String> {
    structured
        .first()
        .map(|instrument| instrument.status.label().to_string())
        .or_else(|| instrument.map(|instrument| instrument.status.label().to_string()))
}

fn settlement_summary_for_proof(
    instrument: Option<&CapacityInstrument>,
    structured: &[&StructuredCapacityInstrument],
) -> Option<String> {
    structured
        .first()
        .map(|instrument| structured_instrument_summary(instrument))
        .or_else(|| instrument.map(capacity_instrument_summary))
}

fn settlement_impact_summary_for_challenge(
    delivery_proof_ids: &[String],
    proofs_by_id: &BTreeMap<&str, &DeliveryProof>,
    capacity_instruments: &BTreeMap<&str, &CapacityInstrument>,
    structured_by_instrument_id: &BTreeMap<&str, Vec<&StructuredCapacityInstrument>>,
    related_challenges: &BTreeMap<String, Vec<&ComputeValidatorChallengeSnapshot>>,
) -> Option<String> {
    delivery_proof_ids.iter().find_map(|delivery_proof_id| {
        let proof = proofs_by_id.get(delivery_proof_id.as_str())?;
        let instrument = proof
            .instrument_id
            .as_deref()
            .and_then(|instrument_id| capacity_instruments.get(instrument_id).copied());
        let structured = proof
            .instrument_id
            .as_deref()
            .and_then(|instrument_id| structured_by_instrument_id.get(instrument_id))
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        let challenge_count = related_challenges
            .get(delivery_proof_id.as_str())
            .map_or(0, Vec::len);
        settlement_summary_for_proof(instrument, structured)
            .map(|summary| format!("{summary} // related_challenges={challenge_count}"))
    })
}

fn delivery_acceptance_summary(proof: &DeliveryProof) -> String {
    match proof.status {
        DeliveryProofStatus::Recorded => format!(
            "recorded quantity={} awaiting acceptance",
            proof.metered_quantity
        ),
        DeliveryProofStatus::Accepted => format!(
            "accepted {}/{} delivered",
            proof.accepted_quantity, proof.metered_quantity
        ),
        DeliveryProofStatus::Rejected => {
            let reason = proof
                .rejection_reason
                .map(|reason| reason.label().to_string())
                .unwrap_or_else(|| "unknown".to_string());
            match proof.variance_reason_detail.as_deref() {
                Some(detail) if !detail.trim().is_empty() => {
                    format!("rejected {reason} ({detail})")
                }
                _ => format!("rejected {reason}"),
            }
        }
    }
}

fn challenge_summary_line(
    challenge: &ComputeValidatorChallengeSnapshot,
    related_count: usize,
) -> String {
    let mut parts = vec![format!("status={}", challenge.status.label())];
    if let Some(result) = challenge.final_result.as_ref() {
        parts.push(format!("verdict={}", result.verdict.label()));
        if let Some(reason_code) = result.reason_code {
            parts.push(format!("reason={}", reason_code.label()));
        }
    }
    parts.push(format!("attempts={}", challenge.attempts_used));
    parts.push(format!("related={related_count}"));
    parts.join(" ")
}

fn proof_environment_binding(proof: &DeliveryProof) -> Option<(String, Option<String>)> {
    proof
        .observed_capability_envelope
        .as_ref()
        .or(proof.promised_capability_envelope.as_ref())
        .and_then(|envelope| envelope.environment_binding.as_ref())
        .map(|binding| {
            (
                binding.environment_ref.clone(),
                binding.environment_version.clone(),
            )
        })
}

fn capability_proof_posture_label(proof: &DeliveryProof) -> &'static str {
    proof_capability_envelope(proof)
        .and_then(|envelope| envelope.proof_posture)
        .map_or("unknown", ComputeProofPosture::label)
}

fn capability_topology_kind_label(proof: &DeliveryProof) -> &'static str {
    proof_capability_envelope(proof)
        .and_then(|envelope| envelope.topology_kind)
        .map_or("unknown", ComputeTopologyKind::label)
}

fn capability_provisioning_kind_label(proof: &DeliveryProof) -> &'static str {
    proof_capability_envelope(proof)
        .and_then(|envelope| envelope.provisioning_kind)
        .map_or("unknown", ComputeProvisioningKind::label)
}

fn proof_capability_envelope(proof: &DeliveryProof) -> Option<&ComputeCapabilityEnvelope> {
    proof
        .observed_capability_envelope
        .as_ref()
        .or(proof.promised_capability_envelope.as_ref())
}

fn capacity_instrument_summary(instrument: &CapacityInstrument) -> String {
    let mut parts = vec![
        capacity_instrument_kind_label(instrument.kind).to_string(),
        instrument.status.label().to_string(),
    ];
    if let Some(reason_code) = settlement_reason_code_for_instrument(instrument) {
        parts.push(format!("reason={reason_code}"));
    }
    if let Some(detail) = instrument.lifecycle_reason_detail.as_deref()
        && !detail.trim().is_empty()
    {
        parts.push(format!("detail={detail}"));
    }
    parts.join(" ")
}

fn structured_instrument_summary(instrument: &StructuredCapacityInstrument) -> String {
    let mut parts = vec![
        structured_instrument_kind_label(instrument.kind).to_string(),
        instrument.status.label().to_string(),
    ];
    if let Some(reason_code) = structured_instrument_reason_code(instrument) {
        parts.push(format!("reason={reason_code}"));
    }
    if let Some(detail) = instrument.lifecycle_reason_detail.as_deref()
        && !detail.trim().is_empty()
    {
        parts.push(format!("detail={detail}"));
    }
    parts.join(" ")
}

fn settlement_reason_code_for_instrument(instrument: &CapacityInstrument) -> Option<String> {
    instrument
        .settlement_failure_reason
        .map(|reason| reason.label().to_string())
        .or_else(|| {
            instrument
                .non_delivery_reason
                .map(|reason| reason.label().to_string())
        })
        .or_else(|| {
            instrument
                .closure_reason
                .map(|reason| reason.label().to_string())
        })
}

fn structured_instrument_reason_code(instrument: &StructuredCapacityInstrument) -> Option<String> {
    match instrument.status {
        StructuredCapacityInstrumentStatus::Defaulted => Some("defaulted".to_string()),
        StructuredCapacityInstrumentStatus::Cancelled => Some("cancelled".to_string()),
        StructuredCapacityInstrumentStatus::Expired => Some("expired".to_string()),
        _ => None,
    }
}

fn settlement_status_terminal(status: &str) -> bool {
    matches!(
        status,
        "settled" | "defaulted" | "cancelled" | "expired" | "rejected"
    )
}

fn capacity_instrument_kind_label(kind: CapacityInstrumentKind) -> &'static str {
    match kind {
        CapacityInstrumentKind::Spot => "spot",
        CapacityInstrumentKind::ForwardPhysical => "forward_physical",
        CapacityInstrumentKind::FutureCash => "future_cash",
        CapacityInstrumentKind::Reservation => "reservation",
    }
}

fn structured_instrument_kind_label(kind: StructuredCapacityInstrumentKind) -> &'static str {
    match kind {
        StructuredCapacityInstrumentKind::Reservation => "reservation",
        StructuredCapacityInstrumentKind::Swap => "swap",
        StructuredCapacityInstrumentKind::Strip => "strip",
    }
}

fn compute_settlement_mode_label(
    mode: openagents_kernel_core::compute::ComputeSettlementMode,
) -> &'static str {
    match mode {
        openagents_kernel_core::compute::ComputeSettlementMode::Physical => "physical",
        openagents_kernel_core::compute::ComputeSettlementMode::Cash => "cash",
        openagents_kernel_core::compute::ComputeSettlementMode::BuyerElection => "buyer_election",
    }
}

fn money_to_sats(money: &Money) -> Option<u64> {
    match money.amount {
        MoneyAmount::AmountSats(value) => Some(value),
        MoneyAmount::AmountMsats(value) => Some(value / 1_000),
    }
}

fn metadata_string(metadata: &Value, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| {
        metadata
            .get(*key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
    })
}

fn metadata_bool(metadata: &Value, keys: &[&str]) -> Option<bool> {
    keys.iter()
        .find_map(|key| metadata.get(*key).and_then(Value::as_bool))
}

fn metadata_u64(metadata: &Value, keys: &[&str]) -> Option<u64> {
    keys.iter().find_map(|key| {
        metadata.get(*key).and_then(|value| match value {
            Value::Number(number) => number.as_u64(),
            Value::String(text) => text.trim().parse::<u64>().ok(),
            _ => None,
        })
    })
}

fn procurement_backend_label(
    backend_family: Option<openagents_kernel_core::compute::ComputeBackendFamily>,
    compute_family: openagents_kernel_core::compute::ComputeFamily,
) -> &'static str {
    match backend_family {
        Some(openagents_kernel_core::compute::ComputeBackendFamily::GptOss) => "gpt_oss",
        Some(openagents_kernel_core::compute::ComputeBackendFamily::AppleFoundationModels) => {
            "apple_foundation_models"
        }
        None if matches!(
            compute_family,
            openagents_kernel_core::compute::ComputeFamily::SandboxExecution
        ) =>
        {
            "sandbox"
        }
        None => "unknown",
    }
}

fn procurement_execution_label(
    value: Option<openagents_kernel_core::compute::ComputeExecutionKind>,
) -> &'static str {
    match value {
        Some(openagents_kernel_core::compute::ComputeExecutionKind::LocalInference) => {
            "local_inference"
        }
        Some(openagents_kernel_core::compute::ComputeExecutionKind::ClusteredInference) => {
            "clustered_inference"
        }
        Some(openagents_kernel_core::compute::ComputeExecutionKind::SandboxExecution) => {
            "sandbox_execution"
        }
        Some(openagents_kernel_core::compute::ComputeExecutionKind::EvaluationRun) => {
            "evaluation_run"
        }
        Some(openagents_kernel_core::compute::ComputeExecutionKind::TrainingJob) => "training_job",
        None => "unspecified",
    }
}

fn procurement_topology_label(
    value: Option<openagents_kernel_core::compute::ComputeTopologyKind>,
) -> &'static str {
    match value {
        Some(kind) => kind.label(),
        None => "unspecified",
    }
}

fn procurement_provisioning_label(
    value: Option<openagents_kernel_core::compute::ComputeProvisioningKind>,
) -> &'static str {
    match value {
        Some(kind) => kind.label(),
        None => "unspecified",
    }
}

fn procurement_proof_posture_label(
    value: Option<openagents_kernel_core::compute::ComputeProofPosture>,
) -> &'static str {
    match value {
        Some(kind) => kind.label(),
        None => "unspecified",
    }
}

fn procurement_environment_ref(
    binding: Option<&openagents_kernel_core::compute::ComputeEnvironmentBinding>,
) -> Option<String> {
    binding
        .map(|binding| binding.environment_ref.trim())
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn desktop_control_procurement_spot_quote_status(
    quote: &crate::state::operations::SpotComputeQuoteCandidate,
    selected_quote_id: Option<&str>,
) -> DesktopControlBuyerProcurementQuoteStatus {
    DesktopControlBuyerProcurementQuoteStatus {
        quote_id: quote.quote_id.clone(),
        rfq_id: quote.rfq_id.clone(),
        product_id: quote.product_id.clone(),
        provider_id: quote.provider_id.clone(),
        compute_family: quote.compute_family_label().to_string(),
        backend: procurement_backend_label(quote.backend_family, quote.compute_family).to_string(),
        execution: procurement_execution_label(quote.execution_kind).to_string(),
        topology: procurement_topology_label(quote.topology_kind).to_string(),
        provisioning: procurement_provisioning_label(quote.provisioning_kind).to_string(),
        proof_posture: procurement_proof_posture_label(quote.proof_posture).to_string(),
        requested_quantity: quote.requested_quantity,
        available_quantity: quote.available_quantity,
        price_sats: quote.price_sats,
        delivery_window_label: quote.delivery_window_label.clone(),
        environment_ref: procurement_environment_ref(quote.environment_binding.as_ref()),
        sandbox_profile_ref: quote.sandbox_profile_ref.clone(),
        source_badge: quote.source_badge.clone(),
        terms_label: quote.terms_label.clone(),
        capability_summary: quote.capability_summary.clone(),
        collateral_summary: None,
        remedy_summary: None,
        selected: selected_quote_id == Some(quote.quote_id.as_str()),
    }
}

fn desktop_control_procurement_forward_quote_status(
    quote: &crate::state::operations::ForwardComputeQuoteCandidate,
    selected_quote_id: Option<&str>,
) -> DesktopControlBuyerProcurementQuoteStatus {
    DesktopControlBuyerProcurementQuoteStatus {
        quote_id: quote.quote_id.clone(),
        rfq_id: quote.rfq_id.clone(),
        product_id: quote.product_id.clone(),
        provider_id: quote.provider_id.clone(),
        compute_family: quote.compute_family_label().to_string(),
        backend: procurement_backend_label(quote.backend_family, quote.compute_family).to_string(),
        execution: procurement_execution_label(quote.execution_kind).to_string(),
        topology: procurement_topology_label(quote.topology_kind).to_string(),
        provisioning: procurement_provisioning_label(quote.provisioning_kind).to_string(),
        proof_posture: procurement_proof_posture_label(quote.proof_posture).to_string(),
        requested_quantity: quote.requested_quantity,
        available_quantity: quote.available_quantity,
        price_sats: quote.price_sats,
        delivery_window_label: quote.delivery_window_label.clone(),
        environment_ref: procurement_environment_ref(quote.environment_binding.as_ref()),
        sandbox_profile_ref: quote.sandbox_profile_ref.clone(),
        source_badge: quote.source_badge.clone(),
        terms_label: quote.terms_label.clone(),
        capability_summary: quote.capability_summary.clone(),
        collateral_summary: Some(quote.collateral_summary.clone()),
        remedy_summary: Some(quote.remedy_summary.clone()),
        selected: selected_quote_id == Some(quote.quote_id.as_str()),
    }
}

fn desktop_control_procurement_spot_order_status(
    order: &crate::state::operations::AcceptedSpotComputeOrder,
) -> DesktopControlBuyerProcurementOrderStatus {
    DesktopControlBuyerProcurementOrderStatus {
        order_id: order.order_id.clone(),
        rfq_id: order.rfq_id.clone(),
        quote_id: order.quote_id.clone(),
        instrument_id: order.instrument_id.clone(),
        product_id: order.product_id.clone(),
        provider_id: order.provider_id.clone(),
        compute_family: match order.compute_family {
            openagents_kernel_core::compute::ComputeFamily::Inference => "inference",
            openagents_kernel_core::compute::ComputeFamily::Embeddings => "embeddings",
            openagents_kernel_core::compute::ComputeFamily::SandboxExecution => "sandbox_execution",
            openagents_kernel_core::compute::ComputeFamily::Evaluation => "evaluation",
            openagents_kernel_core::compute::ComputeFamily::Training => "training",
            openagents_kernel_core::compute::ComputeFamily::AdapterHosting => "adapter_hosting",
        }
        .to_string(),
        backend: procurement_backend_label(order.backend_family, order.compute_family).to_string(),
        execution: procurement_execution_label(order.execution_kind).to_string(),
        topology: procurement_topology_label(order.topology_kind).to_string(),
        provisioning: procurement_provisioning_label(order.provisioning_kind).to_string(),
        proof_posture: procurement_proof_posture_label(order.proof_posture).to_string(),
        quantity: order.quantity,
        price_sats: order.price_sats,
        delivery_window_label: order.delivery_window_label.clone(),
        environment_ref: procurement_environment_ref(order.environment_binding.as_ref()),
        sandbox_profile_ref: order.sandbox_profile_ref.clone(),
        collateral_summary: None,
        remedy_summary: None,
        authority_status: order.authority_status.clone(),
        accepted_at_epoch_seconds: order.accepted_at_epoch_seconds,
    }
}

fn desktop_control_procurement_forward_order_status(
    order: &crate::state::operations::AcceptedForwardComputeOrder,
) -> DesktopControlBuyerProcurementOrderStatus {
    DesktopControlBuyerProcurementOrderStatus {
        order_id: order.order_id.clone(),
        rfq_id: order.rfq_id.clone(),
        quote_id: order.quote_id.clone(),
        instrument_id: order.instrument_id.clone(),
        product_id: order.product_id.clone(),
        provider_id: order.provider_id.clone(),
        compute_family: match order.compute_family {
            openagents_kernel_core::compute::ComputeFamily::Inference => "inference",
            openagents_kernel_core::compute::ComputeFamily::Embeddings => "embeddings",
            openagents_kernel_core::compute::ComputeFamily::SandboxExecution => "sandbox_execution",
            openagents_kernel_core::compute::ComputeFamily::Evaluation => "evaluation",
            openagents_kernel_core::compute::ComputeFamily::Training => "training",
            openagents_kernel_core::compute::ComputeFamily::AdapterHosting => "adapter_hosting",
        }
        .to_string(),
        backend: procurement_backend_label(order.backend_family, order.compute_family).to_string(),
        execution: procurement_execution_label(order.execution_kind).to_string(),
        topology: procurement_topology_label(order.topology_kind).to_string(),
        provisioning: procurement_provisioning_label(order.provisioning_kind).to_string(),
        proof_posture: procurement_proof_posture_label(order.proof_posture).to_string(),
        quantity: order.quantity,
        price_sats: order.price_sats,
        delivery_window_label: order.delivery_window_label.clone(),
        environment_ref: procurement_environment_ref(order.environment_binding.as_ref()),
        sandbox_profile_ref: order.sandbox_profile_ref.clone(),
        collateral_summary: Some(order.collateral_summary.clone()),
        remedy_summary: Some(order.remedy_summary.clone()),
        authority_status: order.authority_status.clone(),
        accepted_at_epoch_seconds: order.accepted_at_epoch_seconds,
    }
}

fn desktop_control_buyer_procurement_status(
    requests: &crate::state::operations::NetworkRequestsState,
) -> DesktopControlBuyerProcurementStatus {
    DesktopControlBuyerProcurementStatus {
        load_state: requests.load_state.label().to_string(),
        quote_mode: requests.quote_mode.label().to_string(),
        last_action: requests.last_action.clone(),
        last_error: requests.last_error.clone(),
        last_spot_rfq_summary: requests.last_spot_rfq.as_ref().map(|rfq| rfq.summary()),
        last_forward_rfq_summary: requests.last_forward_rfq.as_ref().map(|rfq| rfq.summary()),
        selected_spot_quote_id: requests.selected_spot_quote_id.clone(),
        selected_forward_quote_id: requests.selected_forward_quote_id.clone(),
        spot_quotes: requests
            .spot_quote_candidates
            .iter()
            .take(8)
            .map(|quote| {
                desktop_control_procurement_spot_quote_status(
                    quote,
                    requests.selected_spot_quote_id.as_deref(),
                )
            })
            .collect(),
        forward_quotes: requests
            .forward_quote_candidates
            .iter()
            .take(8)
            .map(|quote| {
                desktop_control_procurement_forward_quote_status(
                    quote,
                    requests.selected_forward_quote_id.as_deref(),
                )
            })
            .collect(),
        accepted_spot_orders: requests
            .accepted_spot_orders
            .iter()
            .take(8)
            .map(desktop_control_procurement_spot_order_status)
            .collect(),
        accepted_forward_orders: requests
            .accepted_forward_orders
            .iter()
            .take(8)
            .map(desktop_control_procurement_forward_order_status)
            .collect(),
    }
}

pub fn snapshot_for_state(state: &RenderState) -> DesktopControlSnapshot {
    let signature = crate::snapshot_domains::desktop_control_signature(state);
    snapshot_for_state_with_signature(state, signature)
}

fn snapshot_for_state_with_signature(
    state: &RenderState,
    signature: String,
) -> DesktopControlSnapshot {
    let now = Instant::now();
    let now_epoch_seconds = current_epoch_seconds();
    let buy_mode_requests = crate::nip90_compute_flow::buy_mode_request_flow_snapshots(
        &state.network_requests,
        &state.spark_wallet,
    );
    let buy_mode_request = buy_mode_requests
        .iter()
        .find(|request| !request.status.is_terminal());
    let compute_flow = crate::nip90_compute_flow::build_nip90_compute_flow_snapshot(
        &state.network_requests,
        &state.spark_wallet,
        &state.active_job,
        &state.earn_job_lifecycle_projection,
    );
    let wallet_balance_sats = state.spark_wallet.total_balance_sats();
    let wallet_connected = state.spark_wallet.network_status_label() == "connected";
    let (wallet_can_withdraw, withdraw_block_reason) =
        withdraw_readiness(wallet_balance_sats, wallet_connected);
    let blocker_codes = state
        .provider_blockers()
        .into_iter()
        .map(|blocker| blocker.code().to_string())
        .collect::<Vec<_>>();
    let buy_mode_target_selection = state
        .autopilot_chat
        .select_autopilot_buy_mode_target(now_epoch_seconds);
    let buy_mode_peer_roster = state
        .autopilot_chat
        .autopilot_peer_roster(now_epoch_seconds)
        .into_iter()
        .map(desktop_control_autopilot_peer_status)
        .collect::<Vec<_>>();
    let nip28 = desktop_control_nip28_status(&state.autopilot_chat);
    let recent_request_rows = buy_mode_requests
        .iter()
        .take(6)
        .map(desktop_control_buy_mode_request_status)
        .collect::<Vec<_>>();
    let local_runtime = desktop_control_local_runtime_status(state);
    let inventory = crate::provider_inventory::inventory_status_for_state(state);
    let buyer_procurement = desktop_control_buyer_procurement_status(&state.network_requests);
    let gpt_oss = desktop_control_gpt_oss_status(state);

    let mut snapshot = DesktopControlSnapshot {
        schema_version: DESKTOP_CONTROL_SCHEMA_VERSION,
        snapshot_revision: 0,
        state_signature: String::new(),
        generated_at_epoch_ms: current_epoch_ms(),
        session: DesktopControlSessionStatus {
            pid: std::process::id(),
            shell_mode: "hotbar".to_string(),
            dev_mode_enabled: false,
            buy_mode_surface_enabled: state.mission_control_buy_mode_enabled(),
        },
        mission_control: DesktopControlMissionControlStatus {
            last_action: state.mission_control.last_action.clone(),
            last_error: state.mission_control.last_error.clone(),
            can_go_online: state.mission_control_go_online_enabled(),
            blocker_codes: blocker_codes.clone(),
            log_line_count: state.log_stream.terminal.recent_lines(usize::MAX).len(),
        },
        provider: DesktopControlProviderStatus {
            mode: state.provider_nip90_lane.mode.label().to_string(),
            runtime_mode: state.provider_runtime.mode.label().to_string(),
            desired_mode_hint: provider_desired_mode_hint(state).to_string(),
            online: matches!(
                state.provider_nip90_lane.mode,
                crate::provider_nip90_lane::ProviderNip90LaneMode::Online
                    | crate::provider_nip90_lane::ProviderNip90LaneMode::Degraded
            ),
            blocker_codes,
            connected_relays: state.provider_nip90_lane.connected_relays,
            degraded_reason_code: state.provider_runtime.degraded_reason_code.clone(),
            last_request_event_id: state.provider_nip90_lane.last_request_event_id.clone(),
            last_action: state.provider_runtime.last_result.clone(),
            last_error: state.provider_runtime.last_error_detail.clone(),
            relay_urls: state.configured_provider_relay_urls(),
        },
        local_runtime,
        gpt_oss,
        apple_fm: DesktopControlAppleFmStatus {
            reachable: state.provider_runtime.apple_fm.reachable,
            ready: state.provider_runtime.apple_fm.is_ready(),
            model_available: state.provider_runtime.apple_fm.model_available,
            ready_model: state.provider_runtime.apple_fm.ready_model.clone(),
            adapter_inventory_supported: state
                .provider_runtime
                .apple_fm
                .adapter_inventory_supported,
            adapter_attach_supported: state.provider_runtime.apple_fm.adapter_attach_supported,
            loaded_adapters: state.provider_runtime.apple_fm.loaded_adapters.clone(),
            active_session_id: state.apple_fm_workbench.active_session_id.clone(),
            active_session_adapter: state.apple_fm_workbench.active_session_adapter.clone(),
            bridge_status: state.provider_runtime.apple_fm.bridge_status.clone(),
            last_action: state.provider_runtime.apple_fm.last_action.clone(),
            last_error: state.provider_runtime.apple_fm.last_error.clone(),
        },
        wallet: DesktopControlWalletStatus {
            balance_sats: wallet_balance_sats.unwrap_or(0),
            balance_known: wallet_balance_sats.is_some(),
            balance_reconciling: state.spark_wallet.balance_reconciling(),
            network: state.spark_wallet.network_name().to_string(),
            network_status: state.spark_wallet.network_status_label().to_string(),
            can_withdraw: wallet_can_withdraw,
            withdraw_block_reason,
            last_action: state.spark_wallet.last_action.clone(),
            last_error: state.spark_wallet.last_error.clone(),
        },
        tunnels: DesktopControlTunnelsStatus::default(),
        inventory,
        buyer_procurement,
        cluster: desktop_control_cluster_status(),
        sandbox: desktop_control_sandbox_status(state),
        training: desktop_control_training_status(state),
        proofs: desktop_control_proof_status(state),
        challenges: desktop_control_challenge_status(state),
        buy_mode: DesktopControlBuyModeStatus {
            enabled: state.buy_mode_payments.buy_mode_loop_enabled,
            approved_budget_sats: MISSION_CONTROL_BUY_MODE_BUDGET_SATS,
            cadence_seconds: MISSION_CONTROL_BUY_MODE_INTERVAL_MILLIS.saturating_add(999) / 1_000,
            cadence_millis: MISSION_CONTROL_BUY_MODE_INTERVAL_MILLIS,
            next_dispatch_countdown_seconds: state
                .buy_mode_payments
                .buy_mode_next_dispatch_countdown_seconds(now),
            next_dispatch_countdown_millis: state
                .buy_mode_payments
                .buy_mode_next_dispatch_countdown_millis(now),
            in_flight_request_id: buy_mode_request
                .as_ref()
                .map(|request| request.request_id.clone()),
            in_flight_phase: buy_mode_request
                .as_ref()
                .map(|request| request.phase.as_str().to_string()),
            in_flight_status: buy_mode_request
                .as_ref()
                .map(|request| request.status.label().to_string()),
            selected_provider_pubkey: buy_mode_request
                .as_ref()
                .and_then(|request| request.selected_provider_pubkey.clone()),
            result_provider_pubkey: buy_mode_request
                .as_ref()
                .and_then(|request| request.result_provider_pubkey.clone()),
            invoice_provider_pubkey: buy_mode_request
                .as_ref()
                .and_then(|request| request.invoice_provider_pubkey.clone()),
            payable_provider_pubkey: buy_mode_request
                .as_ref()
                .and_then(|request| request.payable_provider_pubkey.clone()),
            payment_blocker_codes: buy_mode_request
                .as_ref()
                .map(|request| request.payment_blocker_codes.clone())
                .unwrap_or_default(),
            payment_blocker_summary: buy_mode_request
                .as_ref()
                .and_then(|request| request.payment_blocker_summary.clone()),
            target_selection: DesktopControlBuyModeTargetSelectionStatus {
                selected_peer_pubkey: buy_mode_target_selection.selected_peer_pubkey,
                selected_relay_url: buy_mode_target_selection.selected_relay_url,
                selected_ready_model: buy_mode_target_selection.selected_ready_model,
                observed_peer_count: buy_mode_target_selection.observed_peer_count,
                eligible_peer_count: buy_mode_target_selection.eligible_peer_count,
                blocked_reason_code: buy_mode_target_selection.blocked_reason_code,
                blocked_reason: buy_mode_target_selection.blocked_reason,
            },
            peer_roster: buy_mode_peer_roster,
            recent_requests: recent_request_rows,
        },
        active_job: compute_flow.active_job.map(|active_job| {
            let stage = active_job_stage_label(&active_job).to_string();
            let projection_stage = active_job.stage.label().to_string();
            let phase = active_job.phase.as_str().to_string();
            DesktopControlActiveJobStatus {
                job_id: active_job.job_id,
                request_id: active_job.request_id,
                capability: active_job.capability,
                stage,
                projection_stage,
                phase,
                next_expected_event: active_job.next_expected_event,
                projection_authority: active_job.projection_authority,
                quoted_price_sats: active_job.quoted_price_sats,
                pending_result_publish_event_id: active_job.pending_result_publish_event_id,
                result_event_id: active_job.result_event_id,
                result_publish_status: active_job.result_publish_status,
                result_publish_attempt_count: active_job.result_publish_attempt_count,
                result_publish_age_seconds: active_job.result_publish_age_seconds,
                payment_pointer: active_job.payment_pointer,
                pending_bolt11: active_job.pending_bolt11,
                settlement_status: active_job.settlement_status,
                settlement_method: active_job.settlement_method,
                settlement_amount_sats: active_job.settlement_amount_sats,
                settlement_fees_sats: active_job.settlement_fees_sats,
                settlement_net_wallet_delta_sats: active_job.settlement_net_wallet_delta_sats,
                continuity_window_seconds: active_job.continuity_window_seconds,
                failure_reason: active_job.failure_reason,
            }
        }),
        nip28,
        recent_logs: mission_control_recent_lines(state, DESKTOP_CONTROL_LOG_TAIL_LIMIT),
        last_command: state
            .desktop_control
            .last_command_completed_at_epoch_ms
            .zip(state.desktop_control.last_command_summary.clone())
            .map(
                |(completed_at_epoch_ms, summary)| DesktopControlLastCommandStatus {
                    summary,
                    error: state.desktop_control.last_command_error.clone(),
                    completed_at_epoch_ms,
                    snapshot_revision: 0,
                    state_signature: String::new(),
                },
            ),
    };

    let next_revision =
        if state.desktop_control.last_snapshot_signature.as_deref() == Some(signature.as_str()) {
            state.desktop_control.last_snapshot_revision
        } else {
            state
                .desktop_control
                .last_snapshot_revision
                .saturating_add(1)
                .max(1)
        };
    snapshot.snapshot_revision = next_revision;
    snapshot.state_signature = signature.clone();
    if let Some(last_command) = snapshot.last_command.as_mut() {
        last_command.snapshot_revision = next_revision;
        last_command.state_signature = signature;
    }
    snapshot
}

fn desktop_control_nip28_status(
    chat: &crate::app_state::AutopilotChatState,
) -> DesktopControlNip28Status {
    let config = DefaultNip28ChannelConfig::from_env_or_default();
    let browse_mode = match chat.chat_browse_mode() {
        crate::app_state::ChatBrowseMode::Autopilot => "autopilot",
        crate::app_state::ChatBrowseMode::Managed => "managed",
        crate::app_state::ChatBrowseMode::DirectMessages => "direct_messages",
    }
    .to_string();
    let active_group = chat.active_managed_chat_group();
    let active_channel = chat.active_managed_chat_channel();
    let groups = chat
        .managed_chat_projection
        .snapshot
        .groups
        .iter()
        .map(|group| DesktopControlNip28GroupStatus {
            group_id: group.group_id.clone(),
            name: group_name_label(group),
            selected: active_group.is_some_and(|active| active.group_id == group.group_id),
            unread_count: group.unread_count,
            mention_count: group.mention_count,
            channel_count: group.channel_ids.len(),
        })
        .collect::<Vec<_>>();
    let channels = active_group
        .map(|_| {
            chat.active_managed_chat_channels()
                .into_iter()
                .map(|channel| DesktopControlNip28ChannelStatus {
                    channel_id: channel.channel_id.clone(),
                    group_id: channel.group_id.clone(),
                    name: channel_name_label(channel),
                    relay_url: channel.relay_url.clone(),
                    selected: active_channel
                        .is_some_and(|active| active.channel_id == channel.channel_id),
                    unread_count: channel.unread_count,
                    mention_count: channel.mention_count,
                    message_count: channel.message_ids.len(),
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let recent_messages = chat
        .active_managed_chat_message_tail(16)
        .into_iter()
        .map(|message| DesktopControlNip28MessageStatus {
            event_id: message.event_id.clone(),
            author_pubkey: message.author_pubkey.clone(),
            content: message.content.clone(),
            created_at: message.created_at,
            reply_to_event_id: message.reply_to_event_id.clone(),
            delivery_state: match message.delivery_state {
                crate::app_state::ManagedChatDeliveryState::Confirmed => "confirmed",
                crate::app_state::ManagedChatDeliveryState::Publishing => "publishing",
                crate::app_state::ManagedChatDeliveryState::Acked => "acked",
                crate::app_state::ManagedChatDeliveryState::Failed => "failed",
            }
            .to_string(),
            delivery_error: message.delivery_error.clone(),
            attempt_count: message.attempt_count,
        })
        .collect::<Vec<_>>();
    let publishing_outbound_count = chat
        .managed_chat_projection
        .outbound_messages
        .iter()
        .filter(|message| {
            message.delivery_state == crate::app_state::ManagedChatDeliveryState::Publishing
        })
        .count();
    let retryable_event_id = active_channel.and_then(|channel| {
        chat.managed_chat_projection
            .latest_retryable_outbound_event_id(channel.channel_id.as_str())
    });

    DesktopControlNip28Status {
        available: chat.has_managed_chat_browseable_content(),
        browse_mode,
        configured_relay_url: config.relay_url.clone(),
        configured_channel_id: config.channel_id.clone(),
        configured_channel_loaded: chat
            .managed_chat_projection
            .snapshot
            .channels
            .iter()
            .any(|channel| channel.channel_id == config.channel_id),
        local_pubkey: chat.managed_chat_local_pubkey().map(str::to_string),
        selected_group_id: active_group.map(|group| group.group_id.clone()),
        selected_group_name: active_group.map(group_name_label),
        selected_channel_id: active_channel.map(|channel| channel.channel_id.clone()),
        selected_channel_name: active_channel.map(channel_name_label),
        selected_channel_relay_url: active_channel.and_then(|channel| channel.relay_url.clone()),
        publishing_outbound_count,
        retryable_event_id,
        last_action: chat.managed_chat_projection.last_action.clone(),
        last_error: chat
            .last_error
            .clone()
            .or_else(|| chat.managed_chat_projection.last_error.clone()),
        groups,
        channels,
        recent_messages,
    }
}

fn group_name_label(group: &crate::app_state::ManagedChatGroupProjection) -> String {
    group
        .metadata
        .name
        .clone()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| group.group_id.clone())
}

fn channel_name_label(channel: &crate::app_state::ManagedChatChannelProjection) -> String {
    let name = channel.metadata.name.trim();
    if name.is_empty() {
        channel.channel_id.clone()
    } else {
        channel.metadata.name.clone()
    }
}

fn provider_desired_mode_hint(state: &RenderState) -> &'static str {
    match state.provider_runtime.mode {
        crate::state::provider_runtime::ProviderMode::Offline => "offline",
        crate::state::provider_runtime::ProviderMode::Connecting
        | crate::state::provider_runtime::ProviderMode::Online
        | crate::state::provider_runtime::ProviderMode::Degraded => "online",
    }
}

fn withdraw_readiness(balance_sats: Option<u64>, wallet_connected: bool) -> (bool, Option<String>) {
    if !wallet_connected {
        return (false, Some("wallet is not connected to Spark".to_string()));
    }
    let Some(balance_sats) = balance_sats else {
        return (
            false,
            Some("wallet balance is still reconciling".to_string()),
        );
    };
    if balance_sats == 0 {
        return (false, Some("wallet balance is zero".to_string()));
    }
    (true, None)
}

fn desktop_control_buy_mode_request_status(
    request: &crate::nip90_compute_flow::BuyerRequestFlowSnapshot,
) -> DesktopControlBuyModeRequestStatus {
    DesktopControlBuyModeRequestStatus {
        request_id: request.request_id.clone(),
        phase: request.phase.as_str().to_string(),
        status: request.status.label().to_string(),
        next_expected_event: request.next_expected_event.clone(),
        request_event_id: request.published_request_event_id.clone(),
        selected_provider_pubkey: request.selected_provider_pubkey.clone(),
        result_provider_pubkey: request.result_provider_pubkey.clone(),
        invoice_provider_pubkey: request.invoice_provider_pubkey.clone(),
        payable_provider_pubkey: request.payable_provider_pubkey.clone(),
        last_feedback_status: request.last_feedback_status.clone(),
        last_feedback_event_id: request.last_feedback_event_id.clone(),
        last_result_event_id: request.last_result_event_id.clone(),
        winning_result_event_id: request.winning_result_event_id.clone(),
        payment_pointer: request.payment_pointer.clone(),
        pending_bolt11: request.pending_bolt11.clone(),
        payment_blocker_codes: request.payment_blocker_codes.clone(),
        payment_blocker_summary: request.payment_blocker_summary.clone(),
        payment_notice: request.payment_notice.clone(),
        payment_error: request.payment_error.clone(),
        wallet_status: request.wallet_status.clone(),
    }
}

fn desktop_control_autopilot_peer_status(
    row: crate::autopilot_peer_roster::AutopilotPeerRosterRow,
) -> DesktopControlAutopilotPeerStatus {
    DesktopControlAutopilotPeerStatus {
        pubkey: row.pubkey,
        relay_url: row.source_relay_url,
        ready_model: row.ready_model,
        online_for_compute: row.online_for_compute,
        eligible_for_buy_mode: row.eligible_for_buy_mode,
        eligibility_reason: row.eligibility_reason,
        last_chat_message_at: row.last_chat_message_at,
        last_presence_at: row.last_presence_at,
        presence_expires_at: row.presence_expires_at,
    }
}

fn active_job_stage_label(
    active_job: &crate::nip90_compute_flow::ActiveJobFlowSnapshot,
) -> &'static str {
    match active_job.phase {
        crate::nip90_compute_flow::Nip90FlowPhase::RequestingPayment
        | crate::nip90_compute_flow::Nip90FlowPhase::AwaitingPayment
        | crate::nip90_compute_flow::Nip90FlowPhase::SellerSettledPendingWallet => "settling",
        crate::nip90_compute_flow::Nip90FlowPhase::DeliveredUnpaid => "unpaid",
        _ => active_job.stage.label(),
    }
}

fn mission_control_recent_lines(state: &RenderState, limit: usize) -> Vec<String> {
    state
        .log_stream
        .terminal
        .recent_lines(limit.max(1))
        .into_iter()
        .map(|line| line.text.clone())
        .collect()
}

fn append_runtime_events(
    state: &DesktopControlHttpState,
    drafts: Vec<DesktopControlEventDraft>,
) -> Result<Vec<DesktopControlEvent>, StatusCode> {
    let appended = {
        let mut buffer = state
            .events
            .lock()
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        buffer.append(drafts)
    };
    if appended.is_empty() {
        return Ok(appended);
    }
    for event in &appended {
        persist_control_event(event);
    }
    state.event_notify.notify_waiters();
    Ok(appended)
}

fn persist_control_event(event: &DesktopControlEvent) {
    crate::runtime_log::record_control_event(
        event.event_type.as_str(),
        event.summary.clone(),
        json!({
            "event_id": event.event_id,
            "at_epoch_ms": event.at_epoch_ms,
            "command_label": event.command_label,
            "success": event.success,
            "snapshot_revision": event.snapshot_revision,
            "state_signature": event.state_signature,
            "payload": event.payload,
        }),
    );
}

fn runtime_event_batch(
    state: &DesktopControlHttpState,
    after_event_id: u64,
    limit: usize,
    timed_out: bool,
) -> Result<DesktopControlEventBatch, StatusCode> {
    let buffer = state
        .events
        .lock()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(DesktopControlEventBatch {
        last_event_id: buffer.last_event_id(),
        timed_out,
        events: buffer.collect_after(after_event_id, limit),
    })
}

fn current_epoch_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(u64::MAX as u128) as u64)
        .unwrap_or(0)
}

fn current_epoch_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

fn epoch_seconds_rfc3339(epoch_seconds: u64) -> String {
    Utc.timestamp_opt(epoch_seconds as i64, 0)
        .single()
        .map(|timestamp| timestamp.to_rfc3339())
        .unwrap_or_else(|| "1970-01-01T00:00:00+00:00".to_string())
}

fn write_control_manifest(
    path: &std::path::Path,
    manifest: &DesktopControlManifest,
) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "Failed to create desktop control manifest dir {}: {error}",
                parent.display()
            )
        })?;
    }
    let payload = serde_json::to_string_pretty(manifest)
        .map_err(|error| format!("Failed to encode desktop control manifest: {error}"))?;
    fs::write(path, payload).map_err(|error| {
        format!(
            "Failed to write desktop control manifest {}: {error}",
            path.display()
        )
    })
}

fn auth_token_preview(auth_token: &str) -> String {
    let trimmed = auth_token.trim();
    if trimmed.len() <= 10 {
        trimmed.to_string()
    } else {
        format!(
            "{}...{}",
            &trimmed[..4],
            &trimmed[trimmed.len().saturating_sub(4)..]
        )
    }
}

async fn desktop_control_snapshot(
    State(state): State<DesktopControlHttpState>,
    headers: HeaderMap,
) -> Result<Json<DesktopControlSnapshot>, StatusCode> {
    authorize_request(&headers, &state)?;
    let snapshot = state
        .snapshot
        .lock()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .clone();
    Ok(Json(snapshot))
}

async fn desktop_control_events(
    State(state): State<DesktopControlHttpState>,
    headers: HeaderMap,
    Query(query): Query<DesktopControlEventsQuery>,
) -> Result<Json<DesktopControlEventBatch>, StatusCode> {
    authorize_request(&headers, &state)?;
    let limit = query
        .limit
        .unwrap_or(DESKTOP_CONTROL_EVENT_QUERY_LIMIT)
        .max(1)
        .min(DESKTOP_CONTROL_EVENT_QUERY_LIMIT);
    let timeout_ms = query
        .timeout_ms
        .unwrap_or(DESKTOP_CONTROL_EVENT_WAIT_TIMEOUT_MS)
        .min(DESKTOP_CONTROL_EVENT_WAIT_TIMEOUT_MS);
    let notified = state.event_notify.notified();
    let immediate = runtime_event_batch(&state, query.after_event_id, limit, false)?;
    if !immediate.events.is_empty() || timeout_ms == 0 {
        return Ok(Json(immediate));
    }

    let notified = tokio::time::timeout(Duration::from_millis(timeout_ms), notified).await;
    match notified {
        Ok(()) => {
            let batch = runtime_event_batch(&state, query.after_event_id, limit, false)?;
            crate::runtime_log::record_control_event(
                "control.wait.satisfied",
                format!(
                    "event wait satisfied after={} returned={}",
                    query.after_event_id,
                    batch.events.len()
                ),
                json!({
                    "after_event_id": query.after_event_id,
                    "timeout_ms": timeout_ms,
                    "returned_event_count": batch.events.len(),
                    "last_event_id": batch.last_event_id,
                }),
            );
            Ok(Json(batch))
        }
        Err(_) => {
            crate::runtime_log::record_control_event(
                "control.wait.timed_out",
                format!("event wait timed out after={}", query.after_event_id),
                json!({
                    "after_event_id": query.after_event_id,
                    "timeout_ms": timeout_ms,
                }),
            );
            Ok(Json(runtime_event_batch(
                &state,
                query.after_event_id,
                limit,
                true,
            )?))
        }
    }
}

async fn desktop_control_action(
    State(state): State<DesktopControlHttpState>,
    headers: HeaderMap,
    Json(action): Json<DesktopControlActionRequest>,
) -> (StatusCode, Json<DesktopControlActionResponse>) {
    if let Err(status) = authorize_request(&headers, &state) {
        return (
            status,
            Json(DesktopControlActionResponse::error(
                "Unauthorized desktop control request",
            )),
        );
    }
    let (response_tx, response_rx) = oneshot::channel();
    let action_for_response = action.clone();
    let envelope = DesktopControlActionEnvelope {
        action,
        response_tx,
    };
    if state
        .update_tx
        .send(DesktopControlRuntimeUpdate::ActionRequest(envelope))
        .is_err()
    {
        let response = DesktopControlActionResponse::error("Desktop control loop is unavailable");
        let _ = append_runtime_events(
            &state,
            vec![command_outcome_event(&action_for_response, &response)],
        );
        return (StatusCode::SERVICE_UNAVAILABLE, Json(response));
    }
    match tokio::time::timeout(Duration::from_secs(3), response_rx).await {
        Ok(Ok(response)) => (StatusCode::OK, Json(response)),
        Ok(Err(_)) => {
            let response =
                DesktopControlActionResponse::error("Desktop dropped the control action response");
            let _ = append_runtime_events(
                &state,
                vec![command_outcome_event(&action_for_response, &response)],
            );
            (StatusCode::SERVICE_UNAVAILABLE, Json(response))
        }
        Err(_) => {
            let response = DesktopControlActionResponse::error("Desktop control action timed out");
            let _ = append_runtime_events(
                &state,
                vec![command_outcome_event(&action_for_response, &response)],
            );
            (StatusCode::REQUEST_TIMEOUT, Json(response))
        }
    }
}

fn authorize_request(
    headers: &HeaderMap,
    state: &DesktopControlHttpState,
) -> Result<(), StatusCode> {
    let Some(token) = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .map(str::trim)
    else {
        return Err(StatusCode::UNAUTHORIZED);
    };
    let expected = state
        .auth_token
        .lock()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if token == expected.as_str() {
        Ok(())
    } else {
        Err(StatusCode::UNAUTHORIZED)
    }
}

fn run_desktop_control_runtime_loop(
    mut command_rx: tokio_mpsc::UnboundedReceiver<DesktopControlRuntimeCommand>,
    update_tx: Sender<DesktopControlRuntimeUpdate>,
    ready_tx: Sender<Result<SocketAddr, String>>,
    config: DesktopControlRuntimeConfig,
) {
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            let _ = ready_tx.send(Err(format!(
                "Failed to build desktop control runtime: {error}"
            )));
            return;
        }
    };

    runtime.block_on(async move {
        let snapshot = Arc::new(Mutex::new(DesktopControlSnapshot::default()));
        let events = Arc::new(Mutex::new(DesktopControlEventBuffer::default()));
        let event_notify = Arc::new(Notify::new());
        let auth_token = Arc::new(Mutex::new(config.auth_token));
        let state = DesktopControlHttpState {
            snapshot,
            events,
            event_notify,
            auth_token,
            update_tx,
        };
        let listener = match tokio::net::TcpListener::bind(config.listen_addr).await {
            Ok(listener) => listener,
            Err(error) => {
                let _ = ready_tx.send(Err(format!(
                    "Failed to bind desktop control listener: {error}"
                )));
                return;
            }
        };
        let listen_addr = match listener.local_addr() {
            Ok(addr) => addr,
            Err(error) => {
                let _ = ready_tx.send(Err(format!(
                    "Failed to resolve desktop control listener address: {error}"
                )));
                return;
            }
        };
        let router = Router::new()
            .route("/v1/snapshot", get(desktop_control_snapshot))
            .route("/v1/events", get(desktop_control_events))
            .route("/v1/action", post(desktop_control_action))
            .with_state(state.clone());
        let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
        let server_state = state.clone();
        let server = tokio::spawn(async move {
            if let Err(error) = axum::serve(listener, router)
                .with_graceful_shutdown(async {
                    let _ = shutdown_rx.await;
                })
                .await
            {
                let _ = server_state
                    .update_tx
                    .send(DesktopControlRuntimeUpdate::WorkerError(format!(
                        "Desktop control listener failed: {error}"
                    )));
            }
        });
        let _ = ready_tx.send(Ok(listen_addr));
        while let Some(command) = command_rx.recv().await {
            match command {
                DesktopControlRuntimeCommand::SyncSnapshot(next_snapshot) => {
                    if let Ok(mut guard) = state.snapshot.lock() {
                        *guard = *next_snapshot;
                    }
                }
                DesktopControlRuntimeCommand::AppendEvents(events) => {
                    let _ = append_runtime_events(&state, events);
                }
                DesktopControlRuntimeCommand::Shutdown => break,
            }
        }
        let _ = shutdown_tx.send(());
        let _ = server.await;
    });
}

#[cfg(test)]
mod tests {
    use super::{
        DESKTOP_CONTROL_SCHEMA_VERSION, DesktopControlActionRequest, DesktopControlActionResponse,
        DesktopControlAppleAdapterOperatorStatus, DesktopControlAppleFmStatus,
        DesktopControlBuyModeStatus, DesktopControlBuyModeTargetSelectionStatus,
        DesktopControlBuyerProcurementOrderStatus, DesktopControlBuyerProcurementQuoteStatus,
        DesktopControlBuyerProcurementStatus, DesktopControlChallengeStatus,
        DesktopControlClusterStatus, DesktopControlEventBatch, DesktopControlEventDraft,
        DesktopControlGptOssStatus, DesktopControlInventoryProjectionStatus,
        DesktopControlInventorySectionStatus, DesktopControlInventoryStatus,
        DesktopControlLocalRuntimeStatus, DesktopControlMissionControlStatus,
        DesktopControlNip90SentPaymentsReport, DesktopControlProofStatus,
        DesktopControlProviderStatus, DesktopControlRuntime, DesktopControlRuntimeConfig,
        DesktopControlRuntimeUpdate, DesktopControlSandboxStatus, DesktopControlSessionStatus,
        DesktopControlSnapshot, DesktopControlTrainingParticipantStatus,
        DesktopControlTrainingRunStatus, DesktopControlTrainingStatus,
        DesktopControlTunnelServiceStatus, DesktopControlTunnelsStatus, DesktopControlWalletStatus,
        LocalRuntimeDiagnostics, apply_response_snapshot_metadata,
        build_nip90_sent_payments_report_payload, build_settlement_history,
        challenges_by_delivery_proof, command_outcome_event, command_received_event,
        desktop_control_challenge_history_status, desktop_control_proof_history_status,
        snapshot_change_events, snapshot_sync_signature, validate_control_bind_addr,
    };
    use crate::app_state::{
        AutopilotChatState, DefaultNip28ChannelConfig, ManagedChatDeliveryState,
        ManagedChatProjectionState, NetworkRequestSubmission,
    };
    use crate::autopilot_compute_presence::pump_provider_chat_presence;
    use crate::nip28_chat_lane::{Nip28ChatLaneUpdate, Nip28ChatLaneWorker};
    use crate::provider_nip90_lane::{
        ProviderNip90AuthIdentity, ProviderNip90ComputeCapability, ProviderNip90LaneCommand,
        ProviderNip90LaneUpdate, ProviderNip90LaneWorker, ProviderNip90PublishOutcome,
        ProviderNip90PublishRole,
    };
    use crate::spark_wallet::SparkPaneState;
    use crate::state::nip90_buyer_payment_attempts::Nip90BuyerPaymentWindowReport;
    use crate::state::operations::{BuyerResolutionMode, NetworkRequestStatus};
    use crate::state::provider_runtime::{ProviderMode, ProviderRuntimeState};
    use futures_util::{SinkExt, StreamExt};
    use nostr::nip90::{
        JobFeedback, JobResult, JobStatus, create_job_feedback_event, create_job_result_event,
    };
    use nostr::{
        ChannelMetadata, Event, EventTemplate, GroupMetadata, GroupMetadataEvent,
        ManagedChannelCreateEvent, ManagedChannelHints, ManagedChannelMessageEvent,
        ManagedChannelType, NostrIdentity,
    };
    use openagents_kernel_core::compute::{
        CapacityInstrument, CapacityInstrumentKind, CapacityInstrumentStatus,
        ComputeCapabilityEnvelope, ComputeProofPosture, ComputeProvisioningKind,
        ComputeTopologyKind, ComputeValidatorChallengeContext,
        ComputeValidatorChallengeProtocolKind, ComputeValidatorChallengeRequest,
        ComputeValidatorChallengeResult, ComputeValidatorChallengeSnapshot,
        ComputeValidatorChallengeStatus, ComputeValidatorChallengeVerdict, DeliveryProof,
        DeliveryProofStatus, DeliveryVerificationEvidence, StructuredCapacityInstrument,
        StructuredCapacityInstrumentKind, StructuredCapacityInstrumentStatus,
    };
    use openagents_kernel_core::ids::sha256_prefixed_text;
    use serde_json::{Value, json};
    use std::collections::{HashMap, HashSet, VecDeque};
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex, mpsc};
    use std::time::{Duration, Instant};
    use tempfile::tempdir;
    use tokio::net::TcpListener;
    use tokio::sync::{mpsc as tokio_mpsc, oneshot};
    use tokio_tungstenite::{accept_async, tungstenite::Message};

    fn sample_snapshot() -> DesktopControlSnapshot {
        DesktopControlSnapshot {
            schema_version: DESKTOP_CONTROL_SCHEMA_VERSION,
            snapshot_revision: 1,
            state_signature: "sig-001".to_string(),
            generated_at_epoch_ms: 123,
            session: DesktopControlSessionStatus {
                pid: 42,
                shell_mode: "hotbar".to_string(),
                dev_mode_enabled: false,
                buy_mode_surface_enabled: true,
            },
            mission_control: DesktopControlMissionControlStatus {
                last_action: Some("Mission Control ready".to_string()),
                last_error: None,
                can_go_online: true,
                blocker_codes: vec!["APPLE_FM_UNAVAILABLE".to_string()],
                log_line_count: 3,
            },
            provider: DesktopControlProviderStatus {
                mode: "offline".to_string(),
                runtime_mode: "offline".to_string(),
                desired_mode_hint: "offline".to_string(),
                online: false,
                blocker_codes: vec!["APPLE_FM_UNAVAILABLE".to_string()],
                connected_relays: 0,
                degraded_reason_code: None,
                last_request_event_id: None,
                last_action: None,
                last_error: None,
                relay_urls: vec!["wss://relay.example".to_string()],
            },
            local_runtime: DesktopControlLocalRuntimeStatus {
                policy: "apple_foundation_models".to_string(),
                lane: Some("apple_foundation_models".to_string()),
                runtime_ready: true,
                go_online_ready: true,
                supports_sell_compute: true,
                workbench_label: "Apple FM workbench".to_string(),
                supports_run_text: true,
                supports_streaming: true,
                supports_structured: true,
                supports_model_management: false,
                supports_sessions: true,
                show_action_button: true,
                action: "refresh_apple_fm".to_string(),
                action_enabled: true,
                action_label: "REFRESH APPLE FM".to_string(),
                model_label: "apple-foundation-model".to_string(),
                backend_label: "Apple FM bridge (running)".to_string(),
                load_label: "ready".to_string(),
                go_online_hint: None,
                status_stream: "stdout".to_string(),
                status_line:
                    "Apple Foundation Models ready via Swift bridge (apple-foundation-model)."
                        .to_string(),
                detail_lines: vec![
                    "Apple FM: Refreshed Apple FM bridge health; model ready.".to_string(),
                ],
                diagnostics: LocalRuntimeDiagnostics::default(),
            },
            gpt_oss: DesktopControlGptOssStatus {
                detected: false,
                backend: None,
                reachable: false,
                ready: false,
                busy: false,
                supports_sell_compute: false,
                artifact_present: false,
                loaded: false,
                configured_model: None,
                ready_model: None,
                configured_model_path: None,
                loaded_models: Vec::new(),
                last_action: None,
                last_error: None,
            },
            apple_fm: DesktopControlAppleFmStatus {
                reachable: true,
                ready: true,
                model_available: true,
                ready_model: Some("apple-foundation-model".to_string()),
                adapter_inventory_supported: true,
                adapter_attach_supported: true,
                loaded_adapters: Vec::new(),
                active_session_id: None,
                active_session_adapter: None,
                bridge_status: Some("running".to_string()),
                last_action: Some("Refreshed Apple FM bridge health; model ready.".to_string()),
                last_error: None,
            },
            wallet: DesktopControlWalletStatus {
                balance_sats: 77,
                balance_known: true,
                balance_reconciling: false,
                network: "mainnet".to_string(),
                network_status: "connected".to_string(),
                can_withdraw: true,
                withdraw_block_reason: None,
                last_action: Some("Wallet refreshed".to_string()),
                last_error: None,
            },
            tunnels: DesktopControlTunnelsStatus {
                available: true,
                approved_service_count: 1,
                active_service_count: 0,
                open_tunnel_count: 0,
                services: vec![DesktopControlTunnelServiceStatus {
                    service_id: "desktop-control".to_string(),
                    kind: "desktop_control_http".to_string(),
                    protocol: "http_request_response".to_string(),
                    active: false,
                    allowed_peer_count: 1,
                    request_count: 0,
                    response_count: 0,
                    bytes_in: 0,
                    bytes_out: 0,
                    last_error: None,
                }],
                tunnels: Vec::new(),
            },
            inventory: DesktopControlInventoryStatus {
                authority: "kernel_projected".to_string(),
                projection: DesktopControlInventoryProjectionStatus {
                    source: "kernel_projection".to_string(),
                    latest_snapshot_id: Some("snapshot.compute.1".to_string()),
                    compute_products_active: 2,
                    compute_capacity_lots_open: 2,
                    compute_capacity_lots_delivering: 0,
                    compute_inventory_quantity_open: 1024,
                    compute_inventory_quantity_reserved: 0,
                    compute_inventory_quantity_delivering: 0,
                    compute_delivery_proofs_24h: 8,
                    compute_validator_challenges_open: 1,
                },
                sections: vec![
                    DesktopControlInventorySectionStatus {
                        section_id: "local".to_string(),
                        label: "Local".to_string(),
                        available: true,
                        blocker_reason: None,
                        summary: "products=1 ready=1 eligible=1 open_quantity=1024".to_string(),
                        product_count: 1,
                        ready_product_count: 1,
                        eligible_product_count: 1,
                        open_quantity: 1024,
                        products: Vec::new(),
                    },
                    DesktopControlInventorySectionStatus {
                        section_id: "cluster".to_string(),
                        label: "Cluster".to_string(),
                        available: false,
                        blocker_reason: Some(
                            crate::provider_inventory::CLUSTER_NOT_INTEGRATED_REASON.to_string(),
                        ),
                        summary: "topology=not_integrated members=0".to_string(),
                        product_count: 0,
                        ready_product_count: 0,
                        eligible_product_count: 0,
                        open_quantity: 0,
                        products: Vec::new(),
                    },
                    DesktopControlInventorySectionStatus {
                        section_id: "sandbox".to_string(),
                        label: "Sandbox".to_string(),
                        available: true,
                        blocker_reason: None,
                        summary:
                            "profiles=1 ready_profiles=1 products=1 ready=1 eligible=1 open_quantity=32"
                                .to_string(),
                        product_count: 1,
                        ready_product_count: 1,
                        eligible_product_count: 1,
                        open_quantity: 32,
                        products: Vec::new(),
                    },
                ],
            },
            buyer_procurement: DesktopControlBuyerProcurementStatus {
                load_state: "ready".to_string(),
                quote_mode: "spot".to_string(),
                last_action: Some("Loaded 1 compute quote for rfq=rfq-spot-1".to_string()),
                last_error: None,
                last_spot_rfq_summary: Some(
                    "rfq=rfq-spot-1 family=inference backend=gpt_oss qty=1 window=15m max_price=34 sats constraints=none"
                        .to_string(),
                ),
                last_forward_rfq_summary: None,
                selected_spot_quote_id: Some("quote-spot-1".to_string()),
                selected_forward_quote_id: None,
                spot_quotes: vec![DesktopControlBuyerProcurementQuoteStatus {
                    quote_id: "quote-spot-1".to_string(),
                    rfq_id: "rfq-spot-1".to_string(),
                    product_id: "psionic.local.inference.gpt_oss.single_node".to_string(),
                    provider_id: "npub1provider".to_string(),
                    compute_family: "inference".to_string(),
                    backend: "gpt_oss".to_string(),
                    execution: "local_inference".to_string(),
                    topology: "single_node".to_string(),
                    provisioning: "desktop_local".to_string(),
                    proof_posture: "delivery_proof_only".to_string(),
                    requested_quantity: 1,
                    available_quantity: 2,
                    price_sats: 21,
                    delivery_window_label: "15m".to_string(),
                    environment_ref: None,
                    sandbox_profile_ref: None,
                    source_badge: "desktop.go_online".to_string(),
                    terms_label: "spot session / local best effort".to_string(),
                    capability_summary:
                        "backend=gpt_oss execution=local_inference family=inference"
                            .to_string(),
                    collateral_summary: None,
                    remedy_summary: None,
                    selected: true,
                }],
                forward_quotes: Vec::new(),
                accepted_spot_orders: vec![DesktopControlBuyerProcurementOrderStatus {
                    order_id: "spot-order-1".to_string(),
                    rfq_id: "rfq-spot-1".to_string(),
                    quote_id: "quote-spot-1".to_string(),
                    instrument_id: "instrument-spot-1".to_string(),
                    product_id: "psionic.local.inference.gpt_oss.single_node".to_string(),
                    provider_id: "npub1provider".to_string(),
                    compute_family: "inference".to_string(),
                    backend: "gpt_oss".to_string(),
                    execution: "local_inference".to_string(),
                    topology: "single_node".to_string(),
                    provisioning: "desktop_local".to_string(),
                    proof_posture: "delivery_proof_only".to_string(),
                    quantity: 1,
                    price_sats: 21,
                    delivery_window_label: "15m".to_string(),
                    environment_ref: None,
                    sandbox_profile_ref: None,
                    collateral_summary: None,
                    remedy_summary: None,
                    authority_status: "spot-accepted".to_string(),
                    accepted_at_epoch_seconds: 1_762_000_000,
                }],
                accepted_forward_orders: Vec::new(),
            },
            cluster: DesktopControlClusterStatus {
                available: false,
                topology_label: "not_integrated".to_string(),
                member_count: 0,
                members: Vec::new(),
                last_error: Some(crate::provider_inventory::CLUSTER_NOT_INTEGRATED_REASON.to_string()),
            },
            sandbox: DesktopControlSandboxStatus {
                available: true,
                declared_profile_count: 1,
                ready_profile_count: 1,
                job_count: 0,
                active_job_count: 0,
                profiles: Vec::new(),
                jobs: Vec::new(),
                last_error: None,
            },
            training: DesktopControlTrainingStatus {
                available: true,
                source: "kernel_authority".to_string(),
                control_plane_state: "authority_projected".to_string(),
                artifact_plane_state: "staging_idle".to_string(),
                last_synced_at_epoch_ms: Some(1_762_500_002_000),
                run_count: 1,
                active_run_count: 1,
                accepted_run_count: 0,
                accepted_outcome_count: 1,
                environment_versions: vec!["2026.03.13".to_string()],
                checkpoint_refs: vec![
                    "checkpoint://decoder/base".to_string(),
                    "checkpoint://decoder/promoted".to_string(),
                ],
                contributor_set_revision: Some("contributors-7".to_string()),
                contributor_reselection_timing: Some("30000ms".to_string()),
                admitted_participant_count: 2,
                contributing_participant_count: 1,
                stale_rollout_discard_count: 1,
                duplicate_rollout_quarantine_count: 1,
                duplicate_rollout_deweight_count: 2,
                validator_verified_count: 3,
                validator_rejected_count: 1,
                validator_timed_out_count: 0,
                sandbox_ready_profile_count: 1,
                sandbox_active_job_count: 0,
                runs: vec![DesktopControlTrainingRunStatus {
                    training_run_id: "training-run-1".to_string(),
                    status: "running".to_string(),
                    training_policy_ref: "policy://train/weather".to_string(),
                    environment_ref: "env.openagents.weather.agent".to_string(),
                    environment_version: Some("2026.03.13".to_string()),
                    checkpoint_family: "decoder".to_string(),
                    validator_policy_ref: "policy://validator/training".to_string(),
                    benchmark_package_count: 2,
                    rollout_verification_eval_run_count: 1,
                    expected_step_count: Some(64),
                    completed_step_count: Some(23),
                    final_checkpoint_ref: Some("checkpoint://decoder/base".to_string()),
                    promotion_checkpoint_ref: None,
                    accepted_outcome_id: None,
                    best_eval_score_bps: Some(9_320),
                    control_plane_state: "running".to_string(),
                    artifact_plane_state: "artifacts_pending".to_string(),
                }],
                participants: vec![
                    DesktopControlTrainingParticipantStatus {
                        participant_id: "node-a".to_string(),
                        visible_reason: "cluster_member".to_string(),
                        admitted: true,
                        contributing: true,
                        priority_label: "selected".to_string(),
                        deweight_reason: None,
                        exclusion_reason: None,
                    },
                    DesktopControlTrainingParticipantStatus {
                        participant_id: "node-b".to_string(),
                        visible_reason: "cluster_member".to_string(),
                        admitted: true,
                        contributing: false,
                        priority_label: "standby".to_string(),
                        deweight_reason: Some("duplicate_contribution".to_string()),
                        exclusion_reason: None,
                    },
                ],
                operator: DesktopControlAppleAdapterOperatorStatus::default(),
                last_error: None,
            },
            proofs: DesktopControlProofStatus {
                available: false,
                source: "unavailable".to_string(),
                pending_count: 0,
                last_error: Some(
                    "kernel authority unavailable: hosted control endpoint is not configured"
                        .to_string(),
                ),
                ..DesktopControlProofStatus::default()
            },
            challenges: DesktopControlChallengeStatus {
                available: false,
                source: "unavailable".to_string(),
                open_count: 0,
                last_error: Some(
                    "kernel authority unavailable: hosted control endpoint is not configured"
                        .to_string(),
                ),
                ..DesktopControlChallengeStatus::default()
            },
            buy_mode: DesktopControlBuyModeStatus {
                enabled: false,
                approved_budget_sats: 2,
                cadence_seconds: 1,
                cadence_millis: 100,
                next_dispatch_countdown_seconds: None,
                next_dispatch_countdown_millis: None,
                in_flight_request_id: None,
                in_flight_phase: None,
                in_flight_status: None,
                selected_provider_pubkey: None,
                result_provider_pubkey: None,
                invoice_provider_pubkey: None,
                payable_provider_pubkey: None,
                payment_blocker_codes: Vec::new(),
                payment_blocker_summary: None,
                target_selection: DesktopControlBuyModeTargetSelectionStatus::default(),
                peer_roster: Vec::new(),
                recent_requests: Vec::new(),
            },
            active_job: None,
            nip28: super::DesktopControlNip28Status::default(),
            recent_logs: vec!["15:00:00  Provider offline.".to_string()],
            last_command: None,
        }
    }

    fn sample_history_delivery_proof() -> DeliveryProof {
        DeliveryProof {
            delivery_proof_id: "delivery.compute.history-1".to_string(),
            capacity_lot_id: "lot.compute.history-1".to_string(),
            product_id: "psionic.cluster.inference.history".to_string(),
            instrument_id: Some("instrument.compute.history-1".to_string()),
            contract_id: Some("contract.compute.history-1".to_string()),
            created_at_ms: 1_762_500_000_000,
            metered_quantity: 4,
            accepted_quantity: 4,
            status: DeliveryProofStatus::Accepted,
            verification_evidence: Some(DeliveryVerificationEvidence {
                proof_bundle_ref: Some("proof_bundle:history-1".to_string()),
                activation_fingerprint_ref: Some("toploc:history-1".to_string()),
                validator_pool_ref: Some("validator-pool-a".to_string()),
                validator_run_ref: Some("validator-run-a".to_string()),
                challenge_result_refs: vec!["validator_challenge_result:history-1".to_string()],
                environment_ref: Some("env://cluster/runtime".to_string()),
                environment_version: Some("2026.03.13".to_string()),
                eval_run_ref: None,
            }),
            observed_capability_envelope: Some(ComputeCapabilityEnvelope {
                topology_kind: Some(ComputeTopologyKind::Replicated),
                provisioning_kind: Some(ComputeProvisioningKind::ClusterAttached),
                proof_posture: Some(ComputeProofPosture::ChallengeEligible),
                ..ComputeCapabilityEnvelope::default()
            }),
            metadata: json!({
                "runtime_manifest_ref": "manifest://cluster/runtime",
                "runtime_manifest_digest": "sha256:runtime-history",
                "session_claims_ref": "claims://cluster/session-1",
                "session_identity_posture": "claim_bound",
                "transport_identity_posture": "claim_bound",
                "runtime_config_identity_mode": "measured_static_plus_runtime_diff",
                "mutable_runtime_variables_present": true,
            }),
            ..DeliveryProof::default()
        }
    }

    fn sample_history_capacity_instrument() -> CapacityInstrument {
        CapacityInstrument {
            instrument_id: "instrument.compute.history-1".to_string(),
            product_id: "psionic.cluster.inference.history".to_string(),
            provider_id: Some("npub1provider".to_string()),
            quantity: 4,
            created_at_ms: 1_762_500_000_500,
            status: CapacityInstrumentStatus::Settled,
            kind: CapacityInstrumentKind::Spot,
            ..CapacityInstrument::default()
        }
    }

    fn sample_history_structured_instrument() -> StructuredCapacityInstrument {
        StructuredCapacityInstrument {
            structured_instrument_id: "structured.compute.history-1".to_string(),
            product_id: "psionic.cluster.inference.history".to_string(),
            provider_id: Some("npub1provider".to_string()),
            kind: StructuredCapacityInstrumentKind::Reservation,
            created_at_ms: 1_762_500_001_000,
            status: StructuredCapacityInstrumentStatus::Settled,
            legs: vec![openagents_kernel_core::compute::StructuredCapacityLeg {
                instrument_id: "instrument.compute.history-1".to_string(),
                ..openagents_kernel_core::compute::StructuredCapacityLeg::default()
            }],
            lifecycle_reason_detail: Some("reservation_settled_against_delivery".to_string()),
            ..StructuredCapacityInstrument::default()
        }
    }

    fn sample_history_validator_challenge() -> ComputeValidatorChallengeSnapshot {
        ComputeValidatorChallengeSnapshot {
            request: ComputeValidatorChallengeRequest {
                context: ComputeValidatorChallengeContext {
                    challenge_id: "challenge.compute.history-1".to_string(),
                    proof_bundle_digest: sha256_prefixed_text("proof_bundle:history-1"),
                    request_digest: "sha256:request-history".to_string(),
                    delivery_proof_id: Some("delivery.compute.history-1".to_string()),
                    product_id: "psionic.cluster.inference.history".to_string(),
                    runtime_backend: "gpt_oss".to_string(),
                    model_id: Some("gpt-oss-20b".to_string()),
                    validator_pool_ref: Some("validator-pool-a".to_string()),
                    created_at_ms: 1_762_500_000_100,
                    max_attempts: 3,
                    lease_timeout_ms: 30_000,
                },
                protocol: ComputeValidatorChallengeProtocolKind::GpuFreivaldsMerkleV1,
            },
            status: ComputeValidatorChallengeStatus::Verified,
            attempts_used: 1,
            final_result: Some(ComputeValidatorChallengeResult {
                challenge_id: "challenge.compute.history-1".to_string(),
                proof_bundle_digest: sha256_prefixed_text("proof_bundle:history-1"),
                protocol_id: "openagents.validator.gpu_freivalds_merkle.v1".to_string(),
                attempt: 1,
                status: ComputeValidatorChallengeStatus::Verified,
                verdict: ComputeValidatorChallengeVerdict::Verified,
                reason_code: None,
                detail: "validator verified the claimed matrix product".to_string(),
                created_at_ms: 1_762_500_000_100,
                finalized_at_ms: 1_762_500_000_700,
                challenge_seed_digest: Some("sha256:challenge-seed".to_string()),
                verified_row_count: Some(9),
                result_digest: "sha256:result-history".to_string(),
                challenge_result_ref: "validator_challenge_result:history-1".to_string(),
            }),
            ..ComputeValidatorChallengeSnapshot::default()
        }
    }

    #[test]
    fn proof_history_surfaces_settlement_and_identity_review_fields() {
        let proof = sample_history_delivery_proof();
        let challenge = sample_history_validator_challenge();
        let challenges = vec![challenge];
        let proofs = vec![proof.clone()];
        let grouped = challenges_by_delivery_proof(proofs.as_slice(), challenges.as_slice());
        let instrument = sample_history_capacity_instrument();
        let structured = sample_history_structured_instrument();

        let status = desktop_control_proof_history_status(
            &proof,
            Some(&instrument),
            &[&structured],
            grouped
                .get(proof.delivery_proof_id.as_str())
                .expect("challenge group"),
        );

        assert_eq!(status.proof_status, "accepted");
        assert_eq!(status.proof_posture, "challenge_eligible");
        assert_eq!(status.topology_kind, "replicated");
        assert_eq!(status.provisioning_kind, "cluster_attached");
        assert_eq!(
            status.runtime_manifest_ref.as_deref(),
            Some("manifest://cluster/runtime")
        );
        assert_eq!(
            status.runtime_manifest_digest.as_deref(),
            Some("sha256:runtime-history")
        );
        assert_eq!(
            status.session_claims_ref.as_deref(),
            Some("claims://cluster/session-1")
        );
        assert_eq!(
            status.session_identity_posture.as_deref(),
            Some("claim_bound")
        );
        assert_eq!(
            status.transport_identity_posture.as_deref(),
            Some("claim_bound")
        );
        assert_eq!(
            status.runtime_config_identity_mode.as_deref(),
            Some("measured_static_plus_runtime_diff")
        );
        assert_eq!(status.mutable_runtime_variables_present, Some(true));
        assert_eq!(status.settlement_status.as_deref(), Some("settled"));
        assert!(
            status
                .settlement_summary
                .as_deref()
                .is_some_and(|summary| summary.contains("reservation"))
        );
        assert!(
            status
                .challenge_summary
                .as_deref()
                .is_some_and(|summary| summary.contains("status=verified"))
        );
    }

    #[test]
    fn settlement_and_challenge_history_stay_linked_to_same_delivery() {
        let proof = sample_history_delivery_proof();
        let instrument = sample_history_capacity_instrument();
        let structured = sample_history_structured_instrument();
        let challenge = sample_history_validator_challenge();
        let proofs = vec![proof.clone()];
        let challenges = vec![challenge.clone()];
        let grouped = challenges_by_delivery_proof(proofs.as_slice(), challenges.as_slice());
        let settlements =
            build_settlement_history(&[proof.clone()], &[instrument], &[structured], &grouped);
        let challenge_history = desktop_control_challenge_history_status(
            &challenge,
            vec![proof.delivery_proof_id.clone()],
            Some("reservation settled // related_challenges=1".to_string()),
        );

        assert_eq!(settlements.len(), 2);
        assert!(settlements.iter().any(|settlement| {
            settlement.settlement_id == "instrument.compute.history-1"
                && settlement.delivery_proof_ids == vec!["delivery.compute.history-1".to_string()]
                && settlement.challenge_ids == vec!["challenge.compute.history-1".to_string()]
        }));
        assert_eq!(
            challenge_history.delivery_proof_ids,
            vec!["delivery.compute.history-1".to_string()]
        );
        assert_eq!(challenge_history.status, "verified");
        assert!(
            challenge_history
                .settlement_impact_summary
                .as_deref()
                .is_some_and(|summary| summary.contains("reservation settled"))
        );
    }

    #[test]
    fn snapshot_change_events_emit_inventory_event_when_inventory_truth_changes() {
        let previous = sample_snapshot();
        let mut current = sample_snapshot();
        current.inventory.authority = "local_only".to_string();
        current.inventory.projection.source = "local_only".to_string();

        let events = snapshot_change_events(Some(&previous), &current);
        let inventory = events
            .iter()
            .find(|event| event.event_type == "inventory.state.changed")
            .expect("inventory change event should be emitted");

        assert!(inventory.summary.contains("inventory authority=local_only"));
        assert_eq!(inventory.command_label, None);
        assert_eq!(inventory.success, None);
    }

    #[test]
    fn snapshot_change_events_emit_training_event_when_training_truth_changes() {
        let previous = sample_snapshot();
        let mut current = sample_snapshot();
        current.training.control_plane_state = "authority_projected_stale".to_string();
        current.training.duplicate_rollout_quarantine_count = 2;

        let events = snapshot_change_events(Some(&previous), &current);
        let training = events
            .iter()
            .find(|event| event.event_type == "training.state.changed")
            .expect("training change event should be emitted");

        assert!(training.summary.contains("training available=true"));
        assert_eq!(training.command_label, None);
        assert_eq!(training.success, None);
    }

    #[test]
    fn snapshot_change_events_emit_buyer_procurement_event_when_quote_truth_changes() {
        let previous = sample_snapshot();
        let mut current = sample_snapshot();
        current.buyer_procurement.quote_mode = "forward_physical".to_string();
        current.buyer_procurement.last_forward_rfq_summary =
            Some("rfq=rfq-forward-1 family=sandbox_execution".to_string());

        let events = snapshot_change_events(Some(&previous), &current);
        let procurement = events
            .iter()
            .find(|event| event.event_type == "buyer_procurement.state.changed")
            .expect("buyer procurement change event should be emitted");

        assert!(procurement.summary.contains("buyer procurement load=ready"));
        assert_eq!(procurement.command_label, None);
        assert_eq!(procurement.success, None);
    }

    #[test]
    fn snapshot_change_events_emit_buy_mode_event_when_target_selection_changes() {
        let previous = sample_snapshot();
        let mut current = sample_snapshot();
        let selected_peer_pubkey = "11".repeat(32);
        current.buy_mode.enabled = true;
        current.buy_mode.target_selection = DesktopControlBuyModeTargetSelectionStatus {
            selected_peer_pubkey: Some(selected_peer_pubkey.clone()),
            selected_relay_url: Some("wss://relay.openagents.test".to_string()),
            selected_ready_model: Some("apple-foundation-model".to_string()),
            observed_peer_count: 2,
            eligible_peer_count: 1,
            blocked_reason_code: None,
            blocked_reason: None,
        };

        let events = snapshot_change_events(Some(&previous), &current);
        let buy_mode = events
            .iter()
            .find(|event| event.event_type == "buyer.lifecycle.changed")
            .expect("buy mode change event should be emitted");

        assert_eq!(
            buy_mode
                .payload
                .as_ref()
                .and_then(|payload| payload.get("target_selection"))
                .and_then(|value| value.get("selected_peer_pubkey"))
                .and_then(Value::as_str),
            Some(selected_peer_pubkey.as_str())
        );
    }

    #[test]
    fn snapshot_change_events_emit_local_runtime_and_gpt_oss_domains() {
        let previous = sample_snapshot();
        let mut current = sample_snapshot();
        current.local_runtime.lane = Some("gpt_oss".to_string());
        current.local_runtime.policy = "gpt_oss_cuda".to_string();
        current.local_runtime.runtime_ready = false;
        current.local_runtime.go_online_ready = false;
        current.local_runtime.action = "warm_gpt_oss".to_string();
        current.local_runtime.action_label = "WARM GPT-OSS".to_string();
        current.local_runtime.status_line =
            "GPT-OSS runtime reachable but model is unloaded (gpt-oss-20b.gguf).".to_string();
        current.gpt_oss.detected = true;
        current.gpt_oss.backend = Some("cuda".to_string());
        current.gpt_oss.reachable = true;
        current.gpt_oss.artifact_present = true;
        current.gpt_oss.configured_model_path = Some("/tmp/models/gpt-oss-20b.gguf".to_string());

        let events = snapshot_change_events(Some(&previous), &current);
        assert!(
            events
                .iter()
                .any(|event| event.event_type == "local_runtime.state.changed")
        );
        assert!(
            events
                .iter()
                .any(|event| event.event_type == "gpt_oss.state.changed")
        );
        let sync = events
            .iter()
            .find(|event| event.event_type == "control.snapshot.synced")
            .expect("snapshot sync event should be emitted");
        let changed_domains = sync
            .payload
            .as_ref()
            .and_then(|payload| payload.get("changed_domains"))
            .and_then(Value::as_array)
            .expect("changed domains array");
        assert!(
            changed_domains
                .iter()
                .any(|value| value.as_str() == Some("local_runtime"))
        );
        assert!(
            changed_domains
                .iter()
                .any(|value| value.as_str() == Some("gpt_oss"))
        );
    }

    fn repeated_hex(ch: char, len: usize) -> String {
        std::iter::repeat_n(ch, len).collect()
    }

    fn signed_event(
        id: impl Into<String>,
        pubkey: impl Into<String>,
        created_at: u64,
        kind: u16,
        tags: Vec<Vec<String>>,
        content: impl Into<String>,
    ) -> Event {
        Event {
            id: id.into(),
            pubkey: pubkey.into(),
            created_at,
            kind,
            tags,
            content: content.into(),
            sig: repeated_hex('f', 128),
        }
    }

    fn build_test_group_metadata_event() -> Event {
        let template = GroupMetadataEvent::new(
            "oa-main",
            GroupMetadata::new().with_name("OpenAgents Main"),
            10,
        )
        .expect("group metadata");
        signed_event(
            repeated_hex('a', 64),
            repeated_hex('1', 64),
            10,
            39000,
            template.to_tags(),
            String::new(),
        )
    }

    fn build_test_channel_create_event(channel_id: &str) -> Event {
        let template = ManagedChannelCreateEvent::new(
            "oa-main",
            ChannelMetadata::new("main", "OpenAgents main channel", ""),
            20,
        )
        .expect("channel create")
        .with_hints(
            ManagedChannelHints::new()
                .with_slug("main")
                .with_channel_type(ManagedChannelType::Ops)
                .with_category_id("main")
                .with_category_label("Main")
                .with_position(1),
        )
        .expect("channel hints");
        signed_event(
            channel_id.to_string(),
            repeated_hex('2', 64),
            20,
            40,
            template.to_tags().expect("channel tags"),
            template.content().expect("channel content"),
        )
    }

    fn build_test_channel_message_event(
        event_id: &str,
        author_pubkey: &str,
        channel_id: &str,
        relay_url: &str,
        created_at: u64,
        content: &str,
    ) -> Event {
        let template =
            ManagedChannelMessageEvent::new("oa-main", channel_id, relay_url, content, created_at)
                .expect("channel message");
        signed_event(
            event_id.to_string(),
            author_pubkey.to_string(),
            created_at,
            42,
            template.to_tags().expect("message tags"),
            content.to_string(),
        )
    }

    #[derive(Clone, Debug, Default)]
    struct TestNip28RelayFilter {
        ids: Option<HashSet<String>>,
        kinds: Option<HashSet<u16>>,
        e_tags: Option<HashSet<String>>,
        limit: usize,
    }

    impl TestNip28RelayFilter {
        fn matches_event(&self, event: &Event) -> bool {
            if let Some(ids) = self.ids.as_ref()
                && !ids.contains(event.id.as_str())
            {
                return false;
            }
            if let Some(kinds) = self.kinds.as_ref()
                && !kinds.contains(&event.kind)
            {
                return false;
            }
            if let Some(expected_e_tags) = self.e_tags.as_ref() {
                let matched = event.tags.iter().any(|tag| {
                    tag.first().is_some_and(|value| value == "e")
                        && tag
                            .get(1)
                            .is_some_and(|value| expected_e_tags.contains(value.as_str()))
                });
                if !matched {
                    return false;
                }
            }
            true
        }
    }

    struct TestNip28RelayClient {
        sender: tokio_mpsc::UnboundedSender<Message>,
        subscriptions: HashMap<String, Vec<TestNip28RelayFilter>>,
    }

    struct TestNip28RelayState {
        next_client_id: u64,
        events: VecDeque<Event>,
        clients: HashMap<u64, TestNip28RelayClient>,
    }

    impl TestNip28RelayState {
        fn new() -> Self {
            Self {
                next_client_id: 0,
                events: VecDeque::new(),
                clients: HashMap::new(),
            }
        }

        fn register_client(&mut self, sender: tokio_mpsc::UnboundedSender<Message>) -> u64 {
            self.next_client_id = self.next_client_id.saturating_add(1);
            let client_id = self.next_client_id;
            self.clients.insert(
                client_id,
                TestNip28RelayClient {
                    sender,
                    subscriptions: HashMap::new(),
                },
            );
            client_id
        }

        fn remove_client(&mut self, client_id: u64) {
            self.clients.remove(&client_id);
        }

        fn set_subscription(
            &mut self,
            client_id: u64,
            subscription_id: String,
            filters: Vec<TestNip28RelayFilter>,
        ) -> Vec<Event> {
            let matching = test_relay_matching_events(self.events.iter(), filters.as_slice());
            if let Some(client) = self.clients.get_mut(&client_id) {
                client.subscriptions.insert(subscription_id, filters);
            }
            matching
        }

        fn close_subscription(&mut self, client_id: u64, subscription_id: &str) {
            if let Some(client) = self.clients.get_mut(&client_id) {
                client.subscriptions.remove(subscription_id);
            }
        }

        fn store_and_fanout(&mut self, event: Event) {
            if self.events.iter().any(|stored| stored.id == event.id) {
                return;
            }
            self.events.push_back(event.clone());
            let mut deliveries = Vec::<(tokio_mpsc::UnboundedSender<Message>, String)>::new();
            for client in self.clients.values() {
                for (subscription_id, filters) in &client.subscriptions {
                    if filters.iter().any(|filter| filter.matches_event(&event)) {
                        let payload = serde_json::json!(["EVENT", subscription_id, event]);
                        deliveries.push((client.sender.clone(), payload.to_string()));
                    }
                }
            }
            for (sender, payload) in deliveries {
                let _ = sender.send(Message::Text(payload.into()));
            }
        }
    }

    fn parse_test_relay_filters(values: &[Value]) -> Vec<TestNip28RelayFilter> {
        values
            .iter()
            .filter_map(Value::as_object)
            .map(|object| {
                let ids = object.get("ids").and_then(Value::as_array).map(|values| {
                    values
                        .iter()
                        .filter_map(Value::as_str)
                        .map(ToString::to_string)
                        .collect::<HashSet<_>>()
                });
                let kinds = object.get("kinds").and_then(Value::as_array).map(|values| {
                    values
                        .iter()
                        .filter_map(Value::as_u64)
                        .filter_map(|kind| u16::try_from(kind).ok())
                        .collect::<HashSet<_>>()
                });
                let e_tags = object.get("#e").and_then(Value::as_array).map(|values| {
                    values
                        .iter()
                        .filter_map(Value::as_str)
                        .map(ToString::to_string)
                        .collect::<HashSet<_>>()
                });
                let limit = object
                    .get("limit")
                    .and_then(Value::as_u64)
                    .and_then(|limit| usize::try_from(limit).ok())
                    .unwrap_or(256)
                    .max(1);
                TestNip28RelayFilter {
                    ids,
                    kinds,
                    e_tags,
                    limit,
                }
            })
            .collect()
    }

    fn test_relay_matching_events<'a>(
        events: impl Iterator<Item = &'a Event>,
        filters: &[TestNip28RelayFilter],
    ) -> Vec<Event> {
        if filters.is_empty() {
            return Vec::new();
        }
        let limit = filters
            .iter()
            .map(|filter| filter.limit)
            .max()
            .unwrap_or(256);
        let mut matching = Vec::new();
        let mut seen = HashSet::<String>::new();
        for event in events {
            if filters.iter().any(|filter| filter.matches_event(event))
                && seen.insert(event.id.clone())
            {
                matching.push(event.clone());
                if matching.len() >= limit {
                    break;
                }
            }
        }
        matching
    }

    async fn handle_test_nip28_relay_connection(
        state: Arc<Mutex<TestNip28RelayState>>,
        stream: tokio::net::TcpStream,
    ) {
        let websocket = accept_async(stream)
            .await
            .expect("upgrade websocket relay connection");
        let (mut writer, mut reader) = websocket.split();
        let (outbound_tx, mut outbound_rx) = tokio_mpsc::unbounded_channel::<Message>();
        let writer_task = tokio::spawn(async move {
            while let Some(message) = outbound_rx.recv().await {
                if writer.send(message).await.is_err() {
                    break;
                }
            }
        });

        let client_id = {
            let mut guard = state.lock().expect("lock test relay state");
            guard.register_client(outbound_tx.clone())
        };

        while let Some(frame) = reader.next().await {
            let Ok(frame) = frame else {
                break;
            };
            let Message::Text(text) = frame else {
                continue;
            };
            let value: Value = serde_json::from_str(text.as_ref()).expect("parse relay frame");
            let Some(frame) = value.as_array() else {
                continue;
            };
            let Some(kind) = frame.first().and_then(Value::as_str) else {
                continue;
            };
            match kind {
                "REQ" => {
                    if frame.len() < 3 {
                        continue;
                    }
                    let subscription_id = frame[1].as_str().expect("subscription id");
                    let filters = parse_test_relay_filters(&frame[2..]);
                    let matching = {
                        let mut guard = state.lock().expect("lock test relay state");
                        guard.set_subscription(client_id, subscription_id.to_string(), filters)
                    };
                    for event in matching {
                        let payload = serde_json::json!(["EVENT", subscription_id, event]);
                        let _ = outbound_tx.send(Message::Text(payload.to_string().into()));
                    }
                    let eose = serde_json::json!(["EOSE", subscription_id]);
                    let _ = outbound_tx.send(Message::Text(eose.to_string().into()));
                }
                "EVENT" => {
                    if frame.len() < 2 {
                        continue;
                    }
                    let event =
                        serde_json::from_value::<Event>(frame[1].clone()).expect("relay event");
                    {
                        let mut guard = state.lock().expect("lock test relay state");
                        guard.store_and_fanout(event.clone());
                    }
                    let ok = serde_json::json!(["OK", event.id, true, "accepted"]);
                    let _ = outbound_tx.send(Message::Text(ok.to_string().into()));
                }
                "CLOSE" => {
                    if let Some(subscription_id) = frame.get(1).and_then(Value::as_str) {
                        let mut guard = state.lock().expect("lock test relay state");
                        guard.close_subscription(client_id, subscription_id);
                    }
                }
                _ => {}
            }
        }

        {
            let mut guard = state.lock().expect("lock test relay state");
            guard.remove_client(client_id);
        }
        writer_task.abort();
    }

    struct TestNip28Relay {
        url: String,
        state: Arc<Mutex<TestNip28RelayState>>,
        shutdown_tx: Option<oneshot::Sender<()>>,
        join_handle: Option<std::thread::JoinHandle<()>>,
    }

    impl TestNip28Relay {
        fn spawn() -> Self {
            let state = Arc::new(Mutex::new(TestNip28RelayState::new()));
            let (ready_tx, ready_rx) = mpsc::channel::<String>();
            let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
            let relay_state = Arc::clone(&state);
            let join_handle = std::thread::spawn(move || {
                let runtime = tokio::runtime::Builder::new_multi_thread()
                    .worker_threads(2)
                    .enable_all()
                    .build()
                    .expect("build test relay runtime");
                runtime.block_on(async move {
                    let listener = TcpListener::bind("127.0.0.1:0")
                        .await
                        .expect("bind test relay listener");
                    let local_addr = listener.local_addr().expect("resolve test relay addr");
                    ready_tx
                        .send(format!("ws://{local_addr}"))
                        .expect("send test relay addr");
                    let mut shutdown_rx = shutdown_rx;
                    loop {
                        tokio::select! {
                            _ = &mut shutdown_rx => break,
                            accept = listener.accept() => {
                                let Ok((stream, _)) = accept else {
                                    break;
                                };
                                let relay_state = Arc::clone(&relay_state);
                                tokio::spawn(async move {
                                    handle_test_nip28_relay_connection(relay_state, stream).await;
                                });
                            }
                        }
                    }
                });
            });
            let url = ready_rx.recv().expect("receive test relay addr");
            Self {
                url,
                state,
                shutdown_tx: Some(shutdown_tx),
                join_handle: Some(join_handle),
            }
        }

        fn store_events<I>(&self, events: I)
        where
            I: IntoIterator<Item = Event>,
        {
            let mut guard = self.state.lock().expect("lock test relay state");
            for event in events {
                guard.store_and_fanout(event);
            }
        }
    }

    impl Drop for TestNip28Relay {
        fn drop(&mut self) {
            if let Some(shutdown_tx) = self.shutdown_tx.take() {
                let _ = shutdown_tx.send(());
            }
            if let Some(handle) = self.join_handle.take() {
                let _ = handle.join();
            }
        }
    }

    fn pump_nip28_lane(
        chat: &mut AutopilotChatState,
        lane_worker: &mut Nip28ChatLaneWorker,
    ) -> bool {
        let mut changed = false;
        for update in lane_worker.drain_updates() {
            changed = true;
            match update {
                Nip28ChatLaneUpdate::RelayEvent(event) => {
                    chat.managed_chat_projection.record_relay_event(event);
                }
                Nip28ChatLaneUpdate::PublishAck { event_id } => {
                    let _ = chat.managed_chat_projection.ack_outbound_message(&event_id);
                    lane_worker.clear_dispatched(&event_id);
                }
                Nip28ChatLaneUpdate::PublishError { event_id, message } => {
                    let _ = chat
                        .managed_chat_projection
                        .fail_outbound_message(&event_id, &message);
                    lane_worker.clear_dispatched(&event_id);
                }
                Nip28ChatLaneUpdate::Eose { .. } | Nip28ChatLaneUpdate::ConnectionError { .. } => {}
            }
        }
        let pending_events = chat
            .managed_chat_projection
            .outbound_messages
            .iter()
            .filter(|message| message.delivery_state == ManagedChatDeliveryState::Publishing)
            .map(|message| message.event.clone())
            .collect::<Vec<_>>();
        for event in pending_events {
            lane_worker.publish(event);
        }
        if chat.maybe_auto_select_default_nip28_channel() {
            changed = true;
        }
        changed
    }

    fn build_test_snapshot(
        chat: &AutopilotChatState,
        provider_online: bool,
        snapshot_revision: u64,
    ) -> DesktopControlSnapshot {
        let mut snapshot = sample_snapshot();
        snapshot.snapshot_revision = snapshot_revision;
        snapshot.generated_at_epoch_ms = snapshot_revision;
        snapshot.mission_control.can_go_online = !provider_online;
        snapshot.mission_control.blocker_codes = if provider_online {
            Vec::new()
        } else {
            vec!["PROVIDER_OFFLINE".to_string()]
        };
        snapshot.provider.mode = if provider_online {
            "online".to_string()
        } else {
            "offline".to_string()
        };
        snapshot.provider.runtime_mode = snapshot.provider.mode.clone();
        snapshot.provider.desired_mode_hint = if provider_online {
            "online".to_string()
        } else {
            "offline".to_string()
        };
        snapshot.provider.online = provider_online;
        snapshot.provider.blocker_codes = snapshot.mission_control.blocker_codes.clone();
        snapshot.provider.connected_relays = usize::from(provider_online);
        snapshot.provider.last_action = Some(if provider_online {
            "Provider online".to_string()
        } else {
            "Provider offline".to_string()
        });
        snapshot.nip28 = super::desktop_control_nip28_status(chat);
        let now_epoch_seconds = super::current_epoch_seconds();
        let target_selection = chat.select_autopilot_buy_mode_target(now_epoch_seconds);
        snapshot.buy_mode.target_selection = DesktopControlBuyModeTargetSelectionStatus {
            selected_peer_pubkey: target_selection.selected_peer_pubkey,
            selected_relay_url: target_selection.selected_relay_url,
            selected_ready_model: target_selection.selected_ready_model,
            observed_peer_count: target_selection.observed_peer_count,
            eligible_peer_count: target_selection.eligible_peer_count,
            blocked_reason_code: target_selection.blocked_reason_code,
            blocked_reason: target_selection.blocked_reason,
        };
        snapshot.buy_mode.peer_roster = chat
            .autopilot_peer_roster(now_epoch_seconds)
            .into_iter()
            .map(super::desktop_control_autopilot_peer_status)
            .collect();
        snapshot.state_signature = snapshot_sync_signature(&snapshot);
        snapshot
    }

    fn overlay_buy_mode_snapshot(
        snapshot: &mut DesktopControlSnapshot,
        requests: &crate::state::operations::NetworkRequestsState,
        wallet: &SparkPaneState,
        loop_enabled: bool,
    ) {
        let flows = crate::nip90_compute_flow::buy_mode_request_flow_snapshots(requests, wallet);
        let active = flows.first();
        snapshot.buy_mode.enabled = loop_enabled;
        snapshot.buy_mode.next_dispatch_countdown_seconds =
            (loop_enabled && active.is_none()).then_some(0);
        snapshot.buy_mode.next_dispatch_countdown_millis =
            (loop_enabled && active.is_none()).then_some(0);
        snapshot.buy_mode.in_flight_request_id =
            active.as_ref().map(|flow| flow.request_id.clone());
        snapshot.buy_mode.in_flight_status =
            active.as_ref().map(|flow| flow.status.label().to_string());
        snapshot.buy_mode.in_flight_phase =
            active.as_ref().map(|flow| flow.phase.as_str().to_string());
        snapshot.buy_mode.selected_provider_pubkey =
            active.and_then(|flow| flow.selected_provider_pubkey.clone());
        snapshot.buy_mode.result_provider_pubkey =
            active.and_then(|flow| flow.result_provider_pubkey.clone());
        snapshot.buy_mode.invoice_provider_pubkey =
            active.and_then(|flow| flow.invoice_provider_pubkey.clone());
        snapshot.buy_mode.payable_provider_pubkey =
            active.and_then(|flow| flow.payable_provider_pubkey.clone());
        snapshot.buy_mode.payment_blocker_codes = active
            .map(|flow| flow.payment_blocker_codes.clone())
            .unwrap_or_default();
        snapshot.buy_mode.payment_blocker_summary =
            active.and_then(|flow| flow.payment_blocker_summary.clone());
        snapshot.buy_mode.recent_requests = flows
            .iter()
            .take(8)
            .map(super::desktop_control_buy_mode_request_status)
            .collect();
    }

    fn sync_test_snapshot_with_buy_mode(
        runtime: &DesktopControlRuntime,
        previous_snapshot: &mut Option<DesktopControlSnapshot>,
        chat: &AutopilotChatState,
        provider_online: bool,
        next_revision: &mut u64,
        requests: &crate::state::operations::NetworkRequestsState,
        wallet: &SparkPaneState,
        loop_enabled: bool,
    ) -> DesktopControlSnapshot {
        let mut snapshot = build_test_snapshot(chat, provider_online, *next_revision);
        overlay_buy_mode_snapshot(&mut snapshot, requests, wallet, loop_enabled);
        *next_revision = next_revision.saturating_add(1);
        runtime
            .sync_snapshot(snapshot.clone())
            .expect("sync test snapshot with buy mode");
        runtime
            .append_events(snapshot_change_events(
                previous_snapshot.as_ref(),
                &snapshot,
            ))
            .expect("append snapshot events");
        *previous_snapshot = Some(snapshot.clone());
        snapshot
    }

    fn test_identity(seed: u8, label: &str) -> NostrIdentity {
        let private_key = [seed; 32];
        NostrIdentity {
            identity_path: PathBuf::from(format!("/tmp/openagents-{label}-identity")),
            mnemonic: format!("test mnemonic {label}"),
            npub: String::new(),
            nsec: String::new(),
            public_key_hex: nostr::get_public_key_hex(&private_key).expect("fixture pubkey"),
            private_key_hex: hex::encode(private_key),
        }
    }

    fn provider_auth_identity(identity: &NostrIdentity) -> ProviderNip90AuthIdentity {
        ProviderNip90AuthIdentity {
            npub: identity.npub.clone(),
            public_key_hex: identity.public_key_hex.clone(),
            private_key_hex: identity.private_key_hex.clone(),
        }
    }

    fn ready_provider_runtime(now: Instant) -> ProviderRuntimeState {
        let mut runtime = ProviderRuntimeState::default();
        runtime.mode = ProviderMode::Online;
        runtime.mode_changed_at = now;
        runtime.inventory_session_started_at_ms = Some(25_000);
        runtime.apple_fm.reachable = true;
        runtime.apple_fm.model_available = true;
        runtime.apple_fm.ready_model = Some("apple-foundation-model".to_string());
        runtime
    }

    fn fixture_compute_capability() -> ProviderNip90ComputeCapability {
        ProviderNip90ComputeCapability {
            backend: "apple-foundation-model".to_string(),
            reachable: true,
            configured_model: Some("apple-foundation-model".to_string()),
            ready_model: Some("apple-foundation-model".to_string()),
            available_models: vec!["apple-foundation-model".to_string()],
            loaded_models: vec!["apple-foundation-model".to_string()],
            last_error: None,
        }
    }

    fn sign_test_template(identity: &NostrIdentity, template: &EventTemplate) -> Event {
        let key_bytes = hex::decode(identity.private_key_hex.as_str()).expect("decode key hex");
        let private_key: [u8; 32] = key_bytes
            .try_into()
            .expect("identity private key length should be 32");
        nostr::finalize_event(template, &private_key).expect("sign test nostr event")
    }

    fn build_provider_result_event(
        identity: &NostrIdentity,
        request: &crate::app_state::JobInboxNetworkRequest,
        output: &str,
    ) -> Event {
        let mut result = JobResult::new(
            request.request_kind,
            request.request_id.clone(),
            request.requester.clone(),
            output.trim().to_string(),
        )
        .expect("provider result");
        if request.price_sats > 0 {
            result = result.with_amount(request.price_sats.saturating_mul(1000), None);
        }
        let template = create_job_result_event(&result);
        sign_test_template(identity, &template)
    }

    fn build_provider_payment_required_feedback_event(
        identity: &NostrIdentity,
        request: &crate::app_state::JobInboxNetworkRequest,
        bolt11: &str,
    ) -> Event {
        let feedback = JobFeedback::new(
            JobStatus::PaymentRequired,
            request.request_id.as_str(),
            request.requester.as_str(),
        )
        .with_status_extra("lightning settlement required".to_string())
        .with_amount(
            request.price_sats.saturating_mul(1000),
            Some(bolt11.to_string()),
        );
        let template = create_job_feedback_event(&feedback);
        sign_test_template(identity, &template)
    }

    fn wait_for_provider_lane_online(worker: &mut ProviderNip90LaneWorker) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            for update in worker.drain_updates() {
                if let ProviderNip90LaneUpdate::Snapshot(snapshot) = update
                    && snapshot.mode == crate::provider_nip90_lane::ProviderNip90LaneMode::Online
                    && snapshot.connected_relays > 0
                {
                    return;
                }
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        panic!("timed out waiting for provider lane online");
    }

    fn wait_for_ingressed_request(
        worker: &mut ProviderNip90LaneWorker,
        request_id: &str,
        timeout: Duration,
    ) -> Option<crate::app_state::JobInboxNetworkRequest> {
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            for update in worker.drain_updates() {
                if let ProviderNip90LaneUpdate::IngressedRequest(request) = update
                    && request.request_id == request_id
                {
                    return Some(request);
                }
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        None
    }

    fn wait_for_publish_outcome(
        worker: &mut ProviderNip90LaneWorker,
        request_id: &str,
        role: ProviderNip90PublishRole,
        timeout: Duration,
    ) -> Option<ProviderNip90PublishOutcome> {
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            for update in worker.drain_updates() {
                if let ProviderNip90LaneUpdate::PublishOutcome(outcome) = update
                    && outcome.request_id == request_id
                    && outcome.role == role
                {
                    return Some(outcome);
                }
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        None
    }

    fn pump_nip28_pair_until_snapshot(
        runtime: &DesktopControlRuntime,
        previous_snapshot: &mut Option<DesktopControlSnapshot>,
        buyer_chat: &mut AutopilotChatState,
        buyer_lane: &mut Nip28ChatLaneWorker,
        remote_chat: &mut AutopilotChatState,
        remote_lane: &mut Nip28ChatLaneWorker,
        provider_online: bool,
        next_revision: &mut u64,
        requests: &crate::state::operations::NetworkRequestsState,
        wallet: &SparkPaneState,
        loop_enabled: bool,
        predicate: impl Fn(&DesktopControlSnapshot) -> bool,
    ) -> DesktopControlSnapshot {
        for _ in 0..160 {
            let remote_changed = pump_nip28_lane(remote_chat, remote_lane);
            let buyer_changed = pump_nip28_lane(buyer_chat, buyer_lane);
            if remote_changed || buyer_changed {
                let snapshot = sync_test_snapshot_with_buy_mode(
                    runtime,
                    previous_snapshot,
                    buyer_chat,
                    provider_online,
                    next_revision,
                    requests,
                    wallet,
                    loop_enabled,
                );
                if predicate(&snapshot) {
                    return snapshot;
                }
            } else if let Some(snapshot) = previous_snapshot.as_ref()
                && predicate(snapshot)
            {
                return snapshot.clone();
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        panic!("timed out waiting for paired NIP-28 desktop control snapshot predicate");
    }

    fn sync_test_snapshot(
        runtime: &DesktopControlRuntime,
        previous_snapshot: &mut Option<DesktopControlSnapshot>,
        chat: &AutopilotChatState,
        provider_online: bool,
        next_revision: &mut u64,
    ) -> DesktopControlSnapshot {
        let snapshot = build_test_snapshot(chat, provider_online, *next_revision);
        *next_revision = next_revision.saturating_add(1);
        runtime
            .sync_snapshot(snapshot.clone())
            .expect("sync test snapshot");
        runtime
            .append_events(snapshot_change_events(
                previous_snapshot.as_ref(),
                &snapshot,
            ))
            .expect("append snapshot events");
        *previous_snapshot = Some(snapshot.clone());
        snapshot
    }

    fn wait_for_action_request(
        runtime: &mut DesktopControlRuntime,
    ) -> super::DesktopControlActionEnvelope {
        for _ in 0..80 {
            for update in runtime.drain_updates() {
                match update {
                    DesktopControlRuntimeUpdate::ActionRequest(envelope) => return envelope,
                    DesktopControlRuntimeUpdate::WorkerError(error) => {
                        panic!("desktop control runtime worker error: {error}");
                    }
                }
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        panic!("timed out waiting for desktop control action request");
    }

    fn post_action_async(
        client: &reqwest::blocking::Client,
        action_url: &str,
        token: &str,
        action: DesktopControlActionRequest,
    ) -> std::thread::JoinHandle<DesktopControlActionResponse> {
        let client = client.clone();
        let action_url = action_url.to_string();
        let token = token.to_string();
        std::thread::spawn(move || {
            client
                .post(action_url.as_str())
                .bearer_auth(token)
                .json(&action)
                .send()
                .expect("send desktop control action")
                .error_for_status()
                .expect("desktop control action status")
                .json::<DesktopControlActionResponse>()
                .expect("decode desktop control action response")
        })
    }

    fn fetch_snapshot(
        client: &reqwest::blocking::Client,
        snapshot_url: &str,
        token: &str,
    ) -> DesktopControlSnapshot {
        client
            .get(snapshot_url)
            .bearer_auth(token)
            .send()
            .expect("fetch desktop control snapshot")
            .error_for_status()
            .expect("snapshot status")
            .json::<DesktopControlSnapshot>()
            .expect("decode desktop control snapshot")
    }

    fn fetch_events(
        client: &reqwest::blocking::Client,
        events_url: &str,
        token: &str,
    ) -> DesktopControlEventBatch {
        client
            .get(format!(
                "{events_url}?after_event_id=0&limit=128&timeout_ms=0"
            ))
            .bearer_auth(token)
            .send()
            .expect("fetch desktop control events")
            .error_for_status()
            .expect("events status")
            .json::<DesktopControlEventBatch>()
            .expect("decode desktop control events")
    }

    fn pump_until_snapshot(
        runtime: &DesktopControlRuntime,
        previous_snapshot: &mut Option<DesktopControlSnapshot>,
        chat: &mut AutopilotChatState,
        lane_worker: &mut Nip28ChatLaneWorker,
        provider_online: bool,
        next_revision: &mut u64,
        predicate: impl Fn(&DesktopControlSnapshot) -> bool,
    ) -> DesktopControlSnapshot {
        for _ in 0..120 {
            if pump_nip28_lane(chat, lane_worker) {
                let snapshot = sync_test_snapshot(
                    runtime,
                    previous_snapshot,
                    chat,
                    provider_online,
                    next_revision,
                );
                if predicate(&snapshot) {
                    return snapshot;
                }
            } else if let Some(snapshot) = previous_snapshot.as_ref()
                && predicate(snapshot)
            {
                return snapshot.clone();
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        panic!("timed out waiting for NIP-28 desktop control snapshot predicate");
    }

    #[test]
    fn validate_control_bind_addr_rejects_non_loopback_ip() {
        let error =
            validate_control_bind_addr("192.168.1.5:4848").expect_err("private ip should fail");
        assert!(error.contains("loopback"));
    }

    #[test]
    fn snapshot_signature_ignores_revision_metadata_and_detects_state_changes() {
        let first = sample_snapshot();
        let signature = snapshot_sync_signature(&first);

        let mut same_state_new_metadata = first.clone();
        same_state_new_metadata.snapshot_revision = 9;
        same_state_new_metadata.state_signature = "other".to_string();
        same_state_new_metadata.generated_at_epoch_ms = 999;
        assert_eq!(snapshot_sync_signature(&same_state_new_metadata), signature);

        let mut changed = first;
        changed.wallet.balance_sats = 88;
        assert_ne!(snapshot_sync_signature(&changed), signature);
    }

    #[test]
    fn snapshot_signature_changes_when_sandbox_truth_changes() {
        let first = sample_snapshot();
        let signature = snapshot_sync_signature(&first);

        let mut changed = first;
        changed.sandbox.job_count = 1;
        changed.sandbox.active_job_count = 1;
        assert_ne!(snapshot_sync_signature(&changed), signature);
    }

    #[test]
    fn buy_mode_request_status_preserves_result_invoice_and_payable_roles() {
        let selected = "aa".repeat(32);
        let result = "bb".repeat(32);
        let invoice = "cc".repeat(32);
        let status = super::desktop_control_buy_mode_request_status(
            &crate::nip90_compute_flow::BuyerRequestFlowSnapshot {
                request_id: "req-role-split".to_string(),
                request_type: crate::app_state::MISSION_CONTROL_BUY_MODE_REQUEST_TYPE.to_string(),
                budget_sats: 2,
                status: crate::state::operations::NetworkRequestStatus::Streaming,
                authority: crate::nip90_compute_flow::Nip90FlowAuthority::Relay,
                phase: crate::nip90_compute_flow::Nip90FlowPhase::RequestingPayment,
                next_expected_event: "valid provider invoice".to_string(),
                published_request_event_id: Some("event-role-split".to_string()),
                request_published_at_epoch_seconds: None,
                request_publish_selected_relays: Vec::new(),
                request_publish_accepted_relays: Vec::new(),
                request_publish_rejected_relays: Vec::new(),
                provider_observation_history: Vec::new(),
                selected_provider_pubkey: Some(selected.clone()),
                result_provider_pubkey: Some(result.clone()),
                result_relay_urls: Vec::new(),
                invoice_provider_pubkey: Some(invoice.clone()),
                invoice_relay_urls: Vec::new(),
                payable_provider_pubkey: None,
                last_feedback_status: Some("payment-required".to_string()),
                last_feedback_event_id: Some("feedback-role-split".to_string()),
                last_result_event_id: Some("result-role-split".to_string()),
                seller_success_feedback_event_id: None,
                winning_result_event_id: None,
                payment_pointer: None,
                payment_required_at_epoch_seconds: None,
                payment_sent_at_epoch_seconds: None,
                payment_failed_at_epoch_seconds: None,
                pending_bolt11: None,
                payment_blocker_codes: vec![
                    "result_without_invoice".to_string(),
                    "invoice_without_result".to_string(),
                ],
                payment_blocker_summary: Some(
                    "result provider bbbbbb..bbbb has no valid invoice // invoice provider cccccc..cccc has no non-error result"
                        .to_string(),
                ),
                payment_error: None,
                payment_notice: Some("invoice missing bolt11".to_string()),
                timestamp: None,
                wallet_status: "idle".to_string(),
                wallet_method: "-".to_string(),
                invoice_amount_sats: None,
                fees_sats: None,
                total_debit_sats: None,
                net_wallet_delta_sats: None,
                payment_hash: None,
                destination_pubkey: None,
                htlc_status: None,
                htlc_expiry_epoch_seconds: None,
                wallet_detail: None,
                wallet_description: None,
                wallet_invoice: None,
                loser_provider_count: 1,
                loser_reason_summary: Some("no payable winner".to_string()),
            },
        );

        assert_eq!(
            status.selected_provider_pubkey.as_deref(),
            Some(selected.as_str())
        );
        assert_eq!(
            status.result_provider_pubkey.as_deref(),
            Some(result.as_str())
        );
        assert_eq!(
            status.invoice_provider_pubkey.as_deref(),
            Some(invoice.as_str())
        );
        assert_eq!(status.payable_provider_pubkey, None);
        assert_eq!(
            status.payment_blocker_codes,
            vec![
                "result_without_invoice".to_string(),
                "invoice_without_result".to_string(),
            ]
        );
        assert!(
            status
                .payment_blocker_summary
                .as_deref()
                .is_some_and(|summary| summary.contains("result provider"))
        );
    }

    #[test]
    fn action_response_metadata_uses_snapshot_revision_and_signature() {
        let snapshot = sample_snapshot();
        let response =
            apply_response_snapshot_metadata(DesktopControlActionResponse::ok("ok"), &snapshot);

        assert_eq!(response.snapshot_revision, Some(snapshot.snapshot_revision));
        assert_eq!(response.state_signature, Some(snapshot.state_signature));
    }

    #[test]
    fn runtime_serves_snapshot_and_routes_actions() {
        let token = "token-123".to_string();
        let mut runtime = DesktopControlRuntime::spawn(DesktopControlRuntimeConfig {
            listen_addr: "127.0.0.1:0".parse().unwrap(),
            auth_token: token.clone(),
        })
        .expect("spawn desktop control runtime");
        runtime
            .sync_snapshot(sample_snapshot())
            .expect("sync sample snapshot");

        let client = reqwest::blocking::Client::new();
        let snapshot_url = format!("http://{}/v1/snapshot", runtime.listen_addr());
        let action_url = format!("http://{}/v1/action", runtime.listen_addr());

        let unauthorized = client
            .get(snapshot_url.as_str())
            .send()
            .expect("send unauthorized");
        assert_eq!(unauthorized.status(), reqwest::StatusCode::UNAUTHORIZED);

        let snapshot = client
            .get(snapshot_url.as_str())
            .bearer_auth(token.as_str())
            .send()
            .expect("send authorized")
            .error_for_status()
            .expect("authorized status")
            .json::<DesktopControlSnapshot>()
            .expect("decode snapshot");
        assert!(snapshot.snapshot_revision >= 1);
        assert!(!snapshot.state_signature.is_empty());
        assert_eq!(snapshot.wallet.balance_sats, 77);

        let join = std::thread::spawn({
            let client = client.clone();
            let token = token.clone();
            move || {
                client
                    .post(action_url.as_str())
                    .bearer_auth(token)
                    .json(&DesktopControlActionRequest::RefreshWallet)
                    .send()
                    .expect("post action")
                    .error_for_status()
                    .expect("action status")
                    .json::<DesktopControlActionResponse>()
                    .expect("decode action response")
            }
        });

        let mut envelope = None;
        for _ in 0..20 {
            let updates = runtime.drain_updates();
            if let Some(DesktopControlRuntimeUpdate::ActionRequest(request)) =
                updates.into_iter().next()
            {
                envelope = Some(request);
                break;
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        let request = envelope.expect("expected desktop control action request");
        assert_eq!(request.action, DesktopControlActionRequest::RefreshWallet);
        request.respond(DesktopControlActionResponse::ok("Queued wallet refresh"));
        let response = join.join().expect("join action thread");
        assert_eq!(response.message, "Queued wallet refresh");
    }

    #[test]
    fn runtime_serves_gpt_oss_ready_snapshot_fields() {
        let token = "token-gpt-oss".to_string();
        let runtime = DesktopControlRuntime::spawn(DesktopControlRuntimeConfig {
            listen_addr: "127.0.0.1:0".parse().unwrap(),
            auth_token: token.clone(),
        })
        .expect("spawn desktop control runtime");
        let mut snapshot = sample_snapshot();
        snapshot.mission_control.can_go_online = true;
        snapshot.mission_control.blocker_codes.clear();
        snapshot.provider.blocker_codes.clear();
        snapshot.local_runtime = DesktopControlLocalRuntimeStatus {
            policy: "gpt_oss_cuda".to_string(),
            lane: Some("gpt_oss".to_string()),
            runtime_ready: true,
            go_online_ready: true,
            supports_sell_compute: true,
            workbench_label: "GPT-OSS workbench".to_string(),
            supports_run_text: true,
            supports_streaming: false,
            supports_structured: false,
            supports_model_management: true,
            supports_sessions: false,
            show_action_button: true,
            action: "unload_gpt_oss".to_string(),
            action_enabled: true,
            action_label: "UNLOAD GPT-OSS".to_string(),
            model_label: "gpt-oss-20b".to_string(),
            backend_label: "GPT-OSS / CUDA".to_string(),
            load_label: "loaded / artifact present".to_string(),
            go_online_hint: None,
            status_stream: "stdout".to_string(),
            status_line: "GPT-OSS ready via cuda backend (gpt-oss-20b).".to_string(),
            detail_lines: vec![
                "GPT-OSS backend: CUDA".to_string(),
                "GPT-OSS load state: loaded".to_string(),
            ],
            diagnostics: LocalRuntimeDiagnostics::default(),
        };
        snapshot.gpt_oss = DesktopControlGptOssStatus {
            detected: true,
            backend: Some("cuda".to_string()),
            reachable: true,
            ready: true,
            busy: false,
            supports_sell_compute: true,
            artifact_present: true,
            loaded: true,
            configured_model: Some("gpt-oss-20b".to_string()),
            ready_model: Some("gpt-oss-20b".to_string()),
            configured_model_path: Some("/tmp/models/gpt-oss-20b.gguf".to_string()),
            loaded_models: vec!["gpt-oss-20b".to_string()],
            last_action: Some("Psionic model 'gpt-oss-20b' warmed".to_string()),
            last_error: None,
        };
        runtime
            .sync_snapshot(snapshot)
            .expect("sync gpt-oss snapshot");

        let client = reqwest::blocking::Client::new();
        let snapshot_url = format!("http://{}/v1/snapshot", runtime.listen_addr());
        let served = client
            .get(snapshot_url.as_str())
            .bearer_auth(token.as_str())
            .send()
            .expect("send authorized")
            .error_for_status()
            .expect("authorized status")
            .json::<DesktopControlSnapshot>()
            .expect("decode snapshot");

        assert_eq!(served.local_runtime.lane.as_deref(), Some("gpt_oss"));
        assert_eq!(served.local_runtime.policy, "gpt_oss_cuda");
        assert_eq!(served.local_runtime.workbench_label, "GPT-OSS workbench");
        assert!(served.local_runtime.runtime_ready);
        assert!(served.local_runtime.go_online_ready);
        assert!(served.local_runtime.supports_model_management);
        assert!(served.gpt_oss.detected);
        assert_eq!(served.gpt_oss.backend.as_deref(), Some("cuda"));
        assert!(served.gpt_oss.ready);
        assert!(served.gpt_oss.loaded);
        assert!(served.gpt_oss.artifact_present);
        assert!(served.mission_control.can_go_online);
    }

    #[test]
    fn runtime_serves_event_batches_and_long_poll_waits() {
        let token = "token-events".to_string();
        let runtime = DesktopControlRuntime::spawn(DesktopControlRuntimeConfig {
            listen_addr: "127.0.0.1:0".parse().unwrap(),
            auth_token: token.clone(),
        })
        .expect("spawn desktop control runtime");
        runtime
            .sync_snapshot(sample_snapshot())
            .expect("sync sample snapshot");
        runtime
            .append_events(vec![DesktopControlEventDraft {
                event_type: "control.command.applied".to_string(),
                summary: "provider-online applied".to_string(),
                command_label: Some("provider-online".to_string()),
                success: Some(true),
                payload: Some(serde_json::json!({
                    "command_label": "provider-online",
                    "snapshot_revision": 1,
                    "state_signature": "sig-001",
                })),
            }])
            .expect("append sample event");

        let client = reqwest::blocking::Client::new();
        let events_url = format!("http://{}/v1/events", runtime.listen_addr());

        let initial = client
            .get(format!(
                "{events_url}?after_event_id=0&limit=10&timeout_ms=0"
            ))
            .bearer_auth(token.as_str())
            .send()
            .expect("send initial events request")
            .error_for_status()
            .expect("initial events status")
            .json::<DesktopControlEventBatch>()
            .expect("decode initial event batch");
        assert_eq!(initial.events.len(), 1);
        assert_eq!(initial.events[0].event_type, "control.command.applied");
        let after_event_id = initial.last_event_id;

        let join = std::thread::spawn({
            let client = client.clone();
            let token = token.clone();
            let events_url = events_url.clone();
            move || {
                client
                    .get(format!(
                        "{events_url}?after_event_id={after_event_id}&limit=10&timeout_ms=500"
                    ))
                    .bearer_auth(token)
                    .send()
                    .expect("send waiting events request")
                    .error_for_status()
                    .expect("waiting events status")
                    .json::<DesktopControlEventBatch>()
                    .expect("decode waiting event batch")
            }
        });

        std::thread::sleep(Duration::from_millis(40));
        runtime
            .append_events(vec![DesktopControlEventDraft {
                event_type: "wallet.state.changed".to_string(),
                summary: "wallet balance=75 network_status=connected withdraw_ready=true"
                    .to_string(),
                command_label: None,
                success: None,
                payload: Some(serde_json::json!({
                    "balance_sats": 75,
                    "network_status": "connected",
                })),
            }])
            .expect("append waiting event");
        let waited = join.join().expect("join waiting event request");
        assert!(!waited.timed_out);
        assert_eq!(waited.events.len(), 1);
        assert_eq!(waited.events[0].event_type, "wallet.state.changed");

        let timed_out = client
            .get(format!(
                "{events_url}?after_event_id={}&limit=10&timeout_ms=25",
                waited.last_event_id
            ))
            .bearer_auth(token.as_str())
            .send()
            .expect("send timed out events request")
            .error_for_status()
            .expect("timed out events status")
            .json::<DesktopControlEventBatch>()
            .expect("decode timed out event batch");
        assert!(timed_out.timed_out);
        assert!(timed_out.events.is_empty());
    }

    #[test]
    fn desktop_control_request_routes_align_with_ui_owned_actions() {
        assert_eq!(
            DesktopControlActionRequest::SetProviderMode { online: true }
                .provider_mode_online_target(),
            Some(true)
        );
        assert_eq!(
            DesktopControlActionRequest::SetProviderMode { online: false }
                .provider_mode_online_target(),
            Some(false)
        );
        assert_eq!(
            DesktopControlActionRequest::RunAppleFmSmokeTest.label(),
            "apple-fm-smoke-test"
        );
        assert_eq!(
            DesktopControlActionRequest::RunAppleFmSmokeTest.provider_mode_online_target(),
            None
        );
        assert_eq!(
            DesktopControlActionRequest::GetNip90SentPaymentsReport {
                start_epoch_seconds: 1,
                end_epoch_seconds: 2,
                report_date: Some("2026-03-14".to_string()),
            }
            .label(),
            "nip90-sent-payments-report"
        );
    }

    #[test]
    fn nip90_sent_payments_report_payload_encodes_daily_window_totals() {
        let payload = build_nip90_sent_payments_report_payload(
            &Nip90BuyerPaymentWindowReport {
                start_epoch_seconds: 1_773_464_400,
                end_epoch_seconds: 1_773_550_800,
                payment_count: 2,
                total_sats_sent: 42,
                total_fee_sats: 3,
                total_wallet_debit_sats: 45,
                deduped_request_count: 1,
                degraded_binding_count: 0,
            },
            vec!["wss://relay.one".to_string(), "wss://relay.two".to_string()],
            1_773_464_400,
            1_773_550_800,
            Some("2026-03-14"),
            1_773_550_801,
        );

        assert_eq!(
            payload,
            DesktopControlNip90SentPaymentsReport {
                report_date: Some("2026-03-14".to_string()),
                window_start_epoch_seconds: 1_773_464_400,
                window_end_epoch_seconds: 1_773_550_800,
                window_start_rfc3339: "2026-03-14T05:00:00+00:00".to_string(),
                window_end_rfc3339: "2026-03-15T05:00:00+00:00".to_string(),
                payment_count: 2,
                total_sats_sent: 42,
                total_fee_sats: 3,
                total_wallet_debit_sats: 45,
                connected_relay_count: 2,
                relay_urls_considered: vec![
                    "wss://relay.one".to_string(),
                    "wss://relay.two".to_string(),
                ],
                deduped_request_count: 1,
                degraded_binding_count: 0,
                generated_at_epoch_seconds: 1_773_550_801,
                generated_at_rfc3339: "2026-03-15T05:00:01+00:00".to_string(),
            }
        );
    }

    #[test]
    fn event_batches_preserve_command_and_state_change_order_for_agents() {
        let token = "token-order".to_string();
        let runtime = DesktopControlRuntime::spawn(DesktopControlRuntimeConfig {
            listen_addr: "127.0.0.1:0".parse().unwrap(),
            auth_token: token.clone(),
        })
        .expect("spawn desktop control runtime");
        runtime
            .sync_snapshot(sample_snapshot())
            .expect("sync sample snapshot");
        runtime
            .append_events(vec![
                DesktopControlEventDraft {
                    event_type: "control.command.received".to_string(),
                    summary: "provider-online received".to_string(),
                    command_label: Some("provider-online".to_string()),
                    success: None,
                    payload: Some(serde_json::json!({ "command_label": "provider-online" })),
                },
                DesktopControlEventDraft {
                    event_type: "control.command.applied".to_string(),
                    summary: "provider-online applied".to_string(),
                    command_label: Some("provider-online".to_string()),
                    success: Some(true),
                    payload: Some(serde_json::json!({
                        "command_label": "provider-online",
                        "snapshot_revision": 2,
                    })),
                },
                DesktopControlEventDraft {
                    event_type: "provider.mode.changed".to_string(),
                    summary: "provider mode=online runtime=connecting relays=0".to_string(),
                    command_label: None,
                    success: None,
                    payload: Some(serde_json::json!({
                        "mode": "online",
                        "runtime_mode": "connecting",
                    })),
                },
            ])
            .expect("append ordered events");

        let client = reqwest::blocking::Client::new();
        let events_url = format!("http://{}/v1/events", runtime.listen_addr());
        let batch = client
            .get(format!(
                "{events_url}?after_event_id=0&limit=10&timeout_ms=0"
            ))
            .bearer_auth(token.as_str())
            .send()
            .expect("send ordered events request")
            .error_for_status()
            .expect("ordered events status")
            .json::<DesktopControlEventBatch>()
            .expect("decode ordered event batch");

        let event_types = batch
            .events
            .iter()
            .map(|event| event.event_type.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            event_types,
            vec![
                "control.command.received",
                "control.command.applied",
                "provider.mode.changed",
            ]
        );
        assert_eq!(
            batch.events[1]
                .payload
                .as_ref()
                .and_then(|payload| payload.get("snapshot_revision")),
            Some(&serde_json::Value::from(2))
        );
    }

    #[test]
    fn desktop_control_http_harness_goes_online_and_interacts_with_nip28_programmatically() {
        let relay = TestNip28Relay::spawn();
        let main_channel_id = DefaultNip28ChannelConfig::from_env_or_default().channel_id;
        let remote_pubkey = repeated_hex('9', 64);
        relay.store_events(vec![
            build_test_group_metadata_event(),
            build_test_channel_create_event(main_channel_id.as_str()),
            build_test_channel_message_event(
                &repeated_hex('d', 64),
                remote_pubkey.as_str(),
                main_channel_id.as_str(),
                relay.url.as_str(),
                30,
                "hello from remote autopilot",
            ),
        ]);

        let identity = nostr::regenerate_identity().expect("generate test nostr identity");
        let temp = tempdir().expect("tempdir");
        let projection_path = temp.path().join("managed-chat.json");
        let mut chat = AutopilotChatState::default();
        chat.managed_chat_projection =
            ManagedChatProjectionState::from_projection_path_for_tests(projection_path);
        chat.managed_chat_projection
            .set_local_pubkey(Some(identity.public_key_hex.as_str()));

        let mut lane_worker = Nip28ChatLaneWorker::spawn_with_config(DefaultNip28ChannelConfig {
            relay_url: relay.url.clone(),
            channel_id: main_channel_id.clone(),
        });

        let token = "token-nip28-programmatic".to_string();
        let mut runtime = DesktopControlRuntime::spawn(DesktopControlRuntimeConfig {
            listen_addr: "127.0.0.1:0".parse().unwrap(),
            auth_token: token.clone(),
        })
        .expect("spawn desktop control runtime");
        let client = reqwest::blocking::Client::new();
        let snapshot_url = format!("http://{}/v1/snapshot", runtime.listen_addr());
        let action_url = format!("http://{}/v1/action", runtime.listen_addr());
        let events_url = format!("http://{}/v1/events", runtime.listen_addr());

        let mut provider_online = false;
        let mut previous_snapshot = None;
        let mut next_revision = 1;
        let initial_snapshot = sync_test_snapshot(
            &runtime,
            &mut previous_snapshot,
            &chat,
            provider_online,
            &mut next_revision,
        );
        assert!(!initial_snapshot.provider.online);
        assert!(!initial_snapshot.nip28.available);

        let provider_join = post_action_async(
            &client,
            action_url.as_str(),
            token.as_str(),
            DesktopControlActionRequest::SetProviderMode { online: true },
        );
        let provider_request = wait_for_action_request(&mut runtime);
        assert_eq!(
            provider_request.action,
            DesktopControlActionRequest::SetProviderMode { online: true }
        );
        runtime
            .append_events(vec![command_received_event(&provider_request.action)])
            .expect("append provider command received");
        provider_online = true;
        let provider_snapshot = build_test_snapshot(&chat, provider_online, next_revision);
        next_revision = next_revision.saturating_add(1);
        let provider_response = apply_response_snapshot_metadata(
            DesktopControlActionResponse::ok("Queued provider online"),
            &provider_snapshot,
        );
        runtime
            .append_events(vec![command_outcome_event(
                &provider_request.action,
                &provider_response,
            )])
            .expect("append provider command outcome");
        runtime
            .sync_snapshot(provider_snapshot.clone())
            .expect("sync provider snapshot");
        runtime
            .append_events(snapshot_change_events(
                previous_snapshot.as_ref(),
                &provider_snapshot,
            ))
            .expect("append provider snapshot events");
        previous_snapshot = Some(provider_snapshot.clone());
        provider_request.respond(provider_response.clone());
        let provider_response = provider_join.join().expect("join provider action");
        assert!(provider_response.success);
        let provider_snapshot = fetch_snapshot(&client, snapshot_url.as_str(), token.as_str());
        assert!(provider_snapshot.provider.online);

        let loaded_snapshot = pump_until_snapshot(
            &runtime,
            &mut previous_snapshot,
            &mut chat,
            &mut lane_worker,
            provider_online,
            &mut next_revision,
            |snapshot| {
                snapshot.nip28.available
                    && snapshot.nip28.configured_channel_loaded
                    && snapshot.nip28.selected_channel_id.is_some()
                    && snapshot
                        .nip28
                        .recent_messages
                        .iter()
                        .any(|message| message.content == "hello from remote autopilot")
            },
        );
        assert_eq!(
            loaded_snapshot.nip28.selected_channel_id.as_deref(),
            Some(main_channel_id.as_str())
        );

        let select_join = post_action_async(
            &client,
            action_url.as_str(),
            token.as_str(),
            DesktopControlActionRequest::SelectNip28MainChannel,
        );
        let select_request = wait_for_action_request(&mut runtime);
        assert_eq!(
            select_request.action,
            DesktopControlActionRequest::SelectNip28MainChannel
        );
        runtime
            .append_events(vec![command_received_event(&select_request.action)])
            .expect("append nip28 main command received");
        let (_, configured_channel_id) =
            super::configured_nip28_main_channel(&chat).expect("configured main channel");
        let select_message = super::select_nip28_channel(&mut chat, configured_channel_id.as_str())
            .expect("select main channel");
        let select_snapshot = build_test_snapshot(&chat, provider_online, next_revision);
        next_revision = next_revision.saturating_add(1);
        let select_response = apply_response_snapshot_metadata(
            DesktopControlActionResponse::ok_with_payload(
                select_message,
                json!({
                    "group_id": chat
                        .active_managed_chat_group()
                        .map(|group| group.group_id.clone()),
                    "channel_id": chat
                        .active_managed_chat_channel()
                        .map(|channel| channel.channel_id.clone()),
                }),
            ),
            &select_snapshot,
        );
        runtime
            .append_events(vec![command_outcome_event(
                &select_request.action,
                &select_response,
            )])
            .expect("append nip28 main command outcome");
        runtime
            .sync_snapshot(select_snapshot.clone())
            .expect("sync nip28 main snapshot");
        runtime
            .append_events(snapshot_change_events(
                previous_snapshot.as_ref(),
                &select_snapshot,
            ))
            .expect("append nip28 main snapshot events");
        previous_snapshot = Some(select_snapshot);
        select_request.respond(select_response.clone());
        let select_response = select_join.join().expect("join nip28 main action");
        assert!(select_response.success);

        let send_join = post_action_async(
            &client,
            action_url.as_str(),
            token.as_str(),
            DesktopControlActionRequest::SendNip28Message {
                content: "hello from desktop control".to_string(),
                reply_to_event_id: None,
            },
        );
        let send_request = wait_for_action_request(&mut runtime);
        assert!(matches!(
            send_request.action,
            DesktopControlActionRequest::SendNip28Message { .. }
        ));
        runtime
            .append_events(vec![command_received_event(&send_request.action)])
            .expect("append nip28 send command received");
        let send_event_id =
            super::send_nip28_message(&mut chat, &identity, "hello from desktop control", None)
                .expect("queue nip28 message");
        let queued_snapshot = build_test_snapshot(&chat, provider_online, next_revision);
        next_revision = next_revision.saturating_add(1);
        let send_response = apply_response_snapshot_metadata(
            DesktopControlActionResponse::ok_with_payload(
                format!("Queued NIP-28 message {send_event_id}"),
                json!({
                    "event_id": send_event_id,
                    "channel_id": chat
                        .active_managed_chat_channel()
                        .map(|channel| channel.channel_id.clone()),
                    "reply_to_event_id": Value::Null,
                }),
            ),
            &queued_snapshot,
        );
        runtime
            .append_events(vec![command_outcome_event(
                &send_request.action,
                &send_response,
            )])
            .expect("append nip28 send command outcome");
        runtime
            .sync_snapshot(queued_snapshot.clone())
            .expect("sync queued nip28 snapshot");
        runtime
            .append_events(snapshot_change_events(
                previous_snapshot.as_ref(),
                &queued_snapshot,
            ))
            .expect("append queued nip28 snapshot events");
        previous_snapshot = Some(queued_snapshot.clone());
        send_request.respond(send_response.clone());
        let send_response = send_join.join().expect("join nip28 send action");
        assert!(send_response.success);
        assert_eq!(queued_snapshot.nip28.publishing_outbound_count, 1);

        let sent_snapshot = pump_until_snapshot(
            &runtime,
            &mut previous_snapshot,
            &mut chat,
            &mut lane_worker,
            provider_online,
            &mut next_revision,
            |snapshot| {
                snapshot.nip28.publishing_outbound_count == 0
                    && snapshot.nip28.recent_messages.iter().any(|message| {
                        message.content == "hello from desktop control"
                            && message.author_pubkey == identity.public_key_hex
                    })
            },
        );
        assert_eq!(
            sent_snapshot.nip28.selected_channel_id.as_deref(),
            Some(main_channel_id.as_str())
        );
        assert!(
            sent_snapshot
                .nip28
                .recent_messages
                .iter()
                .any(|message| message.content == "hello from remote autopilot")
        );

        let events = fetch_events(&client, events_url.as_str(), token.as_str());
        assert!(
            events
                .events
                .iter()
                .any(|event| event.event_type == "provider.mode.changed")
        );
        assert!(
            events
                .events
                .iter()
                .any(|event| event.event_type == "nip28.state.changed")
        );
        assert!(
            events
                .events
                .iter()
                .any(|event| event.summary.contains("nip28-send applied"))
        );
    }

    #[test]
    fn desktop_control_http_harness_targets_nip28_autopilot_peer_and_settles_buy_mode() {
        let relay = TestNip28Relay::spawn();
        let config = DefaultNip28ChannelConfig::from_env_or_default();
        relay.store_events(vec![
            build_test_group_metadata_event(),
            build_test_channel_create_event(config.channel_id.as_str()),
        ]);

        let buyer_identity = test_identity(0x31, "buyer");
        let target_identity = test_identity(0x32, "target-provider");
        let non_target_identity = test_identity(0x33, "non-target-provider");

        let buyer_temp = tempdir().expect("buyer tempdir");
        let buyer_projection_path = buyer_temp.path().join("buyer-managed-chat.json");
        let mut buyer_chat = AutopilotChatState::default();
        buyer_chat.managed_chat_projection =
            ManagedChatProjectionState::from_projection_path_for_tests(buyer_projection_path);
        buyer_chat
            .managed_chat_projection
            .set_local_pubkey(Some(buyer_identity.public_key_hex.as_str()));
        let mut buyer_chat_lane = Nip28ChatLaneWorker::spawn_with_config(config.clone());

        let target_temp = tempdir().expect("target tempdir");
        let target_projection_path = target_temp.path().join("target-managed-chat.json");
        let mut target_chat = AutopilotChatState::default();
        target_chat.managed_chat_projection =
            ManagedChatProjectionState::from_projection_path_for_tests(target_projection_path);
        target_chat
            .managed_chat_projection
            .set_local_pubkey(Some(target_identity.public_key_hex.as_str()));
        let mut target_chat_lane = Nip28ChatLaneWorker::spawn_with_config(config.clone());

        let token = "token-targeted-buy-mode".to_string();
        let mut runtime = DesktopControlRuntime::spawn(DesktopControlRuntimeConfig {
            listen_addr: "127.0.0.1:0".parse().unwrap(),
            auth_token: token.clone(),
        })
        .expect("spawn desktop control runtime");
        let client = reqwest::blocking::Client::new();
        let snapshot_url = format!("http://{}/v1/snapshot", runtime.listen_addr());
        let action_url = format!("http://{}/v1/action", runtime.listen_addr());
        let events_url = format!("http://{}/v1/events", runtime.listen_addr());

        let mut previous_snapshot = None;
        let mut next_revision = 1;
        let mut requests = crate::state::operations::NetworkRequestsState::default();
        let mut wallet = SparkPaneState::default();
        wallet.balance = Some(openagents_spark::Balance {
            spark_sats: crate::app_state::MISSION_CONTROL_BUY_MODE_BUDGET_SATS,
            lightning_sats: 0,
            onchain_sats: 0,
        });

        let initial_snapshot = sync_test_snapshot_with_buy_mode(
            &runtime,
            &mut previous_snapshot,
            &buyer_chat,
            false,
            &mut next_revision,
            &requests,
            &wallet,
            false,
        );
        assert!(!initial_snapshot.buy_mode.enabled);
        assert!(
            initial_snapshot
                .buy_mode
                .target_selection
                .selected_peer_pubkey
                .is_none()
        );

        let channel_loaded_snapshot = pump_nip28_pair_until_snapshot(
            &runtime,
            &mut previous_snapshot,
            &mut buyer_chat,
            &mut buyer_chat_lane,
            &mut target_chat,
            &mut target_chat_lane,
            false,
            &mut next_revision,
            &requests,
            &wallet,
            false,
            |snapshot| snapshot.nip28.available && snapshot.nip28.configured_channel_loaded,
        );
        assert!(
            channel_loaded_snapshot.nip28.configured_channel_loaded,
            "buyer should load the configured main channel"
        );
        assert!(
            target_chat
                .configured_main_managed_chat_channel(&config)
                .is_some(),
            "target provider should load the configured main channel"
        );

        let now = Instant::now();
        let now_epoch_seconds = super::current_epoch_seconds();
        let mut target_provider_runtime = ready_provider_runtime(now);
        assert!(pump_provider_chat_presence(
            &mut target_provider_runtime,
            &mut target_chat,
            Some(&target_identity),
            now,
            now_epoch_seconds,
        ));

        let roster_snapshot = pump_nip28_pair_until_snapshot(
            &runtime,
            &mut previous_snapshot,
            &mut buyer_chat,
            &mut buyer_chat_lane,
            &mut target_chat,
            &mut target_chat_lane,
            false,
            &mut next_revision,
            &requests,
            &wallet,
            false,
            |snapshot| {
                snapshot
                    .buy_mode
                    .target_selection
                    .selected_peer_pubkey
                    .as_deref()
                    == Some(target_identity.public_key_hex.as_str())
            },
        );
        assert_eq!(
            roster_snapshot
                .buy_mode
                .target_selection
                .selected_peer_pubkey
                .as_deref(),
            Some(target_identity.public_key_hex.as_str())
        );
        assert_eq!(
            roster_snapshot
                .buy_mode
                .target_selection
                .eligible_peer_count,
            1
        );
        assert!(
            roster_snapshot
                .buy_mode
                .peer_roster
                .iter()
                .any(|peer| peer.pubkey == target_identity.public_key_hex
                    && peer.online_for_compute
                    && peer.eligible_for_buy_mode)
        );

        let start_join = post_action_async(
            &client,
            action_url.as_str(),
            token.as_str(),
            DesktopControlActionRequest::StartBuyMode,
        );
        let start_request = wait_for_action_request(&mut runtime);
        assert_eq!(
            start_request.action,
            DesktopControlActionRequest::StartBuyMode
        );
        runtime
            .append_events(vec![command_received_event(&start_request.action)])
            .expect("append buy mode start command received");
        let armed_snapshot = sync_test_snapshot_with_buy_mode(
            &runtime,
            &mut previous_snapshot,
            &buyer_chat,
            false,
            &mut next_revision,
            &requests,
            &wallet,
            true,
        );
        let start_response = apply_response_snapshot_metadata(
            DesktopControlActionResponse::ok("Started buy mode"),
            &armed_snapshot,
        );
        runtime
            .append_events(vec![command_outcome_event(
                &start_request.action,
                &start_response,
            )])
            .expect("append buy mode start command outcome");
        start_request.respond(start_response.clone());
        let start_response = start_join.join().expect("join buy mode start action");
        assert!(start_response.success);
        assert!(armed_snapshot.buy_mode.enabled);

        let mut buyer_request_lane = ProviderNip90LaneWorker::spawn(vec![relay.url.clone()]);
        buyer_request_lane
            .enqueue(ProviderNip90LaneCommand::ConfigureIdentity {
                identity: Some(provider_auth_identity(&buyer_identity)),
            })
            .expect("configure buyer request identity");

        let mut target_provider_lane = ProviderNip90LaneWorker::spawn(vec![relay.url.clone()]);
        target_provider_lane
            .enqueue(ProviderNip90LaneCommand::ConfigureIdentity {
                identity: Some(provider_auth_identity(&target_identity)),
            })
            .expect("configure target provider identity");
        target_provider_lane
            .enqueue(ProviderNip90LaneCommand::ConfigureComputeCapability {
                capability: fixture_compute_capability(),
            })
            .expect("configure target provider capability");
        target_provider_lane
            .enqueue(ProviderNip90LaneCommand::SetOnline { online: true })
            .expect("bring target provider online");

        let mut non_target_provider_lane = ProviderNip90LaneWorker::spawn(vec![relay.url.clone()]);
        non_target_provider_lane
            .enqueue(ProviderNip90LaneCommand::ConfigureIdentity {
                identity: Some(provider_auth_identity(&non_target_identity)),
            })
            .expect("configure non-target provider identity");
        non_target_provider_lane
            .enqueue(ProviderNip90LaneCommand::ConfigureComputeCapability {
                capability: fixture_compute_capability(),
            })
            .expect("configure non-target provider capability");
        non_target_provider_lane
            .enqueue(ProviderNip90LaneCommand::SetOnline { online: true })
            .expect("bring non-target provider online");

        wait_for_provider_lane_online(&mut target_provider_lane);
        wait_for_provider_lane_online(&mut non_target_provider_lane);

        let request_event = crate::input::build_mission_control_buy_mode_request_event(
            Some(&buyer_identity),
            &[relay.url.clone()],
            &[target_identity.public_key_hex.clone()],
        )
        .expect("build targeted buy mode request");
        let request_id = requests
            .queue_request_submission(NetworkRequestSubmission {
                request_id: Some(request_event.id.clone()),
                request_type: crate::app_state::MISSION_CONTROL_BUY_MODE_REQUEST_TYPE.to_string(),
                payload: "Reply with the exact text BUY MODE OK.".to_string(),
                resolution_mode: BuyerResolutionMode::Race,
                target_provider_pubkeys: vec![target_identity.public_key_hex.clone()],
                skill_scope_id: None,
                credit_envelope_ref: None,
                budget_sats: crate::app_state::MISSION_CONTROL_BUY_MODE_BUDGET_SATS,
                timeout_seconds: crate::app_state::MISSION_CONTROL_BUY_MODE_TIMEOUT_SECONDS,
                authority_command_seq: 1,
            })
            .expect("queue targeted buy mode request");
        assert_eq!(request_id, request_event.id);

        buyer_request_lane
            .enqueue(ProviderNip90LaneCommand::TrackBuyerRequestIds {
                request_ids: vec![request_id.clone()],
            })
            .expect("track buyer request id");
        buyer_request_lane
            .enqueue(ProviderNip90LaneCommand::PublishEvent {
                request_id: request_id.clone(),
                role: ProviderNip90PublishRole::Request,
                event: Box::new(request_event),
            })
            .expect("publish targeted buy mode request");

        let request_publish = wait_for_publish_outcome(
            &mut buyer_request_lane,
            request_id.as_str(),
            ProviderNip90PublishRole::Request,
            Duration::from_secs(5),
        )
        .expect("buyer request publish outcome");
        assert!(request_publish.accepted_relays >= 1);
        requests.apply_nip90_request_publish_outcome(
            request_id.as_str(),
            request_publish.event_id.as_str(),
            request_publish.accepted_relays,
            request_publish.rejected_relays,
            request_publish.first_error.as_deref(),
        );

        let published_snapshot = sync_test_snapshot_with_buy_mode(
            &runtime,
            &mut previous_snapshot,
            &buyer_chat,
            false,
            &mut next_revision,
            &requests,
            &wallet,
            true,
        );
        assert_eq!(
            published_snapshot.buy_mode.in_flight_request_id.as_deref(),
            Some(request_id.as_str())
        );

        let targeted_request = wait_for_ingressed_request(
            &mut target_provider_lane,
            request_id.as_str(),
            Duration::from_secs(5),
        )
        .expect("target provider should ingest targeted request");
        assert_eq!(
            targeted_request.target_provider_pubkeys,
            vec![target_identity.public_key_hex.clone()]
        );

        let result_event =
            build_provider_result_event(&target_identity, &targeted_request, "BUY MODE OK.");
        let invoice = "lnbc20n1targetedbuymodeinvoice".to_string();
        let feedback_event = build_provider_payment_required_feedback_event(
            &target_identity,
            &targeted_request,
            invoice.as_str(),
        );
        target_provider_lane
            .enqueue(ProviderNip90LaneCommand::PublishEvent {
                request_id: request_id.clone(),
                role: ProviderNip90PublishRole::Result,
                event: Box::new(result_event.clone()),
            })
            .expect("publish targeted provider result");
        let result_publish = wait_for_publish_outcome(
            &mut target_provider_lane,
            request_id.as_str(),
            ProviderNip90PublishRole::Result,
            Duration::from_secs(5),
        )
        .expect("target provider result publish outcome");
        assert!(result_publish.accepted_relays >= 1);

        target_provider_lane
            .enqueue(ProviderNip90LaneCommand::PublishEvent {
                request_id: request_id.clone(),
                role: ProviderNip90PublishRole::Feedback,
                event: Box::new(feedback_event.clone()),
            })
            .expect("publish targeted provider payment-required feedback");
        let feedback_publish = wait_for_publish_outcome(
            &mut target_provider_lane,
            request_id.as_str(),
            ProviderNip90PublishRole::Feedback,
            Duration::from_secs(5),
        )
        .expect("target provider feedback publish outcome");
        assert!(feedback_publish.accepted_relays >= 1);

        let settle_deadline = Instant::now() + Duration::from_secs(5);
        let mut saw_result = false;
        let mut saw_payment_required = false;
        while Instant::now() < settle_deadline {
            let mut changed = false;
            for update in buyer_request_lane.drain_updates() {
                if let ProviderNip90LaneUpdate::BuyerResponseEvent(event) = update
                    && event.request_id == request_id
                {
                    changed = true;
                    match event.kind {
                        crate::provider_nip90_lane::ProviderNip90BuyerResponseKind::Result => {
                            saw_result = true;
                            let _ = requests.apply_nip90_buyer_result_event(
                                event.request_id.as_str(),
                                event.provider_pubkey.as_str(),
                                event.event_id.as_str(),
                                event.status.as_deref(),
                            );
                        }
                        crate::provider_nip90_lane::ProviderNip90BuyerResponseKind::Feedback => {
                            if event.status.as_deref() == Some("payment-required") {
                                saw_payment_required = true;
                            }
                            let _ = requests.apply_nip90_buyer_feedback_event(
                                event.request_id.as_str(),
                                event.provider_pubkey.as_str(),
                                event.event_id.as_str(),
                                event.status.as_deref(),
                                event.status_extra.as_deref(),
                                event.amount_msats,
                                event.bolt11.as_deref(),
                            );
                        }
                    }
                    if let Some((_bolt11, amount_sats)) = requests
                        .prepare_auto_payment_attempt_for_provider(
                            request_id.as_str(),
                            target_identity.public_key_hex.as_str(),
                            now_epoch_seconds.saturating_add(30),
                        )
                    {
                        assert_eq!(
                            amount_sats,
                            Some(crate::app_state::MISSION_CONTROL_BUY_MODE_BUDGET_SATS)
                        );
                        requests.record_auto_payment_pointer(
                            request_id.as_str(),
                            "wallet-targeted-buy-mode-001",
                        );
                        requests.mark_auto_payment_sent(
                            request_id.as_str(),
                            "wallet-targeted-buy-mode-001",
                            now_epoch_seconds.saturating_add(31),
                        );
                        wallet
                            .recent_payments
                            .push(openagents_spark::PaymentSummary {
                                id: "wallet-targeted-buy-mode-001".to_string(),
                                direction: "send".to_string(),
                                status: "succeeded".to_string(),
                                amount_sats: crate::app_state::MISSION_CONTROL_BUY_MODE_BUDGET_SATS,
                                fees_sats: 0,
                                timestamp: now_epoch_seconds.saturating_add(31),
                                method: "lightning".to_string(),
                                description: Some("Targeted buy mode settlement".to_string()),
                                invoice: Some(invoice.clone()),
                                destination_pubkey: Some(target_identity.public_key_hex.clone()),
                                payment_hash: Some("payment-hash-targeted-buy-mode".to_string()),
                                htlc_status: None,
                                htlc_expiry_epoch_seconds: None,
                                status_detail: None,
                            });
                    }
                }
            }
            if changed {
                let snapshot = sync_test_snapshot_with_buy_mode(
                    &runtime,
                    &mut previous_snapshot,
                    &buyer_chat,
                    false,
                    &mut next_revision,
                    &requests,
                    &wallet,
                    true,
                );
                if saw_result
                    && saw_payment_required
                    && snapshot.buy_mode.in_flight_status.as_deref() == Some("paid")
                {
                    break;
                }
            }
            std::thread::sleep(Duration::from_millis(20));
        }

        let settled_snapshot = fetch_snapshot(&client, snapshot_url.as_str(), token.as_str());
        assert_eq!(
            settled_snapshot
                .buy_mode
                .target_selection
                .selected_peer_pubkey
                .as_deref(),
            Some(target_identity.public_key_hex.as_str())
        );
        assert_eq!(
            settled_snapshot.buy_mode.in_flight_status.as_deref(),
            Some(NetworkRequestStatus::Paid.label())
        );
        assert_eq!(
            settled_snapshot.buy_mode.payable_provider_pubkey.as_deref(),
            Some(target_identity.public_key_hex.as_str())
        );
        assert!(
            settled_snapshot
                .buy_mode
                .recent_requests
                .iter()
                .any(|request| {
                    request.request_id == request_id
                        && (request.payable_provider_pubkey.as_deref()
                            == Some(target_identity.public_key_hex.as_str())
                            || request.selected_provider_pubkey.as_deref()
                                == Some(target_identity.public_key_hex.as_str()))
                        && request.wallet_status == "sent"
                })
        );

        let events = fetch_events(&client, events_url.as_str(), token.as_str());
        assert!(
            events
                .events
                .iter()
                .any(|event| event.event_type == "nip28.state.changed")
        );
        assert!(
            events
                .events
                .iter()
                .any(|event| event.event_type == "buyer.lifecycle.changed")
        );
        assert!(
            events.events.iter().any(|event| {
                event.event_type == "buyer.lifecycle.changed"
                    && event
                        .payload
                        .as_ref()
                        .and_then(|payload| payload.get("target_selection"))
                        .and_then(|selection| selection.get("selected_peer_pubkey"))
                        .and_then(Value::as_str)
                        == Some(target_identity.public_key_hex.as_str())
            }),
            "buyer lifecycle events should carry the selected targeted provider"
        );
        assert!(
            events.events.iter().any(|event| {
                event.event_type == "buyer.lifecycle.changed"
                    && event
                        .payload
                        .as_ref()
                        .and_then(|payload| payload.get("in_flight_status"))
                        .and_then(Value::as_str)
                        == Some(NetworkRequestStatus::Paid.label())
            }),
            "buyer lifecycle events should show paid settlement after targeted payment succeeds"
        );
    }
}
