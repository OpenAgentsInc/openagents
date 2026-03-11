use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread::JoinHandle;
use std::time::Duration;

use anyhow::{Context, Result};
use codex_client::{
    AppServerChannels, AppServerClient, AppServerConfig, AppServerNotification, AppServerRequest,
    AppServerRequestId, AppsListParams, AskForApproval, CancelLoginAccountParams,
    ChatgptAuthTokensRefreshParams, ChatgptAuthTokensRefreshResponse, ClientInfo,
    CollaborationModeListParams, CommandExecParams, CommandExecutionRequestApprovalParams,
    CommandExecutionRequestApprovalResponse, ConfigBatchWriteParams, ConfigReadParams,
    ConfigValueWriteParams, DynamicToolCallOutputContentItem, DynamicToolCallParams,
    DynamicToolCallResponse, ExperimentalFeatureListParams, ExternalAgentConfigDetectParams,
    ExternalAgentConfigImportParams, FileChangeRequestApprovalParams,
    FileChangeRequestApprovalResponse, FuzzyFileSearchSessionStartParams,
    FuzzyFileSearchSessionStopParams, FuzzyFileSearchSessionUpdateParams, GetAccountParams,
    InitializeCapabilities, InitializeParams, ListMcpServerStatusParams, LoginAccountParams,
    McpServerOauthLoginParams, ModelListParams, ReviewStartParams, SandboxMode, ServiceTier,
    SkillScope, SkillsConfigWriteParams, SkillsListParams, SkillsListResponse,
    SkillsRemoteReadParams, SkillsRemoteWriteParams, ThreadArchiveParams, ThreadCompactStartParams,
    ThreadForkParams, ThreadListParams, ThreadLoadedListParams, ThreadReadParams,
    ThreadRealtimeAppendTextParams, ThreadRealtimeStartParams, ThreadRealtimeStopParams,
    ThreadResumeParams, ThreadRollbackParams, ThreadSetNameParams, ThreadStartParams,
    ThreadUnarchiveParams, ThreadUnsubscribeParams, ToolRequestUserInputParams,
    ToolRequestUserInputResponse, TurnInterruptParams, TurnStartParams, TurnSteerParams,
    WindowsSandboxSetupStartParams,
};
use serde_json::Value;
use tokio::runtime::Runtime;
use tokio::sync::mpsc::error::TryRecvError as TokioTryRecvError;

mod normalizer;
mod router;
mod session;
mod types;

use normalizer::{
    extract_latest_thread_compaction_artifact, extract_latest_thread_plan_artifact,
    extract_latest_thread_review_artifact, extract_thread_transcript_messages,
    normalize_notification, thread_status_label,
};
use router::run_codex_lane_loop;
use session::{
    account_summary, fetch_model_catalog, fetch_model_catalog_entries, is_disconnect_error,
    mcp_auth_status_label, rate_limits_summary, summarize_skills_list_response,
};
pub use types::*;

const CODEX_LANE_POLL: Duration = Duration::from_millis(16);

fn default_opt_out_notification_methods() -> Vec<String> {
    codex_client::legacy_codex_event_opt_out_notification_methods()
        .iter()
        .map(|method| (*method).to_string())
        .collect()
}

fn is_pre_materialization_thread_read_error(message: &str) -> bool {
    message.contains("not materialized yet") && message.contains("includeTurns is unavailable")
}

fn sandbox_mode_from_policy(policy: Option<&codex_client::SandboxPolicy>) -> Option<SandboxMode> {
    match policy {
        Some(codex_client::SandboxPolicy::DangerFullAccess) => Some(SandboxMode::DangerFullAccess),
        Some(codex_client::SandboxPolicy::ReadOnly)
        | Some(codex_client::SandboxPolicy::ExternalSandbox { .. }) => Some(SandboxMode::ReadOnly),
        Some(codex_client::SandboxPolicy::WorkspaceWrite { .. }) => {
            Some(SandboxMode::WorkspaceWrite)
        }
        None => None,
    }
}

fn reasoning_effort_label(
    reasoning_effort: Option<codex_client::ReasoningEffort>,
) -> Option<String> {
    reasoning_effort.map(|value| {
        serde_json::to_string(&value)
            .unwrap_or_else(|_| "\"unknown\"".to_string())
            .trim_matches('"')
            .to_string()
    })
}

fn review_delivery_label(delivery: Option<codex_client::ReviewDelivery>) -> String {
    match delivery.unwrap_or(codex_client::ReviewDelivery::Inline) {
        codex_client::ReviewDelivery::Inline => "inline".to_string(),
        codex_client::ReviewDelivery::Detached => "detached".to_string(),
    }
}

