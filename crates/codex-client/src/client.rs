//! Codex app-server JSON-RPC client.

use std::collections::{HashMap, HashSet};
use std::env;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};

use anyhow::{Context, Result};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::{Mutex, mpsc, oneshot};

use crate::types::*;

pub const SUPPORTED_CLIENT_REQUEST_METHODS: &[&str] = &[
    "initialize",
    "thread/start",
    "thread/resume",
    "thread/fork",
    "thread/archive",
    "thread/unsubscribe",
    "thread/increment_elicitation",
    "thread/decrement_elicitation",
    "thread/name/set",
    "thread/metadata/update",
    "thread/unarchive",
    "thread/compact/start",
    "thread/backgroundTerminals/clean",
    "thread/rollback",
    "thread/list",
    "thread/loaded/list",
    "thread/read",
    "skills/list",
    "plugin/list",
    "skills/remote/list",
    "skills/remote/export",
    "app/list",
    "skills/config/write",
    "plugin/install",
    "plugin/uninstall",
    "turn/start",
    "turn/steer",
    "turn/interrupt",
    "thread/realtime/start",
    "thread/realtime/appendAudio",
    "thread/realtime/appendText",
    "thread/realtime/stop",
    "review/start",
    "model/list",
    "experimentalFeature/list",
    "collaborationMode/list",
    "mcpServer/oauth/login",
    "config/mcpServer/reload",
    "mcpServerStatus/list",
    "windowsSandbox/setupStart",
    "account/login/start",
    "account/login/cancel",
    "account/logout",
    "account/rateLimits/read",
    "feedback/upload",
    "command/exec",
    "command/exec/write",
    "command/exec/terminate",
    "command/exec/resize",
    "config/read",
    "externalAgentConfig/detect",
    "externalAgentConfig/import",
    "config/value/write",
    "config/batchWrite",
    "configRequirements/read",
    "account/read",
    "mock/experimentalMethod",
];

pub const SUPPORTED_SERVER_NOTIFICATION_METHODS: &[&str] = &[
    "error",
    "thread/started",
    "thread/status/changed",
    "thread/archived",
    "thread/unarchived",
    "thread/closed",
    "skills/changed",
    "thread/name/updated",
    "thread/tokenUsage/updated",
    "turn/started",
    "hook/started",
    "turn/completed",
    "hook/completed",
    "turn/diff/updated",
    "turn/plan/updated",
    "item/started",
    "item/completed",
    "rawResponseItem/completed",
    "item/agentMessage/delta",
    "item/plan/delta",
    "command/exec/outputDelta",
    "item/commandExecution/outputDelta",
    "item/commandExecution/terminalInteraction",
    "item/fileChange/outputDelta",
    "item/mcpToolCall/progress",
    "mcpServer/oauthLogin/completed",
    "account/updated",
    "account/rateLimits/updated",
    "account/login/completed",
    "app/list/updated",
    "item/reasoning/summaryTextDelta",
    "item/reasoning/summaryPartAdded",
    "item/reasoning/textDelta",
    "thread/compacted",
    "model/rerouted",
    "deprecationNotice",
    "configWarning",
    "fuzzyFileSearch/sessionUpdated",
    "fuzzyFileSearch/sessionCompleted",
    "thread/realtime/started",
    "thread/realtime/itemAdded",
    "thread/realtime/outputAudio/delta",
    "thread/realtime/error",
    "thread/realtime/closed",
    "serverRequest/resolved",
    "windows/worldWritableWarning",
    "windowsSandbox/setupCompleted",
];

pub const SUPPORTED_SERVER_REQUEST_METHODS: &[&str] = &[
    "item/commandExecution/requestApproval",
    "item/fileChange/requestApproval",
    "item/tool/requestUserInput",
    "mcpServer/elicitation/request",
    "item/permissions/requestApproval",
    "item/tool/call",
    "account/chatgptAuthTokens/refresh",
];

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(untagged)]
pub enum AppServerRequestId {
    String(String),
    Integer(i64),
}

#[derive(Debug)]
pub struct AppServerNotification {
    pub method: String,
    pub params: Option<Value>,
}

#[derive(Debug)]
pub struct AppServerRequest {
    pub id: AppServerRequestId,
    pub method: String,
    pub params: Option<Value>,
}

pub struct AppServerChannels {
    pub notifications: mpsc::Receiver<AppServerNotification>,
    pub requests: mpsc::Receiver<AppServerRequest>,
}

