use std::collections::{BTreeMap, HashMap};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread::JoinHandle;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::routing::{get, post};
use axum::{Json, Router, response::Html};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use codex_client::{
    AppServerRequestId, ApprovalDecision, CommandExecutionRequestApprovalResponse,
    FileChangeRequestApprovalResponse, ToolRequestUserInputAnswer, ToolRequestUserInputResponse,
};
use openagents_kernel_core::ids::sha256_prefixed_text;
use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc as tokio_mpsc, oneshot};

use crate::app_state::{
    AutopilotChatCollaborationMode, AutopilotChatPersonality, AutopilotChatServiceTier,
    AutopilotMessageStatus, AutopilotRole, ProviderMode, RenderState,
};
use crate::nip_sa_wallet_bridge::spark_total_balance_sats;

const REMOTE_SYNC_INTERVAL: Duration = Duration::from_millis(250);
const REMOTE_WORKTREE_CACHE_TTL: Duration = Duration::from_secs(5);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CodexRemoteRuntimeConfig {
    pub listen_addr: SocketAddr,
    pub auth_token: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct CodexRemoteSnapshot {
    pub schema_version: u16,
    pub generated_at_epoch_ms: u64,
    pub threads: Vec<CodexRemoteThreadSummary>,
    pub active_thread: Option<CodexRemoteActiveThread>,
    pub approvals: Vec<CodexRemoteApprovalRequest>,
    pub tool_user_inputs: Vec<CodexRemoteToolUserInputRequest>,
    pub session: CodexRemoteSessionStatus,
    pub artifacts: CodexRemoteArtifacts,
    pub wallet: CodexRemoteWalletSummary,
    pub provider: CodexRemoteProviderSummary,
    pub workspace: Option<CodexRemoteWorkspaceContext>,
    pub terminal: Option<CodexRemoteTerminalSnapshot>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct CodexRemoteThreadSummary {
    pub thread_id: String,
    pub thread_name: Option<String>,
    pub preview: String,
    pub status: Option<String>,
    pub loaded: bool,
    pub created_at: Option<i64>,
    pub updated_at: Option<i64>,
    pub is_active: bool,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct CodexRemoteActiveThread {
    pub thread_id: String,
    pub thread_name: Option<String>,
    pub status: Option<String>,
    pub last_turn_status: Option<String>,
    pub active_turn_id: Option<String>,
    pub messages: Vec<CodexRemoteMessage>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct CodexRemoteMessage {
    pub id: u64,
    pub role: String,
    pub status: String,
    pub content: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CodexRemoteApprovalKind {
    Command,
    FileChange,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CodexRemoteApprovalDecision {
    Accept,
    AcceptForSession,
    Decline,
    Cancel,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CodexRemoteApprovalRequest {
    pub kind: CodexRemoteApprovalKind,
    pub request_id: String,
    pub thread_id: String,
    pub turn_id: String,
    pub item_id: String,
    pub reason: Option<String>,
    pub command: Option<String>,
    pub cwd: Option<String>,
    pub grant_root: Option<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct CodexRemoteToolUserInputQuestion {
    pub id: String,
    pub header: String,
    pub question: String,
    pub options: Vec<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct CodexRemoteToolUserInputRequest {
    pub request_id: String,
    pub thread_id: String,
    pub turn_id: String,
    pub item_id: String,
    pub questions: Vec<CodexRemoteToolUserInputQuestion>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct CodexRemoteSessionStatus {
    pub connection_status: String,
    pub readiness_summary: String,
    pub account_summary: String,
    pub config_constraint_summary: Option<String>,
    pub current_model: String,
    pub available_models: Vec<String>,
    pub reasoning_effort: Option<String>,
    pub supported_reasoning_efforts: Vec<String>,
    pub service_tier: String,
    pub approval_mode: String,
    pub sandbox_mode: String,
    pub personality: String,
    pub collaboration_mode: String,
    pub token_usage_summary: Option<String>,
    pub pending_auth_refresh: usize,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct CodexRemotePlanArtifact {
    pub explanation: Option<String>,
    pub steps: Vec<CodexRemotePlanStep>,
    pub updated_at_epoch_ms: u64,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct CodexRemotePlanStep {
    pub step: String,
    pub status: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct CodexRemoteDiffArtifact {
    pub summary: String,
    pub files: Vec<CodexRemoteDiffFile>,
    pub raw_diff: String,
    pub updated_at_epoch_ms: u64,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct CodexRemoteDiffFile {
    pub path: String,
    pub added_line_count: u32,
    pub removed_line_count: u32,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct CodexRemoteArtifacts {
    pub plan: Option<CodexRemotePlanArtifact>,
    pub latest_diff: Option<CodexRemoteDiffArtifact>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct CodexRemoteWalletSummary {
    pub total_sats: u64,
    pub status: String,
    pub last_error: Option<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct CodexRemoteProviderSummary {
    pub mode: String,
    pub online: bool,
    pub last_action: Option<String>,
    pub last_error: Option<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct CodexRemoteWorkspaceContext {
    pub project_id: Option<String>,
    pub project_name: Option<String>,
    pub workspace_root: Option<String>,
    pub cwd: Option<String>,
    pub path: Option<String>,
    pub git_branch: Option<String>,
    pub git_dirty: Option<bool>,
    pub worktree_entries: Vec<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct CodexRemoteTerminalSnapshot {
    pub thread_id: String,
    pub workspace_root: String,
    pub shell: String,
    pub status: String,
    pub exit_code: Option<i32>,
    pub line_count: usize,
    pub lines: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum CodexRemoteActionRequest {
    SelectThread {
        thread_id: String,
    },
    SubmitPrompt {
        prompt: String,
    },
    InterruptTurn,
    RespondApproval {
        kind: CodexRemoteApprovalKind,
        request_id: String,
        decision: CodexRemoteApprovalDecision,
    },
    RespondToolUserInput {
        request_id: String,
        answers: BTreeMap<String, Vec<String>>,
    },
    UpdateSession {
        model: Option<String>,
        reasoning_effort: Option<String>,
        service_tier: Option<String>,
        approval_mode: Option<String>,
        sandbox_mode: Option<String>,
        personality: Option<String>,
        collaboration_mode: Option<String>,
    },
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct CodexRemoteActionResponse {
    pub success: bool,
    pub message: String,
}

impl CodexRemoteActionResponse {
    fn ok(message: impl Into<String>) -> Self {
        Self {
            success: true,
            message: message.into(),
        }
    }

    fn error(message: impl Into<String>) -> Self {
        Self {
            success: false,
            message: message.into(),
        }
    }
}

#[derive(Debug)]
pub struct CodexRemoteActionEnvelope {
    pub action: CodexRemoteActionRequest,
    response_tx: oneshot::Sender<CodexRemoteActionResponse>,
}

impl CodexRemoteActionEnvelope {
    pub fn respond(self, response: CodexRemoteActionResponse) {
        let _ = self.response_tx.send(response);
    }
}

#[derive(Debug)]
pub enum CodexRemoteRuntimeUpdate {
    ActionRequest(CodexRemoteActionEnvelope),
    WorkerError(String),
}

enum CodexRemoteRuntimeCommand {
    SyncSnapshot(Box<CodexRemoteSnapshot>),
    RotateToken(String),
    Shutdown,
}

#[derive(Clone)]
struct CodexRemoteHttpState {
    snapshot: Arc<Mutex<CodexRemoteSnapshot>>,
    auth_token: Arc<Mutex<String>>,
    update_tx: Sender<CodexRemoteRuntimeUpdate>,
}

pub struct DesktopCodexRemoteRuntime {
    command_tx: tokio_mpsc::UnboundedSender<CodexRemoteRuntimeCommand>,
    update_rx: Receiver<CodexRemoteRuntimeUpdate>,
    listen_addr: SocketAddr,
    join_handle: Option<JoinHandle<()>>,
}

impl DesktopCodexRemoteRuntime {
    pub fn spawn(config: CodexRemoteRuntimeConfig) -> Result<Self, String> {
        let (command_tx, command_rx) = tokio_mpsc::unbounded_channel();
        let (update_tx, update_rx) = mpsc::channel::<CodexRemoteRuntimeUpdate>();
        let (ready_tx, ready_rx) = mpsc::channel::<Result<SocketAddr, String>>();
        let join_handle = std::thread::spawn(move || {
            run_remote_runtime_loop(command_rx, update_tx, ready_tx, config);
        });
        let listen_addr = ready_rx
            .recv()
            .map_err(|error| format!("Remote runtime failed to report readiness: {error}"))??;
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

    pub fn sync_snapshot(&self, snapshot: CodexRemoteSnapshot) -> Result<(), String> {
        self.command_tx
            .send(CodexRemoteRuntimeCommand::SyncSnapshot(Box::new(snapshot)))
            .map_err(|error| format!("Remote runtime offline: {error}"))
    }

    pub fn rotate_token(&self, auth_token: String) -> Result<(), String> {
        self.command_tx
            .send(CodexRemoteRuntimeCommand::RotateToken(auth_token))
            .map_err(|error| format!("Remote runtime offline: {error}"))
    }

    pub fn drain_updates(&mut self) -> Vec<CodexRemoteRuntimeUpdate> {
        let mut updates = Vec::new();
        while let Ok(update) = self.update_rx.try_recv() {
            updates.push(update);
        }
        updates
    }

    pub fn shutdown_async(&mut self) {
        let _ = self.command_tx.send(CodexRemoteRuntimeCommand::Shutdown);
        if let Some(handle) = self.join_handle.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for DesktopCodexRemoteRuntime {
    fn drop(&mut self) {
        self.shutdown_async();
    }
}

pub fn validate_remote_bind_addr(raw: &str) -> Result<SocketAddr, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("Remote bind address cannot be empty".to_string());
    }
    let listen_addr = trimmed
        .parse::<SocketAddr>()
        .map_err(|error| format!("Invalid remote bind address `{trimmed}`: {error}"))?;
    if !allowed_remote_bind_ip(listen_addr.ip()) {
        return Err(format!(
            "Remote bind address `{trimmed}` must be loopback, RFC1918 LAN, Tailnet CGNAT, or IPv6 ULA"
        ));
    }
    Ok(listen_addr)
}

pub fn generate_remote_auth_token() -> Result<String, String> {
    let mut bytes = [0_u8; 24];
    getrandom::fill(&mut bytes).map_err(|error| format!("Failed to generate auth token: {error}"))?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}

pub fn remote_base_url(listen_addr: SocketAddr) -> String {
    format!("http://{listen_addr}/")
}

pub fn remote_pairing_url(listen_addr: SocketAddr, auth_token: &str) -> String {
    format!("http://{listen_addr}/#token={auth_token}")
}

pub fn enable_remote_runtime(
    state: &mut RenderState,
    requested_bind_addr: Option<&str>,
) -> Result<String, String> {
    let bind_addr = validate_remote_bind_addr(
        requested_bind_addr.unwrap_or(state.codex_remote.requested_bind_addr.as_str()),
    )?;
    let auth_token = generate_remote_auth_token()?;
    disable_remote_runtime(state);
    let runtime = DesktopCodexRemoteRuntime::spawn(CodexRemoteRuntimeConfig {
        listen_addr: bind_addr,
        auth_token: auth_token.clone(),
    })?;
    let listen_addr = runtime.listen_addr();
    let base_url = remote_base_url(listen_addr);
    let pairing_url = remote_pairing_url(listen_addr, &auth_token);
    runtime.sync_snapshot(snapshot_for_state(state))?;
    state.codex_remote.enabled = true;
    state.codex_remote.requested_bind_addr = bind_addr.to_string();
    state.codex_remote.listen_addr = Some(listen_addr.to_string());
    state.codex_remote.base_url = Some(base_url.clone());
    state.codex_remote.pairing_url = Some(pairing_url.clone());
    state.codex_remote.auth_token_preview = Some(auth_token_preview(&auth_token));
    state.codex_remote.last_error = None;
    state.codex_remote.last_action = Some(format!("Remote companion listening on {listen_addr}"));
    state.codex_remote_runtime = Some(runtime);
    state.codex_remote_last_sync_signature = None;
    state.codex_remote_last_sync_at = None;
    Ok(format!(
        "Remote companion enabled on {listen_addr}. URL: {base_url} token={}",
        auth_token_preview(&auth_token)
    ))
}

pub fn disable_remote_runtime(state: &mut RenderState) -> String {
    if let Some(mut runtime) = state.codex_remote_runtime.take() {
        runtime.shutdown_async();
    }
    state.codex_remote.enabled = false;
    state.codex_remote.listen_addr = None;
    state.codex_remote.base_url = None;
    state.codex_remote.pairing_url = None;
    state.codex_remote.auth_token_preview = None;
    state.codex_remote.last_error = None;
    state.codex_remote.last_action = Some("Remote companion disabled".to_string());
    state.codex_remote_last_sync_signature = None;
    state.codex_remote_last_sync_at = None;
    "Remote companion disabled".to_string()
}

pub fn rotate_remote_runtime_token(state: &mut RenderState) -> Result<String, String> {
    let Some(runtime) = state.codex_remote_runtime.as_ref() else {
        return Err("Remote companion is not enabled".to_string());
    };
    let auth_token = generate_remote_auth_token()?;
    runtime.rotate_token(auth_token.clone())?;
    let Some(listen_addr) = state.codex_remote.listen_addr.as_deref() else {
        return Err("Remote companion is missing its listen address".to_string());
    };
    let listen_addr = listen_addr
        .parse::<SocketAddr>()
        .map_err(|error| format!("Invalid remote listen address state: {error}"))?;
    let pairing_url = remote_pairing_url(listen_addr, &auth_token);
    state.codex_remote.pairing_url = Some(pairing_url.clone());
    state.codex_remote.auth_token_preview = Some(auth_token_preview(&auth_token));
    state.codex_remote.last_error = None;
    state.codex_remote.last_action = Some("Remote auth token rotated".to_string());
    Ok(format!(
        "Remote auth token rotated for {} token={}",
        state.codex_remote
            .listen_addr
            .as_deref()
            .unwrap_or("n/a"),
        auth_token_preview(&auth_token)
    ))
}

pub fn remote_status_lines(state: &RenderState) -> Vec<String> {
    let mut lines = vec![if state.codex_remote.enabled {
        format!(
            "Remote: enabled bind={} token={}",
            state.codex_remote
                .listen_addr
                .as_deref()
                .unwrap_or("n/a"),
            state.codex_remote
                .auth_token_preview
                .as_deref()
                .unwrap_or("n/a")
        )
    } else {
        format!(
            "Remote: disabled (requested bind {}).",
            state.codex_remote.requested_bind_addr
        )
    }];
    if let Some(base_url) = state.codex_remote.base_url.as_deref() {
        lines.push(format!("URL: {base_url}"));
    }
    if let Some(token_preview) = state.codex_remote.auth_token_preview.as_deref() {
        lines.push(format!("Pairing token: {token_preview}"));
    }
    if let Some(error) = state.codex_remote.last_error.as_deref() {
        lines.push(format!("Error: {error}"));
    } else if let Some(action) = state.codex_remote.last_action.as_deref() {
        lines.push(format!("Status: {action}"));
    }
    lines
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
    let updates = match state.codex_remote_runtime.as_mut() {
        Some(runtime) => runtime.drain_updates(),
        None => return false,
    };
    let mut changed = false;
    for update in updates {
        match update {
            CodexRemoteRuntimeUpdate::ActionRequest(envelope) => {
                let response = apply_remote_action(state, &envelope.action);
                envelope.respond(response);
                changed = true;
            }
            CodexRemoteRuntimeUpdate::WorkerError(error) => {
                state.codex_remote.last_error = Some(error);
                changed = true;
            }
        }
    }
    changed
}

fn sync_runtime_snapshot(state: &mut RenderState) -> bool {
    let Some(runtime) = state.codex_remote_runtime.as_ref() else {
        return false;
    };
    let snapshot = snapshot_for_state(state);
    let signature = match serde_json::to_string(&snapshot) {
        Ok(json) => sha256_prefixed_text(json.as_str()),
        Err(error) => {
            state.codex_remote.last_error = Some(format!("Failed to encode remote snapshot: {error}"));
            return false;
        }
    };
    let should_sync = state.codex_remote_last_sync_signature.as_deref() != Some(signature.as_str())
        || state
            .codex_remote_last_sync_at
            .is_none_or(|last| last.elapsed() >= REMOTE_SYNC_INTERVAL);
    if !should_sync {
        return false;
    }
    if let Err(error) = runtime.sync_snapshot(snapshot) {
        state.codex_remote.last_error = Some(error);
        return false;
    }
    state.codex_remote_last_sync_signature = Some(signature);
    state.codex_remote_last_sync_at = Some(Instant::now());
    state.codex_remote.last_error = None;
    false
}

fn apply_remote_action(
    state: &mut RenderState,
    action: &CodexRemoteActionRequest,
) -> CodexRemoteActionResponse {
    match action {
        CodexRemoteActionRequest::SelectThread { thread_id } => {
            match crate::input::remote_select_codex_thread(state, thread_id.as_str()) {
                Ok(()) => CodexRemoteActionResponse::ok(format!("Selected thread {thread_id}")),
                Err(error) => CodexRemoteActionResponse::error(error),
            }
        }
        CodexRemoteActionRequest::SubmitPrompt { prompt } => {
            match crate::input::remote_submit_codex_prompt(state, prompt.clone()) {
                Ok(()) => CodexRemoteActionResponse::ok("Submitted remote prompt"),
                Err(error) => CodexRemoteActionResponse::error(error),
            }
        }
        CodexRemoteActionRequest::InterruptTurn => {
            match crate::input::remote_interrupt_codex_turn(state) {
                Ok(()) => CodexRemoteActionResponse::ok("Interrupt requested"),
                Err(error) => CodexRemoteActionResponse::error(error),
            }
        }
        CodexRemoteActionRequest::RespondApproval {
            kind,
            request_id,
            decision,
        } => match respond_approval(state, *kind, request_id.as_str(), *decision) {
            Ok(message) => CodexRemoteActionResponse::ok(message),
            Err(error) => CodexRemoteActionResponse::error(error),
        },
        CodexRemoteActionRequest::RespondToolUserInput { request_id, answers } => {
            match respond_tool_user_input(state, request_id.as_str(), answers) {
                Ok(message) => CodexRemoteActionResponse::ok(message),
                Err(error) => CodexRemoteActionResponse::error(error),
            }
        }
        CodexRemoteActionRequest::UpdateSession {
            model,
            reasoning_effort,
            service_tier,
            approval_mode,
            sandbox_mode,
            personality,
            collaboration_mode,
        } => match update_session_controls(
            state,
            model.as_deref(),
            reasoning_effort.as_deref(),
            service_tier.as_deref(),
            approval_mode.as_deref(),
            sandbox_mode.as_deref(),
            personality.as_deref(),
            collaboration_mode.as_deref(),
        ) {
            Ok(message) => CodexRemoteActionResponse::ok(message),
            Err(error) => CodexRemoteActionResponse::error(error),
        },
    }
}

fn respond_approval(
    state: &mut RenderState,
    kind: CodexRemoteApprovalKind,
    request_id: &str,
    decision: CodexRemoteApprovalDecision,
) -> Result<String, String> {
    match kind {
        CodexRemoteApprovalKind::Command => {
            let Some(index) = state
                .autopilot_chat
                .pending_command_approvals
                .iter()
                .position(|request| remote_request_id_value(&request.request_id) == request_id)
            else {
                return Err(format!("No pending command approval matched `{request_id}`"));
            };
            let request = state.autopilot_chat.pending_command_approvals[index].clone();
            state.queue_codex_command(
                crate::codex_lane::CodexLaneCommand::ServerRequestCommandApprovalRespond {
                    request_id: request.request_id.clone(),
                    response: CommandExecutionRequestApprovalResponse {
                        decision: approval_decision(decision),
                    },
                },
            )?;
            state.autopilot_chat.pending_command_approvals.remove(index);
            state.autopilot_chat.record_turn_command_approval_response(
                request.turn_id.as_str(),
                request.item_id.as_str(),
                remote_approval_decision_label(decision),
                current_epoch_ms(),
            );
            state
                .autopilot_chat
                .record_turn_timeline_event(format!("remote command approval: {}", remote_approval_decision_label(decision)));
            Ok(format!("Submitted command approval {}", remote_approval_decision_label(decision)))
        }
        CodexRemoteApprovalKind::FileChange => {
            let Some(index) = state
                .autopilot_chat
                .pending_file_change_approvals
                .iter()
                .position(|request| remote_request_id_value(&request.request_id) == request_id)
            else {
                return Err(format!("No pending file approval matched `{request_id}`"));
            };
            let request = state.autopilot_chat.pending_file_change_approvals[index].clone();
            state.queue_codex_command(
                crate::codex_lane::CodexLaneCommand::ServerRequestFileApprovalRespond {
                    request_id: request.request_id.clone(),
                    response: FileChangeRequestApprovalResponse {
                        decision: approval_decision(decision),
                    },
                },
            )?;
            state.autopilot_chat.pending_file_change_approvals.remove(index);
            state.autopilot_chat.record_turn_file_change_approval_response(
                request.turn_id.as_str(),
                request.item_id.as_str(),
                remote_approval_decision_label(decision),
                current_epoch_ms(),
            );
            state
                .autopilot_chat
                .record_turn_timeline_event(format!("remote file approval: {}", remote_approval_decision_label(decision)));
            Ok(format!("Submitted file approval {}", remote_approval_decision_label(decision)))
        }
    }
}

fn respond_tool_user_input(
    state: &mut RenderState,
    request_id: &str,
    answers: &BTreeMap<String, Vec<String>>,
) -> Result<String, String> {
    let Some(index) = state
        .autopilot_chat
        .pending_tool_user_input
        .iter()
        .position(|request| remote_request_id_value(&request.request_id) == request_id)
    else {
        return Err(format!("No pending tool prompt matched `{request_id}`"));
    };
    let request = state.autopilot_chat.pending_tool_user_input[index].clone();
    let mut response_answers = HashMap::new();
    for question in &request.questions {
        let selected = answers
            .get(&question.id)
            .cloned()
            .unwrap_or_else(|| {
                question
                    .options
                    .first()
                    .cloned()
                    .map(|value| vec![value])
                    .unwrap_or_else(|| vec!["ok".to_string()])
            });
        response_answers.insert(question.id.clone(), ToolRequestUserInputAnswer { answers: selected });
    }
    state.queue_codex_command(
        crate::codex_lane::CodexLaneCommand::ServerRequestToolUserInputRespond {
            request_id: request.request_id.clone(),
            response: ToolRequestUserInputResponse {
                answers: response_answers,
            },
        },
    )?;
    state.autopilot_chat.pending_tool_user_input.remove(index);
    state
        .autopilot_chat
        .record_turn_timeline_event("remote tool user-input response submitted");
    Ok("Submitted tool user-input response".to_string())
}

fn update_session_controls(
    state: &mut RenderState,
    model: Option<&str>,
    reasoning_effort: Option<&str>,
    service_tier: Option<&str>,
    approval_mode: Option<&str>,
    sandbox_mode: Option<&str>,
    personality: Option<&str>,
    collaboration_mode: Option<&str>,
) -> Result<String, String> {
    if let Some(model) = model.map(str::trim).filter(|value| !value.is_empty()) {
        state.autopilot_chat.select_or_insert_model(model);
    }
    if let Some(reasoning_effort) = reasoning_effort {
        state
            .autopilot_chat
            .set_reasoning_effort(Some(reasoning_effort.to_string()));
    }
    if let Some(service_tier) = service_tier {
        state
            .autopilot_chat
            .set_service_tier(parse_service_tier(service_tier)?);
    }
    if let Some(approval_mode) = approval_mode {
        state
            .autopilot_chat
            .set_approval_mode(parse_approval_mode(approval_mode)?);
    }
    if let Some(sandbox_mode) = sandbox_mode {
        state
            .autopilot_chat
            .set_sandbox_mode(parse_sandbox_mode(sandbox_mode)?);
    }
    if let Some(personality) = personality {
        state
            .autopilot_chat
            .set_personality(parse_personality(personality)?);
    }
    if let Some(collaboration_mode) = collaboration_mode {
        state
            .autopilot_chat
            .set_collaboration_mode(parse_collaboration_mode(collaboration_mode)?);
    }
    Ok("Updated remote session controls".to_string())
}

pub fn snapshot_for_state(state: &RenderState) -> CodexRemoteSnapshot {
    let active_thread_id = state.autopilot_chat.active_thread_id.as_deref();
    let threads = state
        .autopilot_chat
        .threads
        .iter()
        .filter_map(|thread_id| {
            let metadata = state.autopilot_chat.thread_metadata.get(thread_id)?;
            Some(CodexRemoteThreadSummary {
                thread_id: thread_id.clone(),
                thread_name: metadata.thread_name.clone(),
                preview: metadata.preview.clone().unwrap_or_default(),
                status: metadata.status.clone(),
                loaded: metadata.loaded,
                created_at: metadata.created_at,
                updated_at: metadata.updated_at,
                is_active: active_thread_id == Some(thread_id.as_str()),
            })
        })
        .collect::<Vec<_>>();

    let active_thread = active_thread_id.and_then(|thread_id| {
        let metadata = state.autopilot_chat.thread_metadata.get(thread_id)?;
        Some(CodexRemoteActiveThread {
            thread_id: thread_id.to_string(),
            thread_name: metadata.thread_name.clone(),
            status: metadata.status.clone(),
            last_turn_status: state.autopilot_chat.last_turn_status.clone(),
            active_turn_id: state.autopilot_chat.active_turn_id.clone(),
            messages: state
                .autopilot_chat
                .messages
                .iter()
                .map(|message| CodexRemoteMessage {
                    id: message.id,
                    role: remote_role_label(message.role).to_string(),
                    status: remote_message_status_label(message.status).to_string(),
                    content: if message.content.trim().is_empty() {
                        message
                            .structured
                            .as_ref()
                            .map(remote_rendered_structured_content)
                            .unwrap_or_default()
                    } else {
                        message.content.clone()
                    },
                })
                .collect(),
        })
    });

    let approvals = state
        .autopilot_chat
        .pending_command_approvals
        .iter()
        .map(|request| CodexRemoteApprovalRequest {
            kind: CodexRemoteApprovalKind::Command,
            request_id: remote_request_id_value(&request.request_id),
            thread_id: request.thread_id.clone(),
            turn_id: request.turn_id.clone(),
            item_id: request.item_id.clone(),
            reason: request.reason.clone(),
            command: request.command.clone(),
            cwd: request.cwd.clone(),
            grant_root: None,
        })
        .chain(
            state
                .autopilot_chat
                .pending_file_change_approvals
                .iter()
                .map(|request| CodexRemoteApprovalRequest {
                    kind: CodexRemoteApprovalKind::FileChange,
                    request_id: remote_request_id_value(&request.request_id),
                    thread_id: request.thread_id.clone(),
                    turn_id: request.turn_id.clone(),
                    item_id: request.item_id.clone(),
                    reason: request.reason.clone(),
                    command: None,
                    cwd: None,
                    grant_root: request.grant_root.clone(),
                }),
        )
        .collect();

    let tool_user_inputs = state
        .autopilot_chat
        .pending_tool_user_input
        .iter()
        .map(|request| CodexRemoteToolUserInputRequest {
            request_id: remote_request_id_value(&request.request_id),
            thread_id: request.thread_id.clone(),
            turn_id: request.turn_id.clone(),
            item_id: request.item_id.clone(),
            questions: request
                .questions
                .iter()
                .map(|question| CodexRemoteToolUserInputQuestion {
                    id: question.id.clone(),
                    header: question.header.clone(),
                    question: question.question.clone(),
                    options: question.options.clone(),
                })
                .collect(),
        })
        .collect();

    let supported_reasoning_efforts = state
        .codex_models
        .entries
        .iter()
        .find(|entry| entry.model == state.autopilot_chat.current_model())
        .map(|entry| entry.supported_reasoning_efforts.clone())
        .unwrap_or_default();

    let token_usage_summary = state.autopilot_chat.token_usage.as_ref().map(|usage| {
        format!(
            "input={} cached={} output={}",
            usage.input_tokens, usage.cached_input_tokens, usage.output_tokens
        )
    });

    let session = CodexRemoteSessionStatus {
        connection_status: state.autopilot_chat.connection_status.clone(),
        readiness_summary: state.codex_account.readiness_summary.clone(),
        account_summary: state.codex_account.account_summary.clone(),
        config_constraint_summary: state.codex_account.config_constraint_summary.clone(),
        current_model: state.autopilot_chat.current_model().to_string(),
        available_models: state.autopilot_chat.models.clone(),
        reasoning_effort: state.autopilot_chat.reasoning_effort.clone(),
        supported_reasoning_efforts,
        service_tier: state.autopilot_chat.service_tier.label().to_string(),
        approval_mode: remote_approval_mode_label(state.autopilot_chat.approval_mode).to_string(),
        sandbox_mode: remote_sandbox_mode_label(state.autopilot_chat.sandbox_mode).to_string(),
        personality: state.autopilot_chat.personality.label().to_string(),
        collaboration_mode: state.autopilot_chat.collaboration_mode.label().to_string(),
        token_usage_summary,
        pending_auth_refresh: state.autopilot_chat.pending_auth_refresh.len(),
    };

    let plan = state
        .autopilot_chat
        .active_plan_artifact()
        .map(|artifact| CodexRemotePlanArtifact {
            explanation: artifact.explanation.clone(),
            steps: artifact
                .steps
                .iter()
                .map(|step| CodexRemotePlanStep {
                    step: step.step.clone(),
                    status: step.status.clone(),
                })
                .collect(),
            updated_at_epoch_ms: artifact.updated_at_epoch_ms,
        });

    let latest_diff = state
        .autopilot_chat
        .active_diff_artifact()
        .map(|artifact| CodexRemoteDiffArtifact {
            summary: format!(
                "{} file(s), +{}, -{}",
                artifact.files.len(),
                artifact.added_line_count,
                artifact.removed_line_count
            ),
            files: artifact
                .files
                .iter()
                .map(|file| CodexRemoteDiffFile {
                    path: file.path.clone(),
                    added_line_count: file.added_line_count,
                    removed_line_count: file.removed_line_count,
                })
                .collect(),
            raw_diff: artifact.raw_diff.clone(),
            updated_at_epoch_ms: artifact.updated_at_epoch_ms,
        });

    let wallet = CodexRemoteWalletSummary {
        total_sats: state
            .spark_wallet
            .balance
            .as_ref()
            .map(spark_total_balance_sats)
            .unwrap_or(0),
        status: state.spark_wallet.network_status_label().to_string(),
        last_error: state.spark_wallet.last_error.clone(),
    };

    let provider = CodexRemoteProviderSummary {
        mode: remote_provider_mode_label(state.provider_runtime.mode).to_string(),
        online: !matches!(state.provider_runtime.mode, ProviderMode::Offline),
        last_action: state
            .provider_runtime
            .last_result
            .clone()
            .or_else(|| state.provider_runtime.inventory_last_action.clone()),
        last_error: state.provider_runtime.last_error_detail.clone(),
    };

    CodexRemoteSnapshot {
        schema_version: 1,
        generated_at_epoch_ms: current_epoch_ms(),
        threads,
        active_thread,
        approvals,
        tool_user_inputs,
        session,
        artifacts: CodexRemoteArtifacts { plan, latest_diff },
        wallet,
        provider,
        workspace: workspace_context_for_state(state),
        terminal: terminal_snapshot_for_state(state),
    }
}

fn workspace_context_for_state(state: &RenderState) -> Option<CodexRemoteWorkspaceContext> {
    let thread_id = state.autopilot_chat.active_thread_id.as_deref()?;
    let metadata = state.autopilot_chat.thread_metadata.get(thread_id)?;
    let workspace_root = metadata.workspace_root.clone();
    let worktree_entries = workspace_root
        .as_deref()
        .map(cached_git_worktree_entries)
        .unwrap_or_default();
    Some(CodexRemoteWorkspaceContext {
        project_id: metadata.project_id.clone(),
        project_name: metadata.project_name.clone(),
        workspace_root,
        cwd: metadata.cwd.clone(),
        path: metadata.path.clone(),
        git_branch: metadata.git_branch.clone(),
        git_dirty: metadata.git_dirty,
        worktree_entries,
    })
}

fn terminal_snapshot_for_state(state: &RenderState) -> Option<CodexRemoteTerminalSnapshot> {
    let session = state.autopilot_chat.active_terminal_session()?;
    let lines = session
        .lines
        .iter()
        .rev()
        .take(120)
        .map(|line| {
            let prefix = match line.stream {
                wgpui::components::sections::TerminalStream::Stdout => "stdout",
                wgpui::components::sections::TerminalStream::Stderr => "stderr",
            };
            format!("[{prefix}] {}", line.text)
        })
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<Vec<_>>();
    Some(CodexRemoteTerminalSnapshot {
        thread_id: session.thread_id.clone(),
        workspace_root: session.workspace_root.clone(),
        shell: session.shell.clone(),
        status: session.status.label().to_string(),
        exit_code: session.exit_code,
        line_count: session.lines.len(),
        lines,
    })
}

fn run_remote_runtime_loop(
    mut command_rx: tokio_mpsc::UnboundedReceiver<CodexRemoteRuntimeCommand>,
    update_tx: Sender<CodexRemoteRuntimeUpdate>,
    ready_tx: Sender<Result<SocketAddr, String>>,
    config: CodexRemoteRuntimeConfig,
) {
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            let _ = ready_tx.send(Err(format!("Failed to build remote runtime: {error}")));
            return;
        }
    };

    runtime.block_on(async move {
        let snapshot = Arc::new(Mutex::new(CodexRemoteSnapshot::default()));
        let auth_token = Arc::new(Mutex::new(config.auth_token));
        let state = CodexRemoteHttpState {
            snapshot,
            auth_token,
            update_tx,
        };
        let listener = match tokio::net::TcpListener::bind(config.listen_addr).await {
            Ok(listener) => listener,
            Err(error) => {
                let _ = ready_tx.send(Err(format!("Failed to bind remote listener: {error}")));
                return;
            }
        };
        let listen_addr = match listener.local_addr() {
            Ok(addr) => addr,
            Err(error) => {
                let _ = ready_tx.send(Err(format!("Failed to resolve remote listener address: {error}")));
                return;
            }
        };
        let router = Router::new()
            .route("/", get(remote_index))
            .route("/v1/snapshot", get(remote_snapshot))
            .route("/v1/action", post(remote_action))
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
                    .send(CodexRemoteRuntimeUpdate::WorkerError(format!(
                        "Remote listener failed: {error}"
                    )));
            }
        });
        let _ = ready_tx.send(Ok(listen_addr));
        while let Some(command) = command_rx.recv().await {
            match command {
                CodexRemoteRuntimeCommand::SyncSnapshot(next_snapshot) => {
                    if let Ok(mut guard) = state.snapshot.lock() {
                        *guard = *next_snapshot;
                    }
                }
                CodexRemoteRuntimeCommand::RotateToken(next_token) => {
                    if let Ok(mut guard) = state.auth_token.lock() {
                        *guard = next_token;
                    }
                }
                CodexRemoteRuntimeCommand::Shutdown => break,
            }
        }
        let _ = shutdown_tx.send(());
        let _ = server.await;
    });
}

fn cached_git_worktree_entries(workspace_root: &str) -> Vec<String> {
    static CACHE: OnceLock<Mutex<HashMap<String, (Instant, Vec<String>)>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(guard) = cache.lock()
        && let Some((captured_at, entries)) = guard.get(workspace_root)
        && captured_at.elapsed() < REMOTE_WORKTREE_CACHE_TTL
    {
        return entries.clone();
    }
    let entries = git_worktree_entries(workspace_root);
    if let Ok(mut guard) = cache.lock() {
        guard.insert(workspace_root.to_string(), (Instant::now(), entries.clone()));
    }
    entries
}

fn git_worktree_entries(workspace_root: &str) -> Vec<String> {
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(workspace_root)
        .args(["worktree", "list", "--porcelain"])
        .output();
    let Ok(output) = output else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }
    parse_git_worktree_output(String::from_utf8_lossy(&output.stdout).as_ref())
}

fn parse_git_worktree_output(raw: &str) -> Vec<String> {
    let mut entries = Vec::new();
    let mut path = None::<String>;
    let mut branch = None::<String>;
    let mut detached = false;
    let mut locked = false;

    let flush = |entries: &mut Vec<String>,
                 path: &mut Option<String>,
                 branch: &mut Option<String>,
                 detached: &mut bool,
                 locked: &mut bool| {
        let Some(current_path) = path.take() else {
            *branch = None;
            *detached = false;
            *locked = false;
            return;
        };
        let branch_label = if let Some(branch) = branch.take() {
            branch
        } else if *detached {
            "detached".to_string()
        } else {
            "unknown".to_string()
        };
        let mut summary = format!("{current_path} ({branch_label})");
        if *locked {
            summary.push_str(" locked");
        }
        entries.push(summary);
        *detached = false;
        *locked = false;
    };

    for line in raw.lines() {
        if let Some(next_path) = line.strip_prefix("worktree ") {
            flush(
                &mut entries,
                &mut path,
                &mut branch,
                &mut detached,
                &mut locked,
            );
            path = Some(next_path.trim().to_string());
            continue;
        }
        if let Some(next_branch) = line.strip_prefix("branch ") {
            branch = Some(
                next_branch
                    .trim()
                    .trim_start_matches("refs/heads/")
                    .to_string(),
            );
            continue;
        }
        if line.trim() == "detached" {
            detached = true;
            continue;
        }
        if line.starts_with("locked") {
            locked = true;
            continue;
        }
        if line.trim().is_empty() {
            flush(
                &mut entries,
                &mut path,
                &mut branch,
                &mut detached,
                &mut locked,
            );
        }
    }
    flush(
        &mut entries,
        &mut path,
        &mut branch,
        &mut detached,
        &mut locked,
    );
    entries
}

async fn remote_index() -> Html<&'static str> {
    Html(REMOTE_HTML)
}

async fn remote_snapshot(
    State(state): State<CodexRemoteHttpState>,
    headers: HeaderMap,
) -> Result<Json<CodexRemoteSnapshot>, StatusCode> {
    authorize_request(&headers, &state)?;
    let snapshot = state
        .snapshot
        .lock()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .clone();
    Ok(Json(snapshot))
}

async fn remote_action(
    State(state): State<CodexRemoteHttpState>,
    headers: HeaderMap,
    Json(action): Json<CodexRemoteActionRequest>,
) -> (StatusCode, Json<CodexRemoteActionResponse>) {
    if let Err(status) = authorize_request(&headers, &state) {
        return (
            status,
            Json(CodexRemoteActionResponse::error("Unauthorized remote request")),
        );
    }
    let (response_tx, response_rx) = oneshot::channel();
    let envelope = CodexRemoteActionEnvelope { action, response_tx };
    if state
        .update_tx
        .send(CodexRemoteRuntimeUpdate::ActionRequest(envelope))
        .is_err()
    {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(CodexRemoteActionResponse::error("Desktop remote loop is unavailable")),
        );
    }
    match tokio::time::timeout(Duration::from_secs(3), response_rx).await {
        Ok(Ok(response)) => (StatusCode::OK, Json(response)),
        Ok(Err(_)) => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(CodexRemoteActionResponse::error("Desktop dropped the remote action response")),
        ),
        Err(_) => (
            StatusCode::REQUEST_TIMEOUT,
            Json(CodexRemoteActionResponse::error("Desktop remote action timed out")),
        ),
    }
}

fn authorize_request(headers: &HeaderMap, state: &CodexRemoteHttpState) -> Result<(), StatusCode> {
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

fn allowed_remote_bind_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => v4.is_loopback() || v4.is_private() || shared_cgnat(v4),
        IpAddr::V6(v6) => v6.is_loopback() || unique_local_ipv6(v6),
    }
}

fn shared_cgnat(ip: Ipv4Addr) -> bool {
    let octets = ip.octets();
    octets[0] == 100 && (64..=127).contains(&octets[1])
}

fn unique_local_ipv6(ip: Ipv6Addr) -> bool {
    (ip.segments()[0] & 0xfe00) == 0xfc00
}

fn auth_token_preview(auth_token: &str) -> String {
    let trimmed = auth_token.trim();
    if trimmed.len() <= 10 {
        trimmed.to_string()
    } else {
        format!("{}...{}", &trimmed[..4], &trimmed[trimmed.len().saturating_sub(4)..])
    }
}

fn remote_request_id_value(request_id: &AppServerRequestId) -> String {
    match request_id {
        AppServerRequestId::String(value) => value.clone(),
        AppServerRequestId::Integer(value) => value.to_string(),
    }
}

fn approval_decision(decision: CodexRemoteApprovalDecision) -> ApprovalDecision {
    match decision {
        CodexRemoteApprovalDecision::Accept => ApprovalDecision::Accept,
        CodexRemoteApprovalDecision::AcceptForSession => ApprovalDecision::AcceptForSession,
        CodexRemoteApprovalDecision::Decline => ApprovalDecision::Decline,
        CodexRemoteApprovalDecision::Cancel => ApprovalDecision::Cancel,
    }
}

fn remote_approval_decision_label(decision: CodexRemoteApprovalDecision) -> &'static str {
    match decision {
        CodexRemoteApprovalDecision::Accept => "accept",
        CodexRemoteApprovalDecision::AcceptForSession => "accept-for-session",
        CodexRemoteApprovalDecision::Decline => "decline",
        CodexRemoteApprovalDecision::Cancel => "cancel",
    }
}

fn remote_provider_mode_label(mode: ProviderMode) -> &'static str {
    match mode {
        ProviderMode::Offline => "offline",
        ProviderMode::Connecting => "connecting",
        ProviderMode::Online => "online",
        ProviderMode::Degraded => "degraded",
    }
}

fn remote_message_status_label(status: AutopilotMessageStatus) -> &'static str {
    match status {
        AutopilotMessageStatus::Queued => "queued",
        AutopilotMessageStatus::Running => "running",
        AutopilotMessageStatus::Done => "done",
        AutopilotMessageStatus::Error => "error",
    }
}

fn remote_role_label(role: AutopilotRole) -> &'static str {
    match role {
        AutopilotRole::User => "user",
        AutopilotRole::Codex => "codex",
    }
}

fn remote_rendered_structured_content(
    structured: &crate::app_state::AutopilotStructuredMessage,
) -> String {
    let reasoning = structured.reasoning.trim_end();
    let answer = structured.answer.trim_end();
    if reasoning.is_empty() {
        answer.to_string()
    } else if answer.is_empty() {
        reasoning.to_string()
    } else {
        format!("{reasoning}\n\n{answer}")
    }
}

fn remote_approval_mode_label(mode: codex_client::AskForApproval) -> &'static str {
    match mode {
        codex_client::AskForApproval::Never => "never",
        codex_client::AskForApproval::OnFailure => "on-failure",
        codex_client::AskForApproval::OnRequest => "on-request",
        codex_client::AskForApproval::UnlessTrusted => "unless-trusted",
        codex_client::AskForApproval::Reject { .. } => "reject",
    }
}

fn remote_sandbox_mode_label(mode: codex_client::SandboxMode) -> &'static str {
    match mode {
        codex_client::SandboxMode::DangerFullAccess => "danger-full-access",
        codex_client::SandboxMode::WorkspaceWrite => "workspace-write",
        codex_client::SandboxMode::ReadOnly => "read-only",
    }
}

fn parse_service_tier(raw: &str) -> Result<AutopilotChatServiceTier, String> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "default" => Ok(AutopilotChatServiceTier::Default),
        "fast" => Ok(AutopilotChatServiceTier::Fast),
        "flex" => Ok(AutopilotChatServiceTier::Flex),
        _ => Err("Service tier must be default, fast, or flex".to_string()),
    }
}

fn parse_approval_mode(raw: &str) -> Result<codex_client::AskForApproval, String> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "never" => Ok(codex_client::AskForApproval::Never),
        "on-failure" => Ok(codex_client::AskForApproval::OnFailure),
        "on-request" => Ok(codex_client::AskForApproval::OnRequest),
        "unless-trusted" => Ok(codex_client::AskForApproval::UnlessTrusted),
        _ => Err("Approval mode must be never, on-failure, on-request, or unless-trusted".to_string()),
    }
}

fn parse_sandbox_mode(raw: &str) -> Result<codex_client::SandboxMode, String> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "danger-full-access" => Ok(codex_client::SandboxMode::DangerFullAccess),
        "workspace-write" => Ok(codex_client::SandboxMode::WorkspaceWrite),
        "read-only" => Ok(codex_client::SandboxMode::ReadOnly),
        _ => Err("Sandbox mode must be danger-full-access, workspace-write, or read-only".to_string()),
    }
}

