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
    McpServerOauthLoginParams, ModelListParams, ReviewStartParams, SkillScope,
    SkillsConfigWriteParams, SkillsListParams, SkillsListResponse, SkillsRemoteReadParams,
    SkillsRemoteWriteParams, ThreadArchiveParams, ThreadCompactStartParams, ThreadForkParams,
    ThreadListParams, ThreadLoadedListParams, ThreadReadParams, ThreadRealtimeAppendTextParams,
    ThreadRealtimeStartParams, ThreadRealtimeStopParams, ThreadResumeParams, ThreadRollbackParams,
    ThreadSetNameParams, ThreadStartParams, ThreadUnarchiveParams, ThreadUnsubscribeParams,
    ToolRequestUserInputParams, ToolRequestUserInputResponse, TurnInterruptParams, TurnStartParams,
    WindowsSandboxSetupStartParams,
};
use serde_json::Value;
use tokio::runtime::Runtime;
use tokio::sync::mpsc::error::TryRecvError as TokioTryRecvError;

const CODEX_LANE_POLL: Duration = Duration::from_millis(80);

#[derive(Clone, Debug)]
pub struct CodexLaneConfig {
    pub cwd: Option<PathBuf>,
    pub bootstrap_thread: bool,
    pub bootstrap_model: Option<String>,
    pub wire_log_path: Option<PathBuf>,
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
            client_info: ClientInfo {
                name: "openagents-autopilot-desktop".to_string(),
                title: Some("OpenAgents Autopilot Desktop".to_string()),
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
            approval_policy: Some(AskForApproval::OnRequest),
            experimental_api: true,
            opt_out_notification_methods: Vec::new(),
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
}

impl Default for CodexLaneSnapshot {
    fn default() -> Self {
        Self {
            lifecycle: CodexLaneLifecycle::Starting,
            active_thread_id: None,
            last_error: None,
            last_status: Some("Codex lane starting".to_string()),
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
    },
    ThreadSelected {
        thread_id: String,
    },
    ThreadStarted {
        thread_id: String,
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
    },
    AgentMessageDelta {
        thread_id: String,
        turn_id: String,
        item_id: String,
        delta: String,
    },
    TurnCompleted {
        thread_id: String,
        turn_id: String,
        status: Option<String>,
        error_message: Option<String>,
    },
    TurnDiffUpdated {
        thread_id: String,
        turn_id: String,
        diff: String,
    },
    TurnPlanUpdated {
        thread_id: String,
        turn_id: String,
        explanation: Option<String>,
        plan: Vec<CodexTurnPlanStep>,
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CodexSkillListEntry {
    pub cwd: String,
    pub skills: Vec<CodexSkillSummary>,
    pub errors: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CodexSkillSummary {
    pub name: String,
    pub path: String,
    pub scope: String,
    pub enabled: bool,
    pub interface_display_name: Option<String>,
    pub dependency_count: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CodexThreadListEntry {
    pub thread_id: String,
    pub thread_name: Option<String>,
    pub status: Option<String>,
    pub loaded: bool,
    pub cwd: Option<String>,
    pub path: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CodexThreadTranscriptRole {
    User,
    Codex,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CodexThreadTranscriptMessage {
    pub role: CodexThreadTranscriptRole,
    pub content: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CodexModelCatalogEntry {
    pub model: String,
    pub display_name: String,
    pub description: String,
    pub hidden: bool,
    pub is_default: bool,
    pub default_reasoning_effort: String,
    pub supported_reasoning_efforts: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CodexMcpServerStatusEntry {
    pub name: String,
    pub auth_status: String,
    pub tool_count: usize,
    pub resource_count: usize,
    pub template_count: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CodexAppEntry {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub is_accessible: bool,
    pub is_enabled: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CodexRemoteSkillEntry {
    pub id: String,
    pub name: String,
    pub description: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CodexTurnPlanStep {
    pub step: String,
    pub status: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CodexCommandApprovalRequest {
    pub thread_id: String,
    pub turn_id: String,
    pub item_id: String,
    pub reason: Option<String>,
    pub command: Option<String>,
    pub cwd: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CodexFileChangeApprovalRequest {
    pub thread_id: String,
    pub turn_id: String,
    pub item_id: String,
    pub reason: Option<String>,
    pub grant_root: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CodexToolCallRequest {
    pub thread_id: String,
    pub turn_id: String,
    pub call_id: String,
    pub tool: String,
    pub arguments: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CodexToolUserInputQuestion {
    pub id: String,
    pub header: String,
    pub question: String,
    pub options: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CodexToolUserInputRequest {
    pub thread_id: String,
    pub turn_id: String,
    pub item_id: String,
    pub questions: Vec<CodexToolUserInputQuestion>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CodexAuthTokensRefreshRequest {
    pub reason: String,
    pub previous_account_id: Option<String>,
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
    Command(SequencedCodexCommand),
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
            .send(CodexLaneControl::Command(SequencedCodexCommand {
                command_seq,
                command,
            }))
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
                ..Default::default()
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
        Self {
            snapshot: CodexLaneSnapshot::default(),
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
        eprintln!("codex lane error: {}", message);
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
                cwd: config.cwd.as_ref().map(|path| path.display().to_string()),
                approval_policy: config.approval_policy,
                sandbox: None,
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
                        },
                    ));
                    let _ = update_tx.send(CodexLaneUpdate::Notification(
                        CodexLaneNotification::ThreadListLoaded {
                            entries: vec![CodexThreadListEntry {
                                thread_id,
                                thread_name: None,
                                status: Some("idle".to_string()),
                                loaded: true,
                                cwd: config.cwd.as_ref().map(|value| value.display().to_string()),
                                path: None,
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
                    status: thread.status.as_ref().and_then(thread_status_label),
                    loaded: false,
                    cwd: thread.cwd.map(|value| value.display().to_string()),
                    path: thread.path.map(|value| value.display().to_string()),
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
                if is_disconnect_error(&error) {
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
                    notification: Some(CodexLaneNotification::ThreadStarted { thread_id }),
                })
            }
            CodexLaneCommand::ThreadResume(params) => {
                let response = runtime.block_on(client.thread_resume(params))?;
                let thread_id = response.thread.id;
                Ok(CodexCommandEffect {
                    active_thread_id: Some(thread_id.clone()),
                    notification: Some(CodexLaneNotification::ThreadSelected { thread_id }),
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
                Ok(CodexCommandEffect {
                    active_thread_id: Some(thread_id.clone()),
                    notification: Some(CodexLaneNotification::ThreadReadLoaded {
                        thread_id,
                        messages,
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
                        status: thread.status.as_ref().and_then(thread_status_label),
                        loaded: false,
                        cwd: thread.cwd.map(|value| value.display().to_string()),
                        path: thread.path.map(|value| value.display().to_string()),
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
                let _ = runtime.block_on(client.turn_start(params))?;
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
                Ok(CodexCommandEffect {
                    active_thread_id: None,
                    notification: Some(CodexLaneNotification::ConfigLoaded { config }),
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
                let response = runtime.block_on(client.review_start(params))?;
                Ok(CodexCommandEffect {
                    active_thread_id: None,
                    notification: Some(CodexLaneNotification::ReviewStarted {
                        thread_id,
                        turn_id: response.turn.id,
                        review_thread_id: response.review_thread_id,
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
                        if let Some(normalized) = normalize_notification(notification) {
                            let _ = update_tx.send(CodexLaneUpdate::Notification(normalized));
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
                            eprintln!(
                                "codex server request parse failed method={} error={}",
                                request.method, error
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
                            eprintln!(
                                "codex server request parse failed method={} error={}",
                                request.method, error
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
                            eprintln!(
                                "codex server request parse failed method={} error={}",
                                request.method, error
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
                            eprintln!(
                                "codex server request parse failed method={} error={}",
                                request.method, error
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
                            eprintln!(
                                "codex server request parse failed method={} error={}",
                                request.method, error
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

fn run_codex_lane_loop(
    command_rx: Receiver<CodexLaneControl>,
    update_tx: Sender<CodexLaneUpdate>,
    config: CodexLaneConfig,
    mut runtime_impl: Box<dyn CodexLaneRuntime>,
) {
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            let snapshot = CodexLaneSnapshot {
                lifecycle: CodexLaneLifecycle::Error,
                active_thread_id: None,
                last_error: Some(format!("Codex lane runtime initialization failed: {error}")),
                last_status: Some("Codex lane runtime unavailable".to_string()),
            };
            let _ = update_tx.send(CodexLaneUpdate::Snapshot(Box::new(snapshot)));
            return;
        }
    };

    let mut state = CodexLaneState::new();
    state.publish_snapshot(&update_tx);
    state.handle_connect(&runtime, &config, &update_tx, runtime_impl.as_mut());

    loop {
        state.drain_server_updates(&runtime, &update_tx);

        match command_rx.recv_timeout(CODEX_LANE_POLL) {
            Ok(CodexLaneControl::Command(envelope)) => {
                state.handle_command(&runtime, envelope, &update_tx);
            }
            Ok(CodexLaneControl::Shutdown) => {
                state.shutdown(&runtime, &update_tx);
                break;
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => {
                state.shutdown(&runtime, &update_tx);
                break;
            }
        }
    }
}

fn normalize_notification(notification: AppServerNotification) -> Option<CodexLaneNotification> {
    let method = notification.method;
    let params = notification.params;

    match method.as_str() {
        "thread/started" => {
            let thread_id = thread_id_from_params(params.as_ref())?;
            Some(CodexLaneNotification::ThreadStatusChanged {
                thread_id,
                status: "active".to_string(),
            })
        }
        "thread/status/changed" => {
            let params = params?;
            let thread_id = string_field(&params, "threadId")?;
            let status = params
                .get("status")
                .and_then(thread_status_label)
                .unwrap_or_else(|| "unknown".to_string());
            Some(CodexLaneNotification::ThreadStatusChanged { thread_id, status })
        }
        "thread/archived" => {
            let params = params?;
            Some(CodexLaneNotification::ThreadArchived {
                thread_id: string_field(&params, "threadId")?,
            })
        }
        "thread/unarchived" => {
            let params = params?;
            Some(CodexLaneNotification::ThreadUnarchived {
                thread_id: string_field(&params, "threadId")?,
            })
        }
        "thread/closed" => {
            let params = params?;
            Some(CodexLaneNotification::ThreadClosed {
                thread_id: string_field(&params, "threadId")?,
            })
        }
        "thread/name/updated" => {
            let params = params?;
            Some(CodexLaneNotification::ThreadNameUpdated {
                thread_id: string_field(&params, "threadId")?,
                thread_name: string_field(&params, "threadName"),
            })
        }
        "account/updated" => {
            let params = params?;
            Some(CodexLaneNotification::AccountUpdated {
                auth_mode: string_field(&params, "authMode"),
            })
        }
        "account/login/completed" => {
            let params = params?;
            Some(CodexLaneNotification::AccountLoginCompleted {
                login_id: string_field(&params, "loginId"),
                success: params
                    .get("success")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
                error: string_field(&params, "error"),
            })
        }
        "account/rateLimits/updated" => {
            let params = params?;
            let rate_limits = params.get("rateLimits")?;
            Some(CodexLaneNotification::AccountRateLimitsLoaded {
                summary: rate_limits_summary(rate_limits),
            })
        }
        "model/rerouted" => {
            let params = params?;
            Some(CodexLaneNotification::ModelRerouted {
                thread_id: string_field(&params, "threadId")?,
                turn_id: string_field(&params, "turnId")?,
                from_model: string_field(&params, "fromModel")?,
                to_model: string_field(&params, "toModel")?,
                reason: string_field(&params, "reason").unwrap_or_else(|| "unknown".to_string()),
            })
        }
        "mcpServer/oauthLogin/completed" => {
            let params = params?;
            Some(CodexLaneNotification::McpServerOauthLoginCompleted {
                server_name: string_field(&params, "name").unwrap_or_else(|| "unknown".to_string()),
                success: params
                    .get("success")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
                error: string_field(&params, "error"),
            })
        }
        "app/list/updated" => Some(CodexLaneNotification::AppsListUpdated),
        "fuzzyFileSearch/sessionUpdated" => {
            let params = params?;
            let session_id =
                string_field(&params, "sessionId").unwrap_or_else(|| "unknown".to_string());
            let status = string_field(&params, "status").unwrap_or_else(|| {
                serde_json::to_string(&params).unwrap_or_else(|_| "updated".to_string())
            });
            Some(CodexLaneNotification::FuzzySessionUpdated { session_id, status })
        }
        "fuzzyFileSearch/sessionCompleted" => {
            let params = params?;
            let session_id =
                string_field(&params, "sessionId").unwrap_or_else(|| "unknown".to_string());
            Some(CodexLaneNotification::FuzzySessionCompleted { session_id })
        }
        "thread/realtime/started" => {
            let params = params?;
            Some(CodexLaneNotification::RealtimeStarted {
                thread_id: string_field(&params, "threadId")?,
                session_id: string_field(&params, "sessionId"),
            })
        }
        "thread/realtime/closed" => {
            let params = params?;
            Some(CodexLaneNotification::RealtimeStopped {
                thread_id: string_field(&params, "threadId")?,
            })
        }
        "thread/realtime/error" => {
            let params = params?;
            let message = params
                .get("error")
                .and_then(|error| error.get("message"))
                .and_then(Value::as_str)
                .map(str::to_string)
                .or_else(|| string_field(&params, "message"))
                .unwrap_or_else(|| "thread realtime error".to_string());
            Some(CodexLaneNotification::RealtimeError {
                thread_id: string_field(&params, "threadId")
                    .unwrap_or_else(|| "unknown-thread".to_string()),
                message,
            })
        }
        "windowsSandbox/setupCompleted" => {
            let params = params?;
            Some(CodexLaneNotification::WindowsSandboxSetupCompleted {
                mode: string_field(&params, "mode"),
                success: params.get("success").and_then(Value::as_bool),
            })
        }
        "turn/started" => {
            let params = params?;
            let thread_id = string_field(&params, "threadId")?;
            let turn_id = turn_id_from_value(&params)?;
            Some(CodexLaneNotification::TurnStarted { thread_id, turn_id })
        }
        "item/started" => {
            let params = params?;
            Some(CodexLaneNotification::ItemStarted {
                thread_id: string_field(&params, "threadId")?,
                turn_id: string_field(&params, "turnId")?,
                item_id: item_id_from_params(&params),
                item_type: item_type_from_params(&params),
            })
        }
        "item/completed" => {
            let params = params?;
            Some(CodexLaneNotification::ItemCompleted {
                thread_id: string_field(&params, "threadId")?,
                turn_id: string_field(&params, "turnId")?,
                item_id: item_id_from_params(&params),
                item_type: item_type_from_params(&params),
            })
        }
        "item/agentMessage/delta" | "agent_message/delta" => {
            let params = params?;
            Some(CodexLaneNotification::AgentMessageDelta {
                thread_id: string_field(&params, "threadId")?,
                turn_id: string_field(&params, "turnId")?,
                item_id: string_field(&params, "itemId")?,
                delta: string_field(&params, "delta")?,
            })
        }
        "turn/completed" => {
            let params = params?;
            let thread_id = string_field(&params, "threadId")?;
            let turn_id =
                turn_id_from_value(&params).or_else(|| string_field(&params, "turnId"))?;
            let status = params
                .get("turn")
                .and_then(|turn| turn.get("status"))
                .and_then(Value::as_str)
                .map(str::to_string)
                .or_else(|| string_field(&params, "status"));
            let error_message = params
                .get("turn")
                .and_then(|turn| turn.get("error"))
                .and_then(|error| error.get("message"))
                .and_then(Value::as_str)
                .map(str::to_string)
                .or_else(|| {
                    params
                        .get("error")
                        .and_then(|error| error.get("message"))
                        .and_then(Value::as_str)
                        .map(str::to_string)
                });
            Some(CodexLaneNotification::TurnCompleted {
                thread_id,
                turn_id,
                status,
                error_message,
            })
        }
        "turn/diff/updated" => {
            let params = params?;
            Some(CodexLaneNotification::TurnDiffUpdated {
                thread_id: string_field(&params, "threadId")?,
                turn_id: string_field(&params, "turnId")?,
                diff: string_field(&params, "diff")?,
            })
        }
        "turn/plan/updated" => {
            let params = params?;
            Some(CodexLaneNotification::TurnPlanUpdated {
                thread_id: string_field(&params, "threadId")?,
                turn_id: string_field(&params, "turnId")?,
                explanation: string_field(&params, "explanation"),
                plan: turn_plan_from_params(&params),
            })
        }
        "thread/tokenUsage/updated" => {
            let params = params?;
            let token_usage = params.get("tokenUsage")?;
            let usage_scope = token_usage
                .get("last")
                .filter(|last| last.is_object())
                .unwrap_or(token_usage);
            Some(CodexLaneNotification::ThreadTokenUsageUpdated {
                thread_id: string_field(&params, "threadId")?,
                turn_id: string_field(&params, "turnId")?,
                input_tokens: i64_field(usage_scope, "inputTokens").unwrap_or_default(),
                cached_input_tokens: i64_field(usage_scope, "cachedInputTokens")
                    .unwrap_or_default(),
                output_tokens: i64_field(usage_scope, "outputTokens").unwrap_or_default(),
            })
        }
        "turn/error" | "error" => {
            let params = params?;
            let message = params
                .get("error")
                .and_then(|error| error.get("message"))
                .and_then(Value::as_str)
                .unwrap_or("Unknown turn error")
                .to_string();
            Some(CodexLaneNotification::TurnError {
                thread_id: string_field(&params, "threadId")
                    .unwrap_or_else(|| "unknown-thread".to_string()),
                turn_id: string_field(&params, "turnId")
                    .unwrap_or_else(|| "unknown-turn".to_string()),
                message,
            })
        }
        _ => Some(CodexLaneNotification::Raw { method }),
    }
}

fn thread_id_from_params(params: Option<&Value>) -> Option<String> {
    let params = params?;
    params
        .get("thread")
        .and_then(|thread| thread.get("id"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| string_field(params, "threadId"))
}

fn turn_id_from_value(value: &Value) -> Option<String> {
    value
        .get("turn")
        .and_then(|turn| turn.get("id"))
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn item_id_from_params(value: &Value) -> Option<String> {
    value
        .get("item")
        .and_then(|item| item.get("id"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| string_field(value, "itemId"))
}

fn item_type_from_params(value: &Value) -> Option<String> {
    value
        .get("item")
        .and_then(|item| item.get("type"))
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn turn_plan_from_params(value: &Value) -> Vec<CodexTurnPlanStep> {
    value
        .get("plan")
        .and_then(Value::as_array)
        .map(|steps| {
            steps
                .iter()
                .filter_map(|step| {
                    Some(CodexTurnPlanStep {
                        step: step.get("step")?.as_str()?.to_string(),
                        status: step.get("status")?.as_str()?.to_string(),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn extract_thread_transcript_messages(
    thread: &codex_client::ThreadSnapshot,
) -> Vec<CodexThreadTranscriptMessage> {
    let mut messages = Vec::new();
    for turn in &thread.turns {
        for item in &turn.items {
            collect_transcript_messages(item, &mut messages);
        }
    }
    messages
}

fn collect_transcript_messages(value: &Value, messages: &mut Vec<CodexThreadTranscriptMessage>) {
    let Some(object) = value.as_object() else {
        return;
    };

    if let Some(payload) = object.get("payload") {
        collect_transcript_messages(payload, messages);
    }

    let kind = object.get("type").and_then(Value::as_str);
    match kind {
        Some("user_message") | Some("userMessage") => {
            let content = value
                .get("content")
                .and_then(extract_content_text)
                .or_else(|| string_field(value, "message").and_then(non_empty_text));
            if let Some(content) = content {
                messages.push(CodexThreadTranscriptMessage {
                    role: CodexThreadTranscriptRole::User,
                    content,
                });
            }
        }
        Some("agent_message") | Some("agentMessage") => {
            let content = string_field(value, "text")
                .and_then(non_empty_text)
                .or_else(|| string_field(value, "message").and_then(non_empty_text))
                .or_else(|| value.get("content").and_then(extract_content_text));
            if let Some(content) = content {
                messages.push(CodexThreadTranscriptMessage {
                    role: CodexThreadTranscriptRole::Codex,
                    content,
                });
            }
        }
        _ => {
            let Some(role) = object.get("role").and_then(Value::as_str) else {
                return;
            };
            let Some(mapped_role) = map_transcript_role(role) else {
                return;
            };

            let content = object
                .get("content")
                .and_then(extract_content_text)
                .or_else(|| string_field(value, "message").and_then(non_empty_text));
            if let Some(content) = content {
                messages.push(CodexThreadTranscriptMessage {
                    role: mapped_role,
                    content,
                });
            }
        }
    }
}

fn map_transcript_role(role: &str) -> Option<CodexThreadTranscriptRole> {
    match role {
        "user" => Some(CodexThreadTranscriptRole::User),
        "assistant" | "codex" => Some(CodexThreadTranscriptRole::Codex),
        _ => None,
    }
}

fn extract_content_text(content: &Value) -> Option<String> {
    match content {
        Value::String(value) => non_empty_text(value.to_string()),
        Value::Array(entries) => {
            let parts = entries
                .iter()
                .filter_map(|entry| {
                    entry
                        .get("text")
                        .and_then(Value::as_str)
                        .map(str::to_string)
                        .or_else(|| entry.as_str().map(str::to_string))
                })
                .filter_map(non_empty_text)
                .collect::<Vec<_>>();
            if parts.is_empty() {
                None
            } else {
                Some(parts.join("\n"))
            }
        }
        Value::Object(map) => map
            .get("text")
            .and_then(Value::as_str)
            .map(str::to_string)
            .and_then(non_empty_text),
        _ => None,
    }
}

fn non_empty_text(value: String) -> Option<String> {
    if value.trim().is_empty() {
        None
    } else {
        Some(value)
    }
}

fn string_field(value: &Value, field: &str) -> Option<String> {
    value.get(field).and_then(Value::as_str).map(str::to_string)
}

fn i64_field(value: &Value, field: &str) -> Option<i64> {
    value.get(field).and_then(Value::as_i64)
}

fn thread_status_label(status: &Value) -> Option<String> {
    if let Some(value) = status.as_str() {
        return Some(value.to_string());
    }

    let status_type = status
        .get("type")
        .and_then(Value::as_str)
        .map(str::to_string)?;
    if status_type != "active" {
        return Some(status_type);
    }

    let flags = status
        .get("activeFlags")
        .and_then(Value::as_array)
        .map(|entries| {
            entries
                .iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join("+")
        })
        .unwrap_or_default();
    if flags.is_empty() {
        Some("active".to_string())
    } else {
        Some(format!("active:{flags}"))
    }
}

fn mcp_auth_status_label(status: codex_client::McpAuthStatus) -> &'static str {
    match status {
        codex_client::McpAuthStatus::Unsupported => "unsupported",
        codex_client::McpAuthStatus::NotLoggedIn => "not_logged_in",
        codex_client::McpAuthStatus::BearerToken => "bearer_token",
        codex_client::McpAuthStatus::OAuth => "oauth",
    }
}

fn is_disconnect_error(error: &anyhow::Error) -> bool {
    let text = error.to_string().to_ascii_lowercase();
    text.contains("connection closed")
        || text.contains("channel closed")
        || text.contains("broken pipe")
        || text.contains("transport endpoint is not connected")
        || text.contains("request canceled")
        || text.contains("app-server write failed")
        || text.contains("app-server request canceled")
        || text.contains("app-server connection closed")
}

fn summarize_skills_list_response(response: SkillsListResponse) -> Vec<CodexSkillListEntry> {
    response
        .data
        .into_iter()
        .map(|entry| CodexSkillListEntry {
            cwd: entry.cwd.display().to_string(),
            skills: entry
                .skills
                .into_iter()
                .map(|skill| CodexSkillSummary {
                    name: skill.name,
                    path: skill.path.display().to_string(),
                    scope: skill_scope_label(skill.scope).to_string(),
                    enabled: skill.enabled,
                    interface_display_name: skill
                        .interface
                        .and_then(|interface| interface.display_name),
                    dependency_count: skill
                        .dependencies
                        .map_or(0, |dependencies| dependencies.tools.len()),
                })
                .collect(),
            errors: entry
                .errors
                .into_iter()
                .map(|error| format!("{}: {}", error.path.display(), error.message))
                .collect(),
        })
        .collect()
}

fn account_summary(response: &codex_client::GetAccountResponse) -> String {
    match response.account.as_ref() {
        Some(codex_client::AccountInfo::ApiKey) => "apiKey".to_string(),
        Some(codex_client::AccountInfo::Chatgpt { email, plan_type }) => {
            format!("chatgpt:{}:{plan_type:?}", email)
        }
        None => "none".to_string(),
    }
}

fn rate_limits_summary(rate_limits: &Value) -> String {
    let plan = rate_limits
        .get("planType")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let primary_used = rate_limits
        .get("primary")
        .and_then(|value| value.get("usedPercent"))
        .and_then(Value::as_i64)
        .unwrap_or_default();
    let secondary_used = rate_limits
        .get("secondary")
        .and_then(|value| value.get("usedPercent"))
        .and_then(Value::as_i64)
        .unwrap_or_default();
    format!("plan={plan} primary={primary_used}% secondary={secondary_used}%")
}

fn fetch_model_catalog_entries(
    runtime: &Runtime,
    client: &AppServerClient,
    params: ModelListParams,
) -> Result<(Vec<CodexModelCatalogEntry>, Option<String>)> {
    let mut cursor = params.cursor;
    let limit = params.limit.or(Some(100));
    let include_hidden = params.include_hidden;
    let mut seen = HashSet::new();
    let mut entries = Vec::new();
    let mut default_model = None;

    loop {
        let response = runtime.block_on(client.model_list(ModelListParams {
            cursor: cursor.clone(),
            limit,
            include_hidden,
        }))?;

        for model in response.data {
            let value = model.model.trim();
            if value.is_empty() {
                continue;
            }
            if !seen.insert(value.to_string()) {
                continue;
            }

            if model.is_default && default_model.is_none() {
                default_model = Some(value.to_string());
            }

            let default_reasoning_effort = serde_json::to_string(&model.default_reasoning_effort)
                .unwrap_or_else(|_| "\"unknown\"".to_string())
                .trim_matches('"')
                .to_string();
            let supported_reasoning_efforts = model
                .supported_reasoning_efforts
                .iter()
                .map(|effort| {
                    serde_json::to_string(&effort.reasoning_effort)
                        .unwrap_or_else(|_| "\"unknown\"".to_string())
                        .trim_matches('"')
                        .to_string()
                })
                .collect();

            entries.push(CodexModelCatalogEntry {
                model: value.to_string(),
                display_name: model.display_name,
                description: model.description,
                hidden: model.hidden,
                is_default: model.is_default,
                default_reasoning_effort,
                supported_reasoning_efforts,
            });
        }

        match response.next_cursor {
            Some(next) if !next.is_empty() => {
                cursor = Some(next);
            }
            _ => break,
        }
    }

    Ok((entries, default_model))
}

fn fetch_model_catalog(
    runtime: &Runtime,
    client: &AppServerClient,
) -> Result<(Vec<String>, Option<String>)> {
    let (entries, default_model) = fetch_model_catalog_entries(
        runtime,
        client,
        ModelListParams {
            cursor: None,
            limit: Some(100),
            include_hidden: None,
        },
    )?;
    let models = entries.into_iter().map(|entry| entry.model).collect();
    Ok((models, default_model))
}

fn skill_scope_label(scope: SkillScope) -> &'static str {
    match scope {
        SkillScope::User => "user",
        SkillScope::Repo => "repo",
        SkillScope::System => "system",
        SkillScope::Admin => "admin",
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CodexLaneCommand, CodexLaneCommandKind, CodexLaneCommandResponse, CodexLaneCommandStatus,
        CodexLaneConfig, CodexLaneLifecycle, CodexLaneNotification, CodexLaneRuntime,
        CodexLaneUpdate, CodexLaneWorker, CodexThreadTranscriptRole,
        extract_thread_transcript_messages, normalize_notification,
    };

    use std::collections::HashSet;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

    use anyhow::Result;
    use codex_client::{
        AppServerChannels, AppServerClient, AppsListParams, CollaborationModeListParams,
        CommandExecParams, ExperimentalFeatureListParams, FuzzyFileSearchSessionStartParams,
        FuzzyFileSearchSessionStopParams, FuzzyFileSearchSessionUpdateParams, ReviewStartParams,
        ReviewTarget, SkillsListExtraRootsForCwd, SkillsListParams, SkillsRemoteWriteParams,
        ThreadListParams, ThreadRealtimeAppendTextParams, ThreadRealtimeStartParams,
        ThreadRealtimeStopParams, WindowsSandboxSetupStartParams,
    };
    use serde_json::{Value, json};
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

    struct FailingRuntime;

    impl CodexLaneRuntime for FailingRuntime {
        fn connect(
            &mut self,
            _runtime: &tokio::runtime::Runtime,
            _config: &CodexLaneConfig,
        ) -> Result<(AppServerClient, AppServerChannels)> {
            Err(anyhow::anyhow!("forced startup failure"))
        }
    }

    struct SingleClientRuntime {
        connection: Option<(AppServerClient, AppServerChannels)>,
        _runtime_guard: Option<tokio::runtime::Runtime>,
    }

    impl SingleClientRuntime {
        fn new(
            connection: (AppServerClient, AppServerChannels),
            runtime_guard: tokio::runtime::Runtime,
        ) -> Self {
            Self {
                connection: Some(connection),
                _runtime_guard: Some(runtime_guard),
            }
        }
    }

    impl CodexLaneRuntime for SingleClientRuntime {
        fn connect(
            &mut self,
            _runtime: &tokio::runtime::Runtime,
            _config: &CodexLaneConfig,
        ) -> Result<(AppServerClient, AppServerChannels)> {
            match self.connection.take() {
                Some(connection) => Ok(connection),
                None => Err(anyhow::anyhow!("mock connection already used")),
            }
        }
    }

    #[test]
    fn startup_failure_reports_error_snapshot() {
        let mut worker = CodexLaneWorker::spawn_with_runtime(
            CodexLaneConfig::default(),
            Box::new(FailingRuntime),
        );

        let snapshot = wait_for_snapshot(&mut worker, Duration::from_secs(2), |snapshot| {
            snapshot.lifecycle == CodexLaneLifecycle::Error
        });

        assert_eq!(snapshot.lifecycle, CodexLaneLifecycle::Error);
        let has_message = snapshot
            .last_error
            .as_deref()
            .is_some_and(|message| message.contains("forced startup failure"));
        assert!(has_message);

        worker.shutdown();
    }

    #[test]
    fn startup_bootstrap_transitions_to_ready_snapshot() {
        let (client_stream, server_stream) = tokio::io::duplex(16 * 1024);
        let (client_read, client_write) = tokio::io::split(client_stream);
        let (server_read, mut server_write) = tokio::io::split(server_stream);
        let runtime_guard = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap_or_else(|_| panic!("failed to build runtime"));
        let _entered = runtime_guard.enter();
        let (client, channels) =
            AppServerClient::connect_with_io(Box::new(client_write), Box::new(client_read), None);
        drop(_entered);

        let saw_model_list = Arc::new(AtomicBool::new(false));
        let saw_model_list_clone = Arc::clone(&saw_model_list);
        let server = std::thread::spawn(move || {
            let runtime = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(runtime) => runtime,
                Err(_) => return,
            };
            runtime.block_on(async move {
                let mut reader = BufReader::new(server_read);
                let mut request_line = String::new();
                let mut handled_requests = 0usize;
                loop {
                    request_line.clear();
                    let bytes = reader.read_line(&mut request_line).await.unwrap_or(0);
                    if bytes == 0 {
                        break;
                    }
                    let value: Value = match serde_json::from_str(request_line.trim()) {
                        Ok(value) => value,
                        Err(_) => continue,
                    };
                    if value.get("id").is_none() {
                        continue;
                    }
                    handled_requests = handled_requests.saturating_add(1);
                    let method = value
                        .get("method")
                        .and_then(Value::as_str)
                        .unwrap_or_default();
                    let response = match method {
                        "initialize" => json!({
                            "id": value["id"].clone(),
                            "result": {"userAgent": "test-agent"}
                        }),
                        "thread/start" => json!({
                            "id": value["id"].clone(),
                            "result": {
                                "thread": {"id": "thread-bootstrap"},
                                "model": "gpt-5.3-codex"
                            }
                        }),
                        "model/list" => {
                            saw_model_list_clone.store(true, Ordering::SeqCst);
                            json!({
                                "id": value["id"].clone(),
                                "result": {
                                    "data": [
                                        {
                                            "id": "default",
                                            "model": "o4-mini",
                                            "displayName": "o4-mini",
                                            "description": "o4-mini",
                                            "supportedReasoningEfforts": [],
                                            "defaultReasoningEffort": "medium",
                                            "isDefault": true
                                        }
                                    ],
                                    "nextCursor": null
                                }
                            })
                        }
                        _ => json!({
                            "id": value["id"].clone(),
                            "result": {}
                        }),
                    };
                    if let Ok(line) = serde_json::to_string(&response) {
                        let _ = server_write.write_all(format!("{line}\n").as_bytes()).await;
                        let _ = server_write.flush().await;
                    }
                    if handled_requests >= 3 {
                        break;
                    }
                }
                drop(server_write);
            });
        });

        let mut worker = CodexLaneWorker::spawn_with_runtime(
            CodexLaneConfig::default(),
            Box::new(SingleClientRuntime::new((client, channels), runtime_guard)),
        );

        let mut saw_ready = false;
        let mut active_thread_id = None;
        let mut models_loaded = None;
        let deadline = Instant::now() + Duration::from_secs(2);
        while Instant::now() <= deadline && (!saw_ready || models_loaded.is_none()) {
            for update in worker.drain_updates() {
                match update {
                    CodexLaneUpdate::Snapshot(snapshot) => {
                        if snapshot.lifecycle == CodexLaneLifecycle::Ready {
                            saw_ready = true;
                            active_thread_id = snapshot.active_thread_id.clone();
                        }
                    }
                    CodexLaneUpdate::Notification(CodexLaneNotification::ModelsLoaded {
                        models,
                        default_model,
                    }) => {
                        models_loaded = Some((models, default_model));
                    }
                    CodexLaneUpdate::Notification(_) | CodexLaneUpdate::CommandResponse(_) => {}
                }
            }
            std::thread::sleep(Duration::from_millis(10));
        }

        assert!(saw_ready, "missing ready snapshot");
        assert_eq!(active_thread_id.as_deref(), Some("thread-bootstrap"));
        assert!(saw_model_list.load(Ordering::SeqCst));
        let (models, default_model) = models_loaded.expect("expected models loaded notification");
        assert_eq!(models, vec!["o4-mini".to_string()]);
        assert_eq!(default_model.as_deref(), Some("o4-mini"));

        worker.shutdown();
        let _ = server.join();
    }

    #[test]
    fn disconnect_transitions_to_disconnected_state() {
        let (client_stream, server_stream) = tokio::io::duplex(16 * 1024);
        let (client_read, client_write) = tokio::io::split(client_stream);
        let (server_read, mut server_write) = tokio::io::split(server_stream);
        let runtime_guard = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap_or_else(|_| panic!("failed to build runtime"));
        let _entered = runtime_guard.enter();
        let (client, channels) =
            AppServerClient::connect_with_io(Box::new(client_write), Box::new(client_read), None);
        drop(_entered);

        let server = std::thread::spawn(move || {
            let runtime = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(runtime) => runtime,
                Err(_) => return,
            };
            runtime.block_on(async move {
                let mut reader = BufReader::new(server_read);
                let mut request_line = String::new();
                let mut handled_requests = 0usize;
                loop {
                    request_line.clear();
                    let bytes = reader.read_line(&mut request_line).await.unwrap_or(0);
                    if bytes == 0 {
                        break;
                    }
                    let value: Value = match serde_json::from_str(request_line.trim()) {
                        Ok(value) => value,
                        Err(_) => continue,
                    };
                    if value.get("id").is_none() {
                        continue;
                    }
                    handled_requests = handled_requests.saturating_add(1);
                    let method = value
                        .get("method")
                        .and_then(Value::as_str)
                        .unwrap_or_default();
                    let response = match method {
                        "initialize" => json!({
                            "id": value["id"].clone(),
                            "result": {"userAgent": "test-agent"}
                        }),
                        "thread/start" => json!({
                            "id": value["id"].clone(),
                            "result": {
                                "thread": {"id": "thread-bootstrap"},
                                "model": "gpt-5.3-codex"
                            }
                        }),
                        _ => json!({
                            "id": value["id"].clone(),
                            "result": {}
                        }),
                    };
                    if let Ok(line) = serde_json::to_string(&response) {
                        let _ = server_write.write_all(format!("{line}\n").as_bytes()).await;
                    }
                    let _ = server_write.flush().await;
                    if handled_requests >= 2 {
                        break;
                    }
                }
                drop(server_write);
            });
        });

        let mut worker = CodexLaneWorker::spawn_with_runtime(
            CodexLaneConfig::default(),
            Box::new(SingleClientRuntime::new((client, channels), runtime_guard)),
        );

        let _ = wait_for_snapshot(&mut worker, Duration::from_secs(2), |snapshot| {
            snapshot.lifecycle == CodexLaneLifecycle::Ready
        });

        let enqueue_result =
            worker.enqueue(7, CodexLaneCommand::ThreadList(ThreadListParams::default()));
        assert!(enqueue_result.is_ok());

        let disconnected = wait_for_snapshot(&mut worker, Duration::from_secs(2), |snapshot| {
            snapshot.lifecycle == CodexLaneLifecycle::Disconnected
                || snapshot.lifecycle == CodexLaneLifecycle::Error
        });
        assert!(
            matches!(
                disconnected.lifecycle,
                CodexLaneLifecycle::Disconnected | CodexLaneLifecycle::Error
            ),
            "expected disconnected/error lifecycle, got {:?}",
            disconnected.lifecycle
        );

        worker.shutdown();
        let _ = server.join();
    }

    #[test]
    fn turn_lifecycle_notifications_are_forwarded() {
        let (client_stream, server_stream) = tokio::io::duplex(16 * 1024);
        let (client_read, client_write) = tokio::io::split(client_stream);
        let (server_read, mut server_write) = tokio::io::split(server_stream);
        let runtime_guard = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap_or_else(|_| panic!("failed to build runtime"));
        let _entered = runtime_guard.enter();
        let (client, channels) =
            AppServerClient::connect_with_io(Box::new(client_write), Box::new(client_read), None);
        drop(_entered);

        let server = std::thread::spawn(move || {
            let runtime = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(runtime) => runtime,
                Err(_) => return,
            };
            runtime.block_on(async move {
                let mut reader = BufReader::new(server_read);
                let mut request_line = String::new();
                let mut bootstrapped = false;
                loop {
                    request_line.clear();
                    let bytes = reader.read_line(&mut request_line).await.unwrap_or(0);
                    if bytes == 0 {
                        break;
                    }
                    let value: Value = match serde_json::from_str(request_line.trim()) {
                        Ok(value) => value,
                        Err(_) => continue,
                    };
                    if value.get("id").is_none() {
                        continue;
                    }

                    let method = value
                        .get("method")
                        .and_then(Value::as_str)
                        .unwrap_or_default();
                    let response = match method {
                        "initialize" => json!({
                            "id": value["id"].clone(),
                            "result": {"userAgent": "test-agent"}
                        }),
                        "thread/start" => {
                            bootstrapped = true;
                            json!({
                                "id": value["id"].clone(),
                                "result": {
                                    "thread": {"id": "thread-bootstrap"},
                                    "model": "gpt-5.3-codex"
                                }
                            })
                        }
                        _ => json!({
                            "id": value["id"].clone(),
                            "result": {}
                        }),
                    };

                    if let Ok(line) = serde_json::to_string(&response) {
                        let _ = server_write.write_all(format!("{line}\n").as_bytes()).await;
                        let _ = server_write.flush().await;
                    }

                    if bootstrapped {
                        let notifications = [
                            json!({
                                "jsonrpc": "2.0",
                                "method": "turn/started",
                                "params": {
                                    "threadId": "thread-bootstrap",
                                    "turn": {"id": "turn-1"}
                                }
                            }),
                            json!({
                                "jsonrpc": "2.0",
                                "method": "item/agentMessage/delta",
                                "params": {
                                    "threadId": "thread-bootstrap",
                                    "turnId": "turn-1",
                                    "itemId": "item-1",
                                    "delta": "hello world"
                                }
                            }),
                            json!({
                                "jsonrpc": "2.0",
                                "method": "turn/completed",
                                "params": {
                                    "threadId": "thread-bootstrap",
                                    "turn": {"id": "turn-1"}
                                }
                            }),
                            json!({
                                "jsonrpc": "2.0",
                                "method": "error",
                                "params": {
                                    "threadId": "thread-bootstrap",
                                    "turnId": "turn-1",
                                    "willRetry": false,
                                    "error": {"message": "boom"}
                                }
                            }),
                        ];
                        for notification in notifications {
                            if let Ok(line) = serde_json::to_string(&notification) {
                                let _ =
                                    server_write.write_all(format!("{line}\n").as_bytes()).await;
                                let _ = server_write.flush().await;
                            }
                        }
                        tokio::time::sleep(Duration::from_millis(50)).await;
                        break;
                    }
                }
                drop(server_write);
            });
        });

        let mut worker = CodexLaneWorker::spawn_with_runtime(
            CodexLaneConfig::default(),
            Box::new(SingleClientRuntime::new((client, channels), runtime_guard)),
        );

        let mut saw_ready = false;
        let mut saw_turn_started = false;
        let mut saw_delta = false;
        let mut saw_turn_completed = false;
        let mut saw_turn_error = false;
        let deadline = Instant::now() + Duration::from_secs(2);
        while Instant::now() <= deadline
            && !(saw_ready && saw_turn_started && saw_delta && saw_turn_completed && saw_turn_error)
        {
            for update in worker.drain_updates() {
                match update {
                    CodexLaneUpdate::Snapshot(snapshot) => {
                        if snapshot.lifecycle == CodexLaneLifecycle::Ready {
                            saw_ready = true;
                        }
                    }
                    CodexLaneUpdate::Notification(notification) => match notification {
                        CodexLaneNotification::TurnStarted { thread_id, turn_id } => {
                            if thread_id == "thread-bootstrap" && turn_id == "turn-1" {
                                saw_turn_started = true;
                            }
                        }
                        CodexLaneNotification::AgentMessageDelta {
                            thread_id,
                            turn_id,
                            item_id,
                            delta,
                        } => {
                            if thread_id == "thread-bootstrap"
                                && turn_id == "turn-1"
                                && item_id == "item-1"
                                && delta == "hello world"
                            {
                                saw_delta = true;
                            }
                        }
                        CodexLaneNotification::TurnCompleted {
                            thread_id, turn_id, ..
                        } => {
                            if thread_id == "thread-bootstrap" && turn_id == "turn-1" {
                                saw_turn_completed = true;
                            }
                        }
                        CodexLaneNotification::TurnError {
                            thread_id,
                            turn_id,
                            message,
                        } => {
                            if thread_id == "thread-bootstrap"
                                && turn_id == "turn-1"
                                && message == "boom"
                            {
                                saw_turn_error = true;
                            }
                        }
                        _ => {}
                    },
                    CodexLaneUpdate::CommandResponse(_) => {}
                }
            }
            std::thread::sleep(Duration::from_millis(10));
        }

        assert!(saw_ready, "missing ready snapshot");
        assert!(saw_turn_started, "missing turn/started notification");
        assert!(saw_delta, "missing item/agentMessage/delta notification");
        assert!(saw_turn_completed, "missing turn/completed notification");
        assert!(saw_turn_error, "missing error notification");

        worker.shutdown();
        let _ = server.join();
    }

    #[test]
    fn thread_lifecycle_notifications_are_normalized() {
        let started = normalize_notification(codex_client::AppServerNotification {
            method: "thread/started".to_string(),
            params: Some(json!({
                "threadId": "thread-0"
            })),
        });
        assert_eq!(
            started,
            Some(CodexLaneNotification::ThreadStatusChanged {
                thread_id: "thread-0".to_string(),
                status: "active".to_string(),
            })
        );

        let status = normalize_notification(codex_client::AppServerNotification {
            method: "thread/status/changed".to_string(),
            params: Some(json!({
                "threadId": "thread-1",
                "status": {"type": "active", "activeFlags": ["waitingOnApproval"]}
            })),
        });
        assert_eq!(
            status,
            Some(CodexLaneNotification::ThreadStatusChanged {
                thread_id: "thread-1".to_string(),
                status: "active:waitingOnApproval".to_string(),
            })
        );

        let archived = normalize_notification(codex_client::AppServerNotification {
            method: "thread/archived".to_string(),
            params: Some(json!({
                "threadId": "thread-2"
            })),
        });
        assert_eq!(
            archived,
            Some(CodexLaneNotification::ThreadArchived {
                thread_id: "thread-2".to_string(),
            })
        );

        let renamed = normalize_notification(codex_client::AppServerNotification {
            method: "thread/name/updated".to_string(),
            params: Some(json!({
                "threadId": "thread-3",
                "threadName": "Renamed Thread"
            })),
        });
        assert_eq!(
            renamed,
            Some(CodexLaneNotification::ThreadNameUpdated {
                thread_id: "thread-3".to_string(),
                thread_name: Some("Renamed Thread".to_string()),
            })
        );

        let oauth_completed = normalize_notification(codex_client::AppServerNotification {
            method: "mcpServer/oauthLogin/completed".to_string(),
            params: Some(json!({
                "name": "github",
                "success": true,
                "error": null
            })),
        });
        assert_eq!(
            oauth_completed,
            Some(CodexLaneNotification::McpServerOauthLoginCompleted {
                server_name: "github".to_string(),
                success: true,
                error: None,
            })
        );

        let app_list_updated = normalize_notification(codex_client::AppServerNotification {
            method: "app/list/updated".to_string(),
            params: None,
        });
        assert_eq!(
            app_list_updated,
            Some(CodexLaneNotification::AppsListUpdated)
        );

        let fuzzy_updated = normalize_notification(codex_client::AppServerNotification {
            method: "fuzzyFileSearch/sessionUpdated".to_string(),
            params: Some(json!({
                "sessionId": "session-1",
                "status": "indexing"
            })),
        });
        assert_eq!(
            fuzzy_updated,
            Some(CodexLaneNotification::FuzzySessionUpdated {
                session_id: "session-1".to_string(),
                status: "indexing".to_string(),
            })
        );

        let fuzzy_completed = normalize_notification(codex_client::AppServerNotification {
            method: "fuzzyFileSearch/sessionCompleted".to_string(),
            params: Some(json!({
                "sessionId": "session-1"
            })),
        });
        assert_eq!(
            fuzzy_completed,
            Some(CodexLaneNotification::FuzzySessionCompleted {
                session_id: "session-1".to_string(),
            })
        );

        let realtime_started = normalize_notification(codex_client::AppServerNotification {
            method: "thread/realtime/started".to_string(),
            params: Some(json!({
                "threadId": "thread-rt",
                "sessionId": "rt-session"
            })),
        });
        assert_eq!(
            realtime_started,
            Some(CodexLaneNotification::RealtimeStarted {
                thread_id: "thread-rt".to_string(),
                session_id: Some("rt-session".to_string()),
            })
        );

        let windows_setup = normalize_notification(codex_client::AppServerNotification {
            method: "windowsSandbox/setupCompleted".to_string(),
            params: Some(json!({
                "mode": "enable",
                "success": true
            })),
        });
        assert_eq!(
            windows_setup,
            Some(CodexLaneNotification::WindowsSandboxSetupCompleted {
                mode: Some("enable".to_string()),
                success: Some(true),
            })
        );
    }

    #[test]
    fn thread_read_parser_handles_camel_case_items() {
        let thread = codex_client::ThreadSnapshot {
            id: "thread-1".to_string(),
            preview: "preview".to_string(),
            turns: vec![codex_client::ThreadTurn {
                id: "turn-1".to_string(),
                items: vec![
                    json!({
                        "type": "userMessage",
                        "content": [
                            {"type": "text", "text": "hello from user"}
                        ]
                    }),
                    json!({
                        "type": "agentMessage",
                        "text": "hello from codex"
                    }),
                ],
            }],
        };

        let messages = extract_thread_transcript_messages(&thread);
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].role, CodexThreadTranscriptRole::User);
        assert_eq!(messages[0].content, "hello from user");
        assert_eq!(messages[1].role, CodexThreadTranscriptRole::Codex);
        assert_eq!(messages[1].content, "hello from codex");
    }

    #[test]
    fn apps_and_remote_skill_export_emit_notifications() {
        let (client_stream, server_stream) = tokio::io::duplex(16 * 1024);
        let (client_read, client_write) = tokio::io::split(client_stream);
        let (server_read, mut server_write) = tokio::io::split(server_stream);
        let runtime_guard = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap_or_else(|_| panic!("failed to build runtime"));
        let _entered = runtime_guard.enter();
        let (client, channels) =
            AppServerClient::connect_with_io(Box::new(client_write), Box::new(client_read), None);
        drop(_entered);

        let saw_app_list = Arc::new(AtomicBool::new(false));
        let saw_export = Arc::new(AtomicBool::new(false));
        let saw_app_list_clone = Arc::clone(&saw_app_list);
        let saw_export_clone = Arc::clone(&saw_export);
        let server = std::thread::spawn(move || {
            let runtime = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(runtime) => runtime,
                Err(_) => return,
            };
            runtime.block_on(async move {
                let mut reader = BufReader::new(server_read);
                let mut request_line = String::new();
                let mut handled_app = false;
                let mut handled_export = false;
                loop {
                    request_line.clear();
                    let bytes = reader.read_line(&mut request_line).await.unwrap_or(0);
                    if bytes == 0 {
                        break;
                    }
                    let value: Value = match serde_json::from_str(request_line.trim()) {
                        Ok(value) => value,
                        Err(_) => continue,
                    };
                    if value.get("id").is_none() {
                        continue;
                    }
                    let method = value
                        .get("method")
                        .and_then(Value::as_str)
                        .unwrap_or_default();
                    let response = match method {
                        "initialize" => json!({
                            "id": value["id"].clone(),
                            "result": {"userAgent": "test-agent"}
                        }),
                        "thread/start" => json!({
                            "id": value["id"].clone(),
                            "result": {
                                "thread": {"id": "thread-bootstrap"},
                                "model": "gpt-5.3-codex"
                            }
                        }),
                        "app/list" => {
                            handled_app = true;
                            saw_app_list_clone.store(true, Ordering::SeqCst);
                            json!({
                                "id": value["id"].clone(),
                                "result": {
                                    "data": [
                                        {
                                            "id": "github",
                                            "name": "GitHub",
                                            "description": "Code hosting",
                                            "isAccessible": true,
                                            "isEnabled": true
                                        }
                                    ],
                                    "nextCursor": null
                                }
                            })
                        }
                        "skills/remote/export" => {
                            handled_export = true;
                            saw_export_clone.store(true, Ordering::SeqCst);
                            json!({
                                "id": value["id"].clone(),
                                "result": {
                                    "id": "skill-1",
                                    "path": "/tmp/skill-1"
                                }
                            })
                        }
                        _ => json!({
                            "id": value["id"].clone(),
                            "result": {}
                        }),
                    };
                    if let Ok(line) = serde_json::to_string(&response) {
                        let _ = server_write.write_all(format!("{line}\n").as_bytes()).await;
                        let _ = server_write.flush().await;
                    }
                    if handled_app && handled_export {
                        break;
                    }
                }
                drop(server_write);
            });
        });

        let mut worker = CodexLaneWorker::spawn_with_runtime(
            CodexLaneConfig::default(),
            Box::new(SingleClientRuntime::new((client, channels), runtime_guard)),
        );

        let _ = wait_for_snapshot(&mut worker, Duration::from_secs(2), |snapshot| {
            snapshot.lifecycle == CodexLaneLifecycle::Ready
        });

        let app_enqueue =
            worker.enqueue(901, CodexLaneCommand::AppsList(AppsListParams::default()));
        assert!(app_enqueue.is_ok(), "failed to enqueue app/list");
        let app_response = wait_for_command_response(&mut worker, Duration::from_secs(2), |resp| {
            resp.command_seq == 901
        });

        let export_enqueue = worker.enqueue(
            902,
            CodexLaneCommand::SkillsRemoteExport(SkillsRemoteWriteParams {
                hazelnut_id: "skill-1".to_string(),
            }),
        );
        assert!(
            export_enqueue.is_ok(),
            "failed to enqueue skills/remote/export"
        );
        let export_response =
            wait_for_command_response(&mut worker, Duration::from_secs(2), |resp| {
                resp.command_seq == 902
            });
        assert_eq!(app_response.command, CodexLaneCommandKind::AppsList);
        assert_eq!(app_response.status, CodexLaneCommandStatus::Accepted);
        assert_eq!(
            export_response.command,
            CodexLaneCommandKind::SkillsRemoteExport
        );
        assert_eq!(export_response.status, CodexLaneCommandStatus::Accepted);

        assert!(saw_app_list.load(Ordering::SeqCst));
        assert!(saw_export.load(Ordering::SeqCst));

        worker.shutdown();
        let _ = server.join();
    }

    #[test]
    fn server_request_command_approval_round_trip() {
        let (client_stream, server_stream) = tokio::io::duplex(16 * 1024);
        let (client_read, client_write) = tokio::io::split(client_stream);
        let (server_read, mut server_write) = tokio::io::split(server_stream);
        let runtime_guard = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap_or_else(|_| panic!("failed to build runtime"));
        let _entered = runtime_guard.enter();
        let (client, channels) =
            AppServerClient::connect_with_io(Box::new(client_write), Box::new(client_read), None);
        drop(_entered);

        let saw_approval_response = Arc::new(AtomicBool::new(false));
        let saw_approval_response_clone = Arc::clone(&saw_approval_response);
        let server = std::thread::spawn(move || {
            let runtime = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(runtime) => runtime,
                Err(_) => return,
            };
            runtime.block_on(async move {
                let mut reader = BufReader::new(server_read);
                let mut request_line = String::new();
                let mut sent_approval_request = false;

                loop {
                    request_line.clear();
                    let bytes = reader.read_line(&mut request_line).await.unwrap_or(0);
                    if bytes == 0 {
                        break;
                    }
                    let value: Value = match serde_json::from_str(request_line.trim()) {
                        Ok(value) => value,
                        Err(_) => continue,
                    };

                    let method = value
                        .get("method")
                        .and_then(Value::as_str)
                        .unwrap_or_default();

                    if method == "initialize" {
                        let response = json!({
                            "id": value["id"].clone(),
                            "result": {"userAgent": "test-agent"}
                        });
                        if let Ok(line) = serde_json::to_string(&response) {
                            let _ = server_write.write_all(format!("{line}\n").as_bytes()).await;
                            let _ = server_write.flush().await;
                        }
                        continue;
                    }

                    if method == "thread/start" {
                        let response = json!({
                            "id": value["id"].clone(),
                            "result": {
                                "thread": {"id": "thread-bootstrap"},
                                "model": "gpt-5.3-codex"
                            }
                        });
                        if let Ok(line) = serde_json::to_string(&response) {
                            let _ = server_write.write_all(format!("{line}\n").as_bytes()).await;
                            let _ = server_write.flush().await;
                        }
                        continue;
                    }

                    if method == "model/list" {
                        let response = json!({
                            "id": value["id"].clone(),
                            "result": {
                                "data": [],
                                "nextCursor": null
                            }
                        });
                        if let Ok(line) = serde_json::to_string(&response) {
                            let _ = server_write.write_all(format!("{line}\n").as_bytes()).await;
                            let _ = server_write.flush().await;
                        }
                        continue;
                    }

                    if method == "thread/list" {
                        let response = json!({
                            "id": value["id"].clone(),
                            "result": {"data": [], "nextCursor": null}
                        });
                        if let Ok(line) = serde_json::to_string(&response) {
                            let _ = server_write.write_all(format!("{line}\n").as_bytes()).await;
                            let _ = server_write.flush().await;
                        }

                        let approval_request = json!({
                            "jsonrpc": "2.0",
                            "id": "approve-1",
                            "method": "item/commandExecution/requestApproval",
                            "params": {
                                "threadId": "thread-bootstrap",
                                "turnId": "turn-1",
                                "itemId": "item-1",
                                "reason": "needs approval",
                                "command": "ls"
                            }
                        });
                        if let Ok(line) = serde_json::to_string(&approval_request) {
                            let _ = server_write.write_all(format!("{line}\n").as_bytes()).await;
                            let _ = server_write.flush().await;
                        }
                        sent_approval_request = true;
                        continue;
                    }

                    if sent_approval_request {
                        let id = value.get("id");
                        let decision = value
                            .get("result")
                            .and_then(|result| result.get("decision"))
                            .and_then(Value::as_str);
                        if id == Some(&json!("approve-1")) && decision == Some("accept") {
                            saw_approval_response_clone.store(true, Ordering::SeqCst);
                            break;
                        }
                    }
                }
                drop(server_write);
            });
        });

        let mut worker = CodexLaneWorker::spawn_with_runtime(
            CodexLaneConfig::default(),
            Box::new(SingleClientRuntime::new((client, channels), runtime_guard)),
        );
        let _ = wait_for_snapshot(&mut worker, Duration::from_secs(2), |snapshot| {
            snapshot.lifecycle == CodexLaneLifecycle::Ready
        });
        let _ = worker.enqueue(
            990,
            CodexLaneCommand::ThreadList(ThreadListParams::default()),
        );

        let mut approval_request_id = None;
        let deadline = Instant::now() + Duration::from_secs(2);
        while Instant::now() <= deadline && approval_request_id.is_none() {
            for update in worker.drain_updates() {
                if let CodexLaneUpdate::Notification(
                    CodexLaneNotification::CommandApprovalRequested { request_id, .. },
                ) = update
                {
                    approval_request_id = Some(request_id);
                }
            }
            std::thread::sleep(Duration::from_millis(10));
        }

        let Some(request_id) = approval_request_id else {
            worker.shutdown();
            let _ = server.join();
            panic!("expected command approval request");
        };
        let enqueue_result = worker.enqueue(
            991,
            CodexLaneCommand::ServerRequestCommandApprovalRespond {
                request_id,
                response: codex_client::CommandExecutionRequestApprovalResponse {
                    decision: codex_client::ApprovalDecision::Accept,
                },
            },
        );
        assert!(enqueue_result.is_ok());

        let response = wait_for_command_response(&mut worker, Duration::from_secs(2), |response| {
            response.command_seq == 991
        });
        assert_eq!(
            response.command,
            CodexLaneCommandKind::ServerRequestCommandApprovalRespond
        );
        assert_eq!(response.status, CodexLaneCommandStatus::Accepted);
        assert!(saw_approval_response.load(Ordering::SeqCst));

        worker.shutdown();
        let _ = server.join();
    }

    #[test]
    fn command_routing_sends_thread_list_request() {
        let (client_stream, server_stream) = tokio::io::duplex(16 * 1024);
        let (client_read, client_write) = tokio::io::split(client_stream);
        let (server_read, mut server_write) = tokio::io::split(server_stream);
        let runtime_guard = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap_or_else(|_| panic!("failed to build runtime"));
        let _entered = runtime_guard.enter();
        let (client, channels) =
            AppServerClient::connect_with_io(Box::new(client_write), Box::new(client_read), None);
        drop(_entered);

        let saw_thread_list = Arc::new(AtomicBool::new(false));
        let saw_thread_list_clone = Arc::clone(&saw_thread_list);
        let server = std::thread::spawn(move || {
            let runtime = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(runtime) => runtime,
                Err(_) => return,
            };
            runtime.block_on(async move {
                let mut reader = BufReader::new(server_read);
                let mut request_line = String::new();
                let mut done = false;
                loop {
                    request_line.clear();
                    let bytes = reader.read_line(&mut request_line).await.unwrap_or(0);
                    if bytes == 0 {
                        break;
                    }
                    let value: Value = match serde_json::from_str(request_line.trim()) {
                        Ok(value) => value,
                        Err(_) => continue,
                    };
                    if value.get("id").is_none() {
                        continue;
                    }

                    let method = value
                        .get("method")
                        .and_then(Value::as_str)
                        .unwrap_or_default();
                    let response = match method {
                        "initialize" => json!({
                            "id": value["id"].clone(),
                            "result": {"userAgent": "test-agent"}
                        }),
                        "thread/start" => json!({
                            "id": value["id"].clone(),
                            "result": {
                                "thread": {"id": "thread-bootstrap"},
                                "model": "gpt-5.3-codex"
                            }
                        }),
                        "thread/list" => {
                            done = true;
                            saw_thread_list_clone.store(true, Ordering::SeqCst);
                            json!({
                                "id": value["id"].clone(),
                                "result": {"data": [], "nextCursor": null}
                            })
                        }
                        _ => json!({
                            "id": value["id"].clone(),
                            "result": {}
                        }),
                    };

                    if let Ok(line) = serde_json::to_string(&response) {
                        let _ = server_write.write_all(format!("{line}\n").as_bytes()).await;
                        let _ = server_write.flush().await;
                    }
                    if done {
                        break;
                    }
                }
                drop(server_write);
            });
        });

        let mut worker = CodexLaneWorker::spawn_with_runtime(
            CodexLaneConfig::default(),
            Box::new(SingleClientRuntime::new((client, channels), runtime_guard)),
        );

        let _ = wait_for_snapshot(&mut worker, Duration::from_secs(2), |snapshot| {
            snapshot.lifecycle == CodexLaneLifecycle::Ready
        });

        let enqueue_result = worker.enqueue(
            42,
            CodexLaneCommand::ThreadList(ThreadListParams::default()),
        );
        assert!(enqueue_result.is_ok());

        let response = wait_for_command_response(&mut worker, Duration::from_secs(2), |response| {
            response.command_seq == 42
        });
        assert_eq!(response.command, CodexLaneCommandKind::ThreadList);
        assert_eq!(response.status, CodexLaneCommandStatus::Accepted);
        assert!(saw_thread_list.load(Ordering::SeqCst));

        worker.shutdown();
        let _ = server.join();
    }

    #[test]
    fn command_routing_sends_skills_list_request_with_extra_roots() {
        let fixture_root = unique_fixture_root("skills-list");
        let fixture_skills_root = fixture_root.join("skills");
        assert!(fs::create_dir_all(&fixture_skills_root).is_ok());

        let (client_stream, server_stream) = tokio::io::duplex(16 * 1024);
        let (client_read, client_write) = tokio::io::split(client_stream);
        let (server_read, mut server_write) = tokio::io::split(server_stream);
        let runtime_guard = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap_or_else(|_| panic!("failed to build runtime"));
        let _entered = runtime_guard.enter();
        let (client, channels) =
            AppServerClient::connect_with_io(Box::new(client_write), Box::new(client_read), None);
        drop(_entered);

        let saw_skills_list = Arc::new(AtomicBool::new(false));
        let saw_skills_list_clone = Arc::clone(&saw_skills_list);
        let expected_cwd = fixture_root.display().to_string();
        let expected_root = fixture_skills_root.display().to_string();
        let server = std::thread::spawn(move || {
            let runtime = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(runtime) => runtime,
                Err(_) => return,
            };
            runtime.block_on(async move {
                let mut reader = BufReader::new(server_read);
                let mut request_line = String::new();
                let mut done = false;
                loop {
                    request_line.clear();
                    let bytes = reader.read_line(&mut request_line).await.unwrap_or(0);
                    if bytes == 0 {
                        break;
                    }
                    let value: Value = match serde_json::from_str(request_line.trim()) {
                        Ok(value) => value,
                        Err(_) => continue,
                    };
                    if value.get("id").is_none() {
                        continue;
                    }

                    let method = value
                        .get("method")
                        .and_then(Value::as_str)
                        .unwrap_or_default();
                    let response = match method {
                        "initialize" => json!({
                            "id": value["id"].clone(),
                            "result": {"userAgent": "test-agent"}
                        }),
                        "thread/start" => json!({
                            "id": value["id"].clone(),
                            "result": {
                                "thread": {"id": "thread-bootstrap"},
                                "model": "gpt-5.3-codex"
                            }
                        }),
                        "skills/list" => {
                            saw_skills_list_clone.store(true, Ordering::SeqCst);
                            done = true;
                            assert_eq!(
                                value["params"],
                                json!({
                                    "cwds": [expected_cwd],
                                    "forceReload": true,
                                    "perCwdExtraUserRoots": [
                                        {
                                            "cwd": expected_cwd,
                                            "extraUserRoots": [expected_root]
                                        }
                                    ]
                                })
                            );
                            json!({
                                "id": value["id"].clone(),
                                "result": {
                                    "data": [
                                        {
                                            "cwd": expected_cwd,
                                            "skills": [],
                                            "errors": []
                                        }
                                    ]
                                }
                            })
                        }
                        _ => json!({
                            "id": value["id"].clone(),
                            "result": {}
                        }),
                    };
                    if let Ok(line) = serde_json::to_string(&response) {
                        let _ = server_write.write_all(format!("{line}\n").as_bytes()).await;
                        let _ = server_write.flush().await;
                    }
                    if done {
                        break;
                    }
                }
                drop(server_write);
            });
        });

        let mut worker = CodexLaneWorker::spawn_with_runtime(
            CodexLaneConfig::default(),
            Box::new(SingleClientRuntime::new((client, channels), runtime_guard)),
        );

        let _ = wait_for_snapshot(&mut worker, Duration::from_secs(2), |snapshot| {
            snapshot.lifecycle == CodexLaneLifecycle::Ready
        });

        let enqueue_result = worker.enqueue(
            88,
            CodexLaneCommand::SkillsList(SkillsListParams {
                cwds: vec![fixture_root.clone()],
                force_reload: true,
                per_cwd_extra_user_roots: Some(vec![SkillsListExtraRootsForCwd {
                    cwd: fixture_root.clone(),
                    extra_user_roots: vec![fixture_skills_root.clone()],
                }]),
            }),
        );
        assert!(enqueue_result.is_ok());

        let response = wait_for_command_response(&mut worker, Duration::from_secs(2), |response| {
            response.command_seq == 88
        });
        assert_eq!(response.command, CodexLaneCommandKind::SkillsList);
        assert_eq!(response.status, CodexLaneCommandStatus::Accepted);
        assert!(saw_skills_list.load(Ordering::SeqCst));

        worker.shutdown();
        let _ = server.join();
        let _ = fs::remove_dir_all(&fixture_root);
    }

    #[test]
    fn labs_api_smoke_commands_emit_responses_and_notifications() {
        let (client_stream, server_stream) = tokio::io::duplex(16 * 1024);
        let (client_read, client_write) = tokio::io::split(client_stream);
        let (server_read, mut server_write) = tokio::io::split(server_stream);
        let runtime_guard = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap_or_else(|_| panic!("failed to build runtime"));
        let _entered = runtime_guard.enter();
        let (client, channels) =
            AppServerClient::connect_with_io(Box::new(client_write), Box::new(client_read), None);
        drop(_entered);

        let server = std::thread::spawn(move || {
            let runtime = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(runtime) => runtime,
                Err(_) => return,
            };
            runtime.block_on(async move {
                let mut reader = BufReader::new(server_read);
                let mut request_line = String::new();
                let expected = [
                    "review/start",
                    "command/exec",
                    "collaborationMode/list",
                    "experimentalFeature/list",
                    "thread/realtime/start",
                    "thread/realtime/appendText",
                    "thread/realtime/stop",
                    "windowsSandbox/setupStart",
                    "fuzzyFileSearch/sessionStart",
                    "fuzzyFileSearch/sessionUpdate",
                    "fuzzyFileSearch/sessionStop",
                ];
                let mut seen = HashSet::<String>::new();

                loop {
                    request_line.clear();
                    let bytes = reader.read_line(&mut request_line).await.unwrap_or(0);
                    if bytes == 0 {
                        break;
                    }
                    let value: Value = match serde_json::from_str(request_line.trim()) {
                        Ok(value) => value,
                        Err(_) => continue,
                    };
                    if value.get("id").is_none() {
                        continue;
                    }
                    let method = value
                        .get("method")
                        .and_then(Value::as_str)
                        .unwrap_or_default();
                    if expected.contains(&method) {
                        seen.insert(method.to_string());
                    }
                    let response = match method {
                        "initialize" => json!({
                            "id": value["id"].clone(),
                            "result": {"userAgent": "test-agent"}
                        }),
                        "thread/start" => json!({
                            "id": value["id"].clone(),
                            "result": {
                                "thread": {"id": "thread-bootstrap"},
                                "model": "gpt-5.3-codex"
                            }
                        }),
                        "model/list" => json!({
                            "id": value["id"].clone(),
                            "result": {
                                "data": [
                                    {
                                        "id": "default",
                                        "model": "gpt-5.3-codex",
                                        "displayName": "gpt-5.3-codex",
                                        "description": "default",
                                        "supportedReasoningEfforts": [],
                                        "defaultReasoningEffort": "medium",
                                        "isDefault": true
                                    }
                                ],
                                "nextCursor": null
                            }
                        }),
                        "review/start" => json!({
                            "id": value["id"].clone(),
                            "result": {
                                "turn": {"id": "turn-review"},
                                "reviewThreadId": "review-thread-1"
                            }
                        }),
                        "command/exec" => json!({
                            "id": value["id"].clone(),
                            "result": {
                                "exitCode": 0,
                                "stdout": "ok",
                                "stderr": ""
                            }
                        }),
                        "collaborationMode/list" => json!({
                            "id": value["id"].clone(),
                            "result": {"data": [{"id": "pair"}]}
                        }),
                        "experimentalFeature/list" => json!({
                            "id": value["id"].clone(),
                            "result": {"data": [{"id": "feature"}], "nextCursor": null}
                        }),
                        "thread/realtime/start"
                        | "thread/realtime/appendText"
                        | "thread/realtime/stop"
                        | "fuzzyFileSearch/sessionStart"
                        | "fuzzyFileSearch/sessionUpdate"
                        | "fuzzyFileSearch/sessionStop" => json!({
                            "id": value["id"].clone(),
                            "result": {}
                        }),
                        "windowsSandbox/setupStart" => json!({
                            "id": value["id"].clone(),
                            "result": {"started": true}
                        }),
                        _ => json!({
                            "id": value["id"].clone(),
                            "result": {}
                        }),
                    };

                    if let Ok(line) = serde_json::to_string(&response) {
                        let _ = server_write.write_all(format!("{line}\n").as_bytes()).await;
                        let _ = server_write.flush().await;
                    }
                    if seen.len() == expected.len() {
                        break;
                    }
                }
                drop(server_write);
            });
        });

        let mut worker = CodexLaneWorker::spawn_with_runtime(
            CodexLaneConfig::default(),
            Box::new(SingleClientRuntime::new((client, channels), runtime_guard)),
        );
        let _ = wait_for_snapshot(&mut worker, Duration::from_secs(2), |snapshot| {
            snapshot.lifecycle == CodexLaneLifecycle::Ready
        });

        let commands = [
            (
                1501,
                CodexLaneCommand::ReviewStart(ReviewStartParams {
                    thread_id: "thread-bootstrap".to_string(),
                    target: ReviewTarget::UncommittedChanges,
                    delivery: Some(codex_client::ReviewDelivery::Inline),
                }),
            ),
            (
                1502,
                CodexLaneCommand::CommandExec(CommandExecParams {
                    command: vec!["pwd".to_string()],
                    timeout_ms: Some(5000),
                    cwd: None,
                    sandbox_policy: None,
                }),
            ),
            (
                1503,
                CodexLaneCommand::CollaborationModeList(CollaborationModeListParams::default()),
            ),
            (
                1504,
                CodexLaneCommand::ExperimentalFeatureList(ExperimentalFeatureListParams {
                    cursor: None,
                    limit: Some(100),
                }),
            ),
            (
                1505,
                CodexLaneCommand::ThreadRealtimeStart(ThreadRealtimeStartParams {
                    thread_id: "thread-bootstrap".to_string(),
                    prompt: "start".to_string(),
                    session_id: Some("session-a".to_string()),
                }),
            ),
            (
                1506,
                CodexLaneCommand::ThreadRealtimeAppendText(ThreadRealtimeAppendTextParams {
                    thread_id: "thread-bootstrap".to_string(),
                    text: "hello".to_string(),
                }),
            ),
            (
                1507,
                CodexLaneCommand::ThreadRealtimeStop(ThreadRealtimeStopParams {
                    thread_id: "thread-bootstrap".to_string(),
                }),
            ),
            (
                1508,
                CodexLaneCommand::WindowsSandboxSetupStart(WindowsSandboxSetupStartParams {
                    mode: "enable".to_string(),
                }),
            ),
            (
                1509,
                CodexLaneCommand::FuzzyFileSearchSessionStart(FuzzyFileSearchSessionStartParams {
                    session_id: "session-a".to_string(),
                    roots: vec![".".to_string()],
                }),
            ),
            (
                1510,
                CodexLaneCommand::FuzzyFileSearchSessionUpdate(
                    FuzzyFileSearchSessionUpdateParams {
                        session_id: "session-a".to_string(),
                        query: "codex".to_string(),
                    },
                ),
            ),
            (
                1511,
                CodexLaneCommand::FuzzyFileSearchSessionStop(FuzzyFileSearchSessionStopParams {
                    session_id: "session-a".to_string(),
                }),
            ),
        ];
        for (seq, command) in commands {
            let enqueue = worker.enqueue(seq, command);
            assert!(enqueue.is_ok(), "failed to enqueue command seq={seq}");
        }

        let mut saw_review = false;
        let mut saw_exec = false;
        let mut saw_collab = false;
        let mut saw_features = false;
        let mut saw_realtime_start = false;
        let mut saw_realtime_append = false;
        let mut saw_realtime_stop = false;
        let mut saw_windows = false;
        let mut saw_fuzzy_start = false;
        let mut saw_fuzzy_update = false;
        let mut saw_fuzzy_stop = false;
        let mut accepted_responses = HashSet::new();

        let deadline = Instant::now() + Duration::from_secs(3);
        while Instant::now() <= deadline {
            for update in worker.drain_updates() {
                match update {
                    CodexLaneUpdate::CommandResponse(response) => {
                        if response.command_seq >= 1501 && response.command_seq <= 1511 {
                            assert_eq!(
                                response.status,
                                CodexLaneCommandStatus::Accepted,
                                "unexpected command rejection: {:?}",
                                response
                            );
                            accepted_responses.insert(response.command_seq);
                        }
                    }
                    CodexLaneUpdate::Notification(notification) => match notification {
                        CodexLaneNotification::ReviewStarted { .. } => saw_review = true,
                        CodexLaneNotification::CommandExecCompleted { .. } => saw_exec = true,
                        CodexLaneNotification::CollaborationModesLoaded { .. } => saw_collab = true,
                        CodexLaneNotification::ExperimentalFeaturesLoaded { .. } => {
                            saw_features = true
                        }
                        CodexLaneNotification::RealtimeStarted { .. } => saw_realtime_start = true,
                        CodexLaneNotification::RealtimeTextAppended { .. } => {
                            saw_realtime_append = true
                        }
                        CodexLaneNotification::RealtimeStopped { .. } => saw_realtime_stop = true,
                        CodexLaneNotification::WindowsSandboxSetupStarted { .. } => {
                            saw_windows = true
                        }
                        CodexLaneNotification::FuzzySessionStarted { .. } => saw_fuzzy_start = true,
                        CodexLaneNotification::FuzzySessionUpdated { .. } => {
                            saw_fuzzy_update = true
                        }
                        CodexLaneNotification::FuzzySessionStopped { .. } => saw_fuzzy_stop = true,
                        _ => {}
                    },
                    CodexLaneUpdate::Snapshot(_) => {}
                }
            }
            if accepted_responses.len() == 11
                && saw_review
                && saw_exec
                && saw_collab
                && saw_features
                && saw_realtime_start
                && saw_realtime_append
                && saw_realtime_stop
                && saw_windows
                && saw_fuzzy_start
                && saw_fuzzy_update
                && saw_fuzzy_stop
            {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }

        assert_eq!(
            accepted_responses.len(),
            11,
            "missing accepted command responses"
        );
        assert!(saw_review, "missing review notification");
        assert!(saw_exec, "missing command/exec notification");
        assert!(saw_collab, "missing collaborationMode/list notification");
        assert!(
            saw_features,
            "missing experimentalFeature/list notification"
        );
        assert!(saw_realtime_start, "missing realtime start notification");
        assert!(saw_realtime_append, "missing realtime append notification");
        assert!(saw_realtime_stop, "missing realtime stop notification");
        assert!(saw_windows, "missing windows sandbox notification");
        assert!(saw_fuzzy_start, "missing fuzzy start notification");
        assert!(saw_fuzzy_update, "missing fuzzy update notification");
        assert!(saw_fuzzy_stop, "missing fuzzy stop notification");

        worker.shutdown();
        let _ = server.join();
    }

    #[test]
    fn wire_log_path_is_forwarded_to_lane_runtime() {
        struct CaptureWireLogRuntime {
            expected: PathBuf,
            saw_expected: Arc<AtomicBool>,
        }

        impl CodexLaneRuntime for CaptureWireLogRuntime {
            fn connect(
                &mut self,
                _runtime: &tokio::runtime::Runtime,
                config: &CodexLaneConfig,
            ) -> Result<(AppServerClient, AppServerChannels)> {
                if config.wire_log_path.as_ref() == Some(&self.expected) {
                    self.saw_expected.store(true, Ordering::SeqCst);
                }
                Err(anyhow::anyhow!("captured wire log config"))
            }
        }

        let expected = unique_fixture_root("wire-log").join("codex-wire.log");
        let saw_expected = Arc::new(AtomicBool::new(false));
        let mut config = CodexLaneConfig::default();
        config.wire_log_path = Some(expected.clone());

        let mut worker = CodexLaneWorker::spawn_with_runtime(
            config,
            Box::new(CaptureWireLogRuntime {
                expected,
                saw_expected: Arc::clone(&saw_expected),
            }),
        );

        let snapshot = wait_for_snapshot(&mut worker, Duration::from_secs(2), |snapshot| {
            snapshot.lifecycle == CodexLaneLifecycle::Error
        });
        assert!(saw_expected.load(Ordering::SeqCst));
        let has_error = snapshot
            .last_error
            .as_deref()
            .is_some_and(|message| message.contains("captured wire log config"));
        assert!(has_error, "expected captured wire log startup failure");

        worker.shutdown();
    }

    fn wait_for_snapshot<F>(
        worker: &mut CodexLaneWorker,
        timeout: Duration,
        predicate: F,
    ) -> super::CodexLaneSnapshot
    where
        F: Fn(&super::CodexLaneSnapshot) -> bool,
    {
        let deadline = Instant::now() + timeout;
        let mut matched: Option<super::CodexLaneSnapshot> = None;
        while Instant::now() <= deadline {
            for update in worker.drain_updates() {
                if let CodexLaneUpdate::Snapshot(snapshot) = update {
                    if predicate(&snapshot) {
                        matched = Some(*snapshot);
                        break;
                    }
                }
            }
            if matched.is_some() {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(
            matched.is_some(),
            "timed out waiting for codex lane snapshot"
        );
        matched.unwrap_or_default()
    }

    fn unique_fixture_root(tag: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |duration| duration.as_nanos());
        std::env::temp_dir().join(format!(
            "openagents-codex-lane-{tag}-{}-{nanos}",
            std::process::id()
        ))
    }

    fn wait_for_command_response<F>(
        worker: &mut CodexLaneWorker,
        timeout: Duration,
        predicate: F,
    ) -> CodexLaneCommandResponse
    where
        F: Fn(&super::CodexLaneCommandResponse) -> bool,
    {
        let deadline = Instant::now() + timeout;
        let mut matched: Option<super::CodexLaneCommandResponse> = None;
        while Instant::now() <= deadline {
            for update in worker.drain_updates() {
                if let CodexLaneUpdate::CommandResponse(response) = update {
                    if predicate(&response) {
                        matched = Some(response);
                        break;
                    }
                }
            }
            if matched.is_some() {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(
            matched.is_some(),
            "timed out waiting for codex lane command response"
        );
        matched.unwrap_or(super::CodexLaneCommandResponse {
            command_seq: 0,
            command: super::CodexLaneCommandKind::ThreadList,
            status: super::CodexLaneCommandStatus::Retryable,
            error: Some("missing command response".to_string()),
        })
    }
}
