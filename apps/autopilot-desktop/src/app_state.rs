use std::sync::Arc;
use std::time::{Duration, Instant};
use std::{cell::RefCell, rc::Rc};

use nostr::NostrIdentity;
use wgpui::components::TextInput;
use wgpui::components::hud::{CommandPalette, Hotbar, PaneFrame, ResizablePane, ResizeEdge};
use wgpui::renderer::Renderer;
use wgpui::{Bounds, EventContext, Modifiers, Point, TextSystem};
use winit::window::Window;

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
    JobInbox,
    ActiveJob,
    JobHistory,
    NostrIdentity,
    SparkWallet,
    SparkCreateInvoice,
    SparkPayInvoice,
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
            budget_sats: TextInput::new().value("1500").placeholder("Budget sats"),
            timeout_seconds: TextInput::new().value("60").placeholder("Timeout seconds"),
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
            load_state: PaneLoadState::Ready,
            last_error: None,
            last_action: Some("Relay map loaded from deterministic cache".to_string()),
            relays: vec![
                RelayConnectionRow {
                    url: "wss://relay.damus.io".to_string(),
                    status: RelayConnectionStatus::Connected,
                    latency_ms: Some(84),
                    last_seen_seconds_ago: Some(1),
                    last_error: None,
                },
                RelayConnectionRow {
                    url: "wss://relay.primal.net".to_string(),
                    status: RelayConnectionStatus::Connecting,
                    latency_ms: None,
                    last_seen_seconds_ago: None,
                    last_error: None,
                },
                RelayConnectionRow {
                    url: "wss://relay.example.invalid".to_string(),
                    status: RelayConnectionStatus::Error,
                    latency_ms: None,
                    last_seen_seconds_ago: Some(45),
                    last_error: Some("TLS handshake failed".to_string()),
                },
                RelayConnectionRow {
                    url: "wss://relay.offline.example".to_string(),
                    status: RelayConnectionStatus::Disconnected,
                    latency_ms: None,
                    last_seen_seconds_ago: Some(380),
                    last_error: None,
                },
            ],
            selected_url: Some("wss://relay.damus.io".to_string()),
        }
    }
}

impl RelayConnectionsState {
    pub fn select_by_index(&mut self, index: usize) -> bool {
        let Some(url) = self.relays.get(index).map(|row| row.url.clone()) else {
            return false;
        };
        self.selected_url = Some(url);
        self.last_error = None;
        true
    }

    pub fn selected(&self) -> Option<&RelayConnectionRow> {
        let selected = self.selected_url.as_deref()?;
        self.relays.iter().find(|row| row.url == selected)
    }

    pub fn add_relay(&mut self, relay_url: &str) -> Result<(), String> {
        let relay = relay_url.trim();
        if relay.is_empty() {
            self.last_error = Some("Relay URL cannot be empty".to_string());
            self.load_state = PaneLoadState::Error;
            return Err("Relay URL cannot be empty".to_string());
        }
        if !relay.starts_with("wss://") {
            self.last_error = Some("Relay URL must start with wss://".to_string());
            self.load_state = PaneLoadState::Error;
            return Err("Relay URL must start with wss://".to_string());
        }
        if self.relays.iter().any(|row| row.url == relay) {
            self.last_error = Some("Relay already configured".to_string());
            self.load_state = PaneLoadState::Error;
            return Err("Relay already configured".to_string());
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
        self.last_error = None;
        self.load_state = PaneLoadState::Ready;
        self.last_action = Some(format!("Added relay {relay_url}"));
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
            self.last_error = Some("Selected relay no longer exists".to_string());
            self.load_state = PaneLoadState::Error;
            return Err("Selected relay no longer exists".to_string());
        }

        self.selected_url = self.relays.first().map(|row| row.url.clone());
        self.last_error = None;
        self.load_state = PaneLoadState::Ready;
        self.last_action = Some(format!("Removed relay {selected}"));
        Ok(selected)
    }

