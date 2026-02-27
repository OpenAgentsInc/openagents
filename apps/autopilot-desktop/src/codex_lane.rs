use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread::JoinHandle;
use std::time::Duration;

use anyhow::{Context, Result};
use codex_client::{
    AppServerChannels, AppServerClient, AppServerConfig, AppServerNotification, AppServerRequest,
    AskForApproval, ClientInfo, InitializeCapabilities, InitializeParams, ModelListParams,
    SkillScope, SkillsConfigWriteParams, SkillsListParams, SkillsListResponse, ThreadArchiveParams,
    ThreadCompactStartParams, ThreadForkParams, ThreadListParams, ThreadLoadedListParams,
    ThreadReadParams, ThreadResumeParams, ThreadRollbackParams, ThreadSetNameParams,
    ThreadStartParams, ThreadUnarchiveParams, ThreadUnsubscribeParams, TurnInterruptParams,
    TurnStartParams,
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
            bootstrap_model: Some("gpt-5-codex".to_string()),
            client_info: ClientInfo {
                name: "openagents-autopilot-desktop".to_string(),
                title: Some("OpenAgents Autopilot Desktop".to_string()),
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
            approval_policy: Some(AskForApproval::OnRequest),
            experimental_api: false,
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
    ThreadListLoaded {
        entries: Vec<CodexThreadListEntry>,
    },
    ThreadLoadedListLoaded {
        thread_ids: Vec<String>,
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CodexTurnPlanStep {
    pub step: String,
    pub status: String,
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
        runtime
            .block_on(AppServerClient::spawn(AppServerConfig {
                cwd: config.cwd.clone(),
                ..Default::default()
            }))
            .context("failed to spawn codex app-server")
    }
}

struct CodexLaneState {
    snapshot: CodexLaneSnapshot,
    client: Option<AppServerClient>,
    channels: Option<AppServerChannels>,
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

        if let Some(client) = self.client.as_ref() {
            let capabilities = if config.experimental_api
                || !config.opt_out_notification_methods.is_empty()
            {
                Some(InitializeCapabilities {
                    experimental_api: config.experimental_api,
                    opt_out_notification_methods: if config.opt_out_notification_methods.is_empty()
                    {
                        None
                    } else {
                        Some(config.opt_out_notification_methods.clone())
                    },
                })
            } else {
                None
            };
            let initialized = runtime.block_on(client.initialize(InitializeParams {
                client_info: config.client_info.clone(),
                capabilities,
            }));
            if let Err(error) = initialized {
                self.set_error(
                    update_tx,
                    format!("Codex lane initialize failed: {error}"),
                    false,
                );
                return;
            }

            if config.bootstrap_thread {
                let thread_start = ThreadStartParams {
                    model: config.bootstrap_model.clone(),
                    model_provider: None,
                    cwd: config.cwd.as_ref().map(|path| path.display().to_string()),
                    approval_policy: config.approval_policy,
                    sandbox: None,
                };
                let started = runtime.block_on(client.thread_start(thread_start));
                match started {
                    Ok(response) => {
                        let thread_id = response.thread.id;
                        self.snapshot.active_thread_id = Some(thread_id.clone());
                        let _ = update_tx.send(CodexLaneUpdate::Notification(
                            CodexLaneNotification::ThreadSelected {
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
                                    cwd: config
                                        .cwd
                                        .as_ref()
                                        .map(|value| value.display().to_string()),
                                    path: None,
                                }],
                            },
                        ));
                        self.set_ready(update_tx, "Codex lane ready");
                        self.publish_models_from_server(runtime, update_tx);
                    }
                    Err(error) => {
                        self.set_error(
                            update_tx,
                            format!("Codex lane bootstrap thread failed: {error}"),
                            false,
                        );
                    }
                }
            } else {
                self.set_ready(update_tx, "Codex lane ready");
                self.publish_models_from_server(runtime, update_tx);
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

        let result = if let Some(client) = self.client.as_ref() {
            Self::dispatch_command(runtime, client, envelope.command)
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
                    notification: Some(CodexLaneNotification::ThreadSelected { thread_id }),
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
                let thread_id = response.thread.id;
                Ok(CodexCommandEffect {
                    active_thread_id: Some(thread_id.clone()),
                    notification: Some(CodexLaneNotification::ThreadSelected { thread_id }),
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
            let ack = serde_json::json!({ "status": "unsupported" });
            let _ = runtime.block_on(client.respond(request.id, &ack));
        }
    }

    fn shutdown(&mut self, runtime: &Runtime, update_tx: &Sender<CodexLaneUpdate>) {
        if let Some(client) = self.client.take() {
            let _ = runtime.block_on(client.shutdown());
        }
        self.channels = None;
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
            Some(CodexLaneNotification::ThreadStarted { thread_id })
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

fn fetch_model_catalog(
    runtime: &Runtime,
    client: &AppServerClient,
) -> Result<(Vec<String>, Option<String>)> {
    let mut cursor = None;
    let mut raw = Vec::<(String, bool)>::new();

    loop {
        let response = runtime.block_on(client.model_list(ModelListParams {
            cursor: cursor.clone(),
            limit: Some(100),
            include_hidden: None,
        }))?;

        for model in response.data {
            let value = model.model.trim();
            if value.is_empty() {
                continue;
            }
            raw.push((value.to_string(), model.is_default));
        }

        match response.next_cursor {
            Some(next) if !next.is_empty() => {
                cursor = Some(next);
            }
            _ => break,
        }
    }

    let mut seen = HashSet::new();
    let mut models = Vec::new();
    let mut default_model = None;
    for (model, is_default) in raw {
        if !seen.insert(model.clone()) {
            continue;
        }
        if is_default && default_model.is_none() {
            default_model = Some(model.clone());
        }
        models.push(model);
    }

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
        CodexLaneUpdate, CodexLaneWorker, normalize_notification,
    };

    use std::fs;
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

    use anyhow::Result;
    use codex_client::{
        AppServerChannels, AppServerClient, SkillsListExtraRootsForCwd, SkillsListParams,
        ThreadListParams,
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
                                "model": "gpt-5-codex"
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
                                "model": "gpt-5-codex"
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
                                    "model": "gpt-5-codex"
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
                                "model": "gpt-5-codex"
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
                                "model": "gpt-5-codex"
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