fn parse_personality(raw: &str) -> Result<AutopilotChatPersonality, String> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "auto" => Ok(AutopilotChatPersonality::Auto),
        "friendly" => Ok(AutopilotChatPersonality::Friendly),
        "pragmatic" => Ok(AutopilotChatPersonality::Pragmatic),
        "none" => Ok(AutopilotChatPersonality::None),
        _ => Err("Personality must be auto, friendly, pragmatic, or none".to_string()),
    }
}

fn parse_collaboration_mode(raw: &str) -> Result<AutopilotChatCollaborationMode, String> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "off" => Ok(AutopilotChatCollaborationMode::Off),
        "default" => Ok(AutopilotChatCollaborationMode::Default),
        "plan" => Ok(AutopilotChatCollaborationMode::Plan),
        _ => Err("Collaboration mode must be off, default, or plan".to_string()),
    }
}

fn current_epoch_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

const REMOTE_HTML: &str = r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <title>Autopilot Codex Remote</title>
  <style>
    :root {
      --bg: #0d1318;
      --bg-panel: rgba(14, 26, 34, 0.92);
      --bg-elevated: rgba(22, 39, 50, 0.95);
      --border: rgba(117, 156, 176, 0.26);
      --text: #eef6f9;
      --muted: #9ab1bf;
      --accent: #5fc9b4;
      --accent-2: #f0b96d;
      --danger: #f27e7e;
      --shadow: 0 18px 48px rgba(0, 0, 0, 0.35);
    }
    * { box-sizing: border-box; }
    body {
      margin: 0;
      min-height: 100vh;
      font-family: ui-monospace, "SFMono-Regular", "Cascadia Mono", "Liberation Mono", monospace;
      background:
        radial-gradient(circle at top left, rgba(95, 201, 180, 0.18), transparent 28%),
        radial-gradient(circle at top right, rgba(240, 185, 109, 0.18), transparent 24%),
        linear-gradient(160deg, #081016 0%, #101920 44%, #091015 100%);
      color: var(--text);
    }
    .shell {
      display: grid;
      grid-template-columns: 280px minmax(0, 1fr);
      gap: 16px;
      padding: 16px;
    }
    .panel {
      background: var(--bg-panel);
      border: 1px solid var(--border);
      border-radius: 18px;
      box-shadow: var(--shadow);
      overflow: hidden;
    }
    .panel h2, .panel h3 {
      margin: 0;
      font-size: 12px;
      letter-spacing: 0.14em;
      text-transform: uppercase;
      color: var(--muted);
    }
    .panel-header {
      padding: 14px 16px 10px;
      border-bottom: 1px solid var(--border);
    }
    .panel-body {
      padding: 14px 16px 16px;
    }
    .stack { display: grid; gap: 12px; }
    .metrics {
      display: grid;
      grid-template-columns: repeat(auto-fit, minmax(160px, 1fr));
      gap: 10px;
    }
    .metric {
      padding: 12px;
      background: var(--bg-elevated);
      border: 1px solid var(--border);
      border-radius: 14px;
    }
    .metric .label {
      display: block;
      color: var(--muted);
      font-size: 11px;
      margin-bottom: 4px;
      text-transform: uppercase;
      letter-spacing: 0.08em;
    }
    .metric .value {
      font-size: 15px;
      line-height: 1.35;
    }
    .thread-list {
      display: grid;
      gap: 8px;
      max-height: calc(100vh - 32px);
      overflow: auto;
      padding: 14px 16px 16px;
    }
    .thread {
      border: 1px solid var(--border);
      background: rgba(255, 255, 255, 0.02);
      border-radius: 14px;
      padding: 10px 12px;
      cursor: pointer;
    }
    .thread.active {
      border-color: rgba(95, 201, 180, 0.6);
      background: rgba(95, 201, 180, 0.08);
    }
    .thread .name {
      font-size: 13px;
      margin-bottom: 4px;
    }
    .thread .preview, .thread .meta {
      font-size: 11px;
      color: var(--muted);
    }
    .grid-2 {
      display: grid;
      grid-template-columns: repeat(auto-fit, minmax(280px, 1fr));
      gap: 12px;
    }
    .messages {
      display: grid;
      gap: 10px;
      max-height: 42vh;
      overflow: auto;
    }
    .message {
      border: 1px solid var(--border);
      background: rgba(255, 255, 255, 0.02);
      border-radius: 14px;
      padding: 10px 12px;
      white-space: pre-wrap;
      word-break: break-word;
    }
    .message .meta {
      margin-bottom: 6px;
      color: var(--muted);
      font-size: 11px;
      text-transform: uppercase;
      letter-spacing: 0.08em;
    }
    .artifact pre, .terminal pre {
      margin: 0;
      padding: 12px;
      border-radius: 12px;
      background: #091117;
      border: 1px solid var(--border);
      overflow: auto;
      white-space: pre-wrap;
    }
    .controls, .requests {
      display: grid;
      gap: 10px;
    }
    label {
      display: grid;
      gap: 6px;
      font-size: 11px;
      color: var(--muted);
      text-transform: uppercase;
      letter-spacing: 0.08em;
    }
    input, textarea, select, button {
      width: 100%;
      border-radius: 12px;
      border: 1px solid var(--border);
      background: var(--bg-elevated);
      color: var(--text);
      padding: 10px 12px;
      font: inherit;
    }
    textarea {
      min-height: 110px;
      resize: vertical;
    }
    button {
      cursor: pointer;
      background: linear-gradient(135deg, rgba(95, 201, 180, 0.24), rgba(95, 201, 180, 0.08));
    }
    .button-row {
      display: grid;
      grid-template-columns: repeat(auto-fit, minmax(120px, 1fr));
      gap: 8px;
    }
    .danger { color: var(--danger); }
    .hint { color: var(--muted); font-size: 11px; }
    .hidden { display: none !important; }
    .status-line { padding: 12px 16px 0; color: var(--muted); font-size: 12px; }
    .request-card {
      border: 1px solid var(--border);
      background: rgba(255,255,255,0.02);
      border-radius: 14px;
      padding: 10px 12px;
    }
    .request-card h4 { margin: 0 0 6px; font-size: 12px; }
    @media (max-width: 980px) {
      .shell { grid-template-columns: 1fr; }
      .thread-list { max-height: none; }
    }
  </style>
</head>
<body>
  <div class="status-line" id="status-line">Paste a token or use a pairing URL with <code>#token=...</code>.</div>
  <div class="shell">
    <aside class="panel">
      <div class="panel-header">
        <h2>Threads</h2>
      </div>
      <div class="panel-body stack">
        <label>
          Token
          <input id="token-input" autocomplete="off" placeholder="remote auth token" />
        </label>
        <div class="hint">This page stores the token only in memory for this tab.</div>
      </div>
      <div class="thread-list" id="threads"></div>
    </aside>
    <main class="stack">
      <section class="panel">
        <div class="panel-header"><h2>Status</h2></div>
        <div class="panel-body stack">
          <div class="metrics" id="metrics"></div>
          <div class="grid-2">
            <div class="controls">
              <h3>Session</h3>
              <label>Model<select id="model"></select></label>
              <label>Reasoning<select id="reasoning"></select></label>
              <label>Service tier<select id="service-tier"></select></label>
              <label>Approval<select id="approval-mode"></select></label>
              <label>Sandbox<select id="sandbox-mode"></select></label>
              <label>Personality<select id="personality"></select></label>
              <label>Collaboration<select id="collaboration-mode"></select></label>
              <div class="button-row">
                <button id="apply-session">Apply Session</button>
                <button id="interrupt-turn">Interrupt Turn</button>
              </div>
            </div>
            <div class="requests">
              <h3>Requests</h3>
              <div id="requests"></div>
            </div>
          </div>
        </div>
      </section>
      <section class="panel">
        <div class="panel-header"><h2>Transcript</h2></div>
        <div class="panel-body stack">
          <div id="active-thread-meta" class="hint"></div>
          <div class="messages" id="messages"></div>
          <label>
            Follow-up
            <textarea id="prompt" placeholder="Continue the current Codex thread"></textarea>
          </label>
          <button id="send-prompt">Send Prompt</button>
        </div>
      </section>
      <section class="panel">
        <div class="panel-header"><h2>Artifacts</h2></div>
        <div class="panel-body grid-2">
          <div class="artifact stack">
            <h3>Plan</h3>
            <div id="plan"></div>
          </div>
          <div class="artifact stack">
            <h3>Latest Diff</h3>
            <div id="diff"></div>
          </div>
        </div>
      </section>
      <section class="panel hidden" id="workspace-panel">
        <div class="panel-header"><h2>Workspace</h2></div>
        <div class="panel-body stack" id="workspace"></div>
      </section>
      <section class="panel hidden" id="terminal-panel">
        <div class="panel-header"><h2>Terminal</h2></div>
        <div class="panel-body terminal stack" id="terminal"></div>
      </section>
    </main>
  </div>
  <script>
    const els = {
      token: document.getElementById("token-input"),
      status: document.getElementById("status-line"),
      threads: document.getElementById("threads"),
      metrics: document.getElementById("metrics"),
      messages: document.getElementById("messages"),
      requests: document.getElementById("requests"),
      plan: document.getElementById("plan"),
      diff: document.getElementById("diff"),
      prompt: document.getElementById("prompt"),
      sendPrompt: document.getElementById("send-prompt"),
      interruptTurn: document.getElementById("interrupt-turn"),
      activeThreadMeta: document.getElementById("active-thread-meta"),
      model: document.getElementById("model"),
      reasoning: document.getElementById("reasoning"),
      serviceTier: document.getElementById("service-tier"),
      approvalMode: document.getElementById("approval-mode"),
      sandboxMode: document.getElementById("sandbox-mode"),
      personality: document.getElementById("personality"),
      collaborationMode: document.getElementById("collaboration-mode"),
      applySession: document.getElementById("apply-session"),
      workspacePanel: document.getElementById("workspace-panel"),
      workspace: document.getElementById("workspace"),
      terminalPanel: document.getElementById("terminal-panel"),
      terminal: document.getElementById("terminal"),
    };
    let snapshot = null;

    function currentToken() {
      const hash = new URLSearchParams(location.hash.replace(/^#/, ""));
      const hashToken = hash.get("token");
      return (hashToken || els.token.value || "").trim();
    }

    function setStatus(text, isError = false) {
      els.status.textContent = text;
      els.status.classList.toggle("danger", isError);
    }

    function authHeaders() {
      const token = currentToken();
      return token ? { Authorization: `Bearer ${token}` } : {};
    }

    async function fetchSnapshot() {
      const token = currentToken();
      if (!token) {
        setStatus("Paste a token or open the pairing URL from desktop.", true);
        return;
      }
      try {
        const response = await fetch("/v1/snapshot", { headers: authHeaders() });
        if (!response.ok) {
          throw new Error(`snapshot ${response.status}`);
        }
        snapshot = await response.json();
        render();
        setStatus(`Remote linked. Snapshot ${new Date(snapshot.generated_at_epoch_ms).toLocaleTimeString()}`);
      } catch (error) {
        setStatus(`Remote fetch failed: ${error.message}`, true);
      }
    }

    async function postAction(payload) {
      const response = await fetch("/v1/action", {
        method: "POST",
        headers: { "Content-Type": "application/json", ...authHeaders() },
        body: JSON.stringify(payload),
      });
      const result = await response.json();
      if (!response.ok || !result.success) {
        throw new Error(result.message || `action ${response.status}`);
      }
      return result;
    }

    function option(value, selected) {
      const opt = document.createElement("option");
      opt.value = value;
      opt.textContent = value;
      if (selected) opt.selected = true;
      return opt;
    }

    function fillSelect(node, values, current, fallback = []) {
      node.innerHTML = "";
      const seen = new Set();
      [...values, ...fallback].forEach((value) => {
        if (!value || seen.has(value)) return;
        seen.add(value);
        node.appendChild(option(value, value === current));
      });
      if (current && !seen.has(current)) {
        node.appendChild(option(current, true));
      }
    }

    function renderMetrics() {
      const metrics = [
        ["Codex", snapshot.session.readiness_summary],
        ["Wallet", `${snapshot.wallet.total_sats} sats (${snapshot.wallet.status})`],
        ["Provider", snapshot.provider.mode],
        ["Model", snapshot.session.current_model],
        ["Approval", snapshot.session.approval_mode],
        ["Sandbox", snapshot.session.sandbox_mode],
      ];
      els.metrics.innerHTML = "";
      metrics.forEach(([label, value]) => {
        const card = document.createElement("div");
        card.className = "metric";
        card.innerHTML = `<span class="label">${label}</span><div class="value">${value || "n/a"}</div>`;
        els.metrics.appendChild(card);
      });
    }

    function renderThreads() {
      els.threads.innerHTML = "";
      if (!snapshot.threads.length) {
        els.threads.innerHTML = `<div class="hint">No threads are cached in desktop state yet.</div>`;
        return;
      }
      snapshot.threads.forEach((thread) => {
        const item = document.createElement("div");
        item.className = `thread${thread.is_active ? " active" : ""}`;
        item.innerHTML = `
          <div class="name">${thread.thread_name || thread.thread_id}</div>
          <div class="preview">${thread.preview || "No preview yet."}</div>
          <div class="meta">${thread.status || "unknown"} | loaded=${thread.loaded}</div>
        `;
        item.onclick = async () => {
          try {
            await postAction({ action: "select_thread", thread_id: thread.thread_id });
            await fetchSnapshot();
          } catch (error) {
            setStatus(`Thread select failed: ${error.message}`, true);
          }
        };
        els.threads.appendChild(item);
      });
    }

    function renderMessages() {
      els.messages.innerHTML = "";
      if (!snapshot.active_thread) {
        els.activeThreadMeta.textContent = "Select a thread to load transcript state.";
        return;
      }
      els.activeThreadMeta.textContent = `${snapshot.active_thread.thread_name || snapshot.active_thread.thread_id} | status=${snapshot.active_thread.status || "n/a"} | turn=${snapshot.active_thread.last_turn_status || "idle"}`;
      snapshot.active_thread.messages.forEach((message) => {
        const item = document.createElement("div");
        item.className = "message";
        item.innerHTML = `<div class="meta">${message.role} | ${message.status}</div>${message.content || "(empty)"}`;
        els.messages.appendChild(item);
      });
    }

    function renderPlan() {
      els.plan.innerHTML = "";
      const plan = snapshot.artifacts.plan;
      if (!plan) {
        els.plan.innerHTML = `<div class="hint">No saved plan artifact for the active thread.</div>`;
        return;
      }
      if (plan.explanation) {
        const explanation = document.createElement("div");
        explanation.textContent = plan.explanation;
        els.plan.appendChild(explanation);
      }
      const list = document.createElement("div");
      list.className = "stack";
      plan.steps.forEach((step) => {
        const row = document.createElement("div");
        row.textContent = `[${step.status}] ${step.step}`;
        list.appendChild(row);
      });
      els.plan.appendChild(list);
    }

    function renderDiff() {
      els.diff.innerHTML = "";
      const diff = snapshot.artifacts.latest_diff;
      if (!diff) {
        els.diff.innerHTML = `<div class="hint">No diff artifact for the active thread.</div>`;
        return;
      }
      const summary = document.createElement("div");
      summary.textContent = diff.summary;
      els.diff.appendChild(summary);
      if (diff.files.length) {
        const files = document.createElement("div");
        files.className = "stack";
        diff.files.forEach((file) => {
          const row = document.createElement("div");
          row.textContent = `${file.path} (+${file.added_line_count} / -${file.removed_line_count})`;
          files.appendChild(row);
        });
        els.diff.appendChild(files);
      }
      const pre = document.createElement("pre");
      pre.textContent = diff.raw_diff;
      els.diff.appendChild(pre);
    }

    function approvalButtons(kind, requestId) {
      const decisions = [
        ["Accept", "accept"],
        ["Session", "accept_for_session"],
        ["Decline", "decline"],
        ["Cancel", "cancel"],
      ];
      const row = document.createElement("div");
      row.className = "button-row";
      decisions.forEach(([label, decision]) => {
        const button = document.createElement("button");
        button.textContent = label;
        button.onclick = async () => {
          try {
            await postAction({
              action: "respond_approval",
              kind,
              request_id: requestId,
              decision,
            });
            await fetchSnapshot();
          } catch (error) {
            setStatus(`Approval failed: ${error.message}`, true);
          }
        };
        row.appendChild(button);
      });
      return row;
    }

    function renderRequests() {
      els.requests.innerHTML = "";
      const requests = [];
      snapshot.approvals.forEach((request) => requests.push({ kind: "approval", request }));
      snapshot.tool_user_inputs.forEach((request) => requests.push({ kind: "tool", request }));
      if (!requests.length) {
        els.requests.innerHTML = `<div class="hint">No pending approvals or tool prompts.</div>`;
        return;
      }
      requests.forEach((entry) => {
        const card = document.createElement("div");
        card.className = "request-card";
        if (entry.kind === "approval") {
          const request = entry.request;
          card.innerHTML = `
            <h4>${request.kind} approval</h4>
            <div class="hint">${request.reason || "No reason provided."}</div>
            <div>${request.command || request.grant_root || "n/a"}</div>
          `;
          card.appendChild(approvalButtons(request.kind, request.request_id));
        } else {
          const request = entry.request;
          card.innerHTML = `<h4>tool prompt</h4>`;
          const form = document.createElement("div");
          form.className = "stack";
          request.questions.forEach((question) => {
            const label = document.createElement("label");
            label.textContent = `${question.header}: ${question.question}`;
            const input = question.options.length ? document.createElement("select") : document.createElement("input");
            input.dataset.questionId = question.id;
            if (question.options.length) {
              question.options.forEach((value) => input.appendChild(option(value, false)));
            }
            label.appendChild(input);
            form.appendChild(label);
          });
          const button = document.createElement("button");
          button.textContent = "Submit Answers";
          button.onclick = async () => {
            const answers = {};
            form.querySelectorAll("[data-question-id]").forEach((node) => {
              const value = node.value ? [node.value] : [];
              answers[node.dataset.questionId] = value;
            });
            try {
              await postAction({
                action: "respond_tool_user_input",
                request_id: request.request_id,
                answers,
              });
              await fetchSnapshot();
            } catch (error) {
              setStatus(`Tool prompt failed: ${error.message}`, true);
            }
          };
          card.appendChild(form);
          card.appendChild(button);
        }
        els.requests.appendChild(card);
      });
    }

    function renderWorkspace() {
      const workspace = snapshot.workspace;
      els.workspacePanel.classList.toggle("hidden", !workspace);
      els.workspace.innerHTML = "";
      if (!workspace) return;
      [
        ["Project", workspace.project_name || workspace.project_id || "n/a"],
        ["Workspace", workspace.workspace_root || "n/a"],
        ["Cwd", workspace.cwd || "n/a"],
        ["Path", workspace.path || "n/a"],
        ["Branch", workspace.git_branch || "n/a"],
        ["Dirty", workspace.git_dirty == null ? "n/a" : String(workspace.git_dirty)],
      ].forEach(([label, value]) => {
        const row = document.createElement("div");
        row.textContent = `${label}: ${value}`;
        els.workspace.appendChild(row);
      });
      if ((workspace.worktree_entries || []).length) {
        const heading = document.createElement("div");
        heading.textContent = "Worktrees:";
        els.workspace.appendChild(heading);
        workspace.worktree_entries.forEach((entry) => {
          const row = document.createElement("div");
          row.textContent = `- ${entry}`;
          els.workspace.appendChild(row);
        });
      }
    }

    function renderTerminal() {
      const terminal = snapshot.terminal;
      els.terminalPanel.classList.toggle("hidden", !terminal);
      els.terminal.innerHTML = "";
      if (!terminal) return;
      const meta = document.createElement("div");
      meta.textContent = `${terminal.shell} | ${terminal.status} | lines=${terminal.line_count}`;
      els.terminal.appendChild(meta);
      const pre = document.createElement("pre");
      pre.textContent = terminal.lines.join("\n");
      els.terminal.appendChild(pre);
    }

    function renderSessionControls() {
      fillSelect(els.model, snapshot.session.available_models, snapshot.session.current_model, [snapshot.session.current_model]);
      fillSelect(els.reasoning, snapshot.session.supported_reasoning_efforts, snapshot.session.reasoning_effort || "medium", ["low", "medium", "high"]);
      fillSelect(els.serviceTier, ["default", "fast", "flex"], snapshot.session.service_tier);
      fillSelect(els.approvalMode, ["never", "on-failure", "on-request", "unless-trusted"], snapshot.session.approval_mode);
      fillSelect(els.sandboxMode, ["danger-full-access", "workspace-write", "read-only"], snapshot.session.sandbox_mode);
      fillSelect(els.personality, ["auto", "friendly", "pragmatic", "none"], snapshot.session.personality);
      fillSelect(els.collaborationMode, ["off", "default", "plan"], snapshot.session.collaboration_mode);
    }

    function render() {
      if (!snapshot) return;
      renderMetrics();
      renderThreads();
      renderMessages();
      renderPlan();
      renderDiff();
      renderRequests();
      renderWorkspace();
      renderTerminal();
      renderSessionControls();
    }

    els.token.addEventListener("change", fetchSnapshot);
    els.sendPrompt.addEventListener("click", async () => {
      try {
        await postAction({ action: "submit_prompt", prompt: els.prompt.value });
        els.prompt.value = "";
        await fetchSnapshot();
      } catch (error) {
        setStatus(`Prompt failed: ${error.message}`, true);
      }
    });
    els.interruptTurn.addEventListener("click", async () => {
      try {
        await postAction({ action: "interrupt_turn" });
        await fetchSnapshot();
      } catch (error) {
        setStatus(`Interrupt failed: ${error.message}`, true);
      }
    });
    els.applySession.addEventListener("click", async () => {
      try {
        await postAction({
          action: "update_session",
          model: els.model.value,
          reasoning_effort: els.reasoning.value,
          service_tier: els.serviceTier.value,
          approval_mode: els.approvalMode.value,
          sandbox_mode: els.sandboxMode.value,
          personality: els.personality.value,
          collaboration_mode: els.collaborationMode.value,
        });
        await fetchSnapshot();
      } catch (error) {
        setStatus(`Session update failed: ${error.message}`, true);
      }
    });
    if (location.hash.includes("token=")) {
      fetchSnapshot();
    }
    setInterval(fetchSnapshot, 1200);
  </script>
</body>
</html>
"#;

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic, clippy::unwrap_used, reason = "remote runtime tests use direct assertions")]
mod tests {
    use super::*;

    fn sample_snapshot() -> CodexRemoteSnapshot {
        CodexRemoteSnapshot {
            schema_version: 1,
            generated_at_epoch_ms: 42,
            threads: vec![CodexRemoteThreadSummary {
                thread_id: "thread-1".to_string(),
                thread_name: Some("Thread 1".to_string()),
                preview: "preview".to_string(),
                status: Some("idle".to_string()),
                loaded: true,
                created_at: Some(1),
                updated_at: Some(2),
                is_active: true,
            }],
            active_thread: Some(CodexRemoteActiveThread {
                thread_id: "thread-1".to_string(),
                thread_name: Some("Thread 1".to_string()),
                status: Some("idle".to_string()),
                last_turn_status: Some("completed".to_string()),
                active_turn_id: None,
                messages: vec![CodexRemoteMessage {
                    id: 1,
                    role: "user".to_string(),
                    status: "done".to_string(),
                    content: "hello".to_string(),
                }],
            }),
            approvals: Vec::new(),
            tool_user_inputs: Vec::new(),
            session: CodexRemoteSessionStatus {
                connection_status: "ready".to_string(),
                readiness_summary: "ready".to_string(),
                account_summary: "signed in".to_string(),
                config_constraint_summary: None,
                current_model: "gpt-5.3-codex".to_string(),
                available_models: vec!["gpt-5.3-codex".to_string()],
                reasoning_effort: Some("medium".to_string()),
                supported_reasoning_efforts: vec!["low".to_string(), "medium".to_string()],
                service_tier: "default".to_string(),
                approval_mode: "never".to_string(),
                sandbox_mode: "workspace-write".to_string(),
                personality: "auto".to_string(),
                collaboration_mode: "off".to_string(),
                token_usage_summary: None,
                pending_auth_refresh: 0,
            },
            artifacts: CodexRemoteArtifacts::default(),
            wallet: CodexRemoteWalletSummary {
                total_sats: 77,
                status: "connected".to_string(),
                last_error: None,
            },
            provider: CodexRemoteProviderSummary {
                mode: "offline".to_string(),
                online: false,
                last_action: None,
                last_error: None,
            },
            workspace: None,
            terminal: None,
        }
    }

    #[test]
    fn validate_remote_bind_addr_rejects_public_ip() {
        let error = validate_remote_bind_addr("8.8.8.8:4848").expect_err("public ip should fail");
        assert!(error.contains("loopback"));
    }

    #[test]
    fn remote_pairing_url_uses_hash_token() {
        let url = remote_pairing_url("127.0.0.1:4848".parse().unwrap(), "secret");
        assert_eq!(url, "http://127.0.0.1:4848/#token=secret");
    }

    #[test]
    fn parse_git_worktree_output_summarizes_branches_and_detached_entries() {
        let raw = "worktree /repo\nHEAD abcdef\nbranch refs/heads/main\n\nworktree /repo-feature\nHEAD fedcba\nbranch refs/heads/feature/cx-13\nlocked reason\n\nworktree /repo-detached\nHEAD 012345\ndetached\n";
        let entries = parse_git_worktree_output(raw);
        assert_eq!(
            entries,
            vec![
                "/repo (main)".to_string(),
                "/repo-feature (feature/cx-13) locked".to_string(),
                "/repo-detached (detached)".to_string(),
            ]
        );
    }

    #[test]
    fn runtime_serves_snapshot_and_routes_actions() {
        let token = "token-123".to_string();
        let mut runtime = DesktopCodexRemoteRuntime::spawn(CodexRemoteRuntimeConfig {
            listen_addr: "127.0.0.1:0".parse().unwrap(),
            auth_token: token.clone(),
        })
        .expect("spawn remote runtime");
        runtime
            .sync_snapshot(sample_snapshot())
            .expect("sync sample snapshot");

        let client = reqwest::blocking::Client::new();
        let snapshot_url = format!("http://{}/v1/snapshot", runtime.listen_addr());
        let action_url = format!("http://{}/v1/action", runtime.listen_addr());

        let unauthorized = client.get(snapshot_url.as_str()).send().expect("send unauthorized");
        assert_eq!(unauthorized.status(), reqwest::StatusCode::UNAUTHORIZED);

        let snapshot = client
            .get(snapshot_url.as_str())
            .bearer_auth(token.as_str())
            .send()
            .expect("send authorized")
            .error_for_status()
            .expect("authorized status")
            .json::<CodexRemoteSnapshot>()
            .expect("decode snapshot");
        assert_eq!(snapshot.wallet.total_sats, 77);

        let join = std::thread::spawn({
            let client = client.clone();
            let token = token.clone();
            move || {
                client
                    .post(action_url.as_str())
                    .bearer_auth(token)
                    .json(&CodexRemoteActionRequest::InterruptTurn)
                    .send()
                    .expect("post action")
                    .error_for_status()
                    .expect("action status")
                    .json::<CodexRemoteActionResponse>()
                    .expect("decode action response")
            }
        });

        let mut envelope = None;
        for _ in 0..20 {
            let updates = runtime.drain_updates();
            if let Some(CodexRemoteRuntimeUpdate::ActionRequest(request)) =
                updates.into_iter().next()
            {
                envelope = Some(request);
                break;
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        let request = envelope.expect("expected remote action request");
        assert_eq!(request.action, CodexRemoteActionRequest::InterruptTurn);
        request.respond(CodexRemoteActionResponse::ok("interrupted"));
        let response = join.join().expect("join action thread");
        assert_eq!(response.message, "interrupted");
    }

    #[test]
    fn rotating_token_invalidates_old_bearer() {
        let runtime = DesktopCodexRemoteRuntime::spawn(CodexRemoteRuntimeConfig {
            listen_addr: "127.0.0.1:0".parse().unwrap(),
            auth_token: "old".to_string(),
        })
        .expect("spawn remote runtime");
        runtime
            .sync_snapshot(sample_snapshot())
            .expect("sync sample snapshot");
        runtime.rotate_token("new".to_string()).expect("rotate token");

        let client = reqwest::blocking::Client::new();
        let snapshot_url = format!("http://{}/v1/snapshot", runtime.listen_addr());

        let old = client
            .get(snapshot_url.as_str())
            .bearer_auth("old")
            .send()
            .expect("send old token");
        assert_eq!(old.status(), reqwest::StatusCode::UNAUTHORIZED);

        let new = client
            .get(snapshot_url.as_str())
            .bearer_auth("new")
            .send()
            .expect("send new token");
        assert_eq!(new.status(), reqwest::StatusCode::OK);
    }
}