    pub fn retry_selected(&mut self) -> Result<String, String> {
        let selected = self
            .selected_url
            .as_deref()
            .ok_or_else(|| "Select a relay first".to_string())?
            .to_string();
        let Some(relay) = self.relays.iter_mut().find(|row| row.url == selected) else {
            self.last_error = Some("Selected relay no longer exists".to_string());
            self.load_state = PaneLoadState::Error;
            return Err("Selected relay no longer exists".to_string());
        };

        relay.status = RelayConnectionStatus::Connected;
        relay.latency_ms = Some(96);
        relay.last_seen_seconds_ago = Some(0);
        relay.last_error = None;
        self.last_error = None;
        self.load_state = PaneLoadState::Ready;
        self.last_action = Some(format!("Retried relay {selected}"));
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
            load_state: PaneLoadState::Ready,
            last_error: None,
            last_action: Some("Sync health hydrated from local spacetime cache".to_string()),
            spacetime_connection: "connected".to_string(),
            subscription_state: "subscribed".to_string(),
            cursor_position: 4312,
            cursor_stale_after_seconds: 12,
            cursor_last_advanced_seconds_ago: 2,
            recovery_phase: SyncRecoveryPhase::Idle,
            last_applied_event_seq: 4312,
            duplicate_drop_count: 17,
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
        self.last_error = None;
        self.load_state = PaneLoadState::Ready;
        self.last_action = Some(format!(
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
            self.load_state = PaneLoadState::Error;
            self.recovery_phase = SyncRecoveryPhase::Reconnecting;
            self.last_error = Some("Cursor stalled beyond stale threshold".to_string());
        } else if self.recovery_phase != SyncRecoveryPhase::Replaying {
            self.load_state = PaneLoadState::Ready;
            self.recovery_phase = SyncRecoveryPhase::Ready;
            self.last_error = None;
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
    pub request_type: String,
    pub payload: String,
    pub budget_sats: u64,
    pub timeout_seconds: u64,
    pub response_stream_id: String,
    pub status: NetworkRequestStatus,
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
            load_state: PaneLoadState::Ready,
            last_error: None,
            last_action: Some("Network request lane ready".to_string()),
            submitted: vec![
                SubmittedNetworkRequest {
                    request_id: "req-buy-0001".to_string(),
                    request_type: "summarize.text".to_string(),
                    payload: "{\"text\":\"hello world\"}".to_string(),
                    budget_sats: 900,
                    timeout_seconds: 45,
                    response_stream_id: "stream:req-buy-0001".to_string(),
                    status: NetworkRequestStatus::Streaming,
                },
                SubmittedNetworkRequest {
                    request_id: "req-buy-0000".to_string(),
                    request_type: "classify.image".to_string(),
                    payload: "{\"cid\":\"bafy...\"}".to_string(),
                    budget_sats: 1400,
                    timeout_seconds: 120,
                    response_stream_id: "stream:req-buy-0000".to_string(),
                    status: NetworkRequestStatus::Completed,
                },
                SubmittedNetworkRequest {
                    request_id: "req-buy-zz99".to_string(),
                    request_type: "invoice.parse".to_string(),
                    payload: "{\"invoice\":\"lnbc...\"}".to_string(),
                    budget_sats: 600,
                    timeout_seconds: 30,
                    response_stream_id: "stream:req-buy-zz99".to_string(),
                    status: NetworkRequestStatus::Failed,
                },
            ],
            next_request_seq: 2,
        }
    }
}

impl NetworkRequestsState {
    pub fn submit_request(
        &mut self,
        request_type: &str,
        payload: &str,
        budget_sats: &str,
        timeout_seconds: &str,
    ) -> Result<String, String> {
        let request_type = request_type.trim();
        if request_type.is_empty() {
            self.last_error = Some("Request type is required".to_string());
            self.load_state = PaneLoadState::Error;
            return Err("Request type is required".to_string());
        }

        let payload = payload.trim();
        if payload.is_empty() {
            self.last_error = Some("Request payload is required".to_string());
            self.load_state = PaneLoadState::Error;
            return Err("Request payload is required".to_string());
        }

        let budget_sats = budget_sats
            .trim()
            .parse::<u64>()
            .map_err(|error| format!("Budget sats must be an integer: {error}"))?;
        if budget_sats == 0 {
            self.last_error = Some("Budget sats must be greater than 0".to_string());
            self.load_state = PaneLoadState::Error;
            return Err("Budget sats must be greater than 0".to_string());
        }

        let timeout_seconds = timeout_seconds
            .trim()
            .parse::<u64>()
            .map_err(|error| format!("Timeout seconds must be an integer: {error}"))?;
        if timeout_seconds == 0 {
            self.last_error = Some("Timeout seconds must be greater than 0".to_string());
            self.load_state = PaneLoadState::Error;
            return Err("Timeout seconds must be greater than 0".to_string());
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
                budget_sats,
                timeout_seconds,
                response_stream_id: stream_id,
                status: NetworkRequestStatus::Submitted,
            },
        );
        self.last_error = None;
        self.load_state = PaneLoadState::Ready;
        self.last_action = Some(format!("Submitted buyer request {request_id}"));
        Ok(request_id)
    }
}

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
    pub price_sats: u64,
    pub ttl_seconds: u64,
    pub validation: JobInboxValidation,
    pub arrival_seq: u64,
    pub decision: JobInboxDecision,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JobInboxNetworkRequest {
    pub request_id: String,
    pub requester: String,
    pub capability: String,
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
    next_arrival_seq: u64,
}

