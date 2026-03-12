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
    MISSION_CONTROL_BUY_MODE_BUDGET_SATS, MISSION_CONTROL_BUY_MODE_INTERVAL_SECONDS, RenderState,
};
use crate::bitcoin_display::format_sats_amount;
use crate::pane_system::MissionControlPaneAction;

const DESKTOP_CONTROL_SCHEMA_VERSION: u16 = 1;
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
    pub payable_provider_pubkey: Option<String>,
    pub last_feedback_status: Option<String>,
    pub last_feedback_event_id: Option<String>,
    pub last_result_event_id: Option<String>,
    pub winning_result_event_id: Option<String>,
    pub payment_pointer: Option<String>,
    pub pending_bolt11: Option<String>,
    pub payment_notice: Option<String>,
    pub payment_error: Option<String>,
    pub wallet_status: String,
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
    pub payable_provider_pubkey: Option<String>,
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
    SetProviderMode { online: bool },
    RefreshAppleFm,
    RunAppleFmSmokeTest,
    RefreshWallet,
    StartBuyMode,
    StopBuyMode,
    GetActiveJob,
    Withdraw { bolt11: String },
    GetMissionControlLogTail { limit: usize },
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
            Self::Withdraw { .. } => "withdraw",
            Self::GetMissionControlLogTail { .. } => "log-tail",
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
                emit_control_events(
                    state,
                    vec![command_received_event(&envelope.action)],
                    false,
                );
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
        if let Err(error) =
            runtime.append_events(snapshot_change_events(previous_snapshot.as_ref(), &snapshot))
        {
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
        summary: format!("{} {} // {}", action.label(), outcome_label, response.message),
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
        | DesktopControlActionRequest::GetActiveJob => {
            json!({ "command_label": action.label() })
        }
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
    if buy_mode_status_changed(previous.map(|snapshot| &snapshot.buy_mode), &current.buy_mode) {
        changed_domains.push("buy_mode");
        events.push(DesktopControlEventDraft {
            event_type: "buyer.lifecycle.changed".to_string(),
            summary: buy_mode_status_summary(&current.buy_mode),
            command_label: None,
            success: None,
            payload: serde_json::to_value(&current.buy_mode).ok(),
        });
    }
    if active_job_status_changed(previous.and_then(|snapshot| snapshot.active_job.as_ref()), current.active_job.as_ref()) {
        changed_domains.push("active_job");
        events.push(DesktopControlEventDraft {
            event_type: "active_job.lifecycle.changed".to_string(),
            summary: active_job_status_summary(current.active_job.as_ref()),
            command_label: None,
            success: None,
            payload: serde_json::to_value(&current.active_job).ok(),
        });
    }
    if mission_control_status_changed(previous.map(|snapshot| &snapshot.mission_control), &current.mission_control) {
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
            "buy mode request={} status={} phase={}",
            short_request_id(request_id),
            request_status,
            phase
        ),
        (true, Some(request_id), _, _) => {
            format!("buy mode request={} in flight", short_request_id(request_id))
        }
        (true, None, _, _) => "buy mode running with no in-flight request".to_string(),
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
            || previous.payable_provider_pubkey != current.payable_provider_pubkey
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
    let response = match action {
        DesktopControlActionRequest::GetSnapshot => {
            snapshot_payload_response(state, "Captured desktop control snapshot")
        }
        DesktopControlActionRequest::SetProviderMode { online } => {
            apply_provider_mode_action(state, *online)
        }
        DesktopControlActionRequest::RefreshAppleFm => refresh_apple_fm_action(state),
        DesktopControlActionRequest::RunAppleFmSmokeTest => run_apple_fm_smoke_test_action(state),
        DesktopControlActionRequest::RefreshWallet => {
            mission_control_action_response(state, MissionControlPaneAction::RefreshWallet)
        }
        DesktopControlActionRequest::StartBuyMode => start_buy_mode_action(state),
        DesktopControlActionRequest::StopBuyMode => stop_buy_mode_action(state),
        DesktopControlActionRequest::GetActiveJob => active_job_payload_response(state),
        DesktopControlActionRequest::Withdraw { bolt11 } => withdraw_action(state, bolt11.as_str()),
        DesktopControlActionRequest::GetMissionControlLogTail { limit } => {
            log_tail_response(state, *limit)
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
    let buy_mode_requests =
        crate::nip90_compute_flow::buy_mode_request_flow_snapshots(
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
            payable_provider_pubkey: buy_mode_request
                .as_ref()
                .and_then(|request| request.payable_provider_pubkey.clone()),
            recent_requests: recent_request_rows,
        },
        active_job: compute_flow
            .active_job
            .map(|active_job| {
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
            state.desktop_control.last_snapshot_revision.saturating_add(1).max(1)
        };
    snapshot.snapshot_revision = next_revision;
    snapshot.state_signature = signature.clone();
    if let Some(last_command) = snapshot.last_command.as_mut() {
        last_command.snapshot_revision = next_revision;
        last_command.state_signature = signature;
    }
    snapshot
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
        return (
            false,
            Some("wallet is not connected to Spark".to_string()),
        );
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
        payable_provider_pubkey: request.payable_provider_pubkey.clone(),
        last_feedback_status: request.last_feedback_status.clone(),
        last_feedback_event_id: request.last_feedback_event_id.clone(),
        last_result_event_id: request.last_result_event_id.clone(),
        winning_result_event_id: request.winning_result_event_id.clone(),
        payment_pointer: request.payment_pointer.clone(),
        pending_bolt11: request.pending_bolt11.clone(),
        payment_notice: request.payment_notice.clone(),
        payment_error: request.payment_error.clone(),
        wallet_status: request.wallet_status.clone(),
    }
}

fn active_job_stage_label(active_job: &crate::nip90_compute_flow::ActiveJobFlowSnapshot) -> &'static str {
    match active_job.phase {
        crate::nip90_compute_flow::Nip90FlowPhase::RequestingPayment
        | crate::nip90_compute_flow::Nip90FlowPhase::AwaitingPayment => "settling",
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
        let response =
            DesktopControlActionResponse::error("Desktop control loop is unavailable");
        let _ = append_runtime_events(
            &state,
            vec![command_outcome_event(&action_for_response, &response)],
        );
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(response),
        );
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
            let response =
                DesktopControlActionResponse::error("Desktop control action timed out");
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
        DesktopControlEventBatch, DesktopControlEventDraft, DesktopControlMissionControlStatus,
        DesktopControlProviderStatus, DesktopControlRuntime, DesktopControlRuntimeConfig,
        DesktopControlRuntimeUpdate, DesktopControlSessionStatus, DesktopControlSnapshot,
        DesktopControlWalletStatus, apply_response_snapshot_metadata, snapshot_sync_signature,
        validate_control_bind_addr,
    };

    use std::time::Duration;

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
                payable_provider_pubkey: None,
                recent_requests: Vec::new(),
            },
            active_job: None,
            recent_logs: vec!["15:00:00  Provider offline.".to_string()],
            last_command: None,
        }
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
    fn action_response_metadata_uses_snapshot_revision_and_signature() {
        let snapshot = sample_snapshot();
        let response = apply_response_snapshot_metadata(
            DesktopControlActionResponse::ok("ok"),
            &snapshot,
        );

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
            .get(format!("{events_url}?after_event_id=0&limit=10&timeout_ms=0"))
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
}
