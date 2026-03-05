//! Relay/sync/network/starter-job pane state extracted from `app_state.rs`.

use std::time::Instant;

use crate::app_state::{PaneLoadState, PaneStatusAccess};
use crate::runtime_lanes::RuntimeCommandResponse;
use crate::sync_lifecycle::{RuntimeSyncConnectionState, RuntimeSyncHealthSnapshot};

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
    Completed,
    Failed,
}

impl NetworkRequestStatus {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Submitted => "submitted",
            Self::Streaming => "streaming",
            Self::Completed => "completed",
            Self::Failed => "failed",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SubmittedNetworkRequest {
    pub request_id: String,
    pub published_request_event_id: Option<String>,
    pub request_type: String,
    pub payload: String,
    pub target_provider_pubkeys: Vec<String>,
    pub last_provider_pubkey: Option<String>,
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
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NetworkRequestSubmission {
    pub request_id: Option<String>,
    pub request_type: String,
    pub payload: String,
    pub target_provider_pubkeys: Vec<String>,
    pub skill_scope_id: Option<String>,
    pub credit_envelope_ref: Option<String>,
    pub budget_sats: u64,
    pub timeout_seconds: u64,
    pub authority_command_seq: u64,
}

pub struct NetworkRequestsState {
    pub load_state: PaneLoadState,
    pub last_error: Option<String>,
    pub last_action: Option<String>,
    pub submitted: Vec<SubmittedNetworkRequest>,
    next_request_seq: u64,
}

impl Default for NetworkRequestsState {
    fn default() -> Self {
        Self {
            load_state: PaneLoadState::Loading,
            last_error: None,
            last_action: Some("Waiting for request lane snapshot".to_string()),
            submitted: Vec::new(),
            next_request_seq: 0,
        }
    }
}

impl NetworkRequestsState {
    pub fn queue_request_submission(
        &mut self,
        submission: NetworkRequestSubmission,
    ) -> Result<String, String> {
        let NetworkRequestSubmission {
            request_id,
            request_type,
            payload,
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
                target_provider_pubkeys,
                last_provider_pubkey: None,
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
            },
        );
        self.pane_set_ready(format!(
            "Queued buyer request {request_id} -> cmd#{authority_command_seq}"
        ));
        Ok(request_id)
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
const STARTER_DEMAND_DEFAULT_MAX_INFLIGHT_JOBS: usize = 3;

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

    pub fn apply_dispatch_controls(
        &mut self,
        budget_cap_sats: u64,
        dispatch_interval_seconds: u64,
        max_inflight_jobs: usize,
    ) {
        let budget_cap_sats = budget_cap_sats.max(1);
        let dispatch_interval_seconds = dispatch_interval_seconds.clamp(1, 3600);
        let max_inflight_jobs = max_inflight_jobs.clamp(1, 12);
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
            "Starter demand controls updated (budget={} sats interval={}s max_inflight={})",
            self.budget_cap_sats, self.dispatch_interval_seconds, self.max_inflight_jobs
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
                "Starter demand budget exhausted (allocated {} / cap {} sats)",
                self.budget_allocated_sats, self.budget_cap_sats
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
            "Starter demand dispatched {} ({} sats, remaining {} sats)",
            job_id,
            job.payout_sats,
            self.budget_remaining_sats()
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
            "Completed starter quest {} with wallet-confirmed payout ({} sats)",
            job_id, payout_sats
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
