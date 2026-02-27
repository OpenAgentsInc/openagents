use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use std::{cell::RefCell, rc::Rc};

use nostr::NostrIdentity;
use wgpui::components::TextInput;
use wgpui::components::hud::{CommandPalette, Hotbar, PaneFrame, ResizablePane, ResizeEdge};
use wgpui::renderer::Renderer;
use wgpui::{Bounds, EventContext, Modifiers, Point, TextSystem};
use winit::window::Window;

use crate::runtime_lanes::{
    AcCreditCommand, AcLaneSnapshot, AcLaneWorker, RuntimeCommandResponse, SaLaneSnapshot,
    SaLaneWorker, SaLifecycleCommand, SkillTrustTier, SklDiscoveryTrustCommand, SklLaneSnapshot,
    SklLaneWorker,
};
use crate::spark_wallet::{SparkPaneState, SparkWalletWorker};

pub const WINDOW_TITLE: &str = "Autopilot";
pub const WINDOW_WIDTH: f64 = 1280.0;
pub const WINDOW_HEIGHT: f64 = 800.0;

pub struct App {
    pub state: Option<RenderState>,
    pub cursor_position: Point,
}

impl Default for App {
    fn default() -> Self {
        Self {
            state: None,
            cursor_position: Point::ZERO,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PaneKind {
    Empty,
    AutopilotChat,
    GoOnline,
    ProviderStatus,
    EarningsScoreboard,
    RelayConnections,
    SyncHealth,
    NetworkRequests,
    StarterJobs,
    ActivityFeed,
    AlertsRecovery,
    Settings,
    JobInbox,
    ActiveJob,
    JobHistory,
    NostrIdentity,
    SparkWallet,
    SparkCreateInvoice,
    SparkPayInvoice,
    AgentProfileState,
    AgentScheduleTick,
    TrajectoryAudit,
    SkillRegistry,
    SkillTrustRevocation,
    CreditDesk,
    CreditSettlementLedger,
}

#[derive(Clone, Copy)]
pub enum PaneDragMode {
    Moving {
        pane_id: u64,
        start_mouse: Point,
        start_bounds: Bounds,
    },
    Resizing {
        pane_id: u64,
        edge: ResizeEdge,
        start_mouse: Point,
        start_bounds: Bounds,
    },
}

pub struct DesktopPane {
    pub id: u64,
    pub title: String,
    pub kind: PaneKind,
    pub bounds: Bounds,
    pub z_index: i32,
    pub frame: PaneFrame,
}

pub struct SparkPaneInputs {
    pub invoice_amount: TextInput,
    pub send_request: TextInput,
    pub send_amount: TextInput,
}

impl Default for SparkPaneInputs {
    fn default() -> Self {
        Self {
            invoice_amount: TextInput::new().value("1000").placeholder("Invoice sats"),
            send_request: TextInput::new()
                .placeholder("Spark payment request or invoice")
                .mono(true),
            send_amount: TextInput::new().placeholder("Send sats (optional)"),
        }
    }
}

pub struct PayInvoicePaneInputs {
    pub payment_request: TextInput,
    pub amount_sats: TextInput,
}

impl Default for PayInvoicePaneInputs {
    fn default() -> Self {
        Self {
            payment_request: TextInput::new()
                .placeholder("Lightning invoice / payment request")
                .mono(true),
            amount_sats: TextInput::new().placeholder("Send sats (optional)"),
        }
    }
}

pub struct CreateInvoicePaneInputs {
    pub amount_sats: TextInput,
    pub description: TextInput,
    pub expiry_seconds: TextInput,
}

impl Default for CreateInvoicePaneInputs {
    fn default() -> Self {
        Self {
            amount_sats: TextInput::new().value("1000").placeholder("Invoice sats"),
            description: TextInput::new().placeholder("Description (optional)"),
            expiry_seconds: TextInput::new().value("3600").placeholder("Expiry seconds"),
        }
    }
}

pub struct RelayConnectionsPaneInputs {
    pub relay_url: TextInput,
}

impl Default for RelayConnectionsPaneInputs {
    fn default() -> Self {
        Self {
            relay_url: TextInput::new()
                .value("wss://relay.example.com")
                .placeholder("wss://relay.example.com"),
        }
    }
}

pub struct NetworkRequestsPaneInputs {
    pub request_type: TextInput,
    pub payload: TextInput,
    pub skill_scope_id: TextInput,
    pub credit_envelope_ref: TextInput,
    pub budget_sats: TextInput,
    pub timeout_seconds: TextInput,
}

impl Default for NetworkRequestsPaneInputs {
    fn default() -> Self {
        Self {
            request_type: TextInput::new()
                .value("summarize.text")
                .placeholder("Request type"),
            payload: TextInput::new().placeholder("Request payload"),
            skill_scope_id: TextInput::new()
                .value("33400:npub1agent:summarize-text:0.1.0")
                .placeholder("Skill scope id (optional)"),
            credit_envelope_ref: TextInput::new().placeholder("Credit envelope id (optional)"),
            budget_sats: TextInput::new().value("1500").placeholder("Budget sats"),
            timeout_seconds: TextInput::new().value("60").placeholder("Timeout seconds"),
        }
    }
}

pub struct SettingsPaneInputs {
    pub relay_url: TextInput,
    pub wallet_default_send_sats: TextInput,
    pub provider_max_queue_depth: TextInput,
}

impl Default for SettingsPaneInputs {
    fn default() -> Self {
        Self {
            relay_url: TextInput::new()
                .value("wss://relay.damus.io")
                .placeholder("wss://relay.example.com"),
            wallet_default_send_sats: TextInput::new()
                .value("1000")
                .placeholder("Default send sats"),
            provider_max_queue_depth: TextInput::new()
                .value("4")
                .placeholder("Provider max queue depth"),
        }
    }
}

pub struct JobHistoryPaneInputs {
    pub search_job_id: TextInput,
}

impl Default for JobHistoryPaneInputs {
    fn default() -> Self {
        Self {
            search_job_id: TextInput::new().placeholder("Search job id"),
        }
    }
}

pub struct ChatPaneInputs {
    pub composer: TextInput,
}

impl Default for ChatPaneInputs {
    fn default() -> Self {
        Self {
            composer: TextInput::new().placeholder("Ask Autopilot to do work..."),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AutopilotRole {
    User,
    Autopilot,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AutopilotMessageStatus {
    Queued,
    Running,
    Done,
    Error,
}

pub struct AutopilotMessage {
    pub id: u64,
    pub role: AutopilotRole,
    pub status: AutopilotMessageStatus,
    pub content: String,
    pub status_due_at: Option<Instant>,
}

pub struct AutopilotChatState {
    pub threads: Vec<String>,
    pub active_thread: usize,
    pub messages: Vec<AutopilotMessage>,
    pub next_message_id: u64,
    pub last_error: Option<String>,
}

impl Default for AutopilotChatState {
    fn default() -> Self {
        Self {
            threads: vec!["Main".to_string()],
            active_thread: 0,
            messages: vec![AutopilotMessage {
                id: 1,
                role: AutopilotRole::Autopilot,
                status: AutopilotMessageStatus::Done,
                content: "Autopilot ready. Ask for a task to start.".to_string(),
                status_due_at: None,
            }],
            next_message_id: 2,
            last_error: None,
        }
    }
}

impl AutopilotChatState {
    pub fn submit_prompt(&mut self, now: Instant, prompt: String) {
        self.last_error = None;
        let trimmed = prompt.trim();
        if trimmed.is_empty() {
            self.last_error = Some("Prompt cannot be empty".to_string());
            self.messages.push(AutopilotMessage {
                id: self.next_message_id,
                role: AutopilotRole::Autopilot,
                status: AutopilotMessageStatus::Error,
                content: "Cannot run empty prompt".to_string(),
                status_due_at: None,
            });
            self.next_message_id = self.next_message_id.saturating_add(1);
            return;
        }

        self.messages.push(AutopilotMessage {
            id: self.next_message_id,
            role: AutopilotRole::User,
            status: AutopilotMessageStatus::Done,
            content: trimmed.to_string(),
            status_due_at: None,
        });
        self.next_message_id = self.next_message_id.saturating_add(1);

        self.messages.push(AutopilotMessage {
            id: self.next_message_id,
            role: AutopilotRole::Autopilot,
            status: AutopilotMessageStatus::Queued,
            content: format!("Queued local execution for: {trimmed}"),
            status_due_at: Some(now + Duration::from_millis(280)),
        });
        self.next_message_id = self.next_message_id.saturating_add(1);
    }

    pub fn tick(&mut self, now: Instant) -> bool {
        let mut changed = false;
        for message in &mut self.messages {
            match message.status {
                AutopilotMessageStatus::Queued => {
                    if message.status_due_at.is_some_and(|due| now >= due) {
                        message.status = AutopilotMessageStatus::Running;
                        message.status_due_at = Some(now + Duration::from_millis(620));
                        message.content = message.content.replacen(
                            "Queued local execution",
                            "Running local execution",
                            1,
                        );
                        changed = true;
                    }
                }
                AutopilotMessageStatus::Running => {
                    if message.status_due_at.is_some_and(|due| now >= due) {
                        message.status = AutopilotMessageStatus::Done;
                        message.status_due_at = None;
                        message.content = message.content.replacen(
                            "Running local execution",
                            "Completed local execution",
                            1,
                        );
                        changed = true;
                    }
                }
                AutopilotMessageStatus::Done | AutopilotMessageStatus::Error => {}
            }
        }
        changed
    }

    pub fn has_pending_messages(&self) -> bool {
        self.messages.iter().any(|message| {
            matches!(
                message.status,
                AutopilotMessageStatus::Queued | AutopilotMessageStatus::Running
            )
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderMode {
    Offline,
    Connecting,
    Online,
    Degraded,
}

impl ProviderMode {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Offline => "offline",
            Self::Connecting => "connecting",
            Self::Online => "online",
            Self::Degraded => "degraded",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderBlocker {
    IdentityMissing,
    WalletError,
    SkillTrustUnavailable,
    CreditLaneUnavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PaneLoadState {
    Loading,
    Ready,
    Error,
}

impl PaneLoadState {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Loading => "loading",
            Self::Ready => "ready",
            Self::Error => "error",
        }
    }
}

pub trait PaneStatusAccess {
    fn pane_load_state_mut(&mut self) -> &mut PaneLoadState;
    fn pane_last_error_mut(&mut self) -> &mut Option<String>;
    fn pane_last_action_mut(&mut self) -> &mut Option<String>;

    fn pane_set_ready(&mut self, action: impl Into<String>) {
        *self.pane_load_state_mut() = PaneLoadState::Ready;
        *self.pane_last_error_mut() = None;
        *self.pane_last_action_mut() = Some(action.into());
    }

    fn pane_set_error(&mut self, error: impl Into<String>) -> String {
        let error = error.into();
        *self.pane_load_state_mut() = PaneLoadState::Error;
        *self.pane_last_error_mut() = Some(error.clone());
        error
    }

    fn pane_clear_error(&mut self) {
        *self.pane_last_error_mut() = None;
    }
}

macro_rules! impl_pane_status_access {
    ($($state:ty),+ $(,)?) => {
        $(
            impl PaneStatusAccess for $state {
                fn pane_load_state_mut(&mut self) -> &mut PaneLoadState {
                    &mut self.load_state
                }

                fn pane_last_error_mut(&mut self) -> &mut Option<String> {
                    &mut self.last_error
                }

                fn pane_last_action_mut(&mut self) -> &mut Option<String> {
                    &mut self.last_action
                }
            }
        )+
    };
}

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
        request_type: &str,
        payload: &str,
        skill_scope_id: Option<String>,
        credit_envelope_ref: Option<String>,
        budget_sats: u64,
        timeout_seconds: u64,
        authority_command_seq: u64,
    ) -> Result<String, String> {
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
        request.authority_event_id = response.event_id.clone();
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActivityEventDomain {
    Chat,
    Job,
    Wallet,
    Network,
    Sync,
    Sa,
    Skl,
    Ac,
}

impl ActivityEventDomain {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Chat => "chat",
            Self::Job => "job",
            Self::Wallet => "wallet",
            Self::Network => "network",
            Self::Sync => "sync",
            Self::Sa => "sa",
            Self::Skl => "skl",
            Self::Ac => "ac",
        }
    }

    pub const fn source_tag(self) -> &'static str {
        match self {
            Self::Chat => "chat.lane",
            Self::Job => "provider.runtime",
            Self::Wallet => "spark.wallet",
            Self::Network => "nip90.network",
            Self::Sync => "spacetime.sync",
            Self::Sa => "nostr.sa",
            Self::Skl => "nostr.skl",
            Self::Ac => "nostr.ac",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActivityFeedFilter {
    All,
    Chat,
    Job,
    Wallet,
    Network,
    Sync,
    Sa,
    Skl,
    Ac,
}

impl ActivityFeedFilter {
    pub const fn all() -> [Self; 9] {
        [
            Self::All,
            Self::Chat,
            Self::Job,
            Self::Wallet,
            Self::Network,
            Self::Sync,
            Self::Sa,
            Self::Skl,
            Self::Ac,
        ]
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Chat => "chat",
            Self::Job => "job",
            Self::Wallet => "wallet",
            Self::Network => "network",
            Self::Sync => "sync",
            Self::Sa => "sa",
            Self::Skl => "skl",
            Self::Ac => "ac",
        }
    }

    pub fn matches(self, domain: ActivityEventDomain) -> bool {
        match self {
            Self::All => true,
            Self::Chat => domain == ActivityEventDomain::Chat,
            Self::Job => domain == ActivityEventDomain::Job,
            Self::Wallet => domain == ActivityEventDomain::Wallet,
            Self::Network => domain == ActivityEventDomain::Network,
            Self::Sync => domain == ActivityEventDomain::Sync,
            Self::Sa => domain == ActivityEventDomain::Sa,
            Self::Skl => domain == ActivityEventDomain::Skl,
            Self::Ac => domain == ActivityEventDomain::Ac,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActivityEventRow {
    pub event_id: String,
    pub domain: ActivityEventDomain,
    pub source_tag: String,
    pub occurred_at_epoch_seconds: u64,
    pub summary: String,
    pub detail: String,
}

pub struct ActivityFeedState {
    pub load_state: PaneLoadState,
    pub last_error: Option<String>,
    pub last_action: Option<String>,
    pub active_filter: ActivityFeedFilter,
    pub rows: Vec<ActivityEventRow>,
    pub selected_event_id: Option<String>,
}

impl Default for ActivityFeedState {
    fn default() -> Self {
        Self {
            load_state: PaneLoadState::Loading,
            last_error: None,
            last_action: Some("Waiting for activity feed lane snapshot".to_string()),
            active_filter: ActivityFeedFilter::All,
            rows: Vec::new(),
            selected_event_id: None,
        }
    }
}

impl ActivityFeedState {
    pub fn visible_rows(&self) -> Vec<&ActivityEventRow> {
        self.rows
            .iter()
            .filter(|row| self.active_filter.matches(row.domain))
            .collect()
    }

    pub fn selected(&self) -> Option<&ActivityEventRow> {
        let selected = self.selected_event_id.as_deref()?;
        self.rows.iter().find(|row| row.event_id == selected)
    }

    pub fn select_visible_row(&mut self, index: usize) -> bool {
        let Some(event_id) = self
            .visible_rows()
            .get(index)
            .map(|row| row.event_id.clone())
        else {
            return false;
        };
        self.selected_event_id = Some(event_id);
        self.pane_clear_error();
        true
    }

    pub fn set_filter(&mut self, filter: ActivityFeedFilter) {
        self.active_filter = filter;
        if self
            .selected()
            .is_none_or(|row| !filter.matches(row.domain))
        {
            self.selected_event_id = self.visible_rows().first().map(|row| row.event_id.clone());
        }
        self.pane_set_ready(format!("Activity filter -> {}", filter.label()));
    }

    pub fn upsert_event(&mut self, row: ActivityEventRow) {
        if let Some(existing) = self
            .rows
            .iter_mut()
            .find(|existing| existing.event_id == row.event_id)
        {
            *existing = row;
        } else {
            self.rows.push(row);
        }

        self.rows.sort_by(|lhs, rhs| {
            rhs.occurred_at_epoch_seconds
                .cmp(&lhs.occurred_at_epoch_seconds)
                .then_with(|| lhs.event_id.cmp(&rhs.event_id))
        });
        self.rows.truncate(96);
    }

    pub fn record_refresh(&mut self, rows: Vec<ActivityEventRow>) {
        for row in rows {
            self.upsert_event(row);
        }

        if self.selected().is_none() {
            self.selected_event_id = self.visible_rows().first().map(|row| row.event_id.clone());
        }

        self.pane_set_ready(format!(
            "Activity feed refreshed ({} events)",
            self.rows.len()
        ));
    }
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AlertSeverity {
    Info,
    Warning,
    Critical,
}

impl AlertSeverity {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Warning => "warning",
            Self::Critical => "critical",
        }
    }
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AlertLifecycle {
    Active,
    Acknowledged,
    Resolved,
}

impl AlertLifecycle {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Acknowledged => "acknowledged",
            Self::Resolved => "resolved",
        }
    }
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AlertDomain {
    Identity,
    Wallet,
    Relays,
    ProviderRuntime,
    Sync,
    SkillTrust,
    Credit,
}

impl AlertDomain {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Identity => "identity",
            Self::Wallet => "wallet",
            Self::Relays => "relays",
            Self::ProviderRuntime => "provider",
            Self::Sync => "sync",
            Self::SkillTrust => "skill-trust",
            Self::Credit => "credit",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecoveryAlertRow {
    pub alert_id: String,
    pub domain: AlertDomain,
    pub severity: AlertSeverity,
    pub lifecycle: AlertLifecycle,
    pub summary: String,
    pub remediation: String,
    pub last_transition_epoch_seconds: u64,
}

pub struct AlertsRecoveryState {
    pub load_state: PaneLoadState,
    pub last_error: Option<String>,
    pub last_action: Option<String>,
    pub alerts: Vec<RecoveryAlertRow>,
    pub selected_alert_id: Option<String>,
    next_transition_seq: u64,
}

impl Default for AlertsRecoveryState {
    fn default() -> Self {
        Self {
            load_state: PaneLoadState::Loading,
            last_error: None,
            last_action: Some("Waiting for alert lane snapshot".to_string()),
            alerts: Vec::new(),
            selected_alert_id: None,
            next_transition_seq: 1,
        }
    }
}

impl AlertsRecoveryState {
    pub fn select_by_index(&mut self, index: usize) -> bool {
        let Some(alert_id) = self.alerts.get(index).map(|alert| alert.alert_id.clone()) else {
            return false;
        };
        self.selected_alert_id = Some(alert_id);
        self.pane_clear_error();
        true
    }

    pub fn selected(&self) -> Option<&RecoveryAlertRow> {
        let selected = self.selected_alert_id.as_deref()?;
        self.alerts.iter().find(|alert| alert.alert_id == selected)
    }

    pub fn selected_domain(&self) -> Option<AlertDomain> {
        self.selected().map(|alert| alert.domain)
    }

    pub fn acknowledge_selected(&mut self) -> Result<String, String> {
        let selected = self
            .selected_alert_id
            .as_deref()
            .ok_or_else(|| "Select an alert first".to_string())?
            .to_string();
        let transition_epoch = self.next_transition_epoch();
        let Some(alert) = self
            .alerts
            .iter_mut()
            .find(|alert| alert.alert_id == selected)
        else {
            return Err(self.pane_set_error("Selected alert no longer exists"));
        };

        if alert.lifecycle == AlertLifecycle::Resolved {
            return Err(self.pane_set_error("Resolved alert cannot be acknowledged"));
        }

        alert.lifecycle = AlertLifecycle::Acknowledged;
        alert.last_transition_epoch_seconds = transition_epoch;
        let alert_id = alert.alert_id.clone();
        self.pane_set_ready(format!("Acknowledged {alert_id}"));
        Ok(alert_id)
    }

    pub fn resolve_selected(&mut self) -> Result<String, String> {
        let selected = self
            .selected_alert_id
            .as_deref()
            .ok_or_else(|| "Select an alert first".to_string())?
            .to_string();
        let transition_epoch = self.next_transition_epoch();
        let Some(alert) = self
            .alerts
            .iter_mut()
            .find(|alert| alert.alert_id == selected)
        else {
            return Err(self.pane_set_error("Selected alert no longer exists"));
        };

        alert.lifecycle = AlertLifecycle::Resolved;
        alert.last_transition_epoch_seconds = transition_epoch;
        let alert_id = alert.alert_id.clone();
        self.pane_set_ready(format!("Resolved {alert_id}"));
        Ok(alert_id)
    }

    fn next_transition_epoch(&mut self) -> u64 {
        let epoch = 1_761_920_000u64.saturating_add(self.next_transition_seq * 17);
        self.next_transition_seq = self.next_transition_seq.saturating_add(1);
        epoch
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SettingsDocumentV1 {
    pub schema_version: u16,
    pub relay_url: String,
    pub identity_path: String,
    pub wallet_default_send_sats: u64,
    pub provider_max_queue_depth: u32,
    pub reconnect_required: bool,
}

impl Default for SettingsDocumentV1 {
    fn default() -> Self {
        Self {
            schema_version: 1,
            relay_url: "wss://relay.damus.io".to_string(),
            identity_path: settings_identity_path(),
            wallet_default_send_sats: 1000,
            provider_max_queue_depth: 4,
            reconnect_required: false,
        }
    }
}

pub struct SettingsState {
    pub load_state: PaneLoadState,
    pub last_error: Option<String>,
    pub last_action: Option<String>,
    pub document: SettingsDocumentV1,
}

impl Default for SettingsState {
    fn default() -> Self {
        Self {
            load_state: PaneLoadState::Ready,
            last_error: None,
            last_action: Some("Settings loaded from migration-safe defaults".to_string()),
            document: SettingsDocumentV1::default(),
        }
    }
}

impl SettingsState {
    pub fn load_from_disk() -> Self {
        let path = settings_file_path();
        let mut state = Self::default();
        match std::fs::read_to_string(&path) {
            Ok(raw) => match parse_settings_document(&raw) {
                Ok(document) => {
                    state.document = document;
                    state.pane_set_ready(format!("Settings loaded from {}", path.display()));
                }
                Err(error) => {
                    let _ = state.pane_set_error(format!("Settings parse error: {error}"));
                    *state.pane_last_action_mut() =
                        Some("Using migration-safe defaults".to_string());
                }
            },
            Err(error) => {
                if error.kind() != std::io::ErrorKind::NotFound {
                    let _ = state.pane_set_error(format!("Settings read error: {error}"));
                }
            }
        }

        state
    }

    pub fn apply_updates(
        &mut self,
        relay_url: &str,
        wallet_default_send_sats: &str,
        provider_max_queue_depth: &str,
    ) -> Result<(), String> {
        self.apply_updates_internal(
            relay_url,
            wallet_default_send_sats,
            provider_max_queue_depth,
            true,
        )
    }

    fn apply_updates_internal(
        &mut self,
        relay_url: &str,
        wallet_default_send_sats: &str,
        provider_max_queue_depth: &str,
        persist: bool,
    ) -> Result<(), String> {
        let relay_url = relay_url.trim();
        if relay_url.is_empty() {
            return Err(self.pane_set_error("Relay URL is required"));
        }
        if !relay_url.starts_with("wss://") {
            return Err(self.pane_set_error("Relay URL must start with wss://"));
        }

        let wallet_default_send_sats = wallet_default_send_sats
            .trim()
            .parse::<u64>()
            .map_err(|error| format!("Wallet default send sats must be an integer: {error}"))?;
        if wallet_default_send_sats == 0 || wallet_default_send_sats > 10_000_000 {
            return Err(
                self.pane_set_error("Wallet default send sats must be between 1 and 10,000,000")
            );
        }

        let provider_max_queue_depth = provider_max_queue_depth
            .trim()
            .parse::<u32>()
            .map_err(|error| format!("Provider max queue depth must be an integer: {error}"))?;
        if provider_max_queue_depth == 0 || provider_max_queue_depth > 512 {
            return Err(self.pane_set_error("Provider max queue depth must be between 1 and 512"));
        }

        let reconnect_required = relay_url != self.document.relay_url
            || provider_max_queue_depth != self.document.provider_max_queue_depth;
        self.document.relay_url = relay_url.to_string();
        self.document.wallet_default_send_sats = wallet_default_send_sats;
        self.document.provider_max_queue_depth = provider_max_queue_depth;
        self.document.reconnect_required = reconnect_required;

        if persist {
            self.persist_to_disk()?;
        }

        self.pane_set_ready(if reconnect_required {
            "Saved settings. Relay/provider changes require reconnect.".to_string()
        } else {
            "Saved settings.".to_string()
        });
        Ok(())
    }

    pub fn reset_defaults(&mut self) -> Result<(), String> {
        self.reset_defaults_internal(true)
    }

    fn reset_defaults_internal(&mut self, persist: bool) -> Result<(), String> {
        self.document = SettingsDocumentV1::default();
        if persist {
            self.persist_to_disk()?;
        }
        self.pane_set_ready("Reset settings to schema defaults.");
        Ok(())
    }

    fn persist_to_disk(&mut self) -> Result<(), String> {
        let path = settings_file_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| format!("Failed to create settings dir: {error}"))?;
        }
        std::fs::write(&path, serialize_settings_document(&self.document))
            .map_err(|error| format!("Failed to persist settings: {error}"))?;
        Ok(())
    }
}

impl SettingsPaneInputs {
    pub fn from_state(settings: &SettingsState) -> Self {
        Self {
            relay_url: TextInput::new()
                .value(settings.document.relay_url.clone())
                .placeholder("wss://relay.example.com"),
            wallet_default_send_sats: TextInput::new()
                .value(settings.document.wallet_default_send_sats.to_string())
                .placeholder("Default send sats"),
            provider_max_queue_depth: TextInput::new()
                .value(settings.document.provider_max_queue_depth.to_string())
                .placeholder("Provider max queue depth"),
        }
    }

    pub fn sync_from_state(&mut self, settings: &SettingsState) {
        self.relay_url
            .set_value(settings.document.relay_url.clone());
        self.wallet_default_send_sats
            .set_value(settings.document.wallet_default_send_sats.to_string());
        self.provider_max_queue_depth
            .set_value(settings.document.provider_max_queue_depth.to_string());
    }
}

fn settings_file_path() -> PathBuf {
    std::env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(".openagents")
        .join("autopilot-settings-v1.conf")
}

fn settings_identity_path() -> String {
    nostr::identity_mnemonic_path()
        .map(|path| path.display().to_string())
        .unwrap_or_else(|_| "~/.openagents/pylon/identity.mnemonic".to_string())
}

fn serialize_settings_document(document: &SettingsDocumentV1) -> String {
    format!(
        "schema_version={}\nrelay_url={}\nidentity_path={}\nwallet_default_send_sats={}\nprovider_max_queue_depth={}\nreconnect_required={}\n",
        document.schema_version,
        document.relay_url,
        document.identity_path,
        document.wallet_default_send_sats,
        document.provider_max_queue_depth,
        document.reconnect_required,
    )
}

fn parse_settings_document(raw: &str) -> Result<SettingsDocumentV1, String> {
    let mut document = SettingsDocumentV1::default();
    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let Some((key, value)) = trimmed.split_once('=') else {
            return Err(format!("Invalid settings line: {trimmed}"));
        };
        match key.trim() {
            "schema_version" => {
                document.schema_version = value
                    .trim()
                    .parse::<u16>()
                    .map_err(|error| format!("Invalid schema version: {error}"))?;
            }
            "relay_url" => document.relay_url = value.trim().to_string(),
            "identity_path" => document.identity_path = value.trim().to_string(),
            "wallet_default_send_sats" => {
                document.wallet_default_send_sats = value
                    .trim()
                    .parse::<u64>()
                    .map_err(|error| format!("Invalid wallet_default_send_sats: {error}"))?;
            }
            "provider_max_queue_depth" => {
                document.provider_max_queue_depth = value
                    .trim()
                    .parse::<u32>()
                    .map_err(|error| format!("Invalid provider_max_queue_depth: {error}"))?;
            }
            "reconnect_required" => {
                document.reconnect_required = value
                    .trim()
                    .parse::<bool>()
                    .map_err(|error| format!("Invalid reconnect_required: {error}"))?;
            }
            _ => {}
        }
    }

    if document.schema_version != 1 {
        return Err(format!(
            "Unsupported schema version {}, expected 1",
            document.schema_version
        ));
    }

    // Identity path authority is the resolved mnemonic path.
    document.identity_path = settings_identity_path();

    Ok(document)
}

#[allow(dead_code)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum JobInboxValidation {
    Valid,
    Pending,
    Invalid(String),
}

impl JobInboxValidation {
    pub fn label(&self) -> String {
        match self {
            Self::Valid => "valid".to_string(),
            Self::Pending => "pending".to_string(),
            Self::Invalid(reason) => format!("invalid ({reason})"),
        }
    }
}

#[allow(dead_code)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum JobInboxDecision {
    Pending,
    Accepted { reason: String },
    Rejected { reason: String },
}

impl JobInboxDecision {
    pub fn label(&self) -> String {
        match self {
            Self::Pending => "pending".to_string(),
            Self::Accepted { reason } => format!("accepted ({reason})"),
            Self::Rejected { reason } => format!("rejected ({reason})"),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JobInboxRequest {
    pub request_id: String,
    pub requester: String,
    pub capability: String,
    pub skill_scope_id: Option<String>,
    pub skl_manifest_a: Option<String>,
    pub skl_manifest_event_id: Option<String>,
    pub sa_tick_request_event_id: Option<String>,
    pub sa_tick_result_event_id: Option<String>,
    pub ac_envelope_event_id: Option<String>,
    pub price_sats: u64,
    pub ttl_seconds: u64,
    pub validation: JobInboxValidation,
    pub arrival_seq: u64,
    pub decision: JobInboxDecision,
}

#[allow(dead_code)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JobInboxNetworkRequest {
    pub request_id: String,
    pub requester: String,
    pub capability: String,
    pub skill_scope_id: Option<String>,
    pub skl_manifest_a: Option<String>,
    pub skl_manifest_event_id: Option<String>,
    pub sa_tick_request_event_id: Option<String>,
    pub sa_tick_result_event_id: Option<String>,
    pub ac_envelope_event_id: Option<String>,
    pub price_sats: u64,
    pub ttl_seconds: u64,
    pub validation: JobInboxValidation,
}

pub struct JobInboxState {
    pub load_state: PaneLoadState,
    pub last_error: Option<String>,
    pub last_action: Option<String>,
    pub requests: Vec<JobInboxRequest>,
    pub selected_request_id: Option<String>,
    #[allow(dead_code)]
    next_arrival_seq: u64,
}

impl Default for JobInboxState {
    fn default() -> Self {
        Self {
            load_state: PaneLoadState::Loading,
            last_error: None,
            last_action: Some("Waiting for inbox lane snapshot".to_string()),
            requests: Vec::new(),
            selected_request_id: None,
            next_arrival_seq: 1,
        }
    }
}

impl JobInboxState {
    #[allow(dead_code)]
    pub fn upsert_network_request(&mut self, request: JobInboxNetworkRequest) {
        if let Some(existing) = self
            .requests
            .iter_mut()
            .find(|existing| existing.request_id == request.request_id)
        {
            existing.requester = request.requester;
            existing.capability = request.capability;
            existing.skill_scope_id = request.skill_scope_id;
            existing.skl_manifest_a = request.skl_manifest_a;
            existing.skl_manifest_event_id = request.skl_manifest_event_id;
            existing.sa_tick_request_event_id = request.sa_tick_request_event_id;
            existing.sa_tick_result_event_id = request.sa_tick_result_event_id;
            existing.ac_envelope_event_id = request.ac_envelope_event_id;
            existing.price_sats = request.price_sats;
            existing.ttl_seconds = request.ttl_seconds;
            existing.validation = request.validation;
            return;
        }

        let arrival_seq = self.next_arrival_seq;
        self.next_arrival_seq = self.next_arrival_seq.saturating_add(1);
        self.requests.push(JobInboxRequest {
            request_id: request.request_id,
            requester: request.requester,
            capability: request.capability,
            skill_scope_id: request.skill_scope_id,
            skl_manifest_a: request.skl_manifest_a,
            skl_manifest_event_id: request.skl_manifest_event_id,
            sa_tick_request_event_id: request.sa_tick_request_event_id,
            sa_tick_result_event_id: request.sa_tick_result_event_id,
            ac_envelope_event_id: request.ac_envelope_event_id,
            price_sats: request.price_sats,
            ttl_seconds: request.ttl_seconds,
            validation: request.validation,
            arrival_seq,
            decision: JobInboxDecision::Pending,
        });
        self.requests.sort_by_key(|request| request.arrival_seq);
    }

    pub fn select_by_index(&mut self, index: usize) -> bool {
        let Some(request_id) = self
            .requests
            .get(index)
            .map(|request| request.request_id.clone())
        else {
            return false;
        };
        self.selected_request_id = Some(request_id);
        self.last_error = None;
        true
    }

    pub fn selected_request(&self) -> Option<&JobInboxRequest> {
        let selected_id = self.selected_request_id.as_deref()?;
        self.requests
            .iter()
            .find(|request| request.request_id == selected_id)
    }

    pub fn decide_selected(&mut self, accepted: bool, reason: &str) -> Result<String, String> {
        let selected_id = self
            .selected_request_id
            .as_deref()
            .ok_or_else(|| "Select a request first".to_string())?
            .to_string();
        let Some(request) = self
            .requests
            .iter_mut()
            .find(|request| request.request_id == selected_id)
        else {
            return Err("Selected request no longer exists".to_string());
        };

        let decision_reason = reason.trim().to_string();
        request.decision = if accepted {
            JobInboxDecision::Accepted {
                reason: decision_reason.clone(),
            }
        } else {
            JobInboxDecision::Rejected {
                reason: decision_reason.clone(),
            }
        };
        self.last_error = None;
        self.last_action = Some(if accepted {
            format!("Accepted {} ({decision_reason})", request.request_id)
        } else {
            format!("Rejected {} ({decision_reason})", request.request_id)
        });
        Ok(request.request_id.clone())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JobLifecycleStage {
    Received,
    Accepted,
    Running,
    Delivered,
    Paid,
    Failed,
}

impl JobLifecycleStage {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Received => "received",
            Self::Accepted => "accepted",
            Self::Running => "running",
            Self::Delivered => "delivered",
            Self::Paid => "paid",
            Self::Failed => "failed",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActiveJobEvent {
    pub seq: u64,
    pub message: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActiveJobRecord {
    pub job_id: String,
    pub request_id: String,
    pub requester: String,
    pub capability: String,
    pub skill_scope_id: Option<String>,
    pub skl_manifest_a: Option<String>,
    pub skl_manifest_event_id: Option<String>,
    pub sa_tick_request_event_id: Option<String>,
    pub sa_tick_result_event_id: Option<String>,
    pub sa_trajectory_session_id: Option<String>,
    pub ac_envelope_event_id: Option<String>,
    pub ac_settlement_event_id: Option<String>,
    pub ac_default_event_id: Option<String>,
    pub quoted_price_sats: u64,
    pub stage: JobLifecycleStage,
    pub invoice_id: Option<String>,
    pub payment_id: Option<String>,
    pub failure_reason: Option<String>,
    pub events: Vec<ActiveJobEvent>,
}

pub struct ActiveJobState {
    pub load_state: PaneLoadState,
    pub last_error: Option<String>,
    pub last_action: Option<String>,
    pub runtime_supports_abort: bool,
    pub job: Option<ActiveJobRecord>,
    next_event_seq: u64,
}

impl Default for ActiveJobState {
    fn default() -> Self {
        Self {
            load_state: PaneLoadState::Loading,
            last_error: None,
            last_action: Some("Waiting for active job lane snapshot".to_string()),
            runtime_supports_abort: false,
            job: None,
            next_event_seq: 1,
        }
    }
}

impl ActiveJobState {
    pub fn start_from_request(&mut self, request: &JobInboxRequest) {
        let job_id = format!("job-{}", request.request_id);
        self.job = Some(ActiveJobRecord {
            job_id,
            request_id: request.request_id.clone(),
            requester: request.requester.clone(),
            capability: request.capability.clone(),
            skill_scope_id: request.skill_scope_id.clone(),
            skl_manifest_a: request.skl_manifest_a.clone(),
            skl_manifest_event_id: request.skl_manifest_event_id.clone(),
            sa_tick_request_event_id: request.sa_tick_request_event_id.clone(),
            sa_tick_result_event_id: request.sa_tick_result_event_id.clone(),
            sa_trajectory_session_id: Some(format!("traj:{}", request.request_id)),
            ac_envelope_event_id: request.ac_envelope_event_id.clone(),
            ac_settlement_event_id: None,
            ac_default_event_id: None,
            quoted_price_sats: request.price_sats,
            stage: JobLifecycleStage::Accepted,
            invoice_id: None,
            payment_id: None,
            failure_reason: None,
            events: Vec::new(),
        });
        self.next_event_seq = 1;
        self.append_event("received request from inbox");
        self.append_event("accepted request and queued runtime execution");
        self.load_state = PaneLoadState::Ready;
        self.last_error = None;
        self.last_action = Some(format!("Selected {} as active job", request.request_id));
    }

    pub fn append_event(&mut self, message: impl Into<String>) {
        let Some(job) = self.job.as_mut() else {
            return;
        };
        job.events.push(ActiveJobEvent {
            seq: self.next_event_seq,
            message: message.into(),
        });
        self.next_event_seq = self.next_event_seq.saturating_add(1);
    }

    pub fn advance_stage(&mut self) -> Result<JobLifecycleStage, String> {
        let Some(job) = self.job.as_mut() else {
            self.last_error = Some("No active job selected".to_string());
            self.load_state = PaneLoadState::Error;
            return Err("No active job selected".to_string());
        };

        let next_stage = match job.stage {
            JobLifecycleStage::Received => JobLifecycleStage::Accepted,
            JobLifecycleStage::Accepted => JobLifecycleStage::Running,
            JobLifecycleStage::Running => JobLifecycleStage::Delivered,
            JobLifecycleStage::Delivered => JobLifecycleStage::Paid,
            JobLifecycleStage::Paid | JobLifecycleStage::Failed => {
                self.last_error = Some("Active job already terminal".to_string());
                self.load_state = PaneLoadState::Error;
                return Err("Active job already terminal".to_string());
            }
        };

        job.stage = next_stage;
        if next_stage == JobLifecycleStage::Delivered {
            job.invoice_id = Some(format!("inv-{}", job.request_id));
            if job.sa_tick_result_event_id.is_none() {
                job.sa_tick_result_event_id = Some(format!("sa:39211:{}", job.request_id));
            }
        }
        if next_stage == JobLifecycleStage::Paid {
            job.payment_id = Some(format!("pay-{}", job.request_id));
            if job.ac_settlement_event_id.is_none() {
                job.ac_settlement_event_id = Some(format!("ac:39244:{}", job.request_id));
            }
        }
        self.append_event(format!("stage advanced to {}", next_stage.label()));
        self.last_error = None;
        self.load_state = PaneLoadState::Ready;
        self.last_action = Some(format!("Advanced active job to {}", next_stage.label()));
        Ok(next_stage)
    }

    pub fn abort_job(&mut self, reason: &str) -> Result<(), String> {
        if !self.runtime_supports_abort {
            self.last_error =
                Some("Abort unavailable: runtime lane does not support cancel".to_string());
            self.load_state = PaneLoadState::Error;
            return Err("Abort unavailable".to_string());
        }
        let Some(job) = self.job.as_mut() else {
            self.last_error = Some("No active job selected".to_string());
            self.load_state = PaneLoadState::Error;
            return Err("No active job selected".to_string());
        };

        let reason_text = reason.trim().to_string();
        job.stage = JobLifecycleStage::Failed;
        job.failure_reason = Some(reason_text.clone());
        if job.ac_default_event_id.is_none() {
            job.ac_default_event_id = Some(format!("ac:39245:{}", job.request_id));
        }
        self.append_event(format!("job aborted: {reason_text}"));
        self.last_error = None;
        self.load_state = PaneLoadState::Ready;
        self.last_action = Some("Aborted active job".to_string());
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JobHistoryStatus {
    Succeeded,
    Failed,
}

impl JobHistoryStatus {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JobHistoryStatusFilter {
    All,
    Succeeded,
    Failed,
}

impl JobHistoryStatusFilter {
    pub const fn label(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
        }
    }

    pub const fn cycle(self) -> Self {
        match self {
            Self::All => Self::Succeeded,
            Self::Succeeded => Self::Failed,
            Self::Failed => Self::All,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JobHistoryTimeRange {
    All,
    Last24h,
    Last7d,
}

impl JobHistoryTimeRange {
    pub const fn label(self) -> &'static str {
        match self {
            Self::All => "all-time",
            Self::Last24h => "24h",
            Self::Last7d => "7d",
        }
    }

    pub const fn max_age_seconds(self) -> Option<u64> {
        match self {
            Self::All => None,
            Self::Last24h => Some(86_400),
            Self::Last7d => Some(604_800),
        }
    }

    pub const fn cycle(self) -> Self {
        match self {
            Self::All => Self::Last24h,
            Self::Last24h => Self::Last7d,
            Self::Last7d => Self::All,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JobHistoryReceiptRow {
    pub job_id: String,
    pub status: JobHistoryStatus,
    pub completed_at_epoch_seconds: u64,
    pub skill_scope_id: Option<String>,
    pub skl_manifest_a: Option<String>,
    pub skl_manifest_event_id: Option<String>,
    pub sa_tick_result_event_id: Option<String>,
    pub sa_trajectory_session_id: Option<String>,
    pub ac_envelope_event_id: Option<String>,
    pub ac_settlement_event_id: Option<String>,
    pub ac_default_event_id: Option<String>,
    pub payout_sats: u64,
    pub result_hash: String,
    pub payment_pointer: String,
    pub failure_reason: Option<String>,
}

pub struct JobHistoryState {
    pub load_state: PaneLoadState,
    pub last_error: Option<String>,
    pub last_action: Option<String>,
    pub rows: Vec<JobHistoryReceiptRow>,
    pub status_filter: JobHistoryStatusFilter,
    pub time_range: JobHistoryTimeRange,
    pub page: usize,
    pub page_size: usize,
    pub search_job_id: String,
    pub reference_epoch_seconds: u64,
}

impl Default for JobHistoryState {
    fn default() -> Self {
        Self {
            load_state: PaneLoadState::Loading,
            last_error: None,
            last_action: Some("Waiting for receipt lane snapshot".to_string()),
            rows: Vec::new(),
            status_filter: JobHistoryStatusFilter::All,
            time_range: JobHistoryTimeRange::All,
            page: 0,
            page_size: 6,
            search_job_id: String::new(),
            reference_epoch_seconds: 1_761_920_000,
        }
    }
}

impl JobHistoryState {
    pub fn set_search_job_id(&mut self, value: String) {
        self.search_job_id = value;
        self.page = 0;
    }

    pub fn cycle_status_filter(&mut self) {
        self.status_filter = self.status_filter.cycle();
        self.page = 0;
        self.last_error = None;
        self.last_action = Some(format!("Status filter -> {}", self.status_filter.label()));
    }

    pub fn cycle_time_range(&mut self) {
        self.time_range = self.time_range.cycle();
        self.page = 0;
        self.last_error = None;
        self.last_action = Some(format!("Time range -> {}", self.time_range.label()));
    }

    pub fn previous_page(&mut self) {
        if self.page > 0 {
            self.page -= 1;
        }
    }

    pub fn next_page(&mut self) {
        let pages = self.total_pages();
        if self.page + 1 < pages {
            self.page += 1;
        }
    }

    pub fn total_pages(&self) -> usize {
        let filtered = self.filtered_rows();
        ((filtered.len() + self.page_size.saturating_sub(1)) / self.page_size.max(1)).max(1)
    }

    pub fn paged_rows(&self) -> Vec<&JobHistoryReceiptRow> {
        let filtered = self.filtered_rows();
        let page = self.page.min(self.total_pages().saturating_sub(1));
        let start = page.saturating_mul(self.page_size.max(1));
        let end = (start + self.page_size.max(1)).min(filtered.len());
        filtered[start..end].to_vec()
    }

    pub fn upsert_row(&mut self, row: JobHistoryReceiptRow) {
        if let Some(existing) = self
            .rows
            .iter_mut()
            .find(|existing| existing.job_id == row.job_id)
        {
            *existing = row;
        } else {
            self.rows.push(row);
        }
        self.rows.sort_by(|lhs, rhs| {
            rhs.completed_at_epoch_seconds
                .cmp(&lhs.completed_at_epoch_seconds)
                .then_with(|| lhs.job_id.cmp(&rhs.job_id))
        });
    }

    pub fn record_from_active_job(&mut self, job: &ActiveJobRecord, status: JobHistoryStatus) {
        let completed = self
            .reference_epoch_seconds
            .saturating_add(self.rows.len() as u64 * 17);
        self.upsert_row(JobHistoryReceiptRow {
            job_id: job.job_id.clone(),
            status,
            completed_at_epoch_seconds: completed,
            skill_scope_id: job.skill_scope_id.clone(),
            skl_manifest_a: job.skl_manifest_a.clone(),
            skl_manifest_event_id: job.skl_manifest_event_id.clone(),
            sa_tick_result_event_id: job.sa_tick_result_event_id.clone(),
            sa_trajectory_session_id: job.sa_trajectory_session_id.clone(),
            ac_envelope_event_id: job.ac_envelope_event_id.clone(),
            ac_settlement_event_id: job.ac_settlement_event_id.clone(),
            ac_default_event_id: job.ac_default_event_id.clone(),
            payout_sats: if status == JobHistoryStatus::Succeeded {
                job.quoted_price_sats
            } else {
                0
            },
            result_hash: format!("sha256:{}-{}", job.request_id, job.stage.label()),
            payment_pointer: job
                .payment_id
                .clone()
                .or_else(|| job.invoice_id.clone())
                .unwrap_or_else(|| format!("pending:{}", job.request_id)),
            failure_reason: job.failure_reason.clone(),
        });
        self.page = 0;
        self.last_error = None;
        self.load_state = PaneLoadState::Ready;
        self.last_action = Some(format!("Recorded history receipt for {}", job.job_id));
    }

    fn filtered_rows(&self) -> Vec<&JobHistoryReceiptRow> {
        let search = self.search_job_id.trim().to_lowercase();
        self.rows
            .iter()
            .filter(|row| match self.status_filter {
                JobHistoryStatusFilter::All => true,
                JobHistoryStatusFilter::Succeeded => row.status == JobHistoryStatus::Succeeded,
                JobHistoryStatusFilter::Failed => row.status == JobHistoryStatus::Failed,
            })
            .filter(|row| {
                if let Some(max_age) = self.time_range.max_age_seconds() {
                    let age = self
                        .reference_epoch_seconds
                        .saturating_sub(row.completed_at_epoch_seconds);
                    age <= max_age
                } else {
                    true
                }
            })
            .filter(|row| {
                if search.is_empty() {
                    true
                } else {
                    row.job_id.to_lowercase().contains(&search)
                }
            })
            .collect()
    }
}

pub struct AgentProfileStatePaneState {
    pub load_state: PaneLoadState,
    pub last_error: Option<String>,
    pub last_action: Option<String>,
    pub profile_name: String,
    pub profile_about: String,
    pub goals_summary: String,
    pub profile_event_id: Option<String>,
    pub state_event_id: Option<String>,
    pub goals_event_id: Option<String>,
}

impl Default for AgentProfileStatePaneState {
    fn default() -> Self {
        Self {
            load_state: PaneLoadState::Loading,
            last_error: None,
            last_action: Some("Waiting for SA profile/state snapshot".to_string()),
            profile_name: "Autopilot".to_string(),
            profile_about: "Desktop sovereign agent runtime".to_string(),
            goals_summary: "Earn sats and complete queued jobs".to_string(),
            profile_event_id: None,
            state_event_id: None,
            goals_event_id: None,
        }
    }
}

pub struct AgentScheduleTickPaneState {
    pub load_state: PaneLoadState,
    pub last_error: Option<String>,
    pub last_action: Option<String>,
    pub heartbeat_seconds: u64,
    pub next_tick_reason: String,
    pub last_tick_outcome: String,
    pub schedule_event_id: Option<String>,
    pub tick_request_event_id: Option<String>,
    pub tick_result_event_id: Option<String>,
}

impl Default for AgentScheduleTickPaneState {
    fn default() -> Self {
        Self {
            load_state: PaneLoadState::Loading,
            last_error: None,
            last_action: Some("Waiting for SA schedule/tick snapshot".to_string()),
            heartbeat_seconds: 30,
            next_tick_reason: "manual.operator".to_string(),
            last_tick_outcome: "n/a".to_string(),
            schedule_event_id: None,
            tick_request_event_id: None,
            tick_result_event_id: None,
        }
    }
}

pub struct TrajectoryAuditPaneState {
    pub load_state: PaneLoadState,
    pub last_error: Option<String>,
    pub last_action: Option<String>,
    pub active_session_id: Option<String>,
    pub verified_hash: Option<String>,
    pub step_filter: String,
}

impl Default for TrajectoryAuditPaneState {
    fn default() -> Self {
        Self {
            load_state: PaneLoadState::Loading,
            last_error: None,
            last_action: Some("Waiting for trajectory session stream".to_string()),
            active_session_id: None,
            verified_hash: None,
            step_filter: "all".to_string(),
        }
    }
}

pub struct SkillRegistryPaneState {
    pub load_state: PaneLoadState,
    pub last_error: Option<String>,
    pub last_action: Option<String>,
    pub search_query: String,
    pub manifest_slug: String,
    pub manifest_version: String,
    pub manifest_a: Option<String>,
    pub manifest_event_id: Option<String>,
    pub version_event_id: Option<String>,
    pub search_result_event_id: Option<String>,
}

impl Default for SkillRegistryPaneState {
    fn default() -> Self {
        Self {
            load_state: PaneLoadState::Loading,
            last_error: None,
            last_action: Some("Waiting for SKL registry snapshot".to_string()),
            search_query: "summarize".to_string(),
            manifest_slug: "summarize-text".to_string(),
            manifest_version: "0.1.0".to_string(),
            manifest_a: None,
            manifest_event_id: None,
            version_event_id: None,
            search_result_event_id: None,
        }
    }
}

pub struct SkillTrustRevocationPaneState {
    pub load_state: PaneLoadState,
    pub last_error: Option<String>,
    pub last_action: Option<String>,
    pub trust_tier: String,
    pub manifest_a: Option<String>,
    pub attestation_count: u32,
    pub kill_switch_active: bool,
    pub revocation_event_id: Option<String>,
}

impl Default for SkillTrustRevocationPaneState {
    fn default() -> Self {
        Self {
            load_state: PaneLoadState::Loading,
            last_error: None,
            last_action: Some("Waiting for SKL trust gate snapshot".to_string()),
            trust_tier: "unknown".to_string(),
            manifest_a: None,
            attestation_count: 0,
            kill_switch_active: false,
            revocation_event_id: None,
        }
    }
}

pub struct CreditDeskPaneState {
    pub load_state: PaneLoadState,
    pub last_error: Option<String>,
    pub last_action: Option<String>,
    pub scope: String,
    pub requested_sats: u64,
    pub offered_sats: u64,
    pub envelope_cap_sats: u64,
    pub spend_sats: u64,
    pub spend_job_id: String,
    pub intent_event_id: Option<String>,
    pub offer_event_id: Option<String>,
    pub envelope_event_id: Option<String>,
    pub spend_event_id: Option<String>,
}

impl Default for CreditDeskPaneState {
    fn default() -> Self {
        Self {
            load_state: PaneLoadState::Loading,
            last_error: None,
            last_action: Some("Waiting for AC credit desk snapshot".to_string()),
            scope: "skill:33400:npub1agent:summarize-text:0.1.0:constraints".to_string(),
            requested_sats: 1500,
            offered_sats: 1400,
            envelope_cap_sats: 1200,
            spend_sats: 600,
            spend_job_id: "job-credit-001".to_string(),
            intent_event_id: None,
            offer_event_id: None,
            envelope_event_id: None,
            spend_event_id: None,
        }
    }
}

pub struct CreditSettlementLedgerPaneState {
    pub load_state: PaneLoadState,
    pub last_error: Option<String>,
    pub last_action: Option<String>,
    pub result_event_id: String,
    pub payment_pointer: String,
    pub default_reason: String,
    pub settlement_event_id: Option<String>,
    pub default_event_id: Option<String>,
}

impl Default for CreditSettlementLedgerPaneState {
    fn default() -> Self {
        Self {
            load_state: PaneLoadState::Loading,
            last_error: None,
            last_action: Some("Waiting for AC settlement ledger snapshot".to_string()),
            result_event_id: "nip90:result:pending".to_string(),
            payment_pointer: "pay:pending".to_string(),
            default_reason: "settlement timeout".to_string(),
            settlement_event_id: None,
            default_event_id: None,
        }
    }
}

pub struct EarningsScoreboardState {
    pub load_state: PaneLoadState,
    pub last_error: Option<String>,
    pub last_action: Option<String>,
    pub sats_today: u64,
    pub lifetime_sats: u64,
    pub jobs_today: u64,
    pub last_job_result: String,
    pub online_uptime_seconds: u64,
    pub stale_after: Duration,
    pub last_refreshed_at: Option<Instant>,
}

impl Default for EarningsScoreboardState {
    fn default() -> Self {
        Self {
            load_state: PaneLoadState::Loading,
            last_error: None,
            last_action: Some("Waiting for wallet + job receipts".to_string()),
            sats_today: 0,
            lifetime_sats: 0,
            jobs_today: 0,
            last_job_result: "none".to_string(),
            online_uptime_seconds: 0,
            stale_after: Duration::from_secs(12),
            last_refreshed_at: None,
        }
    }
}

impl EarningsScoreboardState {
    pub fn refresh_from_sources(
        &mut self,
        now: Instant,
        provider_runtime: &ProviderRuntimeState,
        job_history: &JobHistoryState,
        spark_wallet: &SparkPaneState,
    ) {
        self.last_refreshed_at = Some(now);
        self.online_uptime_seconds = provider_runtime.uptime_seconds(now);
        self.last_error = None;

        if let Some(error) = spark_wallet.last_error.as_deref() {
            self.load_state = PaneLoadState::Error;
            self.last_error = Some(format!("Wallet source error: {error}"));
            self.last_action = Some("Scoreboard degraded due to wallet error".to_string());
        } else if spark_wallet.balance.is_none() {
            self.load_state = PaneLoadState::Loading;
            self.last_action = Some("Scoreboard waiting for first wallet refresh".to_string());
        } else {
            self.load_state = PaneLoadState::Ready;
            self.last_action = Some("Scoreboard refreshed from authoritative sources".to_string());
        }

        self.lifetime_sats = spark_wallet
            .balance
            .as_ref()
            .map_or(0, |balance| balance.total_sats());

        let threshold = job_history.reference_epoch_seconds.saturating_sub(86_400);
        self.jobs_today = job_history
            .rows
            .iter()
            .filter(|row| {
                row.status == JobHistoryStatus::Succeeded
                    && row.completed_at_epoch_seconds >= threshold
            })
            .count() as u64;
        self.sats_today = job_history
            .rows
            .iter()
            .filter(|row| {
                row.status == JobHistoryStatus::Succeeded
                    && row.completed_at_epoch_seconds >= threshold
            })
            .map(|row| row.payout_sats)
            .sum();

        self.last_job_result = job_history
            .rows
            .first()
            .map(|row| {
                if let Some(reason) = row.failure_reason.as_deref() {
                    format!("{} ({reason})", row.status.label())
                } else {
                    row.status.label().to_string()
                }
            })
            .unwrap_or_else(|| "none".to_string());
    }

    pub fn is_stale(&self, now: Instant) -> bool {
        self.last_refreshed_at
            .is_none_or(|refresh| now.duration_since(refresh) > self.stale_after)
    }
}

impl ProviderBlocker {
    pub const fn code(self) -> &'static str {
        match self {
            Self::IdentityMissing => "IDENTITY_MISSING",
            Self::WalletError => "WALLET_ERROR",
            Self::SkillTrustUnavailable => "SKL_TRUST_UNAVAILABLE",
            Self::CreditLaneUnavailable => "AC_CREDIT_UNAVAILABLE",
        }
    }

    pub const fn detail(self) -> &'static str {
        match self {
            Self::IdentityMissing => "Nostr identity is not ready",
            Self::WalletError => "Spark wallet reports an error",
            Self::SkillTrustUnavailable => "SKL trust gate is not trusted",
            Self::CreditLaneUnavailable => "AC credit lane is not available",
        }
    }
}

pub struct ProviderRuntimeState {
    pub mode: ProviderMode,
    pub mode_changed_at: Instant,
    pub connecting_until: Option<Instant>,
    pub online_since: Option<Instant>,
    pub last_heartbeat_at: Option<Instant>,
    pub heartbeat_interval: Duration,
    pub queue_depth: u32,
    pub last_completed_job_at: Option<Instant>,
    pub last_result: Option<String>,
    pub degraded_reason_code: Option<String>,
    pub last_error_detail: Option<String>,
    pub last_authoritative_status: Option<String>,
    pub last_authoritative_event_id: Option<String>,
    pub last_authoritative_error_class: Option<String>,
}

impl Default for ProviderRuntimeState {
    fn default() -> Self {
        let now = Instant::now();
        Self {
            mode: ProviderMode::Offline,
            mode_changed_at: now,
            connecting_until: None,
            online_since: None,
            last_heartbeat_at: None,
            heartbeat_interval: Duration::from_secs(1),
            queue_depth: 0,
            last_completed_job_at: None,
            last_result: None,
            degraded_reason_code: None,
            last_error_detail: None,
            last_authoritative_status: None,
            last_authoritative_event_id: None,
            last_authoritative_error_class: None,
        }
    }
}

impl ProviderRuntimeState {
    pub fn toggle_online(&mut self, now: Instant, blockers: &[ProviderBlocker]) {
        if self.mode == ProviderMode::Offline {
            self.start_online(now, blockers);
        } else {
            self.go_offline(now);
        }
    }

    pub fn tick(&mut self, now: Instant, blockers: &[ProviderBlocker]) -> bool {
        let mut changed = false;

        if self.mode == ProviderMode::Connecting
            && self.connecting_until.is_some_and(|until| now >= until)
        {
            if blockers.is_empty() {
                self.mode = ProviderMode::Online;
                self.mode_changed_at = now;
                self.connecting_until = None;
                self.online_since = Some(now);
                self.last_heartbeat_at = Some(now);
                self.degraded_reason_code = None;
                self.last_error_detail = None;
            } else {
                self.move_degraded(now, blockers);
            }
            changed = true;
        }

        if self.mode == ProviderMode::Online {
            let should_heartbeat = self
                .last_heartbeat_at
                .is_none_or(|last| now.duration_since(last) >= self.heartbeat_interval);
            if should_heartbeat {
                self.last_heartbeat_at = Some(now);
                changed = true;
            }
        }

        changed
    }

    pub fn uptime_seconds(&self, now: Instant) -> u64 {
        self.online_since
            .and_then(|started| now.checked_duration_since(started))
            .map_or(0, |duration| duration.as_secs())
    }

    pub fn heartbeat_age_seconds(&self, now: Instant) -> Option<u64> {
        self.last_heartbeat_at
            .and_then(|last| now.checked_duration_since(last))
            .map(|duration| duration.as_secs())
    }

    fn start_online(&mut self, now: Instant, blockers: &[ProviderBlocker]) {
        if blockers.is_empty() {
            self.mode = ProviderMode::Connecting;
            self.mode_changed_at = now;
            self.connecting_until = Some(now + Duration::from_millis(900));
            self.degraded_reason_code = None;
            self.last_error_detail = None;
        } else {
            self.move_degraded(now, blockers);
        }
    }

    fn go_offline(&mut self, now: Instant) {
        self.mode = ProviderMode::Offline;
        self.mode_changed_at = now;
        self.connecting_until = None;
        self.online_since = None;
        self.last_heartbeat_at = None;
        self.queue_depth = 0;
        self.degraded_reason_code = None;
        self.last_error_detail = None;
    }

    fn move_degraded(&mut self, now: Instant, blockers: &[ProviderBlocker]) {
        self.mode = ProviderMode::Degraded;
        self.mode_changed_at = now;
        self.connecting_until = None;
        self.online_since = None;
        self.last_heartbeat_at = None;
        self.degraded_reason_code = blockers.first().map(|blocker| blocker.code().to_string());
        self.last_error_detail = Some(
            blockers
                .iter()
                .map(|blocker| blocker.detail())
                .collect::<Vec<_>>()
                .join("; "),
        );
    }
}

pub struct NostrSecretState {
    pub reveal_duration: Duration,
    pub revealed_until: Option<Instant>,
    pub copy_notice: Option<String>,
    pub copy_notice_until: Option<Instant>,
}

impl Default for NostrSecretState {
    fn default() -> Self {
        Self {
            reveal_duration: Duration::from_secs(12),
            revealed_until: None,
            copy_notice: None,
            copy_notice_until: None,
        }
    }
}

impl NostrSecretState {
    pub fn is_revealed(&self, now: Instant) -> bool {
        self.revealed_until.is_some_and(|until| until > now)
    }

    pub fn toggle_reveal(&mut self, now: Instant) {
        if self.is_revealed(now) {
            self.revealed_until = None;
        } else {
            self.revealed_until = Some(now + self.reveal_duration);
        }
    }

    pub fn set_copy_notice(&mut self, now: Instant, message: String) {
        self.copy_notice = Some(message);
        self.copy_notice_until = Some(now + Duration::from_secs(4));
    }

    pub fn expire(&mut self, now: Instant) -> bool {
        let mut changed = false;

        if self.revealed_until.is_some_and(|until| until <= now) {
            self.revealed_until = None;
            changed = true;
        }

        if self.copy_notice_until.is_some_and(|until| until <= now) {
            self.copy_notice = None;
            self.copy_notice_until = None;
            changed = true;
        }

        changed
    }
}

impl_pane_status_access!(
    RelayConnectionsState,
    SyncHealthState,
    NetworkRequestsState,
    StarterJobsState,
    ActivityFeedState,
    AlertsRecoveryState,
    SettingsState,
    JobInboxState,
    ActiveJobState,
    JobHistoryState,
    AgentProfileStatePaneState,
    AgentScheduleTickPaneState,
    TrajectoryAuditPaneState,
    SkillRegistryPaneState,
    SkillTrustRevocationPaneState,
    CreditDeskPaneState,
    CreditSettlementLedgerPaneState,
);

pub struct RenderState {
    pub window: Arc<Window>,
    pub surface: wgpu::Surface<'static>,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub config: wgpu::SurfaceConfiguration,
    pub renderer: Renderer,
    pub text_system: TextSystem,
    pub scale_factor: f32,
    pub hotbar: Hotbar,
    pub hotbar_bounds: Bounds,
    pub event_context: EventContext,
    pub input_modifiers: Modifiers,
    pub panes: Vec<DesktopPane>,
    pub nostr_identity: Option<NostrIdentity>,
    pub nostr_identity_error: Option<String>,
    pub nostr_secret_state: NostrSecretState,
    pub spark_wallet: SparkPaneState,
    pub spark_worker: SparkWalletWorker,
    pub spark_inputs: SparkPaneInputs,
    pub pay_invoice_inputs: PayInvoicePaneInputs,
    pub create_invoice_inputs: CreateInvoicePaneInputs,
    pub relay_connections_inputs: RelayConnectionsPaneInputs,
    pub network_requests_inputs: NetworkRequestsPaneInputs,
    pub settings_inputs: SettingsPaneInputs,
    pub job_history_inputs: JobHistoryPaneInputs,
    pub chat_inputs: ChatPaneInputs,
    pub autopilot_chat: AutopilotChatState,
    pub sa_lane: SaLaneSnapshot,
    pub skl_lane: SklLaneSnapshot,
    pub ac_lane: AcLaneSnapshot,
    pub sa_lane_worker: SaLaneWorker,
    pub skl_lane_worker: SklLaneWorker,
    pub ac_lane_worker: AcLaneWorker,
    pub runtime_command_responses: Vec<RuntimeCommandResponse>,
    pub next_runtime_command_seq: u64,
    pub provider_runtime: ProviderRuntimeState,
    pub earnings_scoreboard: EarningsScoreboardState,
    pub relay_connections: RelayConnectionsState,
    pub sync_health: SyncHealthState,
    pub network_requests: NetworkRequestsState,
    pub starter_jobs: StarterJobsState,
    pub activity_feed: ActivityFeedState,
    pub alerts_recovery: AlertsRecoveryState,
    pub settings: SettingsState,
    pub job_inbox: JobInboxState,
    pub active_job: ActiveJobState,
    pub job_history: JobHistoryState,
    pub agent_profile_state: AgentProfileStatePaneState,
    pub agent_schedule_tick: AgentScheduleTickPaneState,
    pub trajectory_audit: TrajectoryAuditPaneState,
    pub skill_registry: SkillRegistryPaneState,
    pub skill_trust_revocation: SkillTrustRevocationPaneState,
    pub credit_desk: CreditDeskPaneState,
    pub credit_settlement_ledger: CreditSettlementLedgerPaneState,
    pub next_pane_id: u64,
    pub next_z_index: i32,
    pub pane_drag_mode: Option<PaneDragMode>,
    pub pane_resizer: ResizablePane,
    pub hotbar_flash_was_active: bool,
    pub command_palette: CommandPalette,
    pub command_palette_actions: Rc<RefCell<Vec<String>>>,
}

impl RenderState {
    fn allocate_runtime_command_seq(&mut self) -> u64 {
        let seq = self.next_runtime_command_seq;
        self.next_runtime_command_seq = self.next_runtime_command_seq.saturating_add(1);
        seq
    }

    pub fn queue_sa_command(&mut self, command: SaLifecycleCommand) -> Result<u64, String> {
        let seq = self.allocate_runtime_command_seq();
        self.sa_lane_worker.enqueue(seq, command).map(|()| seq)
    }

    pub fn queue_skl_command(&mut self, command: SklDiscoveryTrustCommand) -> Result<u64, String> {
        let seq = self.allocate_runtime_command_seq();
        self.skl_lane_worker.enqueue(seq, command).map(|()| seq)
    }

    pub fn queue_ac_command(&mut self, command: AcCreditCommand) -> Result<u64, String> {
        let seq = self.allocate_runtime_command_seq();
        self.ac_lane_worker.enqueue(seq, command).map(|()| seq)
    }

    pub fn record_runtime_command_response(&mut self, response: RuntimeCommandResponse) {
        self.runtime_command_responses.push(response);
        if self.runtime_command_responses.len() > 128 {
            let overflow = self.runtime_command_responses.len().saturating_sub(128);
            self.runtime_command_responses.drain(0..overflow);
        }
    }

    pub fn provider_blockers(&self) -> Vec<ProviderBlocker> {
        let mut blockers = Vec::new();
        if self.nostr_identity.is_none() {
            blockers.push(ProviderBlocker::IdentityMissing);
        }
        if self.spark_wallet.last_error.is_some() {
            blockers.push(ProviderBlocker::WalletError);
        }
        if self.skl_lane.trust_tier != SkillTrustTier::Trusted {
            blockers.push(ProviderBlocker::SkillTrustUnavailable);
        }
        if !self.ac_lane.credit_available {
            blockers.push(ProviderBlocker::CreditLaneUnavailable);
        }
        blockers
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ActiveJobState, ActivityEventDomain, ActivityEventRow, ActivityFeedFilter,
        ActivityFeedState, AlertDomain, AlertLifecycle, AlertsRecoveryState, AutopilotChatState,
        AutopilotMessageStatus, EarningsScoreboardState, JobHistoryState, JobHistoryStatus,
        JobHistoryStatusFilter, JobHistoryTimeRange, JobInboxDecision, JobInboxNetworkRequest,
        JobInboxState, JobInboxValidation, JobLifecycleStage, NetworkRequestStatus,
        NetworkRequestsState, NostrSecretState, ProviderBlocker, ProviderMode,
        ProviderRuntimeState, RecoveryAlertRow, RelayConnectionStatus, RelayConnectionsState,
        SettingsState, SparkPaneState, StarterJobRow, StarterJobStatus, StarterJobsState,
        SyncHealthState, SyncRecoveryPhase,
    };

    fn fixture_inbox_request(
        request_id: &str,
        capability: &str,
        price_sats: u64,
        ttl_seconds: u64,
        validation: JobInboxValidation,
    ) -> JobInboxNetworkRequest {
        JobInboxNetworkRequest {
            request_id: request_id.to_string(),
            requester: format!("npub1{request_id}"),
            capability: capability.to_string(),
            skill_scope_id: None,
            skl_manifest_a: None,
            skl_manifest_event_id: None,
            sa_tick_request_event_id: None,
            sa_tick_result_event_id: None,
            ac_envelope_event_id: None,
            price_sats,
            ttl_seconds,
            validation,
        }
    }

    fn seed_job_inbox(requests: Vec<JobInboxNetworkRequest>) -> JobInboxState {
        let mut inbox = JobInboxState::default();
        for request in requests {
            inbox.upsert_network_request(request);
        }
        inbox
    }

    fn fixture_history_row(
        job_id: &str,
        status: JobHistoryStatus,
        completed_at_epoch_seconds: u64,
        payout_sats: u64,
    ) -> super::JobHistoryReceiptRow {
        super::JobHistoryReceiptRow {
            job_id: job_id.to_string(),
            status,
            completed_at_epoch_seconds,
            skill_scope_id: None,
            skl_manifest_a: None,
            skl_manifest_event_id: None,
            sa_tick_result_event_id: None,
            sa_trajectory_session_id: None,
            ac_envelope_event_id: None,
            ac_settlement_event_id: None,
            ac_default_event_id: None,
            payout_sats,
            result_hash: format!("sha256:{job_id}"),
            payment_pointer: format!("pay:{job_id}"),
            failure_reason: if status == JobHistoryStatus::Failed {
                Some("failure".to_string())
            } else {
                None
            },
        }
    }

    fn seed_job_history(rows: Vec<super::JobHistoryReceiptRow>) -> JobHistoryState {
        let mut history = JobHistoryState::default();
        for row in rows {
            history.upsert_row(row);
        }
        history
    }

    fn fixture_starter_job(
        job_id: &str,
        payout_sats: u64,
        eligible: bool,
        status: StarterJobStatus,
    ) -> StarterJobRow {
        StarterJobRow {
            job_id: job_id.to_string(),
            summary: "Process starter job".to_string(),
            payout_sats,
            eligible,
            status,
            payout_pointer: None,
        }
    }

    fn fixture_activity_event(
        event_id: &str,
        domain: ActivityEventDomain,
        occurred_at_epoch_seconds: u64,
    ) -> ActivityEventRow {
        ActivityEventRow {
            event_id: event_id.to_string(),
            domain,
            source_tag: domain.source_tag().to_string(),
            occurred_at_epoch_seconds,
            summary: format!("summary {event_id}"),
            detail: format!("detail {event_id}"),
        }
    }

    fn fixture_alert(
        alert_id: &str,
        domain: AlertDomain,
        severity: super::AlertSeverity,
        lifecycle: AlertLifecycle,
    ) -> RecoveryAlertRow {
        RecoveryAlertRow {
            alert_id: alert_id.to_string(),
            domain,
            severity,
            lifecycle,
            summary: format!("summary {alert_id}"),
            remediation: format!("remediation {alert_id}"),
            last_transition_epoch_seconds: 1_761_920_080,
        }
    }

    #[test]
    fn nostr_reveal_state_expires() {
        let mut state = NostrSecretState::default();
        let now = std::time::Instant::now();
        state.toggle_reveal(now);
        assert!(state.is_revealed(now));

        let expired_at = now + state.reveal_duration + std::time::Duration::from_millis(1);
        assert!(state.expire(expired_at));
        assert!(!state.is_revealed(expired_at));
    }

    #[test]
    fn nostr_copy_notice_expires() {
        let mut state = NostrSecretState::default();
        let now = std::time::Instant::now();
        state.set_copy_notice(now, "Copied".to_string());
        assert_eq!(state.copy_notice.as_deref(), Some("Copied"));

        let expired_at = now + std::time::Duration::from_secs(5);
        assert!(state.expire(expired_at));
        assert!(state.copy_notice.is_none());
    }

    #[test]
    fn provider_state_connects_then_becomes_online() {
        let mut provider = ProviderRuntimeState::default();
        let now = std::time::Instant::now();
        provider.toggle_online(now, &[]);
        assert_eq!(provider.mode, ProviderMode::Connecting);

        let advanced = now + std::time::Duration::from_secs(1);
        assert!(provider.tick(advanced, &[]));
        assert_eq!(provider.mode, ProviderMode::Online);
        assert!(provider.online_since.is_some());
        assert!(provider.last_heartbeat_at.is_some());
    }

    #[test]
    fn provider_state_enters_degraded_when_blocked() {
        let mut provider = ProviderRuntimeState::default();
        let now = std::time::Instant::now();
        provider.toggle_online(now, &[ProviderBlocker::IdentityMissing]);
        assert_eq!(provider.mode, ProviderMode::Degraded);
        assert_eq!(
            provider.degraded_reason_code.as_deref(),
            Some(ProviderBlocker::IdentityMissing.code())
        );
    }

    #[test]
    fn chat_state_progresses_queued_to_done() {
        let mut chat = AutopilotChatState::default();
        let now = std::time::Instant::now();
        chat.submit_prompt(now, "ping".to_string());
        assert!(
            chat.messages
                .iter()
                .any(|message| message.status == AutopilotMessageStatus::Queued)
        );

        assert!(chat.tick(now + std::time::Duration::from_millis(300)));
        assert!(
            chat.messages
                .iter()
                .any(|message| message.status == AutopilotMessageStatus::Running)
        );

        assert!(chat.tick(now + std::time::Duration::from_secs(2)));
        assert!(!chat.has_pending_messages());
    }

    #[test]
    fn job_inbox_upsert_collapses_duplicate_request_ids() {
        let inbox = seed_job_inbox(vec![
            fixture_inbox_request("req-dup", "cap.one", 11, 60, JobInboxValidation::Pending),
            fixture_inbox_request("req-dup", "cap.one", 22, 120, JobInboxValidation::Valid),
        ]);

        let duplicates = inbox
            .requests
            .iter()
            .filter(|request| request.request_id == "req-dup")
            .count();
        assert_eq!(duplicates, 1);
        let request = inbox
            .requests
            .iter()
            .find(|request| request.request_id == "req-dup")
            .expect("request should exist");
        assert_eq!(request.price_sats, 22);
        assert_eq!(request.ttl_seconds, 120);
    }

    #[test]
    fn job_inbox_accept_updates_selected_request_decision() {
        let mut inbox = seed_job_inbox(vec![fixture_inbox_request(
            "req-accept",
            "summarize.text",
            900,
            120,
            JobInboxValidation::Valid,
        )]);
        assert!(inbox.select_by_index(0));
        let request_id = inbox
            .selected_request()
            .expect("selection should exist")
            .request_id
            .clone();

        let decided = inbox
            .decide_selected(true, "valid + priced")
            .expect("decision should succeed");
        assert_eq!(decided, request_id);
        let selected = inbox.selected_request().expect("selected request remains");
        assert!(matches!(
            selected.decision,
            JobInboxDecision::Accepted { ref reason } if reason == "valid + priced"
        ));
    }

    #[test]
    fn active_job_advance_stage_updates_lifecycle() {
        let mut inbox = seed_job_inbox(vec![fixture_inbox_request(
            "req-active",
            "summarize.text",
            1500,
            300,
            JobInboxValidation::Valid,
        )]);
        assert!(inbox.select_by_index(0));
        let request = inbox
            .selected_request()
            .expect("request should exist")
            .clone();

        let mut active = ActiveJobState::default();
        active.start_from_request(&request);
        let stage = active.advance_stage().expect("advance should succeed");
        assert_eq!(stage, JobLifecycleStage::Running);
        let stage = active
            .advance_stage()
            .expect("second advance should succeed");
        assert_eq!(stage, JobLifecycleStage::Delivered);
        let current = active.job.as_ref().expect("active job exists");
        assert_eq!(current.stage, JobLifecycleStage::Delivered);
        assert!(current.invoice_id.is_some());
    }

    #[test]
    fn job_history_filters_search_status_and_time() {
        let mut history = seed_job_history(vec![
            fixture_history_row(
                "job-bootstrap-000",
                JobHistoryStatus::Succeeded,
                1_761_919_970,
                2100,
            ),
            fixture_history_row(
                "job-bootstrap-001",
                JobHistoryStatus::Failed,
                1_761_919_940,
                0,
            ),
        ]);
        history.status_filter = JobHistoryStatusFilter::Succeeded;
        history.time_range = JobHistoryTimeRange::All;
        history.set_search_job_id("bootstrap-000".to_string());

        let rows = history.paged_rows();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].status, JobHistoryStatus::Succeeded);
        assert!(rows[0].job_id.contains("bootstrap-000"));
    }

    #[test]
    fn job_history_upsert_keeps_single_row_per_job_id() {
        let mut history = seed_job_history(vec![fixture_history_row(
            "job-bootstrap-000",
            JobHistoryStatus::Succeeded,
            1_761_920_000,
            1200,
        )]);
        let before = history.rows.len();
        history.upsert_row(super::JobHistoryReceiptRow {
            job_id: "job-bootstrap-000".to_string(),
            status: JobHistoryStatus::Failed,
            completed_at_epoch_seconds: history.reference_epoch_seconds + 10,
            skill_scope_id: None,
            skl_manifest_a: None,
            skl_manifest_event_id: None,
            sa_tick_result_event_id: None,
            sa_trajectory_session_id: None,
            ac_envelope_event_id: None,
            ac_settlement_event_id: None,
            ac_default_event_id: None,
            payout_sats: 0,
            result_hash: "sha256:updated".to_string(),
            payment_pointer: "pay:updated".to_string(),
            failure_reason: Some("updated".to_string()),
        });

        assert_eq!(history.rows.len(), before);
        let row = history
            .rows
            .iter()
            .find(|row| row.job_id == "job-bootstrap-000")
            .expect("row should exist");
        assert_eq!(row.result_hash, "sha256:updated");
    }

    #[test]
    fn relay_connections_add_retry_remove_flow() {
        let mut relays = RelayConnectionsState::default();
        assert!(relays.add_relay("wss://relay.new.example").is_ok());
        assert_eq!(
            relays.selected().map(|row| row.url.as_str()),
            Some("wss://relay.new.example")
        );

        assert!(relays.retry_selected().is_ok());
        assert_eq!(
            relays.selected().map(|row| row.status),
            Some(RelayConnectionStatus::Connected)
        );

        assert!(relays.remove_selected().is_ok());
        assert!(
            relays
                .relays
                .iter()
                .all(|row| row.url != "wss://relay.new.example")
        );
    }

    #[test]
    fn sync_health_detects_stale_cursor_and_rebootstrap() {
        let provider = ProviderRuntimeState::default();
        let relays = RelayConnectionsState::default();
        let mut sync = SyncHealthState::default();

        sync.cursor_last_advanced_seconds_ago = sync.cursor_stale_after_seconds + 5;
        sync.refresh_from_runtime(std::time::Instant::now(), &provider, &relays);
        assert_eq!(sync.load_state, super::PaneLoadState::Error);
        assert_eq!(sync.recovery_phase, SyncRecoveryPhase::Reconnecting);

        sync.rebootstrap();
        assert_eq!(sync.load_state, super::PaneLoadState::Ready);
        assert_eq!(sync.recovery_phase, SyncRecoveryPhase::Replaying);
        assert_eq!(sync.cursor_last_advanced_seconds_ago, 0);
    }

    #[test]
    fn network_requests_submit_validates_and_records_stream_link() {
        let mut requests = NetworkRequestsState::default();
        let request_id = requests
            .queue_request_submission(
                "translate.text",
                "{\"text\":\"hola\"}",
                Some("33400:npub1agent:summarize-text:0.1.0".to_string()),
                Some("ac:39242:00000001".to_string()),
                1200,
                90,
                44,
            )
            .expect("request should be accepted");
        let first = requests
            .submitted
            .first()
            .expect("new request should be inserted at head");
        assert_eq!(first.request_id, request_id);
        assert_eq!(first.response_stream_id, format!("stream:{request_id}"));
        assert_eq!(first.status, NetworkRequestStatus::Submitted);
        assert_eq!(first.authority_command_seq, 44);
    }

    #[test]
    fn starter_jobs_complete_selected_sets_payout_pointer() {
        let mut starter_jobs = StarterJobsState::default();
        starter_jobs.jobs.push(fixture_starter_job(
            "job-starter-001",
            1200,
            true,
            StarterJobStatus::Queued,
        ));
        starter_jobs.select_by_index(0);
        let (job_id, _payout, pointer) = starter_jobs
            .complete_selected()
            .expect("eligible starter job should complete");
        let job = starter_jobs
            .jobs
            .iter()
            .find(|job| job.job_id == job_id)
            .expect("job should remain present");
        assert_eq!(job.status, StarterJobStatus::Completed);
        assert_eq!(job.payout_pointer.as_deref(), Some(pointer.as_str()));
    }

    #[test]
    fn activity_feed_upsert_deduplicates_stable_event_ids() {
        let mut feed = ActivityFeedState::default();
        feed.upsert_event(fixture_activity_event(
            "wallet:payment:latest",
            ActivityEventDomain::Wallet,
            1_761_920_180,
        ));
        let baseline_count = feed.rows.len();
        feed.upsert_event(fixture_activity_event(
            "wallet:payment:latest",
            ActivityEventDomain::Wallet,
            1_761_920_200,
        ));
        assert_eq!(feed.rows.len(), baseline_count);

        feed.set_filter(ActivityFeedFilter::Wallet);
        assert!(
            feed.visible_rows()
                .into_iter()
                .all(|row| row.domain == ActivityEventDomain::Wallet)
        );
    }

    #[test]
    fn alerts_recovery_lifecycle_transitions_are_deterministic() {
        let mut alerts = AlertsRecoveryState::default();
        alerts.alerts.push(fixture_alert(
            "alert:identity:missing",
            AlertDomain::Identity,
            super::AlertSeverity::Critical,
            AlertLifecycle::Active,
        ));
        alerts.select_by_index(0);
        let alert_id = alerts
            .acknowledge_selected()
            .expect("active alert should acknowledge");
        let alert = alerts
            .alerts
            .iter()
            .find(|alert| alert.alert_id == alert_id)
            .expect("alert should exist after ack");
        assert_eq!(alert.lifecycle, AlertLifecycle::Acknowledged);
        assert_eq!(alerts.selected_domain(), Some(AlertDomain::Identity));

        let resolved_id = alerts
            .resolve_selected()
            .expect("acknowledged alert should resolve");
        let resolved = alerts
            .alerts
            .iter()
            .find(|alert| alert.alert_id == resolved_id)
            .expect("alert should exist after resolve");
        assert_eq!(resolved.lifecycle, AlertLifecycle::Resolved);
    }

    #[test]
    fn settings_updates_validate_ranges_and_reconnect_notice() {
        let mut settings = SettingsState::default();
        settings
            .apply_updates_internal("wss://relay.primal.net", "2500", "8", false)
            .expect("valid settings update should apply");
        assert_eq!(settings.document.relay_url, "wss://relay.primal.net");
        assert_eq!(settings.document.wallet_default_send_sats, 2500);
        assert_eq!(settings.document.provider_max_queue_depth, 8);
        assert!(settings.document.reconnect_required);

        let invalid = settings.apply_updates_internal("https://bad-relay", "0", "0", false);
        assert!(invalid.is_err());
        assert_eq!(settings.load_state, super::PaneLoadState::Error);
    }

    #[test]
    fn settings_document_default_uses_identity_authority_path() {
        let document = super::SettingsDocumentV1::default();
        assert!(document.identity_path.contains("identity.mnemonic"));
    }

    #[test]
    fn parse_settings_document_overrides_stale_identity_path() {
        let raw = "schema_version=1\nrelay_url=wss://relay.example\nidentity_path=~/.openagents/nostr/identity.json\nwallet_default_send_sats=1000\nprovider_max_queue_depth=4\nreconnect_required=false\n";
        let document = super::parse_settings_document(raw).expect("settings parse should succeed");
        assert_ne!(document.identity_path, "~/.openagents/nostr/identity.json");
        assert!(document.identity_path.contains("identity.mnemonic"));
    }

    #[test]
    fn earnings_scoreboard_refreshes_from_wallet_and_history() {
        let mut score = EarningsScoreboardState::default();
        let provider = ProviderRuntimeState::default();
        let history = seed_job_history(vec![fixture_history_row(
            "job-earned-001",
            JobHistoryStatus::Succeeded,
            1_761_919_970,
            2100,
        )]);
        let mut spark = SparkPaneState::default();
        spark.balance = Some(openagents_spark::Balance {
            spark_sats: 1000,
            lightning_sats: 2000,
            onchain_sats: 3000,
        });

        let now = std::time::Instant::now();
        score.refresh_from_sources(now, &provider, &history, &spark);

        assert_eq!(score.load_state, super::PaneLoadState::Ready);
        assert_eq!(score.lifetime_sats, 6000);
        assert!(score.jobs_today >= 1);
        assert!(score.sats_today >= 1);
        assert!(!score.is_stale(now));
    }
}
