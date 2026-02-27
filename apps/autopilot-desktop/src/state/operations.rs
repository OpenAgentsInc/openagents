//! Relay/sync/network/starter-job pane state extracted from `app_state.rs`.

use std::time::Instant;

use crate::app_state::{PaneLoadState, PaneStatusAccess, ProviderRuntimeState};
use crate::runtime_lanes::RuntimeCommandResponse;

#[allow(dead_code)]
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
        if self.relays.iter().any(|row| row.url == relay) {
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

        relay.status = RelayConnectionStatus::Connected;
        relay.latency_ms = Some(96);
        relay.last_seen_seconds_ago = Some(0);
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
    pub spacetime_connection: String,
    pub subscription_state: String,
    pub cursor_position: u64,
    pub cursor_stale_after_seconds: u64,
    pub cursor_last_advanced_seconds_ago: u64,
    pub recovery_phase: SyncRecoveryPhase,
    pub last_applied_event_seq: u64,
    pub duplicate_drop_count: u64,
    pub replay_count: u32,
}

impl Default for SyncHealthState {
    fn default() -> Self {
        Self {
            load_state: PaneLoadState::Loading,
            last_error: None,
            last_action: Some("Waiting for sync lane telemetry".to_string()),
            spacetime_connection: "unknown".to_string(),
            subscription_state: "unsubscribed".to_string(),
            cursor_position: 0,
            cursor_stale_after_seconds: 12,
            cursor_last_advanced_seconds_ago: 0,
            recovery_phase: SyncRecoveryPhase::Idle,
            last_applied_event_seq: 0,
            duplicate_drop_count: 0,
            replay_count: 0,
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
        self.cursor_last_advanced_seconds_ago = 0;
        self.pane_set_ready(format!(
            "Rebootstrapped sync stream (attempt #{})",
            self.replay_count
        ));
    }

    pub fn refresh_from_runtime(
        &mut self,
        now: Instant,
        provider_runtime: &ProviderRuntimeState,
        relay_connections: &RelayConnectionsState,
    ) {
        self.spacetime_connection = provider_runtime.mode.label().to_string();
        let connected_relays = relay_connections
            .relays
            .iter()
            .filter(|relay| relay.status == RelayConnectionStatus::Connected)
            .count();
        self.subscription_state = if connected_relays > 0 {
            "subscribed".to_string()
        } else {
            "resubscribing".to_string()
        };
        if let Some(age) = provider_runtime.heartbeat_age_seconds(now) {
            self.cursor_last_advanced_seconds_ago = age;
        } else {
            self.cursor_last_advanced_seconds_ago =
                self.cursor_last_advanced_seconds_ago.saturating_add(1);
        }

        if self.cursor_is_stale() {
            self.recovery_phase = SyncRecoveryPhase::Reconnecting;
            let _ = self.pane_set_error("Cursor stalled beyond stale threshold");
        } else if self.recovery_phase != SyncRecoveryPhase::Replaying {
            self.recovery_phase = SyncRecoveryPhase::Ready;
            *self.pane_load_state_mut() = PaneLoadState::Ready;
            self.pane_clear_error();
        }
    }
}

#[allow(dead_code)]
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
    pub request_type: String,
    pub payload: String,
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
    pub request_type: String,
    pub payload: String,
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
            request_type,
            payload,
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

        let request_id = format!("req-buy-{:04}", self.next_request_seq);
        self.next_request_seq = self.next_request_seq.saturating_add(1);
        let stream_id = format!("stream:{request_id}");
        self.submitted.insert(
            0,
            SubmittedNetworkRequest {
                request_id: request_id.clone(),
                request_type: request_type.to_string(),
                payload: payload.to_string(),
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
            crate::runtime_lanes::RuntimeCommandStatus::Accepted => NetworkRequestStatus::Streaming,
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

#[allow(dead_code)]
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

pub struct StarterJobsState {
    pub load_state: PaneLoadState,
    pub last_error: Option<String>,
    pub last_action: Option<String>,
    pub jobs: Vec<StarterJobRow>,
    pub selected_job_id: Option<String>,
}

impl Default for StarterJobsState {
    fn default() -> Self {
        Self {
            load_state: PaneLoadState::Loading,
            last_error: None,
            last_action: Some("Waiting for starter demand lane snapshot".to_string()),
            jobs: Vec::new(),
            selected_job_id: None,
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

    pub fn complete_selected(&mut self) -> Result<(String, u64, String), String> {
        let selected = self
            .selected_job_id
            .as_deref()
            .ok_or_else(|| "Select a starter job first".to_string())?
            .to_string();
        let (job_id, payout_sats, payout_pointer) = {
            let Some(job) = self.jobs.iter_mut().find(|job| job.job_id == selected) else {
                return Err(self.pane_set_error("Selected starter job no longer exists"));
            };
            if !job.eligible {
                return Err(self.pane_set_error("Starter job is not eligible yet"));
            }

            let payout_pointer = format!("pay:{}", job.job_id);
            job.status = StarterJobStatus::Completed;
            job.payout_pointer = Some(payout_pointer.clone());
            (job.job_id.clone(), job.payout_sats, payout_pointer)
        };

        self.pane_set_ready(format!(
            "Completed starter job {} ({} sats)",
            job_id, payout_sats
        ));
        Ok((job_id, payout_sats, payout_pointer))
    }
}