#[derive(Default)]
pub struct AppServerConfig {
    pub cwd: Option<PathBuf>,
    pub wire_log: Option<AppServerWireLog>,
    pub env: Vec<(String, String)>,
}

#[derive(Clone)]
pub struct AppServerWireLog {
    tx: mpsc::UnboundedSender<WireLogCommand>,
}

enum WireLogCommand {
    SetPath(PathBuf),
    Entry {
        direction: WireDirection,
        raw: String,
    },
}

#[derive(Clone, Copy)]
enum WireDirection {
    Inbound,
    Outbound,
}

impl AppServerWireLog {
    pub fn new() -> Self {
        let (tx, mut rx) = mpsc::unbounded_channel();
        tokio::spawn(async move {
            let mut file: Option<tokio::fs::File> = None;
            let mut buffer: Vec<String> = Vec::new();

            while let Some(cmd) = rx.recv().await {
                match cmd {
                    WireLogCommand::SetPath(path) => {
                        if let Some(parent) = path.parent() {
                            let _ = tokio::fs::create_dir_all(parent).await;
                        }
                        match tokio::fs::OpenOptions::new()
                            .create(true)
                            .append(true)
                            .open(&path)
                            .await
                        {
                            Ok(mut opened) => {
                                for line in buffer.drain(..) {
                                    let _ = opened.write_all(line.as_bytes()).await;
                                    let _ = opened.write_all(b"\n").await;
                                }
                                file = Some(opened);
                            }
                            Err(err) => {
                                tracing::warn!(error = %err, path = %path.display(), "Failed to open app-server wire log");
                            }
                        }
                    }
                    WireLogCommand::Entry { direction, raw } => {
                        let entry = serde_json::json!({
                            "timestamp_ms": current_timestamp_ms(),
                            "direction": match direction {
                                WireDirection::Inbound => "in",
                                WireDirection::Outbound => "out",
                            },
                            "raw": raw,
                        });
                        let line =
                            serde_json::to_string(&entry).unwrap_or_else(|_| "{}".to_string());
                        if let Some(file) = file.as_mut() {
                            let _ = file.write_all(line.as_bytes()).await;
                            let _ = file.write_all(b"\n").await;
                        } else {
                            buffer.push(line);
                        }
                    }
                }
            }
        });

        Self { tx }
    }

    pub fn set_path(&self, path: PathBuf) {
        let _ = self.tx.send(WireLogCommand::SetPath(path));
    }

    fn log_inbound(&self, raw: &str) {
        let _ = self.tx.send(WireLogCommand::Entry {
            direction: WireDirection::Inbound,
            raw: raw.to_string(),
        });
    }

    fn log_outbound(&self, raw: &str) {
        let _ = self.tx.send(WireLogCommand::Entry {
            direction: WireDirection::Outbound,
            raw: raw.to_string(),
        });
    }
}

impl Default for AppServerWireLog {
    fn default() -> Self {
        Self::new()
    }
}

struct AppServerTransport {
    stdin: Arc<Mutex<AppServerWriter>>,
    pending_requests: Arc<Mutex<PendingRequests>>,
    request_counter: AtomicI64,
    reader_task: Option<tokio::task::JoinHandle<()>>,
    wire_log: Option<AppServerWireLog>,
}

type PendingRequest = oneshot::Sender<Result<Value>>;
type PendingRequests = HashMap<AppServerRequestId, PendingRequest>;
type AppServerWriter = Box<dyn AsyncWrite + Send + Unpin>;
type AppServerReader = Box<dyn AsyncRead + Send + Unpin>;

