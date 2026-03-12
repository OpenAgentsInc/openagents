use std::fs;
use std::net::{IpAddr, SocketAddr};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::routing::{get, post};
use axum::{Json, Router};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use openagents_kernel_core::ids::sha256_prefixed_text;
use openagents_provider_substrate::ProviderDesiredMode;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::sync::{mpsc as tokio_mpsc, oneshot};

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
    Shutdown,
}

#[derive(Clone)]
struct DesktopControlHttpState {
    snapshot: Arc<Mutex<DesktopControlSnapshot>>,
    auth_token: Arc<Mutex<String>>,
    update_tx: Sender<DesktopControlRuntimeUpdate>,
}

pub struct DesktopControlRuntime {
    command_tx: tokio_mpsc::UnboundedSender<DesktopControlRuntimeCommand>,
    update_rx: Receiver<DesktopControlRuntimeUpdate>,
    listen_addr: SocketAddr,
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

    let runtime = DesktopControlRuntime::spawn(DesktopControlRuntimeConfig {
        listen_addr: bind_addr,
        auth_token: auth_token.clone(),
    })?;
    let listen_addr = runtime.listen_addr();
    let base_url = control_base_url(listen_addr);
    let snapshot = snapshot_for_state(state);
    runtime.sync_snapshot(snapshot.clone())?;

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
                let response = apply_action_request(state, &envelope.action);
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
    let Some(runtime) = state.desktop_control_runtime.as_ref() else {
        return false;
    };
    let snapshot = snapshot_for_state(state);
    let snapshot_revision = snapshot.snapshot_revision;
    let signature = snapshot.state_signature.clone();
    let should_sync = state.desktop_control_last_sync_signature.as_deref()
        != Some(signature.as_str())
        || state
            .desktop_control_last_sync_at
            .is_none_or(|last| last.elapsed() >= DESKTOP_CONTROL_SYNC_INTERVAL);
    if !should_sync {
        return false;
    }
    if let Err(error) = runtime.sync_snapshot(snapshot) {
        state.desktop_control.last_error = Some(error);
        return false;
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
    let envelope = DesktopControlActionEnvelope {
        action,
        response_tx,
    };
    if state
        .update_tx
        .send(DesktopControlRuntimeUpdate::ActionRequest(envelope))
        .is_err()
    {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(DesktopControlActionResponse::error(
                "Desktop control loop is unavailable",
            )),
        );
    }
    match tokio::time::timeout(Duration::from_secs(3), response_rx).await {
        Ok(Ok(response)) => (StatusCode::OK, Json(response)),
        Ok(Err(_)) => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(DesktopControlActionResponse::error(
                "Desktop dropped the control action response",
            )),
        ),
        Err(_) => (
            StatusCode::REQUEST_TIMEOUT,
            Json(DesktopControlActionResponse::error(
                "Desktop control action timed out",
            )),
        ),
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
        let auth_token = Arc::new(Mutex::new(config.auth_token));
        let state = DesktopControlHttpState {
            snapshot,
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
            .route("/v1/action", post(desktop_control_action))
            .with_state(state.clone());
        let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
        let server = tokio::spawn(async move {
            if let Err(error) = axum::serve(listener, router)
                .with_graceful_shutdown(async {
                    let _ = shutdown_rx.await;
                })
                .await
            {
                let _ = state
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
        DesktopControlMissionControlStatus, DesktopControlProviderStatus, DesktopControlRuntime,
        DesktopControlRuntimeConfig, DesktopControlRuntimeUpdate, DesktopControlSessionStatus,
        DesktopControlSnapshot, DesktopControlWalletStatus, apply_response_snapshot_metadata,
        snapshot_sync_signature,
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
}
