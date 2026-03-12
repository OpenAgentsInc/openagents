//! Relay/sync/network/starter-job pane state extracted from `app_state.rs`.

use std::collections::HashSet;
use std::time::Instant;

use crate::app_state::{PaneLoadState, PaneStatusAccess};
use crate::bitcoin_display::format_sats_amount;
use crate::nip90_compute_domain_events;
use crate::nip90_compute_semantics::{
    BuyerInvoiceAmountAnalysis as SharedBuyerInvoiceAmountAnalysis,
    BuyerProviderObservation as SharedBuyerProviderObservation, analyze_invoice_amount_msats,
    normalize_pubkey, provider_has_non_error_result as shared_provider_has_non_error_result,
    provider_has_payable_result as shared_provider_has_payable_result,
    select_budget_approved_payable_winner as shared_select_budget_approved_payable_winner,
};
use crate::runtime_lanes::RuntimeCommandResponse;
use crate::sync_lifecycle::{RuntimeSyncConnectionState, RuntimeSyncHealthSnapshot};
use openagents_kernel_core::compute::{ComputeBackendFamily, ComputeFamily};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RelayConnectionStatus {
    Connected,
    Connecting,
    Disconnected,
    Error,
}

impl RelayConnectionStatus {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Connected => "connected",
            Self::Connecting => "connecting",
            Self::Disconnected => "disconnected",
            Self::Error => "error",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RelayConnectionRow {
    pub url: String,
    pub status: RelayConnectionStatus,
    pub latency_ms: Option<u32>,
    pub last_seen_seconds_ago: Option<u64>,
    pub last_error: Option<String>,
}

pub struct RelayConnectionsState {
    pub load_state: PaneLoadState,
    pub last_error: Option<String>,
    pub last_action: Option<String>,
    pub relays: Vec<RelayConnectionRow>,
    pub selected_url: Option<String>,
}

impl Default for RelayConnectionsState {
    fn default() -> Self {
        Self {
            load_state: PaneLoadState::Loading,
            last_error: None,
            last_action: Some("Waiting for relay lane snapshot".to_string()),
            relays: Vec::new(),
            selected_url: None,
        }
    }
}

impl RelayConnectionsState {
    pub fn replace_configured_relays(&mut self, relay_urls: &[String]) {
        let previous_selected = self.selected_url.clone();
        self.relays = relay_urls
            .iter()
            .map(|relay_url| RelayConnectionRow {
                url: relay_url.clone(),
                status: RelayConnectionStatus::Disconnected,
                latency_ms: None,
                last_seen_seconds_ago: None,
                last_error: None,
            })
            .collect();
        self.selected_url = previous_selected.filter(|selected| {
            self.relays
                .iter()
                .any(|relay| relay.url.as_str() == selected.as_str())
        });
        if self.selected_url.is_none() {
            self.selected_url = self.relays.first().map(|relay| relay.url.clone());
        }
        self.pane_set_ready("Loaded configured relay bundle");
    }

    pub fn select_by_index(&mut self, index: usize) -> bool {
        let Some(url) = self.relays.get(index).map(|row| row.url.clone()) else {
            return false;
        };
        self.selected_url = Some(url);
        self.pane_clear_error();
        true
    }

    pub fn selected(&self) -> Option<&RelayConnectionRow> {
        let selected = self.selected_url.as_deref()?;
        self.relays.iter().find(|row| row.url == selected)
    }

    pub fn add_relay(&mut self, relay_url: &str) -> Result<(), String> {
        let relay = relay_url.trim();
        if relay.is_empty() {
            return Err(self.pane_set_error("Relay URL cannot be empty"));
        }
        if !relay.starts_with("wss://") {
            return Err(self.pane_set_error("Relay URL must start with wss://"));
        }
        if let Some(existing) = self.relays.iter_mut().find(|row| row.url == relay) {
            existing.status = RelayConnectionStatus::Error;
            existing.last_error = Some("Relay already configured".to_string());
            return Err(self.pane_set_error("Relay already configured"));
        }

        let relay_url = relay.to_string();
        self.relays.push(RelayConnectionRow {
            url: relay_url.clone(),
            status: RelayConnectionStatus::Connecting,
            latency_ms: None,
            last_seen_seconds_ago: None,
            last_error: None,
        });
        self.selected_url = Some(relay_url.clone());
        self.pane_set_ready(format!("Added relay {relay_url}"));
        Ok(())
    }

    pub fn remove_selected(&mut self) -> Result<String, String> {
        let selected = self
            .selected_url
            .as_deref()
            .ok_or_else(|| "Select a relay first".to_string())?
            .to_string();
        if let Some(relay) = self.relays.iter_mut().find(|row| row.url == selected) {
            relay.status = RelayConnectionStatus::Disconnected;
        }
        let before = self.relays.len();
        self.relays.retain(|row| row.url != selected);
        if self.relays.len() == before {
            return Err(self.pane_set_error("Selected relay no longer exists"));
        }

        self.selected_url = self.relays.first().map(|row| row.url.clone());
        self.pane_set_ready(format!("Removed relay {selected}"));
        Ok(selected)
    }

    pub fn retry_selected(&mut self) -> Result<String, String> {
        let selected = self
            .selected_url
            .as_deref()
            .ok_or_else(|| "Select a relay first".to_string())?
            .to_string();
        let Some(relay) = self.relays.iter_mut().find(|row| row.url == selected) else {
            return Err(self.pane_set_error("Selected relay no longer exists"));
        };

        relay.status = RelayConnectionStatus::Connecting;
        relay.latency_ms = None;
        relay.last_seen_seconds_ago = None;
        relay.last_error = None;
        self.pane_set_ready(format!("Retried relay {selected}"));
        Ok(selected)
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SyncRecoveryPhase {
    Idle,
    Reconnecting,
    Replaying,
    Ready,
}

impl SyncRecoveryPhase {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Reconnecting => "reconnecting",
            Self::Replaying => "replaying",
            Self::Ready => "ready",
        }
    }
}

pub struct SyncHealthState {
    pub load_state: PaneLoadState,
    pub last_error: Option<String>,
    pub last_action: Option<String>,
    pub source_tag: String,
    pub spacetime_connection: String,
    pub subscription_state: String,
    pub reconnect_posture: String,
    pub stale_cursor_reason: Option<String>,
    pub cursor_position: u64,
    pub cursor_target_position: u64,
    pub cursor_stale_after_seconds: u64,
    pub cursor_last_advanced_seconds_ago: u64,
    pub recovery_phase: SyncRecoveryPhase,
    pub last_applied_event_seq: u64,
    pub duplicate_drop_count: u64,
    pub replay_count: u32,
    pub replay_progress_percent: Option<u8>,
    pub replay_lag_seq: Option<u64>,
    pub next_retry_ms: Option<u64>,
    pub token_refresh_after_in_seconds: Option<u64>,
    pub disconnect_reason: Option<String>,
    cursor_last_advanced_at: Option<Instant>,
}

impl Default for SyncHealthState {
    fn default() -> Self {
        Self {
            load_state: PaneLoadState::Loading,
            last_error: None,
            last_action: Some("Waiting for sync lane telemetry".to_string()),
            source_tag: "spacetime.sync.lifecycle".to_string(),
            spacetime_connection: "unknown".to_string(),
            subscription_state: "unsubscribed".to_string(),
            reconnect_posture: "none".to_string(),
            stale_cursor_reason: None,
            cursor_position: 0,
            cursor_target_position: 0,
            cursor_stale_after_seconds: 12,
            cursor_last_advanced_seconds_ago: 0,
            recovery_phase: SyncRecoveryPhase::Idle,
            last_applied_event_seq: 0,
            duplicate_drop_count: 0,
            replay_count: 0,
            replay_progress_percent: None,
            replay_lag_seq: None,
            next_retry_ms: None,
            token_refresh_after_in_seconds: None,
            disconnect_reason: None,
            cursor_last_advanced_at: Some(Instant::now()),
        }
    }
}

impl SyncHealthState {
    pub fn cursor_is_stale(&self) -> bool {
        self.cursor_last_advanced_seconds_ago > self.cursor_stale_after_seconds
    }

    pub fn rebootstrap(&mut self) {
        self.replay_count = self.replay_count.saturating_add(1);
        self.recovery_phase = SyncRecoveryPhase::Replaying;
        self.cursor_position = self.last_applied_event_seq;
        self.cursor_target_position = self.cursor_position;
        self.cursor_last_advanced_seconds_ago = 0;
        self.cursor_last_advanced_at = Some(Instant::now());
        self.stale_cursor_reason = None;
        self.disconnect_reason = None;
        self.pane_set_ready(format!(
            "Rebootstrapped sync stream (attempt #{})",
            self.replay_count
        ));
    }