impl AppServerTransport {
    fn new(
        stdin: AppServerWriter,
        stdout: AppServerReader,
        wire_log: Option<AppServerWireLog>,
    ) -> (Self, AppServerChannels) {
        let stdin = Arc::new(Mutex::new(stdin));
        let pending_requests: Arc<Mutex<PendingRequests>> =
            Arc::new(Mutex::new(PendingRequests::new()));
        let (notification_tx, notification_rx) = mpsc::channel(256);
        let (request_tx, request_rx) = mpsc::channel(64);

        let pending_requests_clone = pending_requests.clone();
        let notif_tx = notification_tx.clone();
        let req_tx = request_tx.clone();
        let wire_log_reader = wire_log.clone();
        let reader_task = tokio::spawn(async move {
            let reader = BufReader::new(stdout);
            let mut lines = reader.lines();

            while let Ok(Some(line)) = lines.next_line().await {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }
                if let Some(log) = &wire_log_reader {
                    log.log_inbound(trimmed);
                }

                let value: Value = match serde_json::from_str(trimmed) {
                    Ok(value) => value,
                    Err(err) => {
                        tracing::warn!(error = %err, line = %trimmed, "App-server JSON parse failed");
                        continue;
                    }
                };

                if let Some(method) = value.get("method").and_then(Value::as_str) {
                    let params = value.get("params").cloned();
                    if let Some(id_value) = value.get("id") {
                        match serde_json::from_value::<AppServerRequestId>(id_value.clone()) {
                            Ok(id) => {
                                if req_tx
                                    .send(AppServerRequest {
                                        id,
                                        method: method.to_string(),
                                        params,
                                    })
                                    .await
                                    .is_err()
                                {
                                    break;
                                }
                            }
                            Err(err) => {
                                tracing::warn!(
                                    error = %err,
                                    line = %trimmed,
                                    "App-server request id parse failed"
                                );
                            }
                        }
                    } else if notif_tx
                        .send(AppServerNotification {
                            method: method.to_string(),
                            params,
                        })
                        .await
                        .is_err()
                    {
                        break;
                    }
                    continue;
                }

                if let Some(id_value) = value.get("id") {
                    let id = match serde_json::from_value::<AppServerRequestId>(id_value.clone()) {
                        Ok(id) => id,
                        Err(err) => {
                            tracing::warn!(
                                error = %err,
                                line = %trimmed,
                                "App-server response id parse failed"
                            );
                            continue;
                        }
                    };

                    let result = if let Some(error) = value.get("error") {
                        let code = error.get("code").and_then(Value::as_i64).unwrap_or(-1);
                        let message = error
                            .get("message")
                            .and_then(Value::as_str)
                            .unwrap_or("Unknown error");
                        Err(anyhow::anyhow!("App-server error {}: {}", code, message))
                    } else if let Some(result) = value.get("result") {
                        Ok(result.clone())
                    } else {
                        Ok(Value::Null)
                    };

                    let mut pending = pending_requests_clone.lock().await;
                    if let Some(sender) = pending.remove(&id) {
                        let _ = sender.send(result);
                    } else {
                        tracing::warn!(?id, "App-server response without pending request");
                    }
                }
            }

            let mut pending = pending_requests_clone.lock().await;
            for (_, sender) in pending.drain() {
                let _ = sender.send(Err(anyhow::anyhow!("App-server connection closed")));
            }
        });

        (
            Self {
                stdin,
                pending_requests,
                request_counter: AtomicI64::new(1),
                reader_task: Some(reader_task),
                wire_log,
            },
            AppServerChannels {
                notifications: notification_rx,
                requests: request_rx,
            },
        )
    }

    async fn request<T, R>(&self, method: &str, params: Option<&T>) -> Result<R>
    where
        T: Serialize,
        R: DeserializeOwned,
    {
        let id = AppServerRequestId::Integer(self.request_counter.fetch_add(1, Ordering::SeqCst));
        let (tx, rx) = oneshot::channel();

        self.pending_requests.lock().await.insert(id.clone(), tx);

        let request = serde_json::json!({
            "id": id,
            "method": method,
            "params": params,
        });

        self.send_json(request).await?;

        let result = rx.await.context("App-server request canceled")??;
        serde_json::from_value(result).context("App-server response parse failed")
    }

    async fn notify<T>(&self, method: &str, params: Option<&T>) -> Result<()>
    where
        T: Serialize,
    {
        let notification = serde_json::json!({
            "method": method,
            "params": params,
        });
        self.send_json(notification).await
    }

    async fn respond<T>(&self, id: AppServerRequestId, result: &T) -> Result<()>
    where
        T: Serialize,
    {
        let response = serde_json::json!({
            "id": id,
            "result": result,
        });
        self.send_json(response).await
    }

    async fn send_json(&self, value: Value) -> Result<()> {
        let payload = serde_json::to_string(&value).context("App-server JSON encode failed")?;
        if let Some(log) = &self.wire_log {
            log.log_outbound(&payload);
        }
        let mut stdin = self.stdin.lock().await;
        stdin
            .write_all(payload.as_bytes())
            .await
            .context("App-server write failed")?;
        stdin
            .write_all(b"\n")
            .await
            .context("App-server write failed")?;
        stdin.flush().await.context("App-server flush failed")?;
        Ok(())
    }

    async fn shutdown(mut self) -> Result<()> {
        if let Some(task) = self.reader_task.take() {
            let _ = task.await;
        }
        Ok(())
    }
}

