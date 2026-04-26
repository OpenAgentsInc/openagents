use std::collections::{BTreeMap, HashSet};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::treasury::TreasuryPlaceholderPayoutMode;
use openagents_kernel_core::snapshots::{
    ComputeBreakerStatusRow, ComputeRolloutGateRow, ComputeTruthLabelRow,
};
use openagents_provider_substrate::ProviderTrainingCapabilityEnvelopeV2;
use serde::{Deserialize, Serialize};

const AUTHORITY_NAME: &str = "openagents-hosted-nexus";
const RECEIPT_RETENTION_LIMIT: usize = 8_192;
const PUBLIC_RECENT_RECEIPT_LIMIT: usize = 16;
const PUBLIC_STATS_WINDOW_MS: u64 = 86_400_000;

fn default_wallet_storage_runtime_mode() -> String {
    "original".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct AuthorityReceiptContext {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub relay_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub amount_sats: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payment_pointer: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub attributes: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AuthorityReceipt {
    pub seq: u64,
    pub receipt_id: String,
    pub receipt_type: String,
    pub recorded_at_unix_ms: u64,
    pub authority: String,
    #[serde(flatten)]
    pub context: AuthorityReceiptContext,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PublicRecentReceipt {
    pub receipt_id: String,
    pub receipt_type: String,
    pub recorded_at_unix_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub amount_sats: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PublicRecentPylon {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub node_label: Option<String>,
    pub nostr_pubkey_short: String,
    pub last_seen_at_unix_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_version: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub relay_urls: Vec<String>,
    pub eligible_product_count: u64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub products: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ready_model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime_state: Option<String>,
    #[serde(default)]
    pub inference_ready: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub training_capability_envelope_v2: Option<ProviderTrainingCapabilityEnvelopeV2>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PublicPylonClientVersionCount {
    pub client_version: String,
    pub online_sessions: u64,
    pub online_pylons: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PublicHomeworkWorkerPresenceOnlyBlockerCount {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_version: Option<String>,
    pub reason: String,
    pub online_sessions: u64,
    pub online_pylons: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PublicRecentPylonDiagnostic {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub node_label: Option<String>,
    pub nostr_pubkey_short: String,
    pub last_seen_at_unix_ms: u64,
    pub diagnostic_id: String,
    pub model_id: String,
    pub runtime_backend: String,
    pub status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    pub measured_at_unix_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub load_s: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mean_total_s: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mean_ttft_s: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mean_decode_tok_s: Option<f64>,
    pub repeats: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct PublicTrainingQueuePressure {
    #[serde(default)]
    pub state: String,
    #[serde(default)]
    pub active_windows: u64,
    #[serde(default)]
    pub pending_validation_windows: u64,
    #[serde(default)]
    pub validator_challenges_open: u64,
    #[serde(default)]
    pub validator_challenges_queued: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct PublicTrainingLaunchAlert {
    #[serde(default)]
    pub alert_id: String,
    #[serde(default)]
    pub severity: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub detail: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct PublicTrainingLaunchHealthSnapshot {
    #[serde(default)]
    pub generated_at_unix_ms: u64,
    #[serde(default)]
    pub overall_status: String,
    #[serde(default)]
    pub public_snapshot_source: String,
    #[serde(default)]
    pub public_stats_age_ms: u64,
    #[serde(default)]
    pub public_state_drift_from_kernel_ms: u64,
    #[serde(default)]
    pub active_runs: u64,
    #[serde(default)]
    pub run_backlog_slots: u64,
    #[serde(default)]
    pub pending_validation_windows: u64,
    #[serde(default)]
    pub validator_challenges_open: u64,
    #[serde(default)]
    pub validator_challenges_queued: u64,
    #[serde(default)]
    pub accepted_work_pending_payout_count: u64,
    #[serde(default)]
    pub accepted_work_attention_payout_count: u64,
    #[serde(default)]
    pub payouts_failed_24h: u64,
    #[serde(default)]
    pub payouts_skipped_24h: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolver_lookup_latency_p95_ms: Option<u64>,
    #[serde(default)]
    pub resolver_lookup_sample_count: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signed_access_latency_p95_ms: Option<u64>,
    #[serde(default)]
    pub signed_access_sample_count: u64,
    #[serde(default)]
    pub active_alert_count: u64,
    #[serde(default)]
    pub critical_alert_count: u64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub alerts: Vec<PublicTrainingLaunchAlert>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct PublicTrainingWorkClassState {
    #[serde(default)]
    pub work_class: String,
    #[serde(default)]
    pub progress_class: String,
    #[serde(default)]
    pub run_count: u64,
    #[serde(default)]
    pub active_run_count: u64,
    #[serde(default)]
    pub accepted_closeouts: u64,
    #[serde(default)]
    pub payout_eligible_closeouts: u64,
    #[serde(default)]
    pub weak_device_bearing_closeouts: u64,
    #[serde(default)]
    pub progress_bearing_closeouts: u64,
    #[serde(default)]
    pub participation_only_closeouts: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct PublicTrainingRunState {
    #[serde(default)]
    pub training_run_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(default)]
    pub network_id: String,
    #[serde(default)]
    pub run_status: String,
    #[serde(default)]
    pub scheduler_window_state: String,
    #[serde(default)]
    pub current_window_id: String,
    #[serde(default)]
    pub work_class: String,
    #[serde(default)]
    pub progress_class: String,
    #[serde(default)]
    pub replica_type: String,
    #[serde(default)]
    pub assigned_contributors: u64,
    #[serde(default)]
    pub weak_device_assigned_contributors: u64,
    #[serde(default)]
    pub accepted_contributors: u64,
    #[serde(default)]
    pub weak_device_accepted_contributors: u64,
    #[serde(default)]
    pub model_progress_contributors: u64,
    #[serde(default)]
    pub active_window_count: u64,
    #[serde(default)]
    pub pending_validation_window_count: u64,
    #[serde(default)]
    pub validator_challenges_open: u64,
    #[serde(default)]
    pub validator_challenges_queued: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_checkpoint_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_checkpoint_age_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_window_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_window_status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_closeout_status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_aggregate_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_promoted_checkpoint_ref: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct PublicTrainingWindowState {
    #[serde(default)]
    pub window_id: String,
    #[serde(default)]
    pub training_run_id: String,
    #[serde(default)]
    pub network_id: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub stage_id: String,
    #[serde(default)]
    pub work_class: String,
    #[serde(default)]
    pub progress_class: String,
    #[serde(default)]
    pub replica_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub round_index: Option<u64>,
    #[serde(default)]
    pub base_checkpoint_ref: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub planned_local_step_count: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aggregation_rule: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aggregation_weight_basis: Option<String>,
    #[serde(default)]
    pub total_contributions: u32,
    #[serde(default)]
    pub admitted_contributions: u32,
    #[serde(default)]
    pub accepted_contributions: u32,
    #[serde(default)]
    pub replay_required_contributions: u32,
    #[serde(default)]
    pub validator_challenges_open: u64,
    #[serde(default)]
    pub validator_challenges_queued: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aggregated_delta_digest: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accepted_aggregate_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_checkpoint_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub promoted_checkpoint_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accepted_outcome_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub closeout_status: Option<String>,
    #[serde(default)]
    pub payout_eligible: bool,
    #[serde(default)]
    pub weak_device_bearing: bool,
    #[serde(default)]
    pub lineage_advanced: bool,
    #[serde(default)]
    pub planned_at_ms: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub activated_at_ms: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sealed_at_ms: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reconciled_at_ms: Option<i64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct PublicTrainingStatsSnapshot {
    #[serde(default)]
    pub generated_at_unix_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_run_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_network_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_run_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_window_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_work_class: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_progress_class: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_replica_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_checkpoint_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_checkpoint_age_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_aggregate_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_promoted_checkpoint_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_window_status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_closeout_status: Option<String>,
    #[serde(default)]
    pub queue_pressure: PublicTrainingQueuePressure,
    #[serde(default)]
    pub launch_health: PublicTrainingLaunchHealthSnapshot,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub work_classes: Vec<PublicTrainingWorkClassState>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub runs: Vec<PublicTrainingRunState>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub windows: Vec<PublicTrainingWindowState>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct PublicTrainingRunContributionRow {
    #[serde(default)]
    pub contribution_id: String,
    #[serde(default)]
    pub training_run_id: String,
    #[serde(default)]
    pub window_id: String,
    #[serde(default)]
    pub stage_id: String,
    #[serde(default)]
    pub assignment_id: String,
    #[serde(default)]
    pub contributor_node_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub node_label: Option<String>,
    #[serde(default)]
    pub worker_id: String,
    #[serde(default)]
    pub validator_disposition: String,
    #[serde(default)]
    pub aggregation_eligibility: String,
    #[serde(default)]
    pub accepted_for_aggregation: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub local_step_count: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub consumed_token_count: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub consumed_example_count: Option<u64>,
    #[serde(default)]
    pub submission_receipt_digest: String,
    #[serde(default)]
    pub manifest_digest: String,
    #[serde(default)]
    pub object_digest: String,
    #[serde(default)]
    pub provenance_bundle_digest: String,
    #[serde(default)]
    pub validator_receipt_digest: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replay_receipt_digest: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub promotion_receipt_digest: Option<String>,
    #[serde(default)]
    pub recorded_at_ms: i64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct PublicTrainingRunNodeRow {
    #[serde(default)]
    pub node_pubkey_hex: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub node_label: Option<String>,
    #[serde(default)]
    pub role_claims: Vec<String>,
    #[serde(default)]
    pub allowed_networks: Vec<String>,
    #[serde(default)]
    pub release_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub build_version: Option<String>,
    #[serde(default)]
    pub build_digest: String,
    #[serde(default)]
    pub online: bool,
    #[serde(default)]
    pub eligible: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_training_run_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_window_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_assignment_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_successful_run_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_successful_window_id: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct PublicTrainingRunCaveat {
    #[serde(default)]
    pub caveat_id: String,
    #[serde(default)]
    pub severity: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub detail: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct PublicTrainingRunTreasuryStatus {
    #[serde(default)]
    pub payout_loop_health: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub degraded_reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wallet_runtime_status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wallet_last_error: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct PublicTrainingLaunchState {
    #[serde(default)]
    pub launch_id: String,
    #[serde(default)]
    pub phase: String,
    #[serde(default)]
    pub training_run_id: String,
    #[serde(default)]
    pub current_window_id: String,
    #[serde(default)]
    pub requested_at_ms: i64,
    #[serde(default)]
    pub updated_at_ms: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub launch_receipt_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bootstrap_uploaded_at_ms: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bootstrap_verified_at_ms: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scheduler_materialized_at_ms: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub leaseable_at_ms: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failure_reason: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct PublicTrainingRunDetailSnapshot {
    #[serde(default)]
    pub generated_at_unix_ms: u64,
    #[serde(default = "default_public_training_run_detail_snapshot_source")]
    pub snapshot_source: String,
    #[serde(default)]
    pub snapshot_age_ms: u64,
    #[serde(default)]
    pub snapshot_stale: bool,
    #[serde(default)]
    pub training_run_id: String,
    #[serde(default)]
    pub run: PublicTrainingRunState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub featured_window_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub featured_window: Option<PublicTrainingWindowState>,
    #[serde(default)]
    pub queue_pressure: PublicTrainingQueuePressure,
    #[serde(default)]
    pub launch_health: PublicTrainingLaunchHealthSnapshot,
    #[serde(default)]
    pub treasury: PublicTrainingRunTreasuryStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub launch: Option<PublicTrainingLaunchState>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub windows: Vec<PublicTrainingWindowState>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub contributions: Vec<PublicTrainingRunContributionRow>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub nodes: Vec<PublicTrainingRunNodeRow>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub caveats: Vec<PublicTrainingRunCaveat>,
}

fn default_public_training_run_detail_snapshot_source() -> String {
    "live".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PublicStatsSnapshot {
    pub service: String,
    pub authority: String,
    pub hosted_nexus_relay_url: String,
    pub as_of_unix_ms: u64,
    pub window_started_at_unix_ms: u64,
    pub receipt_count: usize,
    pub receipt_persistence_enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub receipt_persistence_error: Option<String>,
    #[serde(default)]
    pub pylons_online_now: u64,
    #[serde(default)]
    pub pylons_seen_24h: u64,
    #[serde(default)]
    pub pylon_sessions_online_now: u64,
    #[serde(default)]
    pub sellable_pylons_online_now: u64,
    #[serde(default)]
    pub inference_ready_pylons_online_now: u64,
    #[serde(default)]
    pub inference_ready_pylon_sessions_online_now: u64,
    #[serde(default)]
    pub pylon_reported_hosts_online_now: u64,
    #[serde(default)]
    pub pylon_sessions_missing_host_fingerprint_online_now: u64,
    #[serde(default)]
    pub likely_same_host_pylon_sessions_online_now: u64,
    #[serde(default)]
    pub likely_same_host_pylons_online_now: u64,
    #[serde(default)]
    pub pylon_presence_stale_after_ms: u64,
    pub sessions_active: usize,
    pub sessions_issued_24h: u64,
    pub sync_tokens_active: usize,
    pub sync_tokens_issued_24h: u64,
    pub starter_demand_budget_cap_sats: u64,
    pub starter_demand_budget_allocated_sats: u64,
    pub starter_offers_waiting_ack: usize,
    pub starter_offers_running: usize,
    pub starter_offers_dispatched_24h: u64,
    pub starter_offers_started_24h: u64,
    pub starter_offer_heartbeats_24h: u64,
    pub starter_offers_completed_24h: u64,
    pub starter_offers_released_24h: u64,
    pub starter_offers_expired_24h: u64,
    pub starter_demand_ineligible_polls_24h: u64,
    pub starter_offer_start_rate_24h: f64,
    pub starter_offer_completion_rate_24h: f64,
    pub starter_offer_loss_rate_24h: f64,
    pub starter_demand_paid_sats_24h: u64,
    pub starter_demand_released_sats_24h: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nexus_wallet_runtime_status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nexus_wallet_last_error: Option<String>,
    #[serde(default = "default_wallet_storage_runtime_mode")]
    pub nexus_wallet_storage_runtime_mode: String,
    #[serde(default)]
    pub nexus_wallet_balance_sats: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nexus_wallet_balance_updated_at_unix_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nexus_treasury_snapshot_generated_at_unix_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nexus_treasury_snapshot_age_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nexus_wallet_sync_lag_ms: Option<u64>,
    #[serde(default)]
    pub nexus_payout_loop_health: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nexus_treasury_degraded_reason: Option<String>,
    #[serde(default)]
    pub nexus_treasury_enabled: bool,
    #[serde(default)]
    pub nexus_treasury_payout_sats_per_window: u64,
    #[serde(default)]
    pub nexus_treasury_payout_interval_seconds: u64,
    #[serde(default)]
    pub nexus_treasury_require_sellable: bool,
    #[serde(default)]
    pub nexus_treasury_daily_budget_cap_sats: u64,
    #[serde(default)]
    pub nexus_placeholder_payout_mode: TreasuryPlaceholderPayoutMode,
    #[serde(default)]
    pub nexus_registered_payout_identities: u64,
    pub nexus_payout_sats_paid_total: u64,
    pub nexus_payout_sats_paid_24h: u64,
    #[serde(default)]
    pub nexus_accepted_work_payout_sats_paid_total: u64,
    #[serde(default)]
    pub nexus_accepted_work_payout_sats_paid_24h: u64,
    #[serde(default)]
    pub nexus_availability_stipend_payout_sats_paid_total: u64,
    #[serde(default)]
    pub nexus_availability_stipend_payout_sats_paid_24h: u64,
    #[serde(default)]
    pub nexus_placeholder_payout_sats_paid_total: u64,
    #[serde(default)]
    pub nexus_placeholder_payout_sats_paid_24h: u64,
    #[serde(default)]
    pub nexus_beta_bonus_payout_sats_paid_total: u64,
    #[serde(default)]
    pub nexus_beta_bonus_payout_sats_paid_24h: u64,
    #[serde(default)]
    pub nexus_weak_device_accepted_work_payout_sats_paid_total: u64,
    #[serde(default)]
    pub nexus_weak_device_accepted_work_payout_sats_paid_24h: u64,
    #[serde(default)]
    pub nexus_strong_lane_accepted_work_payout_sats_paid_total: u64,
    #[serde(default)]
    pub nexus_strong_lane_accepted_work_payout_sats_paid_24h: u64,
    #[serde(default)]
    pub nexus_payouts_dispatched_24h: u64,
    #[serde(default)]
    pub nexus_payouts_confirmed_24h: u64,
    #[serde(default)]
    pub nexus_payouts_failed_24h: u64,
    #[serde(default)]
    pub nexus_payouts_skipped_24h: u64,
    #[serde(default)]
    pub nexus_availability_online_identities_now: u64,
    #[serde(default)]
    pub nexus_availability_online_host_clusters_now: u64,
    #[serde(default)]
    pub nexus_availability_stipend_eligible_beneficiaries_now: u64,
    #[serde(default)]
    pub nexus_placeholder_payout_eligible_online_targets: u64,
    #[serde(default)]
    pub nexus_inference_ready_online_payout_targets: u64,
    #[serde(default)]
    pub nexus_duplicate_host_placeholder_blocked_online_targets: u64,
    #[serde(default)]
    pub nexus_duplicate_host_blocked_beneficiaries_now: u64,
    #[serde(default)]
    pub nexus_duplicate_payout_target_blocked_beneficiaries_now: u64,
    #[serde(default)]
    pub nexus_missing_payout_target_blocked_beneficiaries_now: u64,
    #[serde(default)]
    pub nexus_version_floor_blocked_beneficiaries_now: u64,
    #[serde(default)]
    pub nexus_readiness_blocked_beneficiaries_now: u64,
    #[serde(default)]
    pub training_nodes_admitted: u64,
    #[serde(default)]
    pub training_admitted_contributors: u64,
    #[serde(default)]
    pub training_assigned_contributors: u64,
    #[serde(default)]
    pub training_accepted_contributors: u64,
    #[serde(default)]
    pub training_model_progress_contributors: u64,
    #[serde(default)]
    pub training_weak_device_assigned_contributors: u64,
    #[serde(default)]
    pub training_weak_device_accepted_contributors: u64,
    #[serde(default)]
    pub training_nodes_online: u64,
    #[serde(default)]
    pub training_admitted_nodes_online: u64,
    #[serde(default)]
    pub homework_worker_eligible_pylons_online_now: u64,
    #[serde(default)]
    pub training_runs_active: u64,
    #[serde(default)]
    pub training_windows_active: u64,
    #[serde(default)]
    pub training_windows_pending_validation: u64,
    #[serde(default)]
    pub training_validator_challenges_open: u64,
    #[serde(default)]
    pub training_validator_challenges_queued: u64,
    #[serde(default)]
    pub training_nodes_contributing_to_accepted_progress: u64,
    #[serde(default)]
    pub training_runs_with_accepted_progress: u64,
    #[serde(default)]
    pub training_windows_advanced_checkpoint_lineage: u64,
    #[serde(default)]
    pub training_accepted_closeouts: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub training_checkpoint_max_age_ms: Option<u64>,
    #[serde(default)]
    pub training_artifact_failures_open: u64,
    #[serde(default)]
    pub training_payout_eligible_closeouts: u64,
    #[serde(default)]
    pub training_public_state: PublicTrainingStatsSnapshot,
    pub compute_products_active: u64,
    pub compute_capacity_lots_open: u64,
    pub compute_capacity_lots_delivering: u64,
    pub compute_instruments_active: u64,
    pub compute_inventory_quantity_open: u64,
    pub compute_inventory_quantity_reserved: u64,
    pub compute_inventory_quantity_delivering: u64,
    pub compute_delivery_proofs_24h: u64,
    pub compute_delivery_quantity_24h: u64,
    pub compute_delivery_rejections_24h: u64,
    pub compute_delivery_variances_24h: u64,
    pub compute_validator_challenges_open: u64,
    pub compute_validator_challenges_queued: u64,
    pub compute_validator_challenges_verified_24h: u64,
    pub compute_validator_challenges_rejected_24h: u64,
    pub compute_validator_challenges_timed_out_24h: u64,
    pub compute_delivery_accept_rate_24h: f64,
    pub compute_fill_ratio_24h: f64,
    pub compute_priced_instruments_24h: u64,
    pub compute_indices_published_24h: u64,
    pub compute_index_corrections_24h: u64,
    pub compute_index_thin_windows_24h: u64,
    pub compute_index_settlement_eligible_24h: u64,
    pub compute_index_quality_score_24h: f64,
    pub compute_active_provider_count: u64,
    pub compute_provider_concentration_hhi: f64,
    pub compute_forward_physical_instruments_active: u64,
    pub compute_forward_physical_open_quantity: u64,
    pub compute_forward_physical_defaults_24h: u64,
    pub compute_future_cash_instruments_active: u64,
    pub compute_future_cash_open_interest: u64,
    pub compute_future_cash_cash_settlements_24h: u64,
    pub compute_future_cash_cash_flow_24h: u64,
    pub compute_future_cash_defaults_24h: u64,
    pub compute_future_cash_collateral_shortfall_24h: u64,
    pub compute_structured_instruments_active: u64,
    pub compute_structured_instruments_closed_24h: u64,
    pub compute_max_buyer_concentration_share: f64,
    pub compute_paper_to_physical_ratio: f64,
    pub compute_deliverable_coverage_ratio: f64,
    pub compute_breakers_tripped: u64,
    pub compute_breakers_guarded: u64,
    pub compute_breaker_states: Vec<ComputeBreakerStatusRow>,
    pub compute_rollout_gates: Vec<ComputeRolloutGateRow>,
    pub compute_truth_labels: Vec<ComputeTruthLabelRow>,
    pub compute_reconciliation_gap_24h: u64,
    pub compute_policy_bundle_id: String,
    pub compute_policy_version: String,
    pub liquidity_quotes_active: u64,
    pub liquidity_route_plans_active: u64,
    pub liquidity_envelopes_open: u64,
    pub liquidity_settlements_24h: u64,
    pub liquidity_reserve_partitions_active: u64,
    pub liquidity_value_moved_24h: u64,
    pub risk_coverage_offers_open: u64,
    pub risk_coverage_bindings_active: u64,
    pub risk_prediction_positions_open: u64,
    pub risk_claims_open: u64,
    pub risk_signals_active: u64,
    pub risk_implied_fail_probability_bps: u32,
    pub risk_calibration_score: f64,
    pub risk_coverage_concentration_hhi: f64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub recent_pylons: Vec<PublicRecentPylon>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pylon_client_version_counts: Vec<PublicPylonClientVersionCount>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub homework_worker_eligible_pylon_version_counts: Vec<PublicPylonClientVersionCount>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub homework_worker_presence_only_blocker_counts:
        Vec<PublicHomeworkWorkerPresenceOnlyBlockerCount>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub recent_pylon_diagnostics: Vec<PublicRecentPylonDiagnostic>,
    pub recent_receipts: Vec<PublicRecentReceipt>,
}

#[derive(Debug, Clone, Default)]
pub struct PublicRuntimeSnapshot {
    pub hosted_nexus_relay_url: String,
    pub pylons_online_now: u64,
    pub pylons_seen_24h: u64,
    pub pylon_sessions_online_now: u64,
    pub sellable_pylons_online_now: u64,
    pub inference_ready_pylons_online_now: u64,
    pub inference_ready_pylon_sessions_online_now: u64,
    pub pylon_reported_hosts_online_now: u64,
    pub pylon_sessions_missing_host_fingerprint_online_now: u64,
    pub likely_same_host_pylon_sessions_online_now: u64,
    pub likely_same_host_pylons_online_now: u64,
    pub pylon_presence_stale_after_ms: u64,
    pub sessions_active: usize,
    pub sync_tokens_active: usize,
    pub starter_demand_budget_cap_sats: u64,
    pub starter_demand_budget_allocated_sats: u64,
    pub starter_offers_waiting_ack: usize,
    pub starter_offers_running: usize,
    pub nexus_wallet_runtime_status: Option<String>,
    pub nexus_wallet_last_error: Option<String>,
    pub nexus_wallet_storage_runtime_mode: String,
    pub nexus_wallet_balance_sats: u64,
    pub nexus_wallet_balance_updated_at_unix_ms: Option<u64>,
    pub nexus_treasury_snapshot_generated_at_unix_ms: Option<u64>,
    pub nexus_treasury_snapshot_age_ms: Option<u64>,
    pub nexus_wallet_sync_lag_ms: Option<u64>,
    pub nexus_payout_loop_health: String,
    pub nexus_treasury_degraded_reason: Option<String>,
    pub nexus_treasury_enabled: bool,
    pub nexus_treasury_payout_sats_per_window: u64,
    pub nexus_treasury_payout_interval_seconds: u64,
    pub nexus_treasury_require_sellable: bool,
    pub nexus_treasury_daily_budget_cap_sats: u64,
    pub nexus_placeholder_payout_mode: TreasuryPlaceholderPayoutMode,
    pub nexus_registered_payout_identities: u64,
    pub nexus_payout_sats_paid_total: u64,
    pub nexus_payout_sats_paid_24h: u64,
    pub nexus_accepted_work_payout_sats_paid_total: u64,
    pub nexus_accepted_work_payout_sats_paid_24h: u64,
    pub nexus_availability_stipend_payout_sats_paid_total: u64,
    pub nexus_availability_stipend_payout_sats_paid_24h: u64,
    pub nexus_placeholder_payout_sats_paid_total: u64,
    pub nexus_placeholder_payout_sats_paid_24h: u64,
    pub nexus_beta_bonus_payout_sats_paid_total: u64,
    pub nexus_beta_bonus_payout_sats_paid_24h: u64,
    pub nexus_weak_device_accepted_work_payout_sats_paid_total: u64,
    pub nexus_weak_device_accepted_work_payout_sats_paid_24h: u64,
    pub nexus_strong_lane_accepted_work_payout_sats_paid_total: u64,
    pub nexus_strong_lane_accepted_work_payout_sats_paid_24h: u64,
    pub nexus_payouts_dispatched_24h: u64,
    pub nexus_payouts_confirmed_24h: u64,
    pub nexus_payouts_failed_24h: u64,
    pub nexus_payouts_skipped_24h: u64,
    pub nexus_availability_online_identities_now: u64,
    pub nexus_availability_online_host_clusters_now: u64,
    pub nexus_availability_stipend_eligible_beneficiaries_now: u64,
    pub nexus_placeholder_payout_eligible_online_targets: u64,
    pub nexus_inference_ready_online_payout_targets: u64,
    pub nexus_duplicate_host_placeholder_blocked_online_targets: u64,
    pub nexus_duplicate_host_blocked_beneficiaries_now: u64,
    pub nexus_duplicate_payout_target_blocked_beneficiaries_now: u64,
    pub nexus_missing_payout_target_blocked_beneficiaries_now: u64,
    pub nexus_version_floor_blocked_beneficiaries_now: u64,
    pub nexus_readiness_blocked_beneficiaries_now: u64,
    pub training_nodes_admitted: u64,
    pub training_admitted_contributors: u64,
    pub training_assigned_contributors: u64,
    pub training_accepted_contributors: u64,
    pub training_model_progress_contributors: u64,
    pub training_weak_device_assigned_contributors: u64,
    pub training_weak_device_accepted_contributors: u64,
    pub training_nodes_online: u64,
    pub training_admitted_nodes_online: u64,
    pub homework_worker_eligible_pylons_online_now: u64,
    pub training_runs_active: u64,
    pub training_windows_active: u64,
    pub training_windows_pending_validation: u64,
    pub training_validator_challenges_open: u64,
    pub training_validator_challenges_queued: u64,
    pub training_nodes_contributing_to_accepted_progress: u64,
    pub training_runs_with_accepted_progress: u64,
    pub training_windows_advanced_checkpoint_lineage: u64,
    pub training_accepted_closeouts: u64,
    pub training_checkpoint_max_age_ms: Option<u64>,
    pub training_artifact_failures_open: u64,
    pub training_payout_eligible_closeouts: u64,
    pub training_public_state: PublicTrainingStatsSnapshot,
    pub compute_products_active: u64,
    pub compute_capacity_lots_open: u64,
    pub compute_capacity_lots_delivering: u64,
    pub compute_instruments_active: u64,
    pub compute_inventory_quantity_open: u64,
    pub compute_inventory_quantity_reserved: u64,
    pub compute_inventory_quantity_delivering: u64,
    pub compute_delivery_proofs_24h: u64,
    pub compute_delivery_quantity_24h: u64,
    pub compute_delivery_rejections_24h: u64,
    pub compute_delivery_variances_24h: u64,
    pub compute_validator_challenges_open: u64,
    pub compute_validator_challenges_queued: u64,
    pub compute_validator_challenges_verified_24h: u64,
    pub compute_validator_challenges_rejected_24h: u64,
    pub compute_validator_challenges_timed_out_24h: u64,
    pub compute_delivery_accept_rate_24h: f64,
    pub compute_fill_ratio_24h: f64,
    pub compute_priced_instruments_24h: u64,
    pub compute_indices_published_24h: u64,
    pub compute_index_corrections_24h: u64,
    pub compute_index_thin_windows_24h: u64,
    pub compute_index_settlement_eligible_24h: u64,
    pub compute_index_quality_score_24h: f64,
    pub compute_active_provider_count: u64,
    pub compute_provider_concentration_hhi: f64,
    pub compute_forward_physical_instruments_active: u64,
    pub compute_forward_physical_open_quantity: u64,
    pub compute_forward_physical_defaults_24h: u64,
    pub compute_future_cash_instruments_active: u64,
    pub compute_future_cash_open_interest: u64,
    pub compute_future_cash_cash_settlements_24h: u64,
    pub compute_future_cash_cash_flow_24h: u64,
    pub compute_future_cash_defaults_24h: u64,
    pub compute_future_cash_collateral_shortfall_24h: u64,
    pub compute_structured_instruments_active: u64,
    pub compute_structured_instruments_closed_24h: u64,
    pub compute_max_buyer_concentration_share: f64,
    pub compute_paper_to_physical_ratio: f64,
    pub compute_deliverable_coverage_ratio: f64,
    pub compute_breakers_tripped: u64,
    pub compute_breakers_guarded: u64,
    pub compute_breaker_states: Vec<ComputeBreakerStatusRow>,
    pub compute_rollout_gates: Vec<ComputeRolloutGateRow>,
    pub compute_truth_labels: Vec<ComputeTruthLabelRow>,
    pub compute_reconciliation_gap_24h: u64,
    pub compute_policy_bundle_id: String,
    pub compute_policy_version: String,
    pub liquidity_quotes_active: u64,
    pub liquidity_route_plans_active: u64,
    pub liquidity_envelopes_open: u64,
    pub liquidity_settlements_24h: u64,
    pub liquidity_reserve_partitions_active: u64,
    pub liquidity_value_moved_24h: u64,
    pub risk_coverage_offers_open: u64,
    pub risk_coverage_bindings_active: u64,
    pub risk_prediction_positions_open: u64,
    pub risk_claims_open: u64,
    pub risk_signals_active: u64,
    pub risk_implied_fail_probability_bps: u32,
    pub risk_calibration_score: f64,
    pub risk_coverage_concentration_hhi: f64,
    pub recent_pylons: Vec<PublicRecentPylon>,
    pub pylon_client_version_counts: Vec<PublicPylonClientVersionCount>,
    pub homework_worker_eligible_pylon_version_counts: Vec<PublicPylonClientVersionCount>,
    pub homework_worker_presence_only_blocker_counts:
        Vec<PublicHomeworkWorkerPresenceOnlyBlockerCount>,
    pub recent_pylon_diagnostics: Vec<PublicRecentPylonDiagnostic>,
}

#[derive(Debug, Clone, Default)]
pub struct ReceiptLedger {
    next_receipt_seq: u64,
    receipts: Vec<AuthorityReceipt>,
    receipt_log_path: Option<PathBuf>,
    last_persistence_error: Option<String>,
    treasury_confirmed_payout_request_ids: HashSet<String>,
    treasury_confirmed_payout_sats_total: u64,
}

impl ReceiptLedger {
    pub fn new(receipt_log_path: Option<PathBuf>) -> Self {
        let mut ledger = Self {
            next_receipt_seq: 1,
            receipts: Vec::new(),
            receipt_log_path,
            last_persistence_error: None,
            treasury_confirmed_payout_request_ids: HashSet::new(),
            treasury_confirmed_payout_sats_total: 0,
        };
        ledger.load_existing_receipts();
        ledger
    }

    pub fn record(
        &mut self,
        receipt_type: impl Into<String>,
        recorded_at_unix_ms: u64,
        context: AuthorityReceiptContext,
    ) -> AuthorityReceipt {
        let seq = self.next_receipt_seq;
        self.next_receipt_seq = self.next_receipt_seq.saturating_add(1);
        let receipt = AuthorityReceipt {
            seq,
            receipt_id: format!("nexus-receipt-{seq:08}"),
            receipt_type: receipt_type.into(),
            recorded_at_unix_ms,
            authority: AUTHORITY_NAME.to_string(),
            context,
        };
        self.note_treasury_confirmed_payout(&receipt);
        self.receipts.push(receipt.clone());
        self.trim_retention();
        self.append_receipt_to_log(&receipt);
        receipt
    }

    pub fn treasury_confirmed_payout_sats_total(&self) -> u64 {
        self.treasury_confirmed_payout_sats_total
    }

    pub fn snapshot(
        &self,
        runtime: &PublicRuntimeSnapshot,
        as_of_unix_ms: u64,
    ) -> PublicStatsSnapshot {
        let window_started_at_unix_ms = as_of_unix_ms.saturating_sub(PUBLIC_STATS_WINDOW_MS);
        let mut sessions_issued_24h = 0u64;
        let mut sync_tokens_issued_24h = 0u64;
        let mut starter_offers_dispatched_24h = 0u64;
        let mut starter_offers_started_24h = 0u64;
        let mut starter_offer_heartbeats_24h = 0u64;
        let mut starter_offers_completed_24h = 0u64;
        let mut starter_offers_released_24h = 0u64;
        let mut starter_offers_expired_24h = 0u64;
        let mut starter_demand_ineligible_polls_24h = 0u64;
        let mut starter_demand_paid_sats_24h = 0u64;
        let mut starter_demand_released_sats_24h = 0u64;

        for receipt in &self.receipts {
            if receipt.recorded_at_unix_ms < window_started_at_unix_ms {
                continue;
            }
            match receipt.receipt_type.as_str() {
                "desktop_session.created" => {
                    sessions_issued_24h = sessions_issued_24h.saturating_add(1);
                }
                "sync_token.issued" => {
                    sync_tokens_issued_24h = sync_tokens_issued_24h.saturating_add(1);
                }
                "starter_demand.ineligible" => {
                    starter_demand_ineligible_polls_24h =
                        starter_demand_ineligible_polls_24h.saturating_add(1);
                }
                "starter_offer.dispatched" => {
                    starter_offers_dispatched_24h = starter_offers_dispatched_24h.saturating_add(1);
                }
                "starter_offer.started" => {
                    starter_offers_started_24h = starter_offers_started_24h.saturating_add(1);
                }
                "starter_offer.heartbeat" => {
                    starter_offer_heartbeats_24h = starter_offer_heartbeats_24h.saturating_add(1);
                }
                "starter_offer.completed" => {
                    starter_offers_completed_24h = starter_offers_completed_24h.saturating_add(1);
                    starter_demand_paid_sats_24h = starter_demand_paid_sats_24h
                        .saturating_add(receipt.context.amount_sats.unwrap_or(0));
                }
                "starter_offer.released" => {
                    starter_offers_released_24h = starter_offers_released_24h.saturating_add(1);
                    starter_demand_released_sats_24h = starter_demand_released_sats_24h
                        .saturating_add(receipt.context.amount_sats.unwrap_or(0));
                }
                "starter_offer.expired" => {
                    starter_offers_expired_24h = starter_offers_expired_24h.saturating_add(1);
                    starter_demand_released_sats_24h = starter_demand_released_sats_24h
                        .saturating_add(receipt.context.amount_sats.unwrap_or(0));
                }
                _ => {}
            }
        }

        let starter_offer_start_rate_24h =
            ratio(starter_offers_started_24h, starter_offers_dispatched_24h);
        let starter_offer_completion_rate_24h =
            ratio(starter_offers_completed_24h, starter_offers_started_24h);
        let starter_offer_loss_rate_24h = ratio(
            starter_offers_released_24h.saturating_add(starter_offers_expired_24h),
            starter_offers_dispatched_24h,
        );

        PublicStatsSnapshot {
            service: "nexus-control".to_string(),
            authority: AUTHORITY_NAME.to_string(),
            hosted_nexus_relay_url: runtime.hosted_nexus_relay_url.clone(),
            as_of_unix_ms,
            window_started_at_unix_ms,
            receipt_count: self.receipts.len(),
            receipt_persistence_enabled: self.receipt_log_path.is_some(),
            receipt_persistence_error: self.last_persistence_error.clone(),
            pylons_online_now: runtime.pylons_online_now,
            pylons_seen_24h: runtime.pylons_seen_24h,
            pylon_sessions_online_now: runtime.pylon_sessions_online_now,
            sellable_pylons_online_now: runtime.sellable_pylons_online_now,
            inference_ready_pylons_online_now: runtime.inference_ready_pylons_online_now,
            inference_ready_pylon_sessions_online_now: runtime
                .inference_ready_pylon_sessions_online_now,
            pylon_reported_hosts_online_now: runtime.pylon_reported_hosts_online_now,
            pylon_sessions_missing_host_fingerprint_online_now: runtime
                .pylon_sessions_missing_host_fingerprint_online_now,
            likely_same_host_pylon_sessions_online_now: runtime
                .likely_same_host_pylon_sessions_online_now,
            likely_same_host_pylons_online_now: runtime.likely_same_host_pylons_online_now,
            pylon_client_version_counts: runtime.pylon_client_version_counts.clone(),
            homework_worker_eligible_pylon_version_counts: runtime
                .homework_worker_eligible_pylon_version_counts
                .clone(),
            homework_worker_presence_only_blocker_counts: runtime
                .homework_worker_presence_only_blocker_counts
                .clone(),
            pylon_presence_stale_after_ms: runtime.pylon_presence_stale_after_ms,
            sessions_active: runtime.sessions_active,
            sessions_issued_24h,
            sync_tokens_active: runtime.sync_tokens_active,
            sync_tokens_issued_24h,
            starter_demand_budget_cap_sats: runtime.starter_demand_budget_cap_sats,
            starter_demand_budget_allocated_sats: runtime.starter_demand_budget_allocated_sats,
            starter_offers_waiting_ack: runtime.starter_offers_waiting_ack,
            starter_offers_running: runtime.starter_offers_running,
            starter_offers_dispatched_24h,
            starter_offers_started_24h,
            starter_offer_heartbeats_24h,
            starter_offers_completed_24h,
            starter_offers_released_24h,
            starter_offers_expired_24h,
            starter_demand_ineligible_polls_24h,
            starter_offer_start_rate_24h,
            starter_offer_completion_rate_24h,
            starter_offer_loss_rate_24h,
            starter_demand_paid_sats_24h,
            starter_demand_released_sats_24h,
            nexus_wallet_runtime_status: runtime.nexus_wallet_runtime_status.clone(),
            nexus_wallet_last_error: runtime.nexus_wallet_last_error.clone(),
            nexus_wallet_storage_runtime_mode: runtime.nexus_wallet_storage_runtime_mode.clone(),
            nexus_wallet_balance_sats: runtime.nexus_wallet_balance_sats,
            nexus_wallet_balance_updated_at_unix_ms: runtime
                .nexus_wallet_balance_updated_at_unix_ms,
            nexus_treasury_snapshot_generated_at_unix_ms: runtime
                .nexus_treasury_snapshot_generated_at_unix_ms,
            nexus_treasury_snapshot_age_ms: runtime.nexus_treasury_snapshot_age_ms,
            nexus_wallet_sync_lag_ms: runtime.nexus_wallet_sync_lag_ms,
            nexus_payout_loop_health: runtime.nexus_payout_loop_health.clone(),
            nexus_treasury_degraded_reason: runtime.nexus_treasury_degraded_reason.clone(),
            nexus_treasury_enabled: runtime.nexus_treasury_enabled,
            nexus_treasury_payout_sats_per_window: runtime.nexus_treasury_payout_sats_per_window,
            nexus_treasury_payout_interval_seconds: runtime.nexus_treasury_payout_interval_seconds,
            nexus_treasury_require_sellable: runtime.nexus_treasury_require_sellable,
            nexus_treasury_daily_budget_cap_sats: runtime.nexus_treasury_daily_budget_cap_sats,
            nexus_placeholder_payout_mode: runtime.nexus_placeholder_payout_mode,
            nexus_registered_payout_identities: runtime.nexus_registered_payout_identities,
            nexus_payout_sats_paid_total: runtime.nexus_payout_sats_paid_total,
            nexus_payout_sats_paid_24h: runtime.nexus_payout_sats_paid_24h,
            nexus_accepted_work_payout_sats_paid_total: runtime
                .nexus_accepted_work_payout_sats_paid_total,
            nexus_accepted_work_payout_sats_paid_24h: runtime
                .nexus_accepted_work_payout_sats_paid_24h,
            nexus_availability_stipend_payout_sats_paid_total: runtime
                .nexus_availability_stipend_payout_sats_paid_total,
            nexus_availability_stipend_payout_sats_paid_24h: runtime
                .nexus_availability_stipend_payout_sats_paid_24h,
            nexus_placeholder_payout_sats_paid_total: runtime
                .nexus_placeholder_payout_sats_paid_total,
            nexus_placeholder_payout_sats_paid_24h: runtime.nexus_placeholder_payout_sats_paid_24h,
            nexus_beta_bonus_payout_sats_paid_total: runtime
                .nexus_beta_bonus_payout_sats_paid_total,
            nexus_beta_bonus_payout_sats_paid_24h: runtime.nexus_beta_bonus_payout_sats_paid_24h,
            nexus_weak_device_accepted_work_payout_sats_paid_total: runtime
                .nexus_weak_device_accepted_work_payout_sats_paid_total,
            nexus_weak_device_accepted_work_payout_sats_paid_24h: runtime
                .nexus_weak_device_accepted_work_payout_sats_paid_24h,
            nexus_strong_lane_accepted_work_payout_sats_paid_total: runtime
                .nexus_strong_lane_accepted_work_payout_sats_paid_total,
            nexus_strong_lane_accepted_work_payout_sats_paid_24h: runtime
                .nexus_strong_lane_accepted_work_payout_sats_paid_24h,
            nexus_payouts_dispatched_24h: runtime.nexus_payouts_dispatched_24h,
            nexus_payouts_confirmed_24h: runtime.nexus_payouts_confirmed_24h,
            nexus_payouts_failed_24h: runtime.nexus_payouts_failed_24h,
            nexus_payouts_skipped_24h: runtime.nexus_payouts_skipped_24h,
            nexus_availability_online_identities_now: runtime
                .nexus_availability_online_identities_now,
            nexus_availability_online_host_clusters_now: runtime
                .nexus_availability_online_host_clusters_now,
            nexus_availability_stipend_eligible_beneficiaries_now: runtime
                .nexus_availability_stipend_eligible_beneficiaries_now,
            nexus_placeholder_payout_eligible_online_targets: runtime
                .nexus_placeholder_payout_eligible_online_targets,
            nexus_inference_ready_online_payout_targets: runtime
                .nexus_inference_ready_online_payout_targets,
            nexus_duplicate_host_placeholder_blocked_online_targets: runtime
                .nexus_duplicate_host_placeholder_blocked_online_targets,
            nexus_duplicate_host_blocked_beneficiaries_now: runtime
                .nexus_duplicate_host_blocked_beneficiaries_now,
            nexus_duplicate_payout_target_blocked_beneficiaries_now: runtime
                .nexus_duplicate_payout_target_blocked_beneficiaries_now,
            nexus_missing_payout_target_blocked_beneficiaries_now: runtime
                .nexus_missing_payout_target_blocked_beneficiaries_now,
            nexus_version_floor_blocked_beneficiaries_now: runtime
                .nexus_version_floor_blocked_beneficiaries_now,
            nexus_readiness_blocked_beneficiaries_now: runtime
                .nexus_readiness_blocked_beneficiaries_now,
            training_nodes_admitted: runtime.training_nodes_admitted,
            training_admitted_contributors: runtime.training_admitted_contributors,
            training_assigned_contributors: runtime.training_assigned_contributors,
            training_accepted_contributors: runtime.training_accepted_contributors,
            training_model_progress_contributors: runtime.training_model_progress_contributors,
            training_weak_device_assigned_contributors: runtime
                .training_weak_device_assigned_contributors,
            training_weak_device_accepted_contributors: runtime
                .training_weak_device_accepted_contributors,
            training_nodes_online: runtime.training_nodes_online,
            training_admitted_nodes_online: runtime.training_admitted_nodes_online,
            homework_worker_eligible_pylons_online_now: runtime
                .homework_worker_eligible_pylons_online_now,
            training_runs_active: runtime.training_runs_active,
            training_windows_active: runtime.training_windows_active,
            training_windows_pending_validation: runtime.training_windows_pending_validation,
            training_validator_challenges_open: runtime.training_validator_challenges_open,
            training_validator_challenges_queued: runtime.training_validator_challenges_queued,
            training_nodes_contributing_to_accepted_progress: runtime
                .training_nodes_contributing_to_accepted_progress,
            training_runs_with_accepted_progress: runtime.training_runs_with_accepted_progress,
            training_windows_advanced_checkpoint_lineage: runtime
                .training_windows_advanced_checkpoint_lineage,
            training_accepted_closeouts: runtime.training_accepted_closeouts,
            training_checkpoint_max_age_ms: runtime.training_checkpoint_max_age_ms,
            training_artifact_failures_open: runtime.training_artifact_failures_open,
            training_payout_eligible_closeouts: runtime.training_payout_eligible_closeouts,
            training_public_state: runtime.training_public_state.clone(),
            compute_products_active: runtime.compute_products_active,
            compute_capacity_lots_open: runtime.compute_capacity_lots_open,
            compute_capacity_lots_delivering: runtime.compute_capacity_lots_delivering,
            compute_instruments_active: runtime.compute_instruments_active,
            compute_inventory_quantity_open: runtime.compute_inventory_quantity_open,
            compute_inventory_quantity_reserved: runtime.compute_inventory_quantity_reserved,
            compute_inventory_quantity_delivering: runtime.compute_inventory_quantity_delivering,
            compute_delivery_proofs_24h: runtime.compute_delivery_proofs_24h,
            compute_delivery_quantity_24h: runtime.compute_delivery_quantity_24h,
            compute_delivery_rejections_24h: runtime.compute_delivery_rejections_24h,
            compute_delivery_variances_24h: runtime.compute_delivery_variances_24h,
            compute_validator_challenges_open: runtime.compute_validator_challenges_open,
            compute_validator_challenges_queued: runtime.compute_validator_challenges_queued,
            compute_validator_challenges_verified_24h: runtime
                .compute_validator_challenges_verified_24h,
            compute_validator_challenges_rejected_24h: runtime
                .compute_validator_challenges_rejected_24h,
            compute_validator_challenges_timed_out_24h: runtime
                .compute_validator_challenges_timed_out_24h,
            compute_delivery_accept_rate_24h: runtime.compute_delivery_accept_rate_24h,
            compute_fill_ratio_24h: runtime.compute_fill_ratio_24h,
            compute_priced_instruments_24h: runtime.compute_priced_instruments_24h,
            compute_indices_published_24h: runtime.compute_indices_published_24h,
            compute_index_corrections_24h: runtime.compute_index_corrections_24h,
            compute_index_thin_windows_24h: runtime.compute_index_thin_windows_24h,
            compute_index_settlement_eligible_24h: runtime.compute_index_settlement_eligible_24h,
            compute_index_quality_score_24h: runtime.compute_index_quality_score_24h,
            compute_active_provider_count: runtime.compute_active_provider_count,
            compute_provider_concentration_hhi: runtime.compute_provider_concentration_hhi,
            compute_forward_physical_instruments_active: runtime
                .compute_forward_physical_instruments_active,
            compute_forward_physical_open_quantity: runtime.compute_forward_physical_open_quantity,
            compute_forward_physical_defaults_24h: runtime.compute_forward_physical_defaults_24h,
            compute_future_cash_instruments_active: runtime.compute_future_cash_instruments_active,
            compute_future_cash_open_interest: runtime.compute_future_cash_open_interest,
            compute_future_cash_cash_settlements_24h: runtime
                .compute_future_cash_cash_settlements_24h,
            compute_future_cash_cash_flow_24h: runtime.compute_future_cash_cash_flow_24h,
            compute_future_cash_defaults_24h: runtime.compute_future_cash_defaults_24h,
            compute_future_cash_collateral_shortfall_24h: runtime
                .compute_future_cash_collateral_shortfall_24h,
            compute_structured_instruments_active: runtime.compute_structured_instruments_active,
            compute_structured_instruments_closed_24h: runtime
                .compute_structured_instruments_closed_24h,
            compute_max_buyer_concentration_share: runtime.compute_max_buyer_concentration_share,
            compute_paper_to_physical_ratio: runtime.compute_paper_to_physical_ratio,
            compute_deliverable_coverage_ratio: runtime.compute_deliverable_coverage_ratio,
            compute_breakers_tripped: runtime.compute_breakers_tripped,
            compute_breakers_guarded: runtime.compute_breakers_guarded,
            compute_breaker_states: runtime.compute_breaker_states.clone(),
            compute_rollout_gates: runtime.compute_rollout_gates.clone(),
            compute_truth_labels: runtime.compute_truth_labels.clone(),
            compute_reconciliation_gap_24h: runtime.compute_reconciliation_gap_24h,
            compute_policy_bundle_id: runtime.compute_policy_bundle_id.clone(),
            compute_policy_version: runtime.compute_policy_version.clone(),
            liquidity_quotes_active: runtime.liquidity_quotes_active,
            liquidity_route_plans_active: runtime.liquidity_route_plans_active,
            liquidity_envelopes_open: runtime.liquidity_envelopes_open,
            liquidity_settlements_24h: runtime.liquidity_settlements_24h,
            liquidity_reserve_partitions_active: runtime.liquidity_reserve_partitions_active,
            liquidity_value_moved_24h: runtime.liquidity_value_moved_24h,
            risk_coverage_offers_open: runtime.risk_coverage_offers_open,
            risk_coverage_bindings_active: runtime.risk_coverage_bindings_active,
            risk_prediction_positions_open: runtime.risk_prediction_positions_open,
            risk_claims_open: runtime.risk_claims_open,
            risk_signals_active: runtime.risk_signals_active,
            risk_implied_fail_probability_bps: runtime.risk_implied_fail_probability_bps,
            risk_calibration_score: runtime.risk_calibration_score,
            risk_coverage_concentration_hhi: runtime.risk_coverage_concentration_hhi,
            recent_pylons: runtime.recent_pylons.clone(),
            recent_pylon_diagnostics: runtime.recent_pylon_diagnostics.clone(),
            recent_receipts: self.recent_receipts(),
        }
    }

    fn recent_receipts(&self) -> Vec<PublicRecentReceipt> {
        self.receipts
            .iter()
            .rev()
            .take(PUBLIC_RECENT_RECEIPT_LIMIT)
            .map(|receipt| PublicRecentReceipt {
                receipt_id: receipt.receipt_id.clone(),
                receipt_type: receipt.receipt_type.clone(),
                recorded_at_unix_ms: receipt.recorded_at_unix_ms,
                request_id: receipt.context.request_id.clone(),
                status: receipt.context.status.clone(),
                reason: receipt.context.reason.clone(),
                amount_sats: receipt.context.amount_sats,
            })
            .collect()
    }

    fn trim_retention(&mut self) {
        if self.receipts.len() > RECEIPT_RETENTION_LIMIT {
            let remove_count = self.receipts.len().saturating_sub(RECEIPT_RETENTION_LIMIT);
            self.receipts.drain(0..remove_count);
        }
    }

    fn load_existing_receipts(&mut self) {
        let Some(path) = self.receipt_log_path.clone() else {
            return;
        };
        if !path.exists() {
            return;
        }
        let Ok(contents) = fs::read_to_string(path.as_path()) else {
            self.last_persistence_error =
                Some(format!("failed_to_read_receipt_log:{}", path.display()));
            return;
        };
        for (index, line) in contents.lines().enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            match serde_json::from_str::<AuthorityReceipt>(line) {
                Ok(receipt) => {
                    self.next_receipt_seq =
                        self.next_receipt_seq.max(receipt.seq.saturating_add(1));
                    self.note_treasury_confirmed_payout(&receipt);
                    self.receipts.push(receipt);
                }
                Err(error) => {
                    self.last_persistence_error = Some(format!(
                        "failed_to_parse_receipt_log_line:{}:{}",
                        index.saturating_add(1),
                        error
                    ));
                }
            }
        }
        self.trim_retention();
    }

    fn append_receipt_to_log(&mut self, receipt: &AuthorityReceipt) {
        let Some(path) = self.receipt_log_path.clone() else {
            return;
        };
        if let Some(parent) = parent_directory(path.as_path())
            && let Err(error) = fs::create_dir_all(parent)
        {
            self.last_persistence_error = Some(format!(
                "failed_to_create_receipt_log_parent:{}:{}",
                parent.display(),
                error
            ));
            return;
        }
        let file_result = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path.as_path());
        let mut file = match file_result {
            Ok(file) => file,
            Err(error) => {
                self.last_persistence_error = Some(format!(
                    "failed_to_open_receipt_log:{}:{}",
                    path.display(),
                    error
                ));
                return;
            }
        };
        match serde_json::to_vec(receipt) {
            Ok(serialized) => {
                if let Err(error) = file.write_all(serialized.as_slice()) {
                    self.last_persistence_error = Some(format!(
                        "failed_to_append_receipt_log:{}:{}",
                        path.display(),
                        error
                    ));
                    return;
                }
                if let Err(error) = file.write_all(b"\n") {
                    self.last_persistence_error = Some(format!(
                        "failed_to_terminate_receipt_log_line:{}:{}",
                        path.display(),
                        error
                    ));
                    return;
                }
                self.last_persistence_error = None;
            }
            Err(error) => {
                self.last_persistence_error = Some(format!(
                    "failed_to_serialize_receipt:{}:{error}",
                    receipt.receipt_id
                ));
            }
        }
    }

    fn note_treasury_confirmed_payout(&mut self, receipt: &AuthorityReceipt) {
        if receipt.receipt_type != "treasury.payout.confirmed" {
            return;
        }
        let request_id = receipt
            .context
            .request_id
            .clone()
            .unwrap_or_else(|| receipt.receipt_id.clone());
        if self
            .treasury_confirmed_payout_request_ids
            .insert(request_id)
        {
            self.treasury_confirmed_payout_sats_total = self
                .treasury_confirmed_payout_sats_total
                .saturating_add(receipt.context.amount_sats.unwrap_or(0));
        }
    }
}

fn parent_directory(path: &Path) -> Option<&Path> {
    path.parent()
        .filter(|parent| !parent.as_os_str().is_empty())
}

fn ratio(numerator: u64, denominator: u64) -> f64 {
    if denominator == 0 {
        0.0
    } else {
        numerator as f64 / denominator as f64
    }
}

#[cfg(test)]
mod tests {
    use super::{
        AuthorityReceiptContext, PublicRuntimeSnapshot, PublicTrainingStatsSnapshot, ReceiptLedger,
        TreasuryPlaceholderPayoutMode,
    };

    #[test]
    fn snapshot_aggregates_receipts_by_type() {
        let mut ledger = ReceiptLedger::new(None);
        ledger.record(
            "desktop_session.created",
            1_000,
            AuthorityReceiptContext::default(),
        );
        ledger.record(
            "sync_token.issued",
            2_000,
            AuthorityReceiptContext::default(),
        );
        ledger.record(
            "starter_offer.dispatched",
            3_000,
            AuthorityReceiptContext {
                amount_sats: Some(120),
                ..AuthorityReceiptContext::default()
            },
        );
        ledger.record(
            "starter_offer.started",
            4_000,
            AuthorityReceiptContext::default(),
        );
        ledger.record(
            "starter_offer.completed",
            5_000,
            AuthorityReceiptContext {
                amount_sats: Some(120),
                ..AuthorityReceiptContext::default()
            },
        );
        let snapshot = ledger.snapshot(
            &PublicRuntimeSnapshot {
                hosted_nexus_relay_url: "wss://nexus.openagents.com/".to_string(),
                pylons_online_now: 0,
                pylons_seen_24h: 0,
                pylon_sessions_online_now: 0,
                sellable_pylons_online_now: 0,
                inference_ready_pylons_online_now: 0,
                inference_ready_pylon_sessions_online_now: 0,
                pylon_reported_hosts_online_now: 0,
                pylon_sessions_missing_host_fingerprint_online_now: 0,
                likely_same_host_pylon_sessions_online_now: 0,
                likely_same_host_pylons_online_now: 0,
                pylon_client_version_counts: Vec::new(),
                homework_worker_eligible_pylon_version_counts: Vec::new(),
                homework_worker_presence_only_blocker_counts: Vec::new(),
                pylon_presence_stale_after_ms: 0,
                sessions_active: 1,
                sync_tokens_active: 1,
                starter_demand_budget_cap_sats: 5_000,
                starter_demand_budget_allocated_sats: 0,
                starter_offers_waiting_ack: 0,
                starter_offers_running: 0,
                nexus_wallet_runtime_status: None,
                nexus_wallet_last_error: None,
                nexus_wallet_storage_runtime_mode: "original".to_string(),
                nexus_wallet_balance_sats: 0,
                nexus_wallet_balance_updated_at_unix_ms: None,
                nexus_treasury_snapshot_generated_at_unix_ms: None,
                nexus_treasury_snapshot_age_ms: None,
                nexus_wallet_sync_lag_ms: None,
                nexus_payout_loop_health: "disabled".to_string(),
                nexus_treasury_degraded_reason: None,
                nexus_treasury_enabled: false,
                nexus_treasury_payout_sats_per_window: 0,
                nexus_treasury_payout_interval_seconds: 0,
                nexus_treasury_require_sellable: false,
                nexus_treasury_daily_budget_cap_sats: 0,
                nexus_placeholder_payout_mode: TreasuryPlaceholderPayoutMode::default(),
                nexus_registered_payout_identities: 0,
                nexus_payout_sats_paid_total: 0,
                nexus_payout_sats_paid_24h: 0,
                nexus_accepted_work_payout_sats_paid_total: 0,
                nexus_accepted_work_payout_sats_paid_24h: 0,
                nexus_availability_stipend_payout_sats_paid_total: 0,
                nexus_availability_stipend_payout_sats_paid_24h: 0,
                nexus_placeholder_payout_sats_paid_total: 0,
                nexus_placeholder_payout_sats_paid_24h: 0,
                nexus_beta_bonus_payout_sats_paid_total: 0,
                nexus_beta_bonus_payout_sats_paid_24h: 0,
                nexus_weak_device_accepted_work_payout_sats_paid_total: 0,
                nexus_weak_device_accepted_work_payout_sats_paid_24h: 0,
                nexus_strong_lane_accepted_work_payout_sats_paid_total: 0,
                nexus_strong_lane_accepted_work_payout_sats_paid_24h: 0,
                nexus_payouts_dispatched_24h: 0,
                nexus_payouts_confirmed_24h: 0,
                nexus_payouts_failed_24h: 0,
                nexus_payouts_skipped_24h: 0,
                nexus_availability_online_identities_now: 0,
                nexus_availability_online_host_clusters_now: 0,
                nexus_availability_stipend_eligible_beneficiaries_now: 0,
                nexus_placeholder_payout_eligible_online_targets: 0,
                nexus_inference_ready_online_payout_targets: 0,
                nexus_duplicate_host_placeholder_blocked_online_targets: 0,
                nexus_duplicate_host_blocked_beneficiaries_now: 0,
                nexus_duplicate_payout_target_blocked_beneficiaries_now: 0,
                nexus_missing_payout_target_blocked_beneficiaries_now: 0,
                nexus_version_floor_blocked_beneficiaries_now: 0,
                nexus_readiness_blocked_beneficiaries_now: 0,
                training_nodes_admitted: 0,
                training_admitted_contributors: 0,
                training_assigned_contributors: 0,
                training_accepted_contributors: 0,
                training_model_progress_contributors: 0,
                training_weak_device_assigned_contributors: 0,
                training_weak_device_accepted_contributors: 0,
                training_nodes_online: 0,
                training_admitted_nodes_online: 0,
                homework_worker_eligible_pylons_online_now: 0,
                training_runs_active: 0,
                training_windows_active: 0,
                training_windows_pending_validation: 0,
                training_validator_challenges_open: 0,
                training_validator_challenges_queued: 0,
                training_nodes_contributing_to_accepted_progress: 0,
                training_runs_with_accepted_progress: 0,
                training_windows_advanced_checkpoint_lineage: 0,
                training_accepted_closeouts: 0,
                training_checkpoint_max_age_ms: None,
                training_artifact_failures_open: 0,
                training_payout_eligible_closeouts: 0,
                training_public_state: PublicTrainingStatsSnapshot::default(),
                compute_products_active: 0,
                compute_capacity_lots_open: 0,
                compute_capacity_lots_delivering: 0,
                compute_instruments_active: 0,
                compute_inventory_quantity_open: 0,
                compute_inventory_quantity_reserved: 0,
                compute_inventory_quantity_delivering: 0,
                compute_delivery_proofs_24h: 0,
                compute_delivery_quantity_24h: 0,
                compute_delivery_rejections_24h: 0,
                compute_delivery_variances_24h: 0,
                compute_validator_challenges_open: 0,
                compute_validator_challenges_queued: 0,
                compute_validator_challenges_verified_24h: 0,
                compute_validator_challenges_rejected_24h: 0,
                compute_validator_challenges_timed_out_24h: 0,
                compute_delivery_accept_rate_24h: 0.0,
                compute_fill_ratio_24h: 0.0,
                compute_priced_instruments_24h: 0,
                compute_indices_published_24h: 0,
                compute_index_corrections_24h: 0,
                compute_index_thin_windows_24h: 0,
                compute_index_settlement_eligible_24h: 0,
                compute_index_quality_score_24h: 0.0,
                compute_active_provider_count: 0,
                compute_provider_concentration_hhi: 0.0,
                compute_forward_physical_instruments_active: 0,
                compute_forward_physical_open_quantity: 0,
                compute_forward_physical_defaults_24h: 0,
                compute_future_cash_instruments_active: 0,
                compute_future_cash_open_interest: 0,
                compute_future_cash_cash_settlements_24h: 0,
                compute_future_cash_cash_flow_24h: 0,
                compute_future_cash_defaults_24h: 0,
                compute_future_cash_collateral_shortfall_24h: 0,
                compute_structured_instruments_active: 0,
                compute_structured_instruments_closed_24h: 0,
                compute_max_buyer_concentration_share: 0.0,
                compute_paper_to_physical_ratio: 0.0,
                compute_deliverable_coverage_ratio: 0.0,
                compute_breakers_tripped: 0,
                compute_breakers_guarded: 0,
                compute_breaker_states: Vec::new(),
                compute_rollout_gates: Vec::new(),
                compute_truth_labels: Vec::new(),
                compute_reconciliation_gap_24h: 0,
                compute_policy_bundle_id: String::new(),
                compute_policy_version: String::new(),
                liquidity_quotes_active: 0,
                liquidity_route_plans_active: 0,
                liquidity_envelopes_open: 0,
                liquidity_settlements_24h: 0,
                liquidity_reserve_partitions_active: 0,
                liquidity_value_moved_24h: 0,
                risk_coverage_offers_open: 0,
                risk_coverage_bindings_active: 0,
                risk_prediction_positions_open: 0,
                risk_claims_open: 0,
                risk_signals_active: 0,
                risk_implied_fail_probability_bps: 0,
                risk_calibration_score: 0.0,
                risk_coverage_concentration_hhi: 0.0,
                recent_pylons: Vec::new(),
                recent_pylon_diagnostics: Vec::new(),
            },
            6_000,
        );
        assert_eq!(snapshot.sessions_issued_24h, 1);
        assert_eq!(snapshot.sync_tokens_issued_24h, 1);
        assert_eq!(snapshot.starter_offers_dispatched_24h, 1);
        assert_eq!(snapshot.starter_offers_started_24h, 1);
        assert_eq!(snapshot.starter_offers_completed_24h, 1);
        assert_eq!(snapshot.starter_demand_paid_sats_24h, 120);
        assert_eq!(snapshot.starter_offer_start_rate_24h, 1.0);
        assert_eq!(snapshot.starter_offer_completion_rate_24h, 1.0);
    }
}