    pub fn refresh_from_lifecycle(
        &mut self,
        now: Instant,
        lifecycle: Option<&RuntimeSyncHealthSnapshot>,
    ) {
        self.source_tag = "spacetime.sync.lifecycle".to_string();
        self.cursor_position = self.cursor_position.max(self.last_applied_event_seq);
        self.cursor_target_position = self.cursor_target_position.max(self.cursor_position);

        let Some(snapshot) = lifecycle else {
            self.spacetime_connection = "unknown".to_string();
            self.subscription_state = "unsubscribed".to_string();
            self.reconnect_posture = "none".to_string();
            self.next_retry_ms = None;
            self.replay_progress_percent = None;
            self.replay_lag_seq = None;
            self.token_refresh_after_in_seconds = None;
            self.disconnect_reason = None;
            self.stale_cursor_reason = None;
            self.cursor_last_advanced_seconds_ago = 0;
            self.cursor_last_advanced_at = Some(now);
            self.recovery_phase = SyncRecoveryPhase::Idle;
            *self.pane_load_state_mut() = PaneLoadState::Loading;
            self.pane_clear_error();
            return;
        };

        self.spacetime_connection = snapshot.state.as_str().to_string();
        self.next_retry_ms = snapshot.next_retry_ms;
        self.token_refresh_after_in_seconds = snapshot.token_refresh_after_in_seconds;
        self.disconnect_reason = snapshot
            .last_disconnect_reason
            .map(|reason| reason.as_str().to_string());
        self.last_error = snapshot.last_error.clone();

        self.subscription_state = match snapshot.state {
            RuntimeSyncConnectionState::Live => {
                if snapshot.replay_lag_seq.unwrap_or(0) > 0 {
                    "replaying".to_string()
                } else {
                    "subscribed".to_string()
                }
            }
            RuntimeSyncConnectionState::Connecting => "subscribing".to_string(),
            RuntimeSyncConnectionState::Backoff => "resubscribing".to_string(),
            RuntimeSyncConnectionState::Idle => "idle".to_string(),
        };
        self.reconnect_posture = if snapshot.state == RuntimeSyncConnectionState::Backoff {
            snapshot.next_retry_ms.map_or_else(
                || "backoff".to_string(),
                |delay| format!("backoff ({delay}ms)"),
            )
        } else if snapshot.state == RuntimeSyncConnectionState::Connecting {
            "connecting".to_string()
        } else {
            "none".to_string()
        };

        let previous_cursor = self.cursor_position;
        let next_cursor = snapshot
            .replay_cursor_seq
            .unwrap_or(self.last_applied_event_seq)
            .max(self.last_applied_event_seq);
        self.cursor_position = next_cursor;
        self.cursor_target_position = snapshot
            .replay_target_seq
            .unwrap_or(self.cursor_target_position)
            .max(self.cursor_position);
        self.replay_lag_seq = snapshot.replay_lag_seq;
        self.replay_progress_percent = snapshot.replay_progress_pct;

        if self.cursor_position > previous_cursor {
            self.cursor_last_advanced_at = Some(now);
        }

        let monitor_staleness = matches!(snapshot.state, RuntimeSyncConnectionState::Live)
            && self.subscription_state != "replaying";
        if monitor_staleness {
            let last_advanced_at = self.cursor_last_advanced_at.get_or_insert(now);
            self.cursor_last_advanced_seconds_ago = now
                .checked_duration_since(*last_advanced_at)
                .map_or(0, |duration| duration.as_secs());
        } else {
            self.cursor_last_advanced_seconds_ago = 0;
            self.cursor_last_advanced_at = Some(now);
        }

        if monitor_staleness && self.cursor_is_stale() {
            self.recovery_phase = SyncRecoveryPhase::Reconnecting;
            self.stale_cursor_reason = Some(format!(
                "cursor stalled for {}s (> {}s threshold)",
                self.cursor_last_advanced_seconds_ago, self.cursor_stale_after_seconds
            ));
            let _ = self.pane_set_error("Cursor stalled beyond stale threshold");
            return;
        }
        self.stale_cursor_reason = None;

        self.recovery_phase = match snapshot.state {
            RuntimeSyncConnectionState::Backoff | RuntimeSyncConnectionState::Connecting => {
                SyncRecoveryPhase::Reconnecting
            }
            RuntimeSyncConnectionState::Live => {
                if self.subscription_state == "replaying" {
                    SyncRecoveryPhase::Replaying
                } else {
                    SyncRecoveryPhase::Ready
                }
            }
            RuntimeSyncConnectionState::Idle => SyncRecoveryPhase::Idle,
        };

        if self.last_error.is_some() {
            *self.pane_load_state_mut() = PaneLoadState::Error;
        } else {
            *self.pane_load_state_mut() = PaneLoadState::Ready;
            self.pane_clear_error();
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NetworkRequestStatus {
    Submitted,
    Streaming,
    Processing,
    PaymentRequired,
    ResultReceived,
    Paid,
    Completed,
    Failed,
}

impl NetworkRequestStatus {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Submitted => "submitted",
            Self::Streaming => "streaming",
            Self::Processing => "processing",
            Self::PaymentRequired => "payment-required",
            Self::ResultReceived => "result-received",
            Self::Paid => "paid",
            Self::Completed => "completed",
            Self::Failed => "failed",
        }
    }

    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Paid | Self::Completed | Self::Failed)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BuyerResolutionMode {
    Race,
    Windowed,
}

impl BuyerResolutionMode {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Race => "race",
            Self::Windowed => "windowed",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BuyerResolutionReason {
    FirstValidResult,
    LostRace,
    LateResultUnpaid,
}

impl BuyerResolutionReason {
    pub const fn code(self) -> &'static str {
        match self {
            Self::FirstValidResult => "first-valid-result",
            Self::LostRace => "lost-race",
            Self::LateResultUnpaid => "late-result-unpaid",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NetworkRequestDuplicateKind {
    Feedback,
    Result,
}

impl NetworkRequestDuplicateKind {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Feedback => "feedback",
            Self::Result => "result",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NetworkRequestDuplicateOutcome {
    pub provider_pubkey: String,
    pub event_id: String,
    pub kind: NetworkRequestDuplicateKind,
    pub status: Option<String>,
    pub status_extra: Option<String>,
    pub reason_code: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NetworkRequestResolutionFeedback {
    pub provider_pubkey: String,
    pub feedback_event_id: String,
    pub reason_code: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BuyerResolutionAction {
    pub request_id: String,
    pub provider_pubkey: String,
    pub reason: BuyerResolutionReason,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NetworkRequestProviderObservation {
    pub provider_pubkey: String,
    pub last_feedback_event_id: Option<String>,
    pub last_feedback_status: Option<String>,
    pub last_feedback_status_extra: Option<String>,
    pub last_feedback_amount_msats: Option<u64>,
    pub last_feedback_bolt11: Option<String>,
    pub last_result_event_id: Option<String>,
    pub last_result_status: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AutoPaymentBudgetRefusal {
    pub provider_pubkey: String,
    pub invoice_amount_sats: u64,
    pub approved_budget_sats: u64,
    pub amount_mismatch: bool,
}

impl AutoPaymentBudgetRefusal {
    pub fn notice_message(&self) -> String {
        let mismatch_suffix = if self.amount_mismatch {
            " (provider metadata mismatched the BOLT11 amount)"
        } else {
            ""
        };
        format!(
            "provider {} requested {} sats above approved budget {}; refusing auto-payment{}",
            self.provider_pubkey,
            self.invoice_amount_sats,
            self.approved_budget_sats,
            mismatch_suffix,
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SubmittedNetworkRequest {
    pub request_id: String,
    pub published_request_event_id: Option<String>,
    pub request_type: String,
    pub payload: String,
    pub resolution_mode: BuyerResolutionMode,
    pub target_provider_pubkeys: Vec<String>,
    pub last_provider_pubkey: Option<String>,
    pub result_provider_pubkey: Option<String>,
    pub invoice_provider_pubkey: Option<String>,
    pub last_feedback_status: Option<String>,
    pub last_feedback_event_id: Option<String>,
    pub last_result_event_id: Option<String>,
    pub last_payment_pointer: Option<String>,
    pub payment_required_at_epoch_seconds: Option<u64>,
    pub payment_sent_at_epoch_seconds: Option<u64>,
    pub payment_failed_at_epoch_seconds: Option<u64>,
    pub payment_error: Option<String>,
    pub payment_notice: Option<String>,
    pub pending_bolt11: Option<String>,
    pub skill_scope_id: Option<String>,
    pub credit_envelope_ref: Option<String>,
    pub budget_sats: u64,
    pub timeout_seconds: u64,
    pub response_stream_id: String,
    pub status: NetworkRequestStatus,
    pub authority_command_seq: u64,
    pub authority_status: Option<String>,
    pub authority_event_id: Option<String>,
    pub authority_error_class: Option<String>,
    pub winning_provider_pubkey: Option<String>,
    pub winning_result_event_id: Option<String>,
    pub resolution_reason_code: Option<String>,
    pub duplicate_outcomes: Vec<NetworkRequestDuplicateOutcome>,
    pub resolution_feedbacks: Vec<NetworkRequestResolutionFeedback>,
    pub observed_buyer_event_ids: Vec<String>,
    pub provider_observations: Vec<NetworkRequestProviderObservation>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NetworkRequestSubmission {
    pub request_id: Option<String>,
    pub request_type: String,
    pub payload: String,
    pub resolution_mode: BuyerResolutionMode,
    pub target_provider_pubkeys: Vec<String>,
    pub skill_scope_id: Option<String>,
    pub credit_envelope_ref: Option<String>,
    pub budget_sats: u64,
    pub timeout_seconds: u64,
    pub authority_command_seq: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Default)]
pub struct SpotComputeCapabilityConstraints {
    pub accelerator_vendor: Option<String>,
    pub accelerator_family: Option<String>,
    pub min_memory_gb: Option<u32>,
    pub max_latency_ms: Option<u32>,
    pub min_throughput_per_minute: Option<u32>,
    pub model_policy: Option<String>,
    pub model_family: Option<String>,
}

impl SpotComputeCapabilityConstraints {
    pub fn summary(&self) -> String {
        let mut parts = Vec::new();
        if let Some(vendor) = self.accelerator_vendor.as_deref() {
            parts.push(format!("accelerator_vendor={vendor}"));
        }
        if let Some(family) = self.accelerator_family.as_deref() {
            parts.push(format!("accelerator_family={family}"));
        }
        if let Some(memory_gb) = self.min_memory_gb {
            parts.push(format!("min_memory_gb={memory_gb}"));
        }
        if let Some(latency_ms) = self.max_latency_ms {
            parts.push(format!("max_latency_ms={latency_ms}"));
        }
        if let Some(throughput) = self.min_throughput_per_minute {
            parts.push(format!("min_throughput_per_minute={throughput}"));
        }
        if let Some(model_policy) = self.model_policy.as_deref() {
            parts.push(format!("model_policy={model_policy}"));
        }
        if let Some(model_family) = self.model_family.as_deref() {
            parts.push(format!("model_family={model_family}"));
        }
        if parts.is_empty() {
            "none".to_string()
        } else {
            parts.join(", ")
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ComputeQuoteMode {
    Spot,
    ForwardPhysical,
}

impl ComputeQuoteMode {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Spot => "spot",
            Self::ForwardPhysical => "forward_physical",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SpotComputeRfqDraft {
    pub rfq_id: String,
    pub compute_family: ComputeFamily,
    pub preferred_backend: Option<ComputeBackendFamily>,
    pub quantity: u64,
    pub window_minutes: u64,
    pub max_price_sats: u64,
    pub capability_constraints: SpotComputeCapabilityConstraints,
}

impl SpotComputeRfqDraft {
    pub const fn compute_family_label(&self) -> &'static str {
        match self.compute_family {
            ComputeFamily::Inference => "inference",
            ComputeFamily::Embeddings => "embeddings",
        }
    }

    pub const fn preferred_backend_label(&self) -> &'static str {
        match self.preferred_backend {
            Some(ComputeBackendFamily::Ollama) => "ollama",
            Some(ComputeBackendFamily::AppleFoundationModels) => "apple_foundation_models",
            None => "any",
        }
    }

    pub fn summary(&self) -> String {
        format!(
            "rfq={} family={} backend={} qty={} window={}m max_price={} constraints={}",
            self.rfq_id,
            self.compute_family_label(),
            self.preferred_backend_label(),
            self.quantity,
            self.window_minutes,
            format_sats_amount(self.max_price_sats),
            self.capability_constraints.summary()
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ForwardComputeRfqDraft {
    pub rfq_id: String,
    pub compute_family: ComputeFamily,
    pub preferred_backend: Option<ComputeBackendFamily>,
    pub quantity: u64,
    pub delivery_start_minutes: u64,
    pub window_minutes: u64,
    pub max_price_sats: u64,
    pub capability_constraints: SpotComputeCapabilityConstraints,
}

impl ForwardComputeRfqDraft {
    pub const fn compute_family_label(&self) -> &'static str {
        match self.compute_family {
            ComputeFamily::Inference => "inference",
            ComputeFamily::Embeddings => "embeddings",
        }
    }

    pub const fn preferred_backend_label(&self) -> &'static str {
        match self.preferred_backend {
            Some(ComputeBackendFamily::Ollama) => "ollama",
            Some(ComputeBackendFamily::AppleFoundationModels) => "apple_foundation_models",
            None => "any",
        }
    }

    pub fn summary(&self) -> String {
        format!(
            "rfq={} family={} backend={} qty={} start_in={}m window={}m max_price={} constraints={}",
            self.rfq_id,
            self.compute_family_label(),
            self.preferred_backend_label(),
            self.quantity,
            self.delivery_start_minutes,
            self.window_minutes,
            format_sats_amount(self.max_price_sats),
            self.capability_constraints.summary()
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SpotComputeQuoteCandidate {
    pub quote_id: String,
    pub rfq_id: String,
    pub product_id: String,
    pub capacity_lot_id: String,
    pub provider_id: String,
    pub backend_family: ComputeBackendFamily,
    pub compute_family: ComputeFamily,
    pub available_quantity: u64,
    pub requested_quantity: u64,
    pub price_sats: u64,
    pub delivery_window_label: String,
    pub capability_summary: String,
    pub source_badge: String,
    pub terms_label: String,
}

impl SpotComputeQuoteCandidate {
    pub const fn backend_label(&self) -> &'static str {
        match self.backend_family {
            ComputeBackendFamily::Ollama => "ollama",
            ComputeBackendFamily::AppleFoundationModels => "apple_foundation_models",
        }
    }

    pub const fn compute_family_label(&self) -> &'static str {
        match self.compute_family {
            ComputeFamily::Inference => "inference",
            ComputeFamily::Embeddings => "embeddings",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ForwardComputeQuoteCandidate {
    pub quote_id: String,
    pub rfq_id: String,
    pub product_id: String,
    pub capacity_lot_id: String,
    pub provider_id: String,
    pub backend_family: ComputeBackendFamily,
    pub compute_family: ComputeFamily,
    pub available_quantity: u64,
    pub requested_quantity: u64,
    pub price_sats: u64,
    pub delivery_start_ms: i64,
    pub delivery_end_ms: i64,
    pub delivery_window_label: String,
    pub capability_summary: String,
    pub source_badge: String,
    pub terms_label: String,
    pub collateral_summary: String,
    pub remedy_summary: String,
}

impl ForwardComputeQuoteCandidate {
    pub const fn backend_label(&self) -> &'static str {
        match self.backend_family {
            ComputeBackendFamily::Ollama => "ollama",
            ComputeBackendFamily::AppleFoundationModels => "apple_foundation_models",
        }
    }

    pub const fn compute_family_label(&self) -> &'static str {
        match self.compute_family {
            ComputeFamily::Inference => "inference",
            ComputeFamily::Embeddings => "embeddings",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcceptedSpotComputeOrder {
    pub order_id: String,
    pub rfq_id: String,
    pub quote_id: String,
    pub instrument_id: String,
    pub product_id: String,
    pub capacity_lot_id: String,
    pub provider_id: String,
    pub backend_family: ComputeBackendFamily,
    pub compute_family: ComputeFamily,
    pub quantity: u64,
    pub price_sats: u64,
    pub delivery_window_label: String,
    pub authority_status: String,
    pub accepted_at_epoch_seconds: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcceptedForwardComputeOrder {
    pub order_id: String,
    pub rfq_id: String,
    pub quote_id: String,
    pub instrument_id: String,
    pub product_id: String,
    pub capacity_lot_id: String,
    pub provider_id: String,
    pub backend_family: ComputeBackendFamily,
    pub compute_family: ComputeFamily,
    pub quantity: u64,
    pub price_sats: u64,
    pub delivery_start_ms: i64,
    pub delivery_end_ms: i64,
    pub delivery_window_label: String,
    pub collateral_summary: String,
    pub remedy_summary: String,
    pub authority_status: String,
    pub accepted_at_epoch_seconds: u64,
}

pub struct NetworkRequestsState {
    pub load_state: PaneLoadState,
    pub last_error: Option<String>,
    pub last_action: Option<String>,
    pub quote_mode: ComputeQuoteMode,
    pub last_spot_rfq: Option<SpotComputeRfqDraft>,
    pub spot_quote_candidates: Vec<SpotComputeQuoteCandidate>,
    pub selected_spot_quote_id: Option<String>,
    pub accepted_spot_orders: Vec<AcceptedSpotComputeOrder>,
    pub last_forward_rfq: Option<ForwardComputeRfqDraft>,
    pub forward_quote_candidates: Vec<ForwardComputeQuoteCandidate>,
    pub selected_forward_quote_id: Option<String>,
    pub accepted_forward_orders: Vec<AcceptedForwardComputeOrder>,
    pub submitted: Vec<SubmittedNetworkRequest>,
    pub pending_auto_payment_request_id: Option<String>,
    next_request_seq: u64,
}

impl Default for NetworkRequestsState {
    fn default() -> Self {
        Self {
            load_state: PaneLoadState::Loading,
            last_error: None,
            last_action: Some("Waiting for request lane snapshot".to_string()),
            quote_mode: ComputeQuoteMode::Spot,
            last_spot_rfq: None,
            spot_quote_candidates: Vec::new(),
            selected_spot_quote_id: None,
            accepted_spot_orders: Vec::new(),
            last_forward_rfq: None,
            forward_quote_candidates: Vec::new(),
            selected_forward_quote_id: None,
            accepted_forward_orders: Vec::new(),
            submitted: Vec::new(),
            pending_auto_payment_request_id: None,
            next_request_seq: 0,
        }
    }
}

impl NetworkRequestsState {
    pub fn latest_request_by_type(&self, request_type: &str) -> Option<&SubmittedNetworkRequest> {
        self.submitted
            .iter()
            .find(|request| request.request_type == request_type)
    }

    pub fn has_in_flight_request_by_type(&self, request_type: &str) -> bool {
        self.submitted
            .iter()
            .any(|request| request.request_type == request_type && !request.status.is_terminal())
    }

    pub fn queue_request_submission(
        &mut self,
        submission: NetworkRequestSubmission,
    ) -> Result<String, String> {
        let NetworkRequestSubmission {
            request_id,
            request_type,
            payload,
            resolution_mode,
            target_provider_pubkeys,
            skill_scope_id,
            credit_envelope_ref,
            budget_sats,
            timeout_seconds,
            authority_command_seq,
        } = submission;

        let request_type = request_type.trim();
        if request_type.is_empty() {
            return Err(self.pane_set_error("Request type is required"));
        }

        let payload = payload.trim();
        if payload.is_empty() {
            return Err(self.pane_set_error("Request payload is required"));
        }

        if budget_sats == 0 {
            return Err(self.pane_set_error("Budget sats must be greater than 0"));
        }

        if timeout_seconds == 0 {
            return Err(self.pane_set_error("Timeout seconds must be greater than 0"));
        }

        let request_id = request_id
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| {
                let generated = format!("req-buy-{:04}", self.next_request_seq);
                self.next_request_seq = self.next_request_seq.saturating_add(1);
                generated
            });
        let stream_id = format!("stream:{request_id}");
        let mut target_provider_pubkeys = target_provider_pubkeys
            .into_iter()
            .map(|pubkey| pubkey.trim().to_string())
            .filter(|pubkey| !pubkey.is_empty())
            .collect::<Vec<_>>();
        target_provider_pubkeys.sort();
        target_provider_pubkeys.dedup();
        self.submitted.insert(
            0,
            SubmittedNetworkRequest {
                request_id: request_id.clone(),
                published_request_event_id: None,
                request_type: request_type.to_string(),
                payload: payload.to_string(),
                resolution_mode,
                target_provider_pubkeys,
                last_provider_pubkey: None,
                result_provider_pubkey: None,
                invoice_provider_pubkey: None,
                last_feedback_status: None,
                last_feedback_event_id: None,
                last_result_event_id: None,
                last_payment_pointer: None,
                payment_required_at_epoch_seconds: None,
                payment_sent_at_epoch_seconds: None,
                payment_failed_at_epoch_seconds: None,
                payment_error: None,
                payment_notice: None,
                pending_bolt11: None,
                skill_scope_id,
                credit_envelope_ref,
                budget_sats,
                timeout_seconds,
                response_stream_id: stream_id,
                status: NetworkRequestStatus::Submitted,
                authority_command_seq,
                authority_status: None,
                authority_event_id: None,
                authority_error_class: None,
                winning_provider_pubkey: None,
                winning_result_event_id: None,
                resolution_reason_code: None,
                duplicate_outcomes: Vec::new(),
                resolution_feedbacks: Vec::new(),
                observed_buyer_event_ids: Vec::new(),
                provider_observations: Vec::new(),
            },
        );
        self.pane_set_ready(format!(
            "Queued buyer request {request_id} -> cmd#{authority_command_seq}"
        ));
        Ok(request_id)
    }

    pub fn replace_spot_quotes(
        &mut self,
        rfq: SpotComputeRfqDraft,
        mut quotes: Vec<SpotComputeQuoteCandidate>,
    ) {
        quotes.sort_by(|left, right| {
            left.price_sats
                .cmp(&right.price_sats)
                .then_with(|| left.product_id.cmp(&right.product_id))
                .then_with(|| left.capacity_lot_id.cmp(&right.capacity_lot_id))
        });
        self.last_spot_rfq = Some(rfq.clone());
        self.selected_spot_quote_id = quotes.first().map(|quote| quote.quote_id.clone());
        self.spot_quote_candidates = quotes;
        self.load_state = PaneLoadState::Ready;
        self.last_error = None;
        self.last_action = Some(format!(
            "Loaded {} compute quote{} for {}",
            self.spot_quote_candidates.len(),
            if self.spot_quote_candidates.len() == 1 {
                ""
            } else {
                "s"
            },
            rfq.summary()
        ));
        self.quote_mode = ComputeQuoteMode::Spot;
    }

    pub fn clear_spot_quotes_with_error(&mut self, error: impl Into<String>) -> String {
        self.spot_quote_candidates.clear();
        self.selected_spot_quote_id = None;
        self.last_spot_rfq = None;
        self.quote_mode = ComputeQuoteMode::Spot;
        self.pane_set_error(error)
    }

    pub fn select_spot_quote_by_index(&mut self, index: usize) -> bool {
        let Some(quote_id) = self
            .spot_quote_candidates
            .get(index)
            .map(|quote| quote.quote_id.clone())
        else {
            return false;
        };
        self.selected_spot_quote_id = Some(quote_id.clone());
        self.load_state = PaneLoadState::Ready;
        self.last_error = None;
        self.last_action = Some(format!("Selected compute quote {quote_id}"));
        true
    }

    pub fn selected_spot_quote(&self) -> Option<&SpotComputeQuoteCandidate> {
        let selected = self.selected_spot_quote_id.as_deref()?;
        self.spot_quote_candidates
            .iter()
            .find(|quote| quote.quote_id == selected)
    }

    pub fn record_spot_order_acceptance(&mut self, order: AcceptedSpotComputeOrder) {
        let selected_quote_id = order.quote_id.clone();
        let accepted_quantity = order.quantity;
        self.accepted_spot_orders.insert(0, order.clone());
        self.accepted_spot_orders.truncate(32);

        for quote in &mut self.spot_quote_candidates {
            if quote.quote_id != selected_quote_id {
                continue;
            }
            quote.available_quantity = quote.available_quantity.saturating_sub(accepted_quantity);
        }
        self.spot_quote_candidates
            .retain(|quote| quote.available_quantity >= quote.requested_quantity);
        self.selected_spot_quote_id = self
            .spot_quote_candidates
            .first()
            .map(|quote| quote.quote_id.clone());
        self.load_state = PaneLoadState::Ready;
        self.last_error = None;
        self.last_action = Some(format!(
            "Accepted compute quote {} -> instrument {}",
            order.quote_id, order.instrument_id
        ));
        self.quote_mode = ComputeQuoteMode::Spot;
    }

    pub fn replace_forward_quotes(
        &mut self,
        rfq: ForwardComputeRfqDraft,
        mut quotes: Vec<ForwardComputeQuoteCandidate>,
    ) {
        quotes.sort_by(|left, right| {
            left.price_sats
                .cmp(&right.price_sats)
                .then_with(|| left.delivery_start_ms.cmp(&right.delivery_start_ms))
                .then_with(|| left.product_id.cmp(&right.product_id))
                .then_with(|| left.capacity_lot_id.cmp(&right.capacity_lot_id))
        });
        self.last_forward_rfq = Some(rfq.clone());
        self.selected_forward_quote_id = quotes.first().map(|quote| quote.quote_id.clone());
        self.forward_quote_candidates = quotes;
        self.load_state = PaneLoadState::Ready;
        self.last_error = None;
        self.last_action = Some(format!(
            "Loaded {} forward compute quote{} for {}",
            self.forward_quote_candidates.len(),
            if self.forward_quote_candidates.len() == 1 {
                ""
            } else {
                "s"
            },
            rfq.summary()
        ));
        self.quote_mode = ComputeQuoteMode::ForwardPhysical;
    }

    pub fn clear_forward_quotes_with_error(&mut self, error: impl Into<String>) -> String {
        self.forward_quote_candidates.clear();
        self.selected_forward_quote_id = None;
        self.last_forward_rfq = None;
        self.quote_mode = ComputeQuoteMode::ForwardPhysical;
        self.pane_set_error(error)
    }

    pub fn select_forward_quote_by_index(&mut self, index: usize) -> bool {
        let Some(quote_id) = self
            .forward_quote_candidates
            .get(index)
            .map(|quote| quote.quote_id.clone())
        else {
            return false;
        };
        self.selected_forward_quote_id = Some(quote_id.clone());
        self.load_state = PaneLoadState::Ready;
        self.last_error = None;
        self.last_action = Some(format!("Selected compute quote {quote_id}"));
        true
    }

    pub fn selected_forward_quote(&self) -> Option<&ForwardComputeQuoteCandidate> {
        let selected = self.selected_forward_quote_id.as_deref()?;
        self.forward_quote_candidates
            .iter()
            .find(|quote| quote.quote_id == selected)
    }

    pub fn record_forward_order_acceptance(&mut self, order: AcceptedForwardComputeOrder) {
        let selected_quote_id = order.quote_id.clone();
        let accepted_quantity = order.quantity;
        self.accepted_forward_orders.insert(0, order.clone());
        self.accepted_forward_orders.truncate(32);

        for quote in &mut self.forward_quote_candidates {
            if quote.quote_id != selected_quote_id {
                continue;
            }
            quote.available_quantity = quote.available_quantity.saturating_sub(accepted_quantity);
        }
        self.forward_quote_candidates
            .retain(|quote| quote.available_quantity >= quote.requested_quantity);
        self.selected_forward_quote_id = self
            .forward_quote_candidates
            .first()
            .map(|quote| quote.quote_id.clone());
        self.load_state = PaneLoadState::Ready;
        self.last_error = None;
        self.last_action = Some(format!(
            "Accepted compute quote {} -> instrument {}",
            order.quote_id, order.instrument_id
        ));
        self.quote_mode = ComputeQuoteMode::ForwardPhysical;
    }

    pub fn active_quote_count(&self) -> usize {
        match self.quote_mode {
            ComputeQuoteMode::Spot => self.spot_quote_candidates.len(),
            ComputeQuoteMode::ForwardPhysical => self.forward_quote_candidates.len(),
        }
    }

    pub fn select_active_quote_by_index(&mut self, index: usize) -> bool {
        match self.quote_mode {
            ComputeQuoteMode::Spot => self.select_spot_quote_by_index(index),
            ComputeQuoteMode::ForwardPhysical => self.select_forward_quote_by_index(index),
        }
    }

    pub fn apply_nip90_request_publish_outcome(
        &mut self,
        request_id: &str,
        event_id: &str,
        accepted_relays: usize,
        rejected_relays: usize,
        first_error: Option<&str>,
    ) {
        let request_id = {
            let Some(request) = self
                .submitted
                .iter_mut()
                .find(|request| request.request_id == request_id)
            else {
                return;
            };
            request.published_request_event_id = Some(event_id.to_string());
            if accepted_relays > 0 {
                request.status = NetworkRequestStatus::Streaming;
            } else {
                request.status = NetworkRequestStatus::Failed;
            }
            request.request_id.clone()
        };

        if accepted_relays > 0 {
            self.pane_set_ready(format!(
                "Published request {} (accepted={}, rejected={})",
                request_id, accepted_relays, rejected_relays
            ));
        } else {
            let error = first_error.unwrap_or("All relays rejected request publish");
            let _ = self.pane_set_error(format!(
                "Failed publishing request {}: {}",
                request_id, error
            ));
        }
    }

    pub fn apply_nip90_buyer_feedback_event(
        &mut self,
        request_id: &str,
        provider_pubkey: &str,
        event_id: &str,
        status: Option<&str>,
        status_extra: Option<&str>,
        amount_msats: Option<u64>,
        bolt11: Option<&str>,
    ) -> Option<BuyerResolutionAction> {
        let Some(request) = self
            .submitted
            .iter_mut()
            .find(|request| request.request_id == request_id)
        else {
            return None;
        };
        if request.status.is_terminal() {
            return None;
        }
        if request
            .observed_buyer_event_ids
            .iter()
            .any(|observed| observed == event_id)
        {
            return None;
        };
        request.observed_buyer_event_ids.push(event_id.to_string());

        let provider = observed_provider_mut(request, provider_pubkey);
        provider.last_feedback_event_id = Some(event_id.to_string());
        provider.last_feedback_status = status.map(ToString::to_string);
        provider.last_feedback_status_extra = status_extra.map(ToString::to_string);
        if let Some(amount_msats) = amount_msats {
            provider.last_feedback_amount_msats = Some(amount_msats);
        }
        if let Some(bolt11) = bolt11
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string)
        {
            provider.last_feedback_bolt11 = Some(bolt11);
            request.invoice_provider_pubkey = Some(provider_pubkey.to_string());
        }

        request.last_provider_pubkey = Some(provider_pubkey.to_string());
        request.last_feedback_event_id = Some(event_id.to_string());
        request.last_feedback_status = status.map(ToString::to_string);
        select_payable_winner(request, Some(provider_pubkey));
        request.status = compute_request_status(request);
        let resolution_action = maybe_race_resolution_action_for_feedback(
            request,
            provider_pubkey,
            event_id,
            status,
            status_extra,
        );

        let status_label = status.unwrap_or("unknown");
        let status_extra = status_extra.unwrap_or("none");
        self.pane_set_ready(format!(
            "Request {} feedback={} provider={} detail={}",
            request_id, status_label, provider_pubkey, status_extra
        ));
        resolution_action
    }

    pub fn apply_nip90_buyer_result_event(
        &mut self,
        request_id: &str,
        provider_pubkey: &str,
        event_id: &str,
        status: Option<&str>,
    ) -> Option<BuyerResolutionAction> {
        let Some(request) = self
            .submitted
            .iter_mut()
            .find(|request| request.request_id == request_id)
        else {
            return None;
        };
        if request.status.is_terminal() {
            return None;
        }
        if request
            .observed_buyer_event_ids
            .iter()
            .any(|observed| observed == event_id)
        {
            return None;
        };
        request.observed_buyer_event_ids.push(event_id.to_string());

        let provider = observed_provider_mut(request, provider_pubkey);
        provider.last_result_event_id = Some(event_id.to_string());
        provider.last_result_status = status.map(ToString::to_string);

        request.last_provider_pubkey = Some(provider_pubkey.to_string());
        if !matches!(
            status
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_ascii_lowercase)
                .as_deref(),
            Some("error")
        ) {
            request.result_provider_pubkey = Some(provider_pubkey.to_string());
        }
        request.last_result_event_id = Some(event_id.to_string());
        select_payable_winner(request, Some(provider_pubkey));
        request.status = compute_request_status(request);
        let resolution_action =
            maybe_race_resolution_action_for_result(request, provider_pubkey, event_id, status);
        self.pane_set_ready(format!(
            "Request {} result event {} from provider {}",
            request_id, event_id, provider_pubkey
        ));
        resolution_action
    }

    pub fn record_resolution_feedback(
        &mut self,
        request_id: &str,
        provider_pubkey: &str,
        feedback_event_id: &str,
        reason: BuyerResolutionReason,
    ) {
        let Some(request) = self
            .submitted
            .iter_mut()
            .find(|request| request.request_id == request_id)
        else {
            return;
        };
        if request.resolution_feedbacks.iter().any(|feedback| {
            feedback.provider_pubkey == provider_pubkey && feedback.reason_code == reason.code()
        }) {
            return;
        }
        request
            .resolution_feedbacks
            .push(NetworkRequestResolutionFeedback {
                provider_pubkey: provider_pubkey.to_string(),
                feedback_event_id: feedback_event_id.to_string(),
                reason_code: reason.code().to_string(),
            });
    }

    pub fn mark_direct_authority_ready(
        &mut self,
        request_id: &str,
        authority_status: &str,
        authority_event_id: Option<&str>,
    ) {
        let Some(request) = self
            .submitted
            .iter_mut()
            .find(|request| request.request_id == request_id)
        else {
            return;
        };
        request.authority_status = Some(authority_status.to_string());
        request.authority_event_id = authority_event_id.map(ToString::to_string);
        request.authority_error_class = None;
        self.pane_set_ready(format!(
            "Request {} attached to direct authority path {}",
            request_id, authority_status
        ));
    }

    pub fn prepare_auto_payment_attempt(
        &mut self,
        request_id: &str,
        bolt11: &str,
        amount_msats: Option<u64>,
        now_epoch_seconds: u64,
    ) -> Option<(String, Option<u64>)> {
        self.prepare_auto_payment_attempt_internal(
            request_id,
            None,
            bolt11,
            amount_msats,
            now_epoch_seconds,
        )
    }

    pub fn prepare_auto_payment_attempt_for_provider(
        &mut self,
        request_id: &str,
        provider_pubkey: &str,
        now_epoch_seconds: u64,
    ) -> Option<(String, Option<u64>)> {
        let (budget_refusal_notice, bolt11, amount_msats) = {
            let request = self
                .submitted
                .iter_mut()
                .find(|request| request.request_id == request_id)?;
            if request.status.is_terminal() {
                return None;
            }
            select_payable_winner(request, Some(provider_pubkey));

            let observation = request.provider_observations.iter().find(|observation| {
                normalize_pubkey(observation.provider_pubkey.as_str())
                    == normalize_pubkey(provider_pubkey)
            })?;
            let budget_refusal_notice = budget_refusal_for_request(
                request,
                Some(provider_pubkey),
                observation.last_feedback_amount_msats,
                observation.last_feedback_bolt11.as_deref(),
            )
            .map(|refusal| refusal.notice_message());
            if let Some(notice) = budget_refusal_notice {
                (
                    Some(notice),
                    observation.last_feedback_bolt11.clone(),
                    observation.last_feedback_amount_msats,
                )
            } else {
                let winner = request.winning_provider_pubkey.as_deref()?;
                if normalize_pubkey(winner) != normalize_pubkey(provider_pubkey) {
                    return None;
                }

                if !provider_has_payable_result(observation) {
                    return None;
                }

                (
                    None,
                    observation.last_feedback_bolt11.clone(),
                    observation.last_feedback_amount_msats,
                )
            }
        };
        if let Some(notice) = budget_refusal_notice {
            self.record_auto_payment_notice(request_id, notice.as_str(), now_epoch_seconds);
            return None;
        }
        let bolt11 = bolt11?;

        self.prepare_auto_payment_attempt_internal(
            request_id,
            Some(provider_pubkey),
            bolt11.as_str(),
            amount_msats,
            now_epoch_seconds,
        )
    }

    pub fn auto_payment_budget_refusal_for_provider(
        &self,
        request_id: &str,
        provider_pubkey: &str,
    ) -> Option<AutoPaymentBudgetRefusal> {
        let request = self
            .submitted
            .iter()
            .find(|request| request.request_id == request_id)?;
        let observation = request.provider_observations.iter().find(|observation| {
            normalize_pubkey(observation.provider_pubkey.as_str())
                == normalize_pubkey(provider_pubkey)
        })?;
        budget_refusal_for_request(
            request,
            Some(provider_pubkey),
            observation.last_feedback_amount_msats,
            observation.last_feedback_bolt11.as_deref(),
        )
    }

    fn prepare_auto_payment_attempt_internal(
        &mut self,
        request_id: &str,
        provider_pubkey: Option<&str>,
        bolt11: &str,
        amount_msats: Option<u64>,
        now_epoch_seconds: u64,
    ) -> Option<(String, Option<u64>)> {
        let bolt11 = bolt11.trim();
        if bolt11.is_empty() {
            self.mark_auto_payment_failed(
                request_id,
                "provider feedback is missing bolt11 invoice",
                now_epoch_seconds,
            );
            return None;
        }

        if let Some(active_request_id) = self.pending_auto_payment_request_id.as_deref() {
            if active_request_id != request_id {
                self.pane_set_ready(format!(
                    "Deferred auto-payment for {} while {} is still in-flight",
                    request_id, active_request_id
                ));
                return None;
            }
            return None;
        }

        let Some(request) = self
            .submitted
            .iter_mut()
            .find(|request| request.request_id == request_id)
        else {
            return None;
        };

        if request.last_payment_pointer.is_some() {
            return None;
        }

        if let Some(refusal) =
            budget_refusal_for_request(request, provider_pubkey, amount_msats, Some(bolt11))
        {
            let notice = refusal.notice_message();
            request.pending_bolt11 = None;
            request.payment_error = None;
            request.payment_failed_at_epoch_seconds = None;
            request
                .payment_required_at_epoch_seconds
                .get_or_insert(now_epoch_seconds);
            request.payment_notice = Some(notice.clone());
            request.status = compute_request_status(request);
            if self.pending_auto_payment_request_id.as_deref() == Some(request_id) {
                self.pending_auto_payment_request_id = None;
            }
            let _ = self.pane_set_ready(format!(
                "Request {} auto-payment blocked: {}",
                request_id, notice
            ));
            return None;
        }

        request.pending_bolt11 = Some(bolt11.to_string());
        request
            .payment_required_at_epoch_seconds
            .get_or_insert(now_epoch_seconds);
        request.payment_error = None;
        request.payment_notice = None;
        request.payment_failed_at_epoch_seconds = None;
        request.status = NetworkRequestStatus::PaymentRequired;
        self.pending_auto_payment_request_id = Some(request_id.to_string());

        let amount_sats = analyze_invoice_amount_msats(amount_msats, Some(bolt11))
            .effective_amount_msats
            .map(msats_to_sats_ceil)
            .filter(|amount| *amount > 0);
        self.pane_set_ready(format!(
            "Request {} received payment-required invoice; queueing Spark payment",
            request_id
        ));
        Some((bolt11.to_string(), amount_sats))
    }

    pub fn mark_auto_payment_sent(
        &mut self,
        request_id: &str,
        payment_pointer: &str,
        now_epoch_seconds: u64,
    ) {
        let payment_pointer = payment_pointer.trim();
        if payment_pointer.is_empty() {
            self.mark_auto_payment_failed(
                request_id,
                "Spark payment succeeded but payment pointer is empty",
                now_epoch_seconds,
            );
            return;
        }

        if let Some(request) = self
            .submitted
            .iter_mut()
            .find(|request| request.request_id == request_id)
        {
            request.last_payment_pointer = Some(payment_pointer.to_string());
            request.payment_sent_at_epoch_seconds = Some(now_epoch_seconds);
            request.payment_error = None;
            request.payment_notice = None;
            request.pending_bolt11 = None;
            request.status = NetworkRequestStatus::Paid;
        }
        self.pending_auto_payment_request_id = None;
        self.pane_set_ready(format!(
            "Request {} settled buyer payment pointer {}",
            request_id, payment_pointer
        ));
    }

    pub fn record_auto_payment_pointer(&mut self, request_id: &str, payment_pointer: &str) {
        let payment_pointer = payment_pointer.trim();
        if payment_pointer.is_empty() {
            return;
        }
        let Some(request) = self
            .submitted
            .iter_mut()
            .find(|request| request.request_id == request_id)
        else {
            return;
        };
        if request.last_payment_pointer.as_deref() == Some(payment_pointer) {
            return;
        }
        request.last_payment_pointer = Some(payment_pointer.to_string());
    }

    pub fn record_auto_payment_notice(
        &mut self,
        request_id: &str,
        notice: &str,
        now_epoch_seconds: u64,
    ) {
        let notice = notice.trim();
        if notice.is_empty() {
            return;
        }

        let Some(request) = self
            .submitted
            .iter_mut()
            .find(|request| request.request_id == request_id)
        else {
            return;
        };
        if request.status.is_terminal() {
            return;
        }

        request
            .payment_required_at_epoch_seconds
            .get_or_insert(now_epoch_seconds);
        request.payment_notice = Some(notice.to_string());
        request.status = compute_request_status(request);
        self.pane_set_ready(format!(
            "Request {} payment blocked: {}",
            request_id, notice
        ));
    }

    pub fn mark_auto_payment_failed(
        &mut self,
        request_id: &str,
        error: &str,
        now_epoch_seconds: u64,
    ) {
        let error = error.trim();
        let error = if error.is_empty() {
            "Spark payment flow failed without explicit detail"
        } else {
            error
        };

        if let Some(request) = self
            .submitted
            .iter_mut()
            .find(|request| request.request_id == request_id)
        {
            request.status = NetworkRequestStatus::Failed;
            request.payment_error = Some(error.to_string());
            request.payment_notice = None;
            request.payment_failed_at_epoch_seconds = Some(now_epoch_seconds);
            request.pending_bolt11 = None;
        }
        if self.pending_auto_payment_request_id.as_deref() == Some(request_id) {
            self.pending_auto_payment_request_id = None;
        }
        let _ = self.pane_set_error(format!("Request {} payment failed: {}", request_id, error));
    }

    pub fn apply_authority_response(&mut self, response: &RuntimeCommandResponse) {
        let Some(request) = self
            .submitted
            .iter_mut()
            .find(|request| request.authority_command_seq == response.command_seq)
        else {
            return;
        };

        request.authority_status = Some(response.status.label().to_string());
        request.authority_event_id.clone_from(&response.event_id);
        request.authority_error_class = response
            .error
            .as_ref()
            .map(|error| error.class.label().to_string());
        request.status = match response.status {
            crate::runtime_lanes::RuntimeCommandStatus::Accepted => {
                if response.event_id.is_some() {
                    NetworkRequestStatus::Completed
                } else {
                    NetworkRequestStatus::Streaming
                }
            }
            crate::runtime_lanes::RuntimeCommandStatus::Rejected => NetworkRequestStatus::Failed,
            crate::runtime_lanes::RuntimeCommandStatus::Retryable => NetworkRequestStatus::Failed,
        };

        let message = if let Some(error) = response.error.as_ref() {
            format!(
                "Request {} {} ({})",
                request.request_id,
                response.status.label(),
                error.class.label()
            )
        } else {
            format!("Request {} {}", request.request_id, response.status.label())
        };
        if response.status == crate::runtime_lanes::RuntimeCommandStatus::Accepted {
            self.pane_set_ready(message);
        } else {
            let _ = self.pane_set_error(message);
        }
    }

    pub fn mark_authority_enqueue_failure(
        &mut self,
        authority_command_seq: u64,
        error_class: &str,
        message: &str,
    ) {
        if let Some(request) = self
            .submitted
            .iter_mut()
            .find(|request| request.authority_command_seq == authority_command_seq)
        {
            request.status = NetworkRequestStatus::Failed;
            request.authority_status = Some("retryable".to_string());
            request.authority_event_id = None;
            request.authority_error_class = Some(error_class.to_string());
        }
        let _ = self.pane_set_error(message.to_string());
    }
}

fn maybe_race_resolution_action_for_feedback(
    request: &mut SubmittedNetworkRequest,
    provider_pubkey: &str,
    event_id: &str,
    status: Option<&str>,
    status_extra: Option<&str>,
) -> Option<BuyerResolutionAction> {
    if request.resolution_mode != BuyerResolutionMode::Race {
        return None;
    }
    let Some(winner) = request.winning_provider_pubkey.as_deref() else {
        return None;
    };
    if normalize_pubkey(provider_pubkey) == normalize_pubkey(winner) {
        return None;
    }
    let status = status
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string);
    let normalized_status = status.as_deref().map(str::to_ascii_lowercase);
    if matches!(normalized_status.as_deref(), Some("error")) {
        nip90_compute_domain_events::emit_provider_loser_feedback_ignored(
            request.request_id.as_str(),
            provider_pubkey,
            Some(winner),
            status.as_deref(),
            status_extra,
            "non-winning error feedback ignored",
        );
        return None;
    }
    let reason = BuyerResolutionReason::LostRace;
    if request.duplicate_outcomes.iter().any(|outcome| {
        normalize_pubkey(outcome.provider_pubkey.as_str()) == normalize_pubkey(provider_pubkey)
            && outcome.reason_code == reason.code()
    }) {
        return None;
    }
    request
        .duplicate_outcomes
        .push(NetworkRequestDuplicateOutcome {
            provider_pubkey: provider_pubkey.to_string(),
            event_id: event_id.to_string(),
            kind: NetworkRequestDuplicateKind::Feedback,
            status,
            status_extra: status_extra.map(ToString::to_string),
            reason_code: reason.code().to_string(),
        });
    Some(BuyerResolutionAction {
        request_id: request.request_id.clone(),
        provider_pubkey: provider_pubkey.to_string(),
        reason,
    })
}

fn maybe_race_resolution_action_for_result(
    request: &mut SubmittedNetworkRequest,
    provider_pubkey: &str,
    event_id: &str,
    status: Option<&str>,
) -> Option<BuyerResolutionAction> {
    if request.resolution_mode != BuyerResolutionMode::Race {
        return None;
    }
    let normalized_status = status
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_ascii_lowercase);
    let Some(winner) = request.winning_provider_pubkey.as_deref() else {
        return None;
    };
    if normalized_status.as_deref() == Some("error") {
        return None;
    }
    if normalize_pubkey(provider_pubkey) == normalize_pubkey(winner) {
        return None;
    }
    let reason = BuyerResolutionReason::LateResultUnpaid;
    if request.duplicate_outcomes.iter().any(|outcome| {
        normalize_pubkey(outcome.provider_pubkey.as_str()) == normalize_pubkey(provider_pubkey)
            && outcome.reason_code == reason.code()
    }) {
        return None;
    }
    request
        .duplicate_outcomes
        .push(NetworkRequestDuplicateOutcome {
            provider_pubkey: provider_pubkey.to_string(),
            event_id: event_id.to_string(),
            kind: NetworkRequestDuplicateKind::Result,
            status: status.map(ToString::to_string),
            status_extra: None,
            reason_code: reason.code().to_string(),
        });
    Some(BuyerResolutionAction {
        request_id: request.request_id.clone(),
        provider_pubkey: provider_pubkey.to_string(),
        reason,
    })
}

fn observed_provider_mut<'a>(
    request: &'a mut SubmittedNetworkRequest,
    provider_pubkey: &str,
) -> &'a mut NetworkRequestProviderObservation {
    let normalized_provider = normalize_pubkey(provider_pubkey);
    if let Some(index) = request
        .provider_observations
        .iter()
        .position(|observation| {
            normalize_pubkey(observation.provider_pubkey.as_str()) == normalized_provider
        })
    {
        return request
            .provider_observations
            .get_mut(index)
            .expect("observation index should remain valid");
    }

    request
        .provider_observations
        .push(NetworkRequestProviderObservation {
            provider_pubkey: provider_pubkey.to_string(),
            last_feedback_event_id: None,
            last_feedback_status: None,
            last_feedback_status_extra: None,
            last_feedback_amount_msats: None,
            last_feedback_bolt11: None,
            last_result_event_id: None,
            last_result_status: None,
        });
    request
        .provider_observations
        .last_mut()
        .expect("new observation should be present")
}

fn as_shared_provider_observation(
    observation: &NetworkRequestProviderObservation,
) -> SharedBuyerProviderObservation<'_> {
    SharedBuyerProviderObservation {
        provider_pubkey: observation.provider_pubkey.as_str(),
        last_feedback_event_id: observation.last_feedback_event_id.as_deref(),
        last_feedback_status: observation.last_feedback_status.as_deref(),
        last_feedback_amount_msats: observation.last_feedback_amount_msats,
        last_feedback_bolt11: observation.last_feedback_bolt11.as_deref(),
        last_result_event_id: observation.last_result_event_id.as_deref(),
        last_result_status: observation.last_result_status.as_deref(),
    }
}

fn provider_has_non_error_result(observation: &NetworkRequestProviderObservation) -> bool {
    shared_provider_has_non_error_result(&as_shared_provider_observation(observation))
}

fn provider_has_payable_result(observation: &NetworkRequestProviderObservation) -> bool {
    shared_provider_has_payable_result(&as_shared_provider_observation(observation))
}

fn provider_has_processing_feedback(observation: &NetworkRequestProviderObservation) -> bool {
    matches!(
        observation
            .last_feedback_status
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_ascii_lowercase)
            .as_deref(),
        Some("processing")
    )
}

fn provider_has_payment_feedback(observation: &NetworkRequestProviderObservation) -> bool {
    matches!(
        observation
            .last_feedback_status
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_ascii_lowercase)
            .as_deref(),
        Some("payment-required")
    )
}

fn provider_has_error_only_signal(observation: &NetworkRequestProviderObservation) -> bool {
    let feedback_error = matches!(
        observation
            .last_feedback_status
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_ascii_lowercase)
            .as_deref(),
        Some("error")
    );
    let result_error = matches!(
        observation
            .last_result_status
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_ascii_lowercase)
            .as_deref(),
        Some("error")
    );

    (feedback_error || result_error)
        && !provider_has_non_error_result(observation)
        && !provider_has_payment_feedback(observation)
        && !provider_has_processing_feedback(observation)
}

fn select_payable_winner(
    request: &mut SubmittedNetworkRequest,
    preferred_provider_pubkey: Option<&str>,
) {
    if request.last_payment_pointer.is_some() {
        return;
    }

    let previous_winner = request.winning_provider_pubkey.clone();
    let observations: Vec<_> = request
        .provider_observations
        .iter()
        .map(as_shared_provider_observation)
        .collect();

    if let Some(selection) = shared_select_budget_approved_payable_winner(
        previous_winner.as_deref(),
        preferred_provider_pubkey,
        observations.as_slice(),
        request.budget_sats,
    ) {
        let Some(observation) = request.provider_observations.iter().find(|observation| {
            normalize_pubkey(observation.provider_pubkey.as_str())
                == normalize_pubkey(selection.provider_pubkey.as_str())
        }) else {
            return;
        };
        let selected_provider_pubkey = observation.provider_pubkey.clone();
        let selected_result_event_id = observation.last_result_event_id.clone();
        let selected_feedback_event_id = observation.last_feedback_event_id.clone();
        let selected_amount_msats =
            invoice_amount_analysis_for_observation(observation).effective_amount_msats;
        let provider_changed = previous_winner.as_deref().map(normalize_pubkey)
            != Some(normalize_pubkey(selected_provider_pubkey.as_str()));

        request.winning_provider_pubkey = Some(selected_provider_pubkey.clone());
        request.winning_result_event_id = selected_result_event_id.clone();
        request.resolution_reason_code =
            Some(BuyerResolutionReason::FirstValidResult.code().to_string());
        if provider_changed {
            nip90_compute_domain_events::emit_buyer_selected_payable_provider(
                request.request_id.as_str(),
                selected_provider_pubkey.as_str(),
                previous_winner.as_deref(),
                selected_result_event_id.as_deref(),
                selected_feedback_event_id.as_deref(),
                selected_amount_msats,
                selection.selection_source,
            );
        }
        return;
    }

    request.winning_provider_pubkey = None;
    request.winning_result_event_id = None;
    request.resolution_reason_code = None;
}

fn invoice_amount_analysis_for_observation(
    observation: &NetworkRequestProviderObservation,
) -> SharedBuyerInvoiceAmountAnalysis {
    analyze_invoice_amount_msats(
        observation.last_feedback_amount_msats,
        observation.last_feedback_bolt11.as_deref(),
    )
}

fn budget_refusal_for_request(
    request: &SubmittedNetworkRequest,
    provider_pubkey: Option<&str>,
    metadata_amount_msats: Option<u64>,
    bolt11: Option<&str>,
) -> Option<AutoPaymentBudgetRefusal> {
    let provider_pubkey = provider_pubkey
        .map(str::trim)
        .filter(|provider| !provider.is_empty())
        .or_else(|| request.winning_provider_pubkey.as_deref())
        .or_else(|| request.invoice_provider_pubkey.as_deref())?;
    let analysis = analyze_invoice_amount_msats(metadata_amount_msats, bolt11);
    let effective_amount_msats = analysis.effective_amount_msats?;
    let approved_budget_msats = request.budget_sats.saturating_mul(1_000);
    if effective_amount_msats <= approved_budget_msats {
        return None;
    }

    Some(AutoPaymentBudgetRefusal {
        provider_pubkey: provider_pubkey.to_string(),
        invoice_amount_sats: msats_to_sats_ceil(effective_amount_msats),
        approved_budget_sats: request.budget_sats,
        amount_mismatch: analysis.amount_mismatch,
    })
}

fn compute_request_status(request: &SubmittedNetworkRequest) -> NetworkRequestStatus {
    if request.payment_error.is_some() {
        return NetworkRequestStatus::Failed;
    }
    if request.last_payment_pointer.is_some() {
        return NetworkRequestStatus::Paid;
    }

    let has_non_error_result = request
        .provider_observations
        .iter()
        .any(provider_has_non_error_result);
    if has_non_error_result {
        return NetworkRequestStatus::ResultReceived;
    }

    if request.pending_bolt11.is_some()
        || request.payment_notice.is_some()
        || request
            .provider_observations
            .iter()
            .any(provider_has_payment_feedback)
    {
        return NetworkRequestStatus::PaymentRequired;
    }

    if request
        .provider_observations
        .iter()
        .any(provider_has_processing_feedback)
    {
        return NetworkRequestStatus::Processing;
    }

    if !request.provider_observations.is_empty()
        && request
            .provider_observations
            .iter()
            .all(provider_has_error_only_signal)
    {
        return NetworkRequestStatus::Failed;
    }

    if request.published_request_event_id.is_some() {
        return NetworkRequestStatus::Streaming;
    }

    NetworkRequestStatus::Submitted
}

fn msats_to_sats_ceil(msats: u64) -> u64 {
    if msats == 0 {
        return 0;
    }
    msats.saturating_add(999) / 1000
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StarterJobStatus {
    Queued,
    Running,
    Completed,
}

impl StarterJobStatus {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Running => "running",
            Self::Completed => "completed",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StarterJobRow {
    pub job_id: String,
    pub summary: String,
    pub payout_sats: u64,
    pub eligible: bool,
    pub status: StarterJobStatus,
    pub payout_pointer: Option<String>,
    pub start_confirm_by_unix_ms: Option<u64>,
    pub execution_started_at_unix_ms: Option<u64>,
    pub execution_expires_at_unix_ms: Option<u64>,
    pub last_heartbeat_at_unix_ms: Option<u64>,
    pub next_heartbeat_due_at_unix_ms: Option<u64>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct StarterDemandTemplate {
    summary: &'static str,
    payout_sats: u64,
}

const STARTER_DEMAND_TEMPLATES: [StarterDemandTemplate; 4] = [
    StarterDemandTemplate {
        summary: "Summarize a short project update into three bullets",
        payout_sats: 120,
    },
    StarterDemandTemplate {
        summary: "Extract action items from a meeting note",
        payout_sats: 150,
    },
    StarterDemandTemplate {
        summary: "Translate a paragraph to plain English",
        payout_sats: 90,
    },
    StarterDemandTemplate {
        summary: "Classify a support ticket and suggest a response",
        payout_sats: 110,
    },
];

const STARTER_DEMAND_DEFAULT_BUDGET_SATS: u64 = 5_000;
const STARTER_DEMAND_DEFAULT_DISPATCH_INTERVAL_SECONDS: u64 = 12;
const STARTER_DEMAND_DEFAULT_MAX_INFLIGHT_JOBS: usize = 1;

pub struct StarterJobsState {
    pub load_state: PaneLoadState,
    pub last_error: Option<String>,
    pub last_action: Option<String>,
    pub jobs: Vec<StarterJobRow>,
    pub selected_job_id: Option<String>,
    pub kill_switch_enabled: bool,
    pub budget_cap_sats: u64,
    pub budget_allocated_sats: u64,
    pub dispatch_interval_seconds: u64,
    pub max_inflight_jobs: usize,
    pub last_dispatched_at: Option<Instant>,
    pub next_hosted_sync_due_at: Option<Instant>,
    pub active_hosted_request_id: Option<String>,
    pub next_hosted_heartbeat_due_at: Option<Instant>,
    next_dispatch_due_at: Option<Instant>,
    next_dispatch_seq: u64,
}

impl Default for StarterJobsState {
    fn default() -> Self {
        Self {
            load_state: PaneLoadState::Loading,
            last_error: None,
            last_action: Some("Waiting for starter demand lane snapshot".to_string()),
            jobs: Vec::new(),
            selected_job_id: None,
            kill_switch_enabled: false,
            budget_cap_sats: STARTER_DEMAND_DEFAULT_BUDGET_SATS,
            budget_allocated_sats: 0,
            dispatch_interval_seconds: STARTER_DEMAND_DEFAULT_DISPATCH_INTERVAL_SECONDS,
            max_inflight_jobs: STARTER_DEMAND_DEFAULT_MAX_INFLIGHT_JOBS,
            last_dispatched_at: None,
            next_hosted_sync_due_at: None,
            active_hosted_request_id: None,
            next_hosted_heartbeat_due_at: None,
            next_dispatch_due_at: None,
            next_dispatch_seq: 0,
        }
    }
}

impl StarterJobsState {
    pub fn select_by_index(&mut self, index: usize) -> bool {
        let Some(job_id) = self.jobs.get(index).map(|job| job.job_id.clone()) else {
            return false;
        };
        self.selected_job_id = Some(job_id);
        self.pane_clear_error();
        true
    }

    pub fn selected(&self) -> Option<&StarterJobRow> {
        let selected = self.selected_job_id.as_deref()?;
        self.jobs.iter().find(|job| job.job_id == selected)
    }

    pub fn toggle_kill_switch(&mut self) -> bool {
        self.kill_switch_enabled = !self.kill_switch_enabled;
        let state = if self.kill_switch_enabled {
            "enabled"
        } else {
            "disabled"
        };
        self.pane_set_ready(format!("Starter demand kill switch {state}"));
        self.kill_switch_enabled
    }

    pub fn sync_hosted_offers(
        &mut self,
        jobs: Vec<StarterJobRow>,
        budget_cap_sats: u64,
        budget_allocated_sats: u64,
        dispatch_interval_seconds: u64,
        max_inflight_jobs: usize,
        next_sync_due_at: Option<Instant>,
        reason: &str,
    ) {
        let existing_by_job_id = self
            .jobs
            .iter()
            .map(|job| (job.job_id.clone(), job.clone()))
            .collect::<std::collections::HashMap<_, _>>();
        self.jobs = jobs
            .into_iter()
            .map(|mut job| {
                if let Some(existing) = existing_by_job_id.get(job.job_id.as_str()) {
                    if existing.status != StarterJobStatus::Queued
                        && job.status == StarterJobStatus::Queued
                    {
                        job.status = existing.status;
                    }
                    if job.payout_pointer.is_none() {
                        job.payout_pointer = existing.payout_pointer.clone();
                    }
                    if job.execution_started_at_unix_ms.is_none() {
                        job.execution_started_at_unix_ms = existing.execution_started_at_unix_ms;
                    }
                    if job.execution_expires_at_unix_ms.is_none() {
                        job.execution_expires_at_unix_ms = existing.execution_expires_at_unix_ms;
                    }
                    if job.last_heartbeat_at_unix_ms.is_none() {
                        job.last_heartbeat_at_unix_ms = existing.last_heartbeat_at_unix_ms;
                    }
                    if job.next_heartbeat_due_at_unix_ms.is_none() {
                        job.next_heartbeat_due_at_unix_ms = existing.next_heartbeat_due_at_unix_ms;
                    }
                    if job.start_confirm_by_unix_ms.is_none() {
                        job.start_confirm_by_unix_ms = existing.start_confirm_by_unix_ms;
                    }
                }
                job
            })
            .collect();
        self.budget_cap_sats = budget_cap_sats.max(1);
        self.budget_allocated_sats = budget_allocated_sats.min(self.budget_cap_sats);
        self.dispatch_interval_seconds = dispatch_interval_seconds.clamp(1, 3600);
        self.max_inflight_jobs = max_inflight_jobs.clamp(1, 1);
        self.next_hosted_sync_due_at = next_sync_due_at;
        if let Some(selected_id) = self.selected_job_id.as_deref()
            && !self.jobs.iter().any(|job| job.job_id == selected_id)
        {
            self.selected_job_id = self.jobs.first().map(|job| job.job_id.clone());
        } else if self.selected_job_id.is_none() {
            self.selected_job_id = self.jobs.first().map(|job| job.job_id.clone());
        }
        self.pane_set_ready(reason.to_string());
    }

    pub fn clear_hosted_offers(&mut self, reason: &str) -> bool {
        self.clear_hosted_offers_except(reason, None)
    }

    pub fn clear_hosted_offers_except(&mut self, reason: &str, keep_job_id: Option<&str>) -> bool {
        let changed = !self.jobs.is_empty()
            || self.selected_job_id.is_some()
            || self.load_state != PaneLoadState::Ready
            || self.last_action.as_deref() != Some(reason);
        self.jobs
            .retain(|job| keep_job_id == Some(job.job_id.as_str()));
        self.selected_job_id = keep_job_id
            .and_then(|job_id| self.jobs.iter().find(|job| job.job_id == job_id))
            .map(|job| job.job_id.clone());
        if self
            .active_hosted_request_id
            .as_deref()
            .is_some_and(|request_id| keep_job_id != Some(request_id))
        {
            self.active_hosted_request_id = None;
            self.next_hosted_heartbeat_due_at = None;
        }
        self.next_hosted_sync_due_at = None;
        self.pane_set_ready(reason.to_string());
        changed
    }

    pub fn hosted_offer(&self, job_id: &str) -> Option<&StarterJobRow> {
        self.jobs.iter().find(|job| job.job_id == job_id)
    }

    pub fn mark_running(
        &mut self,
        job_id: &str,
        execution_started_at_unix_ms: Option<u64>,
        execution_expires_at_unix_ms: Option<u64>,
        last_heartbeat_at_unix_ms: Option<u64>,
        next_heartbeat_due_at_unix_ms: Option<u64>,
        next_heartbeat_due_at: Option<Instant>,
    ) {
        let updated_job_id = self
            .jobs
            .iter_mut()
            .find(|job| job.job_id == job_id)
            .map(|job| {
                job.status = StarterJobStatus::Running;
                job.execution_started_at_unix_ms = execution_started_at_unix_ms;
                job.execution_expires_at_unix_ms = execution_expires_at_unix_ms;
                job.last_heartbeat_at_unix_ms = last_heartbeat_at_unix_ms;
                job.next_heartbeat_due_at_unix_ms = next_heartbeat_due_at_unix_ms;
                job.job_id.clone()
            });
        if let Some(updated_job_id) = updated_job_id {
            self.active_hosted_request_id = Some(updated_job_id.clone());
            self.next_hosted_heartbeat_due_at = next_heartbeat_due_at;
            self.selected_job_id = Some(updated_job_id.clone());
            self.pane_set_ready(format!("Hosted starter offer running {}", updated_job_id));
        }
    }

    pub fn mark_heartbeat(
        &mut self,
        job_id: &str,
        last_heartbeat_at_unix_ms: u64,
        next_heartbeat_due_at_unix_ms: u64,
        execution_expires_at_unix_ms: u64,
        next_heartbeat_due_at: Option<Instant>,
    ) {
        if let Some(job) = self.jobs.iter_mut().find(|job| job.job_id == job_id) {
            job.status = StarterJobStatus::Running;
            job.last_heartbeat_at_unix_ms = Some(last_heartbeat_at_unix_ms);
            job.next_heartbeat_due_at_unix_ms = Some(next_heartbeat_due_at_unix_ms);
            job.execution_expires_at_unix_ms = Some(execution_expires_at_unix_ms);
            self.active_hosted_request_id = Some(job_id.to_string());
            self.next_hosted_heartbeat_due_at = next_heartbeat_due_at;
            self.pane_set_ready(format!("Hosted starter lease heartbeat {}", job_id));
        }
    }

    pub fn mark_released(&mut self, job_id: &str, reason: &str) {
        self.jobs.retain(|job| job.job_id != job_id);
        if self.selected_job_id.as_deref() == Some(job_id) {
            self.selected_job_id = self.jobs.first().map(|job| job.job_id.clone());
        }
        if self.active_hosted_request_id.as_deref() == Some(job_id) {
            self.active_hosted_request_id = None;
            self.next_hosted_heartbeat_due_at = None;
        }
        self.pane_set_ready(format!(
            "Hosted starter lease released {} ({})",
            job_id, reason
        ));
    }

    pub fn mark_completed(&mut self, job_id: &str, payment_pointer: &str) {
        let updated_job_id = self
            .jobs
            .iter_mut()
            .find(|job| job.job_id == job_id)
            .map(|job| {
                job.status = StarterJobStatus::Completed;
                job.payout_pointer = Some(payment_pointer.to_string());
                job.next_heartbeat_due_at_unix_ms = None;
                job.job_id.clone()
            });
        if let Some(updated_job_id) = updated_job_id {
            if self.active_hosted_request_id.as_deref() == Some(updated_job_id.as_str()) {
                self.active_hosted_request_id = None;
                self.next_hosted_heartbeat_due_at = None;
            }
            self.pane_set_ready(format!("Hosted starter offer completed {}", updated_job_id));
        }
    }

    pub fn apply_dispatch_controls(
        &mut self,
        budget_cap_sats: u64,
        dispatch_interval_seconds: u64,
        max_inflight_jobs: usize,
    ) {
        let budget_cap_sats = budget_cap_sats.max(1);
        let dispatch_interval_seconds = dispatch_interval_seconds.clamp(1, 3600);
        let max_inflight_jobs = max_inflight_jobs.clamp(1, 1);
        if self.budget_cap_sats == budget_cap_sats
            && self.dispatch_interval_seconds == dispatch_interval_seconds
            && self.max_inflight_jobs == max_inflight_jobs
        {
            return;
        }

        self.budget_cap_sats = budget_cap_sats;
        self.dispatch_interval_seconds = dispatch_interval_seconds;
        self.max_inflight_jobs = max_inflight_jobs;
        self.last_action = Some(format!(
            "Starter demand controls updated (budget={} interval={}s max_inflight={})",
            format_sats_amount(self.budget_cap_sats),
            self.dispatch_interval_seconds,
            self.max_inflight_jobs
        ));
    }

    pub fn inflight_jobs(&self) -> usize {
        self.jobs
            .iter()
            .filter(|job| job.status != StarterJobStatus::Completed)
            .count()
    }

    pub fn budget_remaining_sats(&self) -> u64 {
        self.budget_cap_sats
            .saturating_sub(self.budget_allocated_sats)
    }

    pub fn dispatch_next_if_due(&mut self, now: Instant) -> Result<Option<StarterJobRow>, String> {
        if self.kill_switch_enabled {
            self.last_action = Some("Starter demand paused by kill switch".to_string());
            return Ok(None);
        }

        if let Some(next_due_at) = self.next_dispatch_due_at
            && now < next_due_at
        {
            return Ok(None);
        }

        if self.inflight_jobs() >= self.max_inflight_jobs {
            self.next_dispatch_due_at =
                Some(now + std::time::Duration::from_secs(self.dispatch_interval_seconds.max(1)));
            self.last_action = Some(format!(
                "Starter demand waiting ({} inflight, max={})",
                self.inflight_jobs(),
                self.max_inflight_jobs
            ));
            return Ok(None);
        }

        let remaining = self.budget_remaining_sats();
        let Some(template) = self.next_template_for_remaining_budget(remaining) else {
            self.next_dispatch_due_at =
                Some(now + std::time::Duration::from_secs(self.dispatch_interval_seconds.max(1)));
            self.last_action = Some(format!(
                "Starter demand budget exhausted (allocated {} / cap {})",
                format_sats_amount(self.budget_allocated_sats),
                format_sats_amount(self.budget_cap_sats)
            ));
            return Ok(None);
        };

        let job_id = format!("starter-quest-{:04}", self.next_dispatch_seq);
        self.next_dispatch_seq = self.next_dispatch_seq.saturating_add(1);
        let job = StarterJobRow {
            job_id: job_id.clone(),
            summary: template.summary.to_string(),
            payout_sats: template.payout_sats,
            eligible: true,
            status: StarterJobStatus::Queued,
            payout_pointer: None,
            start_confirm_by_unix_ms: None,
            execution_started_at_unix_ms: None,
            execution_expires_at_unix_ms: None,
            last_heartbeat_at_unix_ms: None,
            next_heartbeat_due_at_unix_ms: None,
        };
        self.jobs.insert(0, job.clone());
        if self.selected_job_id.is_none() {
            self.selected_job_id = Some(job_id.clone());
        }
        self.budget_allocated_sats = self.budget_allocated_sats.saturating_add(job.payout_sats);
        self.last_dispatched_at = Some(now);
        self.next_dispatch_due_at =
            Some(now + std::time::Duration::from_secs(self.dispatch_interval_seconds.max(1)));
        self.pane_set_ready(format!(
            "Starter demand dispatched {} ({}, remaining {})",
            job_id,
            format_sats_amount(job.payout_sats),
            format_sats_amount(self.budget_remaining_sats())
        ));
        Ok(Some(job))
    }

    pub fn rollback_dispatched_job(&mut self, job_id: &str) -> bool {
        let Some(index) = self
            .jobs
            .iter()
            .position(|job| job.job_id == job_id && job.status == StarterJobStatus::Queued)
        else {
            return false;
        };

        let removed = self.jobs.remove(index);
        self.budget_allocated_sats = self
            .budget_allocated_sats
            .saturating_sub(removed.payout_sats);
        if self.selected_job_id.as_deref() == Some(job_id) {
            self.selected_job_id = self.jobs.first().map(|job| job.job_id.clone());
        }
        self.last_action = Some(format!("Rolled back starter dispatch {}", removed.job_id));
        true
    }

    pub fn start_selected_execution(&mut self) -> Result<(String, u64), String> {
        let selected = self
            .selected_job_id
            .as_deref()
            .ok_or_else(|| "Select a starter job first".to_string())?
            .to_string();
        let (job_id, payout_sats) = {
            let Some(job) = self.jobs.iter_mut().find(|job| job.job_id == selected) else {
                return Err(self.pane_set_error("Selected starter job no longer exists"));
            };
            if !job.eligible {
                return Err(self.pane_set_error("Starter job is not eligible yet"));
            }
            if job.status == StarterJobStatus::Completed {
                return Err(self.pane_set_error("Starter job already completed"));
            }

            job.status = StarterJobStatus::Running;
            job.payout_pointer = None;
            (job.job_id.clone(), job.payout_sats)
        };
        self.pane_set_ready(format!("Starter quest execution started for {}", job_id));
        Ok((job_id, payout_sats))
    }

    pub fn complete_selected_with_payment(
        &mut self,
        payment_pointer: &str,
    ) -> Result<(String, u64, String), String> {
        let selected = self
            .selected_job_id
            .as_deref()
            .ok_or_else(|| "Select a starter job first".to_string())?
            .to_string();
        let payment_pointer = payment_pointer.trim();
        if payment_pointer.is_empty() {
            return Err(self.pane_set_error("Wallet payment pointer is required"));
        }
        if payment_pointer.starts_with("pay:") || payment_pointer.starts_with("pay-req-") {
            return Err(self.pane_set_error(
                "Synthetic payment pointer is not allowed for starter payout settlement",
            ));
        }

        let (job_id, payout_sats, payout_pointer) = {
            let Some(job) = self.jobs.iter_mut().find(|job| job.job_id == selected) else {
                return Err(self.pane_set_error("Selected starter job no longer exists"));
            };
            if !job.eligible {
                return Err(self.pane_set_error("Starter job is not eligible yet"));
            }
            if job.status != StarterJobStatus::Running {
                return Err(
                    self.pane_set_error("Starter job must be running before payout settlement")
                );
            }

            job.status = StarterJobStatus::Completed;
            job.payout_pointer = Some(payment_pointer.to_string());
            (
                job.job_id.clone(),
                job.payout_sats,
                payment_pointer.to_string(),
            )
        };
        self.pane_set_ready(format!(
            "Completed starter quest {} with wallet-confirmed payout ({})",
            job_id,
            format_sats_amount(payout_sats)
        ));
        Ok((job_id, payout_sats, payout_pointer))
    }

    fn next_template_for_remaining_budget(
        &self,
        remaining_budget_sats: u64,
    ) -> Option<&'static StarterDemandTemplate> {
        let total = STARTER_DEMAND_TEMPLATES.len();
        for offset in 0..total {
            let index = (self.next_dispatch_seq as usize + offset) % total;
            let template = &STARTER_DEMAND_TEMPLATES[index];
            if template.payout_sats <= remaining_budget_sats {
                return Some(template);
            }
        }
        None
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReciprocalLoopDirection {
    LocalToPeer,
    PeerToLocal,
}

impl ReciprocalLoopDirection {
    pub const fn label(self) -> &'static str {
        match self {
            Self::LocalToPeer => "local->peer",
            Self::PeerToLocal => "peer->local",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReciprocalLoopFailureClass {
    Dispatch,
    Payment,
    Job,
}

impl ReciprocalLoopFailureClass {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Dispatch => "dispatch",
            Self::Payment => "payment",
            Self::Job => "job",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReciprocalLoopFailureDisposition {
    Recoverable,
    Terminal,
}

impl ReciprocalLoopFailureDisposition {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Recoverable => "recoverable",
            Self::Terminal => "terminal",
        }
    }
}

const RECIPROCAL_LOOP_DEFAULT_AMOUNT_SATS: u64 = 10;
const RECIPROCAL_LOOP_DEFAULT_TIMEOUT_SECONDS: u64 = 90;
const RECIPROCAL_LOOP_DEFAULT_STALE_TIMEOUT_SECONDS: u64 = 120;
const RECIPROCAL_LOOP_DEFAULT_MAX_RETRY_ATTEMPTS: u32 = 6;
const RECIPROCAL_LOOP_DEFAULT_RETRY_BACKOFF_SECONDS: u64 = 2;
const RECIPROCAL_LOOP_DEFAULT_RETRY_BACKOFF_MAX_SECONDS: u64 = 60;
const RECIPROCAL_LOOP_DEFAULT_MAX_IN_FLIGHT_LOCAL_TO_PEER: u8 = 1;
const RECIPROCAL_LOOP_DEFAULT_MAX_IN_FLIGHT_PEER_TO_LOCAL: u8 = 1;
const RECIPROCAL_LOOP_SCOPE_ID: &str = "earn.loop.pingpong.v1";

pub struct ReciprocalLoopState {
    pub load_state: PaneLoadState,
    pub last_error: Option<String>,
    pub last_action: Option<String>,
    pub running: bool,
    pub kill_switch_active: bool,
    pub amount_sats: u64,
    pub timeout_seconds: u64,
    pub stale_timeout_seconds: u64,
    pub skill_scope_id: String,
    pub local_pubkey: Option<String>,
    pub peer_pubkey: Option<String>,
    pub next_direction: ReciprocalLoopDirection,
    pub max_in_flight_local_to_peer: u8,
    pub max_in_flight_peer_to_local: u8,
    pub in_flight_request_id: Option<String>,
    pub in_flight_since_epoch_seconds: Option<u64>,
    pub peer_wait_since_epoch_seconds: Option<u64>,
    pub retry_attempts: u32,
    pub max_retry_attempts: u32,
    pub retry_backoff_seconds: u64,
    pub retry_backoff_max_seconds: u64,
    pub retry_backoff_until_epoch_seconds: Option<u64>,
    pub local_to_peer_dispatched: u64,
    pub local_to_peer_paid: u64,
    pub local_to_peer_failed: u64,
    pub peer_to_local_paid: u64,
    pub peer_to_local_failed: u64,
    pub sats_sent: u64,
    pub sats_received: u64,
    pub last_payment_pointer: Option<String>,
    pub last_failure_class: Option<ReciprocalLoopFailureClass>,
    pub last_failure_disposition: Option<ReciprocalLoopFailureDisposition>,
    pub last_failure_detail: Option<String>,
    seen_terminal_outbound_request_ids: HashSet<String>,
    seen_terminal_inbound_job_ids: HashSet<String>,
}

impl Default for ReciprocalLoopState {
    fn default() -> Self {
        Self {
            load_state: PaneLoadState::Loading,
            last_error: None,
            last_action: Some("Reciprocal loop is idle".to_string()),
            running: false,
            kill_switch_active: false,
            amount_sats: RECIPROCAL_LOOP_DEFAULT_AMOUNT_SATS,
            timeout_seconds: RECIPROCAL_LOOP_DEFAULT_TIMEOUT_SECONDS,
            stale_timeout_seconds: RECIPROCAL_LOOP_DEFAULT_STALE_TIMEOUT_SECONDS,
            skill_scope_id: RECIPROCAL_LOOP_SCOPE_ID.to_string(),
            local_pubkey: None,
            peer_pubkey: None,
            next_direction: ReciprocalLoopDirection::LocalToPeer,
            max_in_flight_local_to_peer: RECIPROCAL_LOOP_DEFAULT_MAX_IN_FLIGHT_LOCAL_TO_PEER,
            max_in_flight_peer_to_local: RECIPROCAL_LOOP_DEFAULT_MAX_IN_FLIGHT_PEER_TO_LOCAL,
            in_flight_request_id: None,
            in_flight_since_epoch_seconds: None,
            peer_wait_since_epoch_seconds: None,
            retry_attempts: 0,
            max_retry_attempts: RECIPROCAL_LOOP_DEFAULT_MAX_RETRY_ATTEMPTS,
            retry_backoff_seconds: RECIPROCAL_LOOP_DEFAULT_RETRY_BACKOFF_SECONDS,
            retry_backoff_max_seconds: RECIPROCAL_LOOP_DEFAULT_RETRY_BACKOFF_MAX_SECONDS,
            retry_backoff_until_epoch_seconds: None,
            local_to_peer_dispatched: 0,
            local_to_peer_paid: 0,
            local_to_peer_failed: 0,
            peer_to_local_paid: 0,
            peer_to_local_failed: 0,
            sats_sent: 0,
            sats_received: 0,
            last_payment_pointer: None,
            last_failure_class: None,
            last_failure_disposition: None,
            last_failure_detail: None,
            seen_terminal_outbound_request_ids: HashSet::new(),
            seen_terminal_inbound_job_ids: HashSet::new(),
        }
    }
}

impl ReciprocalLoopState {
    pub fn normalized_pubkey(value: &str) -> String {
        value.trim().to_ascii_lowercase()
    }

    pub fn set_local_pubkey(&mut self, pubkey: Option<&str>) {
        self.local_pubkey = pubkey
            .map(Self::normalized_pubkey)
            .filter(|value| !value.is_empty());
    }

    pub fn set_peer_pubkey(&mut self, pubkey: Option<&str>) {
        self.peer_pubkey = pubkey
            .map(Self::normalized_pubkey)
            .filter(|value| !value.is_empty());
    }

    pub fn start(&mut self) -> Result<(), String> {
        let Some(local) = self.local_pubkey.as_deref() else {
            return Err(self.pane_set_error("Reciprocal loop start requires local identity pubkey"));
        };
        let Some(peer) = self.peer_pubkey.as_deref() else {
            return Err(self.pane_set_error("Reciprocal loop start requires peer pubkey"));
        };
        if local == peer {
            return Err(
                self.pane_set_error("Reciprocal loop peer pubkey must differ from local pubkey")
            );
        }

        self.running = true;
        self.kill_switch_active = false;
        self.next_direction = if local <= peer {
            ReciprocalLoopDirection::LocalToPeer
        } else {
            ReciprocalLoopDirection::PeerToLocal
        };
        self.in_flight_request_id = None;
        self.in_flight_since_epoch_seconds = None;
        self.peer_wait_since_epoch_seconds = None;
        self.retry_attempts = 0;
        self.retry_backoff_until_epoch_seconds = None;
        self.last_failure_class = None;
        self.last_failure_disposition = None;
        self.last_failure_detail = None;
        self.pane_set_ready(format!(
            "Reciprocal loop started (peer={} next={})",
            peer,
            self.next_direction.label()
        ));
        Ok(())
    }

    pub fn stop(&mut self, reason: &str) {
        self.running = false;
        self.kill_switch_active = true;
        self.in_flight_request_id = None;
        self.in_flight_since_epoch_seconds = None;
        self.peer_wait_since_epoch_seconds = None;
        self.retry_backoff_until_epoch_seconds = None;
        self.pane_set_ready(format!(
            "Reciprocal loop stopped ({})",
            reason.trim().to_string()
        ));
    }

    pub fn reset_counters(&mut self) {
        self.in_flight_request_id = None;
        self.in_flight_since_epoch_seconds = None;
        self.peer_wait_since_epoch_seconds = None;
        self.local_to_peer_dispatched = 0;
        self.local_to_peer_paid = 0;
        self.local_to_peer_failed = 0;
        self.peer_to_local_paid = 0;
        self.peer_to_local_failed = 0;
        self.sats_sent = 0;
        self.sats_received = 0;
        self.retry_attempts = 0;
        self.retry_backoff_until_epoch_seconds = None;
        self.last_payment_pointer = None;
        self.last_failure_class = None;
        self.last_failure_disposition = None;
        self.last_failure_detail = None;
        self.seen_terminal_outbound_request_ids.clear();
        self.seen_terminal_inbound_job_ids.clear();
        self.pane_set_ready("Reciprocal loop counters reset");
    }

    pub fn in_flight_local_to_peer(&self) -> u8 {
        if self.in_flight_request_id.is_some() {
            1
        } else {
            0
        }
    }

    pub fn in_flight_peer_to_local(&self) -> u8 {
        if self.running
            && self.in_flight_request_id.is_none()
            && self.next_direction == ReciprocalLoopDirection::PeerToLocal
        {
            1
        } else {
            0
        }
    }

    pub fn ready_to_dispatch(&self) -> bool {
        self.running
            && !self.kill_switch_active
            && self.next_direction == ReciprocalLoopDirection::LocalToPeer
            && self.in_flight_local_to_peer() < self.max_in_flight_local_to_peer.max(1)
            && self.in_flight_peer_to_local() < self.max_in_flight_peer_to_local.max(1)
            && self.in_flight_request_id.is_none()
            && self.retry_backoff_until_epoch_seconds.is_none()
    }

    pub fn clear_retry_backoff_if_elapsed(&mut self, now_epoch_seconds: u64) -> bool {
        let Some(until) = self.retry_backoff_until_epoch_seconds else {
            return false;
        };
        if now_epoch_seconds < until {
            return false;
        }
        self.retry_backoff_until_epoch_seconds = None;
        self.pane_set_ready("Reciprocal loop backoff elapsed; dispatch may resume");
        true
    }

    pub fn in_backoff_window(&self, now_epoch_seconds: u64) -> bool {
        self.retry_backoff_until_epoch_seconds
            .is_some_and(|until| now_epoch_seconds < until)
    }

    pub fn mark_peer_wait_started(&mut self, now_epoch_seconds: u64) {
        if self.running
            && self.next_direction == ReciprocalLoopDirection::PeerToLocal
            && self.in_flight_request_id.is_none()
            && self.peer_wait_since_epoch_seconds.is_none()
        {
            self.peer_wait_since_epoch_seconds = Some(now_epoch_seconds);
        }
    }

    pub fn outbound_stale_timed_out(&self, now_epoch_seconds: u64) -> bool {
        let Some(since) = self.in_flight_since_epoch_seconds else {
            return false;
        };
        now_epoch_seconds.saturating_sub(since) >= self.stale_timeout_seconds
    }

    pub fn inbound_wait_timed_out(&self, now_epoch_seconds: u64) -> bool {
        if !self.running
            || self.next_direction != ReciprocalLoopDirection::PeerToLocal
            || self.in_flight_request_id.is_some()
        {
            return false;
        }
        let Some(since) = self.peer_wait_since_epoch_seconds else {
            return false;
        };
        now_epoch_seconds.saturating_sub(since) >= self.stale_timeout_seconds
    }

    pub fn mark_outbound_stale_timeout(&mut self) -> Option<String> {
        let request_id = self.in_flight_request_id.take();
        let Some(request_id) = request_id else {
            return None;
        };
        self.seen_terminal_outbound_request_ids
            .insert(request_id.clone());
        self.local_to_peer_failed = self.local_to_peer_failed.saturating_add(1);
        self.in_flight_since_epoch_seconds = None;
        self.next_direction = ReciprocalLoopDirection::LocalToPeer;
        self.peer_wait_since_epoch_seconds = None;
        Some(request_id)
    }

    pub fn mark_inbound_stale_timeout(&mut self) {
        self.peer_to_local_failed = self.peer_to_local_failed.saturating_add(1);
        self.next_direction = ReciprocalLoopDirection::LocalToPeer;
        self.peer_wait_since_epoch_seconds = None;
    }

    pub fn in_flight_limit_violation(&self) -> Option<String> {
        let local_in_flight = self.in_flight_local_to_peer();
        if local_in_flight > self.max_in_flight_local_to_peer.max(1) {
            return Some(format!(
                "local->peer in-flight {} exceeds max {}",
                local_in_flight, self.max_in_flight_local_to_peer
            ));
        }
        let peer_in_flight = self.in_flight_peer_to_local();
        if peer_in_flight > self.max_in_flight_peer_to_local.max(1) {
            return Some(format!(
                "peer->local in-flight {} exceeds max {}",
                peer_in_flight, self.max_in_flight_peer_to_local
            ));
        }
        None
    }

    pub fn clear_retry_state_after_success(&mut self) {
        self.retry_attempts = 0;
        self.retry_backoff_until_epoch_seconds = None;
        self.last_failure_class = None;
        self.last_failure_disposition = None;
        self.last_failure_detail = None;
    }

    pub fn record_recoverable_failure(
        &mut self,
        class: ReciprocalLoopFailureClass,
        detail: &str,
        now_epoch_seconds: u64,
    ) -> bool {
        let detail = detail.trim();
        let detail = if detail.is_empty() {
            "recoverable loop runtime error"
        } else {
            detail
        };
        self.last_failure_class = Some(class);
        self.last_failure_disposition = Some(ReciprocalLoopFailureDisposition::Recoverable);
        self.last_failure_detail = Some(detail.to_string());

        self.retry_attempts = self.retry_attempts.saturating_add(1);
        if self.retry_attempts > self.max_retry_attempts {
            self.record_terminal_failure(
                class,
                format!(
                    "{} (retry budget exceeded {}/{})",
                    detail, self.retry_attempts, self.max_retry_attempts
                )
                .as_str(),
            );
            return true;
        }

        let retry_index = self.retry_attempts.saturating_sub(1).min(20);
        let backoff_multiplier = 1u64.checked_shl(retry_index).unwrap_or(u64::MAX);
        let backoff_seconds = self
            .retry_backoff_seconds
            .saturating_mul(backoff_multiplier)
            .clamp(1, self.retry_backoff_max_seconds.max(1));
        self.retry_backoff_until_epoch_seconds =
            Some(now_epoch_seconds.saturating_add(backoff_seconds));
        self.next_direction = ReciprocalLoopDirection::LocalToPeer;
        self.in_flight_request_id = None;
        self.in_flight_since_epoch_seconds = None;
        self.peer_wait_since_epoch_seconds = None;
        let _ = self.pane_set_error(format!(
            "Reciprocal loop recoverable failure class={} retry={}/{} backoff={}s detail={}",
            class.label(),
            self.retry_attempts,
            self.max_retry_attempts,
            backoff_seconds,
            detail
        ));
        false
    }

    pub fn record_terminal_failure(&mut self, class: ReciprocalLoopFailureClass, detail: &str) {
        let detail = detail.trim();
        let detail = if detail.is_empty() {
            "terminal loop runtime error"
        } else {
            detail
        };
        self.running = false;
        self.kill_switch_active = true;
        self.next_direction = ReciprocalLoopDirection::LocalToPeer;
        self.in_flight_request_id = None;
        self.in_flight_since_epoch_seconds = None;
        self.peer_wait_since_epoch_seconds = None;
        self.retry_backoff_until_epoch_seconds = None;
        self.last_failure_class = Some(class);
        self.last_failure_disposition = Some(ReciprocalLoopFailureDisposition::Terminal);
        self.last_failure_detail = Some(detail.to_string());
        let _ = self.pane_set_error(format!(
            "Reciprocal loop terminal failure class={} detail={}",
            class.label(),
            detail
        ));
    }

    pub fn register_outbound_dispatch(&mut self, request_id: &str, now_epoch_seconds: u64) {
        let request_id = request_id.trim();
        if request_id.is_empty() {
            return;
        }

        self.local_to_peer_dispatched = self.local_to_peer_dispatched.saturating_add(1);
        self.in_flight_request_id = Some(request_id.to_string());
        self.in_flight_since_epoch_seconds = Some(now_epoch_seconds);
        self.peer_wait_since_epoch_seconds = None;
        self.next_direction = ReciprocalLoopDirection::PeerToLocal;
        self.clear_retry_state_after_success();
        self.pane_set_ready(format!(
            "Reciprocal loop dispatched request {} ({})",
            request_id,
            format_sats_amount(self.amount_sats)
        ));
    }

    pub fn match_outbound_request(&self, request: &SubmittedNetworkRequest) -> bool {
        if request.budget_sats != self.amount_sats
            || request.skill_scope_id.as_deref() != Some(self.skill_scope_id.as_str())
        {
            return false;
        }
        let Some(peer) = self.peer_pubkey.as_deref() else {
            return false;
        };
        request
            .target_provider_pubkeys
            .iter()
            .map(|provider| Self::normalized_pubkey(provider.as_str()))
            .any(|provider| provider == peer)
    }

    pub fn reconcile_outbound_terminal_statuses(
        &mut self,
        submitted_requests: &[SubmittedNetworkRequest],
    ) -> bool {
        let mut changed = false;
        for request in submitted_requests {
            if !self.match_outbound_request(request) {
                continue;
            }
            if self
                .seen_terminal_outbound_request_ids
                .contains(request.request_id.as_str())
            {
                continue;
            }

            match request.status {
                NetworkRequestStatus::Paid | NetworkRequestStatus::Completed => {
                    self.seen_terminal_outbound_request_ids
                        .insert(request.request_id.clone());
                    self.local_to_peer_paid = self.local_to_peer_paid.saturating_add(1);
                    self.sats_sent = self.sats_sent.saturating_add(self.amount_sats);
                    if let Some(pointer) = request.last_payment_pointer.as_deref() {
                        self.last_payment_pointer = Some(pointer.to_string());
                    }
                    if self.in_flight_request_id.as_deref() == Some(request.request_id.as_str()) {
                        self.in_flight_request_id = None;
                        self.in_flight_since_epoch_seconds = None;
                    }
                    self.next_direction = ReciprocalLoopDirection::PeerToLocal;
                    self.peer_wait_since_epoch_seconds = None;
                    self.clear_retry_state_after_success();
                    self.pane_set_ready(format!(
                        "Reciprocal loop outbound settled request {} pointer={}",
                        request.request_id,
                        request.last_payment_pointer.as_deref().unwrap_or("none")
                    ));
                    changed = true;
                }
                NetworkRequestStatus::Failed => {
                    self.seen_terminal_outbound_request_ids
                        .insert(request.request_id.clone());
                    self.local_to_peer_failed = self.local_to_peer_failed.saturating_add(1);
                    if self.in_flight_request_id.as_deref() == Some(request.request_id.as_str()) {
                        self.in_flight_request_id = None;
                        self.in_flight_since_epoch_seconds = None;
                    }
                    self.next_direction = ReciprocalLoopDirection::LocalToPeer;
                    self.peer_wait_since_epoch_seconds = None;
                    self.last_failure_class = Some(ReciprocalLoopFailureClass::Payment);
                    self.last_failure_disposition =
                        Some(ReciprocalLoopFailureDisposition::Recoverable);
                    self.last_failure_detail = request.payment_error.clone().or_else(|| {
                        Some("loop outbound request reached failed status".to_string())
                    });
                    let _ = self.pane_set_error(format!(
                        "Reciprocal loop outbound failed request {}",
                        request.request_id
                    ));
                    changed = true;
                }
                _ => {}
            }
        }
        changed
    }

    pub fn match_inbound_history_row(&self, row: &crate::app_state::JobHistoryReceiptRow) -> bool {
        row.demand_source == crate::app_state::JobDemandSource::OpenNetwork
            && row.payout_sats == self.amount_sats
            && row.skill_scope_id.as_deref() == Some(self.skill_scope_id.as_str())
    }

    pub fn reconcile_inbound_history(
        &mut self,
        history_rows: &[crate::app_state::JobHistoryReceiptRow],
    ) -> bool {
        let mut changed = false;
        for row in history_rows {
            if !self.match_inbound_history_row(row) {
                continue;
            }
            if self
                .seen_terminal_inbound_job_ids
                .contains(row.job_id.as_str())
            {
                continue;
            }

            self.seen_terminal_inbound_job_ids
                .insert(row.job_id.clone());
            match row.status {
                crate::app_state::JobHistoryStatus::Succeeded => {
                    self.peer_to_local_paid = self.peer_to_local_paid.saturating_add(1);
                    self.sats_received = self.sats_received.saturating_add(row.payout_sats);
                    if !row.payment_pointer.trim().is_empty() {
                        self.last_payment_pointer = Some(row.payment_pointer.clone());
                    }
                    self.next_direction = ReciprocalLoopDirection::LocalToPeer;
                    self.peer_wait_since_epoch_seconds = None;
                    self.clear_retry_state_after_success();
                    self.pane_set_ready(format!(
                        "Reciprocal loop inbound settled job {} pointer={}",
                        row.job_id, row.payment_pointer
                    ));
                    changed = true;
                }
                crate::app_state::JobHistoryStatus::Failed => {
                    self.peer_to_local_failed = self.peer_to_local_failed.saturating_add(1);
                    self.next_direction = ReciprocalLoopDirection::LocalToPeer;
                    self.peer_wait_since_epoch_seconds = None;
                    self.last_failure_class = Some(ReciprocalLoopFailureClass::Job);
                    self.last_failure_disposition =
                        Some(ReciprocalLoopFailureDisposition::Recoverable);
                    self.last_failure_detail = row.failure_reason.clone().or_else(|| {
                        Some("loop inbound job finished with failed history receipt".to_string())
                    });
                    let _ = self.pane_set_error(format!(
                        "Reciprocal loop inbound failed job {}",
                        row.job_id
                    ));
                    changed = true;
                }
            }
        }
        changed
    }
}

#[cfg(test)]
mod tests {
    use super::{
        AcceptedForwardComputeOrder, AcceptedSpotComputeOrder, ComputeQuoteMode,
        ForwardComputeQuoteCandidate, ForwardComputeRfqDraft, NetworkRequestsState,
        SpotComputeCapabilityConstraints, SpotComputeQuoteCandidate, SpotComputeRfqDraft,
    };
    use openagents_kernel_core::compute::{ComputeBackendFamily, ComputeFamily};

    fn sample_rfq() -> SpotComputeRfqDraft {
        SpotComputeRfqDraft {
            rfq_id: "rfq-spot-1".to_string(),
            compute_family: ComputeFamily::Inference,
            preferred_backend: Some(ComputeBackendFamily::Ollama),
            quantity: 1,
            window_minutes: 15,
            max_price_sats: 34,
            capability_constraints: SpotComputeCapabilityConstraints::default(),
        }
    }

    fn sample_quote(quote_id: &str, available_quantity: u64) -> SpotComputeQuoteCandidate {
        SpotComputeQuoteCandidate {
            quote_id: quote_id.to_string(),
            rfq_id: "rfq-spot-1".to_string(),
            product_id: "ollama.text_generation".to_string(),
            capacity_lot_id: format!("lot-{quote_id}"),
            provider_id: "npub1provider".to_string(),
            backend_family: ComputeBackendFamily::Ollama,
            compute_family: ComputeFamily::Inference,
            available_quantity,
            requested_quantity: 1,
            price_sats: 21,
            delivery_window_label: "15m".to_string(),
            capability_summary: "backend=ollama family=inference".to_string(),
            source_badge: "desktop.go_online".to_string(),
            terms_label: "spot session / local best effort".to_string(),
        }
    }

    fn sample_forward_rfq() -> ForwardComputeRfqDraft {
        ForwardComputeRfqDraft {
            rfq_id: "rfq-forward-1".to_string(),
            compute_family: ComputeFamily::Inference,
            preferred_backend: Some(ComputeBackendFamily::Ollama),
            quantity: 1,
            delivery_start_minutes: 180,
            window_minutes: 60,
            max_price_sats: 144,
            capability_constraints: SpotComputeCapabilityConstraints::default(),
        }
    }

    fn sample_forward_quote(
        quote_id: &str,
        available_quantity: u64,
    ) -> ForwardComputeQuoteCandidate {
        ForwardComputeQuoteCandidate {
            quote_id: quote_id.to_string(),
            rfq_id: "rfq-forward-1".to_string(),
            product_id: "ollama.text_generation".to_string(),
            capacity_lot_id: format!("forward-lot-{quote_id}"),
            provider_id: "npub1provider".to_string(),
            backend_family: ComputeBackendFamily::Ollama,
            compute_family: ComputeFamily::Inference,
            available_quantity,
            requested_quantity: 1,
            price_sats: 55,
            delivery_start_ms: 1_762_000_180_000,
            delivery_end_ms: 1_762_000_240_000,
            delivery_window_label: "start+180m for 60m".to_string(),
            capability_summary: "backend=ollama family=inference".to_string(),
            source_badge: "desktop.forward_inventory".to_string(),
            terms_label: "forward physical / committed local window".to_string(),
            collateral_summary: "bond=performance_bond".to_string(),
            remedy_summary: "default=>non_delivery remedy".to_string(),
        }
    }

    #[test]
    fn replacing_spot_quotes_selects_lowest_price_candidate() {
        let mut state = NetworkRequestsState::default();
        let mut expensive = sample_quote("quote-b", 4);
        expensive.price_sats = 34;
        let cheap = sample_quote("quote-a", 4);

        state.replace_spot_quotes(sample_rfq(), vec![expensive, cheap.clone()]);

        assert_eq!(state.spot_quote_candidates[0].quote_id, "quote-a");
        assert_eq!(state.selected_spot_quote_id.as_deref(), Some("quote-a"));
        assert_eq!(
            state.last_spot_rfq.as_ref().map(|rfq| rfq.rfq_id.as_str()),
            Some("rfq-spot-1")
        );
    }

    #[test]
    fn accepting_spot_order_consumes_selected_quote_capacity() {
        let mut state = NetworkRequestsState::default();
        state.replace_spot_quotes(sample_rfq(), vec![sample_quote("quote-a", 1)]);

        state.record_spot_order_acceptance(AcceptedSpotComputeOrder {
            order_id: "order-1".to_string(),
            rfq_id: "rfq-spot-1".to_string(),
            quote_id: "quote-a".to_string(),
            instrument_id: "instrument-1".to_string(),
            product_id: "ollama.text_generation".to_string(),
            capacity_lot_id: "lot-quote-a".to_string(),
            provider_id: "npub1provider".to_string(),
            backend_family: ComputeBackendFamily::Ollama,
            compute_family: ComputeFamily::Inference,
            quantity: 1,
            price_sats: 21,
            delivery_window_label: "15m".to_string(),
            authority_status: "spot-accepted".to_string(),
            accepted_at_epoch_seconds: 1_762_000_000,
        });

        assert!(state.spot_quote_candidates.is_empty());
        assert_eq!(state.accepted_spot_orders.len(), 1);
        assert_eq!(
            state.last_action.as_deref(),
            Some("Accepted compute quote quote-a -> instrument instrument-1")
        );
    }

    #[test]
    fn replacing_forward_quotes_switches_mode_and_selects_first_quote() {
        let mut state = NetworkRequestsState::default();
        state.replace_forward_quotes(
            sample_forward_rfq(),
            vec![sample_forward_quote("forward-a", 1)],
        );

        assert_eq!(state.quote_mode, ComputeQuoteMode::ForwardPhysical);
        assert_eq!(
            state.selected_forward_quote_id.as_deref(),
            Some("forward-a")
        );
    }

    #[test]
    fn accepting_forward_order_consumes_selected_quote_capacity() {
        let mut state = NetworkRequestsState::default();
        state.replace_forward_quotes(
            sample_forward_rfq(),
            vec![sample_forward_quote("forward-a", 1)],
        );

        state.record_forward_order_acceptance(AcceptedForwardComputeOrder {
            order_id: "forward-order-1".to_string(),
            rfq_id: "rfq-forward-1".to_string(),
            quote_id: "forward-a".to_string(),
            instrument_id: "instrument-forward-1".to_string(),
            product_id: "ollama.text_generation".to_string(),
            capacity_lot_id: "forward-lot-forward-a".to_string(),
            provider_id: "npub1provider".to_string(),
            backend_family: ComputeBackendFamily::Ollama,
            compute_family: ComputeFamily::Inference,
            quantity: 1,
            price_sats: 55,
            delivery_start_ms: 1_762_000_180_000,
            delivery_end_ms: 1_762_000_240_000,
            delivery_window_label: "start+180m for 60m".to_string(),
            collateral_summary: "bond=performance_bond".to_string(),
            remedy_summary: "default=>non_delivery remedy".to_string(),
            authority_status: "forward-accepted".to_string(),
            accepted_at_epoch_seconds: 1_762_000_000,
        });

        assert!(state.forward_quote_candidates.is_empty());
        assert_eq!(state.accepted_forward_orders.len(), 1);
        assert_eq!(state.quote_mode, ComputeQuoteMode::ForwardPhysical);
    }
}