impl Drop for AppServerTransport {
    fn drop(&mut self) {
        if let Some(task) = self.reader_task.take() {
            task.abort();
        }
    }
}

pub struct AppServerClient {
    transport: AppServerTransport,
    process: Option<Child>,
}

impl AppServerClient {
    pub async fn spawn(config: AppServerConfig) -> Result<(Self, AppServerChannels)> {
        let command =
            resolve_app_server_command().context("Unable to find codex app-server executable")?;
        let resolved_program = command.program.clone();
        let resolved_args = command.args.clone();

        if let Some(version) = probe_executable_version(&resolved_program) {
            tracing::info!(
                program = %resolved_program.display(),
                args = ?resolved_args,
                version = %version,
                "Spawning codex app-server process"
            );
        } else {
            tracing::info!(
                program = %resolved_program.display(),
                args = ?resolved_args,
                "Spawning codex app-server process"
            );
        }

        let mut cmd = Command::new(command.program);
        cmd.args(command.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());

        if let Some(cwd) = config.cwd {
            cmd.current_dir(cwd);
        }

        for (key, value) in config.env {
            cmd.env(key, value);
        }

        let mut child = cmd.spawn().context("Failed to spawn codex app-server")?;
        let stdin = child
            .stdin
            .take()
            .context("codex app-server stdin missing")?;
        let stdout = child
            .stdout
            .take()
            .context("codex app-server stdout missing")?;

        let (transport, channels) =
            AppServerTransport::new(Box::new(stdin), Box::new(stdout), config.wire_log);

        Ok((
            Self {
                transport,
                process: Some(child),
            },
            channels,
        ))
    }

    #[allow(dead_code)]
    pub fn connect_with_io(
        stdin: AppServerWriter,
        stdout: AppServerReader,
        wire_log: Option<AppServerWireLog>,
    ) -> (Self, AppServerChannels) {
        let (transport, channels) = AppServerTransport::new(stdin, stdout, wire_log);
        (
            Self {
                transport,
                process: None,
            },
            channels,
        )
    }

    pub async fn initialize<T>(&self, params: T) -> Result<InitializeResponse>
    where
        T: Into<InitializeParams>,
    {
        let params = params.into();
        let response = self.transport.request("initialize", Some(&params)).await;
        match response {
            Ok(result) => {
                self.transport.notify::<Value>("initialized", None).await?;
                Ok(result)
            }
            Err(err) => {
                if format!("{err}").contains("Already initialized") {
                    self.transport.notify::<Value>("initialized", None).await?;
                    Ok(InitializeResponse {
                        user_agent: String::new(),
                    })
                } else {
                    Err(err)
                }
            }
        }
    }

    pub async fn thread_start(&self, params: ThreadStartParams) -> Result<ThreadStartResponse> {
        self.transport.request("thread/start", Some(&params)).await
    }

    pub async fn thread_resume(&self, params: ThreadResumeParams) -> Result<ThreadResumeResponse> {
        self.transport.request("thread/resume", Some(&params)).await
    }

    pub async fn thread_fork(&self, params: ThreadForkParams) -> Result<ThreadForkResponse> {
        self.transport.request("thread/fork", Some(&params)).await
    }

    pub async fn thread_read(&self, params: ThreadReadParams) -> Result<ThreadReadResponse> {
        self.transport.request("thread/read", Some(&params)).await
    }

    pub async fn thread_archive(
        &self,
        params: ThreadArchiveParams,
    ) -> Result<ThreadArchiveResponse> {
        self.transport
            .request("thread/archive", Some(&params))
            .await
    }

    pub async fn thread_list(&self, params: ThreadListParams) -> Result<ThreadListResponse> {
        self.transport.request("thread/list", Some(&params)).await
    }

    pub async fn thread_loaded_list(
        &self,
        params: ThreadLoadedListParams,
    ) -> Result<ThreadLoadedListResponse> {
        self.transport
            .request("thread/loaded/list", Some(&params))
            .await
    }

    pub async fn thread_unsubscribe(
        &self,
        params: ThreadUnsubscribeParams,
    ) -> Result<ThreadUnsubscribeResponse> {
        self.transport
            .request("thread/unsubscribe", Some(&params))
            .await
    }

