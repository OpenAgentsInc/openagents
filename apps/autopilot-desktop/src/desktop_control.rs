use std::collections::VecDeque;
use std::fs;
use std::net::{IpAddr, SocketAddr};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::routing::{get, post};
use axum::{Json, Router};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use openagents_kernel_core::ids::sha256_prefixed_text;
use openagents_provider_substrate::ProviderDesiredMode;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::sync::{Notify, mpsc as tokio_mpsc, oneshot};

use crate::app_state::{
    DefaultNip28ChannelConfig, MISSION_CONTROL_BUY_MODE_BUDGET_SATS,
    MISSION_CONTROL_BUY_MODE_INTERVAL_SECONDS, RenderState,
};
use crate::bitcoin_display::format_sats_amount;
use crate::pane_system::MissionControlPaneAction;

const DESKTOP_CONTROL_SCHEMA_VERSION: u16 = 3;
const DESKTOP_CONTROL_SYNC_INTERVAL: Duration = Duration::from_millis(250);
const DESKTOP_CONTROL_MANIFEST_SCHEMA_VERSION: u16 = 1;
const DESKTOP_CONTROL_MANIFEST_FILENAME: &str = "desktop-control.json";
const DESKTOP_CONTROL_LOG_TAIL_LIMIT: usize = 64;
const DESKTOP_CONTROL_EVENT_BUFFER_LIMIT: usize = 512;
const DESKTOP_CONTROL_EVENT_QUERY_LIMIT: usize = 128;
const DESKTOP_CONTROL_EVENT_WAIT_TIMEOUT_MS: u64 = 20_000;

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
    pub apple_fm: DesktopControlAppleFmStatus,
    pub wallet: DesktopControlWalletStatus,
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
pub struct DesktopControlAppleFmStatus {
    pub reachable: bool,
    pub ready: bool,
    pub model_available: bool,
    pub ready_model: Option<String>,
    pub bridge_status: Option<String>,
    pub last_action: Option<String>,
    pub last_error: Option<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct DesktopControlWalletStatus {
    pub balance_sats: u64,
    pub network: String,
    pub network_status: String,
    pub can_withdraw: bool,
    pub withdraw_block_reason: Option<String>,
    pub last_action: Option<String>,
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
    pub next_dispatch_countdown_seconds: Option<u64>,
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

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum DesktopControlActionRequest {
    GetSnapshot,
    SetProviderMode {
        online: bool,
    },
    RefreshAppleFm,
    RunAppleFmSmokeTest,
    RefreshWallet,
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
            Self::SetProviderMode { online: true } => "provider-online",
            Self::SetProviderMode { online: false } => "provider-offline",
            Self::RefreshAppleFm => "apple-fm-refresh",
            Self::RunAppleFmSmokeTest => "apple-fm-smoke-test",
            Self::RefreshWallet => "wallet-refresh",
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

    fn mission_control_pane_action(&self) -> Option<MissionControlPaneAction> {
        match self {
            Self::RunAppleFmSmokeTest => Some(MissionControlPaneAction::RunLocalFmSummaryTest),
            Self::RefreshWallet => Some(MissionControlPaneAction::RefreshWallet),
            Self::StartBuyMode | Self::StopBuyMode => {
                Some(MissionControlPaneAction::ToggleBuyModeLoop)
            }
            Self::Withdraw { .. } => Some(MissionControlPaneAction::SendWithdrawal),
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
    if sync_runtime_snapshot(state) {
        changed = true;
    }
    changed
}

fn drain_runtime_updates(state: &mut RenderState) -> bool {
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

fn sync_runtime_snapshot(state: &mut RenderState) -> bool {
    let snapshot = snapshot_for_state(state);
    let snapshot_revision = snapshot.snapshot_revision;
    let signature = snapshot.state_signature.clone();
    let signature_changed =
        state.desktop_control_last_sync_signature.as_deref() != Some(signature.as_str());
    let should_sync = signature_changed
        || state
            .desktop_control_last_sync_at
            .is_none_or(|last| last.elapsed() >= DESKTOP_CONTROL_SYNC_INTERVAL);
    if !should_sync {
        return false;
    }
    let Some(runtime) = state.desktop_control_runtime.as_mut() else {
        return false;
    };
    if let Err(error) = runtime.sync_snapshot(snapshot.clone()) {
        state.desktop_control.last_error = Some(error);
        return false;
    }
    if signature_changed {
        let previous_snapshot = runtime.last_event_snapshot.clone();
        if let Err(error) = runtime.append_events(snapshot_change_events(
            previous_snapshot.as_ref(),
            &snapshot,
        )) {
            state.desktop_control.last_error = Some(error);
        }
        runtime.last_event_snapshot = Some(snapshot.clone());
    }
    state.desktop_control.last_snapshot_revision = snapshot_revision;
    state.desktop_control.last_snapshot_signature = Some(signature.clone());
    state.desktop_control_last_sync_signature = Some(signature);
    state.desktop_control_last_sync_at = Some(Instant::now());
    true
}

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
        .mission_control
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
        DesktopControlActionRequest::SetProviderMode { online } => {
            json!({ "command_label": action.label(), "online": online })
        }
        DesktopControlActionRequest::RefreshAppleFm
        | DesktopControlActionRequest::RunAppleFmSmokeTest
        | DesktopControlActionRequest::RefreshWallet
        | DesktopControlActionRequest::StartBuyMode
        | DesktopControlActionRequest::StopBuyMode
        | DesktopControlActionRequest::GetActiveJob
        | DesktopControlActionRequest::SelectNip28MainChannel => {
            json!({ "command_label": action.label() })
        }
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

fn apple_fm_status_summary(status: &DesktopControlAppleFmStatus) -> String {
    if status.ready {
        format!(
            "apple fm ready model={}",
            status.ready_model.as_deref().unwrap_or("unknown")
        )
    } else if status.reachable {
        "apple fm reachable; waiting for model readiness".to_string()
    } else {
        status
            .last_error
            .clone()
            .unwrap_or_else(|| "apple fm unavailable".to_string())
    }
}

fn wallet_status_summary(status: &DesktopControlWalletStatus) -> String {
    format!(
        "wallet balance={} network_status={} withdraw_ready={}",
        status.balance_sats, status.network_status, status.can_withdraw
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
        record_command_outcome(state, action.label(), &response);
        return attach_snapshot_metadata(state, response);
    }
    if let DesktopControlActionRequest::Withdraw { bolt11 } = action {
        let response = withdraw_action(state, bolt11.as_str());
        record_command_outcome(state, action.label(), &response);
        return attach_snapshot_metadata(state, response);
    }
    if let Some(mission_control_action) = action.mission_control_pane_action() {
        let response = match action {
            DesktopControlActionRequest::StartBuyMode => start_buy_mode_action(state),
            DesktopControlActionRequest::StopBuyMode => stop_buy_mode_action(state),
            DesktopControlActionRequest::RunAppleFmSmokeTest => {
                run_apple_fm_smoke_test_action(state)
            }
            _ => mission_control_action_response(state, mission_control_action),
        };
        record_command_outcome(state, action.label(), &response);
        return attach_snapshot_metadata(state, response);
    }
    let response = match action {
        DesktopControlActionRequest::GetSnapshot => {
            snapshot_payload_response(state, "Captured desktop control snapshot")
        }
        DesktopControlActionRequest::RefreshAppleFm => refresh_apple_fm_action(state),
        DesktopControlActionRequest::GetActiveJob => active_job_payload_response(state),
        DesktopControlActionRequest::SelectNip28MainChannel => {
            select_nip28_main_channel_action(state)
        }
        DesktopControlActionRequest::SelectNip28Group { group_id } => {
            select_nip28_group_action(state, group_id.as_str())
        }
        DesktopControlActionRequest::SelectNip28Channel { channel_id } => {
            select_nip28_channel_action(state, channel_id.as_str())
        }
        DesktopControlActionRequest::SendNip28Message {
            content,
            reply_to_event_id,
        } => send_nip28_message_action(state, content.as_str(), reply_to_event_id.as_deref()),
        DesktopControlActionRequest::RetryNip28Message { event_id } => {
            retry_nip28_message_action(state, event_id.as_str())
        }
        DesktopControlActionRequest::GetMissionControlLogTail { limit } => {
            log_tail_response(state, *limit)
        }
        DesktopControlActionRequest::SetProviderMode { .. }
        | DesktopControlActionRequest::RunAppleFmSmokeTest
        | DesktopControlActionRequest::RefreshWallet
        | DesktopControlActionRequest::StartBuyMode
        | DesktopControlActionRequest::StopBuyMode
        | DesktopControlActionRequest::Withdraw { .. } => {
            unreachable!("action-specific routes should be handled above")
        }
    };
    record_command_outcome(state, action.label(), &response);
    attach_snapshot_metadata(state, response)
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
) -> DesktopControlActionResponse {
    let snapshot = snapshot_for_state(state);
    match serde_json::to_value(snapshot) {
        Ok(payload) => DesktopControlActionResponse::ok_with_payload(message, payload),
        Err(error) => DesktopControlActionResponse::error(format!(
            "Failed to encode desktop control snapshot: {error}"
        )),
    }
}

fn attach_snapshot_metadata(
    state: &RenderState,
    response: DesktopControlActionResponse,
) -> DesktopControlActionResponse {
    let snapshot = snapshot_for_state(state);
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

fn refresh_apple_fm_action(state: &mut RenderState) -> DesktopControlActionResponse {
    if !crate::input::ensure_mission_control_apple_fm_refresh(state) {
        return DesktopControlActionResponse::error(
            "Apple FM refresh is unavailable in this session",
        );
    }
    mission_control_status_response(state, "Queued Apple FM refresh")
}

fn run_apple_fm_smoke_test_action(state: &mut RenderState) -> DesktopControlActionResponse {
    mission_control_action_response(state, MissionControlPaneAction::RunLocalFmSummaryTest)
}

fn mission_control_action_response(
    state: &mut RenderState,
    action: MissionControlPaneAction,
) -> DesktopControlActionResponse {
    crate::input::desktop_control_run_mission_control_action(state, action);
    mission_control_status_response(state, "Mission Control action applied")
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

fn start_buy_mode_action(state: &mut RenderState) -> DesktopControlActionResponse {
    if !state.mission_control_buy_mode_enabled() {
        return DesktopControlActionResponse::error("Buy Mode is disabled for this session");
    }
    if state.mission_control.buy_mode_loop_enabled {
        return DesktopControlActionResponse::ok(format!(
            "Buy Mode already running ({} every {}s)",
            format_sats_amount(MISSION_CONTROL_BUY_MODE_BUDGET_SATS),
            MISSION_CONTROL_BUY_MODE_INTERVAL_SECONDS
        ));
    }
    mission_control_action_response(state, MissionControlPaneAction::ToggleBuyModeLoop)
}

fn stop_buy_mode_action(state: &mut RenderState) -> DesktopControlActionResponse {
    if !state.mission_control.buy_mode_loop_enabled {
        return DesktopControlActionResponse::ok("Buy Mode already stopped");
    }
    mission_control_action_response(state, MissionControlPaneAction::ToggleBuyModeLoop)
}

fn active_job_payload_response(state: &RenderState) -> DesktopControlActionResponse {
    let payload = snapshot_for_state(state)
        .active_job
        .and_then(|active_job| serde_json::to_value(active_job).ok())
        .unwrap_or(Value::Null);
    if payload.is_null() {
        DesktopControlActionResponse::ok_with_payload("No active job", payload)
    } else {
        DesktopControlActionResponse::ok_with_payload("Captured active job state", payload)
    }
}

fn withdraw_action(state: &mut RenderState, bolt11: &str) -> DesktopControlActionResponse {
    let trimmed = bolt11.trim();
    if trimmed.is_empty() {
        return DesktopControlActionResponse::error("Withdrawal bolt11 invoice is required");
    }
    state
        .mission_control
        .withdraw_invoice
        .set_value(trimmed.to_string());
    mission_control_action_response(state, MissionControlPaneAction::SendWithdrawal)
}

fn log_tail_response(state: &RenderState, limit: usize) -> DesktopControlActionResponse {
    let lines = mission_control_recent_lines(state, limit);
    DesktopControlActionResponse::ok_with_payload(
        format!("Captured {} Mission Control log line(s)", lines.len()),
        json!({ "lines": lines }),
    )
}

pub fn snapshot_for_state(state: &RenderState) -> DesktopControlSnapshot {
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
    let wallet_balance_sats = state
        .spark_wallet
        .balance
        .as_ref()
        .map_or(0, |balance| balance.total_sats());
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

    let mut snapshot = DesktopControlSnapshot {
        schema_version: DESKTOP_CONTROL_SCHEMA_VERSION,
        snapshot_revision: 0,
        state_signature: String::new(),
        generated_at_epoch_ms: current_epoch_ms(),
        session: DesktopControlSessionStatus {
            pid: std::process::id(),
            shell_mode: if state.dev_mode_enabled() {
                "dev".to_string()
            } else {
                "production".to_string()
            },
            dev_mode_enabled: state.dev_mode_enabled(),
            buy_mode_surface_enabled: state.mission_control_buy_mode_enabled(),
        },
        mission_control: DesktopControlMissionControlStatus {
            last_action: state.mission_control.last_action.clone(),
            last_error: state.mission_control.last_error.clone(),
            can_go_online: state.mission_control_go_online_enabled(),
            blocker_codes: blocker_codes.clone(),
            log_line_count: state
                .mission_control
                .log_stream
                .recent_lines(usize::MAX)
                .len(),
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
        apple_fm: DesktopControlAppleFmStatus {
            reachable: state.provider_runtime.apple_fm.reachable,
            ready: state.provider_runtime.apple_fm.is_ready(),
            model_available: state.provider_runtime.apple_fm.model_available,
            ready_model: state.provider_runtime.apple_fm.ready_model.clone(),
            bridge_status: state.provider_runtime.apple_fm.bridge_status.clone(),
            last_action: state.provider_runtime.apple_fm.last_action.clone(),
            last_error: state.provider_runtime.apple_fm.last_error.clone(),
        },
        wallet: DesktopControlWalletStatus {
            balance_sats: wallet_balance_sats,
            network: state.spark_wallet.network_name().to_string(),
            network_status: state.spark_wallet.network_status_label().to_string(),
            can_withdraw: wallet_can_withdraw,
            withdraw_block_reason,
            last_action: state.spark_wallet.last_action.clone(),
            last_error: state.spark_wallet.last_error.clone(),
        },
        buy_mode: DesktopControlBuyModeStatus {
            enabled: state.mission_control.buy_mode_loop_enabled,
            approved_budget_sats: MISSION_CONTROL_BUY_MODE_BUDGET_SATS,
            cadence_seconds: MISSION_CONTROL_BUY_MODE_INTERVAL_SECONDS,
            next_dispatch_countdown_seconds: state
                .mission_control
                .buy_mode_next_dispatch_countdown_seconds(now),
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

    let signature = snapshot_sync_signature(&snapshot);
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
    let recent_messages = {
        let mut tail = chat.active_managed_chat_messages();
        if tail.len() > 16 {
            tail.drain(0..tail.len().saturating_sub(16));
        }
        tail.into_iter()
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
            .collect::<Vec<_>>()
    };
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

fn withdraw_readiness(balance_sats: u64, wallet_connected: bool) -> (bool, Option<String>) {
    if !wallet_connected {
        return (false, Some("wallet is not connected to Spark".to_string()));
    }
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
        | crate::nip90_compute_flow::Nip90FlowPhase::AwaitingPayment => "settling",
        crate::nip90_compute_flow::Nip90FlowPhase::DeliveredUnpaid => "unpaid",
        _ => active_job.stage.label(),
    }
}

fn mission_control_recent_lines(state: &RenderState, limit: usize) -> Vec<String> {
    state
        .mission_control
        .log_stream
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
        DesktopControlAppleFmStatus, DesktopControlBuyModeStatus,
        DesktopControlBuyModeTargetSelectionStatus, DesktopControlEventBatch,
        DesktopControlEventDraft, DesktopControlMissionControlStatus, DesktopControlProviderStatus,
        DesktopControlRuntime, DesktopControlRuntimeConfig, DesktopControlRuntimeUpdate,
        DesktopControlSessionStatus, DesktopControlSnapshot, DesktopControlWalletStatus,
        apply_response_snapshot_metadata, command_outcome_event, command_received_event,
        snapshot_change_events, snapshot_sync_signature, validate_control_bind_addr,
    };
    use crate::app_state::{
        AutopilotChatState, DefaultNip28ChannelConfig, ManagedChatDeliveryState,
        ManagedChatProjectionState, NetworkRequestSubmission,
    };
    use crate::autopilot_compute_presence::pump_provider_chat_presence;
    use crate::nip28_chat_lane::{Nip28ChatLaneUpdate, Nip28ChatLaneWorker};
    use crate::pane_system::MissionControlPaneAction;
    use crate::provider_nip90_lane::{
        ProviderNip90AuthIdentity, ProviderNip90ComputeCapability, ProviderNip90LaneCommand,
        ProviderNip90LaneUpdate, ProviderNip90LaneWorker, ProviderNip90PublishOutcome,
        ProviderNip90PublishRole,
    };
    use crate::spark_wallet::SparkPaneState;
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
                shell_mode: "production".to_string(),
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
            apple_fm: DesktopControlAppleFmStatus {
                reachable: true,
                ready: true,
                model_available: true,
                ready_model: Some("apple-foundation-model".to_string()),
                bridge_status: Some("running".to_string()),
                last_action: Some("Refreshed Apple FM bridge health; model ready.".to_string()),
                last_error: None,
            },
            wallet: DesktopControlWalletStatus {
                balance_sats: 77,
                network: "mainnet".to_string(),
                network_status: "connected".to_string(),
                can_withdraw: true,
                withdraw_block_reason: None,
                last_action: Some("Wallet refreshed".to_string()),
                last_error: None,
            },
            buy_mode: DesktopControlBuyModeStatus {
                enabled: false,
                approved_budget_sats: 2,
                cadence_seconds: 12,
                next_dispatch_countdown_seconds: None,
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
                selected_provider_pubkey: Some(selected.clone()),
                result_provider_pubkey: Some(result.clone()),
                invoice_provider_pubkey: Some(invoice.clone()),
                payable_provider_pubkey: None,
                last_feedback_status: Some("payment-required".to_string()),
                last_feedback_event_id: Some("feedback-role-split".to_string()),
                last_result_event_id: Some("result-role-split".to_string()),
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
            DesktopControlActionRequest::RunAppleFmSmokeTest.mission_control_pane_action(),
            Some(MissionControlPaneAction::RunLocalFmSummaryTest)
        );
        assert_eq!(
            DesktopControlActionRequest::RefreshWallet.mission_control_pane_action(),
            Some(MissionControlPaneAction::RefreshWallet)
        );
        assert_eq!(
            DesktopControlActionRequest::StartBuyMode.mission_control_pane_action(),
            Some(MissionControlPaneAction::ToggleBuyModeLoop)
        );
        assert_eq!(
            DesktopControlActionRequest::StopBuyMode.mission_control_pane_action(),
            Some(MissionControlPaneAction::ToggleBuyModeLoop)
        );
        assert_eq!(
            DesktopControlActionRequest::Withdraw {
                bolt11: "lnbc1example".to_string(),
            }
            .mission_control_pane_action(),
            Some(MissionControlPaneAction::SendWithdrawal)
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