impl Default for JobInboxState {
    fn default() -> Self {
        let mut state = Self {
            load_state: PaneLoadState::Loading,
            last_error: None,
            last_action: Some("Inbox synced from deterministic replay lane".to_string()),
            requests: Vec::new(),
            selected_request_id: None,
            next_arrival_seq: 1,
        };

        state.upsert_network_request(JobInboxNetworkRequest {
            request_id: "req-7f3d".to_string(),
            requester: "npub1alpha...2kz".to_string(),
            capability: "summarize.pdf".to_string(),
            price_sats: 2400,
            ttl_seconds: 900,
            validation: JobInboxValidation::Valid,
        });
        state.upsert_network_request(JobInboxNetworkRequest {
            request_id: "req-a19c".to_string(),
            requester: "npub1beta...m4r".to_string(),
            capability: "classify.image".to_string(),
            price_sats: 1300,
            ttl_seconds: 600,
            validation: JobInboxValidation::Pending,
        });
        state.upsert_network_request(JobInboxNetworkRequest {
            request_id: "req-c332".to_string(),
            requester: "npub1gamma...9vt".to_string(),
            capability: "invoice.parse".to_string(),
            price_sats: 700,
            ttl_seconds: 420,
            validation: JobInboxValidation::Invalid("signature mismatch".to_string()),
        });
        state.upsert_network_request(JobInboxNetworkRequest {
            request_id: "req-7f3d".to_string(),
            requester: "npub1alpha...2kz".to_string(),
            capability: "summarize.pdf".to_string(),
            price_sats: 2400,
            ttl_seconds: 900,
            validation: JobInboxValidation::Valid,
        });
        state.selected_request_id = state
            .requests
            .first()
            .map(|request| request.request_id.clone());
        state.load_state = PaneLoadState::Ready;
        state
    }
}

impl JobInboxState {
    pub fn upsert_network_request(&mut self, request: JobInboxNetworkRequest) {
        if let Some(existing) = self
            .requests
            .iter_mut()
            .find(|existing| existing.request_id == request.request_id)
        {
            existing.requester = request.requester;
            existing.capability = request.capability;
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
        let mut state = Self {
            load_state: PaneLoadState::Loading,
            last_error: None,
            last_action: Some("Recovered active job snapshot from replay lane".to_string()),
            runtime_supports_abort: false,
            job: None,
            next_event_seq: 1,
        };

        state.job = Some(ActiveJobRecord {
            job_id: "job-bootstrap-001".to_string(),
            request_id: "req-bootstrap-001".to_string(),
            requester: "npub1seed...r8k".to_string(),
            capability: "demo.capability".to_string(),
            quoted_price_sats: 1500,
            stage: JobLifecycleStage::Running,
            invoice_id: None,
            payment_id: None,
            failure_reason: None,
            events: vec![
                ActiveJobEvent {
                    seq: 1,
                    message: "received request from relay lane".to_string(),
                },
                ActiveJobEvent {
                    seq: 2,
                    message: "accepted request and queued runtime execution".to_string(),
                },
                ActiveJobEvent {
                    seq: 3,
                    message: "runtime execution started".to_string(),
                },
            ],
        });
        state.next_event_seq = 4;
        state.load_state = PaneLoadState::Ready;
        state
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
        }
        if next_stage == JobLifecycleStage::Paid {
            job.payment_id = Some(format!("pay-{}", job.request_id));
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
        let mut state = Self {
            load_state: PaneLoadState::Loading,
            last_error: None,
            last_action: Some("History loaded from deterministic receipt lane".to_string()),
            rows: Vec::new(),
            status_filter: JobHistoryStatusFilter::All,
            time_range: JobHistoryTimeRange::All,
            page: 0,
            page_size: 6,
            search_job_id: String::new(),
            reference_epoch_seconds: 1_761_920_000,
        };

        state.upsert_row(JobHistoryReceiptRow {
            job_id: "job-bootstrap-000".to_string(),
            status: JobHistoryStatus::Succeeded,
            completed_at_epoch_seconds: 1_761_919_780,
            payout_sats: 2100,
            result_hash: "sha256:7f7d72a3e0f10933".to_string(),
            payment_pointer: "pay:req-bootstrap-000".to_string(),
            failure_reason: None,
        });
        state.upsert_row(JobHistoryReceiptRow {
            job_id: "job-bootstrap-001".to_string(),
            status: JobHistoryStatus::Failed,
            completed_at_epoch_seconds: 1_761_915_200,
            payout_sats: 0,
            result_hash: "sha256:2ce0b2ff4ef9a010".to_string(),
            payment_pointer: "pay:req-bootstrap-001".to_string(),
            failure_reason: Some("invoice settlement timeout".to_string()),
        });
        state.load_state = PaneLoadState::Ready;
        state
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
        }
    }