    pub async fn thread_name_set(
        &self,
        params: ThreadSetNameParams,
    ) -> Result<ThreadSetNameResponse> {
        self.transport
            .request("thread/name/set", Some(&params))
            .await
    }

    pub async fn thread_unarchive(
        &self,
        params: ThreadUnarchiveParams,
    ) -> Result<ThreadUnarchiveResponse> {
        self.transport
            .request("thread/unarchive", Some(&params))
            .await
    }

    pub async fn thread_compact_start(
        &self,
        params: ThreadCompactStartParams,
    ) -> Result<ThreadCompactStartResponse> {
        self.transport
            .request("thread/compact/start", Some(&params))
            .await
    }

    pub async fn thread_background_terminals_clean(
        &self,
        params: ThreadBackgroundTerminalsCleanParams,
    ) -> Result<ThreadBackgroundTerminalsCleanResponse> {
        self.transport
            .request("thread/backgroundTerminals/clean", Some(&params))
            .await
    }

    pub async fn thread_rollback(
        &self,
        params: ThreadRollbackParams,
    ) -> Result<ThreadRollbackResponse> {
        self.transport
            .request("thread/rollback", Some(&params))
            .await
    }

    pub async fn model_list(&self, params: ModelListParams) -> Result<ModelListResponse> {
        self.transport.request("model/list", Some(&params)).await
    }

    pub async fn collaboration_mode_list(
        &self,
        params: CollaborationModeListParams,
    ) -> Result<CollaborationModeListResponse> {
        self.transport
            .request("collaborationMode/list", Some(&params))
            .await
    }

    pub async fn experimental_feature_list(
        &self,
        params: ExperimentalFeatureListParams,
    ) -> Result<ExperimentalFeatureListResponse> {
        self.transport
            .request("experimentalFeature/list", Some(&params))
            .await
    }

    pub async fn config_read(&self, params: ConfigReadParams) -> Result<ConfigReadResponse> {
        self.transport.request("config/read", Some(&params)).await
    }

    pub async fn config_value_write(
        &self,
        params: ConfigValueWriteParams,
    ) -> Result<ConfigWriteResponse> {
        self.transport
            .request("config/value/write", Some(&params))
            .await
    }

    #[allow(dead_code)]
    pub async fn config_batch_write(
        &self,
        params: ConfigBatchWriteParams,
    ) -> Result<ConfigWriteResponse> {
        self.transport
            .request("config/batchWrite", Some(&params))
            .await
    }

    pub async fn config_requirements_read(&self) -> Result<ConfigRequirementsReadResponse> {
        self.transport
            .request::<Option<()>, _>("configRequirements/read", None)
            .await
    }

    pub async fn external_agent_config_detect(
        &self,
        params: ExternalAgentConfigDetectParams,
    ) -> Result<ExternalAgentConfigDetectResponse> {
        self.transport
            .request("externalAgentConfig/detect", Some(&params))
            .await
    }

    pub async fn external_agent_config_import(
        &self,
        params: ExternalAgentConfigImportParams,
    ) -> Result<ExternalAgentConfigImportResponse> {
        self.transport
            .request("externalAgentConfig/import", Some(&params))
            .await
    }

    pub async fn feedback_upload(
        &self,
        params: FeedbackUploadParams,
    ) -> Result<FeedbackUploadResponse> {
        self.transport
            .request("feedback/upload", Some(&params))
            .await
    }

    pub async fn account_read(&self, params: GetAccountParams) -> Result<GetAccountResponse> {
        self.transport.request("account/read", Some(&params)).await
    }

    pub async fn account_login_start(
        &self,
        params: LoginAccountParams,
    ) -> Result<LoginAccountResponse> {
        self.transport
            .request("account/login/start", Some(&params))
            .await
    }

    pub async fn account_login_cancel(
        &self,
        params: CancelLoginAccountParams,
    ) -> Result<CancelLoginAccountResponse> {
        self.transport
            .request("account/login/cancel", Some(&params))
            .await
    }

    pub async fn account_logout(&self) -> Result<LogoutAccountResponse> {
        self.transport
            .request::<Value, _>("account/logout", None)
            .await
    }

    pub async fn account_rate_limits_read(&self) -> Result<GetAccountRateLimitsResponse> {
        self.transport
            .request::<Value, _>("account/rateLimits/read", None)
            .await
    }