fn review_target_label(target: &codex_client::ReviewTarget) -> String {
    match target {
        codex_client::ReviewTarget::UncommittedChanges => "uncommitted changes".to_string(),
        codex_client::ReviewTarget::BaseBranch { branch } => format!("base branch {branch}"),
        codex_client::ReviewTarget::Commit { sha, title } => title
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .map(|title| format!("commit {} ({title})", sha))
            .unwrap_or_else(|| format!("commit {sha}")),
        codex_client::ReviewTarget::Custom { instructions } => {
            let trimmed = instructions.trim();
            if trimmed.is_empty() {
                "custom review".to_string()
            } else {
                trimmed.to_string()
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct CodexLaneConfig {
    pub cwd: Option<PathBuf>,
    pub bootstrap_thread: bool,
    pub bootstrap_model: Option<String>,
    pub wire_log_path: Option<PathBuf>,
    pub env: Vec<(String, String)>,
    pub client_info: ClientInfo,
    pub approval_policy: Option<AskForApproval>,
    pub experimental_api: bool,
    pub opt_out_notification_methods: Vec<String>,
}

impl Default for CodexLaneConfig {
    fn default() -> Self {
        Self {
            cwd: std::env::current_dir().ok(),
            bootstrap_thread: true,
            // Use app-server current default model unless the caller overrides.
            bootstrap_model: None,
            wire_log_path: std::env::var("OPENAGENTS_CODEX_WIRE_LOG_PATH")
                .ok()
                .filter(|value| !value.trim().is_empty())
                .map(PathBuf::from),
            env: Vec::new(),
            client_info: ClientInfo {
                name: "openagents-autopilot-desktop".to_string(),
                title: Some("OpenAgents Autopilot Desktop".to_string()),
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
            approval_policy: Some(AskForApproval::Never),
            experimental_api: true,
            opt_out_notification_methods: default_opt_out_notification_methods(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CodexLaneLifecycle {
    Starting,
    Ready,
    Error,
    Disconnected,
    Stopped,
}

impl CodexLaneLifecycle {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Starting => "starting",
            Self::Ready => "ready",
            Self::Error => "error",
            Self::Disconnected => "disconnected",
            Self::Stopped => "stopped",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CodexLaneSnapshot {
    pub lifecycle: CodexLaneLifecycle,
    pub active_thread_id: Option<String>,
    pub last_error: Option<String>,
    pub last_status: Option<String>,
    pub install_probe: codex_client::CodexInstallationProbe,
}

impl Default for CodexLaneSnapshot {
    fn default() -> Self {
        Self {
            lifecycle: CodexLaneLifecycle::Starting,
            active_thread_id: None,
            last_error: None,
            last_status: Some("Codex lane starting".to_string()),
            install_probe: codex_client::CodexInstallationProbe::default(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CodexLaneCommandKind {
    ThreadStart,
    ThreadResume,
    ThreadFork,
    ThreadArchive,
    ThreadUnsubscribe,
    ThreadNameSet,
    ThreadUnarchive,
    ThreadCompactStart,
    ThreadRollback,
    ThreadRead,
    ThreadList,
    ThreadLoadedList,
    TurnStart,
    TurnSteer,
    TurnInterrupt,
    ServerRequestCommandApprovalRespond,
    ServerRequestFileApprovalRespond,
    ServerRequestToolCallRespond,
    ServerRequestToolUserInputRespond,
    ServerRequestAuthRefreshRespond,
    AccountRead,
    AccountLoginStart,
    AccountLoginCancel,
    AccountLogout,
    AccountRateLimitsRead,
    ModelList,
    ConfigRead,
    ConfigRequirementsRead,
    ConfigValueWrite,
    ConfigBatchWrite,
    ExternalAgentConfigDetect,
    ExternalAgentConfigImport,
    McpServerStatusList,
    McpServerOauthLogin,
    McpServerReload,
    AppsList,
    ReviewStart,
    CommandExec,
    CollaborationModeList,
    ExperimentalFeatureList,
    ThreadRealtimeStart,
    ThreadRealtimeAppendText,
    ThreadRealtimeStop,
    WindowsSandboxSetupStart,
    FuzzyFileSearchSessionStart,
    FuzzyFileSearchSessionUpdate,
    FuzzyFileSearchSessionStop,
    SkillsRemoteList,
    SkillsRemoteExport,
    SkillsList,
    SkillsConfigWrite,
}

impl CodexLaneCommandKind {
    pub const fn label(self) -> &'static str {
        match self {
            Self::ThreadStart => "thread/start",
            Self::ThreadResume => "thread/resume",
            Self::ThreadFork => "thread/fork",
            Self::ThreadArchive => "thread/archive",
            Self::ThreadUnsubscribe => "thread/unsubscribe",
            Self::ThreadNameSet => "thread/name/set",
            Self::ThreadUnarchive => "thread/unarchive",
            Self::ThreadCompactStart => "thread/compact/start",
            Self::ThreadRollback => "thread/rollback",
            Self::ThreadRead => "thread/read",
            Self::ThreadList => "thread/list",
            Self::ThreadLoadedList => "thread/loaded/list",
            Self::TurnStart => "turn/start",
            Self::TurnSteer => "turn/steer",
            Self::TurnInterrupt => "turn/interrupt",
            Self::ServerRequestCommandApprovalRespond => {
                "item/commandExecution/requestApproval:respond"
            }
            Self::ServerRequestFileApprovalRespond => "item/fileChange/requestApproval:respond",
            Self::ServerRequestToolCallRespond => "item/tool/call:respond",
            Self::ServerRequestToolUserInputRespond => "item/tool/requestUserInput:respond",
            Self::ServerRequestAuthRefreshRespond => "account/chatgptAuthTokens/refresh:respond",
            Self::AccountRead => "account/read",
            Self::AccountLoginStart => "account/login/start",
            Self::AccountLoginCancel => "account/login/cancel",
            Self::AccountLogout => "account/logout",
            Self::AccountRateLimitsRead => "account/rateLimits/read",
            Self::ModelList => "model/list",
            Self::ConfigRead => "config/read",
            Self::ConfigRequirementsRead => "configRequirements/read",
            Self::ConfigValueWrite => "config/value/write",
            Self::ConfigBatchWrite => "config/batchWrite",
            Self::ExternalAgentConfigDetect => "externalAgentConfig/detect",
            Self::ExternalAgentConfigImport => "externalAgentConfig/import",
            Self::McpServerStatusList => "mcpServerStatus/list",
            Self::McpServerOauthLogin => "mcpServer/oauth/login",
            Self::McpServerReload => "config/mcpServer/reload",
            Self::AppsList => "app/list",
            Self::ReviewStart => "review/start",
            Self::CommandExec => "command/exec",
            Self::CollaborationModeList => "collaborationMode/list",
            Self::ExperimentalFeatureList => "experimentalFeature/list",
            Self::ThreadRealtimeStart => "thread/realtime/start",
            Self::ThreadRealtimeAppendText => "thread/realtime/appendText",
            Self::ThreadRealtimeStop => "thread/realtime/stop",
            Self::WindowsSandboxSetupStart => "windowsSandbox/setupStart",
            Self::FuzzyFileSearchSessionStart => "fuzzyFileSearch/sessionStart",
            Self::FuzzyFileSearchSessionUpdate => "fuzzyFileSearch/sessionUpdate",
            Self::FuzzyFileSearchSessionStop => "fuzzyFileSearch/sessionStop",
            Self::SkillsRemoteList => "skills/remote/list",
            Self::SkillsRemoteExport => "skills/remote/export",
            Self::SkillsList => "skills/list",
            Self::SkillsConfigWrite => "skills/config/write",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CodexLaneCommandStatus {
    Accepted,
    Rejected,
    Retryable,
}

impl CodexLaneCommandStatus {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Accepted => "accepted",
            Self::Rejected => "rejected",
            Self::Retryable => "retryable",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CodexLaneCommandResponse {
    pub command_seq: u64,
    pub command: CodexLaneCommandKind,
    pub status: CodexLaneCommandStatus,
    pub error: Option<String>,
}

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub enum CodexLaneCommand {
    ThreadStart(ThreadStartParams),
    ThreadResume(ThreadResumeParams),
    ThreadFork(ThreadForkParams),
    ThreadArchive(ThreadArchiveParams),
    ThreadUnsubscribe(ThreadUnsubscribeParams),
    ThreadNameSet(ThreadSetNameParams),
    ThreadUnarchive(ThreadUnarchiveParams),
    ThreadCompactStart(ThreadCompactStartParams),
    ThreadRollback(ThreadRollbackParams),
    ThreadRead(ThreadReadParams),
    ThreadList(ThreadListParams),
    ThreadLoadedList(ThreadLoadedListParams),
    TurnStart(TurnStartParams),
    TurnSteer(TurnSteerParams),
    TurnInterrupt(TurnInterruptParams),
    ServerRequestCommandApprovalRespond {
        request_id: AppServerRequestId,
        response: CommandExecutionRequestApprovalResponse,
    },
    ServerRequestFileApprovalRespond {
        request_id: AppServerRequestId,
        response: FileChangeRequestApprovalResponse,
    },
    ServerRequestToolCallRespond {
        request_id: AppServerRequestId,
        response: DynamicToolCallResponse,
    },
    ServerRequestToolUserInputRespond {
        request_id: AppServerRequestId,
        response: ToolRequestUserInputResponse,
    },
    ServerRequestAuthRefreshRespond {
        request_id: AppServerRequestId,
        response: ChatgptAuthTokensRefreshResponse,
    },
    AccountRead(GetAccountParams),
    AccountLoginStart(LoginAccountParams),
    AccountLoginCancel(CancelLoginAccountParams),
    AccountLogout,
    AccountRateLimitsRead,
    ModelList(ModelListParams),
    ConfigRead(ConfigReadParams),
    ConfigRequirementsRead,
    ConfigValueWrite(ConfigValueWriteParams),
    ConfigBatchWrite(ConfigBatchWriteParams),
    ExternalAgentConfigDetect(ExternalAgentConfigDetectParams),
    ExternalAgentConfigImport(ExternalAgentConfigImportParams),
    McpServerStatusList(ListMcpServerStatusParams),
    McpServerOauthLogin(McpServerOauthLoginParams),
    McpServerReload,
    AppsList(AppsListParams),
    ReviewStart(ReviewStartParams),
    CommandExec(CommandExecParams),
    CollaborationModeList(CollaborationModeListParams),
    ExperimentalFeatureList(ExperimentalFeatureListParams),
    ThreadRealtimeStart(ThreadRealtimeStartParams),
    ThreadRealtimeAppendText(ThreadRealtimeAppendTextParams),
    ThreadRealtimeStop(ThreadRealtimeStopParams),
    WindowsSandboxSetupStart(WindowsSandboxSetupStartParams),
    FuzzyFileSearchSessionStart(FuzzyFileSearchSessionStartParams),
    FuzzyFileSearchSessionUpdate(FuzzyFileSearchSessionUpdateParams),
    FuzzyFileSearchSessionStop(FuzzyFileSearchSessionStopParams),
    SkillsRemoteList(SkillsRemoteReadParams),
    SkillsRemoteExport(SkillsRemoteWriteParams),
    SkillsList(SkillsListParams),
    SkillsConfigWrite(SkillsConfigWriteParams),
}

impl CodexLaneCommand {
    fn kind(&self) -> CodexLaneCommandKind {
        match self {
            Self::ThreadStart(_) => CodexLaneCommandKind::ThreadStart,
            Self::ThreadResume(_) => CodexLaneCommandKind::ThreadResume,
            Self::ThreadFork(_) => CodexLaneCommandKind::ThreadFork,
            Self::ThreadArchive(_) => CodexLaneCommandKind::ThreadArchive,
            Self::ThreadUnsubscribe(_) => CodexLaneCommandKind::ThreadUnsubscribe,
            Self::ThreadNameSet(_) => CodexLaneCommandKind::ThreadNameSet,
            Self::ThreadUnarchive(_) => CodexLaneCommandKind::ThreadUnarchive,
            Self::ThreadCompactStart(_) => CodexLaneCommandKind::ThreadCompactStart,
            Self::ThreadRollback(_) => CodexLaneCommandKind::ThreadRollback,
            Self::ThreadRead(_) => CodexLaneCommandKind::ThreadRead,
            Self::ThreadList(_) => CodexLaneCommandKind::ThreadList,
            Self::ThreadLoadedList(_) => CodexLaneCommandKind::ThreadLoadedList,
            Self::TurnStart(_) => CodexLaneCommandKind::TurnStart,
            Self::TurnSteer(_) => CodexLaneCommandKind::TurnSteer,
            Self::TurnInterrupt(_) => CodexLaneCommandKind::TurnInterrupt,
            Self::ServerRequestCommandApprovalRespond { .. } => {
                CodexLaneCommandKind::ServerRequestCommandApprovalRespond
            }
            Self::ServerRequestFileApprovalRespond { .. } => {
                CodexLaneCommandKind::ServerRequestFileApprovalRespond
            }
            Self::ServerRequestToolCallRespond { .. } => {
                CodexLaneCommandKind::ServerRequestToolCallRespond
            }
            Self::ServerRequestToolUserInputRespond { .. } => {
                CodexLaneCommandKind::ServerRequestToolUserInputRespond
            }
            Self::ServerRequestAuthRefreshRespond { .. } => {
                CodexLaneCommandKind::ServerRequestAuthRefreshRespond
            }
            Self::AccountRead(_) => CodexLaneCommandKind::AccountRead,
            Self::AccountLoginStart(_) => CodexLaneCommandKind::AccountLoginStart,
            Self::AccountLoginCancel(_) => CodexLaneCommandKind::AccountLoginCancel,
            Self::AccountLogout => CodexLaneCommandKind::AccountLogout,
            Self::AccountRateLimitsRead => CodexLaneCommandKind::AccountRateLimitsRead,
            Self::ModelList(_) => CodexLaneCommandKind::ModelList,
            Self::ConfigRead(_) => CodexLaneCommandKind::ConfigRead,
            Self::ConfigRequirementsRead => CodexLaneCommandKind::ConfigRequirementsRead,
            Self::ConfigValueWrite(_) => CodexLaneCommandKind::ConfigValueWrite,
            Self::ConfigBatchWrite(_) => CodexLaneCommandKind::ConfigBatchWrite,
            Self::ExternalAgentConfigDetect(_) => CodexLaneCommandKind::ExternalAgentConfigDetect,
            Self::ExternalAgentConfigImport(_) => CodexLaneCommandKind::ExternalAgentConfigImport,
            Self::McpServerStatusList(_) => CodexLaneCommandKind::McpServerStatusList,
            Self::McpServerOauthLogin(_) => CodexLaneCommandKind::McpServerOauthLogin,
            Self::McpServerReload => CodexLaneCommandKind::McpServerReload,
            Self::AppsList(_) => CodexLaneCommandKind::AppsList,
            Self::ReviewStart(_) => CodexLaneCommandKind::ReviewStart,
            Self::CommandExec(_) => CodexLaneCommandKind::CommandExec,
            Self::CollaborationModeList(_) => CodexLaneCommandKind::CollaborationModeList,
            Self::ExperimentalFeatureList(_) => CodexLaneCommandKind::ExperimentalFeatureList,
            Self::ThreadRealtimeStart(_) => CodexLaneCommandKind::ThreadRealtimeStart,
            Self::ThreadRealtimeAppendText(_) => CodexLaneCommandKind::ThreadRealtimeAppendText,
            Self::ThreadRealtimeStop(_) => CodexLaneCommandKind::ThreadRealtimeStop,
            Self::WindowsSandboxSetupStart(_) => CodexLaneCommandKind::WindowsSandboxSetupStart,
            Self::FuzzyFileSearchSessionStart(_) => {
                CodexLaneCommandKind::FuzzyFileSearchSessionStart
            }
            Self::FuzzyFileSearchSessionUpdate(_) => {
                CodexLaneCommandKind::FuzzyFileSearchSessionUpdate
            }
            Self::FuzzyFileSearchSessionStop(_) => CodexLaneCommandKind::FuzzyFileSearchSessionStop,
            Self::SkillsRemoteList(_) => CodexLaneCommandKind::SkillsRemoteList,
            Self::SkillsRemoteExport(_) => CodexLaneCommandKind::SkillsRemoteExport,
            Self::SkillsList(_) => CodexLaneCommandKind::SkillsList,
            Self::SkillsConfigWrite(_) => CodexLaneCommandKind::SkillsConfigWrite,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CodexLaneNotification {
    SkillsListLoaded {
        entries: Vec<CodexSkillListEntry>,
    },
    ModelsLoaded {
        models: Vec<String>,
        default_model: Option<String>,
    },
    ModelCatalogLoaded {
        entries: Vec<CodexModelCatalogEntry>,
        include_hidden: bool,
        default_model: Option<String>,
    },
    ModelRerouted {
        thread_id: String,
        turn_id: String,
        from_model: String,
        to_model: String,
        reason: String,
    },
    AccountLoaded {
        summary: String,
        requires_openai_auth: bool,
    },
    AccountRateLimitsLoaded {
        summary: String,
    },
    AccountUpdated {
        auth_mode: Option<String>,
    },
    AccountLoginStarted {
        login_id: Option<String>,
        auth_url: Option<String>,
    },
    AccountLoginCompleted {
        login_id: Option<String>,
        success: bool,
        error: Option<String>,
    },
    ConfigLoaded {
        config: String,
        origins: String,
        layers: String,
    },
    ConfigRequirementsLoaded {
        requirements: String,
    },
    ConfigWriteApplied {
        status: String,
        version: String,
    },
    ExternalAgentConfigDetected {
        count: usize,
    },
    ExternalAgentConfigImported,
    McpServerStatusListLoaded {
        entries: Vec<CodexMcpServerStatusEntry>,
        next_cursor: Option<String>,
    },
    McpServerOauthLoginStarted {
        server_name: String,
        authorization_url: String,
    },
    McpServerOauthLoginCompleted {
        server_name: String,
        success: bool,
        error: Option<String>,
    },
    McpServerReloaded,
    AppsListLoaded {
        entries: Vec<CodexAppEntry>,
        next_cursor: Option<String>,
    },
    AppsListUpdated,
    ReviewStarted {
        thread_id: String,
        turn_id: String,
        review_thread_id: String,
        delivery: String,
        target: String,
    },
    CommandExecCompleted {
        exit_code: i32,
        stdout: String,
        stderr: String,
    },
    CollaborationModesLoaded {
        modes_json: String,
        count: usize,
    },
    ExperimentalFeaturesLoaded {
        features_json: String,
        count: usize,
        next_cursor: Option<String>,
    },
    RealtimeStarted {
        thread_id: String,
        session_id: Option<String>,
    },
    RealtimeTextAppended {
        thread_id: String,
        text_len: usize,
    },
    RealtimeStopped {
        thread_id: String,
    },
    RealtimeError {
        thread_id: String,
        message: String,
    },
    WindowsSandboxSetupStarted {
        mode: String,
        started: bool,
    },
    WindowsSandboxSetupCompleted {
        mode: Option<String>,
        success: Option<bool>,
    },
    FuzzySessionStarted {
        session_id: String,
    },
    FuzzySessionUpdated {
        session_id: String,
        status: String,
    },
    FuzzySessionCompleted {
        session_id: String,
    },
    FuzzySessionStopped {
        session_id: String,
    },
    SkillsRemoteListLoaded {
        entries: Vec<CodexRemoteSkillEntry>,
    },
    SkillsRemoteExported {
        id: String,
        path: String,
    },
    ThreadListLoaded {
        entries: Vec<CodexThreadListEntry>,
    },
    ThreadLoadedListLoaded {
        thread_ids: Vec<String>,
    },
    ThreadReadLoaded {
        thread_id: String,
        messages: Vec<CodexThreadTranscriptMessage>,
        latest_plan: Option<CodexThreadPlanArtifact>,
        latest_review: Option<CodexThreadReviewArtifact>,
        latest_compaction: Option<CodexThreadCompactionArtifact>,
    },
    ThreadSelected {
        thread_id: String,
    },
    ThreadStarted {
        thread_id: String,
        model: Option<String>,
        cwd: Option<String>,
        approval_policy: Option<AskForApproval>,
        sandbox_mode: Option<SandboxMode>,
        service_tier: Option<ServiceTier>,
        reasoning_effort: Option<String>,
    },
    ThreadSessionConfigured {
        thread_id: String,
        model: Option<String>,
        cwd: Option<String>,
        approval_policy: Option<AskForApproval>,
        sandbox_mode: Option<SandboxMode>,
        service_tier: Option<ServiceTier>,
        reasoning_effort: Option<String>,
    },
    ThreadStatusChanged {
        thread_id: String,
        status: String,
    },
    ThreadArchived {
        thread_id: String,
    },
    ThreadUnarchived {
        thread_id: String,
    },
    ThreadClosed {
        thread_id: String,
    },
    ThreadNameUpdated {
        thread_id: String,
        thread_name: Option<String>,
    },
    TurnStarted {
        thread_id: String,
        turn_id: String,
    },
    ItemStarted {
        thread_id: String,
        turn_id: String,
        item_id: Option<String>,
        item_type: Option<String>,
    },
    ItemCompleted {
        thread_id: String,
        turn_id: String,
        item_id: Option<String>,
        item_type: Option<String>,
        message: Option<String>,
    },
    AgentMessageDelta {
        thread_id: String,
        turn_id: String,
        item_id: String,
        delta: String,
    },
    AgentMessageCompleted {
        thread_id: String,
        turn_id: String,
        item_id: Option<String>,
        message: String,
    },
    ReasoningDelta {
        thread_id: String,
        turn_id: String,
        item_id: Option<String>,
        delta: String,
    },
    TurnCompleted {
        thread_id: String,
        turn_id: String,
        status: Option<String>,
        error_message: Option<String>,
        final_message: Option<String>,
    },
    TurnDiffUpdated {
        thread_id: String,
        turn_id: String,
        diff: String,
    },
    ReviewProgressUpdated {
        thread_id: String,
        turn_id: String,
        review: String,
        completed: bool,
    },
    TurnPlanUpdated {
        thread_id: String,
        turn_id: String,
        explanation: Option<String>,
        plan: Vec<CodexTurnPlanStep>,
    },
    ThreadCompacted {
        thread_id: String,
        turn_id: String,
    },
    ThreadTokenUsageUpdated {
        thread_id: String,
        turn_id: String,
        input_tokens: i64,
        cached_input_tokens: i64,
        output_tokens: i64,
    },
    TurnError {
        thread_id: String,
        turn_id: String,
        message: String,
    },
    CommandApprovalRequested {
        request_id: AppServerRequestId,
        request: CodexCommandApprovalRequest,
    },
    FileChangeApprovalRequested {
        request_id: AppServerRequestId,
        request: CodexFileChangeApprovalRequest,
    },
    ToolCallRequested {
        request_id: AppServerRequestId,
        request: CodexToolCallRequest,
    },
    ToolUserInputRequested {
        request_id: AppServerRequestId,
        request: CodexToolUserInputRequest,
    },
    AuthTokensRefreshRequested {
        request_id: AppServerRequestId,
        request: CodexAuthTokensRefreshRequest,
    },
    ServerRequest {
        method: String,
    },
    Raw {
        method: String,
    },
}

#[derive(Clone, Debug)]
pub enum CodexLaneUpdate {
    Snapshot(Box<CodexLaneSnapshot>),
    CommandResponse(CodexLaneCommandResponse),
    Notification(CodexLaneNotification),
}

struct SequencedCodexCommand {
    command_seq: u64,
    command: CodexLaneCommand,
}

enum CodexLaneControl {
    Command(Box<SequencedCodexCommand>),
    Shutdown,
}

pub struct CodexLaneWorker {
    command_tx: Sender<CodexLaneControl>,
    update_rx: Receiver<CodexLaneUpdate>,
    join_handle: Option<JoinHandle<()>>,
    shutdown_sent: bool,
}

impl CodexLaneWorker {
    pub fn spawn(config: CodexLaneConfig) -> Self {
        Self::spawn_with_runtime(config, Box::new(ProcessCodexLaneRuntime))
    }

    fn spawn_with_runtime(
        config: CodexLaneConfig,
        runtime_impl: Box<dyn CodexLaneRuntime>,
    ) -> Self {
        let (command_tx, command_rx) = mpsc::channel::<CodexLaneControl>();
        let (update_tx, update_rx) = mpsc::channel::<CodexLaneUpdate>();

        let join_handle = std::thread::spawn(move || {
            run_codex_lane_loop(command_rx, update_tx, config, runtime_impl);
        });

        Self {
            command_tx,
            update_rx,
            join_handle: Some(join_handle),
            shutdown_sent: false,
        }
    }

    pub fn enqueue(&self, command_seq: u64, command: CodexLaneCommand) -> Result<(), String> {
        self.command_tx
            .send(CodexLaneControl::Command(Box::new(SequencedCodexCommand {
                command_seq,
                command,
            })))
            .map_err(|error| format!("Codex lane offline: {error}"))
    }

    pub fn drain_updates(&mut self) -> Vec<CodexLaneUpdate> {
        let mut updates = Vec::new();
        while let Ok(update) = self.update_rx.try_recv() {
            updates.push(update);
        }
        updates
    }

    pub fn shutdown(&mut self) {
        if self.shutdown_sent {
            return;
        }
        self.shutdown_sent = true;
        let _ = self.command_tx.send(CodexLaneControl::Shutdown);
        if let Some(join_handle) = self.join_handle.take() {
            let _ = join_handle.join();
        }
    }

    pub fn shutdown_async(&mut self) {
        if self.shutdown_sent {
            return;
        }
        self.shutdown_sent = true;
        let _ = self.command_tx.send(CodexLaneControl::Shutdown);
        if let Some(join_handle) = self.join_handle.take() {
            std::thread::spawn(move || {
                let _ = join_handle.join();
            });
        }
    }
}

impl Drop for CodexLaneWorker {
    fn drop(&mut self) {
        self.shutdown();
    }
}

trait CodexLaneRuntime: Send {
    fn connect(
        &mut self,
        runtime: &Runtime,
        config: &CodexLaneConfig,
    ) -> Result<(AppServerClient, AppServerChannels)>;
}

struct ProcessCodexLaneRuntime;

impl CodexLaneRuntime for ProcessCodexLaneRuntime {
    fn connect(
        &mut self,
        runtime: &Runtime,
        config: &CodexLaneConfig,
    ) -> Result<(AppServerClient, AppServerChannels)> {
        let wire_log = config.wire_log_path.as_ref().map(|path| {
            let wire_log = codex_client::AppServerWireLog::new();
            wire_log.set_path(path.clone());
            wire_log
        });
        runtime
            .block_on(AppServerClient::spawn(AppServerConfig {
                cwd: config.cwd.clone(),
                wire_log,
                env: config.env.clone(),
            }))
            .context("failed to spawn codex app-server")
    }
}

struct CodexLaneState {
    snapshot: CodexLaneSnapshot,
    client: Option<AppServerClient>,
    channels: Option<AppServerChannels>,
    pending_server_requests: HashMap<AppServerRequestId, String>,
}

struct CodexCommandEffect {
    active_thread_id: Option<String>,
    notification: Option<CodexLaneNotification>,
}

impl CodexLaneState {
    fn new() -> Self {
        let mut snapshot = CodexLaneSnapshot::default();
        snapshot.install_probe = codex_client::probe_codex_installation();
        Self {
            snapshot,
            client: None,
            channels: None,
            pending_server_requests: HashMap::new(),
        }
    }

    fn publish_snapshot(&self, update_tx: &Sender<CodexLaneUpdate>) {
        let _ = update_tx.send(CodexLaneUpdate::Snapshot(Box::new(self.snapshot.clone())));
    }

    fn set_error(
        &mut self,
        update_tx: &Sender<CodexLaneUpdate>,
        message: impl Into<String>,
        disconnected: bool,
    ) {
        let message = message.into();
        tracing::info!("codex lane error: {}", message);
        self.snapshot.lifecycle = if disconnected {
            CodexLaneLifecycle::Disconnected
        } else {
            CodexLaneLifecycle::Error
        };
        self.snapshot.last_error = Some(message.clone());
        self.snapshot.last_status = Some(message);
        self.publish_snapshot(update_tx);
    }

    fn set_ready(&mut self, update_tx: &Sender<CodexLaneUpdate>, status: impl Into<String>) {
        self.snapshot.lifecycle = CodexLaneLifecycle::Ready;
        self.snapshot.last_error = None;
        self.snapshot.last_status = Some(status.into());
        self.publish_snapshot(update_tx);
    }

    fn handle_connect(
        &mut self,
        runtime: &Runtime,
        config: &CodexLaneConfig,
        update_tx: &Sender<CodexLaneUpdate>,
        runtime_impl: &mut dyn CodexLaneRuntime,
    ) {
        let connected = runtime_impl.connect(runtime, config);
        let (client, channels) = match connected {
            Ok(value) => value,
            Err(error) => {
                self.set_error(
                    update_tx,
                    format!("Codex lane startup failed: {error}"),
                    false,
                );
                return;
            }
        };

        self.client = Some(client);
        self.channels = Some(channels);

        let capabilities = if config.experimental_api
            || !config.opt_out_notification_methods.is_empty()
        {
            Some(InitializeCapabilities {
                experimental_api: config.experimental_api,
                opt_out_notification_methods: if config.opt_out_notification_methods.is_empty() {
                    None
                } else {
                    Some(config.opt_out_notification_methods.clone())
                },
            })
        } else {
            None
        };
        let initialized = {
            let Some(client) = self.client.as_ref() else {
                self.set_error(update_tx, "Codex lane unavailable after connect", false);
                return;
            };
            runtime.block_on(client.initialize(InitializeParams {
                client_info: config.client_info.clone(),
                capabilities,
            }))
        };
        if let Err(error) = initialized {
            self.set_error(
                update_tx,
                format!("Codex lane initialize failed: {error}"),
                false,
            );
            return;
        }

        self.set_ready(update_tx, "Codex lane ready");
        self.publish_models_from_server(runtime, update_tx);
        let thread_count = self.publish_threads_from_server(runtime, update_tx);
        if thread_count == 0 && config.bootstrap_thread {
            let thread_start = ThreadStartParams {
                model: config.bootstrap_model.clone(),
                model_provider: None,
                service_tier: Some(None),
                cwd: config.cwd.as_ref().map(|path| path.display().to_string()),
                approval_policy: config.approval_policy,
                sandbox: Some(SandboxMode::DangerFullAccess),
                personality: None,
                ephemeral: None,
                dynamic_tools: Some(
                    crate::openagents_dynamic_tools::openagents_dynamic_tool_specs(),
                ),
            };
            let started = {
                let Some(client) = self.client.as_ref() else {
                    self.set_error(
                        update_tx,
                        "Codex lane unavailable for bootstrap thread",
                        false,
                    );
                    return;
                };
                runtime.block_on(client.thread_start(thread_start))
            };
            match started {
                Ok(response) => {
                    let thread_id = response.thread.id;
                    self.snapshot.active_thread_id = Some(thread_id.clone());
                    self.publish_snapshot(update_tx);
                    let _ = update_tx.send(CodexLaneUpdate::Notification(
                        CodexLaneNotification::ThreadStarted {
                            thread_id: thread_id.clone(),
                            model: Some(response.model),
                            cwd: response.cwd.map(|value| value.display().to_string()),
                            approval_policy: response.approval_policy,
                            sandbox_mode: sandbox_mode_from_policy(
                                response.sandbox_policy.as_ref(),
                            ),
                            service_tier: response.service_tier,
                            reasoning_effort: reasoning_effort_label(response.reasoning_effort),
                        },
                    ));
                    let _ = update_tx.send(CodexLaneUpdate::Notification(
                        CodexLaneNotification::ThreadListLoaded {
                            entries: vec![CodexThreadListEntry {
                                thread_id,
                                thread_name: None,
                                preview: String::new(),
                                status: Some("idle".to_string()),
                                loaded: true,
                                cwd: config.cwd.as_ref().map(|value| value.display().to_string()),
                                path: None,
                                created_at: 0,
                                updated_at: 0,
                            }],
                        },
                    ));
                }
                Err(error) => {
                    self.set_error(
                        update_tx,
                        format!("Codex lane bootstrap thread failed: {error}"),
                        false,
                    );
                }
            }
        }
    }

    fn publish_models_from_server(&self, runtime: &Runtime, update_tx: &Sender<CodexLaneUpdate>) {
        let Some(client) = self.client.as_ref() else {
            return;
        };

        let Ok((models, default_model)) = fetch_model_catalog(runtime, client) else {
            return;
        };
        if models.is_empty() {
            return;
        }

        let _ = update_tx.send(CodexLaneUpdate::Notification(
            CodexLaneNotification::ModelsLoaded {
                models,
                default_model,
            },
        ));
    }

    fn publish_threads_from_server(
        &mut self,
        runtime: &Runtime,
        update_tx: &Sender<CodexLaneUpdate>,
    ) -> usize {
        let Some(client) = self.client.as_ref() else {
            return 0;
        };

        let mut thread_count = 0;

        if let Ok(response) = runtime.block_on(client.thread_list(ThreadListParams {
            cursor: None,
            limit: Some(100),
            ..ThreadListParams::default()
        })) {
            let entries = response
                .data
                .into_iter()
                .map(|thread| CodexThreadListEntry {
                    thread_id: thread.id,
                    thread_name: thread.name,
                    preview: thread.preview,
                    status: thread.status.as_ref().and_then(thread_status_label),
                    loaded: false,
                    cwd: thread.cwd.map(|value| value.display().to_string()),
                    path: thread.path.map(|value| value.display().to_string()),
                    created_at: thread.created_at,
                    updated_at: thread.updated_at,
                })
                .collect::<Vec<_>>();
            thread_count = entries.len();
            let first_thread = entries.first().map(|entry| entry.thread_id.clone());
            let _ = update_tx.send(CodexLaneUpdate::Notification(
                CodexLaneNotification::ThreadListLoaded { entries },
            ));
            if let Some(thread_id) = first_thread {
                self.snapshot.active_thread_id = Some(thread_id.clone());
                self.publish_snapshot(update_tx);
                let _ = update_tx.send(CodexLaneUpdate::Notification(
                    CodexLaneNotification::ThreadSelected { thread_id },
                ));
            }
        }

        if let Ok(response) = runtime.block_on(client.thread_loaded_list(ThreadLoadedListParams {
            cursor: None,
            limit: Some(200),
        })) {
            let _ = update_tx.send(CodexLaneUpdate::Notification(
                CodexLaneNotification::ThreadLoadedListLoaded {
                    thread_ids: response.data,
                },
            ));
        }

        thread_count
    }

    fn handle_command(
        &mut self,
        runtime: &Runtime,
        envelope: SequencedCodexCommand,
        update_tx: &Sender<CodexLaneUpdate>,
    ) {
        let kind = envelope.command.kind();
        let mut response = CodexLaneCommandResponse {
            command_seq: envelope.command_seq,
            command: kind,
            status: CodexLaneCommandStatus::Accepted,
            error: None,
        };

        let result = if let Some(client) = self.client.take() {
            let result = self.dispatch_command(runtime, &client, envelope.command);
            self.client = Some(client);
            result
        } else {
            Err(anyhow::anyhow!("Codex lane unavailable"))
        };

        match result {
            Ok(effect) => {
                if let Some(thread_id) = effect.active_thread_id {
                    self.snapshot.active_thread_id = Some(thread_id);
                    self.publish_snapshot(update_tx);
                }
                if let Some(notification) = effect.notification {
                    let _ = update_tx.send(CodexLaneUpdate::Notification(notification));
                }
            }
            Err(error) => {
                let message = error.to_string();
                if kind == CodexLaneCommandKind::ThreadRead
                    && is_pre_materialization_thread_read_error(&message)
                {
                    tracing::info!(
                        "codex thread/read unavailable before first user message: {}",
                        message
                    );
                } else if is_disconnect_error(&error) {
                    self.client = None;
                    self.channels = None;
                    self.set_error(
                        update_tx,
                        format!("Codex lane disconnected: {message}"),
                        true,
                    );
                    response.status = CodexLaneCommandStatus::Retryable;
                } else if self.client.is_none() {
                    response.status = CodexLaneCommandStatus::Retryable;
                    self.set_error(
                        update_tx,
                        format!("Codex lane unavailable: {message}"),
                        false,
                    );
                } else {
                    response.status = CodexLaneCommandStatus::Rejected;
                    self.set_error(
                        update_tx,
                        format!("Codex lane command failed: {message}"),
                        false,
                    );
                }
                response.error = Some(message);
            }
        }

        let _ = update_tx.send(CodexLaneUpdate::CommandResponse(response));
    }

    fn dispatch_command(
        &mut self,
        runtime: &Runtime,
        client: &AppServerClient,
        command: CodexLaneCommand,
    ) -> Result<CodexCommandEffect> {
        match command {
            CodexLaneCommand::ThreadStart(params) => {
                let response = runtime.block_on(client.thread_start(params))?;
                let thread_id = response.thread.id;
                Ok(CodexCommandEffect {
                    active_thread_id: Some(thread_id.clone()),
                    notification: Some(CodexLaneNotification::ThreadStarted {
                        thread_id,
                        model: Some(response.model),
                        cwd: response.cwd.map(|value| value.display().to_string()),
                        approval_policy: response.approval_policy,
                        sandbox_mode: sandbox_mode_from_policy(response.sandbox_policy.as_ref()),
                        service_tier: response.service_tier,
                        reasoning_effort: reasoning_effort_label(response.reasoning_effort),
                    }),
                })
            }
            CodexLaneCommand::ThreadResume(params) => {
                let thread_id = params.thread_id.clone();
                let response = runtime.block_on(client.thread_resume(params))?;
                Ok(CodexCommandEffect {
                    active_thread_id: None,
                    notification: Some(CodexLaneNotification::ThreadSessionConfigured {
                        thread_id,
                        model: Some(response.model),
                        cwd: response.cwd.map(|value| value.display().to_string()),
                        approval_policy: response.approval_policy,
                        sandbox_mode: sandbox_mode_from_policy(response.sandbox_policy.as_ref()),
                        service_tier: response.service_tier,
                        reasoning_effort: reasoning_effort_label(response.reasoning_effort),
                    }),
                })
            }
            CodexLaneCommand::ThreadFork(params) => {
                let response = runtime.block_on(client.thread_fork(params))?;
                let thread_id = response.thread.id;
                Ok(CodexCommandEffect {
                    active_thread_id: Some(thread_id.clone()),
                    notification: Some(CodexLaneNotification::ThreadSelected { thread_id }),
                })
            }
            CodexLaneCommand::ThreadArchive(params) => {
                let thread_id = params.thread_id.clone();
                let _ = runtime.block_on(client.thread_archive(params))?;
                Ok(CodexCommandEffect {
                    active_thread_id: None,
                    notification: Some(CodexLaneNotification::ThreadArchived { thread_id }),
                })
            }
            CodexLaneCommand::ThreadUnsubscribe(params) => {
                let thread_id = params.thread_id.clone();
                let _ = runtime.block_on(client.thread_unsubscribe(params))?;
                Ok(CodexCommandEffect {
                    active_thread_id: None,
                    notification: Some(CodexLaneNotification::ThreadClosed { thread_id }),
                })
            }
            CodexLaneCommand::ThreadNameSet(params) => {
                let thread_id = params.thread_id.clone();
                let thread_name = Some(params.name.clone());
                let _ = runtime.block_on(client.thread_name_set(params))?;
                Ok(CodexCommandEffect {
                    active_thread_id: None,
                    notification: Some(CodexLaneNotification::ThreadNameUpdated {
                        thread_id,
                        thread_name,
                    }),
                })
            }
            CodexLaneCommand::ThreadUnarchive(params) => {
                let response = runtime.block_on(client.thread_unarchive(params))?;
                let thread_id = response.thread.id;
                Ok(CodexCommandEffect {
                    active_thread_id: None,
                    notification: Some(CodexLaneNotification::ThreadUnarchived { thread_id }),
                })
            }
            CodexLaneCommand::ThreadCompactStart(params) => {
                let _ = runtime.block_on(client.thread_compact_start(params))?;
                Ok(CodexCommandEffect {
                    active_thread_id: None,
                    notification: None,
                })
            }
            CodexLaneCommand::ThreadRollback(params) => {
                let response = runtime.block_on(client.thread_rollback(params))?;
                let thread_id = response.thread.id;
                Ok(CodexCommandEffect {
                    active_thread_id: Some(thread_id.clone()),
                    notification: Some(CodexLaneNotification::ThreadSelected { thread_id }),
                })
            }
            CodexLaneCommand::ThreadRead(params) => {
                let response = runtime.block_on(client.thread_read(params))?;
                let thread_id = response.thread.id.clone();
                let messages = extract_thread_transcript_messages(&response.thread);
                let latest_plan = extract_latest_thread_plan_artifact(&response.thread);
                let latest_review = extract_latest_thread_review_artifact(&response.thread);
                let latest_compaction =
                    extract_latest_thread_compaction_artifact(&response.thread);
                Ok(CodexCommandEffect {
                    active_thread_id: None,
                    notification: Some(CodexLaneNotification::ThreadReadLoaded {
                        thread_id,
                        messages,
                        latest_plan,
                        latest_review,
                        latest_compaction,
                    }),
                })
            }
            CodexLaneCommand::ThreadList(params) => {
                let response = runtime.block_on(client.thread_list(params))?;
                let entries = response
                    .data
                    .into_iter()
                    .map(|thread| CodexThreadListEntry {
                        thread_id: thread.id,
                        thread_name: thread.name,
                        preview: thread.preview,
                        status: thread.status.as_ref().and_then(thread_status_label),
                        loaded: false,
                        cwd: thread.cwd.map(|value| value.display().to_string()),
                        path: thread.path.map(|value| value.display().to_string()),
                        created_at: thread.created_at,
                        updated_at: thread.updated_at,
                    })
                    .collect();
                Ok(CodexCommandEffect {
                    active_thread_id: None,
                    notification: Some(CodexLaneNotification::ThreadListLoaded { entries }),
                })
            }
            CodexLaneCommand::ThreadLoadedList(params) => {
                let response = runtime.block_on(client.thread_loaded_list(params))?;
                Ok(CodexCommandEffect {
                    active_thread_id: None,
                    notification: Some(CodexLaneNotification::ThreadLoadedListLoaded {
                        thread_ids: response.data,
                    }),
                })
            }
            CodexLaneCommand::TurnStart(params) => {
                let thread_id = params.thread_id.clone();
                let response = runtime.block_on(client.turn_start(params))?;
                Ok(CodexCommandEffect {
                    active_thread_id: None,
                    notification: Some(CodexLaneNotification::TurnStarted {
                        thread_id,
                        turn_id: response.turn.id,
                    }),
                })
            }
            CodexLaneCommand::TurnSteer(params) => {
                let _ = runtime.block_on(client.turn_steer(params))?;
                Ok(CodexCommandEffect {
                    active_thread_id: None,
                    notification: None,
                })
            }
            CodexLaneCommand::TurnInterrupt(params) => {
                let _ = runtime.block_on(client.turn_interrupt(params))?;
                Ok(CodexCommandEffect {
                    active_thread_id: None,
                    notification: None,
                })
            }
            CodexLaneCommand::ServerRequestCommandApprovalRespond {
                request_id,
                response,
            } => {
                self.respond_to_server_request_value(
                    runtime,
                    client,
                    request_id,
                    "item/commandExecution/requestApproval",
                    serde_json::to_value(response)?,
                )?;
                Ok(CodexCommandEffect {
                    active_thread_id: None,
                    notification: None,
                })
            }
            CodexLaneCommand::ServerRequestFileApprovalRespond {
                request_id,
                response,
            } => {
                self.respond_to_server_request_value(
                    runtime,
                    client,
                    request_id,
                    "item/fileChange/requestApproval",
                    serde_json::to_value(response)?,
                )?;
                Ok(CodexCommandEffect {
                    active_thread_id: None,
                    notification: None,
                })
            }
            CodexLaneCommand::ServerRequestToolCallRespond {
                request_id,
                response,
            } => {
                self.respond_to_server_request_value(
                    runtime,
                    client,
                    request_id,
                    "item/tool/call",
                    serde_json::to_value(response)?,
                )?;
                Ok(CodexCommandEffect {
                    active_thread_id: None,
                    notification: None,
                })
            }
            CodexLaneCommand::ServerRequestToolUserInputRespond {
                request_id,
                response,
            } => {
                self.respond_to_server_request_value(
                    runtime,
                    client,
                    request_id,
                    "item/tool/requestUserInput",
                    serde_json::to_value(response)?,
                )?;
                Ok(CodexCommandEffect {
                    active_thread_id: None,
                    notification: None,
                })
            }
            CodexLaneCommand::ServerRequestAuthRefreshRespond {
                request_id,
                response,
            } => {
                self.respond_to_server_request_value(
                    runtime,
                    client,
                    request_id,
                    "account/chatgptAuthTokens/refresh",
                    serde_json::to_value(response)?,
                )?;
                Ok(CodexCommandEffect {
                    active_thread_id: None,
                    notification: None,
                })
            }
            CodexLaneCommand::AccountRead(params) => {
                let response = runtime.block_on(client.account_read(params))?;
                Ok(CodexCommandEffect {
                    active_thread_id: None,
                    notification: Some(CodexLaneNotification::AccountLoaded {
                        summary: account_summary(&response),
                        requires_openai_auth: response.requires_openai_auth,
                    }),
                })
            }
            CodexLaneCommand::AccountLoginStart(params) => {
                let response = runtime.block_on(client.account_login_start(params))?;
                let (login_id, auth_url) = match response {
                    codex_client::LoginAccountResponse::ApiKey => (None, None),
                    codex_client::LoginAccountResponse::Chatgpt { login_id, auth_url } => {
                        (Some(login_id), Some(auth_url))
                    }
                };
                Ok(CodexCommandEffect {
                    active_thread_id: None,
                    notification: Some(CodexLaneNotification::AccountLoginStarted {
                        login_id,
                        auth_url,
                    }),
                })
            }
            CodexLaneCommand::AccountLoginCancel(params) => {
                let _ = runtime.block_on(client.account_login_cancel(params))?;
                Ok(CodexCommandEffect {
                    active_thread_id: None,
                    notification: None,
                })
            }
            CodexLaneCommand::AccountLogout => {
                let _ = runtime.block_on(client.account_logout())?;
                Ok(CodexCommandEffect {
                    active_thread_id: None,
                    notification: Some(CodexLaneNotification::AccountLoaded {
                        summary: "logged out".to_string(),
                        requires_openai_auth: true,
                    }),
                })
            }
            CodexLaneCommand::AccountRateLimitsRead => {
                let response = runtime.block_on(client.account_rate_limits_read())?;
                let summary = serde_json::to_value(&response.rate_limits)
                    .map(|value| rate_limits_summary(&value))
                    .unwrap_or_else(|_| "rate limits unavailable".to_string());
                Ok(CodexCommandEffect {
                    active_thread_id: None,
                    notification: Some(CodexLaneNotification::AccountRateLimitsLoaded { summary }),
                })
            }
            CodexLaneCommand::ModelList(params) => {
                let include_hidden = params.include_hidden.unwrap_or(false);
                let (entries, default_model) =
                    fetch_model_catalog_entries(runtime, client, params)?;
                Ok(CodexCommandEffect {
                    active_thread_id: None,
                    notification: Some(CodexLaneNotification::ModelCatalogLoaded {
                        entries,
                        include_hidden,
                        default_model,
                    }),
                })
            }
            CodexLaneCommand::ConfigRead(params) => {
                let response = runtime.block_on(client.config_read(params))?;
                let config = serde_json::to_string_pretty(&response.config)
                    .unwrap_or_else(|_| "{}".to_string());
                let origins = serde_json::to_string_pretty(&response.origins)
                    .unwrap_or_else(|_| "{}".to_string());
                let layers = response
                    .layers
                    .as_ref()
                    .map(|value| {
                        serde_json::to_string_pretty(value).unwrap_or_else(|_| "[]".to_string())
                    })
                    .unwrap_or_else(|| "[]".to_string());
                Ok(CodexCommandEffect {
                    active_thread_id: None,
                    notification: Some(CodexLaneNotification::ConfigLoaded {
                        config,
                        origins,
                        layers,
                    }),
                })
            }
            CodexLaneCommand::ConfigRequirementsRead => {
                let response = runtime.block_on(client.config_requirements_read())?;
                let requirements = response
                    .requirements
                    .as_ref()
                    .map(|value| {
                        serde_json::to_string_pretty(value).unwrap_or_else(|_| "{}".to_string())
                    })
                    .unwrap_or_else(|| "null".to_string());
                Ok(CodexCommandEffect {
                    active_thread_id: None,
                    notification: Some(CodexLaneNotification::ConfigRequirementsLoaded {
                        requirements,
                    }),
                })
            }
            CodexLaneCommand::ConfigValueWrite(params) => {
                let response = runtime.block_on(client.config_value_write(params))?;
                Ok(CodexCommandEffect {
                    active_thread_id: None,
                    notification: Some(CodexLaneNotification::ConfigWriteApplied {
                        status: response.status,
                        version: response.version,
                    }),
                })
            }
            CodexLaneCommand::ConfigBatchWrite(params) => {
                let response = runtime.block_on(client.config_batch_write(params))?;
                Ok(CodexCommandEffect {
                    active_thread_id: None,
                    notification: Some(CodexLaneNotification::ConfigWriteApplied {
                        status: response.status,
                        version: response.version,
                    }),
                })
            }
            CodexLaneCommand::ExternalAgentConfigDetect(params) => {
                let response = runtime.block_on(client.external_agent_config_detect(params))?;
                Ok(CodexCommandEffect {
                    active_thread_id: None,
                    notification: Some(CodexLaneNotification::ExternalAgentConfigDetected {
                        count: response.items.len(),
                    }),
                })
            }
            CodexLaneCommand::ExternalAgentConfigImport(params) => {
                let _ = runtime.block_on(client.external_agent_config_import(params))?;
                Ok(CodexCommandEffect {
                    active_thread_id: None,
                    notification: Some(CodexLaneNotification::ExternalAgentConfigImported),
                })
            }
            CodexLaneCommand::McpServerStatusList(params) => {
                let response = runtime.block_on(client.mcp_server_status_list(params))?;
                let entries = response
                    .data
                    .into_iter()
                    .map(|entry| CodexMcpServerStatusEntry {
                        name: entry.name,
                        auth_status: mcp_auth_status_label(entry.auth_status).to_string(),
                        tool_count: entry.tools.len(),
                        resource_count: entry.resources.len(),
                        template_count: entry.resource_templates.len(),
                    })
                    .collect();
                Ok(CodexCommandEffect {
                    active_thread_id: None,
                    notification: Some(CodexLaneNotification::McpServerStatusListLoaded {
                        entries,
                        next_cursor: response.next_cursor,
                    }),
                })
            }
            CodexLaneCommand::McpServerOauthLogin(params) => {
                let server_name = params.name.clone();
                let response = runtime.block_on(client.mcp_server_oauth_login(params))?;
                Ok(CodexCommandEffect {
                    active_thread_id: None,
                    notification: Some(CodexLaneNotification::McpServerOauthLoginStarted {
                        server_name,
                        authorization_url: response.authorization_url,
                    }),
                })
            }
            CodexLaneCommand::McpServerReload => {
                let _ = runtime.block_on(client.mcp_server_reload())?;
                Ok(CodexCommandEffect {
                    active_thread_id: None,
                    notification: Some(CodexLaneNotification::McpServerReloaded),
                })
            }
            CodexLaneCommand::AppsList(params) => {
                let response = runtime.block_on(client.app_list(params))?;
                let entries = response
                    .data
                    .into_iter()
                    .map(|entry| CodexAppEntry {
                        id: entry.id,
                        name: entry.name,
                        description: entry.description,
                        is_accessible: entry.is_accessible,
                        is_enabled: entry.is_enabled,
                    })
                    .collect();
                Ok(CodexCommandEffect {
                    active_thread_id: None,
                    notification: Some(CodexLaneNotification::AppsListLoaded {
                        entries,
                        next_cursor: response.next_cursor,
                    }),
                })
            }
            CodexLaneCommand::ReviewStart(params) => {
                let thread_id = params.thread_id.clone();
                let delivery = review_delivery_label(params.delivery);
                let target = review_target_label(&params.target);
                let response = runtime.block_on(client.review_start(params))?;
                Ok(CodexCommandEffect {
                    active_thread_id: None,
                    notification: Some(CodexLaneNotification::ReviewStarted {
                        thread_id,
                        turn_id: response.turn.id,
                        review_thread_id: response.review_thread_id,
                        delivery,
                        target,
                    }),
                })
            }
            CodexLaneCommand::CommandExec(params) => {
                let response = runtime.block_on(client.command_exec(params))?;
                Ok(CodexCommandEffect {
                    active_thread_id: None,
                    notification: Some(CodexLaneNotification::CommandExecCompleted {
                        exit_code: response.exit_code,
                        stdout: response.stdout,
                        stderr: response.stderr,
                    }),
                })
            }
            CodexLaneCommand::CollaborationModeList(params) => {
                let response = runtime.block_on(client.collaboration_mode_list(params))?;
                let count = response.data.len();
                let modes_json = serde_json::to_string_pretty(&response.data)
                    .unwrap_or_else(|_| "[]".to_string());
                Ok(CodexCommandEffect {
                    active_thread_id: None,
                    notification: Some(CodexLaneNotification::CollaborationModesLoaded {
                        modes_json,
                        count,
                    }),
                })
            }
            CodexLaneCommand::ExperimentalFeatureList(params) => {
                let response = runtime.block_on(client.experimental_feature_list(params))?;
                let count = response.data.len();
                let features_json = serde_json::to_string_pretty(&response.data)
                    .unwrap_or_else(|_| "[]".to_string());
                Ok(CodexCommandEffect {
                    active_thread_id: None,
                    notification: Some(CodexLaneNotification::ExperimentalFeaturesLoaded {
                        features_json,
                        count,
                        next_cursor: response.next_cursor,
                    }),
                })
            }
            CodexLaneCommand::ThreadRealtimeStart(params) => {
                let thread_id = params.thread_id.clone();
                let session_id = params.session_id.clone();
                let _ = runtime.block_on(client.thread_realtime_start(params))?;
                Ok(CodexCommandEffect {
                    active_thread_id: None,
                    notification: Some(CodexLaneNotification::RealtimeStarted {
                        thread_id,
                        session_id,
                    }),
                })
            }
            CodexLaneCommand::ThreadRealtimeAppendText(params) => {
                let thread_id = params.thread_id.clone();
                let text_len = params.text.chars().count();
                let _ = runtime.block_on(client.thread_realtime_append_text(params))?;
                Ok(CodexCommandEffect {
                    active_thread_id: None,
                    notification: Some(CodexLaneNotification::RealtimeTextAppended {
                        thread_id,
                        text_len,
                    }),
                })
            }
            CodexLaneCommand::ThreadRealtimeStop(params) => {
                let thread_id = params.thread_id.clone();
                let _ = runtime.block_on(client.thread_realtime_stop(params))?;
                Ok(CodexCommandEffect {
                    active_thread_id: None,
                    notification: Some(CodexLaneNotification::RealtimeStopped { thread_id }),
                })
            }
            CodexLaneCommand::WindowsSandboxSetupStart(params) => {
                let mode = params.mode.clone();
                let response = runtime.block_on(client.windows_sandbox_setup_start(params))?;
                Ok(CodexCommandEffect {
                    active_thread_id: None,
                    notification: Some(CodexLaneNotification::WindowsSandboxSetupStarted {
                        mode,
                        started: response.started,
                    }),
                })
            }
            CodexLaneCommand::FuzzyFileSearchSessionStart(params) => {
                let session_id = params.session_id.clone();
                let _ = runtime.block_on(client.fuzzy_file_search_session_start(params))?;
                Ok(CodexCommandEffect {
                    active_thread_id: None,
                    notification: Some(CodexLaneNotification::FuzzySessionStarted { session_id }),
                })
            }
            CodexLaneCommand::FuzzyFileSearchSessionUpdate(params) => {
                let session_id = params.session_id.clone();
                let status = format!("query={}", params.query);
                let _ = runtime.block_on(client.fuzzy_file_search_session_update(params))?;
                Ok(CodexCommandEffect {
                    active_thread_id: None,
                    notification: Some(CodexLaneNotification::FuzzySessionUpdated {
                        session_id,
                        status,
                    }),
                })
            }
            CodexLaneCommand::FuzzyFileSearchSessionStop(params) => {
                let session_id = params.session_id.clone();
                let _ = runtime.block_on(client.fuzzy_file_search_session_stop(params))?;
                Ok(CodexCommandEffect {
                    active_thread_id: None,
                    notification: Some(CodexLaneNotification::FuzzySessionStopped { session_id }),
                })
            }
            CodexLaneCommand::SkillsRemoteList(params) => {
                let response = runtime.block_on(client.skills_remote_list(params))?;
                let entries = response
                    .data
                    .into_iter()
                    .map(|entry| CodexRemoteSkillEntry {
                        id: entry.id,
                        name: entry.name,
                        description: entry.description,
                    })
                    .collect();
                Ok(CodexCommandEffect {
                    active_thread_id: None,
                    notification: Some(CodexLaneNotification::SkillsRemoteListLoaded { entries }),
                })
            }
            CodexLaneCommand::SkillsRemoteExport(params) => {
                let response = runtime.block_on(client.skills_remote_export(params))?;
                Ok(CodexCommandEffect {
                    active_thread_id: None,
                    notification: Some(CodexLaneNotification::SkillsRemoteExported {
                        id: response.id,
                        path: response.path.display().to_string(),
                    }),
                })
            }
            CodexLaneCommand::SkillsList(params) => {
                let response = runtime.block_on(client.skills_list(params))?;
                let entries = summarize_skills_list_response(response);
                Ok(CodexCommandEffect {
                    active_thread_id: None,
                    notification: Some(CodexLaneNotification::SkillsListLoaded { entries }),
                })
            }
            CodexLaneCommand::SkillsConfigWrite(params) => {
                let _ = runtime.block_on(client.skills_config_write(params))?;
                Ok(CodexCommandEffect {
                    active_thread_id: None,
                    notification: None,
                })
            }
        }
    }

    fn drain_server_updates(&mut self, runtime: &Runtime, update_tx: &Sender<CodexLaneUpdate>) {
        let mut disconnected = false;
        let mut pending_requests = Vec::new();

        if let Some(channels) = self.channels.as_mut() {
            loop {
                match channels.notifications.try_recv() {
                    Ok(notification) => {
                        let method = notification.method.clone();
                        if let Some(normalized) = normalize_notification(notification) {
                            let _ = update_tx.send(CodexLaneUpdate::Notification(normalized));
                        } else {
                            tracing::info!(
                                "codex notify/ignored method={} reason=normalize-none",
                                method
                            );
                            let _ = update_tx.send(CodexLaneUpdate::Notification(
                                CodexLaneNotification::Raw { method },
                            ));
                        }
                    }
                    Err(TokioTryRecvError::Empty) => break,
                    Err(TokioTryRecvError::Disconnected) => {
                        disconnected = true;
                        break;
                    }
                }
            }

            if !disconnected {
                loop {
                    match channels.requests.try_recv() {
                        Ok(request) => {
                            pending_requests.push(request);
                        }
                        Err(TokioTryRecvError::Empty) => break,
                        Err(TokioTryRecvError::Disconnected) => {
                            disconnected = true;
                            break;
                        }
                    }
                }
            }
        }

        for request in pending_requests {
            self.handle_server_request(runtime, request, update_tx);
        }

        if disconnected {
            self.client = None;
            self.channels = None;
            self.pending_server_requests.clear();
            self.set_error(update_tx, "Codex lane disconnected from app-server", true);
        }
    }

    fn handle_server_request(
        &mut self,
        runtime: &Runtime,
        request: AppServerRequest,
        update_tx: &Sender<CodexLaneUpdate>,
    ) {
        let _ = update_tx.send(CodexLaneUpdate::Notification(
            CodexLaneNotification::ServerRequest {
                method: request.method.clone(),
            },
        ));

        if let Some(client) = self.client.as_ref() {
            match request.method.as_str() {
                "item/commandExecution/requestApproval" => {
                    let parsed = request
                        .params
                        .clone()
                        .ok_or_else(|| anyhow::anyhow!("missing request params"))
                        .and_then(|params| {
                            serde_json::from_value::<CommandExecutionRequestApprovalParams>(params)
                                .map_err(|error| anyhow::anyhow!(error))
                        });
                    match parsed {
                        Ok(params) => {
                            self.pending_server_requests
                                .insert(request.id.clone(), request.method.clone());
                            let _ = update_tx.send(CodexLaneUpdate::Notification(
                                CodexLaneNotification::CommandApprovalRequested {
                                    request_id: request.id,
                                    request: CodexCommandApprovalRequest {
                                        thread_id: params.thread_id,
                                        turn_id: params.turn_id,
                                        item_id: params.item_id,
                                        reason: params.reason,
                                        command: params.command,
                                        cwd: params.cwd.map(|path| path.display().to_string()),
                                    },
                                },
                            ));
                        }
                        Err(error) => {
                            tracing::info!(
                                "codex server request parse failed method={} error={}",
                                request.method,
                                error
                            );
                            let _ = runtime.block_on(client.respond(
                                request.id,
                                &CommandExecutionRequestApprovalResponse {
                                    decision: codex_client::ApprovalDecision::Decline,
                                },
                            ));
                        }
                    }
                }
                "item/fileChange/requestApproval" => {
                    let parsed = request
                        .params
                        .clone()
                        .ok_or_else(|| anyhow::anyhow!("missing request params"))
                        .and_then(|params| {
                            serde_json::from_value::<FileChangeRequestApprovalParams>(params)
                                .map_err(|error| anyhow::anyhow!(error))
                        });
                    match parsed {
                        Ok(params) => {
                            self.pending_server_requests
                                .insert(request.id.clone(), request.method.clone());
                            let _ = update_tx.send(CodexLaneUpdate::Notification(
                                CodexLaneNotification::FileChangeApprovalRequested {
                                    request_id: request.id,
                                    request: CodexFileChangeApprovalRequest {
                                        thread_id: params.thread_id,
                                        turn_id: params.turn_id,
                                        item_id: params.item_id,
                                        reason: params.reason,
                                        grant_root: params
                                            .grant_root
                                            .map(|path| path.display().to_string()),
                                    },
                                },
                            ));
                        }
                        Err(error) => {
                            tracing::info!(
                                "codex server request parse failed method={} error={}",
                                request.method,
                                error
                            );
                            let _ = runtime.block_on(client.respond(
                                request.id,
                                &FileChangeRequestApprovalResponse {
                                    decision: codex_client::ApprovalDecision::Decline,
                                },
                            ));
                        }
                    }
                }
                "item/tool/call" => {
                    let parsed = request
                        .params
                        .clone()
                        .ok_or_else(|| anyhow::anyhow!("missing request params"))
                        .and_then(|params| {
                            serde_json::from_value::<DynamicToolCallParams>(params)
                                .map_err(|error| anyhow::anyhow!(error))
                        });
                    match parsed {
                        Ok(params) => {
                            self.pending_server_requests
                                .insert(request.id.clone(), request.method.clone());
                            let _ = update_tx.send(CodexLaneUpdate::Notification(
                                CodexLaneNotification::ToolCallRequested {
                                    request_id: request.id,
                                    request: CodexToolCallRequest {
                                        thread_id: params.thread_id,
                                        turn_id: params.turn_id,
                                        call_id: params.call_id,
                                        tool: params.tool,
                                        arguments: serde_json::to_string(&params.arguments)
                                            .unwrap_or_else(|_| "{}".to_string()),
                                    },
                                },
                            ));
                        }
                        Err(error) => {
                            tracing::info!(
                                "codex server request parse failed method={} error={}",
                                request.method,
                                error
                            );
                            let _ = runtime.block_on(client.respond(
                                request.id,
                                &DynamicToolCallResponse {
                                    content_items: vec![DynamicToolCallOutputContentItem::InputText {
                                        text: format!(
                                            "OpenAgents desktop failed to parse tool request: {error}"
                                        ),
                                    }],
                                    success: false,
                                },
                            ));
                        }
                    }
                }
                "item/tool/requestUserInput" => {
                    let parsed = request
                        .params
                        .clone()
                        .ok_or_else(|| anyhow::anyhow!("missing request params"))
                        .and_then(|params| {
                            serde_json::from_value::<ToolRequestUserInputParams>(params)
                                .map_err(|error| anyhow::anyhow!(error))
                        });
                    match parsed {
                        Ok(params) => {
                            self.pending_server_requests
                                .insert(request.id.clone(), request.method.clone());
                            let _ = update_tx.send(CodexLaneUpdate::Notification(
                                CodexLaneNotification::ToolUserInputRequested {
                                    request_id: request.id,
                                    request: CodexToolUserInputRequest {
                                        thread_id: params.thread_id,
                                        turn_id: params.turn_id,
                                        item_id: params.item_id,
                                        questions: params
                                            .questions
                                            .into_iter()
                                            .map(|question| CodexToolUserInputQuestion {
                                                id: question.id,
                                                header: question.header,
                                                question: question.question,
                                                options: question
                                                    .options
                                                    .unwrap_or_default()
                                                    .into_iter()
                                                    .map(|option| option.label)
                                                    .collect(),
                                            })
                                            .collect(),
                                    },
                                },
                            ));
                        }
                        Err(error) => {
                            tracing::info!(
                                "codex server request parse failed method={} error={}",
                                request.method,
                                error
                            );
                            let _ = runtime.block_on(client.respond(
                                request.id,
                                &ToolRequestUserInputResponse {
                                    answers: HashMap::new(),
                                },
                            ));
                        }
                    }
                }
                "account/chatgptAuthTokens/refresh" => {
                    let parsed = request
                        .params
                        .clone()
                        .ok_or_else(|| anyhow::anyhow!("missing request params"))
                        .and_then(|params| {
                            serde_json::from_value::<ChatgptAuthTokensRefreshParams>(params)
                                .map_err(|error| anyhow::anyhow!(error))
                        });
                    match parsed {
                        Ok(params) => {
                            self.pending_server_requests
                                .insert(request.id.clone(), request.method.clone());
                            let _ = update_tx.send(CodexLaneUpdate::Notification(
                                CodexLaneNotification::AuthTokensRefreshRequested {
                                    request_id: request.id,
                                    request: CodexAuthTokensRefreshRequest {
                                        reason: params.reason,
                                        previous_account_id: params.previous_account_id,
                                    },
                                },
                            ));
                        }
                        Err(error) => {
                            tracing::info!(
                                "codex server request parse failed method={} error={}",
                                request.method,
                                error
                            );
                            let _ = runtime.block_on(client.respond(
                                request.id,
                                &ChatgptAuthTokensRefreshResponse {
                                    access_token: String::new(),
                                    chatgpt_account_id: String::new(),
                                    chatgpt_plan_type: None,
                                },
                            ));
                        }
                    }
                }
                _ => {
                    let ack = serde_json::json!({ "status": "unsupported" });
                    let _ = runtime.block_on(client.respond(request.id, &ack));
                }
            }
        }
    }

    fn respond_to_server_request_value(
        &mut self,
        runtime: &Runtime,
        client: &AppServerClient,
        request_id: AppServerRequestId,
        expected_method: &str,
        response: Value,
    ) -> Result<()> {
        let Some(method) = self.pending_server_requests.remove(&request_id) else {
            return Err(anyhow::anyhow!("server request id not pending"));
        };

        if method != expected_method {
            self.pending_server_requests
                .insert(request_id.clone(), method.clone());
            return Err(anyhow::anyhow!(
                "server request method mismatch: expected {}, got {}",
                expected_method,
                method
            ));
        }

        runtime.block_on(client.respond(request_id, &response))
    }

    fn shutdown(&mut self, runtime: &Runtime, update_tx: &Sender<CodexLaneUpdate>) {
        if let Some(client) = self.client.take() {
            let _ = runtime.block_on(client.shutdown());
        }
        self.channels = None;
        self.pending_server_requests.clear();
        self.snapshot.lifecycle = CodexLaneLifecycle::Stopped;
        self.snapshot.last_status = Some("Codex lane stopped".to_string());
        self.snapshot.last_error = None;
        self.publish_snapshot(update_tx);
    }
}

#[cfg(test)]
#[path = "codex_lane/tests.rs"]
mod tests;