    pub const fn detail(self) -> &'static str {
        match self {
            Self::IdentityMissing => "Nostr identity is not ready",
            Self::WalletError => "Spark wallet reports an error",
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
    pub job_history_inputs: JobHistoryPaneInputs,
    pub chat_inputs: ChatPaneInputs,
    pub autopilot_chat: AutopilotChatState,
    pub provider_runtime: ProviderRuntimeState,
    pub earnings_scoreboard: EarningsScoreboardState,
    pub relay_connections: RelayConnectionsState,
    pub sync_health: SyncHealthState,
    pub network_requests: NetworkRequestsState,
    pub job_inbox: JobInboxState,
    pub active_job: ActiveJobState,
    pub job_history: JobHistoryState,
    pub next_pane_id: u64,
    pub next_z_index: i32,
    pub pane_drag_mode: Option<PaneDragMode>,
    pub pane_resizer: ResizablePane,
    pub hotbar_flash_was_active: bool,
    pub command_palette: CommandPalette,
    pub command_palette_actions: Rc<RefCell<Vec<String>>>,
}

impl RenderState {
    pub fn provider_blockers(&self) -> Vec<ProviderBlocker> {
        let mut blockers = Vec::new();
        if self.nostr_identity.is_none() {
            blockers.push(ProviderBlocker::IdentityMissing);
        }
        if self.spark_wallet.last_error.is_some() {
            blockers.push(ProviderBlocker::WalletError);
        }
        blockers
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ActiveJobState, AutopilotChatState, AutopilotMessageStatus, EarningsScoreboardState,
        JobHistoryState, JobHistoryStatus, JobHistoryStatusFilter, JobHistoryTimeRange,
        JobInboxDecision, JobInboxNetworkRequest, JobInboxState, JobInboxValidation,
        JobLifecycleStage, NetworkRequestStatus, NetworkRequestsState, NostrSecretState,
        ProviderBlocker, ProviderMode, ProviderRuntimeState, RelayConnectionStatus,
        RelayConnectionsState, SparkPaneState, SyncHealthState, SyncRecoveryPhase,
    };

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
        let mut inbox = JobInboxState::default();
        inbox.upsert_network_request(JobInboxNetworkRequest {
            request_id: "req-dup".to_string(),
            requester: "npub1dup".to_string(),
            capability: "cap.one".to_string(),
            price_sats: 11,
            ttl_seconds: 60,
            validation: JobInboxValidation::Pending,
        });
        inbox.upsert_network_request(JobInboxNetworkRequest {
            request_id: "req-dup".to_string(),
            requester: "npub1dup".to_string(),
            capability: "cap.one".to_string(),
            price_sats: 22,
            ttl_seconds: 120,
            validation: JobInboxValidation::Valid,
        });

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
        let mut inbox = JobInboxState::default();
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
        let mut active = ActiveJobState::default();
        let stage = active.advance_stage().expect("advance should succeed");
        assert_eq!(stage, JobLifecycleStage::Delivered);
        let current = active.job.as_ref().expect("active job exists");
        assert_eq!(current.stage, JobLifecycleStage::Delivered);
        assert!(current.invoice_id.is_some());
    }

    #[test]
    fn job_history_filters_search_status_and_time() {
        let mut history = JobHistoryState::default();
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
        let mut history = JobHistoryState::default();
        let before = history.rows.len();
        history.upsert_row(super::JobHistoryReceiptRow {
            job_id: "job-bootstrap-000".to_string(),
            status: JobHistoryStatus::Failed,
            completed_at_epoch_seconds: history.reference_epoch_seconds + 10,
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
            .submit_request("translate.text", "{\"text\":\"hola\"}", "1200", "90")
            .expect("request should be accepted");
        let first = requests
            .submitted
            .first()
            .expect("new request should be inserted at head");
        assert_eq!(first.request_id, request_id);
        assert_eq!(first.response_stream_id, format!("stream:{request_id}"));
        assert_eq!(first.status, NetworkRequestStatus::Submitted);
    }

    #[test]
    fn earnings_scoreboard_refreshes_from_wallet_and_history() {
        let mut score = EarningsScoreboardState::default();
        let provider = ProviderRuntimeState::default();
        let history = JobHistoryState::default();
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