    pub async fn mcp_server_status_list(
        &self,
        params: ListMcpServerStatusParams,
    ) -> Result<ListMcpServerStatusResponse> {
        self.transport
            .request("mcpServerStatus/list", Some(&params))
            .await
    }

    pub async fn mcp_server_oauth_login(
        &self,
        params: McpServerOauthLoginParams,
    ) -> Result<McpServerOauthLoginResponse> {
        self.transport
            .request("mcpServer/oauth/login", Some(&params))
            .await
    }

    pub async fn mcp_server_reload(&self) -> Result<McpServerRefreshResponse> {
        self.transport
            .request::<Option<()>, _>("config/mcpServer/reload", None)
            .await
    }

    pub async fn skills_list(&self, params: SkillsListParams) -> Result<SkillsListResponse> {
        self.transport.request("skills/list", Some(&params)).await
    }

    pub async fn skills_remote_list(
        &self,
        params: SkillsRemoteReadParams,
    ) -> Result<SkillsRemoteReadResponse> {
        self.transport
            .request("skills/remote/list", Some(&params))
            .await
    }

    pub async fn skills_remote_export(
        &self,
        params: SkillsRemoteWriteParams,
    ) -> Result<SkillsRemoteWriteResponse> {
        self.transport
            .request("skills/remote/export", Some(&params))
            .await
    }

    pub async fn app_list(&self, params: AppsListParams) -> Result<AppsListResponse> {
        self.transport.request("app/list", Some(&params)).await
    }

    pub async fn skills_config_write(
        &self,
        params: SkillsConfigWriteParams,
    ) -> Result<SkillsConfigWriteResponse> {
        self.transport
            .request("skills/config/write", Some(&params))
            .await
    }

    pub async fn turn_start(&self, params: TurnStartParams) -> Result<TurnStartResponse> {
        self.transport.request("turn/start", Some(&params)).await
    }

    pub async fn turn_interrupt(
        &self,
        params: TurnInterruptParams,
    ) -> Result<TurnInterruptResponse> {
        self.transport
            .request("turn/interrupt", Some(&params))
            .await
    }

    pub async fn turn_steer(&self, params: TurnSteerParams) -> Result<TurnSteerResponse> {
        self.transport.request("turn/steer", Some(&params)).await
    }

    pub async fn review_start(&self, params: ReviewStartParams) -> Result<ReviewStartResponse> {
        self.transport.request("review/start", Some(&params)).await
    }

    pub async fn thread_realtime_start(
        &self,
        params: ThreadRealtimeStartParams,
    ) -> Result<ThreadRealtimeStartResponse> {
        self.transport
            .request("thread/realtime/start", Some(&params))
            .await
    }

    pub async fn thread_realtime_append_audio(
        &self,
        params: ThreadRealtimeAppendAudioParams,
    ) -> Result<ThreadRealtimeAppendAudioResponse> {
        self.transport
            .request("thread/realtime/appendAudio", Some(&params))
            .await
    }

    pub async fn thread_realtime_append_text(
        &self,
        params: ThreadRealtimeAppendTextParams,
    ) -> Result<ThreadRealtimeAppendTextResponse> {
        self.transport
            .request("thread/realtime/appendText", Some(&params))
            .await
    }

    pub async fn thread_realtime_stop(
        &self,
        params: ThreadRealtimeStopParams,
    ) -> Result<ThreadRealtimeStopResponse> {
        self.transport
            .request("thread/realtime/stop", Some(&params))
            .await
    }

    pub async fn command_exec(&self, params: CommandExecParams) -> Result<CommandExecResponse> {
        self.transport.request("command/exec", Some(&params)).await
    }

    pub async fn windows_sandbox_setup_start(
        &self,
        params: WindowsSandboxSetupStartParams,
    ) -> Result<WindowsSandboxSetupStartResponse> {
        self.transport
            .request("windowsSandbox/setupStart", Some(&params))
            .await
    }

    pub async fn fuzzy_file_search_session_start(
        &self,
        params: FuzzyFileSearchSessionStartParams,
    ) -> Result<FuzzyFileSearchSessionStartResponse> {
        self.transport
            .request("fuzzyFileSearch/sessionStart", Some(&params))
            .await
    }

    pub async fn fuzzy_file_search_session_update(
        &self,
        params: FuzzyFileSearchSessionUpdateParams,
    ) -> Result<FuzzyFileSearchSessionUpdateResponse> {
        self.transport
            .request("fuzzyFileSearch/sessionUpdate", Some(&params))
            .await
    }

    pub async fn fuzzy_file_search_session_stop(
        &self,
        params: FuzzyFileSearchSessionStopParams,
    ) -> Result<FuzzyFileSearchSessionStopResponse> {
        self.transport
            .request("fuzzyFileSearch/sessionStop", Some(&params))
            .await
    }

    pub async fn mock_experimental_method(
        &self,
        params: MockExperimentalMethodParams,
    ) -> Result<MockExperimentalMethodResponse> {
        self.transport
            .request("mock/experimentalMethod", Some(&params))
            .await
    }

    pub async fn respond<T>(&self, id: AppServerRequestId, result: &T) -> Result<()>
    where
        T: Serialize,
    {
        self.transport.respond(id, result).await
    }

    pub async fn shutdown(mut self) -> Result<()> {
        if let Some(mut process) = self.process.take() {
            let _ = process.kill().await;
            let _ = process.wait().await;
        }
        self.transport.shutdown().await
    }
}

fn current_timestamp_ms() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

struct AppServerCommand {
    program: PathBuf,
    args: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CodexInstallationProbe {
    pub available: bool,
    pub invocation: Option<String>,
    pub resolved_program: Option<PathBuf>,
    pub version: Option<String>,
    pub error: Option<String>,
}

fn common_bin_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    let mut seen = HashSet::new();
    let mut push = |path: PathBuf| {
        if seen.insert(path.clone()) {
            dirs.push(path);
        }
    };

    if let Ok(home_override) = env::var("CODEX_HOME") {
        let trimmed = home_override.trim();
        if !trimmed.is_empty() {
            let root = PathBuf::from(trimmed);
            push(root.join("bin"));
            push(root);
        }
    }

    if let Some(home) = dirs::home_dir() {
        push(home.join(".codex/bin"));
        push(home.join(".codex"));
        push(home.join(".npm-global/bin"));
        push(home.join(".local/bin"));
        push(home.join(".local/share/mise/shims"));
        push(home.join(".cargo/bin"));
        push(home.join(".bun/bin"));
        push(home.join("node_modules/.bin"));

        let nvm_root = home.join(".nvm/versions/node");
        if let Ok(entries) = std::fs::read_dir(nvm_root) {
            for entry in entries.flatten() {
                let bin_path = entry.path().join("bin");
                if bin_path.is_dir() {
                    push(bin_path);
                }
            }
        }
    }

    for path in [
        "/opt/homebrew/bin",
        "/usr/local/bin",
        "/usr/bin",
        "/bin",
        "/usr/sbin",
        "/sbin",
    ] {
        push(PathBuf::from(path));
    }

    dirs
}

fn find_in_common_bins(binary: &str) -> Option<PathBuf> {
    for dir in common_bin_dirs() {
        let candidate = dir.join(binary);
        if candidate.exists() && candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

fn resolve_codex_bin_from_env() -> Option<PathBuf> {
    let value = env::var("CODEX_BIN").ok()?;
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    if trimmed.ends_with("codex-app-server") {
        return None;
    }
    let candidate = PathBuf::from(trimmed);
    if candidate.exists() && candidate.is_file() {
        return Some(candidate);
    }
    which::which(trimmed).ok()
}

fn resolve_codex_app_server_override() -> Option<PathBuf> {
    if let Ok(value) = env::var("CODEX_APP_SERVER") {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            let candidate = PathBuf::from(trimmed);
            if candidate.exists() && candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    if let Ok(value) = env::var("CODEX_BIN") {
        let trimmed = value.trim();
        if trimmed.ends_with("codex-app-server") {
            let candidate = PathBuf::from(trimmed);
            if candidate.exists() && candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    find_in_common_bins("codex-app-server")
}

fn resolve_app_server_command() -> Result<AppServerCommand> {
    if let Ok(program) = which::which("codex-app-server") {
        return Ok(AppServerCommand {
            program,
            args: Vec::new(),
        });
    }

    if let Some(program) = resolve_codex_app_server_override() {
        return Ok(AppServerCommand {
            program,
            args: Vec::new(),
        });
    }

    let program = find_codex_executable().context("codex executable not found")?;

    Ok(AppServerCommand {
        program,
        args: vec!["app-server".to_string()],
    })
}

fn find_codex_executable() -> Result<PathBuf> {
    if let Some(path) = resolve_codex_bin_from_env() {
        return Ok(path);
    }

    if let Ok(path) = which::which("codex") {
        return Ok(path);
    }

    if let Some(path) = find_in_common_bins("codex") {
        return Ok(path);
    }

    Err(anyhow::anyhow!("codex executable not found"))
}

/// Check if Codex app-server is available on this system.
pub fn is_codex_available() -> bool {
    probe_codex_installation().available
}

pub fn probe_codex_installation() -> CodexInstallationProbe {
    match resolve_app_server_command() {
        Ok(command) => CodexInstallationProbe {
            available: true,
            invocation: Some(format_command_invocation(&command)),
            resolved_program: Some(command.program.clone()),
            version: probe_executable_version(&command.program),
            error: None,
        },
        Err(error) => CodexInstallationProbe {
            available: false,
            invocation: None,
            resolved_program: None,
            version: None,
            error: Some(error.to_string()),
        },
    }
}

fn format_command_invocation(command: &AppServerCommand) -> String {
    let mut parts = vec![command.program.display().to_string()];
    parts.extend(command.args.iter().cloned());
    parts.join(" ")
}

fn probe_executable_version(program: &Path) -> Option<String> {
    let output = std::process::Command::new(program)
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .ok()?;

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if !stdout.is_empty() {
        return Some(stdout);
    }

    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if !stderr.is_empty() {
        return Some(stderr);
    }

    None
}

#[cfg(test)]
#[allow(
    clippy::panic,
    reason = "These installation-probe tests use explicit panic messages for fixture setup."
)]
mod tests {
    use super::{probe_codex_installation, resolve_app_server_command};
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;
    use std::sync::{Mutex, OnceLock};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn unique_temp_dir(tag: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        std::env::temp_dir().join(format!(
            "openagents-codex-client-{tag}-{}-{nanos}",
            std::process::id()
        ))
    }

    #[test]
    fn probe_codex_installation_reports_resolved_command_and_version() {
        let _guard = env_lock()
            .lock()
            .unwrap_or_else(|_| panic!("env lock poisoned"));
        let root = unique_temp_dir("probe");
        fs::create_dir_all(&root).unwrap_or_else(|_| panic!("failed to create temp dir"));
        let fake = root.join("codex-app-server");
        fs::write(
            &fake,
            "#!/bin/sh\nif [ \"$1\" = \"--version\" ]; then\n  printf 'codex-test 1.2.3\\n'\n  exit 0\nfi\nexit 0\n",
        )
        .unwrap_or_else(|_| panic!("failed to write fake codex-app-server"));
        let mut permissions = fs::metadata(&fake)
            .unwrap_or_else(|_| panic!("failed to stat fake codex-app-server"))
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&fake, permissions)
            .unwrap_or_else(|_| panic!("failed to chmod fake codex-app-server"));

        let original_path = std::env::var_os("PATH");
        let original_codex_bin = std::env::var_os("CODEX_BIN");
        let original_codex_app_server = std::env::var_os("CODEX_APP_SERVER");
        // Serialized by `env_lock`, so mutating process env stays deterministic here.
        unsafe {
            std::env::set_var("PATH", &root);
            std::env::remove_var("CODEX_BIN");
            std::env::remove_var("CODEX_APP_SERVER");
        }

        let resolved =
            resolve_app_server_command().unwrap_or_else(|_| panic!("expected fake command"));
        assert_eq!(resolved.program, fake);

        let probe = probe_codex_installation();
        assert!(probe.available);
        assert_eq!(probe.resolved_program.as_deref(), Some(fake.as_path()));
        let expected_invocation = fake.display().to_string();
        assert_eq!(
            probe.invocation.as_deref(),
            Some(expected_invocation.as_str())
        );
        assert_eq!(probe.version.as_deref(), Some("codex-test 1.2.3"));
        assert!(probe.error.is_none());

        unsafe {
            if let Some(value) = original_path {
                std::env::set_var("PATH", value);
            } else {
                std::env::remove_var("PATH");
            }
            if let Some(value) = original_codex_bin {
                std::env::set_var("CODEX_BIN", value);
            } else {
                std::env::remove_var("CODEX_BIN");
            }
            if let Some(value) = original_codex_app_server {
                std::env::set_var("CODEX_APP_SERVER", value);
            } else {
                std::env::remove_var("CODEX_APP_SERVER");
            }
        }
        let _ = fs::remove_file(&fake);
        let _ = fs::remove_dir_all(&root);
    }
}
